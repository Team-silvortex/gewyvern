#![cfg(unix)]

use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::Shutdown;
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt, PermissionsExt};
use std::os::unix::net::UnixStream;
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command as ProcessCommand, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Barrier, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use leserpent_domain::{
    CAPABILITY_RUNTIME_REFRESH, CAPABILITY_RUNTIME_REGISTER, CapabilitySet, Command,
    CommandEnvelope, CommandId, CommandOrigin, CommandStatus, Confirmation, DOMAIN_SCHEMA_VERSION,
    IdempotencyKey, Principal, Revision, RuntimeId, RuntimeTags,
};
use leserpent_protocol::{
    AuthorityWriterClaimRequest, AuthorityWriterFence, CAPABILITY_AUTHORITY_WRITER,
    PROTOCOL_SCHEMA_VERSION, ProtocolRequest, ProtocolResponse, RequestEnvelope, ResponseEnvelope,
    decode_response,
};
use leserpent_runtime::ControlRuntime;
use leserpentd::{MAX_IPC_CONNECTIONS_PER_TICK, MAX_IPC_SOCKET_PATH_BYTES};
use rusqlite::Connection;

const TOKEN: &str = "0123456789abcdef0123456789abcdef";
const WRONG_TOKEN: &str = "fedcba9876543210fedcba9876543210";
const WRITER_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const WRITER_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const WRITER_C: &str = "cccccccccccccccccccccccccccccccc";
const SATURATED_DUPLICATE_GROUPS: usize = 16;
const READABLE_RETRIES_PER_GROUP: usize = 3;
const MIXED_PEER_GROUPS: usize = 16;
const REPEATED_HOSTILE_BATCHES: usize = 2;
const RESOURCE_LIFECYCLE_CYCLES: usize = 3;
const RECONNECT_FAIRNESS_WAVES: usize = 3;
const RECONNECTS_PER_FAIRNESS_WAVE: usize = 4;
const SLOW_PEERS_PER_RECONNECT_GROUP: usize = 15;
const RECONNECT_FAIRNESS_BUDGET: Duration = Duration::from_secs(5);
const CRASH_WORKER_DATABASE: &str = "LESERPENT_TEST_AUTHORITY_WRITER_DATABASE";
const CRASH_WORKER_ID: &str = "LESERPENT_TEST_AUTHORITY_WRITER_ID";
const LONGEST_TEST_SOCKET_FILE_NAME: &str = "post-recovery-duplicate-retry.sock";
const MAX_PARALLEL_AUTHORITY_SCENARIOS: usize = 4;
const ROLLBACK_JOURNAL_OBSERVATION_BUDGET: Duration = Duration::from_secs(4);
const CRASH_WORKER_PAUSE_AT_JOURNAL: u8 = b'P';
const CRASH_WORKER_COMMIT: u8 = b'C';
const CRASH_WORKER_JOURNAL_MARKER: &str = "authority-writer-worker-journal-ready";
static TEMP_ROOT_SEQUENCE: AtomicU64 = AtomicU64::new(1);
static AUTHORITY_SCENARIO_LIMIT: (Mutex<usize>, Condvar) = (Mutex::new(0), Condvar::new());

struct AuthorityScenarioPermit {
    slots: usize,
}

impl AuthorityScenarioPermit {
    fn shared() -> Self {
        Self::acquire(1)
    }

    fn exclusive() -> Self {
        Self::acquire(MAX_PARALLEL_AUTHORITY_SCENARIOS)
    }

    fn acquire(slots: usize) -> Self {
        let (active, available) = &AUTHORITY_SCENARIO_LIMIT;
        let mut active = active.lock().unwrap_or_else(|error| error.into_inner());
        while active.saturating_add(slots) > MAX_PARALLEL_AUTHORITY_SCENARIOS {
            active = available
                .wait(active)
                .unwrap_or_else(|error| error.into_inner());
        }
        *active += slots;
        Self { slots }
    }
}

impl Drop for AuthorityScenarioPermit {
    fn drop(&mut self) {
        let (active, available) = &AUTHORITY_SCENARIO_LIMIT;
        let mut active = active.lock().unwrap_or_else(|error| error.into_inner());
        *active = active.saturating_sub(self.slots);
        available.notify_all();
    }
}

struct TempRoot(PathBuf, AuthorityScenarioPermit);

impl TempRoot {
    fn new() -> Self {
        Self::with_permit(AuthorityScenarioPermit::shared())
    }

    fn exclusive() -> Self {
        Self::with_permit(AuthorityScenarioPermit::exclusive())
    }

    fn with_permit(permit: AuthorityScenarioPermit) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let sequence = TEMP_ROOT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let name = format!(
            "leserpent-aw-{}-{unique:x}-{sequence:x}",
            std::process::id()
        );
        let platform_temp =
            fs::canonicalize(std::env::temp_dir()).unwrap_or_else(|_| std::env::temp_dir());
        let mut root = platform_temp.join(&name);
        if root.join(LONGEST_TEST_SOCKET_FILE_NAME).as_os_str().len() > MAX_IPC_SOCKET_PATH_BYTES {
            let short_temp = fs::canonicalize("/tmp").unwrap_or_else(|_| PathBuf::from("/tmp"));
            root = short_temp.join(name);
        }
        assert!(
            root.join(LONGEST_TEST_SOCKET_FILE_NAME).as_os_str().len() <= MAX_IPC_SOCKET_PATH_BYTES
        );
        let mut builder = fs::DirBuilder::new();
        builder.mode(0o700).create(&root).unwrap();
        let root = fs::canonicalize(root).unwrap();
        assert_eq!(
            fs::metadata(&root).unwrap().permissions().mode() & 0o777,
            0o700
        );
        Self(root, permit)
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _permit = &self.1;
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn authority_writer_temp_roots_are_private_and_socket_safe() {
    let root = TempRoot::new();

    assert_eq!(
        fs::metadata(&root.0).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert!(
        root.0.join(LONGEST_TEST_SOCKET_FILE_NAME).as_os_str().len() <= MAX_IPC_SOCKET_PATH_BYTES
    );
}

struct DaemonProcess {
    child: Option<Child>,
}

impl DaemonProcess {
    fn spawn(binary: &Path, database: &Path, socket: &Path) -> Self {
        let mut child = ProcessCommand::new(binary)
            .args([
                "--database",
                database.to_str().unwrap(),
                "--socket",
                socket.to_str().unwrap(),
            ])
            .env("LESERPENT_IPC_TOKEN", TOKEN)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if socket.exists() && UnixStream::connect(socket).is_ok() {
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
                panic!("leserpentd exited before IPC readiness ({status}): {stderr}");
            }
            assert!(
                Instant::now() < deadline,
                "leserpentd did not create its IPC socket"
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
        // The production signal loop must release the SQLite owner lease and
        // socket before a fresh authority process is admitted.
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
                let _ = child.kill();
                let _ = child.wait();
                panic!("leserpentd did not stop after SIGTERM within {budget:?}");
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn sigkill(mut self) {
        let mut child = self.child.take().unwrap();
        assert_eq!(unsafe { libc::kill(child.id() as i32, libc::SIGKILL) }, 0);
        let status = child.wait().unwrap();
        assert_eq!(status.signal(), Some(libc::SIGKILL));
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
        Some(ProcessResources {
            open_fds: fs::read_dir(process.join("fd")).unwrap().count(),
            tasks: fs::read_dir(process.join("task")).unwrap().count(),
        })
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = pid;
        None
    }
}

fn wait_for_process_resources_at_most(
    pid: u32,
    baseline: Option<ProcessResources>,
) -> Option<ProcessResources> {
    let baseline = baseline?;
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let current = inspect_process_resources(pid).unwrap();
        if current.open_fds <= baseline.open_fds + 2 && current.tasks <= baseline.tasks {
            return Some(current);
        }
        assert!(
            Instant::now() < deadline,
            "daemon resources did not return to baseline {baseline:?}: {current:?}"
        );
        thread::sleep(Duration::from_millis(10));
    }
}

fn wait_for_saturated_reader_resources(
    pid: u32,
    baseline: Option<ProcessResources>,
) -> Option<ProcessResources> {
    let baseline = baseline?;
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let current = inspect_process_resources(pid).unwrap();
        if current.open_fds >= baseline.open_fds + MAX_IPC_CONNECTIONS_PER_TICK
            && current.tasks >= baseline.tasks + MAX_IPC_CONNECTIONS_PER_TICK
        {
            return Some(current);
        }
        assert!(
            Instant::now() < deadline,
            "daemon never exposed a saturated reader wave above {baseline:?}: {current:?}"
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

fn send(
    socket: &Path,
    request: &RequestEnvelope,
    writer_fence: Option<&AuthorityWriterFence>,
) -> ResponseEnvelope {
    let mut stream = UnixStream::connect(socket).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let mut frame = serde_json::to_vec(&serde_json::json!({
        "token": TOKEN,
        "writer_fence": writer_fence,
        "request": request,
    }))
    .unwrap();
    frame.push(b'\n');
    stream.write_all(&frame).unwrap();
    stream.shutdown(Shutdown::Write).unwrap();
    let mut response = Vec::new();
    stream.read_to_end(&mut response).unwrap();
    decode_response(&response).unwrap()
}

fn send_without_reading_response(socket: &Path, request: &RequestEnvelope) -> UnixStream {
    send_with_token_without_reading_response(socket, request, TOKEN)
}

fn send_with_token_without_reading_response(
    socket: &Path,
    request: &RequestEnvelope,
    token: &str,
) -> UnixStream {
    let mut stream = UnixStream::connect(socket).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let mut frame = serde_json::to_vec(&serde_json::json!({
        "token": token,
        "request": request,
    }))
    .unwrap();
    frame.push(b'\n');
    stream.write_all(&frame).unwrap();
    stream.flush().unwrap();
    stream.shutdown(Shutdown::Write).unwrap();
    stream
}

fn queue_raw_prefix(socket: &Path, prefix: &[u8]) -> UnixStream {
    let mut stream = UnixStream::connect(socket).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    stream.write_all(prefix).unwrap();
    stream.flush().unwrap();
    stream
}

fn queue_raw_frame(socket: &Path, frame: &[u8]) -> UnixStream {
    let stream = queue_raw_prefix(socket, frame);
    stream.shutdown(Shutdown::Write).unwrap();
    stream
}

fn read_queued_response(mut stream: UnixStream) -> ResponseEnvelope {
    let mut response = Vec::new();
    stream.read_to_end(&mut response).unwrap();
    decode_response(&response).unwrap()
}

fn assert_protocol_error(response: ResponseEnvelope, expected_code: &str) {
    assert!(matches!(
        response.response,
        ProtocolResponse::Error(ref error) if error.code == expected_code
    ));
}

fn hold_ipc_accept(socket: &Path) -> UnixStream {
    let mut stream = UnixStream::connect(socket).unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    stream.write_all(b"{").unwrap();
    stream.flush().unwrap();
    thread::sleep(Duration::from_millis(100));
    stream
}

fn claimed(response: ResponseEnvelope) -> (u64, String, bool) {
    match response.response {
        ProtocolResponse::AuthorityWriterClaimed(claim) => {
            (claim.generation, claim.writer_id, claim.replayed)
        }
        other => panic!("expected authority writer claim response, got {other:?}"),
    }
}

fn claim(writer_id: &str) -> RequestEnvelope {
    RequestEnvelope {
        schema_version: PROTOCOL_SCHEMA_VERSION,
        request: ProtocolRequest::AuthorityWriterClaim(AuthorityWriterClaimRequest {
            principal: Principal {
                id: "cold-takeover-test".into(),
            },
            capabilities: CapabilitySet::new([CAPABILITY_AUTHORITY_WRITER]),
            writer_id: writer_id.into(),
        }),
    }
}

fn registration() -> RequestEnvelope {
    command(
        "cold-register-command",
        "cold-register-request",
        CapabilitySet::new([CAPABILITY_RUNTIME_REGISTER]),
        None,
        Confirmation::Confirmed,
        Command::RuntimeRegister {
            runtime_id: RuntimeId::new("runtime-cold-takeover").unwrap(),
            name: "Runtime Cold Takeover".into(),
            endpoint: "https://127.0.0.1:9443".into(),
            sidecar_endpoint: None,
            tags: RuntimeTags::default(),
        },
    )
}

fn refresh() -> RequestEnvelope {
    command(
        "cold-refresh-command",
        "cold-refresh-request",
        CapabilitySet::new([CAPABILITY_RUNTIME_REFRESH]),
        Some(Revision(1)),
        Confirmation::NotRequired,
        Command::RuntimeRefresh {
            runtime_id: RuntimeId::new("runtime-cold-takeover").unwrap(),
        },
    )
}

fn command(
    command_id: &str,
    idempotency_key: &str,
    capabilities: CapabilitySet,
    expected_revision: Option<Revision>,
    confirmation: Confirmation,
    command: Command,
) -> RequestEnvelope {
    RequestEnvelope {
        schema_version: PROTOCOL_SCHEMA_VERSION,
        request: ProtocolRequest::Command(CommandEnvelope {
            schema_version: DOMAIN_SCHEMA_VERSION,
            command_id: CommandId::new(command_id).unwrap(),
            idempotency_key: IdempotencyKey::new(idempotency_key).unwrap(),
            expected_revision,
            principal: Principal {
                id: "cold-takeover-test".into(),
            },
            capabilities,
            origin: CommandOrigin::CompatibilityAdapter,
            confirmation,
            dry_run: false,
            command,
        }),
    }
}

fn fence(writer_id: &str, generation: u64) -> AuthorityWriterFence {
    AuthorityWriterFence {
        generation,
        writer_id: writer_id.into(),
    }
}

struct ClaimCrashWorker {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    stderr: ChildStderr,
}

impl ClaimCrashWorker {
    fn spawn(database: &Path, writer_id: &str) -> Self {
        let mut child = ProcessCommand::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "authority_writer_claim_crash_worker",
                "--nocapture",
            ])
            .env(CRASH_WORKER_DATABASE, database)
            .env(CRASH_WORKER_ID, writer_id)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        let stderr = child.stderr.take().unwrap();
        Self {
            child,
            stdin,
            stdout,
            stderr,
        }
    }

    fn send(&mut self, command: u8) {
        self.stdin.write_all(&[command]).unwrap();
        self.stdin.flush().unwrap();
    }

    fn wait_for_marker(&mut self, marker: &str) {
        let mut transcript = String::new();
        loop {
            let mut line = String::new();
            let read = self.stdout.read_line(&mut line).unwrap();
            if read == 0 {
                let mut stderr = String::new();
                self.stderr.read_to_string(&mut stderr).unwrap();
                panic!(
                    "claim worker exited before {marker}: stdout={transcript:?} stderr={stderr:?}"
                );
            }
            transcript.push_str(&line);
            if line.contains(marker) {
                return;
            }
        }
    }

    fn is_running(&mut self) -> bool {
        self.child.try_wait().unwrap().is_none()
    }

    fn sigkill(&mut self) {
        assert!(
            self.is_running(),
            "claim worker exited before the requested SIGKILL"
        );
        assert_eq!(
            unsafe { libc::kill(self.child.id() as i32, libc::SIGKILL) },
            0
        );
        let status = self.child.wait().unwrap();
        assert_eq!(status.signal(), Some(libc::SIGKILL));
        let mut stderr = String::new();
        self.stderr.read_to_string(&mut stderr).unwrap();
        assert!(
            stderr.is_empty(),
            "claim worker emitted stderr before SIGKILL: {stderr}"
        );
    }
}

impl Drop for ClaimCrashWorker {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn inspect_writer_fence(database: &Path) -> (i64, String, i64) {
    let connection = Connection::open(database).unwrap();
    let integrity: String = connection
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .unwrap();
    assert_eq!(integrity, "ok");
    let (generation, writer_id) = connection
        .query_row(
            "SELECT generation, writer_id FROM authority_writer_fence WHERE id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    let lease_expires_at = connection
        .query_row(
            "SELECT lease_expires_at_unix_ms FROM runtime_owner WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    (generation, writer_id, lease_expires_at)
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

fn wait_for_owner_lease_extension(database: &Path, owner_token: &str, previous_expiry: i64) -> i64 {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let (current_owner, current_expiry) = inspect_owner_lease(database);
        assert_eq!(
            current_owner, owner_token,
            "runtime owner changed unexpectedly"
        );
        if current_expiry > previous_expiry
            && current_expiry.saturating_sub(unix_time_ms()) >= 29_000
        {
            return current_expiry;
        }
        assert!(
            Instant::now() < deadline,
            "runtime owner lease did not advance beyond {previous_expiry}"
        );
        thread::sleep(Duration::from_millis(10));
    }
}

fn run_saturated_hostile_replay_batch(socket: &Path, writer_id: &str, generation: u64) -> Duration {
    assert_eq!(MIXED_PEER_GROUPS * 4, MAX_IPC_CONNECTIONS_PER_TICK);
    let accept_gate = hold_ipc_accept(socket);
    let mut peers = Vec::with_capacity(MIXED_PEER_GROUPS);
    let mut slow_peers = Vec::with_capacity(MIXED_PEER_GROUPS);
    for group in 0..MIXED_PEER_GROUPS {
        let unauthorized_writer = format!("{:032x}", 0x400_usize + group);
        let malformed = queue_raw_frame(socket, b"{not-json}\n");
        let unauthorized = send_with_token_without_reading_response(
            socket,
            &claim(&unauthorized_writer),
            WRONG_TOKEN,
        );
        slow_peers.push(queue_raw_prefix(socket, b"{"));
        let replay = send_without_reading_response(socket, &claim(writer_id));
        peers.push((malformed, unauthorized, replay));
    }

    let started = Instant::now();
    drop(accept_gate);
    for (malformed, unauthorized, replay) in peers {
        assert_protocol_error(read_queued_response(malformed), "invalid_json");
        assert_protocol_error(read_queued_response(unauthorized), "unauthorized");
        assert_eq!(
            claimed(read_queued_response(replay)),
            (generation, writer_id.to_string(), true)
        );
    }
    for mut slow in slow_peers {
        let mut response = Vec::new();
        slow.read_to_end(&mut response).unwrap();
        assert!(response.is_empty());
    }
    let elapsed = started.elapsed();
    assert!(
        elapsed <= Duration::from_secs(5),
        "repeated hostile replay batch exceeded its budget: {elapsed:?}"
    );
    elapsed
}

fn run_saturated_reconnect_fairness_wave(
    socket: &Path,
    writer_id: &str,
    generation: u64,
) -> (Duration, Vec<Duration>) {
    assert_eq!(
        RECONNECTS_PER_FAIRNESS_WAVE * (SLOW_PEERS_PER_RECONNECT_GROUP + 1),
        MAX_IPC_CONNECTIONS_PER_TICK
    );
    let accept_gate = hold_ipc_accept(socket);
    let mut slow_peers =
        Vec::with_capacity(RECONNECTS_PER_FAIRNESS_WAVE * SLOW_PEERS_PER_RECONNECT_GROUP);
    let mut reconnects = Vec::with_capacity(RECONNECTS_PER_FAIRNESS_WAVE);
    for _ in 0..RECONNECTS_PER_FAIRNESS_WAVE {
        slow_peers
            .extend((0..SLOW_PEERS_PER_RECONNECT_GROUP).map(|_| queue_raw_prefix(socket, b"{")));
        let reconnect = send_without_reading_response(socket, &claim(writer_id));
        reconnect
            .set_read_timeout(Some(Duration::from_secs(6)))
            .unwrap();
        reconnects.push(reconnect);
    }

    let started = Instant::now();
    drop(accept_gate);
    let reconnect_elapsed = reconnects
        .into_iter()
        .map(|reconnect| {
            assert_eq!(
                claimed(read_queued_response(reconnect)),
                (generation, writer_id.to_string(), true)
            );
            let elapsed = started.elapsed();
            assert!(
                elapsed <= RECONNECT_FAIRNESS_BUDGET,
                "valid reconnect starved behind a saturated hostile wave: {elapsed:?}"
            );
            elapsed
        })
        .collect::<Vec<_>>();
    for mut slow in slow_peers {
        let mut response = Vec::new();
        slow.read_to_end(&mut response).unwrap();
        assert!(response.is_empty());
    }
    let elapsed = started.elapsed();
    assert!(
        elapsed <= RECONNECT_FAIRNESS_BUDGET,
        "saturated reconnect fairness wave exceeded its budget: {elapsed:?}"
    );
    (elapsed, reconnect_elapsed)
}

fn wait_for_writer_fence(database: &Path, expected_generation: i64, expected_writer: &str) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let current = Connection::open(database).and_then(|connection| {
            connection.query_row(
                "SELECT generation, writer_id FROM authority_writer_fence WHERE id = 1",
                [],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
            )
        });
        if matches!(
            current,
            Ok((generation, ref writer_id))
                if generation == expected_generation && writer_id == expected_writer
        ) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "authority writer fence did not reach generation {expected_generation} for {expected_writer}"
        );
        thread::sleep(Duration::from_millis(10));
    }
}

fn unix_time_ms() -> i64 {
    i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis(),
    )
    .unwrap()
}

fn sigkill_after_lost_claim(
    daemon: DaemonProcess,
    socket: &Path,
    database: &Path,
    writer_id: &str,
    generation: i64,
) -> fs::Metadata {
    let unread_response = send_without_reading_response(socket, &claim(writer_id));
    wait_for_writer_fence(database, generation, writer_id);
    daemon.sigkill();
    drop(unread_response);
    let stale_socket = fs::symlink_metadata(socket).unwrap();
    assert!(stale_socket.file_type().is_socket());
    assert_eq!(stale_socket.permissions().mode() & 0o777, 0o600);
    assert!(matches!(
        UnixStream::connect(socket),
        Err(ref error) if error.kind() == std::io::ErrorKind::ConnectionRefused
    ));
    stale_socket
}

fn reject_before_owner_expiry(
    binary: &Path,
    database: &Path,
    socket: &Path,
    stale_socket: &fs::Metadata,
) {
    let pre_expiry = ProcessCommand::new(binary)
        .args([
            "--database",
            database.to_str().unwrap(),
            "--socket",
            socket.to_str().unwrap(),
            "--once",
        ])
        .env("LESERPENT_IPC_TOKEN", TOKEN)
        .output()
        .unwrap();
    assert!(!pre_expiry.status.success());
    assert!(
        String::from_utf8(pre_expiry.stderr)
            .unwrap()
            .contains("owned by another live process")
    );
    let current = fs::symlink_metadata(socket).unwrap();
    assert_eq!(current.dev(), stale_socket.dev());
    assert_eq!(current.ino(), stale_socket.ino());
    assert_eq!(current.mode(), stale_socket.mode());
}

fn wait_for_owner_lease_expiry(database: &Path, generation: i64, writer_id: &str) {
    let (current_generation, current_writer_id, lease_expires_at) = inspect_writer_fence(database);
    assert_eq!(
        (current_generation, current_writer_id.as_str()),
        (generation, writer_id)
    );
    let remaining_lease_ms = lease_expires_at.saturating_sub(unix_time_ms());
    assert!(remaining_lease_ms <= 30_000);
    if remaining_lease_ms > 0 {
        thread::sleep(Duration::from_millis((remaining_lease_ms + 100) as u64));
    }
}

fn recover_with_queued_claims(
    binary: &Path,
    database: &Path,
    socket: &Path,
    replay_writer: &str,
    replay_generation: u64,
    competitor_writer: &str,
    competitor_generation: u64,
) -> DaemonProcess {
    let replacement = DaemonProcess::spawn(binary, database, socket);
    assert!(UnixStream::connect(socket).is_ok());
    let accept_gate = hold_ipc_accept(socket);
    let replay_stream = send_without_reading_response(socket, &claim(replay_writer));
    thread::sleep(Duration::from_millis(50));
    let competitor_stream = send_without_reading_response(socket, &claim(competitor_writer));
    drop(accept_gate);
    assert_eq!(
        claimed(read_queued_response(replay_stream)),
        (replay_generation, replay_writer.to_string(), true)
    );
    assert_eq!(
        claimed(read_queued_response(competitor_stream)),
        (competitor_generation, competitor_writer.to_string(), false)
    );
    wait_for_writer_fence(database, competitor_generation as i64, competitor_writer);
    replacement
}

#[test]
fn authority_writer_claim_crash_worker() {
    let Some(database) = std::env::var_os(CRASH_WORKER_DATABASE) else {
        return;
    };
    let database = PathBuf::from(database);
    let writer_id = std::env::var(CRASH_WORKER_ID).unwrap();
    let mut runtime = ControlRuntime::open(&database).unwrap();
    println!("authority-writer-worker-ready");
    std::io::stdout().flush().unwrap();
    let mut command = [0_u8; 1];
    std::io::stdin().read_exact(&mut command).unwrap();
    if command == [CRASH_WORKER_PAUSE_AT_JOURNAL] {
        let rollback_journal = PathBuf::from(format!("{}-journal", database.display()));
        let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(0);
        thread::spawn(move || {
            ready_tx.send(()).unwrap();
            let deadline = Instant::now() + ROLLBACK_JOURNAL_OBSERVATION_BUDGET;
            loop {
                if fs::metadata(&rollback_journal)
                    .map(|metadata| metadata.len() > 0)
                    .unwrap_or(false)
                {
                    println!("{CRASH_WORKER_JOURNAL_MARKER}");
                    std::io::stdout().flush().unwrap();
                    assert_eq!(
                        unsafe { libc::kill(std::process::id() as i32, libc::SIGSTOP) },
                        0
                    );
                    return;
                }
                if Instant::now() >= deadline {
                    eprintln!(
                        "claim worker did not observe a rollback journal within {ROLLBACK_JOURNAL_OBSERVATION_BUDGET:?}"
                    );
                    std::process::exit(3);
                }
                thread::sleep(Duration::from_millis(1));
            }
        });
        ready_rx.recv().unwrap();
    } else {
        assert_eq!(command, [CRASH_WORKER_COMMIT]);
    }
    let claim = runtime.claim_authority_writer(&writer_id).unwrap();
    println!(
        "authority-writer-worker-committed generation={} writer={}",
        claim.generation, claim.writer_id
    );
    std::io::stdout().flush().unwrap();
    std::io::stdin().read_exact(&mut command).unwrap();
}

#[test]
fn sigkill_at_writer_claim_commit_preserves_an_atomic_generation() {
    let root = TempRoot::exclusive();
    let database = root.0.join("claim-crash.sqlite");
    {
        let mut baseline = ControlRuntime::open(&database).unwrap();
        let claim = baseline.claim_authority_writer(WRITER_A).unwrap();
        assert_eq!(claim.generation, 1);
    }

    let mut interrupted = ClaimCrashWorker::spawn(&database, WRITER_B);
    interrupted.wait_for_marker("authority-writer-worker-ready");
    let blocker = Connection::open(&database).unwrap();
    blocker.execute_batch("BEGIN DEFERRED").unwrap();
    let mut blocker_statement = blocker
        .prepare("SELECT generation FROM authority_writer_fence WHERE id = 1")
        .unwrap();
    let mut blocker_rows = blocker_statement.query([]).unwrap();
    let baseline_generation: i64 = blocker_rows.next().unwrap().unwrap().get(0).unwrap();
    assert_eq!(baseline_generation, 1);
    let rollback_journal = PathBuf::from(format!("{}-journal", database.display()));
    assert!(
        !fs::metadata(&rollback_journal)
            .map(|metadata| metadata.len() > 0)
            .unwrap_or(false)
    );
    interrupted.send(CRASH_WORKER_PAUSE_AT_JOURNAL);
    interrupted.wait_for_marker(CRASH_WORKER_JOURNAL_MARKER);
    assert!(fs::metadata(&rollback_journal).unwrap().len() > 0);
    interrupted.sigkill();
    drop(blocker_rows);
    drop(blocker_statement);
    blocker.execute_batch("ROLLBACK").unwrap();
    drop(blocker);

    let (generation, writer_id, lease_expires_at) = inspect_writer_fence(&database);
    assert_eq!((generation, writer_id.as_str()), (1, WRITER_A));
    assert!(matches!(
        ControlRuntime::open(&database),
        Err(leserpent_runtime::RuntimeError::Storage(ref error))
            if error.contains("owned by another live process")
    ));

    let remaining_lease_ms = lease_expires_at.saturating_sub(unix_time_ms());
    assert!(remaining_lease_ms <= 30_000);
    if remaining_lease_ms > 0 {
        thread::sleep(Duration::from_millis((remaining_lease_ms + 100) as u64));
    }
    {
        let mut replacement = ControlRuntime::open(&database).unwrap();
        let takeover = replacement.claim_authority_writer(WRITER_B).unwrap();
        assert_eq!(takeover.generation, 2);
        assert!(!takeover.replayed);
        replacement
            .require_authority_writer(Some(2), Some(WRITER_B))
            .unwrap();
        assert!(
            replacement
                .require_authority_writer(Some(1), Some(WRITER_A))
                .is_err()
        );
    }

    let mut committed = ClaimCrashWorker::spawn(&database, WRITER_C);
    committed.wait_for_marker("authority-writer-worker-ready");
    committed.send(CRASH_WORKER_COMMIT);
    committed.wait_for_marker("authority-writer-worker-committed generation=3");
    committed.sigkill();
    let (generation, writer_id, _) = inspect_writer_fence(&database);
    assert_eq!((generation, writer_id.as_str()), (3, WRITER_C));
}

#[test]
fn lost_claim_response_race_is_linearizable_for_same_and_competing_writers() {
    let binary = PathBuf::from(env!("CARGO_BIN_EXE_leserpentd"));
    let root = TempRoot::new();
    let database = root.0.join("lost-claim-response.sqlite");
    let socket = root.0.join("lost-claim-response.sock");
    let daemon = DaemonProcess::spawn(&binary, &database, &socket);

    let unread_response = send_without_reading_response(&socket, &claim(WRITER_A));
    wait_for_writer_fence(&database, 1, WRITER_A);
    drop(unread_response);

    let barrier = Arc::new(Barrier::new(3));
    let (same_response, competing_response) = thread::scope(|scope| {
        let same_barrier = Arc::clone(&barrier);
        let competing_barrier = Arc::clone(&barrier);
        let socket = &socket;
        let same = scope.spawn(move || {
            same_barrier.wait();
            send(socket, &claim(WRITER_A), None)
        });
        let competing = scope.spawn(move || {
            competing_barrier.wait();
            send(socket, &claim(WRITER_B), None)
        });
        barrier.wait();
        (same.join().unwrap(), competing.join().unwrap())
    });

    let same = claimed(same_response);
    let competing = claimed(competing_response);
    assert_eq!(competing, (2, WRITER_B.to_string(), false));
    let (final_generation, final_writer, stale_generation, stale_writer, order) = match same {
        (1, ref writer_id, true) if writer_id == WRITER_A => {
            (2, WRITER_B, 1, WRITER_A, "same-then-competing")
        }
        (3, ref writer_id, false) if writer_id == WRITER_A => {
            (3, WRITER_A, 2, WRITER_B, "competing-then-same")
        }
        other => panic!("same-writer claim was not linearizable: {other:?}"),
    };
    println!(
        "authority-writer-lost-response-linearization order={order} final_generation={final_generation} final_writer={final_writer}"
    );
    wait_for_writer_fence(&database, final_generation, final_writer);

    let stale = send(
        &socket,
        &registration(),
        Some(&fence(stale_writer, stale_generation as u64)),
    );
    assert!(matches!(
        stale.response,
        ProtocolResponse::Error(ref error)
            if error.code == "authority_writer_fence_rejected"
    ));
    let applied = send(
        &socket,
        &registration(),
        Some(&fence(final_writer, final_generation as u64)),
    );
    assert!(matches!(
        applied.response,
        ProtocolResponse::Command(ref result) if result.status == CommandStatus::Applied
    ));
    assert_eq!(
        claimed(send(&socket, &claim(final_writer), None)),
        (final_generation as u64, final_writer.to_string(), true)
    );
    daemon.stop();
}

#[test]
fn lost_final_claim_response_replays_after_cold_restart_before_queued_competitor() {
    let binary = PathBuf::from(env!("CARGO_BIN_EXE_leserpentd"));
    let root = TempRoot::new();
    let database = root.0.join("lost-final-claim-response.sqlite");
    let socket = root.0.join("lost-final-claim-response.sock");

    let first = DaemonProcess::spawn(&binary, &database, &socket);
    assert_eq!(
        claimed(send(&socket, &claim(WRITER_A), None)),
        (1, WRITER_A.to_string(), false)
    );
    let unread_response = send_without_reading_response(&socket, &claim(WRITER_B));
    wait_for_writer_fence(&database, 2, WRITER_B);
    drop(unread_response);
    first.stop();
    assert!(!socket.exists());

    let second = DaemonProcess::spawn(&binary, &database, &socket);
    let accept_gate = hold_ipc_accept(&socket);
    let replay_stream = send_without_reading_response(&socket, &claim(WRITER_B));
    thread::sleep(Duration::from_millis(50));
    let competitor_stream = send_without_reading_response(&socket, &claim(WRITER_C));
    drop(accept_gate);

    assert_eq!(
        claimed(read_queued_response(replay_stream)),
        (2, WRITER_B.to_string(), true)
    );
    assert_eq!(
        claimed(read_queued_response(competitor_stream)),
        (3, WRITER_C.to_string(), false)
    );
    wait_for_writer_fence(&database, 3, WRITER_C);

    let stale = send(&socket, &registration(), Some(&fence(WRITER_B, 2)));
    assert!(matches!(
        stale.response,
        ProtocolResponse::Error(ref error)
            if error.code == "authority_writer_fence_rejected"
    ));
    let applied = send(&socket, &registration(), Some(&fence(WRITER_C, 3)));
    assert!(matches!(
        applied.response,
        ProtocolResponse::Command(ref result) if result.status == CommandStatus::Applied
    ));
    second.stop();
    assert!(!socket.exists());

    let third = DaemonProcess::spawn(&binary, &database, &socket);
    assert_eq!(
        claimed(send(&socket, &claim(WRITER_C), None)),
        (3, WRITER_C.to_string(), true)
    );
    println!(
        "authority-writer-cold-replay queued_order=writer_b-replay-then-writer_c-competitor final_generation=3 final_writer={WRITER_C}"
    );
    third.stop();
}

#[test]
fn lost_claim_response_survives_sigkill_lease_expiry_and_same_socket_recovery() {
    let binary = PathBuf::from(env!("CARGO_BIN_EXE_leserpentd"));
    let root = TempRoot::new();
    let database = root.0.join("lost-claim-sigkill.sqlite");
    let socket = root.0.join("lost-claim-sigkill.sock");

    let first = DaemonProcess::spawn(&binary, &database, &socket);
    assert_eq!(
        claimed(send(&socket, &claim(WRITER_A), None)),
        (1, WRITER_A.to_string(), false)
    );
    let stale_socket = sigkill_after_lost_claim(first, &socket, &database, WRITER_B, 2);
    reject_before_owner_expiry(&binary, &database, &socket, &stale_socket);
    wait_for_owner_lease_expiry(&database, 2, WRITER_B);
    let replacement =
        recover_with_queued_claims(&binary, &database, &socket, WRITER_B, 2, WRITER_C, 3);

    let stale = send(&socket, &registration(), Some(&fence(WRITER_B, 2)));
    assert!(matches!(
        stale.response,
        ProtocolResponse::Error(ref error)
            if error.code == "authority_writer_fence_rejected"
    ));
    let applied = send(&socket, &registration(), Some(&fence(WRITER_C, 3)));
    assert!(matches!(
        applied.response,
        ProtocolResponse::Command(ref result) if result.status == CommandStatus::Applied
    ));
    println!(
        "authority-writer-unclean-replay lease_expiry=natural stale_socket=reclaimed replay_generation=2 final_generation=3"
    );
    replacement.stop();
    assert!(!socket.exists());
}

#[test]
fn repeated_unclean_response_recovery_preserves_generations_and_same_socket() {
    let binary = PathBuf::from(env!("CARGO_BIN_EXE_leserpentd"));
    let root = TempRoot::new();
    let database = root.0.join("repeated-lost-claim-sigkill.sqlite");
    let socket = root.0.join("repeated-lost-claim-sigkill.sock");

    let first = DaemonProcess::spawn(&binary, &database, &socket);
    assert_eq!(
        claimed(send(&socket, &claim(WRITER_A), None)),
        (1, WRITER_A.to_string(), false)
    );

    let first_stale = sigkill_after_lost_claim(first, &socket, &database, WRITER_B, 2);
    reject_before_owner_expiry(&binary, &database, &socket, &first_stale);
    wait_for_owner_lease_expiry(&database, 2, WRITER_B);
    let second = recover_with_queued_claims(&binary, &database, &socket, WRITER_B, 2, WRITER_C, 3);

    let second_stale = sigkill_after_lost_claim(second, &socket, &database, WRITER_A, 4);
    reject_before_owner_expiry(&binary, &database, &socket, &second_stale);
    wait_for_owner_lease_expiry(&database, 4, WRITER_A);
    let third = recover_with_queued_claims(&binary, &database, &socket, WRITER_A, 4, WRITER_B, 5);

    for stale_fence in [fence(WRITER_C, 3), fence(WRITER_A, 4)] {
        let stale = send(&socket, &registration(), Some(&stale_fence));
        assert!(matches!(
            stale.response,
            ProtocolResponse::Error(ref error)
                if error.code == "authority_writer_fence_rejected"
        ));
    }
    let applied = send(&socket, &registration(), Some(&fence(WRITER_B, 5)));
    assert!(matches!(
        applied.response,
        ProtocolResponse::Command(ref result) if result.status == CommandStatus::Applied
    ));
    assert_eq!(
        claimed(send(&socket, &claim(WRITER_B), None)),
        (5, WRITER_B.to_string(), true)
    );
    println!(
        "authority-writer-repeated-unclean-replay cycles=2 generations=1,2,3,4,5 final_writer={WRITER_B}"
    );
    third.stop();
    assert!(!socket.exists());
}

#[test]
fn post_recovery_writer_contention_is_bounded_and_generation_contiguous() {
    assert_eq!(MAX_IPC_CONNECTIONS_PER_TICK, 64);
    let binary = PathBuf::from(env!("CARGO_BIN_EXE_leserpentd"));
    let root = TempRoot::new();
    let database = root.0.join("post-recovery-writer-contention.sqlite");
    let socket = root.0.join("post-recovery-writer-contention.sock");

    let first = DaemonProcess::spawn(&binary, &database, &socket);
    assert_eq!(
        claimed(send(&socket, &claim(WRITER_A), None)),
        (1, WRITER_A.to_string(), false)
    );
    let stale_socket = sigkill_after_lost_claim(first, &socket, &database, WRITER_B, 2);
    reject_before_owner_expiry(&binary, &database, &socket, &stale_socket);
    wait_for_owner_lease_expiry(&database, 2, WRITER_B);

    let replacement = DaemonProcess::spawn(&binary, &database, &socket);
    assert_eq!(
        claimed(send(&socket, &claim(WRITER_B), None)),
        (2, WRITER_B.to_string(), true)
    );

    let writers = (0..MAX_IPC_CONNECTIONS_PER_TICK)
        .map(|index| format!("{index:032x}"))
        .collect::<Vec<_>>();
    let barrier = Arc::new(Barrier::new(writers.len() + 1));
    let (mut claims, elapsed) = thread::scope(|scope| {
        let handles = writers
            .iter()
            .map(|writer| {
                let barrier = Arc::clone(&barrier);
                let socket = &socket;
                scope.spawn(move || {
                    barrier.wait();
                    claimed(send(socket, &claim(writer), None))
                })
            })
            .collect::<Vec<_>>();
        let started = Instant::now();
        barrier.wait();
        let claims = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>();
        (claims, started.elapsed())
    });
    assert!(
        elapsed <= Duration::from_secs(5),
        "post-recovery claim contention exceeded its budget: {elapsed:?}"
    );

    claims.sort_by_key(|(generation, _, _)| *generation);
    assert_eq!(claims.len(), MAX_IPC_CONNECTIONS_PER_TICK);
    for (offset, (generation, _, replayed)) in claims.iter().enumerate() {
        assert_eq!(*generation, u64::try_from(offset).unwrap() + 3);
        assert!(!replayed);
    }
    let mut observed_writers = claims
        .iter()
        .map(|(_, writer, _)| writer.clone())
        .collect::<Vec<_>>();
    observed_writers.sort();
    let mut expected_writers = writers.clone();
    expected_writers.sort();
    assert_eq!(observed_writers, expected_writers);

    let (penultimate_generation, penultimate_writer, _) =
        claims.get(MAX_IPC_CONNECTIONS_PER_TICK - 2).unwrap();
    let (final_generation, final_writer, _) = claims.get(MAX_IPC_CONNECTIONS_PER_TICK - 1).unwrap();
    assert_eq!(
        *final_generation,
        u64::try_from(MAX_IPC_CONNECTIONS_PER_TICK).unwrap() + 2
    );
    wait_for_writer_fence(
        &database,
        i64::try_from(*final_generation).unwrap(),
        final_writer,
    );
    for stale_fence in [
        fence(WRITER_B, 2),
        fence(penultimate_writer, *penultimate_generation),
    ] {
        let stale = send(&socket, &registration(), Some(&stale_fence));
        assert!(matches!(
            stale.response,
            ProtocolResponse::Error(ref error)
                if error.code == "authority_writer_fence_rejected"
        ));
    }
    let applied = send(
        &socket,
        &registration(),
        Some(&fence(final_writer, *final_generation)),
    );
    assert!(matches!(
        applied.response,
        ProtocolResponse::Command(ref result) if result.status == CommandStatus::Applied
    ));
    assert_eq!(
        claimed(send(&socket, &claim(final_writer), None)),
        (*final_generation, final_writer.clone(), true)
    );
    println!(
        "authority-writer-post-recovery-contention contenders={} generations=3..{} elapsed_ms={} final_writer={final_writer}",
        MAX_IPC_CONNECTIONS_PER_TICK,
        final_generation,
        elapsed.as_millis()
    );
    replacement.stop();
    assert!(!socket.exists());
}

#[test]
fn post_recovery_saturated_duplicate_retries_survive_abandoned_responses() {
    assert_eq!(MAX_IPC_CONNECTIONS_PER_TICK, 64);
    assert_eq!(
        SATURATED_DUPLICATE_GROUPS * (READABLE_RETRIES_PER_GROUP + 1),
        MAX_IPC_CONNECTIONS_PER_TICK
    );
    let binary = PathBuf::from(env!("CARGO_BIN_EXE_leserpentd"));
    let root = TempRoot::new();
    let database = root.0.join("post-recovery-duplicate-retry.sqlite");
    let socket = root.0.join("post-recovery-duplicate-retry.sock");

    let first = DaemonProcess::spawn(&binary, &database, &socket);
    assert_eq!(
        claimed(send(&socket, &claim(WRITER_A), None)),
        (1, WRITER_A.to_string(), false)
    );
    let stale_socket = sigkill_after_lost_claim(first, &socket, &database, WRITER_B, 2);
    reject_before_owner_expiry(&binary, &database, &socket, &stale_socket);
    wait_for_owner_lease_expiry(&database, 2, WRITER_B);

    let replacement = DaemonProcess::spawn(&binary, &database, &socket);
    assert_eq!(
        claimed(send(&socket, &claim(WRITER_B), None)),
        (2, WRITER_B.to_string(), true)
    );

    let accept_gate = hold_ipc_accept(&socket);
    let mut readable_retries =
        Vec::with_capacity(SATURATED_DUPLICATE_GROUPS * READABLE_RETRIES_PER_GROUP);
    let mut abandoned_responses = Vec::with_capacity(SATURATED_DUPLICATE_GROUPS);
    for group in 0..SATURATED_DUPLICATE_GROUPS {
        let writer = format!("{:032x}", 0x100_usize + group);
        let generation = u64::try_from(group).unwrap() + 3;
        let abandoned = send_without_reading_response(&socket, &claim(&writer));
        abandoned.shutdown(Shutdown::Read).unwrap();
        abandoned_responses.push(abandoned);
        for _ in 0..READABLE_RETRIES_PER_GROUP {
            readable_retries.push((
                writer.clone(),
                generation,
                send_without_reading_response(&socket, &claim(&writer)),
            ));
        }
    }
    assert_eq!(
        readable_retries.len(),
        SATURATED_DUPLICATE_GROUPS * READABLE_RETRIES_PER_GROUP
    );

    let started = Instant::now();
    drop(accept_gate);
    for (writer, generation, stream) in readable_retries {
        assert_eq!(
            claimed(read_queued_response(stream)),
            (generation, writer, true)
        );
    }
    drop(abandoned_responses);
    let elapsed = started.elapsed();
    assert!(
        elapsed <= Duration::from_secs(5),
        "post-recovery duplicate retry batch exceeded its budget: {elapsed:?}"
    );

    let final_generation = u64::try_from(SATURATED_DUPLICATE_GROUPS).unwrap() + 2;
    let final_writer = format!("{:032x}", 0x100_usize + SATURATED_DUPLICATE_GROUPS - 1);
    wait_for_writer_fence(
        &database,
        i64::try_from(final_generation).unwrap(),
        &final_writer,
    );
    let penultimate_writer = format!("{:032x}", 0x100_usize + SATURATED_DUPLICATE_GROUPS - 2);
    for stale_fence in [
        fence(WRITER_B, 2),
        fence(&penultimate_writer, final_generation - 1),
    ] {
        let stale = send(&socket, &registration(), Some(&stale_fence));
        assert!(matches!(
            stale.response,
            ProtocolResponse::Error(ref error)
                if error.code == "authority_writer_fence_rejected"
        ));
    }
    let applied = send(
        &socket,
        &registration(),
        Some(&fence(&final_writer, final_generation)),
    );
    assert!(matches!(
        applied.response,
        ProtocolResponse::Command(ref result) if result.status == CommandStatus::Applied
    ));
    assert_eq!(
        claimed(send(&socket, &claim(&final_writer), None)),
        (final_generation, final_writer.clone(), true)
    );
    println!(
        "authority-writer-post-recovery-duplicate-retry claims={} abandoned={} readable_replays={} generations=3..{} elapsed_ms={} final_writer={final_writer}",
        MAX_IPC_CONNECTIONS_PER_TICK,
        SATURATED_DUPLICATE_GROUPS,
        SATURATED_DUPLICATE_GROUPS * READABLE_RETRIES_PER_GROUP,
        final_generation,
        elapsed.as_millis()
    );
    replacement.stop();
    assert!(!socket.exists());
}

#[test]
fn post_recovery_mixed_hostile_and_slow_peers_preserve_valid_claim_progress() {
    assert_eq!(MAX_IPC_CONNECTIONS_PER_TICK, 64);
    assert_eq!(MIXED_PEER_GROUPS * 4, MAX_IPC_CONNECTIONS_PER_TICK);
    let binary = PathBuf::from(env!("CARGO_BIN_EXE_leserpentd"));
    let root = TempRoot::new();
    let database = root.0.join("post-recovery-mixed-peers.sqlite");
    let socket = root.0.join("post-recovery-mixed-peers.sock");

    let first = DaemonProcess::spawn(&binary, &database, &socket);
    assert_eq!(
        claimed(send(&socket, &claim(WRITER_A), None)),
        (1, WRITER_A.to_string(), false)
    );
    let stale_socket = sigkill_after_lost_claim(first, &socket, &database, WRITER_B, 2);
    reject_before_owner_expiry(&binary, &database, &socket, &stale_socket);
    wait_for_owner_lease_expiry(&database, 2, WRITER_B);

    let replacement = DaemonProcess::spawn(&binary, &database, &socket);
    assert_eq!(
        claimed(send(&socket, &claim(WRITER_B), None)),
        (2, WRITER_B.to_string(), true)
    );

    let accept_gate = hold_ipc_accept(&socket);
    let mut groups = Vec::with_capacity(MIXED_PEER_GROUPS);
    let mut slow_peers = Vec::with_capacity(MIXED_PEER_GROUPS);
    for group in 0..MIXED_PEER_GROUPS {
        let writer = format!("{:032x}", 0x200_usize + group);
        let unauthorized_writer = format!("{:032x}", 0x300_usize + group);
        let malformed = queue_raw_frame(&socket, b"{not-json}\n");
        let unauthorized = send_with_token_without_reading_response(
            &socket,
            &claim(&unauthorized_writer),
            WRONG_TOKEN,
        );
        slow_peers.push(queue_raw_prefix(&socket, b"{"));
        let valid = send_without_reading_response(&socket, &claim(&writer));
        groups.push((
            u64::try_from(group).unwrap() + 3,
            writer,
            malformed,
            unauthorized,
            valid,
        ));
    }

    let started = Instant::now();
    drop(accept_gate);
    for (generation, writer, malformed, unauthorized, valid) in groups {
        assert_protocol_error(read_queued_response(malformed), "invalid_json");
        assert_protocol_error(read_queued_response(unauthorized), "unauthorized");
        assert_eq!(
            claimed(read_queued_response(valid)),
            (generation, writer, false)
        );
    }
    for mut slow in slow_peers {
        let mut response = Vec::new();
        slow.read_to_end(&mut response).unwrap();
        assert!(response.is_empty());
    }
    let elapsed = started.elapsed();
    assert!(
        elapsed <= Duration::from_secs(5),
        "post-recovery mixed peer batch exceeded its budget: {elapsed:?}"
    );

    let final_generation = u64::try_from(MIXED_PEER_GROUPS).unwrap() + 2;
    let final_writer = format!("{:032x}", 0x200_usize + MIXED_PEER_GROUPS - 1);
    wait_for_writer_fence(
        &database,
        i64::try_from(final_generation).unwrap(),
        &final_writer,
    );
    let penultimate_writer = format!("{:032x}", 0x200_usize + MIXED_PEER_GROUPS - 2);
    for stale_fence in [
        fence(WRITER_B, 2),
        fence(&penultimate_writer, final_generation - 1),
    ] {
        let stale = send(&socket, &registration(), Some(&stale_fence));
        assert_protocol_error(stale, "authority_writer_fence_rejected");
    }
    let applied = send(
        &socket,
        &registration(),
        Some(&fence(&final_writer, final_generation)),
    );
    assert!(matches!(
        applied.response,
        ProtocolResponse::Command(ref result) if result.status == CommandStatus::Applied
    ));
    assert_eq!(
        claimed(send(&socket, &claim(&final_writer), None)),
        (final_generation, final_writer.clone(), true)
    );
    println!(
        "authority-writer-post-recovery-mixed-peers total={} malformed={} unauthorized={} slow_timeouts={} valid={} generations=3..{} read_timeout_ms=2000 elapsed_ms={} final_writer={final_writer}",
        MAX_IPC_CONNECTIONS_PER_TICK,
        MIXED_PEER_GROUPS,
        MIXED_PEER_GROUPS,
        MIXED_PEER_GROUPS,
        MIXED_PEER_GROUPS,
        final_generation,
        elapsed.as_millis()
    );
    replacement.stop();
    assert!(!socket.exists());
}

#[test]
fn repeated_hostile_batches_preserve_owner_heartbeat_and_bounded_sigterm() {
    assert_eq!(MAX_IPC_CONNECTIONS_PER_TICK, 64);
    let binary = PathBuf::from(env!("CARGO_BIN_EXE_leserpentd"));
    let root = TempRoot::new();
    let database = root.0.join("repeated-hostile-lifecycle.sqlite");
    let socket = root.0.join("repeated-hostile-lifecycle.sock");

    let daemon = DaemonProcess::spawn(&binary, &database, &socket);
    assert_eq!(
        claimed(send(&socket, &claim(WRITER_A), None)),
        (1, WRITER_A.to_string(), false)
    );
    let (owner_token, mut lease_expiry) = inspect_owner_lease(&database);
    let mut batch_elapsed = Vec::with_capacity(REPEATED_HOSTILE_BATCHES);
    for _ in 0..REPEATED_HOSTILE_BATCHES {
        batch_elapsed.push(run_saturated_hostile_replay_batch(&socket, WRITER_A, 1));
        lease_expiry = wait_for_owner_lease_extension(&database, &owner_token, lease_expiry);
        assert_eq!(
            inspect_writer_fence(&database).0,
            1,
            "hostile replay peers allocated a writer generation"
        );
    }

    let accept_gate = hold_ipc_accept(&socket);
    let (shutdown_owner, shutdown_gate_expiry) = inspect_owner_lease(&database);
    assert_eq!(shutdown_owner, owner_token);
    let slow_peers = (0..MAX_IPC_CONNECTIONS_PER_TICK)
        .map(|_| queue_raw_prefix(&socket, b"{"))
        .collect::<Vec<_>>();
    drop(accept_gate);
    let shutdown_batch_expiry =
        wait_for_owner_lease_extension(&database, &owner_token, shutdown_gate_expiry);
    assert!(shutdown_batch_expiry > lease_expiry);
    thread::sleep(Duration::from_millis(250));

    let shutdown_elapsed = daemon.stop_with_budget(Duration::from_secs(2));
    assert!(
        shutdown_elapsed < Duration::from_secs(1),
        "SIGTERM did not interrupt active slow frame readers: {shutdown_elapsed:?}"
    );
    drop(slow_peers);
    assert!(!socket.exists());
    let owner_rows: i64 = Connection::open(&database)
        .unwrap()
        .query_row("SELECT COUNT(*) FROM runtime_owner", [], |row| row.get(0))
        .unwrap();
    assert_eq!(owner_rows, 0, "graceful shutdown retained its owner lease");

    let replacement = DaemonProcess::spawn(&binary, &database, &socket);
    assert_eq!(
        claimed(send(&socket, &claim(WRITER_A), None)),
        (1, WRITER_A.to_string(), true)
    );
    println!(
        "authority-writer-repeated-hostile-lifecycle batches={} peers_per_batch={} batch_elapsed_ms={:?} shutdown_slow_peers={} shutdown_elapsed_ms={} owner_lease_released=true immediate_restart=true generation=1",
        REPEATED_HOSTILE_BATCHES,
        MAX_IPC_CONNECTIONS_PER_TICK,
        batch_elapsed
            .iter()
            .map(Duration::as_millis)
            .collect::<Vec<_>>(),
        MAX_IPC_CONNECTIONS_PER_TICK,
        shutdown_elapsed.as_millis()
    );
    replacement.stop();
    assert!(!socket.exists());
}

#[test]
fn repeated_hostile_shutdown_restart_cycles_bound_process_resources() {
    assert_eq!(MAX_IPC_CONNECTIONS_PER_TICK, 64);
    let binary = PathBuf::from(env!("CARGO_BIN_EXE_leserpentd"));
    let root = TempRoot::new();
    let database = root.0.join("hostile-resource-cycles.sqlite");
    let socket = root.0.join("hostile-resource-cycles.sock");
    let mut batch_elapsed = Vec::with_capacity(RESOURCE_LIFECYCLE_CYCLES);
    let mut shutdown_elapsed = Vec::with_capacity(RESOURCE_LIFECYCLE_CYCLES);
    let mut baseline_resources = Vec::with_capacity(RESOURCE_LIFECYCLE_CYCLES);
    let mut returned_resources = Vec::with_capacity(RESOURCE_LIFECYCLE_CYCLES);
    let mut active_resources = Vec::with_capacity(RESOURCE_LIFECYCLE_CYCLES);

    for cycle in 0..RESOURCE_LIFECYCLE_CYCLES {
        let daemon = DaemonProcess::spawn(&binary, &database, &socket);
        assert_eq!(
            claimed(send(&socket, &claim(WRITER_A), None)),
            (1, WRITER_A.to_string(), cycle > 0)
        );
        let pid = daemon.pid();
        let baseline = inspect_process_resources(pid);
        baseline_resources.push(baseline);
        let (owner_token, lease_expiry) = inspect_owner_lease(&database);

        batch_elapsed.push(run_saturated_hostile_replay_batch(&socket, WRITER_A, 1));
        let refreshed_expiry =
            wait_for_owner_lease_extension(&database, &owner_token, lease_expiry);
        let returned = wait_for_process_resources_at_most(pid, baseline);
        returned_resources.push(returned);
        assert_eq!(inspect_writer_fence(&database).0, 1);

        let accept_gate = hold_ipc_accept(&socket);
        let (shutdown_owner, shutdown_gate_expiry) = inspect_owner_lease(&database);
        assert_eq!(shutdown_owner, owner_token);
        let slow_peers = (0..MAX_IPC_CONNECTIONS_PER_TICK)
            .map(|_| queue_raw_prefix(&socket, b"{"))
            .collect::<Vec<_>>();
        drop(accept_gate);
        let shutdown_batch_expiry =
            wait_for_owner_lease_extension(&database, &owner_token, shutdown_gate_expiry);
        assert!(shutdown_batch_expiry > refreshed_expiry);
        active_resources.push(wait_for_saturated_reader_resources(
            pid,
            returned.or(baseline),
        ));

        let elapsed = daemon.stop_with_budget(Duration::from_secs(2));
        assert!(
            elapsed < Duration::from_secs(1),
            "resource lifecycle cycle {cycle} exceeded its SIGTERM budget: {elapsed:?}"
        );
        shutdown_elapsed.push(elapsed);
        drop(slow_peers);
        assert!(!socket.exists());
        let owner_rows: i64 = Connection::open(&database)
            .unwrap()
            .query_row("SELECT COUNT(*) FROM runtime_owner", [], |row| row.get(0))
            .unwrap();
        assert_eq!(owner_rows, 0, "cycle {cycle} retained its owner row");
        assert_process_resources_released(pid);
    }

    let observed_returned = returned_resources
        .iter()
        .copied()
        .flatten()
        .collect::<Vec<_>>();
    if let Some(expected) = observed_returned.first() {
        assert_eq!(observed_returned.len(), RESOURCE_LIFECYCLE_CYCLES);
        assert!(
            observed_returned
                .iter()
                .all(|resources| resources == expected),
            "post-batch process resources drifted across cycles: {observed_returned:?}"
        );
    }

    let final_fence = Connection::open(&database)
        .unwrap()
        .query_row(
            "SELECT generation, writer_id FROM authority_writer_fence WHERE id = 1",
            [],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
        )
        .unwrap();
    assert_eq!(final_fence, (1, WRITER_A.to_string()));
    println!(
        "authority-writer-hostile-resource-cycles cycles={} peers_per_completed_batch={} peers_per_shutdown_batch={} batch_elapsed_ms={:?} shutdown_elapsed_ms={:?} baseline_resources={baseline_resources:?} returned_resources={returned_resources:?} active_resources={active_resources:?} proc_released_each_cycle=true owner_rows_released_each_cycle=true socket_released_each_cycle=true final_generation=1",
        RESOURCE_LIFECYCLE_CYCLES,
        MAX_IPC_CONNECTIONS_PER_TICK,
        MAX_IPC_CONNECTIONS_PER_TICK,
        batch_elapsed
            .iter()
            .map(Duration::as_millis)
            .collect::<Vec<_>>(),
        shutdown_elapsed
            .iter()
            .map(Duration::as_millis)
            .collect::<Vec<_>>()
    );
}

#[test]
fn burst_reconnects_remain_fair_across_repeated_saturated_hostile_waves() {
    assert_eq!(MAX_IPC_CONNECTIONS_PER_TICK, 64);
    let binary = PathBuf::from(env!("CARGO_BIN_EXE_leserpentd"));
    let root = TempRoot::new();
    let database = root.0.join("hostile-reconnect-fairness.sqlite");
    let socket = root.0.join("hostile-reconnect-fairness.sock");

    let daemon = DaemonProcess::spawn(&binary, &database, &socket);
    assert_eq!(
        claimed(send(&socket, &claim(WRITER_A), None)),
        (1, WRITER_A.to_string(), false)
    );
    let (owner_token, mut lease_expiry) = inspect_owner_lease(&database);
    let mut wave_elapsed = Vec::with_capacity(RECONNECT_FAIRNESS_WAVES);
    let mut reconnect_elapsed = Vec::with_capacity(RECONNECT_FAIRNESS_WAVES);
    for _ in 0..RECONNECT_FAIRNESS_WAVES {
        let (wave, reconnects) = run_saturated_reconnect_fairness_wave(&socket, WRITER_A, 1);
        wave_elapsed.push(wave);
        reconnect_elapsed.push(reconnects);
        lease_expiry = wait_for_owner_lease_extension(&database, &owner_token, lease_expiry);
        assert_eq!(inspect_writer_fence(&database).0, 1);
    }
    assert_eq!(
        claimed(send(&socket, &claim(WRITER_A), None)),
        (1, WRITER_A.to_string(), true)
    );

    println!(
        "authority-writer-hostile-reconnect-fairness waves={} peers_per_wave={} slow_peers_per_wave={} reconnects_per_wave={} wave_elapsed_ms={:?} reconnect_elapsed_ms={:?} heartbeat_advanced_each_wave=true final_generation=1",
        RECONNECT_FAIRNESS_WAVES,
        MAX_IPC_CONNECTIONS_PER_TICK,
        RECONNECTS_PER_FAIRNESS_WAVE * SLOW_PEERS_PER_RECONNECT_GROUP,
        RECONNECTS_PER_FAIRNESS_WAVE,
        wave_elapsed
            .iter()
            .map(Duration::as_millis)
            .collect::<Vec<_>>(),
        reconnect_elapsed
            .iter()
            .map(|wave| wave.iter().map(Duration::as_millis).collect::<Vec<_>>())
            .collect::<Vec<_>>()
    );
    daemon.stop();
    assert!(!socket.exists());
}

#[test]
fn fresh_daemon_process_preserves_and_advances_the_writer_generation() {
    let binary = PathBuf::from(env!("CARGO_BIN_EXE_leserpentd"));
    let root = TempRoot::new();
    let database = root.0.join("authority.sqlite");
    let socket = root.0.join("authority.sock");

    let first = DaemonProcess::spawn(&binary, &database, &socket);
    let first_claim = send(&socket, &claim(WRITER_A), None);
    assert!(matches!(
        first_claim.response,
        ProtocolResponse::AuthorityWriterClaimed(ref response)
            if response.generation == 1 && !response.replayed
    ));
    let registered = send(&socket, &registration(), Some(&fence(WRITER_A, 1)));
    assert!(matches!(
        registered.response,
        ProtocolResponse::Command(ref result) if result.status == CommandStatus::Applied
    ));

    let contender_socket = root.0.join("contender.sock");
    let contender = ProcessCommand::new(&binary)
        .args([
            "--database",
            database.to_str().unwrap(),
            "--socket",
            contender_socket.to_str().unwrap(),
            "--once",
        ])
        .env("LESERPENT_IPC_TOKEN", TOKEN)
        .output()
        .unwrap();
    assert!(!contender.status.success());
    assert!(
        String::from_utf8(contender.stderr)
            .unwrap()
            .contains("owned by another live process")
    );
    assert!(!contender_socket.exists());
    first.stop();
    assert!(!socket.exists());

    let second = DaemonProcess::spawn(&binary, &database, &socket);
    let missing = send(&socket, &refresh(), None);
    assert!(matches!(
        missing.response,
        ProtocolResponse::Error(ref error)
            if error.code == "authority_writer_fence_required"
    ));
    let takeover = send(&socket, &claim(WRITER_B), None);
    assert!(matches!(
        takeover.response,
        ProtocolResponse::AuthorityWriterClaimed(ref response)
            if response.generation == 2 && !response.replayed
    ));
    let stale = send(&socket, &refresh(), Some(&fence(WRITER_A, 1)));
    assert!(matches!(
        stale.response,
        ProtocolResponse::Error(ref error)
            if error.code == "authority_writer_fence_rejected"
    ));
    let refreshed = send(&socket, &refresh(), Some(&fence(WRITER_B, 2)));
    assert!(matches!(
        refreshed.response,
        ProtocolResponse::Command(ref result)
            if result.status == CommandStatus::Applied && result.runtime.revision == Revision(2)
    ));
    second.stop();

    let third = DaemonProcess::spawn(&binary, &database, &socket);
    let replay = send(&socket, &claim(WRITER_B), None);
    assert!(matches!(
        replay.response,
        ProtocolResponse::AuthorityWriterClaimed(ref response)
            if response.generation == 2 && response.replayed
    ));
    third.stop();
}
