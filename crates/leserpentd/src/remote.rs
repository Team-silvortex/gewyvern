use std::borrow::Cow;
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
use leserpent_protocol::{
    AUTHORITY_WRITER_GENERATION_HEADER, AUTHORITY_WRITER_ID_HEADER, AuthorityWriterFence,
    MAX_PROTOCOL_MESSAGE_BYTES, decode_request, encode_response,
};
use leserpent_runtime::ControlRuntime;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject};
use rustls::{ServerConfig, ServerConnection, StreamOwned};
use silvortex_bounded_io::{
    BoundedFile, MAX_HTTP_HEADER_BYTES, is_http_header_name, open_bounded_regular_file,
};
use zeroize::{Zeroize, Zeroizing};

use crate::bootstrap_submission::{decode_and_submit, error as bootstrap_error};
use crate::daemon_retirement_submission::{
    decode_and_submit as decode_and_submit_daemon_retirement, error as daemon_retirement_error,
};
use crate::events::{EventSession, MAX_EVENT_SESSIONS, is_event_upgrade};
use crate::language_packs::{self, LanguagePackAsset};
use crate::provisioning_submission::{
    decode_and_submit as decode_and_submit_provisioning, error as provisioning_error,
};
use crate::retirement_submission::{
    decode_and_submit as decode_and_submit_retirement, error as retirement_error,
};
use crate::runtime_target_registration::RuntimeTargetRegistrationAuthority;
use crate::web_console::{self, ConsoleApiMethod, ConsoleApiRoute, ConsoleAsset};
use crate::web_console_write::{self, ConsoleWriteStatus};
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
    pub(crate) fn set_send_buffer_size(&self, bytes: usize) -> std::io::Result<()> {
        socket2::SockRef::from(&self.inner.sock).set_send_buffer_size(bytes)
    }

    pub(crate) fn set_nonblocking(&self, nonblocking: bool) -> std::io::Result<()> {
        self.inner.sock.set_nonblocking(nonblocking)
    }
}

pub struct RemoteServer {
    listener: TcpListener,
    tls: Arc<ServerConfig>,
    token: Zeroizing<Vec<u8>>,
    event_sessions: Vec<EventSession>,
    debugger_authority: Option<crate::SharedDebuggerAuthority>,
    bootstrap_verifier: Option<Arc<dyn BootstrapSessionVerifier>>,
    bootstrap_submission_enabled: bool,
    provisioning_submission_enabled: bool,
    retirement_submission_enabled: bool,
    daemon_retirement_submission_enabled: bool,
    web_console_writer: Option<AuthorityWriterFence>,
    runtime_target_registration: Option<RuntimeTargetRegistrationAuthority>,
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
            debugger_authority: None,
            bootstrap_verifier: None,
            bootstrap_submission_enabled: false,
            provisioning_submission_enabled: false,
            retirement_submission_enabled: false,
            daemon_retirement_submission_enabled: false,
            web_console_writer: None,
            runtime_target_registration: None,
        })
    }

    pub fn with_bootstrap_verifier(mut self, verifier: Arc<dyn BootstrapSessionVerifier>) -> Self {
        self.bootstrap_verifier = Some(verifier);
        self
    }

    pub fn with_debugger_authority(mut self, authority: crate::SharedDebuggerAuthority) -> Self {
        self.debugger_authority = Some(authority);
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

    pub fn with_web_console_writer(mut self, writer_fence: AuthorityWriterFence) -> Self {
        self.web_console_writer = Some(writer_fence);
        self
    }

    pub fn with_runtime_target_registration(
        mut self,
        authority: RuntimeTargetRegistrationAuthority,
    ) -> Self {
        self.runtime_target_registration = Some(authority);
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
        // Reclaim closed event sessions before applying the capacity limit to a reconnect.
        self.poll_event_sessions(runtime);
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
        let (status, body, response_policy): (HttpStatus, Cow<'static, [u8]>, HttpResponsePolicy) =
            match request {
                Ok(HttpRequest {
                    route: HttpRoute::ConsoleAsset(asset),
                    ..
                }) => (
                    HttpStatus::Ok,
                    Cow::Borrowed(asset.payload),
                    HttpResponsePolicy {
                        content_type: asset.content_type,
                        cache_control: asset.cache_control,
                        document: asset.document,
                        content_disposition: None,
                    },
                ),
                Ok(HttpRequest {
                    route: HttpRoute::LanguagePack(asset),
                    ..
                }) => (
                    HttpStatus::Ok,
                    Cow::Borrowed(asset.payload),
                    HttpResponsePolicy::public_json(),
                ),
                Ok(HttpRequest {
                    route: HttpRoute::ConsoleApi(route),
                    body,
                    ..
                }) => {
                    let persistence_export = route == ConsoleApiRoute::PersistenceExport;
                    let accepted_response = route.accepted_response();
                    let writer_available =
                        web_console_writer_available(runtime, self.web_console_writer.as_ref());
                    let registration = writer_available
                        .then_some(self.runtime_target_registration.as_ref())
                        .flatten();
                    let response = if route.method() == ConsoleApiMethod::Get {
                        web_console::render_api_with_registration(
                            &route,
                            runtime,
                            writer_available,
                            registration.is_some(),
                        )
                    } else {
                        web_console_write::execute_with_registration(
                            &route,
                            &body,
                            runtime,
                            self.web_console_writer.as_ref(),
                            registration,
                        )
                    };
                    match response {
                        Ok(body) => (
                            if accepted_response {
                                HttpStatus::Accepted
                            } else {
                                HttpStatus::Ok
                            },
                            Cow::Owned(body),
                            if persistence_export {
                                HttpResponsePolicy::private_json_download()
                            } else {
                                HttpResponsePolicy::private_json()
                            },
                        ),
                        Err(error) => (
                            console_write_http_status(error.status),
                            Cow::Owned(error.body()),
                            HttpResponsePolicy::private_json(),
                        ),
                    }
                }
                Ok(HttpRequest {
                    route: HttpRoute::Wire,
                    body,
                    writer_fence,
                }) => {
                    let response = match decode_request(&body) {
                        Ok(request) => match &self.debugger_authority {
                            Some(authority) => match authority.lock() {
                                Ok(mut debugger) => crate::wire::execute_request_with_debugger(
                                    runtime,
                                    request,
                                    self.bootstrap_verifier.as_deref(),
                                    writer_fence.as_ref(),
                                    false,
                                    Some(&mut debugger),
                                ),
                                Err(_) => error_response(
                                    "debugger_authority_unavailable",
                                    "Leselang VM debugger authority is unavailable",
                                ),
                            },
                            None => execute_request(
                                runtime,
                                request,
                                self.bootstrap_verifier.as_deref(),
                                writer_fence.as_ref(),
                                false,
                            ),
                        },
                        Err(_) => {
                            error_response("invalid_request", "wire protocol request is invalid")
                        }
                    };
                    (
                        HttpStatus::Ok,
                        Cow::Owned(encode_response(&response).map_err(|error| error.to_string())?),
                        HttpResponsePolicy::private_json(),
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
                        None => {
                            decode_and_submit(runtime, &body, self.bootstrap_submission_enabled)
                        }
                    };
                    (
                        HttpStatus::Ok,
                        Cow::Owned(
                            encode_bootstrap_response(&response)
                                .map_err(|error| error.to_string())?,
                        ),
                        HttpResponsePolicy::private_json(),
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
                        Cow::Owned(
                            encode_provisioning_response(&response)
                                .map_err(|error| error.to_string())?,
                        ),
                        HttpResponsePolicy::private_json(),
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
                        Cow::Owned(
                            encode_retirement_response(&response)
                                .map_err(|error| error.to_string())?,
                        ),
                        HttpResponsePolicy::private_json(),
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
                        Cow::Owned(
                            encode_daemon_retirement_response(&response)
                                .map_err(|error| error.to_string())?,
                        ),
                        HttpResponsePolicy::private_json(),
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
                        Cow::Owned(
                            encode_leselang_export_response(&response)
                                .map_err(|error| error.message().to_string())?,
                        ),
                        HttpResponsePolicy::private_json(),
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
                        encode_retirement_response(&retirement_error(
                            None,
                            error.code,
                            error.message,
                        ))
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
                    (
                        error.status,
                        Cow::Owned(body),
                        HttpResponsePolicy::private_json(),
                    )
                }
            };
        if cancelled.load(Ordering::Acquire) {
            return Err("remote request cancelled".into());
        }
        write_http_response(&mut stream, status, &body, response_policy)?;
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

fn web_console_writer_available(
    runtime: &mut ControlRuntime,
    writer_fence: Option<&AuthorityWriterFence>,
) -> bool {
    writer_fence.is_some_and(|writer_fence| {
        runtime
            .require_authority_writer(Some(writer_fence.generation), Some(&writer_fence.writer_id))
            .is_ok()
    })
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
    Accepted,
    BadRequest,
    Unauthorized,
    NotFound,
    Conflict,
    MethodNotAllowed,
    PayloadTooLarge,
    UnsupportedMediaType,
    InternalServerError,
    ServiceUnavailable,
}

impl HttpStatus {
    fn line(self) -> &'static str {
        match self {
            Self::Ok => "200 OK",
            Self::Accepted => "202 Accepted",
            Self::BadRequest => "400 Bad Request",
            Self::Unauthorized => "401 Unauthorized",
            Self::NotFound => "404 Not Found",
            Self::Conflict => "409 Conflict",
            Self::MethodNotAllowed => "405 Method Not Allowed",
            Self::PayloadTooLarge => "413 Payload Too Large",
            Self::UnsupportedMediaType => "415 Unsupported Media Type",
            Self::InternalServerError => "500 Internal Server Error",
            Self::ServiceUnavailable => "503 Service Unavailable",
        }
    }
}

fn console_write_http_status(status: ConsoleWriteStatus) -> HttpStatus {
    match status {
        ConsoleWriteStatus::BadRequest => HttpStatus::BadRequest,
        ConsoleWriteStatus::NotFound => HttpStatus::NotFound,
        ConsoleWriteStatus::Conflict => HttpStatus::Conflict,
        ConsoleWriteStatus::ServiceUnavailable => HttpStatus::ServiceUnavailable,
        ConsoleWriteStatus::InternalServerError => HttpStatus::InternalServerError,
    }
}

#[derive(Debug)]
struct HttpError {
    status: HttpStatus,
    code: &'static str,
    message: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum HttpRoute {
    ConsoleAsset(ConsoleAsset),
    LanguagePack(LanguagePackAsset),
    ConsoleApi(ConsoleApiRoute),
    Wire,
    Bootstrap,
    Provisioning,
    Retirement,
    DaemonRetirement,
    LeselangExport,
}

struct HttpRequest {
    route: HttpRoute,
    body: Zeroizing<Vec<u8>>,
    writer_fence: Option<AuthorityWriterFence>,
}

impl std::fmt::Debug for HttpRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("HttpRequest")
            .field("route", &self.route)
            .field("body_len", &self.body.len())
            .field("writer_fence", &self.writer_fence)
            .finish()
    }
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
        let mut chunk = Zeroizing::new([0_u8; 1024]);
        let read = stream
            .read(&mut *chunk)
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
    let mut admin_token = None;
    let mut content_length = None;
    let mut content_type = None;
    let mut intent = None;
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
        } else if name.eq_ignore_ascii_case("x-leserpent-admin-token") {
            if admin_token.replace(value).is_some() {
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
        } else if name.eq_ignore_ascii_case("x-leserpent-intent") {
            if intent.replace(value).is_some() {
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

    if let Some(asset) = web_console::find_asset(parts[1]) {
        if authorization.is_some() || admin_token.is_some() {
            return Err(HttpError {
                status: HttpStatus::BadRequest,
                code: "console_asset_credentials_forbidden",
                message: "public console asset requests must not carry credentials",
            });
        }
        if parts[0] != "GET" {
            return Err(HttpError {
                status: HttpStatus::MethodNotAllowed,
                code: "method_not_allowed",
                message: "console asset endpoints require GET",
            });
        }
        if content_type.is_some()
            || intent.is_some()
            || writer_id.is_some()
            || writer_generation.is_some()
            || content_length.is_some_and(|value| value != "0")
            || bytes.len() != header_end
        {
            return Err(HttpError::bad_request());
        }
        return Ok(HttpRequest {
            route: HttpRoute::ConsoleAsset(asset),
            body: Zeroizing::new(Vec::new()),
            writer_fence: None,
        });
    }

    if parts[1].starts_with("/language-packs/") {
        if authorization.is_some() || admin_token.is_some() {
            return Err(HttpError {
                status: HttpStatus::BadRequest,
                code: "language_pack_credentials_forbidden",
                message: "public language-pack requests must not carry credentials",
            });
        }
        if parts[0] != "GET" {
            return Err(HttpError {
                status: HttpStatus::MethodNotAllowed,
                code: "method_not_allowed",
                message: "language-pack endpoints require GET",
            });
        }
        if content_type.is_some()
            || intent.is_some()
            || writer_id.is_some()
            || writer_generation.is_some()
            || content_length.is_some_and(|value| value != "0")
            || bytes.len() != header_end
        {
            return Err(HttpError::bad_request());
        }
        let asset = language_packs::find(parts[1]).ok_or(HttpError {
            status: HttpStatus::NotFound,
            code: "not_found",
            message: "language pack was not found",
        })?;
        return Ok(HttpRequest {
            route: HttpRoute::LanguagePack(asset),
            body: Zeroizing::new(Vec::new()),
            writer_fence: None,
        });
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
    if admin_token.is_some_and(|token| !constant_time_equals(token.as_bytes(), expected_token)) {
        return Err(HttpError {
            status: HttpStatus::Unauthorized,
            code: "ambiguous_credentials",
            message: "remote authentication credentials disagree",
        });
    }
    if duplicate_writer_header {
        return Err(HttpError::invalid_authority_writer_fence());
    }
    let writer_fence = parse_authority_writer_fence(writer_id, writer_generation)?;
    let console_api =
        web_console::parse_api_route_for_method(parts[0], parts[1]).map_err(|_| HttpError {
            status: HttpStatus::BadRequest,
            code: "invalid_console_query",
            message: "Rust Web compatibility query is invalid",
        })?;
    if let Some(route) = console_api {
        if writer_fence.is_some() {
            return Err(HttpError::invalid_authority_writer_fence());
        }
        let expected_method = match route.method() {
            ConsoleApiMethod::Get => "GET",
            ConsoleApiMethod::PostEmpty | ConsoleApiMethod::PostJson => "POST",
        };
        if parts[0] != expected_method {
            return Err(HttpError {
                status: HttpStatus::MethodNotAllowed,
                code: "method_not_allowed",
                message: "Rust Web compatibility endpoint method is not allowed",
            });
        }
        let body = match route.method() {
            ConsoleApiMethod::Get => {
                if content_type.is_some()
                    || intent.is_some()
                    || content_length.is_some_and(|value| value != "0")
                    || bytes.len() != header_end
                {
                    return Err(HttpError::bad_request());
                }
                Zeroizing::new(Vec::new())
            }
            ConsoleApiMethod::PostEmpty => {
                if intent != Some("mutate")
                    || content_type.is_some()
                    || content_length.is_some_and(|value| value != "0")
                    || bytes.len() != header_end
                {
                    return Err(HttpError::bad_request());
                }
                Zeroizing::new(Vec::new())
            }
            ConsoleApiMethod::PostJson => {
                if intent != Some("mutate") {
                    return Err(HttpError::bad_request());
                }
                if !content_type.is_some_and(|value| {
                    value.split(';').next().is_some_and(|media_type| {
                        media_type.trim().eq_ignore_ascii_case("application/json")
                    })
                }) {
                    return Err(HttpError {
                        status: HttpStatus::UnsupportedMediaType,
                        code: "unsupported_media_type",
                        message: "Rust Web JSON endpoint requires application/json",
                    });
                }
                let content_length = content_length
                    .ok_or_else(HttpError::bad_request)?
                    .parse::<usize>()
                    .map_err(|_| HttpError::bad_request())?;
                if content_length == 0 {
                    return Err(HttpError::bad_request());
                }
                let maximum = route
                    .max_json_body_bytes()
                    .ok_or_else(HttpError::bad_request)?;
                if content_length > maximum {
                    return Err(HttpError {
                        status: HttpStatus::PayloadTooLarge,
                        code: "payload_too_large",
                        message: "Rust Web request is too large",
                    });
                }
                let mut body = Zeroizing::new(bytes.split_off(header_end));
                if body.len() > content_length {
                    return Err(HttpError::bad_request());
                }
                if body.len() < content_length {
                    let missing = content_length - body.len();
                    let mut remainder = Zeroizing::new(vec![0_u8; missing]);
                    stream
                        .read_exact(&mut remainder)
                        .map_err(|_| HttpError::bad_request())?;
                    body.extend_from_slice(&remainder);
                }
                body
            }
        };
        return Ok(HttpRequest {
            route: HttpRoute::ConsoleApi(route),
            body,
            writer_fence: None,
        });
    }
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
        HttpRoute::ConsoleAsset(_) | HttpRoute::ConsoleApi(_) | HttpRoute::LanguagePack(_) => {
            return Err(HttpError::bad_request());
        }
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

    let mut body = Zeroizing::new(bytes.split_off(header_end));
    if body.len() > content_length {
        return Err(HttpError::bad_request());
    }
    if body.len() < content_length {
        let missing = content_length - body.len();
        let mut remainder = Zeroizing::new(vec![0_u8; missing]);
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct HttpResponsePolicy {
    content_type: &'static str,
    cache_control: &'static str,
    document: bool,
    content_disposition: Option<&'static str>,
}

impl HttpResponsePolicy {
    fn private_json() -> Self {
        Self {
            content_type: "application/json",
            cache_control: "no-store",
            document: false,
            content_disposition: None,
        }
    }

    fn private_json_download() -> Self {
        Self {
            content_type: "application/json",
            cache_control: "no-store",
            document: false,
            content_disposition: Some(
                "attachment; filename=\"leserpent-control-plane-state.json\"",
            ),
        }
    }

    fn public_json() -> Self {
        Self {
            content_type: "application/json",
            cache_control: "no-cache",
            document: false,
            content_disposition: None,
        }
    }
}

fn write_http_response(
    stream: &mut impl Write,
    status: HttpStatus,
    body: &[u8],
    policy: HttpResponsePolicy,
) -> Result<(), String> {
    let challenge = if matches!(status, HttpStatus::Unauthorized) {
        "WWW-Authenticate: Bearer\r\n"
    } else {
        ""
    };
    let content_security_policy = if policy.document {
        "Content-Security-Policy: default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'self' https: wss:; frame-src 'self' https:; object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'self'\r\n"
    } else {
        ""
    };
    let content_disposition = policy.content_disposition.map_or(String::new(), |value| {
        format!("Content-Disposition: {value}\r\n")
    });
    let header = format!(
        "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: {}\r\nX-Content-Type-Options: nosniff\r\nX-Frame-Options: DENY\r\nReferrer-Policy: no-referrer\r\nPermissions-Policy: camera=(), microphone=(), geolocation=()\r\nCross-Origin-Opener-Policy: same-origin\r\n{}{}{}Connection: close\r\n\r\n",
        status.line(),
        policy.content_type,
        body.len(),
        policy.cache_control,
        content_security_policy,
        content_disposition,
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
    use std::collections::BTreeMap;
    use std::fs;
    use std::io::{Cursor, Read, Write};
    use std::net::TcpStream;
    #[cfg(unix)]
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::path::PathBuf;
    use std::sync::Mutex;
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};

    use leselang_hir::CAPABILITY_UI_PRESENTATION;
    use leselang_vm::PresentationOperation;
    use leserpent_adapters::{
        GewyvernTargetCatalog, MutableSecretStore, SecretKey, SecretStore, SecretStoreError,
        SecretValue,
    };
    use leserpent_domain::{
        CAPABILITY_DEBUGGER_CONTROL, CAPABILITY_RUNTIME_DEPLOY, CapabilitySet, Command,
        CommandEnvelope, CommandId, CommandOrigin, CommandStatus, Confirmation,
        DOMAIN_SCHEMA_VERSION, IdempotencyKey, Principal, Revision, RuntimeId,
    };
    use leserpent_protocol::{
        AuthorityWriterFence, DebuggerMutationStatus, DebuggerPresentationAcknowledgeRequest,
        DebuggerPresentationOutcome, DebuggerPresentationStatus, DebuggerSessionStartRequest,
        HealthRequest, PROTOCOL_SCHEMA_VERSION, ProtocolEvent, ProtocolRequest, ProtocolResponse,
        RequestEnvelope, decode_event, decode_response, encode_request,
    };
    use rcgen::{CertifiedKey, generate_simple_self_signed};
    use rustls::pki_types::ServerName;
    use rustls::{ClientConfig, ClientConnection, RootCertStore};
    use tungstenite::client::{IntoClientRequest, client_with_config};
    use tungstenite::http::HeaderValue;
    use tungstenite::protocol::WebSocketConfig;

    use super::*;

    const TOKEN: &str = "0123456789abcdef0123456789abcdef";

    #[derive(Default)]
    struct RemoteTestSecretStore {
        values: Mutex<BTreeMap<String, String>>,
    }

    impl SecretStore for RemoteTestSecretStore {
        fn load(&self, key: &SecretKey) -> Result<Option<SecretValue>, SecretStoreError> {
            self.values
                .lock()
                .map_err(|_| SecretStoreError::Unavailable)?
                .get(key.as_str())
                .map(|value| SecretValue::new(value.clone()))
                .transpose()
        }
    }

    impl MutableSecretStore for RemoteTestSecretStore {
        fn store_atomic(
            &self,
            key: &SecretKey,
            value: &SecretValue,
        ) -> Result<(), SecretStoreError> {
            self.values
                .lock()
                .map_err(|_| SecretStoreError::Unavailable)?
                .insert(key.as_str().to_string(), value.expose_secret().to_string());
            Ok(())
        }

        fn remove(&self, key: &SecretKey) -> Result<bool, SecretStoreError> {
            Ok(self
                .values
                .lock()
                .map_err(|_| SecretStoreError::Unavailable)?
                .remove(key.as_str())
                .is_some())
        }
    }

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

    fn debugger_start_body(session_id: &str, source: &str) -> Vec<u8> {
        encode_request(&RequestEnvelope {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            request: ProtocolRequest::DebuggerSessionStart(DebuggerSessionStartRequest {
                principal: Principal {
                    id: "debugger-operator".into(),
                },
                capabilities: CapabilitySet::new([CAPABILITY_DEBUGGER_CONTROL]),
                session_id: session_id.into(),
                source: source.into(),
                expected_revision: Some(Revision(7)),
                timeout_ms: 300_000,
            }),
        })
        .unwrap()
    }

    fn debugger_presentation_body(
        session_id: &str,
        effect_id: &str,
        expected_revision: Revision,
    ) -> Vec<u8> {
        encode_request(&RequestEnvelope {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            request: ProtocolRequest::DebuggerPresentationAcknowledge(
                DebuggerPresentationAcknowledgeRequest {
                    principal: Principal {
                        id: "debugger-operator".into(),
                    },
                    capabilities: CapabilitySet::new([
                        CAPABILITY_DEBUGGER_CONTROL,
                        CAPABILITY_UI_PRESENTATION,
                    ]),
                    session_id: session_id.into(),
                    effect_id: effect_id.into(),
                    expected_revision,
                    outcome: DebuggerPresentationOutcome::Applied {
                        node_id: "remote-fleet".into(),
                        focused_node_id: None,
                    },
                },
            ),
        })
        .unwrap()
    }

    fn debugger_cancel_body(session_id: &str, dry_run: bool) -> Vec<u8> {
        encode_request(&RequestEnvelope {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            request: ProtocolRequest::Command(CommandEnvelope {
                schema_version: DOMAIN_SCHEMA_VERSION,
                command_id: CommandId::new("remote-debugger-cancel").unwrap(),
                idempotency_key: IdempotencyKey::new("remote-debugger-cancel").unwrap(),
                expected_revision: Some(Revision(7)),
                principal: Principal {
                    id: "debugger-operator".into(),
                },
                capabilities: CapabilitySet::new([CAPABILITY_DEBUGGER_CONTROL]),
                origin: CommandOrigin::Gui,
                confirmation: if dry_run {
                    Confirmation::NotRequired
                } else {
                    Confirmation::Confirmed
                },
                dry_run,
                command: Command::DebuggerCancel {
                    session_id: session_id.into(),
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

    fn language_pack_request(path: &str, extra_headers: &str) -> Vec<u8> {
        format!(
            "GET {path} HTTP/1.1\r\nHost: localhost\r\nAccept: application/json\r\n{extra_headers}\r\n"
        )
        .into_bytes()
    }

    fn console_get_request(path: &str, token: Option<&str>, admin_token: Option<&str>) -> Vec<u8> {
        let authorization = token
            .map(|token| format!("Authorization: Bearer {token}\r\n"))
            .unwrap_or_default();
        let admin = admin_token
            .map(|token| format!("X-Leserpent-Admin-Token: {token}\r\n"))
            .unwrap_or_default();
        format!(
            "GET {path} HTTP/1.1\r\nHost: localhost\r\nAccept: application/json\r\n{authorization}{admin}\r\n"
        )
        .into_bytes()
    }

    fn console_post_request(
        path: &str,
        body: &[u8],
        token: Option<&str>,
        admin_token: Option<&str>,
        intent: Option<&str>,
        json: bool,
    ) -> Vec<u8> {
        let authorization = token
            .map(|token| format!("Authorization: Bearer {token}\r\n"))
            .unwrap_or_default();
        let admin = admin_token
            .map(|token| format!("X-Leserpent-Admin-Token: {token}\r\n"))
            .unwrap_or_default();
        let intent = intent
            .map(|intent| format!("X-Leserpent-Intent: {intent}\r\n"))
            .unwrap_or_default();
        let content_type = if json {
            "Content-Type: application/json\r\n"
        } else {
            ""
        };
        format!(
            "POST {path} HTTP/1.1\r\nHost: localhost\r\nAccept: application/json\r\n{authorization}{admin}{intent}{content_type}Content-Length: {}\r\n\r\n",
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

    fn tls_get_client(
        address: SocketAddr,
        certificate: rustls::pki_types::CertificateDer<'static>,
        path: impl Into<String>,
        token: Option<&'static str>,
        admin_token: Option<&'static str>,
    ) -> thread::JoinHandle<Vec<u8>> {
        let path = path.into();
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
                .write_all(&console_get_request(&path, token, admin_token))
                .unwrap();
            read_response(&mut stream)
        })
    }

    fn tls_console_post_client(
        address: SocketAddr,
        certificate: rustls::pki_types::CertificateDer<'static>,
        path: impl Into<String>,
        body: Vec<u8>,
        json: bool,
    ) -> thread::JoinHandle<Vec<u8>> {
        let path = path.into();
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
                .write_all(&console_post_request(
                    &path,
                    &body,
                    Some(TOKEN),
                    Some(TOKEN),
                    Some("mutate"),
                    json,
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
        assert_eq!(parsed.body.as_slice(), body.as_slice());
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
        assert_eq!(bootstrap.body.as_slice(), b"{}");
        let provisioning = read_http_request(
            &mut Cursor::new(request_at(TOKEN, "/v1/provisioning", b"{}")),
            TOKEN.as_bytes(),
        )
        .unwrap();
        assert_eq!(provisioning.route, HttpRoute::Provisioning);
        assert_eq!(provisioning.body.as_slice(), b"{}");
        let retirement = read_http_request(
            &mut Cursor::new(request_at(TOKEN, "/v1/retirement", b"{}")),
            TOKEN.as_bytes(),
        )
        .unwrap();
        assert_eq!(retirement.route, HttpRoute::Retirement);
        assert_eq!(retirement.body.as_slice(), b"{}");
        let daemon_retirement = read_http_request(
            &mut Cursor::new(request_at(TOKEN, "/v1/daemon-retirement", b"{}")),
            TOKEN.as_bytes(),
        )
        .unwrap();
        assert_eq!(daemon_retirement.route, HttpRoute::DaemonRetirement);
        assert_eq!(daemon_retirement.body.as_slice(), b"{}");
        let leselang_export = read_http_request(
            &mut Cursor::new(request_at(TOKEN, "/v1/leselang-export", b"{}")),
            TOKEN.as_bytes(),
        )
        .unwrap();
        assert_eq!(leselang_export.route, HttpRoute::LeselangExport);
        assert_eq!(leselang_export.body.as_slice(), b"{}");

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
    fn language_pack_routes_are_exact_public_gets_and_reject_credentials() {
        let catalog = read_http_request(
            &mut Cursor::new(language_pack_request(
                "/language-packs/catalog.json",
                "Cache-Control: no-cache\r\n",
            )),
            TOKEN.as_bytes(),
        )
        .unwrap();
        assert!(matches!(catalog.route, HttpRoute::LanguagePack(_)));
        assert!(catalog.body.is_empty());
        assert_eq!(catalog.writer_fence, None);
        let HttpRoute::LanguagePack(catalog_asset) = catalog.route else {
            unreachable!();
        };
        assert_eq!(
            catalog_asset,
            language_packs::find("/language-packs/catalog.json").unwrap()
        );

        let pack = read_http_request(
            &mut Cursor::new(language_pack_request("/language-packs/pt-BR.json", "")),
            TOKEN.as_bytes(),
        )
        .unwrap();
        assert!(matches!(pack.route, HttpRoute::LanguagePack(_)));

        for header in [
            format!("Authorization: Bearer {TOKEN}\r\n"),
            "X-Leserpent-Admin-Token: secret\r\n".to_string(),
        ] {
            let error = read_http_request(
                &mut Cursor::new(language_pack_request(
                    "/language-packs/catalog.json",
                    &header,
                )),
                TOKEN.as_bytes(),
            )
            .unwrap_err();
            assert!(matches!(error.status, HttpStatus::BadRequest));
            assert_eq!(error.code, "language_pack_credentials_forbidden");
        }

        for request in [
            language_pack_request("/language-packs/en.json", ""),
            b"POST /language-packs/catalog.json HTTP/1.1\r\nHost: localhost\r\nContent-Length: 0\r\n\r\n".to_vec(),
            b"GET /language-packs/catalog.json?cache=false HTTP/1.1\r\nHost: localhost\r\n\r\n".to_vec(),
            b"GET /language-packs/catalog.json HTTP/1.1\r\nHost: localhost\r\nContent-Length: 1\r\n\r\nx".to_vec(),
        ] {
            assert!(read_http_request(&mut Cursor::new(request), TOKEN.as_bytes()).is_err());
        }
    }

    #[test]
    fn rust_web_routes_separate_public_assets_from_authenticated_read_projections() {
        let document = read_http_request(
            &mut Cursor::new(console_get_request("/", None, None)),
            TOKEN.as_bytes(),
        )
        .unwrap();
        let HttpRoute::ConsoleAsset(document) = document.route else {
            panic!("root must resolve to an embedded console document");
        };
        assert!(document.document);
        assert_eq!(document.content_type, "text/html; charset=utf-8");

        for header in [
            format!("Authorization: Bearer {TOKEN}\r\n"),
            format!("X-Leserpent-Admin-Token: {TOKEN}\r\n"),
        ] {
            let request = format!("GET /app.js HTTP/1.1\r\nHost: localhost\r\n{header}\r\n");
            let error = read_http_request(&mut Cursor::new(request.into_bytes()), TOKEN.as_bytes())
                .unwrap_err();
            assert!(matches!(error.status, HttpStatus::BadRequest));
            assert_eq!(error.code, "console_asset_credentials_forbidden");
        }

        let runtimes = read_http_request(
            &mut Cursor::new(console_get_request(
                "/v1/runtimes?environment=prod%2Dcn",
                Some(TOKEN),
                Some(TOKEN),
            )),
            TOKEN.as_bytes(),
        )
        .unwrap();
        assert_eq!(
            runtimes.route,
            HttpRoute::ConsoleApi(ConsoleApiRoute::Runtimes(
                leserpent_domain::RuntimeListFilter {
                    environment: Some("prod-cn".into()),
                    cluster: None,
                    role: None,
                }
            ))
        );
        assert!(runtimes.body.is_empty());
        assert!(runtimes.writer_fence.is_none());

        let export = read_http_request(
            &mut Cursor::new(console_get_request(
                "/v1/persistence/export",
                Some(TOKEN),
                Some(TOKEN),
            )),
            TOKEN.as_bytes(),
        )
        .unwrap();
        assert_eq!(
            export.route,
            HttpRoute::ConsoleApi(ConsoleApiRoute::PersistenceExport)
        );

        let save = read_http_request(
            &mut Cursor::new(console_post_request(
                "/v1/persistence/save",
                &[],
                Some(TOKEN),
                Some(TOKEN),
                Some("mutate"),
                false,
            )),
            TOKEN.as_bytes(),
        )
        .unwrap();
        assert_eq!(
            save.route,
            HttpRoute::ConsoleApi(ConsoleApiRoute::PersistenceSave)
        );

        let plan_body = br#"{"name":"Runtime A","endpoint":"https://runtime.invalid"}"#;
        let registration_plan = read_http_request(
            &mut Cursor::new(console_post_request(
                "/v1/runtimes/registration-plan",
                plan_body,
                Some(TOKEN),
                Some(TOKEN),
                Some("mutate"),
                true,
            )),
            TOKEN.as_bytes(),
        )
        .unwrap();
        assert_eq!(
            registration_plan.route,
            HttpRoute::ConsoleApi(ConsoleApiRoute::RegistrationPlan)
        );
        assert_eq!(registration_plan.body.as_slice(), plan_body);

        let secret_body = br#"{"pairingToken":"debug-redaction-secret"}"#;
        let registration = read_http_request(
            &mut Cursor::new(console_post_request(
                "/v1/runtimes/register",
                secret_body,
                Some(TOKEN),
                Some(TOKEN),
                Some("mutate"),
                true,
            )),
            TOKEN.as_bytes(),
        )
        .unwrap();
        assert_eq!(registration.body.as_slice(), secret_body);
        let debug = format!("{registration:?}");
        assert!(debug.contains("body_len"));
        assert!(!debug.contains("debug-redaction-secret"));

        let cleanup_body =
            br#"{"planToken":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}"#;
        let cleanup = read_http_request(
            &mut Cursor::new(console_post_request(
                "/v1/runtimes/delete-unobserved?cluster=edge",
                cleanup_body,
                Some(TOKEN),
                Some(TOKEN),
                Some("mutate"),
                true,
            )),
            TOKEN.as_bytes(),
        )
        .unwrap();
        assert_eq!(
            cleanup.route,
            HttpRoute::ConsoleApi(ConsoleApiRoute::RuntimeCleanup(
                web_console::CleanupKind::Unobserved,
                leserpent_domain::RuntimeListFilter {
                    environment: None,
                    cluster: Some("edge".into()),
                    role: None,
                }
            ))
        );
        assert_eq!(cleanup.body.as_slice(), cleanup_body);
        let oversized_cleanup = console_post_request(
            "/v1/runtimes/delete-unobserved",
            &vec![b'x'; web_console::MAX_CLEANUP_REQUEST_BYTES + 1],
            Some(TOKEN),
            Some(TOKEN),
            Some("mutate"),
            true,
        );
        assert!(matches!(
            read_http_request(&mut Cursor::new(oversized_cleanup), TOKEN.as_bytes())
                .unwrap_err()
                .status,
            HttpStatus::PayloadTooLarge
        ));

        let refresh = read_http_request(
            &mut Cursor::new(console_post_request(
                "/v1/fleet/refresh-status?role=edge",
                &[],
                Some(TOKEN),
                Some(TOKEN),
                Some("mutate"),
                false,
            )),
            TOKEN.as_bytes(),
        )
        .unwrap();
        assert_eq!(
            refresh.route,
            HttpRoute::ConsoleApi(ConsoleApiRoute::FleetRefreshStatus(
                leserpent_domain::RuntimeListFilter {
                    environment: None,
                    cluster: None,
                    role: Some("edge".into()),
                }
            ))
        );

        let delete = read_http_request(
            &mut Cursor::new(console_post_request(
                "/v1/runtimes/runtime%3Aedge/delete",
                &[],
                Some(TOKEN),
                Some(TOKEN),
                Some("mutate"),
                false,
            )),
            TOKEN.as_bytes(),
        )
        .unwrap();
        assert_eq!(
            delete.route,
            HttpRoute::ConsoleApi(ConsoleApiRoute::RuntimeDelete(
                RuntimeId::new("runtime:edge").unwrap()
            ))
        );

        for request in [
            console_post_request(
                "/v1/fleet/refresh-status",
                &[],
                Some(TOKEN),
                Some(TOKEN),
                None,
                false,
            ),
            console_post_request(
                "/v1/runtimes/registration-plan",
                plan_body,
                Some(TOKEN),
                Some(TOKEN),
                Some("mutate"),
                false,
            ),
        ] {
            assert!(read_http_request(&mut Cursor::new(request), TOKEN.as_bytes()).is_err());
        }

        let unauthenticated = read_http_request(
            &mut Cursor::new(console_get_request("/v1/capabilities", None, None)),
            TOKEN.as_bytes(),
        )
        .unwrap_err();
        assert!(matches!(unauthenticated.status, HttpStatus::Unauthorized));

        let ambiguous = read_http_request(
            &mut Cursor::new(console_get_request(
                "/v1/capabilities",
                Some(TOKEN),
                Some("fedcba9876543210fedcba9876543210"),
            )),
            TOKEN.as_bytes(),
        )
        .unwrap_err();
        assert!(matches!(ambiguous.status, HttpStatus::Unauthorized));
        assert_eq!(ambiguous.code, "ambiguous_credentials");

        let invalid_query = read_http_request(
            &mut Cursor::new(console_get_request(
                "/v1/runtimes?role=a&role=b",
                Some(TOKEN),
                Some(TOKEN),
            )),
            TOKEN.as_bytes(),
        )
        .unwrap_err();
        assert!(matches!(invalid_query.status, HttpStatus::BadRequest));
        assert_eq!(invalid_query.code, "invalid_console_query");

        let wrong_method = read_http_request(
            &mut Cursor::new(request_at(TOKEN, "/v1/capabilities", b"{}")),
            TOKEN.as_bytes(),
        )
        .unwrap_err();
        assert!(matches!(wrong_method.status, HttpStatus::MethodNotAllowed));
    }

    #[test]
    fn rust_web_console_serves_real_runtime_state_over_tls() {
        let CertifiedKey { cert, signing_key } =
            generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
        let certificate_path = temp_path("rust-web", "crt");
        let key_path = temp_path("rust-web", "key");
        let database_path = temp_path("rust-web", "sqlite");
        fs::write(&certificate_path, cert.pem()).unwrap();
        fs::write(&key_path, signing_key.serialize_pem()).unwrap();
        #[cfg(unix)]
        fs::set_permissions(&key_path, fs::Permissions::from_mode(0o600)).unwrap();

        let mut runtime = ControlRuntime::open(&database_path).unwrap();
        runtime
            .register_runtime(
                RuntimeId::new("runtime-rust-web").unwrap(),
                "Rust Web runtime",
                "https://runtime.invalid",
            )
            .unwrap();
        let mut server = RemoteServer::bind(
            "127.0.0.1:0".parse().unwrap(),
            &certificate_path,
            &key_path,
            TOKEN,
        )
        .unwrap();
        let address = server.local_addr().unwrap();

        let document = complete_tls_request(
            &mut server,
            &mut runtime,
            tls_get_client(address, cert.der().clone(), "/", None, None),
        );
        assert!(document.starts_with(b"HTTP/1.1 200 OK\r\n"));
        let document_head =
            std::str::from_utf8(&document[..find_header_end(&document).unwrap()]).unwrap();
        assert!(document_head.contains("Content-Type: text/html; charset=utf-8\r\n"));
        assert!(document_head.contains("Content-Security-Policy: default-src 'self';"));
        assert!(document_head.contains("X-Frame-Options: DENY\r\n"));
        assert!(document[find_header_end(&document).unwrap()..].starts_with(b"<!DOCTYPE html>"));

        let unauthorized = complete_tls_request(
            &mut server,
            &mut runtime,
            tls_get_client(address, cert.der().clone(), "/v1/runtimes", None, None),
        );
        assert!(unauthorized.starts_with(b"HTTP/1.1 401 Unauthorized\r\n"));
        assert!(
            unauthorized
                .windows(b"WWW-Authenticate: Bearer\r\n".len())
                .any(|window| window == b"WWW-Authenticate: Bearer\r\n")
        );

        let runtimes = complete_tls_request(
            &mut server,
            &mut runtime,
            tls_get_client(
                address,
                cert.der().clone(),
                "/v1/runtimes",
                Some(TOKEN),
                Some(TOKEN),
            ),
        );
        assert!(runtimes.starts_with(b"HTTP/1.1 200 OK\r\n"));
        let body_start = find_header_end(&runtimes).unwrap();
        let projection: serde_json::Value =
            serde_json::from_slice(&runtimes[body_start..]).unwrap();
        assert_eq!(projection["runtimes"][0]["runtimeId"], "runtime-rust-web");
        assert_eq!(projection["runtimes"][0]["name"], "Rust Web runtime");
        let encoded = std::str::from_utf8(&runtimes[body_start..]).unwrap();
        for forbidden in ["adminToken", "pairingToken", "continuation", "secret"] {
            assert!(
                !encoded.contains(forbidden),
                "TLS projection leaked {forbidden}"
            );
        }

        let export = complete_tls_request(
            &mut server,
            &mut runtime,
            tls_get_client(
                address,
                cert.der().clone(),
                "/v1/persistence/export",
                Some(TOKEN),
                Some(TOKEN),
            ),
        );
        assert!(export.starts_with(b"HTTP/1.1 200 OK\r\n"));
        let export_head =
            std::str::from_utf8(&export[..find_header_end(&export).unwrap()]).unwrap();
        assert!(export_head.contains("Cache-Control: no-store\r\n"));
        assert!(export_head.contains(
            "Content-Disposition: attachment; filename=\"leserpent-control-plane-state.json\"\r\n"
        ));
        let exported: serde_json::Value =
            serde_json::from_slice(&export[find_header_end(&export).unwrap()..]).unwrap();
        assert_eq!(exported["schemaVersion"], 1);
        assert_eq!(exported["runtimes"][0]["runtimeId"], "runtime-rust-web");

        drop(runtime);
        fs::remove_file(certificate_path).unwrap();
        fs::remove_file(key_path).unwrap();
        fs::remove_file(database_path).unwrap();
    }

    #[test]
    fn rust_web_mutations_are_explicit_writer_fenced_over_real_tls() {
        let CertifiedKey { cert, signing_key } =
            generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
        let certificate_path = temp_path("rust-web-write", "crt");
        let key_path = temp_path("rust-web-write", "key");
        let database_path = temp_path("rust-web-write", "sqlite");
        fs::write(&certificate_path, cert.pem()).unwrap();
        fs::write(&key_path, signing_key.serialize_pem()).unwrap();
        #[cfg(unix)]
        fs::set_permissions(&key_path, fs::Permissions::from_mode(0o600)).unwrap();

        let mut runtime = ControlRuntime::open(&database_path).unwrap();
        let runtime_a = RuntimeId::new("runtime-web-write-a").unwrap();
        let runtime_b = RuntimeId::new("runtime-web-write-b").unwrap();
        for (runtime_id, name) in [
            (runtime_a.clone(), "Rust Web write A"),
            (runtime_b.clone(), "Rust Web write B"),
        ] {
            runtime
                .register_runtime(
                    runtime_id,
                    name,
                    format!(
                        "https://{}.invalid",
                        name.replace(' ', "-").to_ascii_lowercase()
                    ),
                )
                .unwrap();
        }
        let writer_a = "0123456789abcdef0123456789abcdef";
        let writer_b = "fedcba9876543210fedcba9876543210";
        let generation = runtime.claim_authority_writer(writer_a).unwrap().generation;
        let writer_fence = AuthorityWriterFence {
            generation,
            writer_id: writer_a.into(),
        };
        let mut server = RemoteServer::bind(
            "127.0.0.1:0".parse().unwrap(),
            &certificate_path,
            &key_path,
            TOKEN,
        )
        .unwrap();
        let address = server.local_addr().unwrap();

        let disabled = complete_tls_request(
            &mut server,
            &mut runtime,
            tls_console_post_client(
                address,
                cert.der().clone(),
                "/v1/fleet/refresh-status",
                Vec::new(),
                false,
            ),
        );
        assert!(disabled.starts_with(b"HTTP/1.1 503 Service Unavailable\r\n"));
        let disabled_body: serde_json::Value =
            serde_json::from_slice(&disabled[find_header_end(&disabled).unwrap()..]).unwrap();
        assert_eq!(disabled_body["error"], "web_console_writer_disabled");

        let save_disabled = complete_tls_request(
            &mut server,
            &mut runtime,
            tls_console_post_client(
                address,
                cert.der().clone(),
                "/v1/persistence/save",
                Vec::new(),
                false,
            ),
        );
        assert!(save_disabled.starts_with(b"HTTP/1.1 503 Service Unavailable\r\n"));

        server.web_console_writer = Some(writer_fence.clone());
        let capabilities = complete_tls_request(
            &mut server,
            &mut runtime,
            tls_get_client(
                address,
                cert.der().clone(),
                "/v1/capabilities",
                Some(TOKEN),
                Some(TOKEN),
            ),
        );
        let capabilities: serde_json::Value =
            serde_json::from_slice(&capabilities[find_header_end(&capabilities).unwrap()..])
                .unwrap();
        assert_eq!(capabilities["webConsole"]["mutationAvailable"], true);
        assert_eq!(capabilities["webConsole"]["cleanupAvailable"], true);
        assert_eq!(capabilities["webConsole"]["cleanupAtomicTargetLimit"], 128);
        assert_eq!(capabilities["webConsole"]["registrationAvailable"], false);
        assert_eq!(capabilities["webConsole"]["orchestraAvailable"], true);
        assert_eq!(
            capabilities["webConsole"]["orchestraMutationAvailable"],
            true
        );
        assert_eq!(
            capabilities["webConsole"]["orchestraSessionHandoffAvailable"],
            true
        );
        assert!(
            capabilities["routes"]
                .as_array()
                .unwrap()
                .iter()
                .any(|route| route == "/v1/orchestra/plans/{id}/{planId}/execute")
        );
        assert_eq!(
            capabilities["webConsole"]["registrationBlocker"],
            "crash_recoverable_registration_transaction"
        );

        let saved = complete_tls_request(
            &mut server,
            &mut runtime,
            tls_console_post_client(
                address,
                cert.der().clone(),
                "/v1/persistence/save",
                Vec::new(),
                false,
            ),
        );
        assert!(saved.starts_with(b"HTTP/1.1 200 OK\r\n"));
        let saved: serde_json::Value =
            serde_json::from_slice(&saved[find_header_end(&saved).unwrap()..]).unwrap();
        assert_eq!(saved["ok"], true);
        assert!(saved["throughSequence"].as_i64().is_some());
        let capabilities_after_save = complete_tls_request(
            &mut server,
            &mut runtime,
            tls_get_client(
                address,
                cert.der().clone(),
                "/v1/capabilities",
                Some(TOKEN),
                Some(TOKEN),
            ),
        );
        let capabilities_after_save: serde_json::Value = serde_json::from_slice(
            &capabilities_after_save[find_header_end(&capabilities_after_save).unwrap()..],
        )
        .unwrap();
        assert!(
            capabilities_after_save["persistence"]["lastSavedAt"]
                .as_str()
                .is_some_and(|value| value.ends_with('Z'))
        );

        let exported = complete_tls_request(
            &mut server,
            &mut runtime,
            tls_get_client(
                address,
                cert.der().clone(),
                "/v1/persistence/export",
                Some(TOKEN),
                Some(TOKEN),
            ),
        );
        assert!(exported.starts_with(b"HTTP/1.1 200 OK\r\n"));
        let imported = complete_tls_request(
            &mut server,
            &mut runtime,
            tls_console_post_client(
                address,
                cert.der().clone(),
                "/v1/persistence/import",
                exported[find_header_end(&exported).unwrap()..].to_vec(),
                true,
            ),
        );
        assert!(imported.starts_with(b"HTTP/1.1 200 OK\r\n"));
        let imported: serde_json::Value =
            serde_json::from_slice(&imported[find_header_end(&imported).unwrap()..]).unwrap();
        assert_eq!(imported["importedRuntimeCount"], 2);
        assert_eq!(imported["importedSessionCount"], 0);

        let orchestra_plan = complete_tls_request(
            &mut server,
            &mut runtime,
            tls_get_client(
                address,
                cert.der().clone(),
                "/v1/orchestra/plans/runtime-web-write-a",
                Some(TOKEN),
                Some(TOKEN),
            ),
        );
        assert!(orchestra_plan.starts_with(b"HTTP/1.1 200 OK\r\n"));
        let orchestra_plan: serde_json::Value =
            serde_json::from_slice(&orchestra_plan[find_header_end(&orchestra_plan).unwrap()..])
                .unwrap();
        let triage_revision = orchestra_plan["plans"]
            .as_array()
            .unwrap()
            .iter()
            .find(|plan| plan["planId"] == "runtime_triage")
            .unwrap()["revision"]
            .as_str()
            .unwrap();
        let orchestra_execute = complete_tls_request(
            &mut server,
            &mut runtime,
            tls_console_post_client(
                address,
                cert.der().clone(),
                "/v1/orchestra/plans/runtime-web-write-a/runtime_triage/execute",
                serde_json::to_vec(&serde_json::json!({
                    "confirmed": true,
                    "expectedRevision": triage_revision,
                    "approvedBy": "automatic",
                    "approvalNote": null,
                    "requestId": "request-tls-orchestra-0001",
                }))
                .unwrap(),
                true,
            ),
        );
        assert!(orchestra_execute.starts_with(b"HTTP/1.1 202 Accepted\r\n"));
        let orchestra_execute: serde_json::Value = serde_json::from_slice(
            &orchestra_execute[find_header_end(&orchestra_execute).unwrap()..],
        )
        .unwrap();
        let orchestra_run_id = orchestra_execute["run"]["runId"]
            .as_str()
            .unwrap()
            .to_string();

        let orchestra_history = complete_tls_request(
            &mut server,
            &mut runtime,
            tls_get_client(
                address,
                cert.der().clone(),
                "/v1/orchestra/runtimes/runtime-web-write-a/runs",
                Some(TOKEN),
                Some(TOKEN),
            ),
        );
        assert!(orchestra_history.starts_with(b"HTTP/1.1 200 OK\r\n"));
        let orchestra_history: serde_json::Value = serde_json::from_slice(
            &orchestra_history[find_header_end(&orchestra_history).unwrap()..],
        )
        .unwrap();
        assert_eq!(orchestra_history["runs"][0]["runId"], orchestra_run_id);

        let event_path =
            format!("/v1/orchestra/runtimes/runtime-web-write-a/runs/{orchestra_run_id}/events");
        let orchestra_events = complete_tls_request(
            &mut server,
            &mut runtime,
            tls_get_client(
                address,
                cert.der().clone(),
                event_path,
                Some(TOKEN),
                Some(TOKEN),
            ),
        );
        let orchestra_events: serde_json::Value = serde_json::from_slice(
            &orchestra_events[find_header_end(&orchestra_events).unwrap()..],
        )
        .unwrap();
        assert_eq!(orchestra_events["events"][0]["eventType"], "run_queued");

        let cancel_path =
            format!("/v1/orchestra/runtimes/runtime-web-write-a/runs/{orchestra_run_id}/cancel");
        let orchestra_cancel = complete_tls_request(
            &mut server,
            &mut runtime,
            tls_console_post_client(address, cert.der().clone(), cancel_path, Vec::new(), false),
        );
        assert!(orchestra_cancel.starts_with(b"HTTP/1.1 202 Accepted\r\n"));

        let retry_path =
            format!("/v1/orchestra/runtimes/runtime-web-write-a/runs/{orchestra_run_id}/retry");
        let orchestra_retry = complete_tls_request(
            &mut server,
            &mut runtime,
            tls_console_post_client(
                address,
                cert.der().clone(),
                retry_path,
                br#"{"confirmed":true,"approvedBy":"automatic","approvalNote":null,"requestId":"request-tls-orchestra-retry-0001"}"#.to_vec(),
                true,
            ),
        );
        assert!(orchestra_retry.starts_with(b"HTTP/1.1 202 Accepted\r\n"));
        let orchestra_retry: serde_json::Value =
            serde_json::from_slice(&orchestra_retry[find_header_end(&orchestra_retry).unwrap()..])
                .unwrap();
        assert_eq!(orchestra_retry["run"]["retriedFromRunId"], orchestra_run_id);

        let orchestra_fleet = complete_tls_request(
            &mut server,
            &mut runtime,
            tls_get_client(
                address,
                cert.der().clone(),
                "/v1/orchestra/runs",
                Some(TOKEN),
                Some(TOKEN),
            ),
        );
        let orchestra_fleet: serde_json::Value =
            serde_json::from_slice(&orchestra_fleet[find_header_end(&orchestra_fleet).unwrap()..])
                .unwrap();
        assert_eq!(orchestra_fleet["runCount"], 2);

        let orchestra_session = complete_tls_request(
            &mut server,
            &mut runtime,
            tls_console_post_client(
                address,
                cert.der().clone(),
                "/v1/orchestra/plans/runtime-web-write-a/session",
                br#"{"pipelineKind":"diagnostic","requestedBy":"operator","requestId":"request-tls-session-0001"}"#.to_vec(),
                true,
            ),
        );
        assert!(orchestra_session.starts_with(b"HTTP/1.1 200 OK\r\n"));
        let orchestra_session: serde_json::Value = serde_json::from_slice(
            &orchestra_session[find_header_end(&orchestra_session).unwrap()..],
        )
        .unwrap();
        assert_eq!(orchestra_session["session"]["status"], "running");
        assert_eq!(orchestra_session["run"]["planId"], "session_preparation");
        assert_eq!(orchestra_session["replayed"], false);
        let orchestra_session_id = orchestra_session["session"]["sessionId"]
            .as_str()
            .unwrap()
            .to_string();

        let orchestra_session_replay = complete_tls_request(
            &mut server,
            &mut runtime,
            tls_console_post_client(
                address,
                cert.der().clone(),
                "/v1/orchestra/plans/runtime-web-write-a/session",
                br#"{"pipelineKind":"diagnostic","requestedBy":"operator","requestId":"request-tls-session-0001"}"#.to_vec(),
                true,
            ),
        );
        let orchestra_session_replay: serde_json::Value = serde_json::from_slice(
            &orchestra_session_replay[find_header_end(&orchestra_session_replay).unwrap()..],
        )
        .unwrap();
        assert_eq!(orchestra_session_replay["replayed"], true);
        assert_eq!(
            orchestra_session_replay["session"]["sessionId"],
            orchestra_session_id
        );

        let session_detail = complete_tls_request(
            &mut server,
            &mut runtime,
            tls_get_client(
                address,
                cert.der().clone(),
                format!("/v1/sessions/{orchestra_session_id}"),
                Some(TOKEN),
                Some(TOKEN),
            ),
        );
        let session_detail: serde_json::Value =
            serde_json::from_slice(&session_detail[find_header_end(&session_detail).unwrap()..])
                .unwrap();
        assert_eq!(session_detail["status"], "running");

        let stopped_session = complete_tls_request(
            &mut server,
            &mut runtime,
            tls_console_post_client(
                address,
                cert.der().clone(),
                format!("/v1/sessions/{orchestra_session_id}/stop"),
                br#"{"requestedBy":"operator","reason":"TLS lifecycle proof complete"}"#.to_vec(),
                true,
            ),
        );
        assert!(stopped_session.starts_with(b"HTTP/1.1 200 OK\r\n"));
        let stopped_session: serde_json::Value =
            serde_json::from_slice(&stopped_session[find_header_end(&stopped_session).unwrap()..])
                .unwrap();
        assert_eq!(stopped_session["status"], "stopped");

        let direct_session = complete_tls_request(
            &mut server,
            &mut runtime,
            tls_console_post_client(
                address,
                cert.der().clone(),
                "/v1/sessions",
                br#"{"runtimeId":"runtime-web-write-b","pipelineKind":"inspection","requestedBy":"operator","requirements":[],"requestId":"request-tls-session-0002"}"#.to_vec(),
                true,
            ),
        );
        assert!(direct_session.starts_with(b"HTTP/1.1 200 OK\r\n"));
        let direct_session: serde_json::Value =
            serde_json::from_slice(&direct_session[find_header_end(&direct_session).unwrap()..])
                .unwrap();
        assert_eq!(direct_session["runtimeId"], "runtime-web-write-b");
        assert_eq!(direct_session["status"], "running");

        let sessions = complete_tls_request(
            &mut server,
            &mut runtime,
            tls_get_client(
                address,
                cert.der().clone(),
                "/v1/sessions",
                Some(TOKEN),
                Some(TOKEN),
            ),
        );
        let sessions: serde_json::Value =
            serde_json::from_slice(&sessions[find_header_end(&sessions).unwrap()..]).unwrap();
        assert_eq!(sessions["sessions"].as_array().unwrap().len(), 2);

        let plan = complete_tls_request(
            &mut server,
            &mut runtime,
            tls_console_post_client(
                address,
                cert.der().clone(),
                "/v1/runtimes/registration-plan",
                br#"{"name":"Runtime C","endpoint":"https://runtime-c.invalid"}"#.to_vec(),
                true,
            ),
        );
        assert!(plan.starts_with(b"HTTP/1.1 200 OK\r\n"));
        let plan: serde_json::Value =
            serde_json::from_slice(&plan[find_header_end(&plan).unwrap()..]).unwrap();
        assert_eq!(plan["allowed"], false);
        assert_eq!(
            plan["reason"],
            "rust_web_registration_transaction_unavailable"
        );

        let runtime_a_refresh_count = runtime
            .runtime_projection(&runtime_a)
            .unwrap()
            .refresh_count;
        let runtime_b_refresh_count = runtime
            .runtime_projection(&runtime_b)
            .unwrap()
            .refresh_count;
        let refreshed = complete_tls_request(
            &mut server,
            &mut runtime,
            tls_console_post_client(
                address,
                cert.der().clone(),
                "/v1/fleet/refresh-status",
                Vec::new(),
                false,
            ),
        );
        assert!(refreshed.starts_with(b"HTTP/1.1 200 OK\r\n"));
        assert_eq!(
            runtime
                .runtime_projection(&runtime_a)
                .unwrap()
                .refresh_count,
            runtime_a_refresh_count + 1
        );
        assert_eq!(
            runtime
                .runtime_projection(&runtime_b)
                .unwrap()
                .refresh_count,
            runtime_b_refresh_count + 1
        );

        let stale_cleanup_plan = complete_tls_request(
            &mut server,
            &mut runtime,
            tls_get_client(
                address,
                cert.der().clone(),
                "/v1/runtimes/cleanup-plan",
                Some(TOKEN),
                Some(TOKEN),
            ),
        );
        let stale_cleanup_plan: serde_json::Value = serde_json::from_slice(
            &stale_cleanup_plan[find_header_end(&stale_cleanup_plan).unwrap()..],
        )
        .unwrap();
        assert_eq!(stale_cleanup_plan["unobserved"]["runtimeCount"], 2);
        let stale_cleanup_token = stale_cleanup_plan["unobserved"]["planToken"]
            .as_str()
            .unwrap()
            .to_string();

        let deleted = complete_tls_request(
            &mut server,
            &mut runtime,
            tls_console_post_client(
                address,
                cert.der().clone(),
                "/v1/runtimes/runtime-web-write-a/delete",
                Vec::new(),
                false,
            ),
        );
        assert!(deleted.starts_with(b"HTTP/1.1 200 OK\r\n"));
        let deleted_body = &deleted[find_header_end(&deleted).unwrap()..];
        let deleted: serde_json::Value = serde_json::from_slice(deleted_body).unwrap();
        assert_eq!(deleted["deleted"], true);
        assert!(runtime.runtime_projection(&runtime_a).is_none());
        assert!(
            !std::str::from_utf8(deleted_body)
                .unwrap()
                .contains(writer_a)
        );

        let stale_cleanup = complete_tls_request(
            &mut server,
            &mut runtime,
            tls_console_post_client(
                address,
                cert.der().clone(),
                "/v1/runtimes/delete-unobserved",
                serde_json::to_vec(&serde_json::json!({
                    "planToken": stale_cleanup_token,
                }))
                .unwrap(),
                true,
            ),
        );
        assert!(stale_cleanup.starts_with(b"HTTP/1.1 409 Conflict\r\n"));
        let stale_cleanup: serde_json::Value =
            serde_json::from_slice(&stale_cleanup[find_header_end(&stale_cleanup).unwrap()..])
                .unwrap();
        assert_eq!(stale_cleanup["error"], "runtime_cleanup_plan_changed");
        assert!(runtime.runtime_projection(&runtime_b).is_some());

        let current_cleanup_plan = complete_tls_request(
            &mut server,
            &mut runtime,
            tls_get_client(
                address,
                cert.der().clone(),
                "/v1/runtimes/cleanup-plan",
                Some(TOKEN),
                Some(TOKEN),
            ),
        );
        let current_cleanup_plan: serde_json::Value = serde_json::from_slice(
            &current_cleanup_plan[find_header_end(&current_cleanup_plan).unwrap()..],
        )
        .unwrap();
        assert_eq!(current_cleanup_plan["unobserved"]["runtimeCount"], 1);
        let current_cleanup_body = serde_json::to_vec(&serde_json::json!({
            "planToken": current_cleanup_plan["unobserved"]["planToken"],
        }))
        .unwrap();
        let cleaned = complete_tls_request(
            &mut server,
            &mut runtime,
            tls_console_post_client(
                address,
                cert.der().clone(),
                "/v1/runtimes/delete-unobserved",
                current_cleanup_body.clone(),
                true,
            ),
        );
        assert!(cleaned.starts_with(b"HTTP/1.1 200 OK\r\n"));
        let cleaned: serde_json::Value =
            serde_json::from_slice(&cleaned[find_header_end(&cleaned).unwrap()..]).unwrap();
        assert_eq!(cleaned["removedRuntimeCount"], 1);
        assert_eq!(cleaned["replayed"], false);
        assert!(runtime.runtime_projection(&runtime_b).is_none());

        let replayed_cleanup = complete_tls_request(
            &mut server,
            &mut runtime,
            tls_console_post_client(
                address,
                cert.der().clone(),
                "/v1/runtimes/delete-unobserved",
                current_cleanup_body,
                true,
            ),
        );
        assert!(replayed_cleanup.starts_with(b"HTTP/1.1 200 OK\r\n"));
        let replayed_cleanup: serde_json::Value = serde_json::from_slice(
            &replayed_cleanup[find_header_end(&replayed_cleanup).unwrap()..],
        )
        .unwrap();
        assert_eq!(replayed_cleanup["removedRuntimeCount"], 1);
        assert_eq!(replayed_cleanup["replayed"], true);

        assert_eq!(
            runtime.claim_authority_writer(writer_b).unwrap().generation,
            2
        );
        let standby = complete_tls_request(
            &mut server,
            &mut runtime,
            tls_console_post_client(
                address,
                cert.der().clone(),
                "/v1/fleet/refresh-capabilities",
                Vec::new(),
                false,
            ),
        );
        assert!(standby.starts_with(b"HTTP/1.1 409 Conflict\r\n"));
        let standby: serde_json::Value =
            serde_json::from_slice(&standby[find_header_end(&standby).unwrap()..]).unwrap();
        assert_eq!(standby["error"], "web_console_writer_standby");

        let standby_capabilities = complete_tls_request(
            &mut server,
            &mut runtime,
            tls_get_client(
                address,
                cert.der().clone(),
                "/v1/capabilities",
                Some(TOKEN),
                Some(TOKEN),
            ),
        );
        let standby_capabilities: serde_json::Value = serde_json::from_slice(
            &standby_capabilities[find_header_end(&standby_capabilities).unwrap()..],
        )
        .unwrap();
        assert_eq!(
            standby_capabilities["webConsole"]["mutationAvailable"],
            false
        );
        assert_eq!(
            standby_capabilities["webConsole"]["orchestraMutationAvailable"],
            false
        );

        drop(runtime);
        fs::remove_file(certificate_path).unwrap();
        fs::remove_file(key_path).unwrap();
        fs::remove_file(database_path).unwrap();
    }

    #[test]
    fn rust_web_registration_crosses_real_tls_without_persisting_or_returning_the_secret() {
        let CertifiedKey { cert, signing_key } =
            generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
        let certificate_path = temp_path("rust-web-registration", "crt");
        let key_path = temp_path("rust-web-registration", "key");
        let database_path = temp_path("rust-web-registration", "sqlite");
        fs::write(&certificate_path, cert.pem()).unwrap();
        fs::write(&key_path, signing_key.serialize_pem()).unwrap();
        #[cfg(unix)]
        fs::set_permissions(&key_path, fs::Permissions::from_mode(0o600)).unwrap();

        let mut runtime = ControlRuntime::open(&database_path).unwrap();
        let writer_id = "cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd";
        let generation = runtime
            .claim_authority_writer(writer_id)
            .unwrap()
            .generation;
        let writer_fence = AuthorityWriterFence {
            generation,
            writer_id: writer_id.into(),
        };
        let targets = GewyvernTargetCatalog::default();
        let secrets = Arc::new(RemoteTestSecretStore::default());
        let mutable_secrets: Arc<dyn MutableSecretStore> = secrets;
        let registration =
            RuntimeTargetRegistrationAuthority::new(targets.clone(), mutable_secrets);
        let mut server = RemoteServer::bind(
            "127.0.0.1:0".parse().unwrap(),
            &certificate_path,
            &key_path,
            TOKEN,
        )
        .unwrap()
        .with_web_console_writer(writer_fence)
        .with_runtime_target_registration(registration);
        let address = server.local_addr().unwrap();

        let capabilities = complete_tls_request(
            &mut server,
            &mut runtime,
            tls_get_client(
                address,
                cert.der().clone(),
                "/v1/capabilities",
                Some(TOKEN),
                Some(TOKEN),
            ),
        );
        let capabilities: serde_json::Value =
            serde_json::from_slice(&capabilities[find_header_end(&capabilities).unwrap()..])
                .unwrap();
        assert_eq!(capabilities["webConsole"]["registrationAvailable"], true);
        assert!(capabilities["webConsole"]["registrationBlocker"].is_null());

        let plan = complete_tls_request(
            &mut server,
            &mut runtime,
            tls_console_post_client(
                address,
                cert.der().clone(),
                "/v1/runtimes/registration-plan",
                br#"{"name":"TLS Runtime","endpoint":"http://127.0.0.1:19411"}"#.to_vec(),
                true,
            ),
        );
        assert!(plan.starts_with(b"HTTP/1.1 200 OK\r\n"));
        let plan: serde_json::Value =
            serde_json::from_slice(&plan[find_header_end(&plan).unwrap()..]).unwrap();
        assert_eq!(plan["allowed"], true);
        assert_eq!(plan["action"], "create");
        let registration_body = serde_json::to_vec(&serde_json::json!({
            "name": "TLS Runtime",
            "endpoint": "http://127.0.0.1:19411",
            "pairingToken": "tls-registration-pairing-secret",
            "capabilities": [],
            "tags": { "environment": null, "cluster": null, "role": null },
            "fetchCapabilities": false,
            "sidecarEndpoint": null,
            "sidecarAdminToken": null,
            "registrationPlanToken": plan["planToken"],
        }))
        .unwrap();
        let registered = complete_tls_request(
            &mut server,
            &mut runtime,
            tls_console_post_client(
                address,
                cert.der().clone(),
                "/v1/runtimes/register",
                registration_body.clone(),
                true,
            ),
        );
        assert!(registered.starts_with(b"HTTP/1.1 200 OK\r\n"));
        let response_body = &registered[find_header_end(&registered).unwrap()..];
        let registered_value: serde_json::Value = serde_json::from_slice(response_body).unwrap();
        assert_eq!(registered_value["registrationReplayed"], false);
        assert!(
            !std::str::from_utf8(response_body)
                .unwrap()
                .contains("tls-registration-pairing-secret")
        );
        assert!(
            targets
                .contains(registered_value["runtimeId"].as_str().unwrap())
                .unwrap()
        );

        let replay = complete_tls_request(
            &mut server,
            &mut runtime,
            tls_console_post_client(
                address,
                cert.der().clone(),
                "/v1/runtimes/register",
                registration_body,
                true,
            ),
        );
        let replay: serde_json::Value =
            serde_json::from_slice(&replay[find_header_end(&replay).unwrap()..]).unwrap();
        assert_eq!(replay["registrationReplayed"], true);

        drop(runtime);
        fs::remove_file(certificate_path).unwrap();
        fs::remove_file(key_path).unwrap();
        fs::remove_file(database_path).unwrap();
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
    fn authenticated_debugger_vm_lifecycle_crosses_real_tls() {
        let CertifiedKey { cert, signing_key } =
            generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
        let certificate_path = temp_path("debugger", "crt");
        let key_path = temp_path("debugger", "key");
        let database_path = temp_path("debugger", "sqlite");
        fs::write(&certificate_path, cert.pem()).unwrap();
        fs::write(&key_path, signing_key.serialize_pem()).unwrap();
        #[cfg(unix)]
        fs::set_permissions(&key_path, fs::Permissions::from_mode(0o600)).unwrap();

        let debugger = Arc::new(Mutex::new(
            crate::DebuggerAuthority::for_database(&database_path).unwrap(),
        ));
        let mut server = RemoteServer::bind(
            "127.0.0.1:0".parse().unwrap(),
            &certificate_path,
            &key_path,
            TOKEN,
        )
        .unwrap()
        .with_debugger_authority(Arc::clone(&debugger));
        let address = server.local_addr().unwrap();
        let mut runtime = ControlRuntime::open(&database_path).unwrap();
        let session_id = "remote-debugger-session";

        let client = tls_post_client(
            address,
            cert.der().clone(),
            "/v1/wire",
            debugger_start_body(session_id, "fn main() = runtime.list()"),
        );
        let response = complete_tls_request(&mut server, &mut runtime, client);
        let body_start = find_header_end(&response).unwrap();
        assert!(matches!(
            decode_response(&response[body_start..]).unwrap().response,
            ProtocolResponse::DebuggerSessionStarted(ref started)
                if started.session.projection.session_id == session_id
                    && started.session.projection.revision == Revision(7)
                    && started.session.projection.state
                        == leselang_ui::DebuggerState::WaitingEffect
                    && started.session.document.revision == Revision(7)
        ));

        let presentation_session_id = "remote-debugger-presentation";
        let client = tls_post_client(
            address,
            cert.der().clone(),
            "/v1/wire",
            debugger_start_body(
                presentation_session_id,
                "fn main() = ui.assert_visible(node_id: \"remote-fleet\")",
            ),
        );
        let response = complete_tls_request(&mut server, &mut runtime, client);
        let body_start = find_header_end(&response).unwrap();
        let ProtocolResponse::DebuggerSessionStarted(started) =
            decode_response(&response[body_start..]).unwrap().response
        else {
            panic!("TLS debugger presentation session was not returned");
        };
        assert_eq!(
            started.session.pending_presentation,
            Some(PresentationOperation::AssertVisible {
                node_id: "remote-fleet".into(),
            })
        );
        let pending = started
            .session
            .projection
            .pending_effect
            .expect("TLS presentation remains suspended");
        let client = tls_post_client(
            address,
            cert.der().clone(),
            "/v1/wire",
            debugger_presentation_body(
                presentation_session_id,
                &pending.effect_id,
                started.session.projection.revision,
            ),
        );
        let response = complete_tls_request(&mut server, &mut runtime, client);
        let body_start = find_header_end(&response).unwrap();
        assert!(matches!(
            decode_response(&response[body_start..]).unwrap().response,
            ProtocolResponse::DebuggerPresentationAdvanced(ref advanced)
                if advanced.effect_id == pending.effect_id
                    && advanced.status == DebuggerPresentationStatus::Applied
                    && advanced.acknowledged_at_ms > 0
                    && advanced.session.projection.state
                        == leselang_ui::DebuggerState::Completed
                    && advanced.session.projection.revision == Revision(8)
                    && advanced.session.projection.pending_effect.is_none()
                    && advanced.session.pending_presentation.is_none()
        ));

        let client = tls_post_client(
            address,
            cert.der().clone(),
            "/v1/wire",
            debugger_cancel_body(session_id, true),
        );
        let response = complete_tls_request(&mut server, &mut runtime, client);
        let body_start = find_header_end(&response).unwrap();
        assert!(matches!(
            decode_response(&response[body_start..]).unwrap().response,
            ProtocolResponse::DebuggerCancelled(ref cancelled)
                if cancelled.status == DebuggerMutationStatus::Planned
                    && cancelled.audited_at_ms.is_none()
                    && cancelled.session.projection.state
                        == leselang_ui::DebuggerState::WaitingEffect
        ));

        let client = tls_post_client(
            address,
            cert.der().clone(),
            "/v1/wire",
            debugger_cancel_body(session_id, false),
        );
        let response = complete_tls_request(&mut server, &mut runtime, client);
        let body_start = find_header_end(&response).unwrap();
        assert!(matches!(
            decode_response(&response[body_start..]).unwrap().response,
            ProtocolResponse::DebuggerCancelled(ref cancelled)
                if cancelled.status == DebuggerMutationStatus::Applied
                    && cancelled.audited_at_ms.is_some()
                    && cancelled.session.projection.state
                        == leselang_ui::DebuggerState::Cancelled
                    && cancelled.session.projection.revision == Revision(8)
                    && cancelled.session.document.revision == Revision(8)
        ));

        drop(runtime);
        drop(server);
        drop(debugger);
        let debugger_root = database_path.with_file_name(format!(
            "{}.leselang-debugger",
            database_path.file_name().unwrap().to_string_lossy()
        ));
        fs::remove_dir_all(debugger_root).unwrap();
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
