use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use leserpent_domain::RuntimeId;
use leserpent_runtime::ControlRuntime;
use leserpentd::RemoteServer;
use rcgen::{CertifiedKey, generate_simple_self_signed};

const TOKEN: &str = "0123456789abcdef0123456789abcdef";

#[test]
#[ignore = "requires the locked .NET SDK used by the named parity shelf"]
fn dotnet_remote_client_consumes_snapshot_and_applies_refresh() {
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
        let mut remote = RemoteServer::bind(
            "127.0.0.1:0".parse().unwrap(),
            server_certificate,
            server_private_key,
            TOKEN,
        )
        .unwrap();
        ready_tx.send(remote.local_addr().unwrap()).unwrap();
        while !server_stop.load(Ordering::Acquire) {
            remote.poll_once(&mut runtime).unwrap();
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
    let project = root.join(
        "apps/leserpent-avalonia/src/Leserpent.RemoteConformance/Leserpent.RemoteConformance.csproj",
    );
    let output = Command::new("dotnet")
        .current_dir(&root)
        .args(["run", "--project"])
        .arg(project)
        .args(["--configuration", "Release", "--", "--connect", &endpoint])
        .arg(&certificate)
        .arg(&cache)
        .args(["--refresh", "runtime-a"])
        .env("LESERPENT_REMOTE_TOKEN", TOKEN)
        .output()
        .unwrap();

    stop.store(true, Ordering::Release);
    server.join().unwrap();
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("remote conformance valid: revision=1, runtimes=1, stale=false"));
    assert!(stdout.contains(
        "remote mutation conformance valid: initial_revision=1, applied_revision=2, event_revision=2, runtime=runtime-a, stale=false"
    ));
    let cached = fs::read_to_string(&cache).unwrap();
    assert!(cached.contains("runtime-a"));
    assert!(cached.contains("\"revision\":2"));
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
