use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use leserpent_adapters::{
    EffectAdapter, GewyvernDiscoveryAdapter, GewyvernTarget, HOST_BOOTSTRAP_EFFECT_KIND,
};
use leserpent_domain::RuntimeId;
use leserpent_domain::bootstrap::DeploymentBootstrap;
use leserpent_protocol::bootstrap::{
    BOOTSTRAP_PROTOCOL_SCHEMA_VERSION, BootstrapResponse, BootstrapResponseEnvelope,
    decode_bootstrap_request, encode_bootstrap_response,
};
use leserpent_protocol::{ProtocolResponse, decode_response};
use leserpent_runtime::{ControlRuntime, EffectExecution};
use leserpentd::{AdapterRegistry, DaemonConfig, DaemonHost, RemoteServer};
use rcgen::{CertifiedKey, generate_simple_self_signed};

const TOKEN: &str = "0123456789abcdef0123456789abcdef";

struct FailedBootstrapAdapter;

impl EffectAdapter for FailedBootstrapAdapter {
    fn kind(&self) -> &str {
        HOST_BOOTSTRAP_EFFECT_KIND
    }

    fn execute(&mut self, payload: &[u8]) -> EffectExecution {
        let request = decode_bootstrap_request(payload).unwrap();
        let mut bootstrap = DeploymentBootstrap::plan(
            &request.request.principal,
            &request.request.capabilities,
            request.request.intent,
        )
        .unwrap();
        bootstrap.begin().unwrap();
        let state = bootstrap.record_fault("test_failure").unwrap();
        EffectExecution::Complete(
            encode_bootstrap_response(&BootstrapResponseEnvelope {
                schema_version: BOOTSTRAP_PROTOCOL_SCHEMA_VERSION,
                response: BootstrapResponse::State(state),
            })
            .unwrap(),
        )
    }
}

#[test]
fn native_cli_preserves_command_semantics_over_authenticated_https() {
    let database = temp_path("sqlite");
    let certificate = temp_path("crt");
    let private_key = temp_path("key");
    let CertifiedKey { cert, signing_key } =
        generate_simple_self_signed(vec!["127.0.0.1".to_string()]).unwrap();
    fs::write(&certificate, cert.pem()).unwrap();
    fs::write(&private_key, signing_key.serialize_pem()).unwrap();
    #[cfg(unix)]
    fs::set_permissions(&private_key, fs::Permissions::from_mode(0o600)).unwrap();

    let capability_listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let capability_address = capability_listener.local_addr().unwrap();
    let capability_server = thread::spawn(move || {
        let (mut stream, _) = capability_listener.accept().unwrap();
        let mut request = [0_u8; 2048];
        let read = stream.read(&mut request).unwrap();
        let request = std::str::from_utf8(&request[..read]).unwrap();
        assert!(request.starts_with("GET /v1/capabilities HTTP/1.1\r\n"));
        let body = br#"{"service":"gewyvern-api","version":"1.2.0","latest_snapshot":true,"authenticated_deployment":false,"serve_required":true,"external_sidecar_context":true,"target_path_segment_encoding":"percent-encoding","target_direct_path_chars":"A-Z a-z 0-9 . _ ~ :","endpoints":["/v1/capabilities"],"protocol_catalog":true}"#;
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )
        .unwrap();
        stream.write_all(body).unwrap();
    });

    let stop = Arc::new(AtomicBool::new(false));
    let (ready_tx, ready_rx) = mpsc::channel();
    let server_stop = Arc::clone(&stop);
    let server_database = database.clone();
    let server_certificate = certificate.clone();
    let server_private_key = private_key.clone();
    let server = thread::spawn(move || {
        let mut runtime = ControlRuntime::open(&server_database).unwrap();
        runtime
            .register_runtime(
                RuntimeId::new("runtime-a").unwrap(),
                "Runtime A",
                "runtime://runtime-a",
            )
            .unwrap();
        let target = GewyvernTarget::loopback(capability_address, None).unwrap();
        let adapter = GewyvernDiscoveryAdapter::new([("runtime-a".to_string(), target)]).unwrap();
        let mut registry = AdapterRegistry::default();
        registry.register(adapter).unwrap();
        registry.register(FailedBootstrapAdapter).unwrap();
        let mut host = DaemonHost::new(
            runtime,
            registry,
            DaemonConfig {
                idle_interval: Duration::from_millis(1),
                ..DaemonConfig::default()
            },
        )
        .unwrap();
        let mut https = RemoteServer::bind(
            "127.0.0.1:0".parse().unwrap(),
            server_certificate,
            server_private_key,
            TOKEN,
        )
        .unwrap()
        .with_bootstrap_submission()
        .with_provisioning_submission();
        ready_tx.send(https.local_addr().unwrap()).unwrap();
        while !server_stop.load(Ordering::Acquire) {
            https.poll_once(host.runtime_mut()).unwrap();
            host.tick().unwrap();
            thread::sleep(Duration::from_millis(1));
        }
    });
    let address = ready_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let endpoint = format!("https://{address}");
    let binary = env!("CARGO_BIN_EXE_leserpent");

    let health = remote_command(binary, &endpoint, &certificate)
        .arg("health")
        .output()
        .unwrap();
    assert!(health.status.success(), "{}", stderr(&health));
    assert!(
        String::from_utf8(health.stdout)
            .unwrap()
            .contains("status=ready authority_owned=true")
    );

    let bootstrap = remote_command(binary, &endpoint, &certificate)
        .args([
            "bootstrap",
            "deploy",
            "bootstrap-https-1",
            "--host",
            "host.example",
            "--credential-handle",
            "vault:ssh:host-example",
            "--yes",
        ])
        .env("LESERPENT_PRINCIPAL", "remote-integration-test")
        .output()
        .unwrap();
    assert!(bootstrap.status.success(), "{}", stderr(&bootstrap));
    assert!(
        String::from_utf8(bootstrap.stdout)
            .unwrap()
            .contains("bootstrap=bootstrap-https-1 phase=planned")
    );

    let provisioning = remote_command(binary, &endpoint, &certificate)
        .args([
            "runtime",
            "provision",
            "runtime-https-new",
            "--provisioning-id",
            "provision-https-1",
            "--host",
            "runtime.example",
            "--credential-handle",
            "vault:ssh:runtime-example",
            "--yes",
        ])
        .env("LESERPENT_PRINCIPAL", "remote-integration-test")
        .output()
        .unwrap();
    assert!(provisioning.status.success(), "{}", stderr(&provisioning));
    let provisioning_output = String::from_utf8(provisioning.stdout).unwrap();
    assert!(provisioning_output.contains("provisioning=provision-https-1"));
    assert!(provisioning_output.contains("runtime=runtime-https-new phase=planned"));
    assert!(!provisioning_output.contains("runtime-example"));
    let bootstrap_deadline = std::time::Instant::now() + Duration::from_secs(3);
    loop {
        let inspect = remote_command(binary, &endpoint, &certificate)
            .args(["bootstrap", "inspect", "bootstrap-https-1"])
            .output()
            .unwrap();
        assert!(inspect.status.success(), "{}", stderr(&inspect));
        let output = String::from_utf8(inspect.stdout).unwrap();
        if output.contains("phase=failed") {
            break;
        }
        assert!(
            std::time::Instant::now() < bootstrap_deadline,
            "bootstrap effect did not settle: {output}"
        );
        thread::sleep(Duration::from_millis(10));
    }

    let list = remote_command(binary, &endpoint, &certificate)
        .args(["--json", "runtime", "list"])
        .env("LESERPENT_PRINCIPAL", "remote-integration-test")
        .output()
        .unwrap();
    assert!(list.status.success(), "{}", stderr(&list));
    let response = decode_response(trim_ascii_whitespace(&list.stdout)).unwrap();
    assert!(matches!(
        response.response,
        ProtocolResponse::Query(leserpent_domain::QueryResult::RuntimeList { ref runtimes, .. })
            if runtimes.len() == 1 && runtimes[0].id.as_str() == "runtime-a"
    ));

    let refresh_capabilities = remote_command(binary, &endpoint, &certificate)
        .args([
            "--json",
            "runtime",
            "refresh-capabilities",
            "runtime-a",
            "--yes",
            "--expected-revision",
            "1",
            "--idempotency-key",
            "remote-integration-capabilities",
        ])
        .output()
        .unwrap();
    assert!(
        refresh_capabilities.status.success(),
        "{}",
        stderr(&refresh_capabilities)
    );
    let refreshed = decode_response(trim_ascii_whitespace(&refresh_capabilities.stdout)).unwrap();
    assert!(matches!(
        refreshed.response,
        ProtocolResponse::Command(ref result)
            if result.status == leserpent_domain::CommandStatus::Applied
                && result.runtime.revision.0 == 2
    ));

    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    let observed = loop {
        let inspect = remote_command(binary, &endpoint, &certificate)
            .args(["runtime", "inspect", "runtime-a"])
            .output()
            .unwrap();
        assert!(inspect.status.success(), "{}", stderr(&inspect));
        let output = String::from_utf8(inspect.stdout).unwrap();
        if output.contains("capabilities=observed") {
            break output;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "capability discovery did not complete: {output}"
        );
        thread::sleep(Duration::from_millis(10));
    };
    assert!(observed.contains("revision=3"));
    assert!(observed.contains("capabilities_observed_for_revision=2"));
    assert!(observed.contains("service=gewyvern-api version=1.2.0"));
    assert!(observed.contains("capability_endpoints=/v1/capabilities"));
    assert!(observed.contains("capability_extensions=protocol_catalog=true"));
    assert!(!observed.contains(&capability_address.to_string()));
    assert!(!observed.contains("Authorization"));

    let watch = remote_command(binary, &endpoint, &certificate)
        .args([
            "runtime",
            "watch",
            "runtime-a",
            "--count",
            "2",
            "--interval-ms",
            "50",
        ])
        .output()
        .unwrap();
    assert!(watch.status.success(), "{}", stderr(&watch));
    let watch_stdout = String::from_utf8(watch.stdout).unwrap();
    assert_eq!(watch_stdout.lines().count(), 4);
    assert!(watch_stdout.contains("capabilities=observed"));
    assert!(watch_stdout.contains("capabilities_observed_for_revision=2"));

    let apply = remote_command(binary, &endpoint, &certificate)
        .args([
            "--json",
            "runtime",
            "refresh",
            "runtime-a",
            "--yes",
            "--expected-revision",
            "3",
            "--idempotency-key",
            "remote-integration-refresh",
        ])
        .output()
        .unwrap();
    assert!(apply.status.success(), "{}", stderr(&apply));
    let applied = decode_response(trim_ascii_whitespace(&apply.stdout)).unwrap();
    assert!(matches!(
        applied.response,
        ProtocolResponse::Command(ref result)
            if result.status == leserpent_domain::CommandStatus::Applied
                && result.runtime.revision.0 == 4
    ));

    let unauthorized = Command::new(binary)
        .args([
            "--remote",
            &endpoint,
            "--remote-ca",
            certificate.to_str().unwrap(),
            "health",
        ])
        .env("LESERPENT_REMOTE_TOKEN", "fedcba9876543210fedcba9876543210")
        .output()
        .unwrap();
    assert_eq!(unauthorized.status.code(), Some(3));
    assert!(stderr(&unauthorized).contains("unauthorized"));

    let unauthorized_bootstrap = Command::new(binary)
        .args([
            "--remote",
            &endpoint,
            "--remote-ca",
            certificate.to_str().unwrap(),
            "bootstrap",
            "deploy",
            "bootstrap-unauthorized",
            "--host",
            "host.example",
            "--credential-handle",
            "vault:ssh:host-example",
            "--yes",
        ])
        .env("LESERPENT_REMOTE_TOKEN", "fedcba9876543210fedcba9876543210")
        .output()
        .unwrap();
    assert_eq!(unauthorized_bootstrap.status.code(), Some(3));
    assert!(stderr(&unauthorized_bootstrap).contains("unauthorized"));

    stop.store(true, Ordering::Release);
    server.join().unwrap();
    capability_server.join().unwrap();
    fs::remove_file(database).unwrap();
    fs::remove_file(certificate).unwrap();
    fs::remove_file(private_key).unwrap();
}

#[test]
fn native_cli_submits_provisioning_bound_retirement_over_authenticated_https() {
    let database = temp_path("retirement.sqlite");
    let certificate = temp_path("retirement.crt");
    let private_key = temp_path("retirement.key");
    let CertifiedKey { cert, signing_key } =
        generate_simple_self_signed(vec!["127.0.0.1".to_string()]).unwrap();
    fs::write(&certificate, cert.pem()).unwrap();
    fs::write(&private_key, signing_key.serialize_pem()).unwrap();
    #[cfg(unix)]
    fs::set_permissions(&private_key, fs::Permissions::from_mode(0o600)).unwrap();

    let stop = Arc::new(AtomicBool::new(false));
    let (ready_tx, ready_rx) = mpsc::channel();
    let server_stop = Arc::clone(&stop);
    let server_database = database.clone();
    let server_certificate = certificate.clone();
    let server_private_key = private_key.clone();
    let server = thread::spawn(move || {
        let mut runtime = ControlRuntime::open(&server_database).unwrap();
        support::seed_registered_runtime(
            &mut runtime,
            "provision-https-retire",
            "runtime-https-retire",
            "runtime.example",
        );
        support::seed_bound_deployment(&mut runtime, "bootstrap-https-retire");
        let mut https = RemoteServer::bind(
            "127.0.0.1:0".parse().unwrap(),
            server_certificate,
            server_private_key,
            TOKEN,
        )
        .unwrap()
        .with_retirement_submission()
        .with_daemon_retirement_submission();
        ready_tx.send(https.local_addr().unwrap()).unwrap();
        while !server_stop.load(Ordering::Acquire) {
            https.poll_once(&mut runtime).unwrap();
            thread::sleep(Duration::from_millis(1));
        }
    });
    let address = ready_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let endpoint = format!("https://{address}");
    let binary = env!("CARGO_BIN_EXE_leserpent");

    let retirement = remote_command(binary, &endpoint, &certificate)
        .args([
            "runtime",
            "retire",
            "runtime-https-retire",
            "--retirement-id",
            "retire-https-1",
            "--provisioning-id",
            "provision-https-retire",
            "--host",
            "runtime.example",
            "--credential-handle",
            "vault:ssh:https-retirement-secret",
            "--yes",
        ])
        .env("LESERPENT_PRINCIPAL", "integration-test")
        .output()
        .unwrap();
    assert!(retirement.status.success(), "{}", stderr(&retirement));
    let output = String::from_utf8(retirement.stdout).unwrap();
    assert!(output.contains("retirement=retire-https-1"));
    assert!(output.contains("runtime=runtime-https-retire phase=planned"));
    assert!(!output.contains("https-retirement-secret"));

    let daemon_retirement = remote_command(binary, &endpoint, &certificate)
        .args([
            "bootstrap",
            "retire",
            "bootstrap-https-retire",
            "--retirement-id",
            "retire-daemon-https-1",
            "--credential-handle",
            "vault:ssh:https-daemon-retirement-secret",
            "--yes",
        ])
        .env("LESERPENT_PRINCIPAL", "integration-test")
        .output()
        .unwrap();
    assert!(
        daemon_retirement.status.success(),
        "{}",
        stderr(&daemon_retirement)
    );
    let output = String::from_utf8(daemon_retirement.stdout).unwrap();
    assert!(output.contains("daemon_retirement=retire-daemon-https-1"));
    assert!(output.contains("bootstrap=bootstrap-https-retire"));
    assert!(output.contains("phase=planned"));
    assert!(!output.contains("https-daemon-retirement-secret"));

    let unauthorized = Command::new(binary)
        .args([
            "--remote",
            &endpoint,
            "--remote-ca",
            certificate.to_str().unwrap(),
            "runtime",
            "retire",
            "runtime-https-retire",
            "--retirement-id",
            "retire-https-unauthorized",
            "--provisioning-id",
            "provision-https-retire",
            "--host",
            "runtime.example",
            "--credential-handle",
            "vault:ssh:https-retirement-secret",
            "--yes",
        ])
        .env("LESERPENT_REMOTE_TOKEN", "fedcba9876543210fedcba9876543210")
        .env("LESERPENT_PRINCIPAL", "integration-test")
        .output()
        .unwrap();
    assert_eq!(unauthorized.status.code(), Some(3));
    assert!(stderr(&unauthorized).contains("unauthorized"));

    let unauthorized_daemon_retirement = Command::new(binary)
        .args([
            "--remote",
            &endpoint,
            "--remote-ca",
            certificate.to_str().unwrap(),
            "bootstrap",
            "retire",
            "bootstrap-https-retire",
            "--retirement-id",
            "retire-daemon-https-unauthorized",
            "--credential-handle",
            "vault:ssh:https-daemon-retirement-secret",
            "--yes",
        ])
        .env("LESERPENT_REMOTE_TOKEN", "fedcba9876543210fedcba9876543210")
        .env("LESERPENT_PRINCIPAL", "integration-test")
        .output()
        .unwrap();
    assert_eq!(unauthorized_daemon_retirement.status.code(), Some(3));
    assert!(stderr(&unauthorized_daemon_retirement).contains("unauthorized"));

    stop.store(true, Ordering::Release);
    server.join().unwrap();
    fs::remove_file(database).unwrap();
    fs::remove_file(certificate).unwrap();
    fs::remove_file(private_key).unwrap();
}

fn remote_command(binary: &str, endpoint: &str, certificate: &Path) -> Command {
    let mut command = Command::new(binary);
    command
        .args([
            "--remote",
            endpoint,
            "--remote-ca",
            certificate.to_str().unwrap(),
        ])
        .env("LESERPENT_REMOTE_TOKEN", TOKEN);
    command
}

fn temp_path(extension: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "leserpent-cli-https-{}-{unique}.{extension}",
        std::process::id()
    ))
}

fn trim_ascii_whitespace(mut value: &[u8]) -> &[u8] {
    while value.last().is_some_and(u8::is_ascii_whitespace) {
        value = &value[..value.len() - 1];
    }
    value
}

fn stderr(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}
mod support;
