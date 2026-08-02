#![cfg(unix)]

use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::Shutdown;
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::os::unix::net::UnixStream;
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command as ProcessCommand, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Barrier};
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
use leserpentd::MAX_IPC_CONNECTIONS_PER_TICK;
use rusqlite::Connection;

const TOKEN: &str = "0123456789abcdef0123456789abcdef";
const WRITER_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const WRITER_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const WRITER_C: &str = "cccccccccccccccccccccccccccccccc";
const CRASH_WORKER_DATABASE: &str = "LESERPENT_TEST_AUTHORITY_WRITER_DATABASE";
const CRASH_WORKER_ID: &str = "LESERPENT_TEST_AUTHORITY_WRITER_ID";
static TEMP_ROOT_SEQUENCE: AtomicU64 = AtomicU64::new(1);

struct TempRoot(PathBuf);

impl TempRoot {
    fn new() -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let sequence = TEMP_ROOT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let root = PathBuf::from(format!(
            "/tmp/leserpent-aw-{}-{unique}-{sequence}",
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

    fn stop(mut self) {
        let mut child = self.child.take().unwrap();
        // The production signal loop must release the SQLite owner lease and
        // socket before a fresh authority process is admitted.
        assert_eq!(unsafe { libc::kill(child.id() as i32, libc::SIGTERM) }, 0);
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                assert!(
                    status.success(),
                    "leserpentd did not stop cleanly: {status}"
                );
                return;
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("leserpentd did not stop after SIGTERM");
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
    let mut stream = UnixStream::connect(socket).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let mut frame = serde_json::to_vec(&serde_json::json!({
        "token": TOKEN,
        "request": request,
    }))
    .unwrap();
    frame.push(b'\n');
    stream.write_all(&frame).unwrap();
    stream.flush().unwrap();
    stream.shutdown(Shutdown::Write).unwrap();
    stream
}

fn read_queued_response(mut stream: UnixStream) -> ResponseEnvelope {
    let mut response = Vec::new();
    stream.read_to_end(&mut response).unwrap();
    decode_response(&response).unwrap()
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

fn spawn_claim_crash_worker(
    database: &Path,
    writer_id: &str,
) -> (Child, ChildStdin, BufReader<ChildStdout>) {
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
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let stdin = child.stdin.take().unwrap();
    let stdout = BufReader::new(child.stdout.take().unwrap());
    (child, stdin, stdout)
}

fn wait_for_worker_marker(reader: &mut BufReader<ChildStdout>, marker: &str) {
    let mut transcript = String::new();
    loop {
        let mut line = String::new();
        let read = reader.read_line(&mut line).unwrap();
        assert!(
            read > 0,
            "claim worker exited before {marker}: {transcript}"
        );
        transcript.push_str(&line);
        if line.contains(marker) {
            return;
        }
    }
}

fn kill_worker(child: &mut Child) {
    assert_eq!(unsafe { libc::kill(child.id() as i32, libc::SIGKILL) }, 0);
    let status = child.wait().unwrap();
    assert_eq!(status.signal(), Some(libc::SIGKILL));
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
    let writer_id = std::env::var(CRASH_WORKER_ID).unwrap();
    let mut runtime = ControlRuntime::open(database).unwrap();
    println!("authority-writer-worker-ready");
    std::io::stdout().flush().unwrap();
    let mut command = [0_u8; 1];
    std::io::stdin().read_exact(&mut command).unwrap();
    assert_eq!(command, [b'C']);
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
    let root = TempRoot::new();
    let database = root.0.join("claim-crash.sqlite");
    {
        let mut baseline = ControlRuntime::open(&database).unwrap();
        let claim = baseline.claim_authority_writer(WRITER_A).unwrap();
        assert_eq!(claim.generation, 1);
    }

    let (mut interrupted, mut interrupted_stdin, mut interrupted_stdout) =
        spawn_claim_crash_worker(&database, WRITER_B);
    wait_for_worker_marker(&mut interrupted_stdout, "authority-writer-worker-ready");
    let blocker = Connection::open(&database).unwrap();
    blocker.execute_batch("BEGIN DEFERRED").unwrap();
    let baseline_generation: i64 = blocker
        .query_row(
            "SELECT generation FROM authority_writer_fence WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(baseline_generation, 1);
    interrupted_stdin.write_all(b"C").unwrap();
    interrupted_stdin.flush().unwrap();
    let rollback_journal = PathBuf::from(format!("{}-journal", database.display()));
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if fs::metadata(&rollback_journal)
            .map(|metadata| metadata.len() > 0)
            .unwrap_or(false)
        {
            break;
        }
        assert!(
            interrupted.try_wait().unwrap().is_none(),
            "claim worker exited before creating its rollback journal"
        );
        assert!(
            Instant::now() < deadline,
            "claim worker did not reach the rollback-journal write boundary"
        );
        thread::sleep(Duration::from_millis(10));
    }
    thread::sleep(Duration::from_millis(100));
    assert!(
        interrupted.try_wait().unwrap().is_none(),
        "reader lock must hold the writer inside COMMIT"
    );
    kill_worker(&mut interrupted);
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

    let (mut committed, mut committed_stdin, mut committed_stdout) =
        spawn_claim_crash_worker(&database, WRITER_C);
    wait_for_worker_marker(&mut committed_stdout, "authority-writer-worker-ready");
    committed_stdin.write_all(b"C").unwrap();
    committed_stdin.flush().unwrap();
    wait_for_worker_marker(
        &mut committed_stdout,
        "authority-writer-worker-committed generation=3",
    );
    kill_worker(&mut committed);
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
