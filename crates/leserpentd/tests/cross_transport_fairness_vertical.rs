#![cfg(unix)]

use std::fs;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::io::AsRawFd;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command as ProcessCommand, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use leserpent_domain::{
    CAPABILITY_RUNTIME_READ, CAPABILITY_RUNTIME_REGISTER, CapabilitySet, Command, CommandEnvelope,
    CommandId, CommandOrigin, CommandStatus, Confirmation, DOMAIN_SCHEMA_VERSION, IdempotencyKey,
    Principal, Query, QueryEnvelope, Revision, RuntimeId, RuntimeListFilter, RuntimeTags,
};
use leserpent_protocol::{
    AuthorityWriterClaimRequest, AuthorityWriterFence, CAPABILITY_AUTHORITY_WRITER,
    MAX_PROTOCOL_MESSAGE_BYTES, PROTOCOL_SCHEMA_VERSION, ProtocolEvent, ProtocolRequest,
    ProtocolResponse, RequestEnvelope, ResponseEnvelope, decode_event, decode_response,
    encode_request,
};
use leserpent_runtime::ControlRuntime;
use rcgen::{CertifiedKey, generate_simple_self_signed};
use rusqlite::Connection;
use rustls::pki_types::{CertificateDer, ServerName};
use rustls::{ClientConfig, ClientConnection, RootCertStore, StreamOwned};
use tungstenite::WebSocket;
use tungstenite::client::{IntoClientRequest, client_with_config};
use tungstenite::error::Error as WebSocketError;
use tungstenite::http::HeaderValue;
use tungstenite::protocol::{Message, WebSocketConfig};

const TOKEN: &str = "0123456789abcdef0123456789abcdef";
const WRITER_ID: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const FAIRNESS_WAVES: usize = 3;
const SLOW_IPC_PEERS_PER_WAVE: usize = 64;
const SLOW_HTTPS_FAIRNESS_WAVES: usize = 3;
const IPC_QUERIES_PER_SLOW_HTTPS_WAVE: usize = 4;
const REMOTE_READ_SHUTDOWN_PHASES: usize = 3;
const REMOTE_BACKLOG_PEERS_PER_PHASE: usize = 64;
const MAXIMUM_EVENT_SESSIONS: usize = 32;
const MAXIMUM_EVENT_SESSION_CYCLES: usize = 3;
const SLOW_EVENT_SEED_RUNTIMES: usize = 128;
const SLOW_EVENT_REVISIONS: usize = 24;
type StalledRemoteAttempt = (Duration, std::io::Result<Vec<u8>>);
type StalledRemoteTask = thread::JoinHandle<StalledRemoteAttempt>;
type EventClient = WebSocket<StreamOwned<ClientConnection, TcpStream>>;
static NEXT_TEMP_ROOT: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug)]
enum RemoteReadPhase {
    TlsHandshake,
    HttpHeader,
    AuthenticatedBody,
}

impl RemoteReadPhase {
    fn label(self) -> &'static str {
        match self {
            Self::TlsHandshake => "tls-handshake",
            Self::HttpHeader => "http-header",
            Self::AuthenticatedBody => "authenticated-body",
        }
    }
}

struct TempRoot(PathBuf);

impl TempRoot {
    fn new() -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let sequence = NEXT_TEMP_ROOT.fetch_add(1, Ordering::Relaxed);
        let root = PathBuf::from(format!(
            "/tmp/leserpent-cross-transport-{}-{unique}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct DaemonProcess {
    child: Option<Child>,
}

impl DaemonProcess {
    fn spawn(
        binary: &Path,
        database: &Path,
        socket: &Path,
        remote_address: SocketAddr,
        certificate: &Path,
        private_key: &Path,
    ) -> Self {
        let mut child = ProcessCommand::new(binary)
            .args([
                "--database",
                database.to_str().unwrap(),
                "--socket",
                socket.to_str().unwrap(),
                "--remote-listen",
                &remote_address.to_string(),
                "--remote-cert",
                certificate.to_str().unwrap(),
                "--remote-key",
                private_key.to_str().unwrap(),
            ])
            .env("LESERPENT_IPC_TOKEN", TOKEN)
            .env("LESERPENT_REMOTE_TOKEN", TOKEN)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if socket.exists()
                && UnixStream::connect(socket).is_ok()
                && TcpStream::connect(remote_address).is_ok()
            {
                break;
            }
            if let Some(status) = child.try_wait().unwrap() {
                let mut stderr = String::new();
                child
                    .stderr
                    .take()
                    .unwrap()
                    .read_to_string(&mut stderr)
                    .unwrap();
                panic!("leserpentd exited before dual-transport readiness ({status}): {stderr}");
            }
            assert!(
                Instant::now() < deadline,
                "leserpentd did not expose both transports"
            );
            thread::sleep(Duration::from_millis(10));
        }
        Self { child: Some(child) }
    }

    fn stop(self) {
        let _ = self.stop_with_budget(Duration::from_secs(5));
    }

    fn pid(&self) -> u32 {
        self.child.as_ref().unwrap().id()
    }

    fn stop_with_budget(mut self, budget: Duration) -> Duration {
        let mut child = self.child.take().unwrap();
        let started = Instant::now();
        assert_eq!(unsafe { libc::kill(child.id() as i32, libc::SIGTERM) }, 0);
        let deadline = started + budget;
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                assert!(
                    status.success(),
                    "leserpentd did not stop cleanly: {status}"
                );
                return started.elapsed();
            }
            if Instant::now() >= deadline {
                let pid = child.id();
                let resources = inspect_process_resources(pid);
                let fd_targets = inspect_process_fd_targets(pid);
                let wait_channel = inspect_process_wait_channel(pid);
                let _ = child.kill();
                let _ = child.wait();
                panic!(
                    "leserpentd did not stop after SIGTERM within {budget:?}: resources={resources:?}, wait_channel={wait_channel:?}, fd_targets={fd_targets:?}"
                );
            }
            thread::sleep(Duration::from_millis(10));
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProcessResources {
    open_fds: usize,
    tasks: usize,
}

fn inspect_process_resources(pid: u32) -> Option<ProcessResources> {
    #[cfg(target_os = "linux")]
    {
        let process = PathBuf::from(format!("/proc/{pid}"));
        return Some(ProcessResources {
            open_fds: fs::read_dir(process.join("fd")).unwrap().count(),
            tasks: fs::read_dir(process.join("task")).unwrap().count(),
        });
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = pid;
        None
    }
}

fn inspect_process_fd_targets(pid: u32) -> Vec<String> {
    #[cfg(target_os = "linux")]
    {
        let mut targets = fs::read_dir(format!("/proc/{pid}/fd"))
            .unwrap()
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let target = fs::read_link(entry.path()).ok()?;
                Some(format!(
                    "{}={}",
                    entry.file_name().to_string_lossy(),
                    target.display()
                ))
            })
            .collect::<Vec<_>>();
        targets.sort();
        return targets;
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = pid;
        Vec::new()
    }
}

fn inspect_process_wait_channel(pid: u32) -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        return fs::read_to_string(format!("/proc/{pid}/wchan"))
            .ok()
            .map(|value| value.trim().to_string());
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = pid;
        None
    }
}

fn wait_for_idle_process_resources(pid: u32) -> Option<ProcessResources> {
    inspect_process_resources(pid)?;
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let current = inspect_process_resources(pid).unwrap();
        let targets = inspect_process_fd_targets(pid);
        let socket_count = targets
            .iter()
            .filter(|target| target.contains("=socket:["))
            .count();
        let journal_open = targets.iter().any(|target| target.ends_with("-journal"));
        if socket_count == 2 && !journal_open {
            return Some(ProcessResources {
                open_fds: targets.len(),
                tasks: current.tasks,
            });
        }
        assert!(
            Instant::now() < deadline,
            "daemon did not expose an idle two-listener resource baseline: {current:?}, fd_targets={targets:?}"
        );
        thread::sleep(Duration::from_millis(10));
    }
}

fn wait_for_stalled_remote_resource(
    pid: u32,
    baseline: Option<ProcessResources>,
) -> Option<ProcessResources> {
    let baseline = baseline?;
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let current = inspect_process_resources(pid).unwrap();
        if current.open_fds == baseline.open_fds + 1 && current.tasks == baseline.tasks {
            return Some(current);
        }
        assert!(
            Instant::now() < deadline,
            "stalled remote read did not expose one connection FD without task growth above {baseline:?}: {current:?}, fd_targets={:?}",
            inspect_process_fd_targets(pid)
        );
        thread::sleep(Duration::from_millis(10));
    }
}

fn wait_for_event_session_resources(
    pid: u32,
    baseline: Option<ProcessResources>,
) -> Option<ProcessResources> {
    wait_for_event_session_count_resources(pid, baseline, MAXIMUM_EVENT_SESSIONS)
}

fn wait_for_event_session_count_resources(
    pid: u32,
    baseline: Option<ProcessResources>,
    sessions: usize,
) -> Option<ProcessResources> {
    let baseline = baseline?;
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let current = inspect_process_resources(pid).unwrap();
        if current.open_fds == baseline.open_fds + sessions && current.tasks == baseline.tasks {
            return Some(current);
        }
        assert!(
            Instant::now() < deadline,
            "event sessions did not expose {sessions} connection FDs without task growth above {baseline:?}: {current:?}, fd_targets={:?}",
            inspect_process_fd_targets(pid)
        );
        thread::sleep(Duration::from_millis(10));
    }
}

fn assert_process_resources_released(pid: u32) {
    #[cfg(target_os = "linux")]
    assert!(
        !PathBuf::from(format!("/proc/{pid}")).exists(),
        "exited daemon retained its proc resource directory"
    );
    #[cfg(not(target_os = "linux"))]
    let _ = pid;
}

impl Drop for DaemonProcess {
    fn drop(&mut self) {
        if let Some(child) = &mut self.child {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn writer_claim() -> RequestEnvelope {
    RequestEnvelope {
        schema_version: PROTOCOL_SCHEMA_VERSION,
        request: ProtocolRequest::AuthorityWriterClaim(AuthorityWriterClaimRequest {
            principal: Principal {
                id: "cross-transport-test".into(),
            },
            capabilities: CapabilitySet::new([CAPABILITY_AUTHORITY_WRITER]),
            writer_id: WRITER_ID.into(),
        }),
    }
}

fn runtime_list_query() -> RequestEnvelope {
    RequestEnvelope {
        schema_version: PROTOCOL_SCHEMA_VERSION,
        request: ProtocolRequest::Query(QueryEnvelope {
            schema_version: DOMAIN_SCHEMA_VERSION,
            principal: Principal {
                id: "cross-transport-test".into(),
            },
            capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
            query: Query::RuntimeList {
                filter: RuntimeListFilter::default(),
            },
        }),
    }
}

fn runtime_registration(cycle: usize) -> RequestEnvelope {
    RequestEnvelope {
        schema_version: PROTOCOL_SCHEMA_VERSION,
        request: ProtocolRequest::Command(CommandEnvelope {
            schema_version: DOMAIN_SCHEMA_VERSION,
            command_id: CommandId::new(format!("event-cycle-{cycle}-command")).unwrap(),
            idempotency_key: IdempotencyKey::new(format!("event-cycle-{cycle}-request")).unwrap(),
            expected_revision: None,
            principal: Principal {
                id: "cross-transport-test".into(),
            },
            capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_REGISTER]),
            origin: CommandOrigin::CompatibilityAdapter,
            confirmation: Confirmation::Confirmed,
            dry_run: false,
            command: Command::RuntimeRegister {
                runtime_id: RuntimeId::new(format!("event-cycle-{cycle}")).unwrap(),
                name: format!("Event Cycle {cycle}"),
                endpoint: format!("https://event-cycle-{cycle}.invalid"),
                sidecar_endpoint: None,
                tags: RuntimeTags::default(),
            },
        }),
    }
}

fn runtime_registration_update(update: usize, expected_revision: Revision) -> RequestEnvelope {
    RequestEnvelope {
        schema_version: PROTOCOL_SCHEMA_VERSION,
        request: ProtocolRequest::Command(CommandEnvelope {
            schema_version: DOMAIN_SCHEMA_VERSION,
            command_id: CommandId::new(format!("slow-event-update-{update}-command")).unwrap(),
            idempotency_key: IdempotencyKey::new(format!("slow-event-update-{update}-request"))
                .unwrap(),
            expected_revision: Some(expected_revision),
            principal: Principal {
                id: "cross-transport-test".into(),
            },
            capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_REGISTER]),
            origin: CommandOrigin::CompatibilityAdapter,
            confirmation: Confirmation::Confirmed,
            dry_run: false,
            command: Command::RuntimeRegistrationUpdate {
                runtime_id: RuntimeId::new("slow-event-runtime-000").unwrap(),
                name: format!("Slow Event Runtime {update:03}"),
                endpoint: "https://slow-event-runtime-000.invalid".into(),
                sidecar_endpoint: None,
                tags: RuntimeTags::default(),
            },
        }),
    }
}

fn send_ipc(socket: &Path, request: &RequestEnvelope) -> ResponseEnvelope {
    send_ipc_with_writer_fence(socket, request, None)
}

fn send_ipc_with_writer_fence(
    socket: &Path,
    request: &RequestEnvelope,
    writer_fence: Option<&AuthorityWriterFence>,
) -> ResponseEnvelope {
    let mut stream = UnixStream::connect(socket).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut frame = serde_json::to_vec(&serde_json::json!({
        "token": TOKEN,
        "writer_fence": writer_fence,
        "request": request,
    }))
    .unwrap();
    frame.push(b'\n');
    stream.write_all(&frame).unwrap();
    stream.shutdown(std::net::Shutdown::Write).unwrap();
    let mut response = Vec::new();
    stream.read_to_end(&mut response).unwrap();
    decode_response(&response).unwrap()
}

fn hold_ipc_accept(socket: &Path) -> UnixStream {
    let mut stream = UnixStream::connect(socket).unwrap();
    stream.write_all(b"{").unwrap();
    stream.flush().unwrap();
    thread::sleep(Duration::from_millis(100));
    stream
}

fn queue_slow_ipc_peer(socket: &Path) -> UnixStream {
    let mut stream = UnixStream::connect(socket).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    stream.write_all(b"{").unwrap();
    stream.flush().unwrap();
    stream
}

fn read_http_body(stream: &mut impl Read) -> Vec<u8> {
    let mut response = Vec::new();
    while !response.windows(4).any(|window| window == b"\r\n\r\n") {
        let mut byte = [0_u8; 1];
        stream.read_exact(&mut byte).unwrap();
        response.push(byte[0]);
    }
    let header_end = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .unwrap()
        + 4;
    let header = std::str::from_utf8(&response[..header_end]).unwrap();
    assert!(header.starts_with("HTTP/1.1 200 OK\r\n"));
    let content_length = header
        .split("\r\n")
        .find_map(|line| {
            line.strip_prefix("Content-Length: ")
                .and_then(|value| value.parse::<usize>().ok())
        })
        .unwrap();
    let mut body = vec![0_u8; content_length];
    stream.read_exact(&mut body).unwrap();
    body
}

fn connect_https(
    address: SocketAddr,
    certificate: CertificateDer<'static>,
) -> StreamOwned<ClientConnection, TcpStream> {
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
    socket
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    socket
        .set_write_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    StreamOwned::new(connection, socket)
}

fn connect_event_session(address: SocketAddr, certificate: CertificateDer<'static>) -> EventClient {
    connect_event_session_at_revision(address, certificate, Revision(0), 0, "shutdown-capacity")
}

fn connect_event_session_at_revision(
    address: SocketAddr,
    certificate: CertificateDer<'static>,
    expected_revision: Revision,
    expected_runtimes: usize,
    context: &str,
) -> EventClient {
    connect_event_session_with_receive_buffer(
        address,
        certificate,
        expected_revision,
        expected_runtimes,
        context,
        None,
    )
    .0
}

fn connect_event_session_with_receive_buffer(
    address: SocketAddr,
    certificate: CertificateDer<'static>,
    expected_revision: Revision,
    expected_runtimes: usize,
    context: &str,
    receive_buffer_bytes: Option<libc::c_int>,
) -> (EventClient, usize) {
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
    if let Some(receive_buffer_bytes) = receive_buffer_bytes {
        let result = unsafe {
            libc::setsockopt(
                socket.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_RCVBUF,
                (&raw const receive_buffer_bytes).cast(),
                std::mem::size_of_val(&receive_buffer_bytes) as libc::socklen_t,
            )
        };
        assert_eq!(
            result, 0,
            "{context} could not bound the TCP receive buffer"
        );
    }
    socket
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    socket
        .set_write_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let stream = StreamOwned::new(connection, socket);
    let mut request = format!("wss://localhost:{}/v1/events", address.port())
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
    let (mut websocket, response) = client_with_config(request, stream, Some(websocket_config))
        .unwrap_or_else(|error| panic!("{context} WebSocket handshake failed: {error}"));
    assert_eq!(response.status(), 101);
    assert_eq!(
        response.headers().get("Sec-WebSocket-Protocol").unwrap(),
        "leserpent.events.v1"
    );
    let initial = websocket.read().unwrap();
    let initial = initial.into_data();
    let initial_bytes = initial.len();
    let event = decode_event(&initial).unwrap();
    assert!(matches!(
        event.event,
        ProtocolEvent::RuntimeSnapshot {
            revision,
            resumed_after: None,
            ref runtimes,
        } if revision == expected_revision && runtimes.len() == expected_runtimes
    ));
    (websocket, initial_bytes)
}

fn read_runtime_snapshot(
    websocket: &mut EventClient,
    expected_revision: Revision,
    expected_previous: Revision,
    expected_runtimes: usize,
) {
    read_runtime_snapshot_containing(
        websocket,
        expected_revision,
        expected_previous,
        expected_runtimes,
        &format!("event-cycle-{}", expected_revision.0 - 1),
    );
}

fn read_runtime_snapshot_containing(
    websocket: &mut EventClient,
    expected_revision: Revision,
    expected_previous: Revision,
    expected_runtimes: usize,
    expected_runtime_id: &str,
) {
    for _ in 0..4 {
        let message = websocket.read().unwrap();
        match message {
            Message::Text(payload) => {
                let event = decode_event(payload.as_bytes()).unwrap();
                match event.event {
                    ProtocolEvent::Heartbeat { .. } => continue,
                    ProtocolEvent::RuntimeSnapshot {
                        revision,
                        resumed_after,
                        runtimes,
                    } => {
                        assert_eq!(revision, expected_revision);
                        assert_eq!(resumed_after, Some(expected_previous));
                        assert_eq!(runtimes.len(), expected_runtimes);
                        assert!(
                            runtimes
                                .iter()
                                .any(|runtime| runtime.id.as_str() == expected_runtime_id)
                        );
                        return;
                    }
                    event => {
                        panic!("unexpected event while waiting for runtime snapshot: {event:?}")
                    }
                }
            }
            Message::Binary(payload) => {
                let event = decode_event(&payload).unwrap();
                assert!(
                    matches!(event.event, ProtocolEvent::Heartbeat { .. }),
                    "unexpected binary event while waiting for runtime snapshot: {:?}",
                    event.event
                );
            }
            Message::Ping(_) | Message::Pong(_) | Message::Frame(_) => {}
            Message::Close(frame) => {
                panic!("event session closed before runtime snapshot: {frame:?}")
            }
        }
    }
    panic!("event session did not receive the expected runtime snapshot");
}

fn assert_event_session_closed_without_application_event(websocket: &mut EventClient) {
    for _ in 0..8 {
        match websocket.read() {
            Ok(Message::Text(_) | Message::Binary(_)) => {
                panic!("event session received an application event after daemon shutdown")
            }
            Ok(Message::Close(_)) => return,
            Ok(Message::Ping(_) | Message::Pong(_) | Message::Frame(_)) => {}
            Err(WebSocketError::Io(error))
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                panic!("event session remained open after daemon shutdown")
            }
            Err(_) => return,
        }
    }
    panic!("event session did not close after daemon shutdown");
}

fn drain_event_session_before_shutdown(websocket: &mut EventClient) -> usize {
    websocket.get_mut().sock.set_nonblocking(true).unwrap();
    let mut drained = 0;
    loop {
        match websocket.read() {
            Ok(Message::Text(payload)) => {
                decode_event(payload.as_bytes()).unwrap();
                drained += 1;
            }
            Ok(Message::Binary(payload)) => {
                decode_event(&payload).unwrap();
                drained += 1;
            }
            Ok(Message::Ping(_) | Message::Pong(_) | Message::Frame(_)) => {}
            Ok(Message::Close(_)) => panic!("event session closed before daemon shutdown"),
            Err(WebSocketError::Io(error)) if error.kind() == std::io::ErrorKind::WouldBlock => {
                break;
            }
            Err(error) => panic!("event session failed before daemon shutdown: {error}"),
        }
    }
    websocket.get_mut().sock.set_nonblocking(false).unwrap();
    drained
}

fn spawn_https_query(
    address: SocketAddr,
    certificate: CertificateDer<'static>,
) -> thread::JoinHandle<(Duration, ResponseEnvelope)> {
    thread::spawn(move || {
        let started = Instant::now();
        let mut stream = connect_https(address, certificate);
        let body = encode_request(&runtime_list_query()).unwrap();
        write!(
            stream,
            "POST /v1/wire HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {TOKEN}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
            body.len()
        )
        .unwrap();
        stream.write_all(&body).unwrap();
        let response = decode_response(&read_http_body(&mut stream)).unwrap();
        (started.elapsed(), response)
    })
}

fn spawn_stalled_remote_read(
    address: SocketAddr,
    certificate: CertificateDer<'static>,
    phase: RemoteReadPhase,
) -> (mpsc::Receiver<()>, StalledRemoteTask) {
    let (ready_tx, ready_rx) = mpsc::sync_channel(0);
    let handle = thread::spawn(move || {
        let started = Instant::now();
        let response = match phase {
            RemoteReadPhase::TlsHandshake => {
                let mut stream = TcpStream::connect(address).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                stream
                    .set_write_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                stream.write_all(&[0x16, 0x03, 0x03]).unwrap();
                stream.flush().unwrap();
                ready_tx.send(()).unwrap();
                let mut response = Vec::new();
                stream.read_to_end(&mut response).map(|_| response)
            }
            RemoteReadPhase::HttpHeader => {
                let mut stream = connect_https(address, certificate);
                write!(
                    stream,
                    "POST /v1/wire HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {TOKEN}\r\nContent-Type: application/json\r\n"
                )
                .unwrap();
                stream.flush().unwrap();
                ready_tx.send(()).unwrap();
                let mut response = Vec::new();
                stream.read_to_end(&mut response).map(|_| response)
            }
            RemoteReadPhase::AuthenticatedBody => {
                let mut stream = connect_https(address, certificate);
                write!(
                    stream,
                    "POST /v1/wire HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {TOKEN}\r\nContent-Type: application/json\r\nContent-Length: 1\r\n\r\n"
                )
                .unwrap();
                stream.flush().unwrap();
                ready_tx.send(()).unwrap();
                let mut response = Vec::new();
                stream.read_to_end(&mut response).map(|_| response)
            }
        };
        (started.elapsed(), response)
    });
    (ready_rx, handle)
}

fn spawn_authenticated_slow_https(
    address: SocketAddr,
    certificate: CertificateDer<'static>,
) -> (mpsc::Receiver<()>, StalledRemoteTask) {
    spawn_stalled_remote_read(address, certificate, RemoteReadPhase::AuthenticatedBody)
}

fn queue_incomplete_tls_backlog_peer(address: SocketAddr) -> TcpStream {
    let mut stream = TcpStream::connect(address).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    stream.write_all(&[0x16, 0x03, 0x03]).unwrap();
    stream.flush().unwrap();
    stream
}

fn spawn_ipc_query(socket: PathBuf) -> thread::JoinHandle<(Duration, ResponseEnvelope)> {
    thread::spawn(move || {
        let started = Instant::now();
        let response = send_ipc(&socket, &runtime_list_query());
        (started.elapsed(), response)
    })
}

fn inspect_owner_lease(database: &Path) -> (String, i64) {
    Connection::open(database)
        .unwrap()
        .query_row(
            "SELECT owner_token, lease_expires_at_unix_ms FROM runtime_owner WHERE id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap()
}

fn wait_for_owner_lease_extension(database: &Path, owner_token: &str, previous: i64) -> i64 {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let (current_owner, current_expiry) = inspect_owner_lease(database);
        assert_eq!(current_owner, owner_token);
        if current_expiry > previous {
            return current_expiry;
        }
        assert!(
            Instant::now() < deadline,
            "maintenance did not advance the owner heartbeat"
        );
        thread::sleep(Duration::from_millis(10));
    }
}

fn runtime_owner_count(database: &Path) -> i64 {
    Connection::open(database)
        .unwrap()
        .query_row("SELECT COUNT(*) FROM runtime_owner", [], |row| row.get(0))
        .unwrap()
}

fn authority_writer_generation(database: &Path) -> i64 {
    Connection::open(database)
        .unwrap()
        .query_row(
            "SELECT generation FROM authority_writer_fence WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .unwrap()
}

fn seed_slow_event_runtime_snapshot(database: &Path) {
    {
        let mut runtime = ControlRuntime::open(database).unwrap();
        for index in 0..SLOW_EVENT_SEED_RUNTIMES {
            let runtime_id = RuntimeId::new(format!("slow-event-runtime-{index:03}")).unwrap();
            runtime
                .register_runtime(
                    runtime_id,
                    format!("Slow Event Runtime {index:03} {}", "x".repeat(96)),
                    format!("https://slow-event-runtime-{index:03}.invalid"),
                )
                .unwrap();
        }
    }
    assert_eq!(runtime_owner_count(database), 0);
}

fn assert_slow_event_session_is_bounded_and_closed(websocket: &mut EventClient) -> usize {
    let mut buffered_snapshots = 0;
    for _ in 0..SLOW_EVENT_REVISIONS + 8 {
        match websocket.read() {
            Ok(Message::Text(payload)) => match decode_event(payload.as_bytes()).unwrap().event {
                ProtocolEvent::RuntimeSnapshot { .. } => buffered_snapshots += 1,
                ProtocolEvent::Heartbeat { .. } => {}
                event => panic!("slow event session received an unexpected event: {event:?}"),
            },
            Ok(Message::Binary(payload)) => match decode_event(&payload).unwrap().event {
                ProtocolEvent::RuntimeSnapshot { .. } => buffered_snapshots += 1,
                ProtocolEvent::Heartbeat { .. } => {}
                event => panic!("slow event session received an unexpected event: {event:?}"),
            },
            Ok(Message::Close(_)) => return buffered_snapshots,
            Ok(Message::Ping(_) | Message::Pong(_) | Message::Frame(_)) => {}
            Err(WebSocketError::Io(error))
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                panic!("slow event session remained open after bounded backpressure")
            }
            Err(_) => return buffered_snapshots,
        }
    }
    panic!("slow event session exceeded its bounded snapshot backlog");
}

fn sample_unchanged_process_resources(
    pid: u32,
    expected: Option<ProcessResources>,
) -> Option<ProcessResources> {
    let expected = expected?;
    for _ in 0..4 {
        thread::sleep(Duration::from_millis(25));
        let current = inspect_process_resources(pid).unwrap();
        assert_eq!(
            current,
            expected,
            "listener backlog changed daemon resources above {expected:?}: fd_targets={:?}",
            inspect_process_fd_targets(pid)
        );
    }
    Some(expected)
}

#[test]
fn https_and_maintenance_progress_across_repeated_saturated_ipc_waves() {
    let binary = PathBuf::from(env!("CARGO_BIN_EXE_leserpentd"));
    let root = TempRoot::new();
    let database = root.0.join("cross-transport.sqlite");
    let socket = root.0.join("cross-transport.sock");
    let certificate_path = root.0.join("remote.crt");
    let private_key_path = root.0.join("remote.key");
    let CertifiedKey { cert, signing_key } =
        generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    fs::write(&certificate_path, cert.pem()).unwrap();
    fs::write(&private_key_path, signing_key.serialize_pem()).unwrap();
    fs::set_permissions(&private_key_path, fs::Permissions::from_mode(0o600)).unwrap();
    let address = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap();

    let daemon = DaemonProcess::spawn(
        &binary,
        &database,
        &socket,
        address,
        &certificate_path,
        &private_key_path,
    );
    let claim = send_ipc(&socket, &writer_claim());
    assert!(matches!(
        claim.response,
        ProtocolResponse::AuthorityWriterClaimed(ref claim)
            if claim.generation == 1 && claim.writer_id == WRITER_ID && !claim.replayed
    ));
    let (owner_token, mut lease_expiry) = inspect_owner_lease(&database);
    let mut https_elapsed = Vec::with_capacity(FAIRNESS_WAVES);
    let mut wave_elapsed = Vec::with_capacity(FAIRNESS_WAVES);

    for _ in 0..FAIRNESS_WAVES {
        let gate = hold_ipc_accept(&socket);
        let (_, gated_expiry) = inspect_owner_lease(&database);
        assert!(gated_expiry >= lease_expiry);
        let slow_peers = (0..SLOW_IPC_PEERS_PER_WAVE)
            .map(|_| queue_slow_ipc_peer(&socket))
            .collect::<Vec<_>>();
        let https = spawn_https_query(address, cert.der().clone());
        let started = Instant::now();
        drop(gate);

        let (remote_elapsed, response) = https.join().unwrap();
        assert!(matches!(response.response, ProtocolResponse::Query(_)));
        assert!(
            remote_elapsed <= Duration::from_secs(5),
            "HTTPS query starved behind saturated IPC: {remote_elapsed:?}"
        );
        https_elapsed.push(remote_elapsed);
        for mut slow in slow_peers {
            let mut response = Vec::new();
            slow.read_to_end(&mut response).unwrap();
            assert!(response.is_empty());
        }
        let elapsed = started.elapsed();
        assert!(
            elapsed <= Duration::from_secs(5),
            "cross-transport fairness wave exceeded its budget: {elapsed:?}"
        );
        wave_elapsed.push(elapsed);
        lease_expiry = wait_for_owner_lease_extension(&database, &owner_token, gated_expiry);
    }

    let final_fence = Connection::open(&database)
        .unwrap()
        .query_row(
            "SELECT generation, writer_id FROM authority_writer_fence WHERE id = 1",
            [],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
        )
        .unwrap();
    assert_eq!(final_fence, (1, WRITER_ID.to_string()));
    println!(
        "cross-transport-fairness waves={} slow_ipc_peers_per_wave={} https_queries={} https_elapsed_ms={:?} wave_elapsed_ms={:?} maintenance_heartbeat_advanced_each_wave=true final_generation=1",
        FAIRNESS_WAVES,
        SLOW_IPC_PEERS_PER_WAVE,
        FAIRNESS_WAVES,
        https_elapsed
            .iter()
            .map(Duration::as_millis)
            .collect::<Vec<_>>(),
        wave_elapsed
            .iter()
            .map(Duration::as_millis)
            .collect::<Vec<_>>()
    );
    daemon.stop();
    assert!(!socket.exists());
}

#[test]
fn ipc_and_maintenance_progress_across_repeated_authenticated_slow_https_waves() {
    let binary = PathBuf::from(env!("CARGO_BIN_EXE_leserpentd"));
    let root = TempRoot::new();
    let database = root.0.join("slow-https.sqlite");
    let socket = root.0.join("slow-https.sock");
    let certificate_path = root.0.join("slow-https.crt");
    let private_key_path = root.0.join("slow-https.key");
    let CertifiedKey { cert, signing_key } =
        generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    fs::write(&certificate_path, cert.pem()).unwrap();
    fs::write(&private_key_path, signing_key.serialize_pem()).unwrap();
    fs::set_permissions(&private_key_path, fs::Permissions::from_mode(0o600)).unwrap();
    let address = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap();

    let daemon = DaemonProcess::spawn(
        &binary,
        &database,
        &socket,
        address,
        &certificate_path,
        &private_key_path,
    );
    let claim = send_ipc(&socket, &writer_claim());
    assert!(matches!(
        claim.response,
        ProtocolResponse::AuthorityWriterClaimed(ref claim)
            if claim.generation == 1 && claim.writer_id == WRITER_ID && !claim.replayed
    ));
    let (owner_token, mut lease_expiry) = inspect_owner_lease(&database);
    let mut ipc_elapsed = Vec::with_capacity(SLOW_HTTPS_FAIRNESS_WAVES);
    let mut slow_https_elapsed = Vec::with_capacity(SLOW_HTTPS_FAIRNESS_WAVES);
    let mut wave_elapsed = Vec::with_capacity(SLOW_HTTPS_FAIRNESS_WAVES);

    for _ in 0..SLOW_HTTPS_FAIRNESS_WAVES {
        let (slow_ready, slow_https) = spawn_authenticated_slow_https(address, cert.der().clone());
        slow_ready
            .recv_timeout(Duration::from_secs(5))
            .expect("authenticated slow HTTPS request did not enter its body-read window");
        let (_, gated_expiry) = inspect_owner_lease(&database);
        assert!(gated_expiry >= lease_expiry);
        let started = Instant::now();
        let queries = (0..IPC_QUERIES_PER_SLOW_HTTPS_WAVE)
            .map(|_| spawn_ipc_query(socket.clone()))
            .collect::<Vec<_>>();
        let mut current_ipc_elapsed = Vec::with_capacity(IPC_QUERIES_PER_SLOW_HTTPS_WAVE);
        for query in queries {
            let (elapsed, response) = query.join().unwrap();
            assert!(matches!(response.response, ProtocolResponse::Query(_)));
            assert!(
                elapsed <= Duration::from_secs(5),
                "IPC query starved behind authenticated slow HTTPS: {elapsed:?}"
            );
            current_ipc_elapsed.push(elapsed);
        }
        ipc_elapsed.push(current_ipc_elapsed);

        let (remote_elapsed, response) = slow_https.join().unwrap();
        let response = response.unwrap();
        assert!(
            remote_elapsed >= Duration::from_millis(2_500),
            "slow HTTPS peer did not consume the remote read budget: {remote_elapsed:?}"
        );
        assert!(
            remote_elapsed <= Duration::from_secs(5),
            "slow HTTPS peer exceeded the remote failure budget: {remote_elapsed:?}"
        );
        assert!(response.starts_with(b"HTTP/1.1 400 Bad Request\r\n"));
        assert!(
            !response
                .windows(TOKEN.len())
                .any(|window| window == TOKEN.as_bytes())
        );
        slow_https_elapsed.push(remote_elapsed);

        let elapsed = started.elapsed();
        assert!(
            elapsed <= Duration::from_secs(5),
            "slow-HTTPS fairness wave exceeded its budget: {elapsed:?}"
        );
        wave_elapsed.push(elapsed);
        lease_expiry = wait_for_owner_lease_extension(&database, &owner_token, gated_expiry);
    }

    let final_fence = Connection::open(&database)
        .unwrap()
        .query_row(
            "SELECT generation, writer_id FROM authority_writer_fence WHERE id = 1",
            [],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
        )
        .unwrap();
    assert_eq!(final_fence, (1, WRITER_ID.to_string()));
    println!(
        "slow-https-cross-transport-fairness waves={} ipc_queries_per_wave={} total_ipc_queries={} ipc_elapsed_ms={:?} slow_https_elapsed_ms={:?} wave_elapsed_ms={:?} maintenance_heartbeat_advanced_each_wave=true final_generation=1",
        SLOW_HTTPS_FAIRNESS_WAVES,
        IPC_QUERIES_PER_SLOW_HTTPS_WAVE,
        SLOW_HTTPS_FAIRNESS_WAVES * IPC_QUERIES_PER_SLOW_HTTPS_WAVE,
        ipc_elapsed
            .iter()
            .map(|wave| wave.iter().map(Duration::as_millis).collect::<Vec<_>>())
            .collect::<Vec<_>>(),
        slow_https_elapsed
            .iter()
            .map(Duration::as_millis)
            .collect::<Vec<_>>(),
        wave_elapsed
            .iter()
            .map(Duration::as_millis)
            .collect::<Vec<_>>()
    );
    daemon.stop();
    assert!(!socket.exists());
}

#[test]
fn sigterm_cancels_authenticated_slow_https_and_allows_immediate_restart() {
    let binary = PathBuf::from(env!("CARGO_BIN_EXE_leserpentd"));
    let root = TempRoot::new();
    let database = root.0.join("slow-https-shutdown.sqlite");
    let socket = root.0.join("slow-https-shutdown.sock");
    let certificate_path = root.0.join("slow-https-shutdown.crt");
    let private_key_path = root.0.join("slow-https-shutdown.key");
    let CertifiedKey { cert, signing_key } =
        generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    fs::write(&certificate_path, cert.pem()).unwrap();
    fs::write(&private_key_path, signing_key.serialize_pem()).unwrap();
    fs::set_permissions(&private_key_path, fs::Permissions::from_mode(0o600)).unwrap();
    let address = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap();

    let daemon = DaemonProcess::spawn(
        &binary,
        &database,
        &socket,
        address,
        &certificate_path,
        &private_key_path,
    );
    let claim = send_ipc(&socket, &writer_claim());
    assert!(matches!(
        claim.response,
        ProtocolResponse::AuthorityWriterClaimed(ref claim)
            if claim.generation == 1 && claim.writer_id == WRITER_ID && !claim.replayed
    ));
    let (slow_ready, slow_https) = spawn_authenticated_slow_https(address, cert.der().clone());
    slow_ready
        .recv_timeout(Duration::from_secs(5))
        .expect("authenticated slow HTTPS request did not enter its body-read window");
    let shutdown_elapsed = daemon.stop_with_budget(Duration::from_secs(1));
    assert!(shutdown_elapsed < Duration::from_secs(1));
    let (_, slow_result) = slow_https.join().unwrap();
    if let Ok(response) = slow_result {
        assert!(
            response.is_empty(),
            "cancelled slow HTTPS request received an application response"
        );
    }
    assert_eq!(runtime_owner_count(&database), 0);
    assert!(!socket.exists());

    let restart_address = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap();
    let restarted = DaemonProcess::spawn(
        &binary,
        &database,
        &socket,
        restart_address,
        &certificate_path,
        &private_key_path,
    );
    let replay = send_ipc(&socket, &writer_claim());
    assert!(matches!(
        replay.response,
        ProtocolResponse::AuthorityWriterClaimed(ref claim)
            if claim.generation == 1 && claim.writer_id == WRITER_ID && claim.replayed
    ));
    println!(
        "slow-https-sigterm shutdown_elapsed_ms={} budget_ms=1000 owner_lease_released=true unix_socket_released=true application_response_suppressed=true immediate_restart=true generation=1",
        shutdown_elapsed.as_millis()
    );
    restarted.stop();
    assert!(!socket.exists());
}

#[test]
fn repeated_remote_read_phase_shutdowns_preserve_process_resource_baselines() {
    let binary = PathBuf::from(env!("CARGO_BIN_EXE_leserpentd"));
    let root = TempRoot::new();
    let database = root.0.join("remote-read-phase-shutdown.sqlite");
    let socket = root.0.join("remote-read-phase-shutdown.sock");
    let certificate_path = root.0.join("remote-read-phase-shutdown.crt");
    let private_key_path = root.0.join("remote-read-phase-shutdown.key");
    let CertifiedKey { cert, signing_key } =
        generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    fs::write(&certificate_path, cert.pem()).unwrap();
    fs::write(&private_key_path, signing_key.serialize_pem()).unwrap();
    fs::set_permissions(&private_key_path, fs::Permissions::from_mode(0o600)).unwrap();
    let phases = [
        RemoteReadPhase::TlsHandshake,
        RemoteReadPhase::HttpHeader,
        RemoteReadPhase::AuthenticatedBody,
    ];
    assert_eq!(phases.len(), REMOTE_READ_SHUTDOWN_PHASES);
    let mut shutdown_elapsed = Vec::with_capacity(REMOTE_READ_SHUTDOWN_PHASES);
    let mut baseline_resources = Vec::with_capacity(REMOTE_READ_SHUTDOWN_PHASES + 1);
    let mut active_resources = Vec::with_capacity(REMOTE_READ_SHUTDOWN_PHASES);

    for (cycle, phase) in phases.into_iter().enumerate() {
        let address = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap();
        let daemon = DaemonProcess::spawn(
            &binary,
            &database,
            &socket,
            address,
            &certificate_path,
            &private_key_path,
        );
        let claim = send_ipc(&socket, &writer_claim());
        assert!(matches!(
            claim.response,
            ProtocolResponse::AuthorityWriterClaimed(ref claim)
                if claim.generation == 1
                    && claim.writer_id == WRITER_ID
                    && claim.replayed == (cycle > 0)
        ));
        let (_, readiness) = spawn_https_query(address, cert.der().clone())
            .join()
            .unwrap();
        assert!(matches!(readiness.response, ProtocolResponse::Query(_)));
        let readiness_fence = send_ipc(&socket, &runtime_list_query());
        assert!(matches!(
            readiness_fence.response,
            ProtocolResponse::Query(_)
        ));
        let pid = daemon.pid();
        let baseline = wait_for_idle_process_resources(pid);
        baseline_resources.push(baseline);
        let (ready, stalled) = spawn_stalled_remote_read(address, cert.der().clone(), phase);
        ready
            .recv_timeout(Duration::from_secs(5))
            .unwrap_or_else(|_| panic!("{} phase did not enter its read window", phase.label()));
        active_resources.push(wait_for_stalled_remote_resource(pid, baseline));

        eprintln!("remote-read-phase={} sending SIGTERM", phase.label());
        let elapsed = daemon.stop_with_budget(Duration::from_secs(1));
        eprintln!(
            "remote-read-phase={} stopped in {} ms",
            phase.label(),
            elapsed.as_millis()
        );
        assert!(
            elapsed < Duration::from_secs(1),
            "{} phase exceeded its SIGTERM budget: {elapsed:?}",
            phase.label()
        );
        shutdown_elapsed.push(elapsed);
        let (_, response) = stalled.join().unwrap();
        if let Ok(response) = response {
            assert!(
                response.is_empty(),
                "{} phase received an application response after cancellation",
                phase.label()
            );
        }
        assert_eq!(
            runtime_owner_count(&database),
            0,
            "{} phase retained its runtime owner row",
            phase.label()
        );
        assert!(
            !socket.exists(),
            "{} phase retained its socket",
            phase.label()
        );
        assert_process_resources_released(pid);
    }

    let restart_address = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap();
    let restarted = DaemonProcess::spawn(
        &binary,
        &database,
        &socket,
        restart_address,
        &certificate_path,
        &private_key_path,
    );
    let replay = send_ipc(&socket, &writer_claim());
    assert!(matches!(
        replay.response,
        ProtocolResponse::AuthorityWriterClaimed(ref claim)
            if claim.generation == 1 && claim.writer_id == WRITER_ID && claim.replayed
    ));
    let (_, readiness) = spawn_https_query(restart_address, cert.der().clone())
        .join()
        .unwrap();
    assert!(matches!(readiness.response, ProtocolResponse::Query(_)));
    let readiness_fence = send_ipc(&socket, &runtime_list_query());
    assert!(matches!(
        readiness_fence.response,
        ProtocolResponse::Query(_)
    ));
    let restart_pid = restarted.pid();
    let restart_baseline = wait_for_idle_process_resources(restart_pid);
    baseline_resources.push(restart_baseline);

    let observed_baselines = baseline_resources
        .iter()
        .copied()
        .flatten()
        .collect::<Vec<_>>();
    if let Some(expected) = observed_baselines.first() {
        assert_eq!(observed_baselines.len(), REMOTE_READ_SHUTDOWN_PHASES + 1);
        assert!(
            observed_baselines
                .iter()
                .all(|resources| resources == expected),
            "remote read phase process baselines drifted across restarts: {observed_baselines:?}"
        );
        let observed_active = active_resources
            .iter()
            .copied()
            .flatten()
            .collect::<Vec<_>>();
        assert_eq!(observed_active.len(), REMOTE_READ_SHUTDOWN_PHASES);
        for active in observed_active {
            assert_eq!(active.open_fds, expected.open_fds + 1);
            assert_eq!(active.tasks, expected.tasks);
        }
    }

    println!(
        "remote-read-phase-shutdown phases={} phase_names={:?} shutdown_elapsed_ms={:?} baseline_resources={baseline_resources:?} active_resources={active_resources:?} stable_fd_task_baselines=true proc_released_each_phase=true owner_rows_released_each_phase=true socket_released_each_phase=true application_response_suppressed_each_phase=true immediate_restart=true generation=1",
        REMOTE_READ_SHUTDOWN_PHASES,
        phases.map(RemoteReadPhase::label),
        shutdown_elapsed
            .iter()
            .map(Duration::as_millis)
            .collect::<Vec<_>>()
    );
    eprintln!("remote-read-final-restart sending SIGTERM");
    let restart_elapsed = restarted.stop_with_budget(Duration::from_secs(1));
    eprintln!(
        "remote-read-final-restart stopped in {} ms",
        restart_elapsed.as_millis()
    );
    assert!(restart_elapsed < Duration::from_secs(1));
    assert_eq!(runtime_owner_count(&database), 0);
    assert!(!socket.exists());
    assert_process_resources_released(restart_pid);
}

#[test]
fn mixed_remote_read_phases_with_listener_backlog_preserve_bounded_shutdown_and_authority() {
    let binary = PathBuf::from(env!("CARGO_BIN_EXE_leserpentd"));
    let root = TempRoot::new();
    let database = root.0.join("remote-backlog-shutdown.sqlite");
    let socket = root.0.join("remote-backlog-shutdown.sock");
    let certificate_path = root.0.join("remote-backlog-shutdown.crt");
    let private_key_path = root.0.join("remote-backlog-shutdown.key");
    let CertifiedKey { cert, signing_key } =
        generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    fs::write(&certificate_path, cert.pem()).unwrap();
    fs::write(&private_key_path, signing_key.serialize_pem()).unwrap();
    fs::set_permissions(&private_key_path, fs::Permissions::from_mode(0o600)).unwrap();
    let phases = [
        RemoteReadPhase::TlsHandshake,
        RemoteReadPhase::HttpHeader,
        RemoteReadPhase::AuthenticatedBody,
    ];
    let mut shutdown_elapsed = Vec::with_capacity(REMOTE_READ_SHUTDOWN_PHASES);
    let mut baseline_resources = Vec::with_capacity(REMOTE_READ_SHUTDOWN_PHASES + 1);
    let mut active_resources = Vec::with_capacity(REMOTE_READ_SHUTDOWN_PHASES);
    let mut backlog_resources = Vec::with_capacity(REMOTE_READ_SHUTDOWN_PHASES);

    for (cycle, phase) in phases.into_iter().enumerate() {
        let address = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap();
        let daemon = DaemonProcess::spawn(
            &binary,
            &database,
            &socket,
            address,
            &certificate_path,
            &private_key_path,
        );
        let claim = send_ipc(&socket, &writer_claim());
        assert!(matches!(
            claim.response,
            ProtocolResponse::AuthorityWriterClaimed(ref claim)
                if claim.generation == 1
                    && claim.writer_id == WRITER_ID
                    && claim.replayed == (cycle > 0)
        ));
        let (_, readiness) = spawn_https_query(address, cert.der().clone())
            .join()
            .unwrap();
        assert!(matches!(readiness.response, ProtocolResponse::Query(_)));
        let readiness_fence = send_ipc(&socket, &runtime_list_query());
        assert!(matches!(
            readiness_fence.response,
            ProtocolResponse::Query(_)
        ));

        let pid = daemon.pid();
        let baseline = wait_for_idle_process_resources(pid);
        baseline_resources.push(baseline);
        let (ready, stalled) = spawn_stalled_remote_read(address, cert.der().clone(), phase);
        ready
            .recv_timeout(Duration::from_secs(5))
            .unwrap_or_else(|_| panic!("{} phase did not enter its read window", phase.label()));
        let active = wait_for_stalled_remote_resource(pid, baseline);
        active_resources.push(active);

        let backlog_peers = (0..REMOTE_BACKLOG_PEERS_PER_PHASE)
            .map(|_| queue_incomplete_tls_backlog_peer(address))
            .collect::<Vec<_>>();
        assert_eq!(backlog_peers.len(), REMOTE_BACKLOG_PEERS_PER_PHASE);
        backlog_resources.push(sample_unchanged_process_resources(pid, active));
        assert_eq!(
            authority_writer_generation(&database),
            1,
            "{} phase backlog allocated an authority generation",
            phase.label()
        );

        eprintln!(
            "remote-read-backlog-phase={} peers={} sending SIGTERM",
            phase.label(),
            REMOTE_BACKLOG_PEERS_PER_PHASE
        );
        let elapsed = daemon.stop_with_budget(Duration::from_secs(1));
        eprintln!(
            "remote-read-backlog-phase={} stopped in {} ms",
            phase.label(),
            elapsed.as_millis()
        );
        assert!(
            elapsed < Duration::from_secs(1),
            "{} phase with listener backlog exceeded its SIGTERM budget: {elapsed:?}",
            phase.label()
        );
        shutdown_elapsed.push(elapsed);
        let (_, response) = stalled.join().unwrap();
        if let Ok(response) = response {
            assert!(
                response.is_empty(),
                "{} phase received an application response after backlog cancellation",
                phase.label()
            );
        }
        drop(backlog_peers);
        assert_eq!(
            runtime_owner_count(&database),
            0,
            "{} phase retained its runtime owner row",
            phase.label()
        );
        assert_eq!(authority_writer_generation(&database), 1);
        assert!(
            !socket.exists(),
            "{} phase retained its socket",
            phase.label()
        );
        assert_process_resources_released(pid);
    }

    let restart_address = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap();
    let restarted = DaemonProcess::spawn(
        &binary,
        &database,
        &socket,
        restart_address,
        &certificate_path,
        &private_key_path,
    );
    let replay = send_ipc(&socket, &writer_claim());
    assert!(matches!(
        replay.response,
        ProtocolResponse::AuthorityWriterClaimed(ref claim)
            if claim.generation == 1 && claim.writer_id == WRITER_ID && claim.replayed
    ));
    let (_, readiness) = spawn_https_query(restart_address, cert.der().clone())
        .join()
        .unwrap();
    assert!(matches!(readiness.response, ProtocolResponse::Query(_)));
    let readiness_fence = send_ipc(&socket, &runtime_list_query());
    assert!(matches!(
        readiness_fence.response,
        ProtocolResponse::Query(_)
    ));
    let restart_pid = restarted.pid();
    baseline_resources.push(wait_for_idle_process_resources(restart_pid));
    assert_eq!(authority_writer_generation(&database), 1);

    let observed_baselines = baseline_resources
        .iter()
        .copied()
        .flatten()
        .collect::<Vec<_>>();
    if let Some(expected) = observed_baselines.first() {
        assert_eq!(observed_baselines.len(), REMOTE_READ_SHUTDOWN_PHASES + 1);
        assert!(
            observed_baselines
                .iter()
                .all(|resources| resources == expected),
            "backlog test process baselines drifted across restarts: {observed_baselines:?}"
        );
        let observed_active = active_resources
            .iter()
            .copied()
            .flatten()
            .collect::<Vec<_>>();
        let observed_backlog = backlog_resources
            .iter()
            .copied()
            .flatten()
            .collect::<Vec<_>>();
        assert_eq!(observed_active.len(), REMOTE_READ_SHUTDOWN_PHASES);
        assert_eq!(observed_backlog.len(), REMOTE_READ_SHUTDOWN_PHASES);
        for resources in observed_active.iter().chain(&observed_backlog) {
            assert_eq!(resources.open_fds, expected.open_fds + 1);
            assert_eq!(resources.tasks, expected.tasks);
        }
    }

    println!(
        "remote-read-backlog-shutdown phases={} phase_names={:?} backlog_peers_per_phase={} shutdown_elapsed_ms={:?} baseline_resources={baseline_resources:?} active_resources={active_resources:?} backlog_resources={backlog_resources:?} stable_fd_task_baselines=true listener_backlog_zero_daemon_fd_amplification=true listener_backlog_zero_task_amplification=true proc_released_each_phase=true owner_rows_released_each_phase=true socket_released_each_phase=true application_response_suppressed_each_phase=true zero_authority_generation_allocation=true immediate_restart=true generation=1",
        REMOTE_READ_SHUTDOWN_PHASES,
        phases.map(RemoteReadPhase::label),
        REMOTE_BACKLOG_PEERS_PER_PHASE,
        shutdown_elapsed
            .iter()
            .map(Duration::as_millis)
            .collect::<Vec<_>>()
    );
    let restart_elapsed = restarted.stop_with_budget(Duration::from_secs(1));
    assert!(restart_elapsed < Duration::from_secs(1));
    assert_eq!(runtime_owner_count(&database), 0);
    assert_eq!(authority_writer_generation(&database), 1);
    assert!(!socket.exists());
    assert_process_resources_released(restart_pid);
}

#[test]
fn maximum_event_sessions_with_stalled_request_preserve_bounded_shutdown_and_resources() {
    let binary = PathBuf::from(env!("CARGO_BIN_EXE_leserpentd"));
    let root = TempRoot::new();
    let database = root.0.join("event-session-shutdown.sqlite");
    let socket = root.0.join("event-session-shutdown.sock");
    let certificate_path = root.0.join("event-session-shutdown.crt");
    let private_key_path = root.0.join("event-session-shutdown.key");
    let CertifiedKey { cert, signing_key } =
        generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    fs::write(&certificate_path, cert.pem()).unwrap();
    fs::write(&private_key_path, signing_key.serialize_pem()).unwrap();
    fs::set_permissions(&private_key_path, fs::Permissions::from_mode(0o600)).unwrap();
    let address = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap();
    let daemon = DaemonProcess::spawn(
        &binary,
        &database,
        &socket,
        address,
        &certificate_path,
        &private_key_path,
    );
    let claim = send_ipc(&socket, &writer_claim());
    assert!(matches!(
        claim.response,
        ProtocolResponse::AuthorityWriterClaimed(ref claim)
            if claim.generation == 1 && claim.writer_id == WRITER_ID && !claim.replayed
    ));
    let (_, readiness) = spawn_https_query(address, cert.der().clone())
        .join()
        .unwrap();
    assert!(matches!(readiness.response, ProtocolResponse::Query(_)));
    let readiness_fence = send_ipc(&socket, &runtime_list_query());
    assert!(matches!(
        readiness_fence.response,
        ProtocolResponse::Query(_)
    ));

    let pid = daemon.pid();
    let baseline = wait_for_idle_process_resources(pid);
    let mut event_sessions = (0..MAXIMUM_EVENT_SESSIONS)
        .map(|_| connect_event_session(address, cert.der().clone()))
        .collect::<Vec<_>>();
    assert_eq!(event_sessions.len(), MAXIMUM_EVENT_SESSIONS);
    let event_resources = wait_for_event_session_resources(pid, baseline);
    assert_eq!(authority_writer_generation(&database), 1);

    let (slow_ready, stalled) = spawn_authenticated_slow_https(address, cert.der().clone());
    slow_ready
        .recv_timeout(Duration::from_secs(5))
        .expect("authenticated stalled request did not enter its body-read window");
    let saturated_resources = wait_for_stalled_remote_resource(pid, event_resources);
    sample_unchanged_process_resources(pid, saturated_resources);
    assert_eq!(authority_writer_generation(&database), 1);
    let drained_pre_shutdown_events = event_sessions
        .iter_mut()
        .map(drain_event_session_before_shutdown)
        .sum::<usize>();

    let shutdown_elapsed = daemon.stop_with_budget(Duration::from_secs(1));
    assert!(shutdown_elapsed < Duration::from_secs(1));
    let (_, stalled_response) = stalled.join().unwrap();
    if let Ok(response) = stalled_response {
        assert!(
            response.is_empty(),
            "cancelled stalled request received an application response"
        );
    }
    for event_session in &mut event_sessions {
        assert_event_session_closed_without_application_event(event_session);
    }
    assert_eq!(runtime_owner_count(&database), 0);
    assert_eq!(authority_writer_generation(&database), 1);
    assert!(!socket.exists());
    assert_process_resources_released(pid);

    let restart_address = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap();
    let restarted = DaemonProcess::spawn(
        &binary,
        &database,
        &socket,
        restart_address,
        &certificate_path,
        &private_key_path,
    );
    let replay = send_ipc(&socket, &writer_claim());
    assert!(matches!(
        replay.response,
        ProtocolResponse::AuthorityWriterClaimed(ref claim)
            if claim.generation == 1 && claim.writer_id == WRITER_ID && claim.replayed
    ));
    let (_, readiness) = spawn_https_query(restart_address, cert.der().clone())
        .join()
        .unwrap();
    assert!(matches!(readiness.response, ProtocolResponse::Query(_)));
    let readiness_fence = send_ipc(&socket, &runtime_list_query());
    assert!(matches!(
        readiness_fence.response,
        ProtocolResponse::Query(_)
    ));
    let restart_pid = restarted.pid();
    let restart_baseline = wait_for_idle_process_resources(restart_pid);
    if let (Some(baseline), Some(event_resources), Some(saturated_resources), Some(restart)) = (
        baseline,
        event_resources,
        saturated_resources,
        restart_baseline,
    ) {
        assert_eq!(restart, baseline);
        assert_eq!(
            event_resources.open_fds,
            baseline.open_fds + MAXIMUM_EVENT_SESSIONS
        );
        assert_eq!(event_resources.tasks, baseline.tasks);
        assert_eq!(
            saturated_resources.open_fds,
            baseline.open_fds + MAXIMUM_EVENT_SESSIONS + 1
        );
        assert_eq!(saturated_resources.tasks, baseline.tasks);
    }
    println!(
        "maximum-event-session-shutdown sessions={} shutdown_elapsed_ms={} drained_pre_shutdown_events={} baseline_resources={baseline:?} event_resources={event_resources:?} saturated_resources={saturated_resources:?} restart_resources={restart_baseline:?} exact_session_fd_accounting=true zero_session_task_amplification=true stalled_request_fd_accounting=true pre_shutdown_event_queue_drained=true application_response_suppressed=true late_application_events_suppressed=true all_event_sessions_closed=true proc_owner_socket_released=true zero_authority_generation_allocation=true immediate_restart=true generation=1",
        MAXIMUM_EVENT_SESSIONS,
        shutdown_elapsed.as_millis(),
        drained_pre_shutdown_events
    );
    let restart_elapsed = restarted.stop_with_budget(Duration::from_secs(1));
    assert!(restart_elapsed < Duration::from_secs(1));
    assert_eq!(runtime_owner_count(&database), 0);
    assert_eq!(authority_writer_generation(&database), 1);
    assert!(!socket.exists());
    assert_process_resources_released(restart_pid);
}

#[test]
fn maximum_event_session_cycles_reclaim_slots_and_preserve_cross_transport_progress() {
    let binary = PathBuf::from(env!("CARGO_BIN_EXE_leserpentd"));
    let root = TempRoot::new();
    let database = root.0.join("event-session-cycles.sqlite");
    let socket = root.0.join("event-session-cycles.sock");
    let certificate_path = root.0.join("event-session-cycles.crt");
    let private_key_path = root.0.join("event-session-cycles.key");
    let CertifiedKey { cert, signing_key } =
        generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    fs::write(&certificate_path, cert.pem()).unwrap();
    fs::write(&private_key_path, signing_key.serialize_pem()).unwrap();
    fs::set_permissions(&private_key_path, fs::Permissions::from_mode(0o600)).unwrap();
    let address = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap();
    let daemon = DaemonProcess::spawn(
        &binary,
        &database,
        &socket,
        address,
        &certificate_path,
        &private_key_path,
    );
    let claim = send_ipc(&socket, &writer_claim());
    assert!(matches!(
        claim.response,
        ProtocolResponse::AuthorityWriterClaimed(ref claim)
            if claim.generation == 1 && claim.writer_id == WRITER_ID && !claim.replayed
    ));
    let (_, readiness) = spawn_https_query(address, cert.der().clone())
        .join()
        .unwrap();
    assert!(matches!(readiness.response, ProtocolResponse::Query(_)));
    assert!(matches!(
        send_ipc(&socket, &runtime_list_query()).response,
        ProtocolResponse::Query(_)
    ));

    let pid = daemon.pid();
    let baseline = wait_for_idle_process_resources(pid);
    let writer_fence = AuthorityWriterFence {
        generation: 1,
        writer_id: WRITER_ID.into(),
    };
    let mut capacity_resources = Vec::with_capacity(MAXIMUM_EVENT_SESSION_CYCLES);
    let mut reclaimed_resources = Vec::with_capacity(MAXIMUM_EVENT_SESSION_CYCLES);
    let mut ipc_elapsed = Vec::with_capacity(MAXIMUM_EVENT_SESSION_CYCLES);
    let mut https_elapsed = Vec::with_capacity(MAXIMUM_EVENT_SESSION_CYCLES);
    let mut fanout_elapsed = Vec::with_capacity(MAXIMUM_EVENT_SESSION_CYCLES);

    for cycle in 0..MAXIMUM_EVENT_SESSION_CYCLES {
        let current_revision = Revision(cycle as u64);
        let mut event_sessions = (0..MAXIMUM_EVENT_SESSIONS)
            .map(|session| {
                connect_event_session_at_revision(
                    address,
                    cert.der().clone(),
                    current_revision,
                    cycle,
                    &format!("cycle={cycle} session={session}"),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(event_sessions.len(), MAXIMUM_EVENT_SESSIONS);
        capacity_resources.push(wait_for_event_session_resources(pid, baseline));

        let ipc = spawn_ipc_query(socket.clone());
        let https = spawn_https_query(address, cert.der().clone());
        let (ipc_duration, ipc_response) = ipc.join().unwrap();
        let (https_duration, https_response) = https.join().unwrap();
        assert!(ipc_duration < Duration::from_secs(5));
        assert!(https_duration < Duration::from_secs(5));
        assert!(matches!(ipc_response.response, ProtocolResponse::Query(_)));
        assert!(matches!(
            https_response.response,
            ProtocolResponse::Query(_)
        ));
        ipc_elapsed.push(ipc_duration);
        https_elapsed.push(https_duration);

        let fanout_started = Instant::now();
        let registration =
            send_ipc_with_writer_fence(&socket, &runtime_registration(cycle), Some(&writer_fence));
        let next_revision = Revision((cycle + 1) as u64);
        assert!(matches!(
            registration.response,
            ProtocolResponse::Command(ref result)
                if result.status == CommandStatus::Applied
                    && result.runtime.id.as_str() == format!("event-cycle-{cycle}")
                    && result.runtime.revision == next_revision
        ));
        for event_session in &mut event_sessions {
            read_runtime_snapshot(event_session, next_revision, current_revision, cycle + 1);
        }
        let elapsed = fanout_started.elapsed();
        assert!(elapsed < Duration::from_secs(5));
        fanout_elapsed.push(elapsed);

        for event_session in &mut event_sessions {
            let _ = event_session.close(None);
        }
        drop(event_sessions);
        let mut reclamation_probe = connect_event_session_at_revision(
            address,
            cert.der().clone(),
            next_revision,
            cycle + 1,
            &format!("cycle={cycle} immediate-reclamation-probe"),
        );
        let _ = reclamation_probe.close(None);
        drop(reclamation_probe);
        let reclaimed = wait_for_idle_process_resources(pid);
        if let (Some(expected), Some(observed)) = (baseline, reclaimed) {
            assert_eq!(observed, expected);
        }
        reclaimed_resources.push(reclaimed);
        assert_eq!(authority_writer_generation(&database), 1);
    }

    let observed_capacity = capacity_resources
        .iter()
        .copied()
        .flatten()
        .collect::<Vec<_>>();
    let observed_reclaimed = reclaimed_resources
        .iter()
        .copied()
        .flatten()
        .collect::<Vec<_>>();
    if let Some(baseline) = baseline {
        assert_eq!(observed_capacity.len(), MAXIMUM_EVENT_SESSION_CYCLES);
        assert_eq!(observed_reclaimed.len(), MAXIMUM_EVENT_SESSION_CYCLES);
        assert!(observed_capacity.iter().all(|resources| {
            resources.open_fds == baseline.open_fds + MAXIMUM_EVENT_SESSIONS
                && resources.tasks == baseline.tasks
        }));
        assert!(
            observed_reclaimed
                .iter()
                .all(|resources| *resources == baseline)
        );
    }
    let shutdown_elapsed = daemon.stop_with_budget(Duration::from_secs(1));
    assert!(shutdown_elapsed < Duration::from_secs(1));
    assert_eq!(runtime_owner_count(&database), 0);
    assert_eq!(authority_writer_generation(&database), 1);
    assert!(!socket.exists());
    assert_process_resources_released(pid);

    let restart_address = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap();
    let restarted = DaemonProcess::spawn(
        &binary,
        &database,
        &socket,
        restart_address,
        &certificate_path,
        &private_key_path,
    );
    let replay = send_ipc(&socket, &writer_claim());
    assert!(matches!(
        replay.response,
        ProtocolResponse::AuthorityWriterClaimed(ref claim)
            if claim.generation == 1 && claim.writer_id == WRITER_ID && claim.replayed
    ));
    let (_, readiness) = spawn_https_query(restart_address, cert.der().clone())
        .join()
        .unwrap();
    assert!(matches!(readiness.response, ProtocolResponse::Query(_)));
    assert!(matches!(
        send_ipc(&socket, &runtime_list_query()).response,
        ProtocolResponse::Query(_)
    ));
    let restart_pid = restarted.pid();
    let restart_baseline = wait_for_idle_process_resources(restart_pid);
    if let (Some(expected), Some(observed)) = (baseline, restart_baseline) {
        assert_eq!(observed, expected);
    }
    println!(
        "maximum-event-session-cycles cycles={} sessions_per_cycle={} capacity_window_sessions={} immediate_reclamation_probes={} total_sessions={} fanout_events={} ipc_elapsed_ms={:?} https_elapsed_ms={:?} fanout_elapsed_ms={:?} shutdown_elapsed_ms={} baseline_resources={baseline:?} capacity_resources={capacity_resources:?} reclaimed_resources={reclaimed_resources:?} restart_resources={restart_baseline:?} exact_one_fd_per_session=true zero_task_amplification=true all_slots_reclaimed_each_cycle=true immediate_reconnect_after_each_cycle=true ipc_https_progress_each_cycle=true complete_fanout_each_cycle=true proc_owner_socket_released=true zero_authority_generation_allocation=true immediate_restart=true generation=1",
        MAXIMUM_EVENT_SESSION_CYCLES,
        MAXIMUM_EVENT_SESSIONS,
        MAXIMUM_EVENT_SESSION_CYCLES * MAXIMUM_EVENT_SESSIONS,
        MAXIMUM_EVENT_SESSION_CYCLES,
        MAXIMUM_EVENT_SESSION_CYCLES * (MAXIMUM_EVENT_SESSIONS + 1),
        MAXIMUM_EVENT_SESSION_CYCLES * MAXIMUM_EVENT_SESSIONS,
        ipc_elapsed
            .iter()
            .map(Duration::as_millis)
            .collect::<Vec<_>>(),
        https_elapsed
            .iter()
            .map(Duration::as_millis)
            .collect::<Vec<_>>(),
        fanout_elapsed
            .iter()
            .map(Duration::as_millis)
            .collect::<Vec<_>>(),
        shutdown_elapsed.as_millis()
    );
    let restart_elapsed = restarted.stop_with_budget(Duration::from_secs(1));
    assert!(restart_elapsed < Duration::from_secs(1));
    assert_eq!(runtime_owner_count(&database), 0);
    assert_eq!(authority_writer_generation(&database), 1);
    assert!(!socket.exists());
    assert_process_resources_released(restart_pid);
}

#[test]
fn slow_event_session_is_bounded_without_blocking_healthy_fanout_or_transports() {
    let binary = PathBuf::from(env!("CARGO_BIN_EXE_leserpentd"));
    let root = TempRoot::new();
    let database = root.0.join("slow-event-session.sqlite");
    let socket = root.0.join("slow-event-session.sock");
    let certificate_path = root.0.join("slow-event-session.crt");
    let private_key_path = root.0.join("slow-event-session.key");
    seed_slow_event_runtime_snapshot(&database);
    let CertifiedKey { cert, signing_key } =
        generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    fs::write(&certificate_path, cert.pem()).unwrap();
    fs::write(&private_key_path, signing_key.serialize_pem()).unwrap();
    fs::set_permissions(&private_key_path, fs::Permissions::from_mode(0o600)).unwrap();
    let address = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap();
    let daemon = DaemonProcess::spawn(
        &binary,
        &database,
        &socket,
        address,
        &certificate_path,
        &private_key_path,
    );
    let claim = send_ipc(&socket, &writer_claim());
    assert!(matches!(
        claim.response,
        ProtocolResponse::AuthorityWriterClaimed(ref claim)
            if claim.generation == 1 && claim.writer_id == WRITER_ID && !claim.replayed
    ));
    let (_, readiness) = spawn_https_query(address, cert.der().clone())
        .join()
        .unwrap();
    assert!(matches!(readiness.response, ProtocolResponse::Query(_)));
    assert!(matches!(
        send_ipc(&socket, &runtime_list_query()).response,
        ProtocolResponse::Query(_)
    ));

    let pid = daemon.pid();
    let baseline = wait_for_idle_process_resources(pid);
    let initial_revision = Revision(SLOW_EVENT_SEED_RUNTIMES as u64);
    let (mut slow_session, initial_snapshot_bytes) = connect_event_session_with_receive_buffer(
        address,
        cert.der().clone(),
        initial_revision,
        SLOW_EVENT_SEED_RUNTIMES,
        "slow-event-session",
        Some(1_024),
    );
    assert!(
        initial_snapshot_bytes >= 32 * 1_024,
        "seeded event snapshot was too small to exercise backpressure: {initial_snapshot_bytes} bytes"
    );
    let mut healthy_sessions = (1..MAXIMUM_EVENT_SESSIONS)
        .map(|session| {
            connect_event_session_at_revision(
                address,
                cert.der().clone(),
                initial_revision,
                SLOW_EVENT_SEED_RUNTIMES,
                &format!("healthy-slow-pressure-session={session}"),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(healthy_sessions.len(), MAXIMUM_EVENT_SESSIONS - 1);
    let capacity_resources = wait_for_event_session_resources(pid, baseline);
    let writer_fence = AuthorityWriterFence {
        generation: 1,
        writer_id: WRITER_ID.into(),
    };
    let mut runtime_revision = Revision(1);
    let mut event_revision = initial_revision;
    let mut ipc_elapsed = Vec::new();
    let mut https_elapsed = Vec::new();

    for update in 0..SLOW_EVENT_REVISIONS {
        let response = send_ipc_with_writer_fence(
            &socket,
            &runtime_registration_update(update, runtime_revision),
            Some(&writer_fence),
        );
        let next_revision = Revision(SLOW_EVENT_SEED_RUNTIMES as u64 + update as u64 + 1);
        assert!(matches!(
            response.response,
            ProtocolResponse::Command(ref result)
                if result.status == CommandStatus::Applied
                    && result.runtime.id.as_str() == "slow-event-runtime-000"
                    && result.runtime.revision == next_revision
        ));
        for healthy_session in &mut healthy_sessions {
            read_runtime_snapshot_containing(
                healthy_session,
                next_revision,
                event_revision,
                SLOW_EVENT_SEED_RUNTIMES,
                "slow-event-runtime-000",
            );
        }
        runtime_revision = next_revision;
        event_revision = next_revision;

        if update % 8 == 7 {
            let ipc = spawn_ipc_query(socket.clone());
            let https = spawn_https_query(address, cert.der().clone());
            let (ipc_duration, ipc_response) = ipc.join().unwrap();
            let (https_duration, https_response) = https.join().unwrap();
            assert!(ipc_duration < Duration::from_secs(5));
            assert!(https_duration < Duration::from_secs(5));
            assert!(matches!(ipc_response.response, ProtocolResponse::Query(_)));
            assert!(matches!(
                https_response.response,
                ProtocolResponse::Query(_)
            ));
            ipc_elapsed.push(ipc_duration);
            https_elapsed.push(https_duration);
        }
    }

    let healthy_resources =
        wait_for_event_session_count_resources(pid, baseline, MAXIMUM_EVENT_SESSIONS - 1);
    let buffered_slow_snapshots =
        assert_slow_event_session_is_bounded_and_closed(&mut slow_session);
    assert!(buffered_slow_snapshots < SLOW_EVENT_REVISIONS);
    assert_eq!(authority_writer_generation(&database), 1);
    for healthy_session in &mut healthy_sessions {
        let _ = healthy_session.close(None);
    }
    drop(healthy_sessions);
    drop(slow_session);
    let reclaimed = wait_for_idle_process_resources(pid);
    if let (Some(expected), Some(observed)) = (baseline, reclaimed) {
        assert_eq!(observed, expected);
    }

    let shutdown_elapsed = daemon.stop_with_budget(Duration::from_secs(1));
    assert!(shutdown_elapsed < Duration::from_secs(1));
    assert_eq!(runtime_owner_count(&database), 0);
    assert_eq!(authority_writer_generation(&database), 1);
    assert!(!socket.exists());
    assert_process_resources_released(pid);

    let restart_address = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap();
    let restarted = DaemonProcess::spawn(
        &binary,
        &database,
        &socket,
        restart_address,
        &certificate_path,
        &private_key_path,
    );
    let replay = send_ipc(&socket, &writer_claim());
    assert!(matches!(
        replay.response,
        ProtocolResponse::AuthorityWriterClaimed(ref claim)
            if claim.generation == 1 && claim.writer_id == WRITER_ID && claim.replayed
    ));
    let (_, readiness) = spawn_https_query(restart_address, cert.der().clone())
        .join()
        .unwrap();
    assert!(matches!(readiness.response, ProtocolResponse::Query(_)));
    let restart_pid = restarted.pid();
    let restart_resources = wait_for_idle_process_resources(restart_pid);
    if let (Some(expected), Some(observed)) = (baseline, restart_resources) {
        assert_eq!(observed, expected);
    }
    println!(
        "slow-event-session-isolation seeded_runtimes={} revisions={} healthy_sessions={} healthy_fanout_events={} initial_snapshot_bytes={} buffered_slow_snapshots={} ipc_elapsed_ms={:?} https_elapsed_ms={:?} shutdown_elapsed_ms={} baseline_resources={baseline:?} capacity_resources={capacity_resources:?} healthy_resources={healthy_resources:?} reclaimed_resources={reclaimed:?} restart_resources={restart_resources:?} bounded_slow_session=true healthy_fanout_complete=true ipc_https_progress=true exact_slow_session_fd_reclamation=true zero_task_amplification=true proc_owner_socket_released=true immediate_restart=true generation=1",
        SLOW_EVENT_SEED_RUNTIMES,
        SLOW_EVENT_REVISIONS,
        MAXIMUM_EVENT_SESSIONS - 1,
        (MAXIMUM_EVENT_SESSIONS - 1) * SLOW_EVENT_REVISIONS,
        initial_snapshot_bytes,
        buffered_slow_snapshots,
        ipc_elapsed
            .iter()
            .map(Duration::as_millis)
            .collect::<Vec<_>>(),
        https_elapsed
            .iter()
            .map(Duration::as_millis)
            .collect::<Vec<_>>(),
        shutdown_elapsed.as_millis()
    );
    let restart_elapsed = restarted.stop_with_budget(Duration::from_secs(1));
    assert!(restart_elapsed < Duration::from_secs(1));
    assert_eq!(runtime_owner_count(&database), 0);
    assert_eq!(authority_writer_generation(&database), 1);
    assert!(!socket.exists());
    assert_process_resources_released(restart_pid);
}
