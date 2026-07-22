use std::io::{BufReader, Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use leserpent_adapters::FileBootstrapTrustStore;
use leserpent_domain::bootstrap::CredentialHandle;
use leserpent_protocol::bootstrap::{
    BootstrapRequestEnvelope, BootstrapResponseEnvelope, MAX_BOOTSTRAP_PROTOCOL_BYTES,
    decode_bootstrap_response, encode_bootstrap_request,
};
use leserpent_protocol::provisioning::{
    MAX_PROVISIONING_PROTOCOL_BYTES, ProvisioningRequestEnvelope, ProvisioningResponseEnvelope,
    decode_provisioning_response, encode_provisioning_request,
};
use leserpent_protocol::transport_safety::{
    BoundedFile, MAX_HTTP_HEADER_BYTES, connect_with_deadline, is_http_header_name,
    open_bounded_regular_file,
};
use leserpent_protocol::{
    MAX_PROTOCOL_MESSAGE_BYTES, RequestEnvelope, ResponseEnvelope, decode_response, encode_request,
};
use rustls::pki_types::{CertificateDer, ServerName, pem::PemObject};
use rustls::{ClientConfig, ClientConnection, RootCertStore, StreamOwned};
use zeroize::Zeroizing;

use crate::CliError;

const MAX_CA_FILE_BYTES: u64 = 1024 * 1024;
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(3);

pub struct HttpsClient {
    endpoint: HttpsEndpoint,
    tls: Arc<ClientConfig>,
    token: Zeroizing<String>,
}

impl HttpsClient {
    pub fn new(endpoint: &str, ca_path: impl AsRef<Path>, token: String) -> Result<Self, CliError> {
        validate_remote_token(&token)?;
        let endpoint = HttpsEndpoint::parse(endpoint)?;
        let ca_path = ca_path.as_ref();
        let mut reader = BufReader::new(open_ca_file(ca_path)?);
        let certificates = CertificateDer::pem_reader_iter(&mut reader)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| CliError::Configuration("remote CA contains invalid PEM".into()))?;
        Self::from_certificates(endpoint, certificates, token)
    }

    pub fn new_with_bootstrap_trust(
        endpoint: &str,
        trust_root: impl AsRef<Path>,
        handle: &CredentialHandle,
        token: String,
    ) -> Result<Self, CliError> {
        let store = FileBootstrapTrustStore::new(trust_root.as_ref())
            .map_err(|_| CliError::Configuration("bootstrap trust store path is unsafe".into()))?;
        let record = store
            .load(handle)
            .map_err(|_| CliError::Configuration("bootstrap trust record is invalid".into()))?
            .ok_or_else(|| {
                CliError::Configuration("bootstrap trust record was not found".into())
            })?;
        if record.endpoint != endpoint {
            return Err(CliError::Configuration(
                "bootstrap trust record does not match the remote endpoint".into(),
            ));
        }
        validate_remote_token(&token)?;
        let endpoint = HttpsEndpoint::parse(endpoint)?;
        let certificates = CertificateDer::pem_slice_iter(record.ca_pem.as_bytes())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| CliError::Configuration("bootstrap trust CA is invalid".into()))?;
        Self::from_certificates(endpoint, certificates, token)
    }

    fn from_certificates(
        endpoint: HttpsEndpoint,
        certificates: Vec<CertificateDer<'static>>,
        token: String,
    ) -> Result<Self, CliError> {
        if certificates.is_empty() {
            return Err(CliError::Configuration(
                "remote CA contains no certificates".into(),
            ));
        }
        let mut roots = RootCertStore::empty();
        for certificate in certificates {
            roots.add(certificate).map_err(|_| {
                CliError::Configuration("remote CA contains an invalid certificate".into())
            })?;
        }
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let mut tls = ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .map_err(|error| CliError::Configuration(format!("TLS setup failed: {error}")))?
            .with_root_certificates(roots)
            .with_no_client_auth();
        tls.alpn_protocols = vec![b"http/1.1".to_vec()];
        Ok(Self {
            endpoint,
            tls: Arc::new(tls),
            token: Zeroizing::new(token),
        })
    }

    pub fn send(&self, request: &RequestEnvelope) -> Result<ResponseEnvelope, CliError> {
        let body =
            encode_request(request).map_err(|error| CliError::Protocol(error.to_string()))?;
        if body.len() > MAX_PROTOCOL_MESSAGE_BYTES {
            return Err(CliError::Protocol(
                "remote request exceeds the protocol limit".into(),
            ));
        }
        let response = self.send_json("/v1/wire", &body)?;
        decode_response(&response).map_err(|error| CliError::Protocol(format!("{error:?}")))
    }

    pub fn send_bootstrap(
        &self,
        request: &BootstrapRequestEnvelope,
    ) -> Result<BootstrapResponseEnvelope, CliError> {
        let body = encode_bootstrap_request(request)
            .map_err(|error| CliError::Protocol(error.to_string()))?;
        if body.len() > MAX_BOOTSTRAP_PROTOCOL_BYTES {
            return Err(CliError::Protocol(
                "remote bootstrap request exceeds the protocol limit".into(),
            ));
        }
        let response = self.send_json("/v1/bootstrap", &body)?;
        decode_bootstrap_response(&response)
            .map_err(|error| CliError::Protocol(format!("{error:?}")))
    }

    pub fn send_provisioning(
        &self,
        request: &ProvisioningRequestEnvelope,
    ) -> Result<ProvisioningResponseEnvelope, CliError> {
        let body = encode_provisioning_request(request)
            .map_err(|error| CliError::Protocol(error.to_string()))?;
        if body.len() > MAX_PROVISIONING_PROTOCOL_BYTES {
            return Err(CliError::Protocol(
                "remote provisioning request exceeds the protocol limit".into(),
            ));
        }
        let response = self.send_json("/v1/provisioning", &body)?;
        decode_provisioning_response(&response)
            .map_err(|error| CliError::Protocol(format!("{error:?}")))
    }

    fn send_json(&self, path: &str, body: &[u8]) -> Result<Vec<u8>, CliError> {
        let socket = connect_endpoint(&self.endpoint)?;
        let connection =
            ClientConnection::new(Arc::clone(&self.tls), self.endpoint.server_name.clone())
                .map_err(|error| CliError::Transport(format!("TLS setup failed: {error}")))?;
        let mut stream = StreamOwned::new(connection, socket);
        let header = Zeroizing::new(format!(
            "POST {path} HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nAccept: application/json\r\nConnection: close\r\n\r\n",
            self.endpoint.authority,
            self.token.as_str(),
            body.len(),
        ));
        stream
            .write_all(header.as_bytes())
            .and_then(|()| stream.write_all(body))
            .and_then(|()| stream.flush())
            .map_err(|error| CliError::Transport(error.to_string()))?;
        read_http_response_body(&mut stream)
    }
}

fn open_ca_file(ca_path: &Path) -> Result<BoundedFile, CliError> {
    open_bounded_regular_file(ca_path, MAX_CA_FILE_BYTES).map_err(|_| {
        CliError::Configuration(
            "remote CA must be a readable regular non-symlink file no larger than 1 MiB".into(),
        )
    })
}

fn connect_endpoint(endpoint: &HttpsEndpoint) -> Result<TcpStream, CliError> {
    connect_with_deadline((endpoint.host.as_str(), endpoint.port), CONNECTION_TIMEOUT)
        .map_err(|_| CliError::Transport("remote HTTPS endpoint is unavailable".into()))
}

struct HttpsEndpoint {
    host: String,
    port: u16,
    authority: String,
    server_name: ServerName<'static>,
}

impl HttpsEndpoint {
    fn parse(value: &str) -> Result<Self, CliError> {
        if value.len() > 2048 || value.bytes().any(|byte| byte <= 0x20) {
            return Err(invalid_endpoint());
        }
        let authority = value
            .strip_prefix("https://")
            .ok_or_else(invalid_endpoint)?;
        if authority.is_empty()
            || authority
                .bytes()
                .any(|byte| matches!(byte, b'/' | b'?' | b'#' | b'@'))
        {
            return Err(invalid_endpoint());
        }
        let (host, port, host_header) = if let Some(bracketed) = authority.strip_prefix('[') {
            let (host, suffix) = bracketed.split_once(']').ok_or_else(invalid_endpoint)?;
            let port = match suffix.strip_prefix(':') {
                Some(value) => parse_port(value)?,
                None if suffix.is_empty() => 443,
                None => return Err(invalid_endpoint()),
            };
            if host.is_empty() || host.parse::<std::net::Ipv6Addr>().is_err() {
                return Err(invalid_endpoint());
            }
            (host.to_string(), port, format!("[{host}]:{port}"))
        } else {
            if authority.matches(':').count() > 1 {
                return Err(invalid_endpoint());
            }
            let (host, port) = match authority.rsplit_once(':') {
                Some((host, port)) => (host, parse_port(port)?),
                None => (authority, 443),
            };
            if host.is_empty() {
                return Err(invalid_endpoint());
            }
            (host.to_string(), port, format!("{host}:{port}"))
        };
        let server_name = ServerName::try_from(host.clone()).map_err(|_| invalid_endpoint())?;
        Ok(Self {
            host,
            port,
            authority: host_header,
            server_name,
        })
    }
}

fn parse_port(value: &str) -> Result<u16, CliError> {
    let port = value.parse::<u16>().map_err(|_| invalid_endpoint())?;
    if port == 0 {
        return Err(invalid_endpoint());
    }
    Ok(port)
}

fn invalid_endpoint() -> CliError {
    CliError::Configuration(
        "remote endpoint must be https://HOST[:PORT] without path, query, or credentials".into(),
    )
}

fn validate_remote_token(token: &str) -> Result<(), CliError> {
    if token.len() < 32 || token.len() > 256 || token.bytes().any(|byte| byte <= 0x20) {
        return Err(CliError::Configuration(
            "LESERPENT_REMOTE_TOKEN must contain 32 to 256 non-whitespace bytes".into(),
        ));
    }
    Ok(())
}

fn read_http_response_body(stream: &mut impl Read) -> Result<Vec<u8>, CliError> {
    let mut bytes = Vec::new();
    let header_end = loop {
        if let Some(position) = find_header_end(&bytes) {
            if position > MAX_HTTP_HEADER_BYTES {
                return Err(CliError::Protocol(
                    "remote response headers exceed the limit".into(),
                ));
            }
            break position;
        }
        if bytes.len() > MAX_HTTP_HEADER_BYTES {
            return Err(CliError::Protocol(
                "remote response headers exceed the limit".into(),
            ));
        }
        let mut chunk = [0_u8; 1024];
        let read = stream
            .read(&mut chunk)
            .map_err(|error| CliError::Transport(error.to_string()))?;
        if read == 0 {
            return Err(CliError::Protocol(
                "remote response ended before its headers".into(),
            ));
        }
        bytes.extend_from_slice(&chunk[..read]);
    };
    let header_bytes = &bytes[..header_end - 4];
    if !header_bytes.is_ascii() {
        return Err(CliError::Protocol(
            "remote response headers are not valid ASCII".into(),
        ));
    }
    let header = std::str::from_utf8(header_bytes)
        .map_err(|_| CliError::Protocol("remote response headers are not valid ASCII".into()))?;
    let mut lines = header.split("\r\n");
    let status_line = lines
        .next()
        .ok_or_else(|| CliError::Protocol("remote response has no status line".into()))?;
    let mut status_parts = status_line.splitn(3, ' ');
    if status_parts.next() != Some("HTTP/1.1") {
        return Err(CliError::Protocol(
            "remote response does not use HTTP/1.1".into(),
        ));
    }
    let status = status_parts
        .next()
        .and_then(|value| value.parse::<u16>().ok())
        .ok_or_else(|| CliError::Protocol("remote response status is invalid".into()))?;
    if !matches!(status, 200 | 400 | 401 | 404 | 405 | 413 | 415) {
        return Err(CliError::Protocol(format!(
            "remote endpoint returned unsupported HTTP status {status}"
        )));
    }

    let mut content_length = None;
    let mut content_type = None;
    for line in lines {
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| CliError::Protocol("remote response header is malformed".into()))?;
        if !is_http_header_name(name) {
            return Err(CliError::Protocol(
                "remote response header name is invalid".into(),
            ));
        }
        let value = value.trim();
        if name.eq_ignore_ascii_case("content-length") {
            if content_length.replace(value).is_some() {
                return Err(CliError::Protocol(
                    "remote response repeats Content-Length".into(),
                ));
            }
        } else if name.eq_ignore_ascii_case("content-type") {
            if content_type.replace(value).is_some() {
                return Err(CliError::Protocol(
                    "remote response repeats Content-Type".into(),
                ));
            }
        } else if name.eq_ignore_ascii_case("transfer-encoding") {
            return Err(CliError::Protocol(
                "remote response uses unsupported transfer encoding".into(),
            ));
        }
    }
    if !content_type.is_some_and(|value| {
        value
            .split(';')
            .next()
            .is_some_and(|media_type| media_type.trim().eq_ignore_ascii_case("application/json"))
    }) {
        return Err(CliError::Protocol(
            "remote response is not application/json".into(),
        ));
    }
    let content_length = content_length
        .ok_or_else(|| CliError::Protocol("remote response has no Content-Length".into()))?
        .parse::<usize>()
        .map_err(|_| CliError::Protocol("remote response Content-Length is invalid".into()))?;
    if content_length > MAX_PROTOCOL_MESSAGE_BYTES {
        return Err(CliError::Protocol(
            "remote response exceeds the protocol limit".into(),
        ));
    }
    let mut body = bytes.split_off(header_end);
    if body.len() > content_length {
        return Err(CliError::Protocol(
            "remote response contains bytes beyond Content-Length".into(),
        ));
    }
    if body.len() < content_length {
        let mut remainder = vec![0_u8; content_length - body.len()];
        stream
            .read_exact(&mut remainder)
            .map_err(|error| CliError::Transport(error.to_string()))?;
        body.extend_from_slice(&remainder);
    }
    body.truncate(content_length);
    Ok(body)
}

fn find_header_end(bytes: &[u8]) -> Option<usize> {
    bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|position| position + 4)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Cursor;
    use std::net::TcpListener;
    #[cfg(unix)]
    use std::os::unix::fs::symlink;
    use std::time::{SystemTime, UNIX_EPOCH};

    use leserpent_adapters::{BootstrapTrustRecord, BootstrapTrustStore, FileBootstrapTrustStore};
    use leserpent_domain::bootstrap::CredentialHandle;
    use leserpent_protocol::{
        HealthResponse, PROTOCOL_SCHEMA_VERSION, ProtocolResponse, ResponseEnvelope,
        encode_response,
    };
    use ring::digest::{SHA256, digest};

    use super::*;

    fn hex(bytes: &[u8]) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut output = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            output.push(char::from(HEX[usize::from(byte >> 4)]));
            output.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
        output
    }

    #[test]
    fn endpoint_parser_accepts_dns_ipv4_and_bracketed_ipv6_only() {
        for endpoint in [
            "https://localhost",
            "https://example.com:9443",
            "https://127.0.0.1:9443",
            "https://[::1]:9443",
        ] {
            assert!(HttpsEndpoint::parse(endpoint).is_ok(), "{endpoint}");
        }
        for endpoint in [
            "http://localhost",
            "https://localhost/path",
            "https://user@localhost",
            "https://localhost:0",
            "https://::1:9443",
            "https://localhost?token=secret",
        ] {
            assert!(HttpsEndpoint::parse(endpoint).is_err(), "{endpoint}");
        }
    }

    #[test]
    fn bootstrap_trust_handle_loads_only_its_bound_endpoint() {
        let root = std::env::temp_dir().join(format!(
            "leserpent-cli-trust-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let store = FileBootstrapTrustStore::new(&root).unwrap();
        let handle = CredentialHandle::new("vault:leserpent-ca:host-example").unwrap();
        let ca = rcgen::generate_simple_self_signed(vec!["host.example".into()])
            .unwrap()
            .cert
            .pem();
        let record = BootstrapTrustRecord {
            endpoint: "https://host.example:7443".into(),
            ca_sha256: hex(digest(&SHA256, ca.as_bytes()).as_ref()),
            ca_pem: ca,
        };
        store.persist(&handle, &record).unwrap();
        let token = "0123456789abcdef0123456789abcdef".to_string();
        assert!(
            HttpsClient::new_with_bootstrap_trust(
                "https://host.example:7443",
                &root,
                &handle,
                token.clone()
            )
            .is_ok()
        );
        assert!(
            HttpsClient::new_with_bootstrap_trust(
                "https://other.example:7443",
                &root,
                &handle,
                token
            )
            .is_err()
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn endpoint_connection_uses_the_parsed_socket_candidates() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = HttpsEndpoint::parse(&format!(
            "https://127.0.0.1:{}",
            listener.local_addr().unwrap().port()
        ))
        .unwrap();

        assert!(connect_endpoint(&endpoint).is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn ca_loader_rejects_symlinks_at_open_time() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir();
        let target = root.join(format!("leserpent-cli-ca-target-{unique}.pem"));
        let link = root.join(format!("leserpent-cli-ca-link-{unique}.pem"));
        std::fs::write(&target, b"not-empty").unwrap();
        symlink(&target, &link).unwrap();

        assert!(open_ca_file(&link).is_err());
        std::fs::remove_file(link).unwrap();
        std::fs::remove_file(target).unwrap();
    }

    #[test]
    fn response_parser_rejects_ambiguous_redirected_and_oversized_http() {
        let body = encode_response(&ResponseEnvelope {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            response: ProtocolResponse::Health(HealthResponse {
                status: "ready".into(),
                authority_owned: true,
                protocol_schema_version: PROTOCOL_SCHEMA_VERSION,
                effect_queue: None,
            }),
        })
        .unwrap();
        let valid = response(
            "200 OK",
            &format!("Content-Length: {}\r\n", body.len()),
            &body,
        );
        assert!(read_http_response_body(&mut Cursor::new(valid)).is_ok());

        let duplicate = response(
            "200 OK",
            &format!(
                "Content-Length: {}\r\nContent-Length: {}\r\n",
                body.len(),
                body.len()
            ),
            &body,
        );
        assert!(read_http_response_body(&mut Cursor::new(duplicate)).is_err());
        let chunked = response("200 OK", "Transfer-Encoding: chunked\r\n", &body);
        assert!(read_http_response_body(&mut Cursor::new(chunked)).is_err());
        let disguised_chunked = response(
            "200 OK",
            &format!(
                "Transfer-Encoding : chunked\r\nContent-Length: {}\r\n",
                body.len()
            ),
            &body,
        );
        assert!(read_http_response_body(&mut Cursor::new(disguised_chunked)).is_err());
        let redirect = response(
            "302 Found",
            &format!("Content-Length: {}\r\n", body.len()),
            &body,
        );
        assert!(read_http_response_body(&mut Cursor::new(redirect)).is_err());
        let oversized = response(
            "200 OK",
            &format!("Content-Length: {}\r\n", MAX_PROTOCOL_MESSAGE_BYTES + 1),
            &[],
        );
        assert!(read_http_response_body(&mut Cursor::new(oversized)).is_err());
    }

    fn response(status: &str, framing: &str, body: &[u8]) -> Vec<u8> {
        format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\n{framing}Connection: close\r\n\r\n"
        )
        .bytes()
        .chain(body.iter().copied())
        .collect()
    }
}
