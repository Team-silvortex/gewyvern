use std::collections::BTreeMap;
use std::fs;
use std::io::{BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use leserpent_domain::{
    RUNTIME_STATUS_REFRESH_EFFECT_KIND, RuntimeStatusObservation, RuntimeStatusRefreshRequest,
    RuntimeStatusSnapshot,
};
use leserpent_runtime::EffectExecution;
use rustls::pki_types::ServerName;
use rustls::{ClientConfig, ClientConnection, RootCertStore, StreamOwned};
use serde::Deserialize;
use zeroize::Zeroize;

use crate::{EffectAdapter, EmptySecretStore, SecretKey, SecretStore, validate_id};

pub const GEWYVERN_HEALTH_EFFECT_KIND: &str = "gewyvern.health.check";
pub const GEWYVERN_STATUS_REFRESH_EFFECT_KIND: &str = RUNTIME_STATUS_REFRESH_EFFECT_KIND;
const MAX_HTTP_HEADER_BYTES: usize = 16 * 1024;
const MAX_HTTP_BODY_BYTES: usize = 64 * 1024;
const MAX_HTTP_REQUEST_BODY_BYTES: usize = 16 * 1024;
const MAX_CA_FILE_BYTES: u64 = 1024 * 1024;

pub(crate) struct HttpJsonResponse {
    pub status: u16,
    pub body: Vec<u8>,
}

#[derive(Clone)]
pub struct GewyvernTarget {
    transport: GewyvernTransport,
    admin_secret: Option<SecretKey>,
}

#[derive(Clone)]
enum GewyvernTransport {
    Loopback(SocketAddr),
    Https {
        endpoint: HttpsEndpoint,
        tls: Arc<ClientConfig>,
    },
}

#[derive(Clone, Debug)]
struct HttpsEndpoint {
    host: String,
    port: u16,
    authority: String,
    server_name: ServerName<'static>,
}

impl std::fmt::Debug for GewyvernTarget {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let transport = match &self.transport {
            GewyvernTransport::Loopback(_) => "loopback-http",
            GewyvernTransport::Https { .. } => "remote-https",
        };
        formatter
            .debug_struct("GewyvernTarget")
            .field("transport", &transport)
            .field("authenticated", &self.admin_secret.is_some())
            .finish()
    }
}

impl GewyvernTarget {
    pub fn loopback(address: SocketAddr, admin_secret: Option<SecretKey>) -> Result<Self, String> {
        if !address.ip().is_loopback() {
            return Err("Gewyvern adapter currently permits loopback targets only".into());
        }
        Ok(Self {
            transport: GewyvernTransport::Loopback(address),
            admin_secret,
        })
    }

    pub fn https(
        origin: &str,
        ca_path: impl AsRef<Path>,
        admin_secret: SecretKey,
    ) -> Result<Self, String> {
        let endpoint = HttpsEndpoint::parse(origin)?;
        let tls = load_tls_config(ca_path.as_ref())?;
        Ok(Self {
            transport: GewyvernTransport::Https {
                endpoint,
                tls: Arc::new(tls),
            },
            admin_secret: Some(admin_secret),
        })
    }

    pub(crate) fn is_authenticated(&self) -> bool {
        self.admin_secret.is_some()
    }
}

impl HttpsEndpoint {
    fn parse(value: &str) -> Result<Self, String> {
        if value.len() > 2048 || value.bytes().any(|byte| byte <= 0x20) {
            return Err(invalid_https_origin());
        }
        let authority = value
            .strip_prefix("https://")
            .ok_or_else(invalid_https_origin)?;
        if authority.is_empty()
            || authority
                .bytes()
                .any(|byte| matches!(byte, b'/' | b'?' | b'#' | b'@'))
        {
            return Err(invalid_https_origin());
        }
        let (host, port, host_header) = if let Some(bracketed) = authority.strip_prefix('[') {
            let (host, suffix) = bracketed.split_once(']').ok_or_else(invalid_https_origin)?;
            let port = match suffix.strip_prefix(':') {
                Some(value) => parse_https_port(value)?,
                None if suffix.is_empty() => 443,
                None => return Err(invalid_https_origin()),
            };
            if host.is_empty() || host.parse::<std::net::Ipv6Addr>().is_err() {
                return Err(invalid_https_origin());
            }
            (host.to_string(), port, format!("[{host}]:{port}"))
        } else {
            if authority.matches(':').count() > 1 {
                return Err(invalid_https_origin());
            }
            let (host, port) = match authority.rsplit_once(':') {
                Some((host, port)) => (host, parse_https_port(port)?),
                None => (authority, 443),
            };
            if host.is_empty() {
                return Err(invalid_https_origin());
            }
            (host.to_string(), port, format!("{host}:{port}"))
        };
        let server_name = ServerName::try_from(host.clone()).map_err(|_| invalid_https_origin())?;
        Ok(Self {
            host,
            port,
            authority: host_header,
            server_name,
        })
    }
}

fn parse_https_port(value: &str) -> Result<u16, String> {
    let port = value.parse::<u16>().map_err(|_| invalid_https_origin())?;
    if port == 0 {
        return Err(invalid_https_origin());
    }
    Ok(port)
}

fn invalid_https_origin() -> String {
    "Gewyvern HTTPS origin must be https://HOST[:PORT] without path, query, or credentials".into()
}

fn load_tls_config(ca_path: &Path) -> Result<ClientConfig, String> {
    let metadata =
        fs::symlink_metadata(ca_path).map_err(|_| "Gewyvern CA file is unavailable".to_string())?;
    if metadata.file_type().is_symlink()
        || !metadata.file_type().is_file()
        || metadata.len() == 0
        || metadata.len() > MAX_CA_FILE_BYTES
    {
        return Err("Gewyvern CA must be a regular non-symlink file no larger than 1 MiB".into());
    }
    let mut reader = BufReader::new(
        fs::File::open(ca_path).map_err(|_| "Gewyvern CA file cannot be opened".to_string())?,
    );
    let certificates = rustls_pemfile::certs(&mut reader)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "Gewyvern CA contains invalid PEM".to_string())?;
    if certificates.is_empty() {
        return Err("Gewyvern CA contains no certificates".into());
    }
    let mut roots = RootCertStore::empty();
    for certificate in certificates {
        roots
            .add(certificate)
            .map_err(|_| "Gewyvern CA contains an invalid certificate".to_string())?;
    }
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut tls = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|error| format!("Gewyvern TLS setup failed: {error}"))?
        .with_root_certificates(roots)
        .with_no_client_auth();
    tls.alpn_protocols = vec![b"http/1.1".to_vec()];
    Ok(tls)
}

pub struct GewyvernHealthAdapter {
    targets: BTreeMap<String, GewyvernTarget>,
    secrets: Arc<dyn SecretStore>,
    timeout: Duration,
}

pub struct GewyvernStatusRefreshAdapter {
    targets: BTreeMap<String, GewyvernTarget>,
    secrets: Arc<dyn SecretStore>,
    timeout: Duration,
}

pub type GewyvernStatusRefreshRequest = RuntimeStatusRefreshRequest;

pub type GewyvernStatusObservation = RuntimeStatusObservation;

impl GewyvernHealthAdapter {
    pub fn new(
        targets: impl IntoIterator<Item = (String, GewyvernTarget)>,
    ) -> Result<Self, String> {
        Self::with_secret_store(targets, Arc::new(EmptySecretStore))
    }

    pub fn with_secret_store(
        targets: impl IntoIterator<Item = (String, GewyvernTarget)>,
        secrets: Arc<dyn SecretStore>,
    ) -> Result<Self, String> {
        Ok(Self {
            targets: normalize_targets(targets)?,
            secrets,
            timeout: Duration::from_secs(3),
        })
    }
}

impl GewyvernStatusRefreshAdapter {
    pub fn new(
        targets: impl IntoIterator<Item = (String, GewyvernTarget)>,
    ) -> Result<Self, String> {
        Self::with_secret_store(targets, Arc::new(EmptySecretStore))
    }

    pub fn with_secret_store(
        targets: impl IntoIterator<Item = (String, GewyvernTarget)>,
        secrets: Arc<dyn SecretStore>,
    ) -> Result<Self, String> {
        Ok(Self {
            targets: normalize_targets(targets)?,
            secrets,
            timeout: Duration::from_secs(3),
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HealthPayload {
    runtime_id: String,
}

impl EffectAdapter for GewyvernHealthAdapter {
    fn kind(&self) -> &str {
        GEWYVERN_HEALTH_EFFECT_KIND
    }

    fn execute(&mut self, payload: &[u8]) -> EffectExecution {
        let request: HealthPayload = match serde_json::from_slice(payload) {
            Ok(request) => request,
            Err(_) => return reject("invalid Gewyvern health payload"),
        };
        if validate_id("runtime_id", &request.runtime_id).is_err() {
            return reject("invalid Gewyvern runtime_id");
        }
        let Some(target) = self.targets.get(&request.runtime_id) else {
            return reject("Gewyvern runtime is not configured");
        };
        match fetch_health(target, self.secrets.as_ref(), self.timeout) {
            Ok(body) => EffectExecution::Complete(body),
            Err(error) => EffectExecution::Retry {
                error,
                after: Duration::from_secs(1),
            },
        }
    }
}

impl EffectAdapter for GewyvernStatusRefreshAdapter {
    fn kind(&self) -> &str {
        GEWYVERN_STATUS_REFRESH_EFFECT_KIND
    }

    fn execute(&mut self, payload: &[u8]) -> EffectExecution {
        let request: GewyvernStatusRefreshRequest = match serde_json::from_slice(payload) {
            Ok(request) => request,
            Err(_) => return reject("invalid Gewyvern status refresh payload"),
        };
        if validate_id("runtime_id", &request.runtime_id).is_err() {
            return reject("invalid Gewyvern runtime_id");
        }
        let Some(target) = self.targets.get(&request.runtime_id) else {
            return reject("Gewyvern runtime is not configured");
        };
        match fetch_status(target, self.secrets.as_ref(), self.timeout, &request) {
            Ok(observation) => match serde_json::to_vec(&observation) {
                Ok(encoded) => EffectExecution::Complete(encoded),
                Err(_) => reject("Gewyvern status observation cannot be encoded"),
            },
            Err(error) => EffectExecution::Retry {
                error,
                after: Duration::from_secs(1),
            },
        }
    }
}

pub(crate) fn normalize_targets(
    targets: impl IntoIterator<Item = (String, GewyvernTarget)>,
) -> Result<BTreeMap<String, GewyvernTarget>, String> {
    let mut normalized = BTreeMap::new();
    for (runtime_id, target) in targets {
        validate_id("runtime_id", &runtime_id)?;
        if normalized.insert(runtime_id.clone(), target).is_some() {
            return Err(format!("duplicate Gewyvern runtime '{runtime_id}'"));
        }
    }
    if normalized.is_empty() {
        return Err("Gewyvern adapter requires at least one target".into());
    }
    Ok(normalized)
}

fn fetch_health(
    target: &GewyvernTarget,
    secrets: &dyn SecretStore,
    timeout: Duration,
) -> Result<Vec<u8>, String> {
    let body = fetch_json(target, secrets, "/health", timeout)?;
    let health: serde_json::Value = serde_json::from_slice(&body)
        .map_err(|_| "Gewyvern health response JSON is invalid".to_string())?;
    if health.get("ok") != Some(&serde_json::Value::Bool(true)) {
        return Err("Gewyvern health response is not ready".into());
    }
    Ok(body)
}

#[derive(Deserialize)]
struct HealthDocument {
    ok: bool,
    has_snapshot: bool,
    kind: Option<String>,
    updated_unix_ms: u128,
    #[serde(default)]
    resilience_degraded: bool,
}

#[derive(Default, Deserialize)]
struct MetaDocument {
    target_count: Option<u64>,
    #[serde(default)]
    has_summary_json: bool,
    #[serde(default)]
    has_analysis_json: bool,
    #[serde(default)]
    has_training_example_json: bool,
    #[serde(default)]
    has_export_json: bool,
    #[serde(default)]
    has_report_json: bool,
    #[serde(default)]
    has_report_html: bool,
    #[serde(default)]
    has_external_sidecar_context: bool,
    #[serde(default)]
    has_external_evidence_chain_enrichment: bool,
    #[serde(default)]
    has_external_diagnostic_opinion: bool,
}

fn fetch_status(
    target: &GewyvernTarget,
    secrets: &dyn SecretStore,
    timeout: Duration,
    request: &GewyvernStatusRefreshRequest,
) -> Result<GewyvernStatusObservation, String> {
    let health: HealthDocument =
        serde_json::from_slice(&fetch_json(target, secrets, "/health", timeout)?)
            .map_err(|_| "Gewyvern health response JSON is invalid".to_string())?;
    if !health.ok {
        return Err("Gewyvern health response is not ready".into());
    }
    let meta: MetaDocument = if health.has_snapshot {
        serde_json::from_slice(&fetch_json(target, secrets, "/v1/latest/meta", timeout)?)
            .map_err(|_| "Gewyvern snapshot metadata JSON is invalid".to_string())?
    } else {
        MetaDocument::default()
    };
    Ok(GewyvernStatusObservation {
        runtime_id: request.runtime_id.clone(),
        expected_revision: request.expected_revision,
        status: RuntimeStatusSnapshot {
            status_source: "gewyvern-api".into(),
            status_fetched_at: Some(health.updated_unix_ms.to_string()),
            status_fetch_error: None,
            has_latest_snapshot: health.has_snapshot,
            snapshot_kind: health.kind,
            target_count: meta.target_count,
            has_summary_json: meta.has_summary_json,
            has_analysis_json: meta.has_analysis_json,
            has_training_example_json: meta.has_training_example_json,
            has_training_dataset_manifest: false,
            has_export_json: meta.has_export_json,
            has_report_json: meta.has_report_json,
            has_report_html: meta.has_report_html,
            has_external_sidecar_context: meta.has_external_sidecar_context,
            has_external_evidence_chain_enrichment: meta.has_external_evidence_chain_enrichment,
            has_external_diagnostic_opinion: meta.has_external_diagnostic_opinion,
            resilience_degraded: health.resilience_degraded,
            resilience_status: None,
            resilience_summary: None,
            socket_service_status: None,
            socket_consecutive_idle_timeouts: None,
            socket_total_idle_timeouts: None,
        },
    })
}

fn fetch_json(
    target: &GewyvernTarget,
    secrets: &dyn SecretStore,
    path: &str,
    timeout: Duration,
) -> Result<Vec<u8>, String> {
    let response = get_json(target, secrets, path, timeout)?;
    if response.status != 200 {
        return Err("Gewyvern API request was rejected".into());
    }
    Ok(response.body)
}

pub(crate) fn get_json(
    target: &GewyvernTarget,
    secrets: &dyn SecretStore,
    path: &str,
    timeout: Duration,
) -> Result<HttpJsonResponse, String> {
    request_json(target, secrets, "GET", path, None, timeout)
}

pub(crate) fn post_json(
    target: &GewyvernTarget,
    secrets: &dyn SecretStore,
    path: &str,
    body: &[u8],
    timeout: Duration,
) -> Result<HttpJsonResponse, String> {
    if body.len() > MAX_HTTP_REQUEST_BODY_BYTES {
        return Err("Gewyvern API request body is too large".into());
    }
    request_json(target, secrets, "POST", path, Some(body), timeout)
}

fn request_json(
    target: &GewyvernTarget,
    secrets: &dyn SecretStore,
    method: &str,
    path: &str,
    body: Option<&[u8]>,
    timeout: Duration,
) -> Result<HttpJsonResponse, String> {
    let admin_token = target
        .admin_secret
        .as_ref()
        .map(|key| {
            secrets
                .load(key)
                .map_err(|_| "Gewyvern admin secret is unavailable".to_string())?
                .ok_or_else(|| "Gewyvern admin secret is missing".to_string())
        })
        .transpose()?;
    if admin_token
        .as_ref()
        .is_some_and(|token| token.expose_secret().len() > 256)
    {
        return Err("Gewyvern admin secret is invalid".into());
    }
    match &target.transport {
        GewyvernTransport::Loopback(address) => {
            let mut stream = TcpStream::connect_timeout(address, timeout)
                .map_err(|_| "Gewyvern API connection failed".to_string())?;
            configure_socket(&stream, timeout)?;
            exchange_json(
                &mut stream,
                &address.to_string(),
                method,
                path,
                body,
                admin_token.as_ref(),
            )
        }
        GewyvernTransport::Https { endpoint, tls } => {
            let socket = connect_https(endpoint, timeout)?;
            configure_socket(&socket, timeout)?;
            let connection = ClientConnection::new(Arc::clone(tls), endpoint.server_name.clone())
                .map_err(|_| "Gewyvern TLS setup failed".to_string())?;
            let mut stream = StreamOwned::new(connection, socket);
            exchange_json(
                &mut stream,
                &endpoint.authority,
                method,
                path,
                body,
                admin_token.as_ref(),
            )
        }
    }
}

fn connect_https(endpoint: &HttpsEndpoint, timeout: Duration) -> Result<TcpStream, String> {
    let addresses = (endpoint.host.as_str(), endpoint.port)
        .to_socket_addrs()
        .map_err(|_| "Gewyvern HTTPS host cannot be resolved".to_string())?;
    for address in addresses.take(8) {
        if let Ok(stream) = TcpStream::connect_timeout(&address, timeout) {
            return Ok(stream);
        }
    }
    Err("Gewyvern API connection failed".into())
}

fn configure_socket(stream: &TcpStream, timeout: Duration) -> Result<(), String> {
    stream
        .set_read_timeout(Some(timeout))
        .map_err(|_| "Gewyvern API timeout setup failed".to_string())?;
    stream
        .set_write_timeout(Some(timeout))
        .map_err(|_| "Gewyvern API timeout setup failed".to_string())?;
    Ok(())
}

fn exchange_json(
    stream: &mut (impl Read + Write),
    authority: &str,
    method: &str,
    path: &str,
    body: Option<&[u8]>,
    admin_token: Option<&crate::SecretValue>,
) -> Result<HttpJsonResponse, String> {
    let mut request = format!(
        "{method} {path} HTTP/1.1\r\nHost: {authority}\r\nAccept: application/json\r\nConnection: close\r\n",
    );
    if let Some(token) = &admin_token {
        let token = token.expose_secret();
        request.push_str("X-Gewyvern-Admin-Token: ");
        request.push_str(token);
        request.push_str("\r\n");
    }
    if let Some(body) = body {
        request.push_str("Content-Type: application/json\r\nContent-Length: ");
        request.push_str(&body.len().to_string());
        request.push_str("\r\n");
    }
    request.push_str("\r\n");
    if let Some(body) = body {
        request.push_str(
            std::str::from_utf8(body)
                .map_err(|_| "Gewyvern API request body is not UTF-8".to_string())?,
        );
    }
    let write_result = stream.write_all(request.as_bytes());
    request.zeroize();
    write_result.map_err(|_| "Gewyvern API write failed".to_string())?;
    stream
        .flush()
        .map_err(|_| "Gewyvern API write failed".to_string())?;
    read_json_response(stream)
}

fn read_json_response(stream: &mut impl Read) -> Result<HttpJsonResponse, String> {
    let mut response = Vec::new();
    stream
        .take((MAX_HTTP_HEADER_BYTES + MAX_HTTP_BODY_BYTES + 5) as u64)
        .read_to_end(&mut response)
        .map_err(|_| "Gewyvern API read failed".to_string())?;
    if response.len() > MAX_HTTP_HEADER_BYTES + MAX_HTTP_BODY_BYTES + 4 {
        return Err("Gewyvern API response is too large".into());
    }
    let header_end = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| "Gewyvern API response is malformed".to_string())?;
    if header_end > MAX_HTTP_HEADER_BYTES {
        return Err("Gewyvern API response headers are too large".into());
    }
    let headers = std::str::from_utf8(&response[..header_end])
        .map_err(|_| "Gewyvern API response headers are invalid".to_string())?;
    let mut lines = headers.split("\r\n");
    let status = lines
        .next()
        .ok_or_else(|| "Gewyvern API response has no status".to_string())?;
    let mut status_parts = status.splitn(3, ' ');
    if status_parts.next() != Some("HTTP/1.1") {
        return Err("Gewyvern API response does not use HTTP/1.1".into());
    }
    let status = status_parts
        .next()
        .and_then(|value| value.parse::<u16>().ok())
        .filter(|value| (100..=599).contains(value))
        .ok_or_else(|| "Gewyvern API response status is invalid".to_string())?;
    let mut content_length = None;
    let mut content_type = None;
    for line in lines {
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| "Gewyvern API response header is malformed".to_string())?;
        let value = value.trim();
        if name.eq_ignore_ascii_case("content-length") {
            if content_length.replace(value).is_some() {
                return Err("Gewyvern API response repeats Content-Length".into());
            }
        } else if name.eq_ignore_ascii_case("content-type") {
            if content_type.replace(value).is_some() {
                return Err("Gewyvern API response repeats Content-Type".into());
            }
        } else if name.eq_ignore_ascii_case("transfer-encoding") {
            return Err("Gewyvern API response uses unsupported transfer encoding".into());
        }
    }
    if !content_type.is_some_and(|value| {
        value
            .split(';')
            .next()
            .is_some_and(|media_type| media_type.trim().eq_ignore_ascii_case("application/json"))
    }) {
        return Err("Gewyvern API response is not application/json".into());
    }
    let content_length = content_length
        .ok_or_else(|| "Gewyvern API response has no Content-Length".to_string())?
        .parse::<usize>()
        .map_err(|_| "Gewyvern API response Content-Length is invalid".to_string())?;
    if content_length > MAX_HTTP_BODY_BYTES {
        return Err("Gewyvern API response body is too large".into());
    }
    let body = &response[header_end + 4..];
    if body.len() != content_length {
        return Err("Gewyvern API response does not match Content-Length".into());
    }
    Ok(HttpJsonResponse {
        status,
        body: body.to_vec(),
    })
}

fn reject(error: &str) -> EffectExecution {
    EffectExecution::Reject {
        error: error.into(),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Cursor;
    use std::net::{IpAddr, Ipv4Addr, TcpListener};
    use std::path::PathBuf;
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};

    use leserpent_domain::Revision;
    use rcgen::{CertifiedKey, generate_simple_self_signed};
    use rustls::pki_types::{PrivateKeyDer, PrivatePkcs8KeyDer};
    use rustls::{ServerConfig, ServerConnection};

    use super::*;
    use crate::{ConfiguredSecretStore, SecretValue};

    fn serve_json(stream: &mut impl Write, body: &[u8]) {
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )
        .unwrap();
        stream.write_all(body).unwrap();
    }

    fn temp_path(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "leserpent-adapter-{label}-{}-{unique}.pem",
            std::process::id()
        ))
    }

    #[test]
    fn health_adapter_calls_bounded_loopback_api_with_configured_token() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 4096];
            let read = stream.read(&mut request).unwrap();
            let request = std::str::from_utf8(&request[..read]).unwrap();
            assert!(request.starts_with("GET /health HTTP/1.1\r\n"));
            assert!(request.contains("X-Gewyvern-Admin-Token: test-token\r\n"));
            let body = br#"{"ok":true,"has_snapshot":false}"#;
            serve_json(&mut stream, body);
        });
        let key = SecretKey::new("runtime-a-admin").unwrap();
        let secrets = Arc::new(
            ConfiguredSecretStore::new([(key.clone(), SecretValue::new("test-token").unwrap())])
                .unwrap(),
        );
        let target = GewyvernTarget::loopback(address, Some(key)).unwrap();
        let mut adapter =
            GewyvernHealthAdapter::with_secret_store([("runtime-a".to_string(), target)], secrets)
                .unwrap();
        let result = adapter.execute(br#"{"runtime_id":"runtime-a"}"#);
        let EffectExecution::Complete(body) = result else {
            panic!("health request should complete");
        };
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&body).unwrap()["ok"],
            true
        );
        server.join().unwrap();
    }

    #[test]
    fn health_adapter_authenticates_over_verified_https() {
        let CertifiedKey { cert, signing_key } =
            generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
        let ca_path = temp_path("gewyvern-ca");
        fs::write(&ca_path, cert.pem()).unwrap();
        let private_key =
            PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(signing_key.serialize_der()));
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let mut server_config = ServerConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_no_client_auth()
            .with_single_cert(vec![cert.der().clone()], private_key)
            .unwrap();
        server_config.alpn_protocols = vec![b"http/1.1".to_vec()];

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (socket, _) = listener.accept().unwrap();
            let connection = ServerConnection::new(Arc::new(server_config)).unwrap();
            let mut stream = StreamOwned::new(connection, socket);
            let mut request = [0_u8; 4096];
            let read = stream.read(&mut request).unwrap();
            let request = std::str::from_utf8(&request[..read]).unwrap();
            assert!(request.starts_with("GET /health HTTP/1.1\r\n"));
            assert!(request.contains(&format!("Host: localhost:{}\r\n", address.port())));
            assert!(request.contains("X-Gewyvern-Admin-Token: test-token\r\n"));
            serve_json(&mut stream, br#"{"ok":true,"has_snapshot":false}"#);
            stream.conn.send_close_notify();
            stream.flush().unwrap();
        });
        let key = SecretKey::new("runtime-a-admin").unwrap();
        let secrets = Arc::new(
            ConfiguredSecretStore::new([(key.clone(), SecretValue::new("test-token").unwrap())])
                .unwrap(),
        );
        let target = GewyvernTarget::https(
            &format!("https://localhost:{}", address.port()),
            &ca_path,
            key,
        )
        .unwrap();
        let mut adapter =
            GewyvernHealthAdapter::with_secret_store([("runtime-a".to_string(), target)], secrets)
                .unwrap();
        let result = adapter.execute(br#"{"runtime_id":"runtime-a"}"#);
        assert!(matches!(result, EffectExecution::Complete(_)), "{result:?}");
        server.join().unwrap();
        fs::remove_file(ca_path).unwrap();
    }

    #[test]
    fn response_parser_rejects_ambiguous_framing_and_non_json() {
        let body = br#"{"ok":true}"#;
        let valid = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
            body.len()
        )
        .into_bytes();
        let mut valid = [valid, body.to_vec()].concat();
        let response = read_json_response(&mut Cursor::new(&valid)).unwrap();
        assert_eq!(response.status, 200);
        assert_eq!(response.body, body);

        valid.extend_from_slice(b"x");
        assert!(read_json_response(&mut Cursor::new(&valid)).is_err());
        for response in [
            b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\nContent-Length: 2\r\n\r\n{}".as_slice(),
            b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\n".as_slice(),
            b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 2\r\n\r\n{}".as_slice(),
        ] {
            assert!(read_json_response(&mut Cursor::new(response)).is_err());
        }
    }

    #[test]
    fn https_target_rejects_non_https_origins_and_invalid_ca_files() {
        let key = SecretKey::new("runtime-a-admin").unwrap();
        for origin in [
            "http://localhost",
            "https://localhost/path",
            "https://user@localhost",
            "https://localhost:0",
        ] {
            assert!(GewyvernTarget::https(origin, "/missing", key.clone()).is_err());
        }
        let empty_ca = temp_path("empty-ca");
        fs::write(&empty_ca, []).unwrap();
        assert!(GewyvernTarget::https("https://localhost", &empty_ca, key).is_err());
        fs::remove_file(empty_ca).unwrap();
    }

    #[test]
    fn status_adapter_combines_health_and_snapshot_metadata() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let responses: [&[u8]; 2] = [
                br#"{"ok":true,"has_snapshot":true,"kind":"multi","updated_unix_ms":1234,"resilience_degraded":true}"#,
                br#"{"target_count":2,"has_summary_json":true,"has_analysis_json":true,"has_training_example_json":true,"has_export_json":true,"has_report_json":true,"has_report_html":false,"has_external_sidecar_context":true,"has_external_evidence_chain_enrichment":false,"has_external_diagnostic_opinion":true}"#,
            ];
            let paths = ["/health", "/v1/latest/meta"];
            for (path, body) in paths.into_iter().zip(responses) {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = [0_u8; 4096];
                let read = stream.read(&mut request).unwrap();
                let request = std::str::from_utf8(&request[..read]).unwrap();
                assert!(request.starts_with(&format!("GET {path} HTTP/1.1\r\n")));
                assert!(request.contains("X-Gewyvern-Admin-Token: test-token\r\n"));
                serve_json(&mut stream, body);
            }
        });
        let key = SecretKey::new("runtime-a-admin").unwrap();
        let secrets = Arc::new(
            ConfiguredSecretStore::new([(key.clone(), SecretValue::new("test-token").unwrap())])
                .unwrap(),
        );
        let target = GewyvernTarget::loopback(address, Some(key)).unwrap();
        let mut adapter = GewyvernStatusRefreshAdapter::with_secret_store(
            [("runtime-a".to_string(), target)],
            secrets,
        )
        .unwrap();
        let result = adapter.execute(br#"{"runtime_id":"runtime-a","expected_revision":7}"#);
        let EffectExecution::Complete(body) = result else {
            panic!("status refresh should complete");
        };
        let observation: GewyvernStatusObservation = serde_json::from_slice(&body).unwrap();
        assert_eq!(observation.runtime_id, "runtime-a");
        assert_eq!(observation.expected_revision, Revision(7));
        assert_eq!(observation.status.status_source, "gewyvern-api");
        assert_eq!(
            observation.status.status_fetched_at.as_deref(),
            Some("1234")
        );
        assert!(observation.status.has_latest_snapshot);
        assert_eq!(observation.status.snapshot_kind.as_deref(), Some("multi"));
        assert_eq!(observation.status.target_count, Some(2));
        assert!(observation.status.has_summary_json);
        assert!(observation.status.has_external_sidecar_context);
        assert!(observation.status.has_external_diagnostic_opinion);
        assert!(observation.status.resilience_degraded);
        assert!(!observation.status.has_report_html);
        server.join().unwrap();
    }

    #[test]
    fn status_payload_validation_fails_before_network_access() {
        let target = GewyvernTarget::loopback("127.0.0.1:9".parse().unwrap(), None).unwrap();
        let mut adapter =
            GewyvernStatusRefreshAdapter::new([("runtime-a".to_string(), target)]).unwrap();
        assert!(matches!(
            adapter.execute(br#"{"runtime_id":"runtime-a","expected_revision":1,"extra":true}"#),
            EffectExecution::Reject { .. }
        ));
        assert!(matches!(
            adapter.execute(br#"{"runtime_id":"missing","expected_revision":1}"#),
            EffectExecution::Reject { .. }
        ));
    }

    #[test]
    fn status_adapter_accepts_empty_runtime_without_requesting_metadata() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 4096];
            let read = stream.read(&mut request).unwrap();
            assert!(
                std::str::from_utf8(&request[..read])
                    .unwrap()
                    .starts_with("GET /health HTTP/1.1\r\n")
            );
            serve_json(
                &mut stream,
                br#"{"ok":true,"has_snapshot":false,"kind":null,"updated_unix_ms":0}"#,
            );
        });
        let target = GewyvernTarget::loopback(address, None).unwrap();
        let mut adapter =
            GewyvernStatusRefreshAdapter::new([("runtime-a".to_string(), target)]).unwrap();
        let EffectExecution::Complete(body) =
            adapter.execute(br#"{"runtime_id":"runtime-a","expected_revision":1}"#)
        else {
            panic!("empty runtime status should complete");
        };
        let observation: GewyvernStatusObservation = serde_json::from_slice(&body).unwrap();
        assert!(!observation.status.has_latest_snapshot);
        assert_eq!(observation.status.target_count, None);
        server.join().unwrap();
    }

    #[test]
    fn target_and_payload_validation_fail_before_network_access() {
        let remote = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1)), 9411);
        assert!(GewyvernTarget::loopback(remote, None).is_err());

        let target = GewyvernTarget::loopback("127.0.0.1:9".parse().unwrap(), None).unwrap();
        let mut adapter = GewyvernHealthAdapter::new([("runtime-a".to_string(), target)]).unwrap();
        assert!(matches!(
            adapter.execute(br#"{"runtime_id":"missing","extra":true}"#),
            EffectExecution::Reject { .. }
        ));
        assert!(matches!(
            adapter.execute(br#"{"runtime_id":"runtime-missing"}"#),
            EffectExecution::Reject { .. }
        ));
    }

    #[test]
    fn missing_configured_secret_fails_before_network_access() {
        let target = GewyvernTarget::loopback(
            "127.0.0.1:9".parse().unwrap(),
            Some(SecretKey::new("runtime-a-admin").unwrap()),
        )
        .unwrap();
        let mut adapter = GewyvernHealthAdapter::new([("runtime-a".to_string(), target)]).unwrap();
        assert!(matches!(
            adapter.execute(br#"{"runtime_id":"runtime-a"}"#),
            EffectExecution::Retry { error, .. } if error == "Gewyvern admin secret is missing"
        ));
    }
}
