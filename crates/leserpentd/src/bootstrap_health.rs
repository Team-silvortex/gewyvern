use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use leserpent_protocol::transport_safety::{
    MAX_HTTP_HEADER_BYTES, connect_with_io_deadline, is_http_header_name,
};
use leserpent_protocol::{
    HealthRequest, MAX_PROTOCOL_MESSAGE_BYTES, PROTOCOL_SCHEMA_VERSION, ProtocolRequest,
    ProtocolResponse, RequestEnvelope, decode_response, encode_request,
};
use rustls::pki_types::{CertificateDer, ServerName, pem::PemObject};
use rustls::{ClientConfig, ClientConnection, RootCertStore, StreamOwned};
use zeroize::Zeroizing;

const HEALTH_DEADLINE: Duration = Duration::from_secs(8);
const ATTEMPT_TIMEOUT: Duration = Duration::from_millis(750);
const RETRY_INTERVAL: Duration = Duration::from_millis(100);

pub fn prove_bootstrap_health(endpoint: &str, ca_pem: &str, token: &str) -> Result<(), String> {
    prove_health(endpoint, ca_pem, token, HealthRoute::Loopback)
}

pub fn prove_remote_bootstrap_health(
    endpoint: &str,
    ca_pem: &str,
    token: &str,
) -> Result<(), String> {
    prove_health(endpoint, ca_pem, token, HealthRoute::Endpoint)
}

fn prove_health(
    endpoint: &str,
    ca_pem: &str,
    token: &str,
    route: HealthRoute,
) -> Result<(), String> {
    let endpoint = HealthEndpoint::parse(endpoint)?;
    let tls = client_config(ca_pem)?;
    let request = RequestEnvelope {
        schema_version: PROTOCOL_SCHEMA_VERSION,
        request: ProtocolRequest::Health(HealthRequest {}),
    };
    let body = encode_request(&request).map_err(|_| "health request encoding failed")?;
    let deadline = Instant::now() + HEALTH_DEADLINE;
    loop {
        if probe_once(&endpoint, route, Arc::clone(&tls), token, &body).is_ok() {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err("authenticated bootstrap health deadline expired".into());
        }
        thread::sleep(RETRY_INTERVAL);
    }
}

fn client_config(ca_pem: &str) -> Result<Arc<ClientConfig>, String> {
    let certificates = CertificateDer::pem_slice_iter(ca_pem.as_bytes())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "bootstrap CA is invalid")?;
    if certificates.is_empty() {
        return Err("bootstrap CA is empty".into());
    }
    let mut roots = RootCertStore::empty();
    for certificate in certificates {
        roots
            .add(certificate)
            .map_err(|_| "bootstrap CA certificate is invalid")?;
    }
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut config = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|_| "bootstrap TLS protocol setup failed")?
        .with_root_certificates(roots)
        .with_no_client_auth();
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    Ok(Arc::new(config))
}

fn probe_once(
    endpoint: &HealthEndpoint,
    route: HealthRoute,
    tls: Arc<ClientConfig>,
    token: &str,
    body: &[u8],
) -> Result<(), String> {
    let socket = match route {
        HealthRoute::Loopback => connect_with_io_deadline(
            SocketAddr::from((Ipv4Addr::LOCALHOST, endpoint.port)),
            ATTEMPT_TIMEOUT,
        ),
        HealthRoute::Endpoint => {
            connect_with_io_deadline((endpoint.host.as_str(), endpoint.port), ATTEMPT_TIMEOUT)
        }
    }
    .map_err(|_| "bootstrap health endpoint unavailable")?;
    let connection = ClientConnection::new(tls, endpoint.server_name.clone())
        .map_err(|_| "bootstrap health TLS setup failed")?;
    let mut stream = StreamOwned::new(connection, socket);
    let header = Zeroizing::new(format!(
        "POST /v1/wire HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nAccept: application/json\r\nConnection: close\r\n\r\n",
        endpoint.authority,
        token,
        body.len()
    ));
    stream
        .write_all(header.as_bytes())
        .and_then(|()| stream.write_all(body))
        .and_then(|()| stream.flush())
        .map_err(|_| "bootstrap health request failed")?;
    let response = read_health_response(&mut stream)?;
    match response.response {
        ProtocolResponse::Health(health)
            if health.status == "ready"
                && health.authority_owned
                && health.protocol_schema_version == PROTOCOL_SCHEMA_VERSION =>
        {
            Ok(())
        }
        _ => Err("bootstrap health response is not authoritative".into()),
    }
}

fn read_health_response(
    reader: &mut impl Read,
) -> Result<leserpent_protocol::ResponseEnvelope, String> {
    let mut bytes = Vec::new();
    let header_end = loop {
        if let Some(position) = find_header_end(&bytes) {
            break position;
        }
        if bytes.len() > MAX_HTTP_HEADER_BYTES {
            return Err("bootstrap health headers exceed the limit".into());
        }
        let mut chunk = [0_u8; 1024];
        let read = reader
            .read(&mut chunk)
            .map_err(|_| "bootstrap health response read failed")?;
        if read == 0 {
            return Err("bootstrap health response ended before headers".into());
        }
        bytes.extend_from_slice(&chunk[..read]);
    };
    if header_end > MAX_HTTP_HEADER_BYTES || !bytes[..header_end - 4].is_ascii() {
        return Err("bootstrap health headers are invalid".into());
    }
    let header = std::str::from_utf8(&bytes[..header_end - 4])
        .map_err(|_| "bootstrap health headers are invalid")?;
    let mut lines = header.split("\r\n");
    let mut status_line = lines
        .next()
        .ok_or("bootstrap health response has no status line")?
        .splitn(3, ' ');
    let status = if status_line.next() == Some("HTTP/1.1") {
        status_line
            .next()
            .and_then(|value| value.parse::<u16>().ok())
    } else {
        None
    };
    if status != Some(200) {
        return Err("bootstrap health returned a non-success status".into());
    }
    let mut content_length = None;
    let mut content_type = None;
    for line in lines {
        let (name, value) = line
            .split_once(':')
            .ok_or("bootstrap health response header is malformed")?;
        if !is_http_header_name(name) {
            return Err("bootstrap health response header is invalid".into());
        }
        let value = value.trim();
        if name.eq_ignore_ascii_case("content-length") {
            if content_length.replace(value).is_some() {
                return Err("bootstrap health repeats content length".into());
            }
        } else if name.eq_ignore_ascii_case("content-type") {
            if content_type.replace(value).is_some() {
                return Err("bootstrap health repeats content type".into());
            }
        } else if name.eq_ignore_ascii_case("transfer-encoding") {
            return Err("bootstrap health transfer encoding is unsupported".into());
        }
    }
    if !content_type.is_some_and(|value| {
        value
            .split(';')
            .next()
            .is_some_and(|media_type| media_type.trim().eq_ignore_ascii_case("application/json"))
    }) {
        return Err("bootstrap health response is not JSON".into());
    }
    let content_length = content_length
        .ok_or("bootstrap health response has no content length")?
        .parse::<usize>()
        .map_err(|_| "bootstrap health content length is invalid")?;
    if content_length > MAX_PROTOCOL_MESSAGE_BYTES {
        return Err("bootstrap health response exceeds the limit".into());
    }
    let mut body = bytes.split_off(header_end);
    if body.len() > content_length {
        return Err("bootstrap health response exceeds content length".into());
    }
    let initial_length = body.len();
    body.resize(content_length, 0);
    reader
        .read_exact(&mut body[initial_length..])
        .map_err(|_| "bootstrap health response body is incomplete")?;
    decode_response(&body).map_err(|_| "bootstrap health wire response is invalid".into())
}

fn find_header_end(bytes: &[u8]) -> Option<usize> {
    bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|position| position + 4)
}

struct HealthEndpoint {
    authority: String,
    host: String,
    port: u16,
    server_name: ServerName<'static>,
}

impl HealthEndpoint {
    fn parse(endpoint: &str) -> Result<Self, String> {
        let authority = endpoint
            .strip_prefix("https://")
            .ok_or("bootstrap health endpoint is not HTTPS")?;
        if authority.is_empty()
            || authority.len() > 320
            || authority.contains(['/', '?', '#', '@'])
            || authority.bytes().any(|byte| byte <= 0x20)
        {
            return Err("bootstrap health endpoint is invalid".into());
        }
        let (host, port) = if let Some(bracketed) = authority.strip_prefix('[') {
            let (host, suffix) = bracketed
                .split_once(']')
                .ok_or("bootstrap health endpoint is invalid")?;
            let port = suffix
                .strip_prefix(':')
                .map(parse_port)
                .transpose()?
                .unwrap_or(443);
            (host.to_string(), port)
        } else {
            let (host, port) = authority
                .rsplit_once(':')
                .map_or((authority, Ok(443)), |(host, port)| {
                    (host, parse_port(port))
                });
            (host.to_string(), port?)
        };
        let server_name = ServerName::try_from(host.clone())
            .map_err(|_| "bootstrap health TLS server name is invalid")?;
        Ok(Self {
            authority: authority.into(),
            host,
            port,
            server_name,
        })
    }
}

#[derive(Clone, Copy)]
enum HealthRoute {
    Loopback,
    Endpoint,
}

fn parse_port(value: &str) -> Result<u16, String> {
    value
        .parse::<u16>()
        .ok()
        .filter(|port| *port != 0)
        .ok_or_else(|| "bootstrap health endpoint port is invalid".into())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Cursor;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use leserpent_protocol::{HealthResponse, ResponseEnvelope, encode_response};
    use leserpent_runtime::ControlRuntime;
    use rcgen::{CertifiedKey, generate_simple_self_signed};

    use super::*;

    struct TempTree(PathBuf);

    impl TempTree {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "leserpent-bootstrap-health-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempTree {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn ready_response() -> Vec<u8> {
        let body = encode_response(&ResponseEnvelope {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            response: ProtocolResponse::Health(HealthResponse {
                status: "ready".into(),
                authority_owned: true,
                protocol_schema_version: PROTOCOL_SCHEMA_VERSION,
                effect_queue: None,
                runtime_unregistration_replay_horizon: None,
                orchestra_delete_replay_horizon: None,
            }),
        })
        .unwrap();
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
            body.len()
        )
        .into_bytes()
        .into_iter()
        .chain(body)
        .collect()
    }

    #[test]
    fn strict_health_response_requires_authoritative_wire_payload() {
        let response = read_health_response(&mut Cursor::new(ready_response())).unwrap();
        assert!(matches!(response.response, ProtocolResponse::Health(_)));

        let malformed = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nContent-Type: application/json\r\n\r\n0\r\n\r\n";
        assert!(read_health_response(&mut Cursor::new(malformed)).is_err());
    }

    #[test]
    fn endpoint_parser_keeps_tls_name_separate_from_loopback_transport() {
        let endpoint = HealthEndpoint::parse("https://host.example:7443").unwrap();
        assert_eq!(endpoint.authority, "host.example:7443");
        assert_eq!(endpoint.port, 7443);
        assert!(HealthEndpoint::parse("https://user@host.example:7443").is_err());
        assert!(HealthEndpoint::parse("http://host.example:7443").is_err());
    }

    #[test]
    fn health_probe_proves_real_tls_token_and_runtime_authority() {
        let temp = TempTree::new();
        let CertifiedKey { cert, signing_key } =
            generate_simple_self_signed(vec!["host.example".into(), "localhost".into()]).unwrap();
        let certificate = cert.pem();
        let certificate_path = temp.0.join("server.crt");
        let private_key_path = temp.0.join("server.key");
        fs::write(&certificate_path, &certificate).unwrap();
        fs::write(&private_key_path, signing_key.serialize_pem()).unwrap();
        #[cfg(unix)]
        fs::set_permissions(&private_key_path, fs::Permissions::from_mode(0o600)).unwrap();
        let token = "0123456789abcdef0123456789abcdef";
        let mut server = crate::RemoteServer::bind(
            "127.0.0.1:0".parse().unwrap(),
            certificate_path,
            private_key_path,
            token,
        )
        .unwrap();
        let endpoint = format!(
            "https://host.example:{}",
            server.local_addr().unwrap().port()
        );
        let probe_endpoint = endpoint.clone();
        let probe_ca = certificate.clone();
        let probe =
            thread::spawn(move || prove_bootstrap_health(&probe_endpoint, &probe_ca, token));
        let mut runtime = ControlRuntime::open(temp.0.join("runtime.sqlite")).unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        while !probe.is_finished() && Instant::now() < deadline {
            server.poll_once(&mut runtime).unwrap();
            thread::sleep(Duration::from_millis(10));
        }
        assert!(probe.join().unwrap().is_ok());

        let remote_endpoint = format!("https://localhost:{}", server.local_addr().unwrap().port());
        let remote_ca = certificate.clone();
        let remote_probe = thread::spawn(move || {
            prove_remote_bootstrap_health(&remote_endpoint, &remote_ca, token)
        });
        let deadline = Instant::now() + Duration::from_secs(3);
        while !remote_probe.is_finished() && Instant::now() < deadline {
            server.poll_once(&mut runtime).unwrap();
            thread::sleep(Duration::from_millis(10));
        }
        assert!(remote_probe.join().unwrap().is_ok());
    }
}
