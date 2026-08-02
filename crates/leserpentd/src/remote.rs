use std::io::{BufReader, Cursor, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use leselang_ui::{
    LeselangExportResponse, MAX_LESELANG_EXPORT_BYTES, decode_leselang_export_request,
    encode_leselang_export_response, export_intent_leselang,
};
use leserpent_protocol::bootstrap::{MAX_BOOTSTRAP_PROTOCOL_BYTES, encode_bootstrap_response};
use leserpent_protocol::bootstrap_retirement_control::{
    MAX_DAEMON_RETIREMENT_PROTOCOL_BYTES, encode_daemon_retirement_response,
};
use leserpent_protocol::provisioning::{
    MAX_PROVISIONING_PROTOCOL_BYTES, encode_provisioning_response,
};
use leserpent_protocol::retirement::{MAX_RETIREMENT_PROTOCOL_BYTES, encode_retirement_response};
use leserpent_protocol::transport_safety::{
    AUTHORITY_WRITER_GENERATION_HEADER, AUTHORITY_WRITER_ID_HEADER, BoundedFile,
    MAX_HTTP_HEADER_BYTES, is_http_header_name, open_bounded_regular_file,
};
use leserpent_protocol::{
    AuthorityWriterFence, MAX_PROTOCOL_MESSAGE_BYTES, decode_request, encode_response,
};
use leserpent_runtime::ControlRuntime;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject};
use rustls::{ServerConfig, ServerConnection, StreamOwned};
use zeroize::{Zeroize, Zeroizing};

use crate::bootstrap_submission::{decode_and_submit, error as bootstrap_error};
use crate::daemon_retirement_submission::{
    decode_and_submit as decode_and_submit_daemon_retirement, error as daemon_retirement_error,
};
use crate::events::{EventSession, MAX_EVENT_SESSIONS, is_event_upgrade};
use crate::provisioning_submission::{
    decode_and_submit as decode_and_submit_provisioning, error as provisioning_error,
};
use crate::retirement_submission::{
    decode_and_submit as decode_and_submit_retirement, error as retirement_error,
};
use crate::wire::{
    BootstrapSessionVerifier, authority_writer_fence_error_details, constant_time_equals,
    error_response, execute_request, validate_auth_token,
};

const MAX_CERTIFICATE_FILE_BYTES: u64 = 1024 * 1024;
const MAX_PRIVATE_KEY_FILE_BYTES: u64 = 64 * 1024;
const MAX_REMOTE_TOKEN_FILE_BYTES: u64 = 256;
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(3);
const CONNECTION_READ_POLL_INTERVAL: Duration = Duration::from_millis(100);

pub(crate) type RemoteTlsStream = StreamOwned<ServerConnection, TcpStream>;

struct CancellableTransport<'a, R> {
    inner: R,
    cancelled: &'a AtomicBool,
    deadline: Instant,
}

impl<R> CancellableTransport<'_, R> {
    fn into_inner(self) -> R {
        self.inner
    }
}

impl<R: Read> Read for CancellableTransport<'_, R> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        loop {
            if self.cancelled.load(Ordering::Acquire) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::ConnectionAborted,
                    "remote request cancelled",
                ));
            }
            if Instant::now() >= self.deadline {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "remote request deadline elapsed",
                ));
            }
            match self.inner.read(buffer) {
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                    ) =>
                {
                    std::thread::sleep(CONNECTION_READ_POLL_INTERVAL)
                }
                result => return result,
            }
        }
    }
}

impl<R: Write> Write for CancellableTransport<'_, R> {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        loop {
            if self.cancelled.load(Ordering::Acquire) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::ConnectionAborted,
                    "remote request cancelled",
                ));
            }
            match self.inner.write(buffer) {
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                    ) =>
                {
                    if Instant::now() >= self.deadline {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::TimedOut,
                            "remote request deadline elapsed",
                        ));
                    }
                    std::thread::sleep(CONNECTION_READ_POLL_INTERVAL)
                }
                result => return result,
            }
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

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
    bootstrap_verifier: Option<Arc<dyn BootstrapSessionVerifier>>,
    bootstrap_submission_enabled: bool,
    provisioning_submission_enabled: bool,
    retirement_submission_enabled: bool,
    daemon_retirement_submission_enabled: bool,
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
            bootstrap_verifier: None,
            bootstrap_submission_enabled: false,
            provisioning_submission_enabled: false,
            retirement_submission_enabled: false,
            daemon_retirement_submission_enabled: false,
        })
    }

    pub fn with_bootstrap_verifier(mut self, verifier: Arc<dyn BootstrapSessionVerifier>) -> Self {
        self.bootstrap_verifier = Some(verifier);
        self
    }

    pub fn with_bootstrap_submission(mut self) -> Self {
        self.bootstrap_submission_enabled = true;
        self
    }

    pub fn with_provisioning_submission(mut self) -> Self {
        self.provisioning_submission_enabled = true;
        self
    }

    pub fn with_retirement_submission(mut self) -> Self {
        self.retirement_submission_enabled = true;
        self
    }

    pub fn with_daemon_retirement_submission(mut self) -> Self {
        self.daemon_retirement_submission_enabled = true;
        self
    }

    pub fn local_addr(&self) -> Result<SocketAddr, String> {
        self.listener
            .local_addr()
            .map_err(|error| error.to_string())
    }

    pub fn poll_once(&mut self, runtime: &mut ControlRuntime) -> Result<bool, String> {
        let cancelled = AtomicBool::new(false);
        self.poll_once_until(runtime, &cancelled)
    }

    pub fn poll_once_until(
        &mut self,
        runtime: &mut ControlRuntime,
        cancelled: &AtomicBool,
    ) -> Result<bool, String> {
        if cancelled.load(Ordering::Acquire) {
            return Ok(false);
        }
        let accepted = match self.listener.accept() {
            Ok((stream, _)) => {
                // Peer-controlled TLS, HTTP, and upgrade failures are isolated to this connection.
                if let Ok(Some(session)) = self.handle(stream, runtime, cancelled)
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
        let cancelled = AtomicBool::new(false);
        let (stream, _) = match self.listener.accept() {
            Ok(connection) => connection,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                self.poll_event_sessions(runtime);
                return Ok(false);
            }
            Err(error) => return Err(error.to_string()),
        };
        if let Some(session) = self.handle(stream, runtime, &cancelled)? {
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
        cancelled: &AtomicBool,
    ) -> Result<Option<EventSession>, String> {
        stream
            .set_nonblocking(true)
            .map_err(|error| error.to_string())?;
        let deadline = Instant::now() + CONNECTION_TIMEOUT;
        let transport = CancellableTransport {
            inner: stream,
            cancelled,
            deadline,
        };
        let connection = ServerConnection::new(Arc::clone(&self.tls))
            .map_err(|error| format!("cannot initialize TLS connection: {error}"))?;
        let mut stream = StreamOwned::new(connection, transport);
        let prefix =
            read_http_head(&mut stream).map_err(|_| "invalid HTTPS request".to_string())?;
        if cancelled.load(Ordering::Acquire) {
            return Err("remote request cancelled".into());
        }
        if is_event_upgrade(&prefix) {
            if self.event_sessions.len() >= MAX_EVENT_SESSIONS {
                return Err("WebSocket event session limit reached".into());
            }
            let socket = stream.sock.into_inner();
            socket
                .set_nonblocking(false)
                .map_err(|error| error.to_string())?;
            let stream = StreamOwned::new(stream.conn, socket);
            return EventSession::upgrade(stream, prefix, &self.token).map(Some);
        }
        let bootstrap_route = prefix.starts_with(b"POST /v1/bootstrap HTTP/1.1\r\n");
        let provisioning_route = prefix.starts_with(b"POST /v1/provisioning HTTP/1.1\r\n");
        let retirement_route = prefix.starts_with(b"POST /v1/retirement HTTP/1.1\r\n");
        let daemon_retirement_route =
            prefix.starts_with(b"POST /v1/daemon-retirement HTTP/1.1\r\n");
        let leselang_export_route = prefix.starts_with(b"POST /v1/leselang-export HTTP/1.1\r\n");
        let mut stream = PrefixedStream::new(prefix, stream);
        let request = read_http_request(&mut stream, &self.token);
        if cancelled.load(Ordering::Acquire) {
            return Err("remote request cancelled".into());
        }
        let (status, body) = match request {
            Ok(HttpRequest {
                route: HttpRoute::Wire,
                body,
                writer_fence,
            }) => {
                let response = match decode_request(&body) {
                    Ok(request) => execute_request(
                        runtime,
                        request,
                        self.bootstrap_verifier.as_deref(),
                        writer_fence.as_ref(),
                        false,
                    ),
                    Err(_) => error_response("invalid_request", "wire protocol request is invalid"),
                };
                (
                    HttpStatus::Ok,
                    encode_response(&response).map_err(|error| error.to_string())?,
                )
            }
            Ok(HttpRequest {
                route: HttpRoute::Bootstrap,
                body,
                writer_fence,
            }) => {
                let response = match specialized_authority_writer_fence_error(
                    runtime,
                    writer_fence.as_ref(),
                ) {
                    Some((code, message)) => bootstrap_error(None, code, message),
                    None => decode_and_submit(runtime, &body, self.bootstrap_submission_enabled),
                };
                (
                    HttpStatus::Ok,
                    encode_bootstrap_response(&response).map_err(|error| error.to_string())?,
                )
            }
            Ok(HttpRequest {
                route: HttpRoute::Provisioning,
                body,
                writer_fence,
            }) => {
                let response = match specialized_authority_writer_fence_error(
                    runtime,
                    writer_fence.as_ref(),
                ) {
                    Some((code, message)) => provisioning_error(None, code, message),
                    None => decode_and_submit_provisioning(
                        runtime,
                        &body,
                        self.provisioning_submission_enabled,
                    ),
                };
                (
                    HttpStatus::Ok,
                    encode_provisioning_response(&response).map_err(|error| error.to_string())?,
                )
            }
            Ok(HttpRequest {
                route: HttpRoute::Retirement,
                body,
                writer_fence,
            }) => {
                let response = match specialized_authority_writer_fence_error(
                    runtime,
                    writer_fence.as_ref(),
                ) {
                    Some((code, message)) => retirement_error(None, code, message),
                    None => decode_and_submit_retirement(
                        runtime,
                        &body,
                        self.retirement_submission_enabled,
                    ),
                };
                (
                    HttpStatus::Ok,
                    encode_retirement_response(&response).map_err(|error| error.to_string())?,
                )
            }
            Ok(HttpRequest {
                route: HttpRoute::DaemonRetirement,
                body,
                writer_fence,
            }) => {
                let response = match specialized_authority_writer_fence_error(
                    runtime,
                    writer_fence.as_ref(),
                ) {
                    Some((code, message)) => daemon_retirement_error(None, code, message),
                    None => decode_and_submit_daemon_retirement(
                        runtime,
                        &body,
                        self.daemon_retirement_submission_enabled,
                    ),
                };
                (
                    HttpStatus::Ok,
                    encode_daemon_retirement_response(&response)
                        .map_err(|error| error.to_string())?,
                )
            }
            Ok(HttpRequest {
                route: HttpRoute::LeselangExport,
                body,
                ..
            }) => {
                let response = match decode_leselang_export_request(&body)
                    .and_then(|request| export_intent_leselang(&request.intent))
                {
                    Ok(source) => LeselangExportResponse::success(source),
                    Err(error) => LeselangExportResponse::failure(&error),
                };
                (
                    HttpStatus::Ok,
                    encode_leselang_export_response(&response)
                        .map_err(|error| error.message().to_string())?,
                )
            }
            Err(error) => {
                let body = if bootstrap_route {
                    encode_bootstrap_response(&bootstrap_error(None, error.code, error.message))
                        .map_err(|error| error.to_string())?
                } else if provisioning_route {
                    encode_provisioning_response(&provisioning_error(
                        None,
                        error.code,
                        error.message,
                    ))
                    .map_err(|error| error.to_string())?
                } else if retirement_route {
                    encode_retirement_response(&retirement_error(None, error.code, error.message))
                        .map_err(|error| error.to_string())?
                } else if daemon_retirement_route {
                    encode_daemon_retirement_response(&daemon_retirement_error(
                        None,
                        error.code,
                        error.message,
                    ))
                    .map_err(|error| error.to_string())?
                } else if leselang_export_route {
                    encode_leselang_export_response(&LeselangExportResponse::failure(
                        &leselang_ui::LeselangExportError::InvalidRequest,
                    ))
                    .map_err(|error| error.message().to_string())?
                } else {
                    encode_response(&error_response(error.code, error.message))
                        .map_err(|error| error.to_string())?
                };
                (error.status, body)
            }
        };
        if cancelled.load(Ordering::Acquire) {
            return Err("remote request cancelled".into());
        }
        write_http_response(&mut stream, status, &body)?;
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

fn specialized_authority_writer_fence_error(
    runtime: &mut ControlRuntime,
    writer_fence: Option<&AuthorityWriterFence>,
) -> Option<(&'static str, &'static str)> {
    runtime
        .require_authority_writer(
            writer_fence.map(|fence| fence.generation),
            writer_fence.map(|fence| fence.writer_id.as_str()),
        )
        .err()
        .map(|error| authority_writer_fence_error_details(&error))
}

pub fn load_remote_token_file(path: impl AsRef<Path>) -> Result<Zeroizing<String>, String> {
    let file = open_regular_file(
        path.as_ref(),
        MAX_REMOTE_TOKEN_FILE_BYTES,
        "remote token file",
    )?;
    #[cfg(unix)]
    {
        let mode = file
            .metadata()
            .map_err(|error| format!("cannot inspect remote token file permissions: {error}"))?
            .permissions()
            .mode();
        if mode & 0o077 != 0 {
            return Err("remote token file must not grant group or other access".into());
        }
    }
    let mut token = Zeroizing::new(String::new());
    file.take(MAX_REMOTE_TOKEN_FILE_BYTES + 1)
        .read_to_string(&mut token)
        .map_err(|_| "remote token file must contain UTF-8 text".to_string())?;
    validate_auth_token(&token).map_err(|error| format!("remote {error}"))?;
    Ok(token)
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HttpRoute {
    Wire,
    Bootstrap,
    Provisioning,
    Retirement,
    DaemonRetirement,
    LeselangExport,
}

#[derive(Debug)]
struct HttpRequest {
    route: HttpRoute,
    body: Vec<u8>,
    writer_fence: Option<AuthorityWriterFence>,
}

impl HttpError {
    fn bad_request() -> Self {
        Self {
            status: HttpStatus::BadRequest,
            code: "invalid_http_request",
            message: "HTTPS request is malformed",
        }
    }

    fn invalid_authority_writer_fence() -> Self {
        Self {
            status: HttpStatus::BadRequest,
            code: "invalid_authority_writer_fence",
            message: "authority writer headers must contain one valid paired ticket",
        }
    }
}

fn read_http_request(
    stream: &mut impl Read,
    expected_token: &[u8],
) -> Result<HttpRequest, HttpError> {
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
    let mut writer_id = None;
    let mut writer_generation = None;
    let mut duplicate_writer_header = false;
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
        } else if name.eq_ignore_ascii_case(AUTHORITY_WRITER_ID_HEADER) {
            if writer_id.replace(value).is_some() {
                duplicate_writer_header = true;
            }
        } else if name.eq_ignore_ascii_case(AUTHORITY_WRITER_GENERATION_HEADER) {
            if writer_generation.replace(value).is_some() {
                duplicate_writer_header = true;
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
    if duplicate_writer_header {
        return Err(HttpError::invalid_authority_writer_fence());
    }
    let writer_fence = parse_authority_writer_fence(writer_id, writer_generation)?;
    if parts[0] != "POST" {
        return Err(HttpError {
            status: HttpStatus::MethodNotAllowed,
            code: "method_not_allowed",
            message: "remote wire endpoint requires POST",
        });
    }
    let route = match parts[1] {
        "/v1/wire" => HttpRoute::Wire,
        "/v1/bootstrap" => HttpRoute::Bootstrap,
        "/v1/provisioning" => HttpRoute::Provisioning,
        "/v1/retirement" => HttpRoute::Retirement,
        "/v1/daemon-retirement" => HttpRoute::DaemonRetirement,
        "/v1/leselang-export" => HttpRoute::LeselangExport,
        _ => {
            return Err(HttpError {
                status: HttpStatus::NotFound,
                code: "not_found",
                message: "remote endpoint was not found",
            });
        }
    };
    if !content_type.is_some_and(|value| {
        value
            .split(';')
            .next()
            .is_some_and(|media_type| media_type.trim().eq_ignore_ascii_case("application/json"))
    }) {
        return Err(HttpError {
            status: HttpStatus::UnsupportedMediaType,
            code: "unsupported_media_type",
            message: "remote endpoint requires application/json",
        });
    }
    let content_length = content_length
        .ok_or_else(HttpError::bad_request)?
        .parse::<usize>()
        .map_err(|_| HttpError::bad_request())?;
    let limit = match route {
        HttpRoute::Wire => MAX_PROTOCOL_MESSAGE_BYTES,
        HttpRoute::Bootstrap => MAX_BOOTSTRAP_PROTOCOL_BYTES,
        HttpRoute::Provisioning => MAX_PROVISIONING_PROTOCOL_BYTES,
        HttpRoute::Retirement => MAX_RETIREMENT_PROTOCOL_BYTES,
        HttpRoute::DaemonRetirement => MAX_DAEMON_RETIREMENT_PROTOCOL_BYTES,
        HttpRoute::LeselangExport => MAX_LESELANG_EXPORT_BYTES,
    };
    if content_length > limit {
        return Err(HttpError {
            status: HttpStatus::PayloadTooLarge,
            code: "payload_too_large",
            message: "remote request is too large",
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
    Ok(HttpRequest {
        route,
        body,
        writer_fence,
    })
}

fn parse_authority_writer_fence(
    writer_id: Option<&str>,
    generation: Option<&str>,
) -> Result<Option<AuthorityWriterFence>, HttpError> {
    let (writer_id, generation) = match (writer_id, generation) {
        (None, None) => return Ok(None),
        (Some(writer_id), Some(generation)) => (writer_id, generation),
        _ => return Err(HttpError::invalid_authority_writer_fence()),
    };
    if writer_id.len() != 32
        || !writer_id.bytes().all(|byte| byte.is_ascii_hexdigit())
        || generation.is_empty()
        || !generation.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(HttpError::invalid_authority_writer_fence());
    }
    let generation = generation
        .parse::<u64>()
        .map_err(|_| HttpError::invalid_authority_writer_fence())?;
    if generation == 0 {
        return Err(HttpError::invalid_authority_writer_fence());
    }
    Ok(Some(AuthorityWriterFence {
        generation,
        writer_id: writer_id.to_string(),
    }))
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
    body: &[u8],
) -> Result<(), String> {
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
        .and_then(|()| stream.write_all(body))
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

    use leserpent_domain::{
        CAPABILITY_RUNTIME_DEPLOY, CapabilitySet, Command, CommandEnvelope, CommandId,
        CommandOrigin, CommandStatus, Confirmation, DOMAIN_SCHEMA_VERSION, IdempotencyKey,
        Principal, Revision, RuntimeId,
    };
    use leserpent_protocol::{
        AuthorityWriterFence, HealthRequest, PROTOCOL_SCHEMA_VERSION, ProtocolEvent,
        ProtocolRequest, RequestEnvelope, decode_event, decode_response, encode_request,
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

    fn deployment_body(runtime_id: &str) -> Vec<u8> {
        encode_request(&RequestEnvelope {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            request: ProtocolRequest::Command(CommandEnvelope {
                schema_version: DOMAIN_SCHEMA_VERSION,
                command_id: CommandId::new("remote-writer-fence-deploy").unwrap(),
                idempotency_key: IdempotencyKey::new("remote-writer-fence-deploy-request").unwrap(),
                expected_revision: None,
                principal: Principal {
                    id: "operator".into(),
                },
                capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_DEPLOY]),
                origin: CommandOrigin::CompatibilityAdapter,
                confirmation: Confirmation::Confirmed,
                dry_run: false,
                command: Command::RuntimeDeploy {
                    runtime_id: RuntimeId::new(runtime_id).unwrap(),
                    pipeline_kind: "capture/http".into(),
                    target: Some("service-a".into()),
                },
            }),
        })
        .unwrap()
    }

    fn retirement_body(retirement_id: &str, runtime_id: &str) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "schema_version": 1,
            "request": {
                "principal": { "id": "operator-a" },
                "capabilities": ["runtime.retire"],
                "intent": {
                    "schema_version": 1,
                    "retirement_id": retirement_id,
                    "provisioning_id": "provision-remote-1",
                    "runtime_id": runtime_id,
                    "target": {
                        "transport": "ssh",
                        "host": "runtime.example",
                        "port": 22
                    },
                    "retirement_credential_handle": "vault:ssh:runtime-example",
                    "requested_by": "operator-a",
                    "confirmed": true
                }
            }
        }))
        .unwrap()
    }

    fn daemon_retirement_body(retirement_id: &str, bootstrap_id: &str) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "schema_version": 1,
            "request": {
                "principal": { "id": "operator-a" },
                "capabilities": ["host.retire"],
                "intent": {
                    "schema_version": 1,
                    "retirement_id": retirement_id,
                    "bootstrap_id": bootstrap_id,
                    "retirement_credential_handle": "vault:ssh:host-example",
                    "requested_by": "operator-a",
                    "confirmed": true
                }
            }
        }))
        .unwrap()
    }

    fn request(token: &str, body: &[u8]) -> Vec<u8> {
        request_at(token, "/v1/wire", body)
    }

    fn request_at(token: &str, path: &str, body: &[u8]) -> Vec<u8> {
        request_at_with_writer_fence(token, path, body, None)
    }

    fn request_at_with_writer_fence(
        token: &str,
        path: &str,
        body: &[u8],
        writer_fence: Option<&AuthorityWriterFence>,
    ) -> Vec<u8> {
        let writer_headers = writer_fence.map_or_else(String::new, |fence| {
            format!(
                "{AUTHORITY_WRITER_ID_HEADER}: {}\r\n{AUTHORITY_WRITER_GENERATION_HEADER}: {}\r\n",
                fence.writer_id, fence.generation
            )
        });
        format!(
            "POST {path} HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {token}\r\n{writer_headers}Content-Type: application/json\r\nContent-Length: {}\r\n\r\n",
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

    fn tls_post_client(
        address: SocketAddr,
        certificate: rustls::pki_types::CertificateDer<'static>,
        path: &'static str,
        body: Vec<u8>,
    ) -> thread::JoinHandle<Vec<u8>> {
        tls_post_client_with_token(address, certificate, path, body, TOKEN)
    }

    fn tls_post_client_with_token(
        address: SocketAddr,
        certificate: rustls::pki_types::CertificateDer<'static>,
        path: &'static str,
        body: Vec<u8>,
        token: &'static str,
    ) -> thread::JoinHandle<Vec<u8>> {
        tls_post_client_with_token_and_writer_fence(address, certificate, path, body, token, None)
    }

    fn tls_post_client_with_writer_fence(
        address: SocketAddr,
        certificate: rustls::pki_types::CertificateDer<'static>,
        path: &'static str,
        body: Vec<u8>,
        writer_fence: Option<AuthorityWriterFence>,
    ) -> thread::JoinHandle<Vec<u8>> {
        tls_post_client_with_token_and_writer_fence(
            address,
            certificate,
            path,
            body,
            TOKEN,
            writer_fence,
        )
    }

    fn tls_post_client_with_token_and_writer_fence(
        address: SocketAddr,
        certificate: rustls::pki_types::CertificateDer<'static>,
        path: &'static str,
        body: Vec<u8>,
        token: &'static str,
        writer_fence: Option<AuthorityWriterFence>,
    ) -> thread::JoinHandle<Vec<u8>> {
        thread::spawn(move || {
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
            stream
                .write_all(&request_at_with_writer_fence(
                    token,
                    path,
                    &body,
                    writer_fence.as_ref(),
                ))
                .unwrap();
            read_response(&mut stream)
        })
    }

    fn complete_tls_request(
        server: &mut RemoteServer,
        runtime: &mut ControlRuntime,
        client: thread::JoinHandle<Vec<u8>>,
    ) -> Vec<u8> {
        for _ in 0..100 {
            if server.poll_once_strict(runtime).unwrap() {
                return client.join().unwrap();
            }
            thread::sleep(Duration::from_millis(5));
        }
        panic!("remote TLS request was not accepted");
    }

    fn specialized_response_error_code(path: &str, response: &[u8]) -> Option<String> {
        let body_start = find_header_end(response).unwrap();
        match path {
            "/v1/bootstrap" => {
                match leserpent_protocol::bootstrap::decode_bootstrap_response(
                    &response[body_start..],
                )
                .unwrap()
                .response
                {
                    leserpent_protocol::bootstrap::BootstrapResponse::Error(error) => {
                        Some(error.code)
                    }
                    _ => None,
                }
            }
            "/v1/provisioning" => {
                match leserpent_protocol::provisioning::decode_provisioning_response(
                    &response[body_start..],
                )
                .unwrap()
                .response
                {
                    leserpent_protocol::provisioning::ProvisioningResponse::Error(error) => {
                        Some(error.code)
                    }
                    _ => None,
                }
            }
            "/v1/retirement" => {
                match leserpent_protocol::retirement::decode_retirement_response(
                    &response[body_start..],
                )
                .unwrap()
                .response
                {
                    leserpent_protocol::retirement::RetirementResponse::Error(error) => {
                        Some(error.code)
                    }
                    _ => None,
                }
            }
            "/v1/daemon-retirement" => {
                match leserpent_protocol::bootstrap_retirement_control::decode_daemon_retirement_response(
                    &response[body_start..],
                )
                .unwrap()
                .response
                {
                    leserpent_protocol::bootstrap_retirement_control::DaemonRetirementResponse::Error(error) => {
                        Some(error.code)
                    }
                    _ => None,
                }
            }
            _ => panic!("unsupported specialized route"),
        }
    }

    #[test]
    fn cancelled_transport_uses_a_nonretryable_error_kind() {
        let cancelled = AtomicBool::new(true);
        let mut transport = CancellableTransport {
            inner: Cursor::new(Vec::new()),
            cancelled: &cancelled,
            deadline: Instant::now() + CONNECTION_TIMEOUT,
        };
        let read_error = transport.read(&mut [0_u8; 1]).unwrap_err();
        assert_eq!(read_error.kind(), std::io::ErrorKind::ConnectionAborted);
        let write_error = transport.write(b"blocked").unwrap_err();
        assert_eq!(write_error.kind(), std::io::ErrorKind::ConnectionAborted);
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
        let parsed =
            read_http_request(&mut Cursor::new(request(TOKEN, &body)), TOKEN.as_bytes()).unwrap();
        assert_eq!(parsed.route, HttpRoute::Wire);
        assert_eq!(parsed.body, body);
        assert_eq!(parsed.writer_fence, None);
        let writer_fence = AuthorityWriterFence {
            generation: 42,
            writer_id: "ABCDEFABCDEFABCDEFABCDEFABCDEFAB".into(),
        };
        let parsed = read_http_request(
            &mut Cursor::new(request_at_with_writer_fence(
                TOKEN,
                "/v1/wire",
                &health_body(),
                Some(&writer_fence),
            )),
            TOKEN.as_bytes(),
        )
        .unwrap();
        assert_eq!(parsed.writer_fence, Some(writer_fence));
        let bootstrap = read_http_request(
            &mut Cursor::new(request_at(TOKEN, "/v1/bootstrap", b"{}")),
            TOKEN.as_bytes(),
        )
        .unwrap();
        assert_eq!(bootstrap.route, HttpRoute::Bootstrap);
        assert_eq!(bootstrap.body, b"{}");
        let provisioning = read_http_request(
            &mut Cursor::new(request_at(TOKEN, "/v1/provisioning", b"{}")),
            TOKEN.as_bytes(),
        )
        .unwrap();
        assert_eq!(provisioning.route, HttpRoute::Provisioning);
        assert_eq!(provisioning.body, b"{}");
        let retirement = read_http_request(
            &mut Cursor::new(request_at(TOKEN, "/v1/retirement", b"{}")),
            TOKEN.as_bytes(),
        )
        .unwrap();
        assert_eq!(retirement.route, HttpRoute::Retirement);
        assert_eq!(retirement.body, b"{}");
        let daemon_retirement = read_http_request(
            &mut Cursor::new(request_at(TOKEN, "/v1/daemon-retirement", b"{}")),
            TOKEN.as_bytes(),
        )
        .unwrap();
        assert_eq!(daemon_retirement.route, HttpRoute::DaemonRetirement);
        assert_eq!(daemon_retirement.body, b"{}");
        let leselang_export = read_http_request(
            &mut Cursor::new(request_at(TOKEN, "/v1/leselang-export", b"{}")),
            TOKEN.as_bytes(),
        )
        .unwrap();
        assert_eq!(leselang_export.route, HttpRoute::LeselangExport);
        assert_eq!(leselang_export.body, b"{}");

        let wrong_token = read_http_request(
            &mut Cursor::new(request("fedcba9876543210fedcba9876543210", &health_body())),
            TOKEN.as_bytes(),
        )
        .unwrap_err();
        assert!(matches!(wrong_token.status, HttpStatus::Unauthorized));

        for headers in [
            format!("{AUTHORITY_WRITER_ID_HEADER}: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\r\n"),
            format!("{AUTHORITY_WRITER_GENERATION_HEADER}: 1\r\n"),
            format!(
                "{AUTHORITY_WRITER_ID_HEADER}: short\r\n{AUTHORITY_WRITER_GENERATION_HEADER}: 1\r\n"
            ),
            format!(
                "{AUTHORITY_WRITER_ID_HEADER}: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\r\n{AUTHORITY_WRITER_GENERATION_HEADER}: 0\r\n"
            ),
        ] {
            let request = format!(
                "POST /v1/wire HTTP/1.1\r\nAuthorization: Bearer {TOKEN}\r\n{headers}Content-Type: application/json\r\nContent-Length: 0\r\n\r\n"
            );
            let error = read_http_request(&mut Cursor::new(request.into_bytes()), TOKEN.as_bytes())
                .unwrap_err();
            assert!(matches!(error.status, HttpStatus::BadRequest));
            assert_eq!(error.code, "invalid_authority_writer_fence");
        }

        let duplicate_writer = format!(
            "POST /v1/wire HTTP/1.1\r\nAuthorization: Bearer {TOKEN}\r\n{AUTHORITY_WRITER_ID_HEADER}: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\r\n{AUTHORITY_WRITER_ID_HEADER}: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\r\n{AUTHORITY_WRITER_GENERATION_HEADER}: 1\r\nContent-Type: application/json\r\nContent-Length: 0\r\n\r\n"
        );
        let error = read_http_request(
            &mut Cursor::new(duplicate_writer.into_bytes()),
            TOKEN.as_bytes(),
        )
        .unwrap_err();
        assert!(matches!(error.status, HttpStatus::BadRequest));
        assert_eq!(error.code, "invalid_authority_writer_fence");

        let unauthenticated_malformed_writer = format!(
            "POST /v1/wire HTTP/1.1\r\nAuthorization: Bearer fedcba9876543210fedcba9876543210\r\n{AUTHORITY_WRITER_ID_HEADER}: short\r\n{AUTHORITY_WRITER_GENERATION_HEADER}: 0\r\nContent-Type: application/json\r\nContent-Length: 0\r\n\r\n"
        );
        let error = read_http_request(
            &mut Cursor::new(unauthenticated_malformed_writer.into_bytes()),
            TOKEN.as_bytes(),
        )
        .unwrap_err();
        assert!(matches!(error.status, HttpStatus::Unauthorized));
        assert_eq!(error.code, "unauthorized");

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
        let oversized_provisioning = format!(
            "POST /v1/provisioning HTTP/1.1\r\nAuthorization: Bearer {TOKEN}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
            MAX_PROVISIONING_PROTOCOL_BYTES + 1
        );
        assert!(matches!(
            read_http_request(
                &mut Cursor::new(oversized_provisioning.into_bytes()),
                TOKEN.as_bytes()
            )
            .unwrap_err()
            .status,
            HttpStatus::PayloadTooLarge
        ));
        let oversized_retirement = format!(
            "POST /v1/retirement HTTP/1.1\r\nAuthorization: Bearer {TOKEN}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
            MAX_RETIREMENT_PROTOCOL_BYTES + 1
        );
        assert!(matches!(
            read_http_request(
                &mut Cursor::new(oversized_retirement.into_bytes()),
                TOKEN.as_bytes()
            )
            .unwrap_err()
            .status,
            HttpStatus::PayloadTooLarge
        ));
        let oversized_daemon_retirement = format!(
            "POST /v1/daemon-retirement HTTP/1.1\r\nAuthorization: Bearer {TOKEN}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
            MAX_DAEMON_RETIREMENT_PROTOCOL_BYTES + 1
        );
        assert!(matches!(
            read_http_request(
                &mut Cursor::new(oversized_daemon_retirement.into_bytes()),
                TOKEN.as_bytes()
            )
            .unwrap_err()
            .status,
            HttpStatus::PayloadTooLarge
        ));
        let oversized_leselang_export = format!(
            "POST /v1/leselang-export HTTP/1.1\r\nAuthorization: Bearer {TOKEN}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
            MAX_LESELANG_EXPORT_BYTES + 1
        );
        assert!(matches!(
            read_http_request(
                &mut Cursor::new(oversized_leselang_export.into_bytes()),
                TOKEN.as_bytes()
            )
            .unwrap_err()
            .status,
            HttpStatus::PayloadTooLarge
        ));
    }

    #[test]
    fn authority_writer_generation_fences_remote_mutations_over_real_tls() {
        let CertifiedKey { cert, signing_key } =
            generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
        let certificate_path = temp_path("writer-fence", "crt");
        let key_path = temp_path("writer-fence", "key");
        let database_path = temp_path("writer-fence", "sqlite");
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
        .unwrap()
        .with_bootstrap_submission()
        .with_provisioning_submission()
        .with_retirement_submission()
        .with_daemon_retirement_submission();
        let address = server.local_addr().unwrap();
        let mut runtime = ControlRuntime::open(&database_path).unwrap();
        crate::retirement_submission::seed_registered_runtime(
            &mut runtime,
            "provision-remote-writer-fence",
            "runtime-remote-writer-fence",
        );
        let writer_a = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let writer_b = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
        assert_eq!(
            runtime.claim_authority_writer(writer_a).unwrap().generation,
            1
        );
        assert_eq!(
            runtime.claim_authority_writer(writer_b).unwrap().generation,
            2
        );
        let stale = AuthorityWriterFence {
            generation: 1,
            writer_id: writer_a.into(),
        };
        let current = AuthorityWriterFence {
            generation: 2,
            writer_id: writer_b.into(),
        };

        let deployment = deployment_body("runtime-remote-writer-fence");
        for (fence, expected) in [
            (None, "authority_writer_fence_required"),
            (Some(stale.clone()), "authority_writer_fence_rejected"),
        ] {
            let client = tls_post_client_with_writer_fence(
                address,
                cert.der().clone(),
                "/v1/wire",
                deployment.clone(),
                fence,
            );
            let response = complete_tls_request(&mut server, &mut runtime, client);
            let body_start = find_header_end(&response).unwrap();
            assert!(matches!(
                decode_response(&response[body_start..]).unwrap().response,
                leserpent_protocol::ProtocolResponse::Error(ref error)
                    if error.code == expected
            ));
        }
        assert_eq!(runtime.effect_queue_stats().unwrap().ready, 0);
        let client = tls_post_client_with_writer_fence(
            address,
            cert.der().clone(),
            "/v1/wire",
            deployment,
            Some(current.clone()),
        );
        let response = complete_tls_request(&mut server, &mut runtime, client);
        let body_start = find_header_end(&response).unwrap();
        assert!(matches!(
            decode_response(&response[body_start..]).unwrap().response,
            leserpent_protocol::ProtocolResponse::Command(ref result)
                if result.status == CommandStatus::Applied
        ));
        assert_eq!(runtime.effect_queue_stats().unwrap().ready, 1);

        let bootstrap =
            include_bytes!("../../leserpent-protocol/tests/fixtures/bootstrap-request-v1.json")
                .to_vec();
        let bootstrap_id = leserpent_domain::bootstrap::BootstrapId::new("bootstrap-1").unwrap();
        for (fence, expected) in [
            (None, "authority_writer_fence_required"),
            (Some(stale.clone()), "authority_writer_fence_rejected"),
        ] {
            let client = tls_post_client_with_writer_fence(
                address,
                cert.der().clone(),
                "/v1/bootstrap",
                bootstrap.clone(),
                fence,
            );
            let response = complete_tls_request(&mut server, &mut runtime, client);
            assert_eq!(
                specialized_response_error_code("/v1/bootstrap", &response).as_deref(),
                Some(expected)
            );
            assert!(
                runtime
                    .bootstrap_checkpoint(&bootstrap_id)
                    .unwrap()
                    .is_none()
            );
        }
        let client = tls_post_client_with_writer_fence(
            address,
            cert.der().clone(),
            "/v1/bootstrap",
            bootstrap,
            Some(current.clone()),
        );
        let response = complete_tls_request(&mut server, &mut runtime, client);
        assert!(response.starts_with(b"HTTP/1.1 200 OK\r\n"));
        assert_eq!(
            runtime
                .bootstrap_checkpoint(&bootstrap_id)
                .unwrap()
                .unwrap()
                .revision,
            1
        );

        for path in [
            "/v1/provisioning",
            "/v1/retirement",
            "/v1/daemon-retirement",
        ] {
            for (fence, expected) in [
                (None, "authority_writer_fence_required"),
                (Some(stale.clone()), "authority_writer_fence_rejected"),
            ] {
                let client = tls_post_client_with_writer_fence(
                    address,
                    cert.der().clone(),
                    path,
                    b"{}".to_vec(),
                    fence,
                );
                let response = complete_tls_request(&mut server, &mut runtime, client);
                assert_eq!(
                    specialized_response_error_code(path, &response).as_deref(),
                    Some(expected)
                );
            }
            let client = tls_post_client_with_writer_fence(
                address,
                cert.der().clone(),
                path,
                b"{}".to_vec(),
                Some(current.clone()),
            );
            let response = complete_tls_request(&mut server, &mut runtime, client);
            assert!(!matches!(
                specialized_response_error_code(path, &response).as_deref(),
                Some("authority_writer_fence_required" | "authority_writer_fence_rejected")
            ));
        }

        let export = serde_json::to_vec(&leselang_ui::LeselangExportRequest {
            schema_version: leselang_ui::LESELANG_EXPORT_SCHEMA_VERSION,
            intent: leselang_ui::LeselangExportIntent::RuntimeDeploy {
                runtime_id: "runtime-a".into(),
                pipeline_kind: "http/request".into(),
                target: None,
            },
        })
        .unwrap();
        let client = tls_post_client(address, cert.der().clone(), "/v1/leselang-export", export);
        let response = complete_tls_request(&mut server, &mut runtime, client);
        let body_start = find_header_end(&response).unwrap();
        let decoded =
            serde_json::from_slice::<leselang_ui::LeselangExportResponse>(&response[body_start..])
                .unwrap();
        assert!(decoded.error.is_none());

        drop(runtime);
        fs::remove_file(certificate_path).unwrap();
        fs::remove_file(key_path).unwrap();
        fs::remove_file(database_path).unwrap();
    }

    #[test]
    fn authenticated_leselang_export_uses_rust_canonical_source_over_real_tls() {
        let CertifiedKey { cert, signing_key } =
            generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
        let certificate_path = temp_path("leselang-export", "crt");
        let key_path = temp_path("leselang-export", "key");
        let database_path = temp_path("leselang-export", "sqlite");
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
        let body = serde_json::to_vec(&leselang_ui::LeselangExportRequest {
            schema_version: leselang_ui::LESELANG_EXPORT_SCHEMA_VERSION,
            intent: leselang_ui::LeselangExportIntent::RuntimeDeploy {
                runtime_id: "runtime-a".into(),
                pipeline_kind: "http/request".into(),
                target: Some("label:\"a\"\\b".into()),
            },
        })
        .unwrap();
        let client = tls_post_client(address, cert.der().clone(), "/v1/leselang-export", body);
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
        let decoded =
            serde_json::from_slice::<leselang_ui::LeselangExportResponse>(&response[body_start..])
                .unwrap();
        let source = decoded.source.unwrap();
        assert!(decoded.error.is_none());
        let parsed = leselang_syntax::parse(&source);
        let program = leselang_hir::lower(&parsed).unwrap();
        assert!(matches!(
            program.function.effect,
            leselang_hir::Effect::RuntimeDeploy {
                runtime_id,
                pipeline_kind,
                target: Some(target),
            } if runtime_id.as_str() == "runtime-a"
                && pipeline_kind == "http/request"
                && target == "label:\"a\"\\b"
        ));

        drop(runtime);
        fs::remove_file(certificate_path).unwrap();
        fs::remove_file(key_path).unwrap();
        fs::remove_file(database_path).unwrap();
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
    fn authenticated_retirement_commits_planned_checkpoint_over_real_tls() {
        let CertifiedKey { cert, signing_key } =
            generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
        let certificate_path = temp_path("retirement", "crt");
        let key_path = temp_path("retirement", "key");
        let database_path = temp_path("retirement", "sqlite");
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
        let body = retirement_body("retire-remote-1", "runtime-remote-retire");
        let mut runtime = ControlRuntime::open(&database_path).unwrap();
        crate::retirement_submission::seed_registered_runtime(
            &mut runtime,
            "provision-remote-1",
            "runtime-remote-retire",
        );
        let client = tls_post_client(address, certificate, "/v1/retirement", body.clone());
        for _ in 0..100 {
            if server.poll_once_strict(&mut runtime).unwrap() {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
        let response = client.join().unwrap();
        assert!(response.starts_with(b"HTTP/1.1 200 OK\r\n"));
        let body_start = find_header_end(&response).unwrap();
        let decoded =
            leserpent_protocol::retirement::decode_retirement_response(&response[body_start..])
                .unwrap();
        assert!(matches!(
            decoded.response,
            leserpent_protocol::retirement::RetirementResponse::Error(ref error)
                if error.code == "retirement_unavailable"
        ));
        let retirement_id =
            leserpent_domain::retirement::RetirementId::new("retire-remote-1").unwrap();
        assert!(
            runtime
                .retirement_checkpoint(&retirement_id)
                .unwrap()
                .is_none()
        );
        drop(server);

        let mut server = RemoteServer::bind(
            "127.0.0.1:0".parse().unwrap(),
            &certificate_path,
            &key_path,
            TOKEN,
        )
        .unwrap()
        .with_retirement_submission();
        let client = tls_post_client(
            server.local_addr().unwrap(),
            cert.der().clone(),
            "/v1/retirement",
            body,
        );
        for _ in 0..100 {
            if server.poll_once_strict(&mut runtime).unwrap() {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
        let response = client.join().unwrap();
        assert!(response.starts_with(b"HTTP/1.1 200 OK\r\n"));
        let body_start = find_header_end(&response).unwrap();
        let decoded =
            leserpent_protocol::retirement::decode_retirement_response(&response[body_start..])
                .unwrap();
        assert!(
            matches!(
                decoded.response,
                leserpent_protocol::retirement::RetirementResponse::State(ref state)
                    if state.phase == leserpent_domain::retirement::RetirementPhase::Planned
            ),
            "unexpected retirement response: {decoded:?}"
        );
        assert_eq!(
            runtime
                .retirement_checkpoint(&retirement_id)
                .unwrap()
                .unwrap()
                .revision,
            1
        );

        drop(runtime);
        fs::remove_file(certificate_path).unwrap();
        fs::remove_file(key_path).unwrap();
        fs::remove_file(database_path).unwrap();
    }

    #[test]
    fn authenticated_daemon_retirement_commits_derived_checkpoint_over_real_tls() {
        let CertifiedKey { cert, signing_key } =
            generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
        let certificate_path = temp_path("daemon-retirement", "crt");
        let key_path = temp_path("daemon-retirement", "key");
        let database_path = temp_path("daemon-retirement", "sqlite");
        fs::write(&certificate_path, cert.pem()).unwrap();
        fs::write(&key_path, signing_key.serialize_pem()).unwrap();
        #[cfg(unix)]
        fs::set_permissions(&key_path, fs::Permissions::from_mode(0o600)).unwrap();

        let mut runtime = ControlRuntime::open(&database_path).unwrap();
        crate::daemon_retirement_submission::seed_bound_deployment(
            &mut runtime,
            "bootstrap-remote-retire",
        );
        let body = daemon_retirement_body("retire-daemon-remote-1", "bootstrap-remote-retire");
        let retirement_id =
            leserpent_domain::retirement::RetirementId::new("retire-daemon-remote-1").unwrap();

        let mut server = RemoteServer::bind(
            "127.0.0.1:0".parse().unwrap(),
            &certificate_path,
            &key_path,
            TOKEN,
        )
        .unwrap();
        let unauthorized = tls_post_client_with_token(
            server.local_addr().unwrap(),
            cert.der().clone(),
            "/v1/daemon-retirement",
            body.clone(),
            "fedcba9876543210fedcba9876543210",
        );
        for _ in 0..100 {
            if server.poll_once_strict(&mut runtime).unwrap() {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
        let response = unauthorized.join().unwrap();
        assert!(response.starts_with(b"HTTP/1.1 401 Unauthorized\r\n"));
        let body_start = find_header_end(&response).unwrap();
        let decoded =
            leserpent_protocol::bootstrap_retirement_control::decode_daemon_retirement_response(
                &response[body_start..],
            )
            .unwrap();
        assert!(matches!(
            decoded.response,
            leserpent_protocol::bootstrap_retirement_control::DaemonRetirementResponse::Error(
                ref error
            ) if error.code == "unauthorized"
        ));
        assert!(
            runtime
                .daemon_retirement_checkpoint(&retirement_id)
                .unwrap()
                .is_none()
        );

        let client = tls_post_client(
            server.local_addr().unwrap(),
            cert.der().clone(),
            "/v1/daemon-retirement",
            body.clone(),
        );
        for _ in 0..100 {
            if server.poll_once_strict(&mut runtime).unwrap() {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
        let response = client.join().unwrap();
        let body_start = find_header_end(&response).unwrap();
        let decoded =
            leserpent_protocol::bootstrap_retirement_control::decode_daemon_retirement_response(
                &response[body_start..],
            )
            .unwrap();
        assert!(matches!(
            decoded.response,
            leserpent_protocol::bootstrap_retirement_control::DaemonRetirementResponse::Error(
                ref error
            ) if error.code == "daemon_retirement_unavailable"
        ));
        assert!(
            runtime
                .daemon_retirement_checkpoint(&retirement_id)
                .unwrap()
                .is_none()
        );
        drop(server);

        let mut server = RemoteServer::bind(
            "127.0.0.1:0".parse().unwrap(),
            &certificate_path,
            &key_path,
            TOKEN,
        )
        .unwrap()
        .with_daemon_retirement_submission();
        let client = tls_post_client(
            server.local_addr().unwrap(),
            cert.der().clone(),
            "/v1/daemon-retirement",
            body,
        );
        for _ in 0..100 {
            if server.poll_once_strict(&mut runtime).unwrap() {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
        let response = client.join().unwrap();
        assert!(response.starts_with(b"HTTP/1.1 200 OK\r\n"));
        let body_start = find_header_end(&response).unwrap();
        let decoded =
            leserpent_protocol::bootstrap_retirement_control::decode_daemon_retirement_response(
                &response[body_start..],
            )
            .unwrap();
        assert!(matches!(
            decoded.response,
            leserpent_protocol::bootstrap_retirement_control::DaemonRetirementResponse::State(
                ref state
            ) if state.phase
                == leserpent_domain::bootstrap_retirement::DaemonRetirementPhase::Planned
        ));
        assert_eq!(
            runtime
                .daemon_retirement_checkpoint(&retirement_id)
                .unwrap()
                .unwrap()
                .revision,
            1
        );

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

    #[test]
    fn remote_token_file_is_private_bounded_and_redacted() {
        let token_path = temp_path("token", "secret");
        fs::write(&token_path, TOKEN).unwrap();
        #[cfg(unix)]
        fs::set_permissions(&token_path, fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(load_remote_token_file(&token_path).unwrap().as_str(), TOKEN);

        #[cfg(unix)]
        {
            fs::set_permissions(&token_path, fs::Permissions::from_mode(0o644)).unwrap();
            let error = load_remote_token_file(&token_path).unwrap_err();
            assert!(!error.contains(TOKEN));
            fs::set_permissions(&token_path, fs::Permissions::from_mode(0o600)).unwrap();
            let link_path = temp_path("token-link", "secret");
            symlink(&token_path, &link_path).unwrap();
            assert!(load_remote_token_file(&link_path).is_err());
            fs::remove_file(link_path).unwrap();
        }
        fs::write(
            &token_path,
            "x".repeat(MAX_REMOTE_TOKEN_FILE_BYTES as usize + 1),
        )
        .unwrap();
        assert!(load_remote_token_file(&token_path).is_err());
        fs::remove_file(token_path).unwrap();
    }
}
