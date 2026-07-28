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

use leserpent_adapters::{GewyvernDiscoveryAdapter, GewyvernTarget};
use leserpent_domain::{RuntimeId, RuntimeLogLevel};
use leserpent_runtime::ControlRuntime;
use leserpentd::{AdapterRegistry, DaemonConfig, DaemonHost, RemoteServer};
use rcgen::{CertifiedKey, generate_simple_self_signed};

const TOKEN: &str = "0123456789abcdef0123456789abcdef";

#[test]
#[ignore = "requires the locked .NET SDK used by the named parity shelf"]
fn dotnet_remote_client_refreshes_and_inspects_workspace() {
    let database = temp_path("sqlite");
    let certificate = temp_path("crt");
    let private_key = temp_path("key");
    let cache = temp_path("json");
    let CertifiedKey { cert, signing_key } =
        generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
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
                "https://secret-runtime.invalid",
            )
            .unwrap();
        runtime
            .append_runtime_log(
                &RuntimeId::new("runtime-a").unwrap(),
                RuntimeLogLevel::Warning,
                "bounded warning\ncontinued",
            )
            .unwrap();
        let target = GewyvernTarget::loopback(capability_address, None).unwrap();
        let adapter = GewyvernDiscoveryAdapter::new([("runtime-a".to_string(), target)]).unwrap();
        let mut registry = AdapterRegistry::default();
        registry.register(adapter).unwrap();
        let mut host = DaemonHost::new(
            runtime,
            registry,
            DaemonConfig {
                idle_interval: Duration::from_millis(1),
                ..DaemonConfig::default()
            },
        )
        .unwrap();
        let mut remote = RemoteServer::bind(
            "127.0.0.1:0".parse().unwrap(),
            server_certificate,
            server_private_key,
            TOKEN,
        )
        .unwrap();
        ready_tx.send(remote.local_addr().unwrap()).unwrap();
        while !server_stop.load(Ordering::Acquire) {
            remote.poll_once(host.runtime_mut()).unwrap();
            host.tick().unwrap();
            thread::sleep(Duration::from_millis(1));
        }
    });
    let address = ready_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let endpoint = format!("https://localhost:{}", address.port());
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let artifacts = TestDotnetArtifacts::new();
    let refresh_output = remote_conformance_command(&root, &artifacts)
        .args(["--", "--connect", &endpoint])
        .arg(&certificate)
        .arg(&cache)
        .args(["--refresh", "runtime-a"])
        .env("LESERPENT_REMOTE_TOKEN", TOKEN)
        .output()
        .unwrap();
    assert!(
        refresh_output.status.success(),
        "refresh stdout:\n{}\nrefresh stderr:\n{}",
        String::from_utf8_lossy(&refresh_output.stdout),
        String::from_utf8_lossy(&refresh_output.stderr),
    );

    let capability_output = remote_conformance_command(&root, &artifacts)
        .args(["--", "--connect", &endpoint])
        .arg(&certificate)
        .arg(&cache)
        .args(["--refresh-capabilities", "runtime-a"])
        .env("LESERPENT_REMOTE_TOKEN", TOKEN)
        .output()
        .unwrap();
    assert!(
        capability_output.status.success(),
        "capability stdout:\n{}\ncapability stderr:\n{}",
        String::from_utf8_lossy(&capability_output.stdout),
        String::from_utf8_lossy(&capability_output.stderr),
    );

    let inspect_output = remote_conformance_command(&root, &artifacts)
        .args(["--", "--connect", &endpoint])
        .arg(&certificate)
        .arg(&cache)
        .args(["--inspect", "runtime-a"])
        .env("LESERPENT_REMOTE_TOKEN", TOKEN)
        .output()
        .unwrap();

    stop.store(true, Ordering::Release);
    server.join().unwrap();
    capability_server.join().unwrap();
    assert!(
        inspect_output.status.success(),
        "inspect stdout:\n{}\ninspect stderr:\n{}",
        String::from_utf8_lossy(&inspect_output.stdout),
        String::from_utf8_lossy(&inspect_output.stderr),
    );
    let refresh_stdout = String::from_utf8(refresh_output.stdout).unwrap();
    let capability_stdout = String::from_utf8(capability_output.stdout).unwrap();
    assert!(
        refresh_stdout.contains("remote conformance valid: revision=1, runtimes=1, stale=false")
    );
    assert!(refresh_stdout.contains(
        "remote mutation conformance valid: kind=runtime_refresh, initial_revision=1, applied_revision=2, event_revision=2, capabilities_observed=false, capabilities_observed_for_revision=none, runtime=runtime-a, stale=false"
    ));
    assert!(capability_stdout.contains(
        "remote mutation conformance valid: kind=runtime_capabilities_refresh, initial_revision=2, applied_revision=3, event_revision=4, capabilities_observed=true, capabilities_observed_for_revision=3, runtime=runtime-a, stale=false"
    ));
    let inspect_stdout = String::from_utf8(inspect_output.stdout).unwrap();
    assert!(
        inspect_stdout.contains("remote conformance valid: revision=4, runtimes=1, stale=false")
    );
    assert!(inspect_stdout.contains(
        "remote workspace conformance valid: revision=4, runtime=runtime-a, history=2, logs=1, endpoint_retained=false"
    ));
    assert!(!refresh_stdout.contains("secret-runtime.invalid"));
    assert!(!capability_stdout.contains("secret-runtime.invalid"));
    assert!(!inspect_stdout.contains("secret-runtime.invalid"));
    assert!(inspect_stdout.contains("capabilities_observed=true"));
    assert!(inspect_stdout.contains("capabilities_observed_for_revision=3"));
    assert!(inspect_stdout.contains("capability_version=1.2.0"));
    let cached = fs::read_to_string(&cache).unwrap();
    assert!(cached.contains("runtime-a"));
    assert!(cached.contains("\"revision\":4"));
    assert!(cached.contains("\"capabilities_observed_for_revision\":3"));
    assert!(cached.contains("\"version\":\"1.2.0\""));
    assert!(!cached.contains("secret-runtime.invalid"));
    #[cfg(unix)]
    assert_eq!(
        fs::metadata(&cache).unwrap().permissions().mode() & 0o077,
        0
    );

    fs::remove_file(database).unwrap();
    fs::remove_file(certificate).unwrap();
    fs::remove_file(private_key).unwrap();
    fs::remove_file(cache).unwrap();
}

struct TestDotnetArtifacts {
    path: PathBuf,
}

impl TestDotnetArtifacts {
    fn new() -> Self {
        Self {
            path: temp_path("dotnet-artifacts"),
        }
    }
}

impl Drop for TestDotnetArtifacts {
    fn drop(&mut self) {
        if self.path.exists() {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

fn remote_conformance_command(root: &Path, artifacts: &TestDotnetArtifacts) -> Command {
    let mut command = Command::new("dotnet");
    command
        .current_dir(root)
        .args(["run", "--project"])
        .arg(root.join(
            "apps/leserpent-avalonia/src/Leserpent.RemoteConformance/Leserpent.RemoteConformance.csproj",
        ))
        .args(["--configuration", "Release", "--artifacts-path"])
        .arg(&artifacts.path);
    command
}

fn temp_path(extension: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "leserpent-dotnet-remote-{}-{unique}.{extension}",
        std::process::id()
    ))
}
