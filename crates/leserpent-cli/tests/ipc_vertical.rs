#![cfg(unix)]

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use leserpent_adapters::{EffectAdapter, GEWYVERN_PROVISIONING_EFFECT_KIND};
use leserpent_domain::RuntimeId;
use leserpent_domain::provisioning::RuntimeProvisioning;
use leserpent_protocol::provisioning::{
    PROVISIONING_PROTOCOL_SCHEMA_VERSION, ProvisioningResponse, ProvisioningResponseEnvelope,
    decode_provisioning_request, encode_provisioning_response,
};
use leserpent_protocol::{ProtocolResponse, decode_response};
use leserpent_runtime::{ControlRuntime, EffectExecution};
use leserpentd::{AdapterRegistry, DaemonConfig, DaemonHost, IpcServer};

const TOKEN: &str = "0123456789abcdef0123456789abcdef";

struct FailedProvisioningAdapter;

impl EffectAdapter for FailedProvisioningAdapter {
    fn kind(&self) -> &str {
        GEWYVERN_PROVISIONING_EFFECT_KIND
    }

    fn execute(&mut self, payload: &[u8]) -> EffectExecution {
        let request = decode_provisioning_request(payload).unwrap();
        let mut provisioning = RuntimeProvisioning::plan(
            &request.request.principal,
            &request.request.capabilities,
            request.request.intent,
        )
        .unwrap();
        provisioning.begin().unwrap();
        let state = provisioning.record_fault("test_install_failed").unwrap();
        EffectExecution::Complete(
            encode_provisioning_response(&ProvisioningResponseEnvelope {
                schema_version: PROVISIONING_PROTOCOL_SCHEMA_VERSION,
                response: ProvisioningResponse::State(state),
            })
            .unwrap(),
        )
    }
}

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
        let ipc = IpcServer::bind(&server_socket, TOKEN)
            .unwrap()
            .with_bootstrap_submission()
            .with_provisioning_submission();
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

    let inspect = Command::new(binary)
        .args([
            "--socket",
            socket.to_str().unwrap(),
            "runtime",
            "inspect",
            "runtime-a",
        ])
        .env("LESERPENT_IPC_TOKEN", TOKEN)
        .output()
        .unwrap();
    assert!(inspect.status.success());
    let inspect_stdout = String::from_utf8(inspect.stdout).unwrap();
    assert!(inspect_stdout.contains("runtime=runtime-a"));
    assert!(inspect_stdout.contains("endpoint=http://127.0.0.1:9411"));

    let watch = Command::new(binary)
        .args([
            "--socket",
            socket.to_str().unwrap(),
            "runtime",
            "watch",
            "runtime-a",
            "--count",
            "2",
            "--interval-ms",
            "50",
        ])
        .env("LESERPENT_IPC_TOKEN", TOKEN)
        .output()
        .unwrap();
    assert!(watch.status.success());
    let watch_stdout = String::from_utf8(watch.stdout).unwrap();
    assert_eq!(watch_stdout.lines().count(), 2);
    assert!(watch_stdout.contains("capabilities=unobserved"));
    assert!(watch_stdout.contains("capabilities_observed_for_revision=none"));
    assert!(watch_stdout.contains("runtime=runtime-a"));

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

    let history = Command::new(binary)
        .args([
            "--socket",
            socket.to_str().unwrap(),
            "runtime",
            "history",
            "runtime-a",
        ])
        .env("LESERPENT_IPC_TOKEN", TOKEN)
        .output()
        .unwrap();
    assert!(history.status.success());
    let history_stdout = String::from_utf8(history.stdout).unwrap();
    assert!(history_stdout.contains("entries=1"));
    assert!(history_stdout.contains("\truntime-a\t2\tapplied"));

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

    let unconfirmed_bootstrap = Command::new(binary)
        .args([
            "--socket",
            socket.to_str().unwrap(),
            "bootstrap",
            "deploy",
            "bootstrap-cli-1",
            "--host",
            "host.example",
            "--credential-handle",
            "vault:ssh:host-example",
        ])
        .env("LESERPENT_IPC_TOKEN", TOKEN)
        .output()
        .unwrap();
    assert_eq!(unconfirmed_bootstrap.status.code(), Some(2));
    assert!(
        String::from_utf8(unconfirmed_bootstrap.stderr)
            .unwrap()
            .contains("requires explicit --yes")
    );

    let bootstrap = Command::new(binary)
        .args([
            "--socket",
            socket.to_str().unwrap(),
            "bootstrap",
            "deploy",
            "bootstrap-cli-1",
            "--host",
            "host.example",
            "--port",
            "22",
            "--credential-handle",
            "vault:ssh:host-example",
            "--yes",
        ])
        .env("LESERPENT_IPC_TOKEN", TOKEN)
        .env("LESERPENT_PRINCIPAL", "integration-test")
        .output()
        .unwrap();
    assert!(bootstrap.status.success());
    assert!(
        String::from_utf8(bootstrap.stdout)
            .unwrap()
            .contains("bootstrap=bootstrap-cli-1 phase=planned target=host.example:22")
    );

    let inspect_bootstrap = Command::new(binary)
        .args([
            "--socket",
            socket.to_str().unwrap(),
            "bootstrap",
            "inspect",
            "bootstrap-cli-1",
        ])
        .env("LESERPENT_IPC_TOKEN", TOKEN)
        .output()
        .unwrap();
    assert!(inspect_bootstrap.status.success());
    assert!(
        String::from_utf8(inspect_bootstrap.stdout)
            .unwrap()
            .contains("bootstrap=bootstrap-cli-1 phase=planned")
    );

    let unconfirmed_provisioning = Command::new(binary)
        .args([
            "--socket",
            socket.to_str().unwrap(),
            "runtime",
            "provision",
            "runtime-new",
            "--provisioning-id",
            "provision-cli-1",
            "--host",
            "runtime.example",
            "--credential-handle",
            "vault:ssh:runtime-example",
        ])
        .env("LESERPENT_IPC_TOKEN", TOKEN)
        .output()
        .unwrap();
    assert_eq!(unconfirmed_provisioning.status.code(), Some(2));
    assert!(
        String::from_utf8(unconfirmed_provisioning.stderr)
            .unwrap()
            .contains("requires explicit --yes")
    );

    let provisioning = Command::new(binary)
        .args([
            "--socket",
            socket.to_str().unwrap(),
            "runtime",
            "provision",
            "runtime-new",
            "--provisioning-id",
            "provision-cli-1",
            "--host",
            "runtime.example",
            "--credential-handle",
            "vault:ssh:runtime-example",
            "--yes",
        ])
        .env("LESERPENT_IPC_TOKEN", TOKEN)
        .env("LESERPENT_PRINCIPAL", "integration-test")
        .output()
        .unwrap();
    assert!(provisioning.status.success());
    let provisioning_output = String::from_utf8(provisioning.stdout).unwrap();
    assert!(provisioning_output.contains("provisioning=provision-cli-1"));
    assert!(provisioning_output.contains("runtime=runtime-new phase=planned"));
    assert!(!provisioning_output.contains("runtime-example"));

    let bounded_wait = Command::new(binary)
        .args([
            "--socket",
            socket.to_str().unwrap(),
            "runtime",
            "provision",
            "runtime-new",
            "--provisioning-id",
            "provision-cli-1",
            "--host",
            "runtime.example",
            "--credential-handle",
            "vault:ssh:runtime-example",
            "--yes",
            "--wait",
            "--count",
            "1",
        ])
        .env("LESERPENT_IPC_TOKEN", TOKEN)
        .env("LESERPENT_PRINCIPAL", "integration-test")
        .output()
        .unwrap();
    assert_eq!(bounded_wait.status.code(), Some(5));
    assert!(
        String::from_utf8(bounded_wait.stdout)
            .unwrap()
            .contains("phase=planned")
    );
    assert!(
        String::from_utf8(bounded_wait.stderr)
            .unwrap()
            .contains("did not reach a terminal phase after 1 observations")
    );

    stop.store(true, Ordering::Release);
    server.join().unwrap();
    fs::remove_file(database).unwrap();
}

#[test]
fn native_cli_wait_returns_a_distinct_terminal_provisioning_failure() {
    let database = temp_path("fail.sqlite");
    let socket = temp_path("fail.sock");
    let stop = Arc::new(AtomicBool::new(false));
    let (ready_tx, ready_rx) = mpsc::channel();
    let server_stop = Arc::clone(&stop);
    let server_database = database.clone();
    let server_socket = socket.clone();
    let server = thread::spawn(move || {
        let runtime = ControlRuntime::open(&server_database).unwrap();
        let mut registry = AdapterRegistry::default();
        registry.register(FailedProvisioningAdapter).unwrap();
        let mut host = DaemonHost::new(
            runtime,
            registry,
            DaemonConfig {
                idle_interval: Duration::from_millis(1),
                ..DaemonConfig::default()
            },
        )
        .unwrap();
        let ipc = IpcServer::bind(&server_socket, TOKEN)
            .unwrap()
            .with_provisioning_submission();
        ready_tx.send(()).unwrap();
        while !server_stop.load(Ordering::Acquire) {
            ipc.poll_once(host.runtime_mut()).unwrap();
            host.tick().unwrap();
            thread::sleep(Duration::from_millis(1));
        }
    });
    ready_rx.recv_timeout(Duration::from_secs(2)).unwrap();

    let result = Command::new(env!("CARGO_BIN_EXE_leserpent"))
        .args([
            "--socket",
            socket.to_str().unwrap(),
            "runtime",
            "provision",
            "runtime-failed",
            "--provisioning-id",
            "provision-failed-1",
            "--host",
            "runtime.example",
            "--credential-handle",
            "vault:ssh:runtime-example",
            "--yes",
            "--wait",
            "--count",
            "20",
            "--interval-ms",
            "50",
        ])
        .env("LESERPENT_IPC_TOKEN", TOKEN)
        .env("LESERPENT_PRINCIPAL", "integration-test")
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(4));
    let stdout = String::from_utf8(result.stdout).unwrap();
    assert!(stdout.contains("phase=planned"));
    assert!(stdout.contains("phase=failed"));
    assert!(stdout.contains("fault=test_install_failed"));

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
