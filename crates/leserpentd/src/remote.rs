use std::io::{BufReader, Cursor, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use leserpent_protocol::transport_safety::{
    BoundedFile, MAX_HTTP_HEADER_BYTES, is_http_header_name, open_bounded_regular_file,
};
use leserpent_protocol::{
    MAX_PROTOCOL_MESSAGE_BYTES, ResponseEnvelope, decode_request, encode_response,
};
use leserpent_runtime::ControlRuntime;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject};
use rustls::{ServerConfig, ServerConnection, StreamOwned};
use zeroize::{Zeroize, Zeroizing};

use crate::events::{EventSession, MAX_EVENT_SESSIONS, is_event_upgrade};
use crate::wire::{constant_time_equals, error_response, execute_request, validate_auth_token};

const MAX_CERTIFICATE_FILE_BYTES: u64 = 1024 * 1024;
const MAX_PRIVATE_KEY_FILE_BYTES: u64 = 64 * 1024;
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(3);

pub(crate) type RemoteTlsStream = StreamOwned<ServerConnection, TcpStream>;

pub(crate) struct PrefixedStream<S> {
    prefix: Cursor<Vec<u8>>,
    inner: S,
}

impl<S> PrefixedStream<S> {
    pub(crate) fn new(prefix: Vec<u8>, inner: S) -> Self {
        Self {
            prefix: Cursor::new(prefix),
            inner,
        }
    }
}

impl<S: Read> Read for PrefixedStream<S> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let read = self.prefix.read(buffer)?;
        if read == 0 {
            self.prefix.get_mut().zeroize();
            self.inner.read(buffer)
        } else {
            Ok(read)
        }
    }
}

impl<S> Drop for PrefixedStream<S> {
    fn drop(&mut self) {
        self.prefix.get_mut().zeroize();
    }
}

impl<S: Write> Write for PrefixedStream<S> {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        self.inner.write(buffer)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

impl PrefixedStream<RemoteTlsStream> {
    pub(crate) fn set_nonblocking(&self, nonblocking: bool) -> std::io::Result<()> {
        self.inner.sock.set_nonblocking(nonblocking)
    }
}

pub struct RemoteServer {
    listener: TcpListener,
    tls: Arc<ServerConfig>,
    token: Zeroizing<Vec<u8>>,
    event_sessions: Vec<EventSession>,
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
        let certificate_file = open_regular_file(
            certificate_path,
            MAX_CERTIFICATE_FILE_BYTES,
            "TLS certificate",
        )?;
        let private_key_file = open_private_key_file(private_key_path)?;

        let mut certificate_reader = BufReader::new(certificate_file);
        let certificates = CertificateDer::pem_reader_iter(&mut certificate_reader)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| "TLS certificate file contains invalid PEM".to_string())?;
        if certificates.is_empty() {
            return Err("TLS certificate file contains no certificates".into());
        }
        let mut key_reader = BufReader::new(private_key_file);
        let private_key = PrivateKeyDer::from_pem_reader(&mut key_reader)
            .map_err(|_| "TLS private key file contains invalid PEM".to_string())?;
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
            token: Zeroizing::new(token.as_bytes().to_vec()),
            event_sessions: Vec::new(),
        })
    }

    pub fn local_addr(&self) -> Result<SocketAddr, String> {
        self.listener
            .local_addr()
            .map_err(|error| error.to_string())
    }

    pub fn poll_once(&mut self, runtime: &mut ControlRuntime) -> Result<bool, String> {
        let accepted = match self.listener.accept() {
            Ok((stream, _)) => {
                // Peer-controlled TLS, HTTP, and upgrade failures are isolated to this connection.
                if let Ok(Some(session)) = self.handle(stream, runtime)
                    && self.event_sessions.len() < MAX_EVENT_SESSIONS
                {
                    self.event_sessions.push(session);
                }
                true
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => false,
            Err(error) => return Err(error.to_string()),
        };
        self.poll_event_sessions(runtime);
        Ok(accepted)
    }

    #[cfg(test)]
    fn poll_once_strict(&mut self, runtime: &mut ControlRuntime) -> Result<bool, String> {
        let (stream, _) = match self.listener.accept() {
            Ok(connection) => connection,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                self.poll_event_sessions(runtime);
                return Ok(false);
            }
            Err(error) => return Err(error.to_string()),
        };
        if let Some(session) = self.handle(stream, runtime)? {
            if self.event_sessions.len() >= MAX_EVENT_SESSIONS {
                return Err("WebSocket event session limit reached".into());
            }
            self.event_sessions.push(session);
        }
        self.poll_event_sessions(runtime);
        Ok(true)
    }

    fn handle(
        &self,
        stream: TcpStream,
        runtime: &mut ControlRuntime,
    ) -> Result<Option<EventSession>, String> {
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
        let prefix =
            read_http_head(&mut stream).map_err(|_| "invalid HTTPS request".to_string())?;
        if is_event_upgrade(&prefix) {
            if self.event_sessions.len() >= MAX_EVENT_SESSIONS {
                return Err("WebSocket event session limit reached".into());
            }
            return EventSession::upgrade(stream, prefix, &self.token).map(Some);
        }
        let mut stream = PrefixedStream::new(prefix, stream);
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
        stream.inner.conn.send_close_notify();
        stream.flush().map_err(|error| error.to_string())?;
        Ok(None)
    }

    fn poll_event_sessions(&mut self, runtime: &ControlRuntime) {
        let (revision, runtimes) = runtime.runtime_event_state();
        self.event_sessions
            .retain_mut(|session| session.poll(revision, &runtimes));
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
    let mut bytes = Zeroizing::new(Vec::new());
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

    let header_bytes = &bytes[..header_end - 4];
    if !header_bytes.is_ascii() {
        return Err(HttpError::bad_request());
    }
    let header = std::str::from_utf8(header_bytes).map_err(|_| HttpError::bad_request())?;
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
        if !is_http_header_name(name) {
            return Err(HttpError::bad_request());
        }
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
    if body.len() > content_length {
        return Err(HttpError::bad_request());
    }
    if body.len() < content_length {
        let missing = content_length - body.len();
        let mut remainder = vec![0_u8; missing];
        stream
            .read_exact(&mut remainder)
            .map_err(|_| HttpError::bad_request())?;
        body.extend_from_slice(&remainder);
    }
    Ok(body)
}

fn read_http_head(stream: &mut impl Read) -> Result<Vec<u8>, HttpError> {
    let mut bytes = Vec::new();
    loop {
        if find_header_end(&bytes).is_some() {
            return Ok(bytes);
        }
        if bytes.len() > MAX_HTTP_HEADER_BYTES {
            return Err(HttpError {
                status: HttpStatus::PayloadTooLarge,
                code: "headers_too_large",
                message: "HTTPS request headers are too large",
            });
        }
        let remaining = MAX_HTTP_HEADER_BYTES + 1 - bytes.len();
        let mut chunk = [0_u8; 1024];
        let capacity = remaining.min(chunk.len());
        let read = stream
            .read(&mut chunk[..capacity])
            .map_err(|_| HttpError::bad_request())?;
        if read == 0 {
            return Err(HttpError::bad_request());
        }
        bytes.extend_from_slice(&chunk[..read]);
    }
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

fn open_regular_file(path: &Path, limit: u64, label: &str) -> Result<BoundedFile, String> {
    open_bounded_regular_file(path, limit)
        .map_err(|error| format!("cannot open bounded {label}: {error}"))
}

fn open_private_key_file(path: &Path) -> Result<BoundedFile, String> {
    let file = open_regular_file(path, MAX_PRIVATE_KEY_FILE_BYTES, "TLS private key")?;
    #[cfg(unix)]
    {
        let mode = file
            .metadata()
            .map_err(|error| format!("cannot inspect TLS private key permissions: {error}"))?
            .permissions()
            .mode();
        if mode & 0o077 != 0 {
            return Err("TLS private key permissions must not grant group or other access".into());
        }
    }
    Ok(file)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::{Cursor, Read, Write};
    use std::net::TcpStream;
    #[cfg(unix)]
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::path::PathBuf;
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};

    use leserpent_domain::{Revision, RuntimeId};
    use leserpent_protocol::{
        HealthRequest, PROTOCOL_SCHEMA_VERSION, ProtocolEvent, ProtocolRequest, RequestEnvelope,
        decode_event, decode_response, encode_request,
    };
    use rcgen::{CertifiedKey, generate_simple_self_signed};
    use rustls::pki_types::ServerName;
    use rustls::{ClientConfig, ClientConnection, RootCertStore};
    use tungstenite::client::{IntoClientRequest, client_with_config};
    use tungstenite::http::HeaderValue;
    use tungstenite::protocol::WebSocketConfig;

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
    fn consumed_http_prefix_is_zeroized_before_reading_the_socket() {
        let mut stream = PrefixedStream::new(
            b"Authorization: Bearer secret\r\n\r\n".to_vec(),
            Cursor::new(b"body".to_vec()),
        );
        let mut output = Vec::new();
        stream.read_to_end(&mut output).unwrap();

        assert_eq!(output, b"Authorization: Bearer secret\r\n\r\nbody");
        assert!(stream.prefix.get_ref().iter().all(|byte| *byte == 0));
    }

    fn event_client(
        address: SocketAddr,
        certificate: rustls::pki_types::CertificateDer<'static>,
        after_revision: u64,
        expected_messages: usize,
    ) -> Vec<Vec<u8>> {
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
        socket.set_write_timeout(Some(CONNECTION_TIMEOUT)).unwrap();
        let stream = StreamOwned::new(connection, socket);
        let mut request = format!(
            "wss://localhost:{}/v1/events?after_revision={after_revision}",
            address.port()
        )
        .into_client_request()
        .unwrap();
        request.headers_mut().insert(
            "Authorization",
            HeaderValue::from_str(&format!("Bearer {TOKEN}")).unwrap(),
        );
        request.headers_mut().insert(
            "Sec-WebSocket-Protocol",
            HeaderValue::from_static("leserpent.events.v1"),
        );
        let websocket_config = WebSocketConfig::default()
            .max_message_size(Some(MAX_PROTOCOL_MESSAGE_BYTES))
            .max_frame_size(Some(MAX_PROTOCOL_MESSAGE_BYTES));
        let (mut websocket, response) =
            client_with_config(request, stream, Some(websocket_config)).unwrap();
        assert_eq!(response.status(), 101);
        assert_eq!(
            response.headers().get("Sec-WebSocket-Protocol").unwrap(),
            "leserpent.events.v1"
        );
        let mut messages = Vec::new();
        for _ in 0..expected_messages {
            messages.push(websocket.read().unwrap().into_data().to_vec());
        }
        websocket.close(None).unwrap();
        messages
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

        let disguised_chunked = format!(
            "POST /v1/wire HTTP/1.1\r\nAuthorization: Bearer {TOKEN}\r\nContent-Type: application/json\r\nTransfer-Encoding : chunked\r\nContent-Length: 0\r\n\r\n"
        );
        assert!(matches!(
            read_http_request(
                &mut Cursor::new(disguised_chunked.into_bytes()),
                TOKEN.as_bytes()
            )
            .unwrap_err()
            .status,
            HttpStatus::BadRequest
        ));

        let mut trailing = request(TOKEN, &body);
        trailing.push(b'x');
        assert!(matches!(
            read_http_request(&mut Cursor::new(trailing), TOKEN.as_bytes())
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

        let mut server = RemoteServer::bind(
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
    fn authenticated_websocket_streams_redacted_revisioned_snapshots_and_resyncs() {
        let CertifiedKey { cert, signing_key } =
            generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
        let certificate_path = temp_path("events", "crt");
        let key_path = temp_path("events", "key");
        let database_path = temp_path("events", "sqlite");
        fs::write(&certificate_path, cert.pem()).unwrap();
        fs::write(&key_path, signing_key.serialize_pem()).unwrap();
        #[cfg(unix)]
        fs::set_permissions(&key_path, fs::Permissions::from_mode(0o600)).unwrap();

        let mut server = RemoteServer::bind(
            "127.0.0.1:0".parse().unwrap(),
            &certificate_path,
            &key_path,
            TOKEN,
        )
        .unwrap();
        let address = server.local_addr().unwrap();
        let certificate = cert.der().clone();
        let mut runtime = ControlRuntime::open(&database_path).unwrap();
        runtime
            .register_runtime(
                RuntimeId::new("runtime-a").unwrap(),
                "Runtime A",
                "https://secret-runtime.invalid",
            )
            .unwrap();

        let first_client = thread::spawn(move || event_client(address, certificate, 0, 2));
        for _ in 0..100 {
            if server.poll_once_strict(&mut runtime).unwrap() {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
        runtime
            .register_runtime(
                RuntimeId::new("runtime-b").unwrap(),
                "Runtime B",
                "https://another-secret.invalid",
            )
            .unwrap();
        for _ in 0..10 {
            server.poll_once_strict(&mut runtime).unwrap();
            thread::sleep(Duration::from_millis(5));
        }
        let messages = first_client.join().unwrap();
        assert_eq!(messages.len(), 2);
        assert!(messages.iter().all(|message| {
            !String::from_utf8_lossy(message).contains("secret-runtime.invalid")
                && !String::from_utf8_lossy(message).contains("another-secret.invalid")
        }));
        let first = decode_event(&messages[0]).unwrap();
        assert!(matches!(
            first.event,
            ProtocolEvent::RuntimeSnapshot {
                revision: Revision(1),
                resumed_after: Some(Revision(0)),
                ref runtimes,
            } if runtimes.len() == 1 && runtimes[0].id.as_str() == "runtime-a"
        ));
        let second = decode_event(&messages[1]).unwrap();
        assert!(matches!(
            second.event,
            ProtocolEvent::RuntimeSnapshot {
                revision: Revision(2),
                resumed_after: Some(Revision(1)),
                ref runtimes,
            } if runtimes.len() == 2
        ));

        let address = server.local_addr().unwrap();
        let certificate = cert.der().clone();
        let resync_client = thread::spawn(move || event_client(address, certificate, 99, 1));
        for _ in 0..100 {
            if server.poll_once_strict(&mut runtime).unwrap() {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
        let resync = resync_client.join().unwrap();
        assert!(matches!(
            decode_event(&resync[0]).unwrap().event,
            ProtocolEvent::ResyncRequired {
                requested_after: Revision(99),
                current_revision: Revision(2),
            }
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

        let mut server = RemoteServer::bind(
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
        assert!(open_private_key_file(&key_path).is_err());
        fs::set_permissions(&key_path, fs::Permissions::from_mode(0o600)).unwrap();
        symlink(&key_path, &link_path).unwrap();
        assert!(open_private_key_file(&link_path).is_err());
        fs::remove_file(link_path).unwrap();
        fs::remove_file(key_path).unwrap();
    }
}
