#![cfg(unix)]

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use leserpent_domain::RuntimeId;
use leserpent_protocol::{ProtocolResponse, decode_response};
use leserpent_runtime::ControlRuntime;
use leserpentd::IpcServer;

const TOKEN: &str = "0123456789abcdef0123456789abcdef";

#[test]
fn native_cli_uses_authenticated_wire_v1_for_health_and_runtime_list() {
    let database = temp_path("sqlite");
    let socket = temp_path("sock");
    let stop = Arc::new(AtomicBool::new(false));
    let (ready_tx, ready_rx) = mpsc::channel();
    let server_stop = Arc::clone(&stop);
    let server_database = database.clone();
    let server_socket = socket.clone();
    let server = thread::spawn(move || {
        let mut runtime = ControlRuntime::open(&server_database).unwrap();
        runtime
            .register_runtime(
                RuntimeId::new("runtime-a").unwrap(),
                "Runtime A",
                "http://127.0.0.1:9411",
            )
            .unwrap();
        let ipc = IpcServer::bind(&server_socket, TOKEN).unwrap();
        ready_tx.send(()).unwrap();
        while !server_stop.load(Ordering::Acquire) {
            ipc.poll_once(&mut runtime).unwrap();
            thread::sleep(Duration::from_millis(1));
        }
    });
    ready_rx.recv_timeout(Duration::from_secs(2)).unwrap();

    let binary = env!("CARGO_BIN_EXE_leserpent");
    let health = Command::new(binary)
        .args(["--socket", socket.to_str().unwrap(), "health"])
        .env("LESERPENT_IPC_TOKEN", TOKEN)
        .output()
        .unwrap();
    assert!(health.status.success());
    let health_stdout = String::from_utf8(health.stdout).unwrap();
    assert!(health_stdout.contains("status=ready authority_owned=true"));
    assert!(health_stdout.contains("queue_active=0"));

    let list = Command::new(binary)
        .args([
            "--socket",
            socket.to_str().unwrap(),
            "--json",
            "runtime",
            "list",
        ])
        .env("LESERPENT_IPC_TOKEN", TOKEN)
        .env("LESERPENT_PRINCIPAL", "integration-test")
        .output()
        .unwrap();
    assert!(list.status.success());
    let response = decode_response(trim_ascii_whitespace(&list.stdout)).unwrap();
    let ProtocolResponse::Query(leserpent_domain::QueryResult::RuntimeList { runtimes, .. }) =
        response.response
    else {
        panic!("CLI JSON must preserve the runtime-list response envelope");
    };
    assert_eq!(runtimes.len(), 1);
    assert_eq!(runtimes[0].id.as_str(), "runtime-a");

    let unconfirmed = Command::new(binary)
        .args([
            "--socket",
            socket.to_str().unwrap(),
            "runtime",
            "refresh",
            "runtime-a",
        ])
        .output()
        .unwrap();
    assert_eq!(unconfirmed.status.code(), Some(2));
    assert!(
        String::from_utf8(unconfirmed.stderr)
            .unwrap()
            .contains("requires --dry-run or explicit --yes")
    );

    let export = Command::new(binary)
        .args(["runtime", "refresh", "runtime-a", "--export-leselang"])
        .output()
        .unwrap();
    assert!(export.status.success());
    assert_eq!(
        String::from_utf8(export.stdout).unwrap().trim(),
        "fn main() = runtime.refresh(runtime_id: \"runtime-a\")"
    );

    let dry_run = Command::new(binary)
        .args([
            "--socket",
            socket.to_str().unwrap(),
            "runtime",
            "refresh",
            "runtime-a",
            "--dry-run",
            "--expected-revision",
            "1",
        ])
        .env("LESERPENT_IPC_TOKEN", TOKEN)
        .output()
        .unwrap();
    assert!(dry_run.status.success());
    assert!(
        String::from_utf8(dry_run.stdout)
            .unwrap()
            .contains("status=planned runtime=runtime-a revision=2")
    );

    let apply = Command::new(binary)
        .args([
            "--socket",
            socket.to_str().unwrap(),
            "--json",
            "runtime",
            "refresh",
            "runtime-a",
            "--yes",
            "--expected-revision",
            "1",
            "--idempotency-key",
            "integration-refresh",
        ])
        .env("LESERPENT_IPC_TOKEN", TOKEN)
        .output()
        .unwrap();
    assert!(apply.status.success());
    let applied = decode_response(trim_ascii_whitespace(&apply.stdout)).unwrap();
    let ProtocolResponse::Command(result) = applied.response else {
        panic!("confirmed refresh must return a command response");
    };
    assert_eq!(result.status, leserpent_domain::CommandStatus::Applied);
    assert_eq!(result.runtime.revision.0, 2);

    let health_after_apply = Command::new(binary)
        .args(["--socket", socket.to_str().unwrap(), "--json", "health"])
        .env("LESERPENT_IPC_TOKEN", TOKEN)
        .output()
        .unwrap();
    assert!(health_after_apply.status.success());
    let health = decode_response(trim_ascii_whitespace(&health_after_apply.stdout)).unwrap();
    let ProtocolResponse::Health(health) = health.response else {
        panic!("health command must return health");
    };
    assert_eq!(health.effect_queue.unwrap().ready, 1);

    stop.store(true, Ordering::Release);
    server.join().unwrap();
    fs::remove_file(database).unwrap();
}

fn temp_path(extension: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "leserpent-cli-{}-{unique}.{extension}",
        std::process::id()
    ))
}

fn trim_ascii_whitespace(mut value: &[u8]) -> &[u8] {
    while value.last().is_some_and(u8::is_ascii_whitespace) {
        value = &value[..value.len() - 1];
    }
    value
}
