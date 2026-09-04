#![cfg(unix)]

mod support;

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use leserpent_adapters::{
    DAEMON_RETIREMENT_EFFECT_KIND, EffectAdapter, GEWYVERN_PROVISIONING_EFFECT_KIND,
    GEWYVERN_RETIREMENT_EFFECT_KIND,
};
use leserpent_domain::bootstrap_retirement::DaemonRetirement;
use leserpent_domain::provisioning::RuntimeProvisioning;
use leserpent_domain::retirement::RuntimeRetirement;
use leserpent_protocol::bootstrap_retirement_control::{
    DAEMON_RETIREMENT_PROTOCOL_SCHEMA_VERSION, DaemonRetirementResponse,
    DaemonRetirementResponseEnvelope, decode_daemon_retirement_effect,
    encode_daemon_retirement_response,
};
use leserpent_protocol::provisioning::{
    PROVISIONING_PROTOCOL_SCHEMA_VERSION, ProvisioningResponse, ProvisioningResponseEnvelope,
    decode_provisioning_request, decode_provisioning_response, encode_provisioning_response,
};
use leserpent_protocol::retirement::{
    RETIREMENT_PROTOCOL_SCHEMA_VERSION, RetirementResponse, RetirementResponseEnvelope,
    decode_retirement_request, encode_retirement_response,
};
use leserpent_protocol::{ProtocolResponse, decode_response};
use leserpent_runtime::{ControlRuntime, EffectExecution};
use leserpentd::{AdapterRegistry, DaemonConfig, DaemonHost, IpcServer};

const TOKEN: &str = "0123456789abcdef0123456789abcdef";

struct FailedProvisioningAdapter;
struct FailedRetirementAdapter;
struct FailedDaemonRetirementAdapter;

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

impl EffectAdapter for FailedRetirementAdapter {
    fn kind(&self) -> &str {
        GEWYVERN_RETIREMENT_EFFECT_KIND
    }

    fn execute(&mut self, payload: &[u8]) -> EffectExecution {
        let request = decode_retirement_request(payload).unwrap();
        let mut retirement = RuntimeRetirement::plan(
            &request.request.principal,
            &request.request.capabilities,
            request.request.intent,
        )
        .unwrap();
        retirement.begin().unwrap();
        let state = retirement.record_fault("test_retirement_failed").unwrap();
        EffectExecution::Complete(
            encode_retirement_response(&RetirementResponseEnvelope {
                schema_version: RETIREMENT_PROTOCOL_SCHEMA_VERSION,
                response: RetirementResponse::State(state),
            })
            .unwrap(),
        )
    }
}

impl EffectAdapter for FailedDaemonRetirementAdapter {
    fn kind(&self) -> &str {
        DAEMON_RETIREMENT_EFFECT_KIND
    }

    fn execute(&mut self, payload: &[u8]) -> EffectExecution {
        let effect = decode_daemon_retirement_effect(payload).unwrap();
        let mut retirement = DaemonRetirement::resume(&effect.checkpoint).unwrap();
        retirement.begin().unwrap();
        let state = retirement
            .record_fault("test_daemon_retirement_failed")
            .unwrap();
        EffectExecution::Complete(
            encode_daemon_retirement_response(&DaemonRetirementResponseEnvelope {
                schema_version: DAEMON_RETIREMENT_PROTOCOL_SCHEMA_VERSION,
                response: DaemonRetirementResponse::State(state),
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
        support::seed_registered_runtime(
            &mut runtime,
            "provision-runtime-a",
            "runtime-a",
            "runtime.example",
        );
        support::seed_bound_deployment(&mut runtime, "bootstrap-cli-retire");
        let ipc = IpcServer::bind(&server_socket, TOKEN)
            .unwrap()
            .with_bootstrap_submission()
            .with_provisioning_submission()
            .with_retirement_submission()
            .with_daemon_retirement_submission();
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
    assert!(health_stdout.contains("unregister_replay=0/256"));
    assert!(health_stdout.contains("next=1 evicted_through=0"));

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
    assert!(inspect_stdout.contains("endpoint=https://runtime.example:9411/"));

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

    let missing_watch = Command::new(binary)
        .args([
            "--socket",
            socket.to_str().unwrap(),
            "--json",
            "runtime",
            "watch",
            "runtime-missing",
            "--count",
            "1",
        ])
        .env("LESERPENT_IPC_TOKEN", TOKEN)
        .output()
        .unwrap();
    assert_eq!(missing_watch.status.code(), Some(3));
    assert!(missing_watch.stderr.is_empty());
    let missing_watch = decode_response(trim_ascii_whitespace(&missing_watch.stdout)).unwrap();
    assert!(matches!(missing_watch.response, ProtocolResponse::Error(_)));

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

    let unconfirmed_daemon_retirement = Command::new(binary)
        .args([
            "--socket",
            socket.to_str().unwrap(),
            "bootstrap",
            "retire",
            "bootstrap-cli-retire",
            "--retirement-id",
            "retire-daemon-cli-1",
            "--credential-handle",
            "vault:ssh:daemon-retirement-secret",
        ])
        .env("LESERPENT_IPC_TOKEN", TOKEN)
        .env("LESERPENT_PRINCIPAL", "integration-test")
        .output()
        .unwrap();
    assert_eq!(unconfirmed_daemon_retirement.status.code(), Some(2));
    assert!(
        String::from_utf8(unconfirmed_daemon_retirement.stderr)
            .unwrap()
            .contains("requires explicit --yes")
    );

    let daemon_retirement = Command::new(binary)
        .args([
            "--socket",
            socket.to_str().unwrap(),
            "bootstrap",
            "retire",
            "bootstrap-cli-retire",
            "--retirement-id",
            "retire-daemon-cli-1",
            "--credential-handle",
            "vault:ssh:daemon-retirement-secret",
            "--yes",
        ])
        .env("LESERPENT_IPC_TOKEN", TOKEN)
        .env("LESERPENT_PRINCIPAL", "integration-test")
        .output()
        .unwrap();
    assert!(
        daemon_retirement.status.success(),
        "{}",
        String::from_utf8_lossy(&daemon_retirement.stderr)
    );
    let output = String::from_utf8(daemon_retirement.stdout).unwrap();
    assert!(output.contains("daemon_retirement=retire-daemon-cli-1"));
    assert!(output.contains("bootstrap=bootstrap-cli-retire"));
    assert!(output.contains("phase=planned"));
    assert!(!output.contains("daemon-retirement-secret"));

    let bounded_daemon_retirement_wait = Command::new(binary)
        .args([
            "--socket",
            socket.to_str().unwrap(),
            "bootstrap",
            "retire",
            "bootstrap-cli-retire",
            "--retirement-id",
            "retire-daemon-cli-1",
            "--credential-handle",
            "vault:ssh:daemon-retirement-secret",
            "--yes",
            "--wait",
            "--count",
            "1",
        ])
        .env("LESERPENT_IPC_TOKEN", TOKEN)
        .env("LESERPENT_PRINCIPAL", "integration-test")
        .output()
        .unwrap();
    assert_eq!(bounded_daemon_retirement_wait.status.code(), Some(5));
    assert!(
        String::from_utf8(bounded_daemon_retirement_wait.stderr)
            .unwrap()
            .contains("daemon retirement retire-daemon-cli-1 did not reach a terminal phase")
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

    let conflicting_provisioning = Command::new(binary)
        .args([
            "--socket",
            socket.to_str().unwrap(),
            "--json",
            "runtime",
            "provision",
            "runtime-conflict",
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
    assert_eq!(conflicting_provisioning.status.code(), Some(3));
    assert!(conflicting_provisioning.stderr.is_empty());
    let conflicting_provisioning =
        decode_provisioning_response(trim_ascii_whitespace(&conflicting_provisioning.stdout))
            .unwrap();
    assert!(matches!(
        conflicting_provisioning.response,
        ProvisioningResponse::Error(_)
    ));

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

    let unconfirmed_retirement = Command::new(binary)
        .args([
            "--socket",
            socket.to_str().unwrap(),
            "runtime",
            "retire",
            "runtime-a",
            "--retirement-id",
            "retire-cli-1",
            "--provisioning-id",
            "provision-runtime-a",
            "--host",
            "runtime.example",
            "--credential-handle",
            "vault:ssh:secret-retirement-handle",
        ])
        .env("LESERPENT_IPC_TOKEN", TOKEN)
        .output()
        .unwrap();
    assert_eq!(unconfirmed_retirement.status.code(), Some(2));
    assert!(
        String::from_utf8(unconfirmed_retirement.stderr)
            .unwrap()
            .contains("requires explicit --yes")
    );

    let retirement = Command::new(binary)
        .args([
            "--socket",
            socket.to_str().unwrap(),
            "runtime",
            "retire",
            "runtime-a",
            "--retirement-id",
            "retire-cli-1",
            "--provisioning-id",
            "provision-runtime-a",
            "--host",
            "runtime.example",
            "--credential-handle",
            "vault:ssh:runtime-retirement",
            "--yes",
        ])
        .env("LESERPENT_IPC_TOKEN", TOKEN)
        .env("LESERPENT_PRINCIPAL", "integration-test")
        .output()
        .unwrap();
    assert!(retirement.status.success());
    let retirement_output = String::from_utf8(retirement.stdout).unwrap();
    assert!(retirement_output.contains("retirement=retire-cli-1"));
    assert!(retirement_output.contains("runtime=runtime-a phase=planned"));
    assert!(!retirement_output.contains("runtime-retirement"));

    let bounded_retirement_wait = Command::new(binary)
        .args([
            "--socket",
            socket.to_str().unwrap(),
            "runtime",
            "retire",
            "runtime-a",
            "--retirement-id",
            "retire-cli-1",
            "--provisioning-id",
            "provision-runtime-a",
            "--host",
            "runtime.example",
            "--credential-handle",
            "vault:ssh:runtime-retirement",
            "--yes",
            "--wait",
            "--count",
            "1",
        ])
        .env("LESERPENT_IPC_TOKEN", TOKEN)
        .env("LESERPENT_PRINCIPAL", "integration-test")
        .output()
        .unwrap();
    assert_eq!(bounded_retirement_wait.status.code(), Some(5));
    assert!(
        String::from_utf8(bounded_retirement_wait.stderr)
            .unwrap()
            .contains("retirement retire-cli-1 did not reach a terminal phase")
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
        let mut runtime = ControlRuntime::open(&server_database).unwrap();
        support::seed_registered_runtime(
            &mut runtime,
            "provision-retirement-failed",
            "runtime-retirement-failed",
            "runtime.example",
        );
        support::seed_bound_deployment(&mut runtime, "bootstrap-retirement-failed");
        let mut registry = AdapterRegistry::default();
        registry.register(FailedProvisioningAdapter).unwrap();
        registry.register(FailedRetirementAdapter).unwrap();
        registry.register(FailedDaemonRetirementAdapter).unwrap();
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
            .with_provisioning_submission()
            .with_retirement_submission()
            .with_daemon_retirement_submission();
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

    let retirement = Command::new(env!("CARGO_BIN_EXE_leserpent"))
        .args([
            "--socket",
            socket.to_str().unwrap(),
            "runtime",
            "retire",
            "runtime-retirement-failed",
            "--retirement-id",
            "retire-failed-1",
            "--provisioning-id",
            "provision-retirement-failed",
            "--host",
            "runtime.example",
            "--credential-handle",
            "vault:ssh:secret-retirement-handle",
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
    assert_eq!(retirement.status.code(), Some(4));
    let stdout = String::from_utf8(retirement.stdout).unwrap();
    assert!(stdout.contains("phase=planned"));
    assert!(stdout.contains("phase=failed"));
    assert!(stdout.contains("fault=test_retirement_failed"));
    assert!(!stdout.contains("secret-retirement-handle"));

    let daemon_retirement = Command::new(env!("CARGO_BIN_EXE_leserpent"))
        .args([
            "--socket",
            socket.to_str().unwrap(),
            "bootstrap",
            "retire",
            "bootstrap-retirement-failed",
            "--retirement-id",
            "retire-daemon-failed-1",
            "--credential-handle",
            "vault:ssh:secret-daemon-retirement-handle",
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
    assert_eq!(daemon_retirement.status.code(), Some(4));
    let stdout = String::from_utf8(daemon_retirement.stdout).unwrap();
    assert!(stdout.contains("phase=planned"));
    assert!(stdout.contains("phase=failed"));
    assert!(stdout.contains("fault=test_daemon_retirement_failed"));
    assert!(!stdout.contains("secret-daemon-retirement-handle"));

    let inspect = Command::new(env!("CARGO_BIN_EXE_leserpent"))
        .args([
            "--socket",
            socket.to_str().unwrap(),
            "runtime",
            "inspect",
            "runtime-retirement-failed",
        ])
        .env("LESERPENT_IPC_TOKEN", TOKEN)
        .output()
        .unwrap();
    assert!(inspect.status.success());
    assert!(
        String::from_utf8(inspect.stdout)
            .unwrap()
            .contains("runtime=runtime-retirement-failed")
    );

    stop.store(true, Ordering::Release);
    server.join().unwrap();
    fs::remove_file(database).unwrap();
}

#[test]
fn native_cli_forwards_the_active_writer_ticket_to_specialized_routes() {
    let database = temp_path("wf.db");
    let socket = temp_path("wf.sock");
    let stop = Arc::new(AtomicBool::new(false));
    let (ready_tx, ready_rx) = mpsc::channel();
    let server_stop = Arc::clone(&stop);
    let server_database = database.clone();
    let server_socket = socket.clone();
    let writer_a = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let writer_b = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    let server = thread::spawn(move || {
        let mut runtime = ControlRuntime::open(&server_database).unwrap();
        assert_eq!(
            runtime.claim_authority_writer(writer_a).unwrap().generation,
            1
        );
        assert_eq!(
            runtime.claim_authority_writer(writer_b).unwrap().generation,
            2
        );
        let ipc = IpcServer::bind(&server_socket, TOKEN)
            .unwrap()
            .with_bootstrap_submission();
        ready_tx.send(()).unwrap();
        while !server_stop.load(Ordering::Acquire) {
            ipc.poll_once(&mut runtime).unwrap();
            thread::sleep(Duration::from_millis(1));
        }
    });
    ready_rx.recv_timeout(Duration::from_secs(2)).unwrap();

    let run = |writer_fence: Option<(&str, &str)>| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_leserpent"));
        command
            .args([
                "--socket",
                socket.to_str().unwrap(),
                "bootstrap",
                "deploy",
                "bootstrap-writer-fence",
                "--host",
                "host.example",
                "--credential-handle",
                "vault:ssh:host-example",
                "--yes",
            ])
            .env("LESERPENT_IPC_TOKEN", TOKEN)
            .env("LESERPENT_PRINCIPAL", "integration-test");
        if let Some((writer_id, generation)) = writer_fence {
            command
                .env("LESERPENT_AUTHORITY_WRITER_ID", writer_id)
                .env("LESERPENT_AUTHORITY_WRITER_GENERATION", generation);
        }
        command.output().unwrap()
    };

    let missing = run(None);
    assert_eq!(missing.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&missing.stderr).contains("authority_writer_fence_required"));
    let stale = run(Some((writer_a, "1")));
    assert_eq!(stale.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&stale.stderr).contains("authority_writer_fence_rejected"));
    let current = run(Some((writer_b, "2")));
    assert!(
        current.status.success(),
        "{}",
        String::from_utf8_lossy(&current.stderr)
    );
    assert!(
        String::from_utf8(current.stdout)
            .unwrap()
            .contains("bootstrap=bootstrap-writer-fence phase=planned")
    );

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
