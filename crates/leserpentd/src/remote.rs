use std::fs;
use std::io::{BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use leserpent_protocol::{
    MAX_PROTOCOL_MESSAGE_BYTES, ResponseEnvelope, decode_request, encode_response,
};
use leserpent_runtime::ControlRuntime;
use rustls::{ServerConfig, ServerConnection, StreamOwned};

use crate::wire::{constant_time_equals, error_response, execute_request, validate_auth_token};

const MAX_HTTP_HEADER_BYTES: usize = 16 * 1024;
const MAX_CERTIFICATE_FILE_BYTES: u64 = 1024 * 1024;
const MAX_PRIVATE_KEY_FILE_BYTES: u64 = 64 * 1024;
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(3);

pub struct RemoteServer {
    listener: TcpListener,
    tls: Arc<ServerConfig>,
    token: Vec<u8>,
}

impl RemoteServer {
    pub fn bind(
        address: SocketAddr,
        certificate_path: impl AsRef<Path>,
        private_key_path: impl AsRef<Path>,
        token: &str,
    ) -> Result<Self, String> {
        validate_auth_token(token).map_err(|error| format!("remote {error}"))?;
        let certificate_path = certificate_path.as_ref();
        let private_key_path = private_key_path.as_ref();
        validate_regular_file(
            certificate_path,
            MAX_CERTIFICATE_FILE_BYTES,
            "TLS certificate",
        )?;
        validate_private_key_file(private_key_path)?;

        let mut certificate_reader = BufReader::new(
            fs::File::open(certificate_path)
                .map_err(|error| format!("cannot open TLS certificate: {error}"))?,
        );
        let certificates = rustls_pemfile::certs(&mut certificate_reader)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| "TLS certificate file contains invalid PEM".to_string())?;
        if certificates.is_empty() {
            return Err("TLS certificate file contains no certificates".into());
        }
        let mut key_reader = BufReader::new(
            fs::File::open(private_key_path)
                .map_err(|error| format!("cannot open TLS private key: {error}"))?,
        );
        let private_key = rustls_pemfile::private_key(&mut key_reader)
            .map_err(|_| "TLS private key file contains invalid PEM".to_string())?
            .ok_or_else(|| "TLS private key file contains no private key".to_string())?;
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let mut tls = ServerConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .map_err(|error| format!("cannot configure TLS protocol versions: {error}"))?
            .with_no_client_auth()
            .with_single_cert(certificates, private_key)
            .map_err(|_| "TLS certificate and private key do not form a valid pair".to_string())?;
        tls.alpn_protocols = vec![b"http/1.1".to_vec()];

        let listener = TcpListener::bind(address)
            .map_err(|error| format!("cannot bind remote HTTPS listener: {error}"))?;
        listener
            .set_nonblocking(true)
            .map_err(|error| format!("cannot configure remote HTTPS listener: {error}"))?;
        Ok(Self {
            listener,
            tls: Arc::new(tls),
            token: token.as_bytes().to_vec(),
        })
    }

    pub fn local_addr(&self) -> Result<SocketAddr, String> {
        self.listener
            .local_addr()
            .map_err(|error| error.to_string())
    }

    pub fn poll_once(&self, runtime: &mut ControlRuntime) -> Result<bool, String> {
        let (stream, _) = match self.listener.accept() {
            Ok(connection) => connection,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => return Ok(false),
            Err(error) => return Err(error.to_string()),
        };
        // Peer-controlled TLS and HTTP failures are isolated to this connection.
        let _ = self.handle(stream, runtime);
        Ok(true)
    }

    #[cfg(test)]
    fn poll_once_strict(&self, runtime: &mut ControlRuntime) -> Result<bool, String> {
        let (stream, _) = match self.listener.accept() {
            Ok(connection) => connection,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => return Ok(false),
            Err(error) => return Err(error.to_string()),
        };
        self.handle(stream, runtime)?;
        Ok(true)
    }

    fn handle(&self, stream: TcpStream, runtime: &mut ControlRuntime) -> Result<(), String> {
        stream
            .set_nonblocking(false)
            .map_err(|error| error.to_string())?;
        stream
            .set_read_timeout(Some(CONNECTION_TIMEOUT))
            .map_err(|error| error.to_string())?;
        stream
            .set_write_timeout(Some(CONNECTION_TIMEOUT))
            .map_err(|error| error.to_string())?;
        let connection = ServerConnection::new(Arc::clone(&self.tls))
            .map_err(|error| format!("cannot initialize TLS connection: {error}"))?;
        let mut stream = StreamOwned::new(connection, stream);
        let (status, response) = match read_http_request(&mut stream, &self.token) {
            Ok(bytes) => match decode_request(&bytes) {
                Ok(request) => (HttpStatus::Ok, execute_request(runtime, request)),
                Err(_) => (
                    HttpStatus::BadRequest,
                    error_response("invalid_request", "wire protocol request is invalid"),
                ),
            },
            Err(error) => (error.status, error_response(error.code, error.message)),
        };
        write_http_response(&mut stream, status, &response)?;
        stream.conn.send_close_notify();
        stream.flush().map_err(|error| error.to_string())
    }
}

#[derive(Clone, Copy, Debug)]
enum HttpStatus {
    Ok,
    BadRequest,
    Unauthorized,
    NotFound,
    MethodNotAllowed,
    PayloadTooLarge,
    UnsupportedMediaType,
}

impl HttpStatus {
    fn line(self) -> &'static str {
        match self {
            Self::Ok => "200 OK",
            Self::BadRequest => "400 Bad Request",
            Self::Unauthorized => "401 Unauthorized",
            Self::NotFound => "404 Not Found",
            Self::MethodNotAllowed => "405 Method Not Allowed",
            Self::PayloadTooLarge => "413 Payload Too Large",
            Self::UnsupportedMediaType => "415 Unsupported Media Type",
        }
    }
}

#[derive(Debug)]
struct HttpError {
    status: HttpStatus,
    code: &'static str,
    message: &'static str,
}

impl HttpError {
    fn bad_request() -> Self {
        Self {
            status: HttpStatus::BadRequest,
            code: "invalid_http_request",
            message: "HTTPS request is malformed",
        }
    }
}

fn read_http_request(stream: &mut impl Read, expected_token: &[u8]) -> Result<Vec<u8>, HttpError> {
    let mut bytes = Vec::new();
    let header_end = loop {
        if let Some(position) = find_header_end(&bytes) {
            if position > MAX_HTTP_HEADER_BYTES {
                return Err(HttpError {
                    status: HttpStatus::PayloadTooLarge,
                    code: "headers_too_large",
                    message: "HTTPS request headers are too large",
                });
            }
            break position;
        }
        if bytes.len() > MAX_HTTP_HEADER_BYTES {
            return Err(HttpError {
                status: HttpStatus::PayloadTooLarge,
                code: "headers_too_large",
                message: "HTTPS request headers are too large",
            });
        }
        let mut chunk = [0_u8; 1024];
        let read = stream
            .read(&mut chunk)
            .map_err(|_| HttpError::bad_request())?;
        if read == 0 {
            return Err(HttpError::bad_request());
        }
        bytes.extend_from_slice(&chunk[..read]);
    };

    let header =
        std::str::from_utf8(&bytes[..header_end - 4]).map_err(|_| HttpError::bad_request())?;
    let mut lines = header.split("\r\n");
    let request_line = lines.next().ok_or_else(HttpError::bad_request)?;
    let parts = request_line.split(' ').collect::<Vec<_>>();
    if parts.len() != 3 || parts[2] != "HTTP/1.1" {
        return Err(HttpError::bad_request());
    }

    let mut authorization = None;
    let mut content_length = None;
    let mut content_type = None;
    for line in lines {
        let (name, value) = line.split_once(':').ok_or_else(HttpError::bad_request)?;
        let value = value.trim();
        if name.eq_ignore_ascii_case("authorization") {
            if authorization.replace(value).is_some() {
                return Err(HttpError::bad_request());
            }
        } else if name.eq_ignore_ascii_case("content-length") {
            if content_length.replace(value).is_some() {
                return Err(HttpError::bad_request());
            }
        } else if name.eq_ignore_ascii_case("content-type") {
            if content_type.replace(value).is_some() {
                return Err(HttpError::bad_request());
            }
        } else if name.eq_ignore_ascii_case("transfer-encoding") {
            return Err(HttpError::bad_request());
        }
    }

    let supplied_token = authorization
        .and_then(|value| value.strip_prefix("Bearer "))
        .unwrap_or_default();
    if !constant_time_equals(supplied_token.as_bytes(), expected_token) {
        return Err(HttpError {
            status: HttpStatus::Unauthorized,
            code: "unauthorized",
            message: "remote authentication failed",
        });
    }
    if parts[0] != "POST" {
        return Err(HttpError {
            status: HttpStatus::MethodNotAllowed,
            code: "method_not_allowed",
            message: "remote wire endpoint requires POST",
        });
    }
    if parts[1] != "/v1/wire" {
        return Err(HttpError {
            status: HttpStatus::NotFound,
            code: "not_found",
            message: "remote endpoint was not found",
        });
    }
    if !content_type.is_some_and(|value| {
        value
            .split(';')
            .next()
            .is_some_and(|media_type| media_type.trim().eq_ignore_ascii_case("application/json"))
    }) {
        return Err(HttpError {
            status: HttpStatus::UnsupportedMediaType,
            code: "unsupported_media_type",
            message: "remote wire endpoint requires application/json",
        });
    }
    let content_length = content_length
        .ok_or_else(HttpError::bad_request)?
        .parse::<usize>()
        .map_err(|_| HttpError::bad_request())?;
    if content_length > MAX_PROTOCOL_MESSAGE_BYTES {
        return Err(HttpError {
            status: HttpStatus::PayloadTooLarge,
            code: "payload_too_large",
            message: "remote wire request is too large",
        });
    }

    let mut body = bytes.split_off(header_end);
    if body.len() < content_length {
        let missing = content_length - body.len();
        let mut remainder = vec![0_u8; missing];
        stream
            .read_exact(&mut remainder)
            .map_err(|_| HttpError::bad_request())?;
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

fn write_http_response(
    stream: &mut impl Write,
    status: HttpStatus,
    response: &ResponseEnvelope,
) -> Result<(), String> {
    let body = encode_response(response).map_err(|error| error.to_string())?;
    let challenge = if matches!(status, HttpStatus::Unauthorized) {
        "WWW-Authenticate: Bearer\r\n"
    } else {
        ""
    };
    let header = format!(
        "HTTP/1.1 {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\n{}Connection: close\r\n\r\n",
        status.line(),
        body.len(),
        challenge,
    );
    stream
        .write_all(header.as_bytes())
        .and_then(|()| stream.write_all(&body))
        .and_then(|()| stream.flush())
        .map_err(|error| error.to_string())
}

fn validate_regular_file(path: &Path, limit: u64, label: &str) -> Result<(), String> {
    let metadata =
        fs::symlink_metadata(path).map_err(|error| format!("cannot inspect {label}: {error}"))?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
        return Err(format!("{label} must be a regular file, not a symlink"));
    }
    if metadata.len() == 0 || metadata.len() > limit {
        return Err(format!("{label} has an invalid size"));
    }
    Ok(())
}

fn validate_private_key_file(path: &Path) -> Result<(), String> {
    validate_regular_file(path, MAX_PRIVATE_KEY_FILE_BYTES, "TLS private key")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mode = fs::metadata(path)
            .map_err(|error| format!("cannot inspect TLS private key permissions: {error}"))?
            .permissions()
            .mode();
        if mode & 0o077 != 0 {
            return Err("TLS private key permissions must not grant group or other access".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Read, Write};
    use std::net::TcpStream;
    #[cfg(unix)]
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::path::PathBuf;
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};

    use leserpent_protocol::{
        HealthRequest, PROTOCOL_SCHEMA_VERSION, ProtocolRequest, RequestEnvelope, decode_response,
        encode_request,
    };
    use rcgen::{CertifiedKey, generate_simple_self_signed};
    use rustls::pki_types::ServerName;
    use rustls::{ClientConfig, ClientConnection, RootCertStore};

    use super::*;

    const TOKEN: &str = "0123456789abcdef0123456789abcdef";

    fn temp_path(label: &str, extension: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "leserpentd-remote-{label}-{}-{unique}.{extension}",
            std::process::id()
        ))
    }

    fn health_body() -> Vec<u8> {
        encode_request(&RequestEnvelope {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            request: ProtocolRequest::Health(HealthRequest {}),
        })
        .unwrap()
    }

    fn request(token: &str, body: &[u8]) -> Vec<u8> {
        format!(
            "POST /v1/wire HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {token}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
            body.len()
        )
        .bytes()
        .chain(body.iter().copied())
            .collect()
    }

    fn read_response(stream: &mut impl Read) -> Vec<u8> {
        let mut response = Vec::new();
        while find_header_end(&response).is_none() {
            let mut byte = [0_u8; 1];
            stream.read_exact(&mut byte).unwrap();
            response.push(byte[0]);
        }
        let header_end = find_header_end(&response).unwrap();
        let header = std::str::from_utf8(&response[..header_end]).unwrap();
        let content_length = header
            .split("\r\n")
            .find_map(|line| {
                line.strip_prefix("Content-Length: ")
                    .and_then(|value| value.parse::<usize>().ok())
            })
            .unwrap();
        let mut body = vec![0_u8; content_length];
        stream.read_exact(&mut body).unwrap();
        response.extend_from_slice(&body);
        response
    }

    #[test]
    fn strict_http_parser_accepts_only_bounded_authenticated_json() {
        let body = health_body();
        assert_eq!(
            read_http_request(&mut Cursor::new(request(TOKEN, &body)), TOKEN.as_bytes()).unwrap(),
            body
        );

        let wrong_token = read_http_request(
            &mut Cursor::new(request("fedcba9876543210fedcba9876543210", &health_body())),
            TOKEN.as_bytes(),
        )
        .unwrap_err();
        assert!(matches!(wrong_token.status, HttpStatus::Unauthorized));

        let duplicate_auth = format!(
            "POST /v1/wire HTTP/1.1\r\nAuthorization: Bearer {TOKEN}\r\nAuthorization: Bearer {TOKEN}\r\nContent-Type: application/json\r\nContent-Length: 0\r\n\r\n"
        );
        assert!(matches!(
            read_http_request(
                &mut Cursor::new(duplicate_auth.into_bytes()),
                TOKEN.as_bytes()
            )
            .unwrap_err()
            .status,
            HttpStatus::BadRequest
        ));

        let chunked = format!(
            "POST /v1/wire HTTP/1.1\r\nAuthorization: Bearer {TOKEN}\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\n\r\n"
        );
        assert!(matches!(
            read_http_request(&mut Cursor::new(chunked.into_bytes()), TOKEN.as_bytes())
                .unwrap_err()
                .status,
            HttpStatus::BadRequest
        ));

        let oversized = format!(
            "POST /v1/wire HTTP/1.1\r\nAuthorization: Bearer {TOKEN}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
            MAX_PROTOCOL_MESSAGE_BYTES + 1
        );
        assert!(matches!(
            read_http_request(&mut Cursor::new(oversized.into_bytes()), TOKEN.as_bytes())
                .unwrap_err()
                .status,
            HttpStatus::PayloadTooLarge
        ));
    }

    #[test]
    fn authenticated_health_round_trips_over_real_tls() {
        let CertifiedKey { cert, signing_key } =
            generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
        let certificate_path = temp_path("roundtrip", "crt");
        let key_path = temp_path("roundtrip", "key");
        let database_path = temp_path("roundtrip", "sqlite");
        fs::write(&certificate_path, cert.pem()).unwrap();
        fs::write(&key_path, signing_key.serialize_pem()).unwrap();
        #[cfg(unix)]
        fs::set_permissions(&key_path, fs::Permissions::from_mode(0o600)).unwrap();

        let server = RemoteServer::bind(
            "127.0.0.1:0".parse().unwrap(),
            &certificate_path,
            &key_path,
            TOKEN,
        )
        .unwrap();
        let address = server.local_addr().unwrap();
        let certificate = cert.der().clone();
        let client = thread::spawn(move || {
            let mut roots = RootCertStore::empty();
            roots.add(certificate).unwrap();
            let provider = Arc::new(rustls::crypto::ring::default_provider());
            let mut config = ClientConfig::builder_with_provider(provider)
                .with_safe_default_protocol_versions()
                .unwrap()
                .with_root_certificates(roots)
                .with_no_client_auth();
            config.alpn_protocols = vec![b"http/1.1".to_vec()];
            let connection =
                ClientConnection::new(Arc::new(config), ServerName::try_from("localhost").unwrap())
                    .unwrap();
            let socket = TcpStream::connect(address).unwrap();
            socket.set_read_timeout(Some(CONNECTION_TIMEOUT)).unwrap();
            let mut stream = StreamOwned::new(connection, socket);
            stream.write_all(&request(TOKEN, &health_body())).unwrap();
            read_response(&mut stream)
        });

        let mut runtime = ControlRuntime::open(&database_path).unwrap();
        for _ in 0..100 {
            if server.poll_once_strict(&mut runtime).unwrap() {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
        let response = client.join().unwrap();
        assert!(response.starts_with(b"HTTP/1.1 200 OK\r\n"));
        let body_start = find_header_end(&response).unwrap();
        let decoded = decode_response(&response[body_start..]).unwrap();
        assert!(matches!(
            decoded.response,
            leserpent_protocol::ProtocolResponse::Health(ref health)
                if health.status == "ready" && health.authority_owned
        ));

        drop(runtime);
        fs::remove_file(certificate_path).unwrap();
        fs::remove_file(key_path).unwrap();
        fs::remove_file(database_path).unwrap();
    }

    #[test]
    fn malformed_tls_peer_is_isolated_from_runtime_authority() {
        let CertifiedKey { cert, signing_key } =
            generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
        let certificate_path = temp_path("bad-peer", "crt");
        let key_path = temp_path("bad-peer", "key");
        let database_path = temp_path("bad-peer", "sqlite");
        fs::write(&certificate_path, cert.pem()).unwrap();
        fs::write(&key_path, signing_key.serialize_pem()).unwrap();
        #[cfg(unix)]
        fs::set_permissions(&key_path, fs::Permissions::from_mode(0o600)).unwrap();

        let server = RemoteServer::bind(
            "127.0.0.1:0".parse().unwrap(),
            &certificate_path,
            &key_path,
            TOKEN,
        )
        .unwrap();
        let address = server.local_addr().unwrap();
        let peer = thread::spawn(move || {
            let mut stream = TcpStream::connect(address).unwrap();
            stream.write_all(b"plaintext is not accepted\r\n").unwrap();
        });
        let mut runtime = ControlRuntime::open(&database_path).unwrap();
        for _ in 0..100 {
            if server.poll_once(&mut runtime).unwrap() {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
        peer.join().unwrap();
        runtime.heartbeat().unwrap();

        drop(runtime);
        fs::remove_file(certificate_path).unwrap();
        fs::remove_file(key_path).unwrap();
        fs::remove_file(database_path).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn private_key_permissions_and_symlinks_fail_closed() {
        let key_path = temp_path("permissions", "key");
        let link_path = temp_path("permissions-link", "key");
        fs::write(&key_path, "not-secret-test-key").unwrap();
        fs::set_permissions(&key_path, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(validate_private_key_file(&key_path).is_err());
        fs::set_permissions(&key_path, fs::Permissions::from_mode(0o600)).unwrap();
        symlink(&key_path, &link_path).unwrap();
        assert!(validate_private_key_file(&link_path).is_err());
        fs::remove_file(link_path).unwrap();
        fs::remove_file(key_path).unwrap();
    }
}
