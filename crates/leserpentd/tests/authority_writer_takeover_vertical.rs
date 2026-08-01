#![cfg(unix)]

use std::fs;
use std::io::{Read, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command as ProcessCommand, Stdio};
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

const TOKEN: &str = "0123456789abcdef0123456789abcdef";
const WRITER_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const WRITER_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

struct TempRoot(PathBuf);

impl TempRoot {
    fn new() -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = PathBuf::from(format!("/tmp/leserpent-aw-{}-{unique}", std::process::id()));
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
            if socket.exists() {
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
