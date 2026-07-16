use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use leserpent_domain::RuntimeId;
use leserpent_protocol::{ProtocolResponse, decode_response};
use leserpent_runtime::ControlRuntime;
use leserpentd::RemoteServer;
use rcgen::{CertifiedKey, generate_simple_self_signed};

const TOKEN: &str = "0123456789abcdef0123456789abcdef";

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
        let mut https = RemoteServer::bind(
            "127.0.0.1:0".parse().unwrap(),
            server_certificate,
            server_private_key,
            TOKEN,
        )
        .unwrap();
        ready_tx.send(https.local_addr().unwrap()).unwrap();
        while !server_stop.load(Ordering::Acquire) {
            https.poll_once(&mut runtime).unwrap();
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
    assert_eq!(String::from_utf8(watch.stdout).unwrap().lines().count(), 1);

    let apply = remote_command(binary, &endpoint, &certificate)
        .args([
            "--json",
            "runtime",
            "refresh",
            "runtime-a",
            "--yes",
            "--expected-revision",
            "1",
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
                && result.runtime.revision.0 == 2
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
