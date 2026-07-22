#![cfg(unix)]

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use leserpent_domain::bootstrap::{BootstrapId, DaemonId};
use leserpent_protocol::bootstrap_installer::{
    BootstrapInstallerRequest, BootstrapInstallerServiceState, decode_bootstrap_installer_response,
    encode_bootstrap_installer_request,
};
use ring::digest::{SHA256, digest};

struct TempHome(PathBuf);

impl TempHome {
    fn new() -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "leserpent-bootstrap-process-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(fs::canonicalize(path).unwrap())
    }
}

impl Drop for TempHome {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

fn run_installer(binary: &Path, home: &Path, request: &BootstrapInstallerRequest) -> Vec<u8> {
    let mut child = Command::new(binary)
        .arg("bootstrap-install-v1")
        .env_clear()
        .env("HOME", home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&encode_bootstrap_installer_request(request).unwrap())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "installer failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

#[test]
fn native_installer_entrypoint_commits_and_replays_a_private_generation() {
    let binary = PathBuf::from(env!("CARGO_BIN_EXE_leserpentd"));
    let binary_bytes = fs::read(&binary).unwrap();
    let home = TempHome::new();
    let token = "0123456789abcdef0123456789abcdef";
    let request = BootstrapInstallerRequest::new(
        BootstrapId::new("bootstrap-process-1").unwrap(),
        DaemonId::new("daemon-process-1").unwrap(),
        "https://host.example:7443",
        "user",
        hex(digest(&SHA256, &binary_bytes).as_ref()),
        token,
    )
    .unwrap();

    let first_bytes = run_installer(&binary, &home.0, &request);
    assert!(
        !first_bytes
            .windows(token.len())
            .any(|window| window == token.as_bytes())
    );
    let first = decode_bootstrap_installer_response(&first_bytes).unwrap();
    assert_eq!(
        first.service_state,
        BootstrapInstallerServiceState::Installed
    );
    assert!(!first.replayed);
    assert!(
        first
            .tls_ca_pem
            .starts_with("-----BEGIN CERTIFICATE-----\n")
    );
    assert_eq!(
        first.tls_ca_sha256,
        hex(digest(&SHA256, first.tls_ca_pem.as_bytes()).as_ref())
    );

    #[cfg(target_os = "macos")]
    let root = home
        .0
        .join("Library/Application Support/Leserpent/bootstrap");
    #[cfg(target_os = "linux")]
    let root = home.0.join(".local/share/leserpent/bootstrap");
    assert_eq!(
        fs::read_to_string(root.join("current")).unwrap(),
        format!("{}\n", first.generation)
    );
    let generation = root.join("generations").join(&first.generation);
    assert!(generation.join("server.crt").is_file());
    assert!(generation.join("server.key").is_file());
    #[cfg(target_os = "macos")]
    let descriptor = generation.join("service.plist");
    #[cfg(target_os = "linux")]
    let descriptor = generation.join("service.service");
    let descriptor = fs::read_to_string(descriptor).unwrap();
    assert!(descriptor.contains("--remote-token-file"));
    assert!(!descriptor.contains(token));
    #[cfg(target_os = "macos")]
    let published = home
        .0
        .join("Library/LaunchAgents/org.gewyvern.leserpentd.daemon-process-1.plist");
    #[cfg(target_os = "linux")]
    let published = home
        .0
        .join(".config/systemd/user/leserpentd-daemon-process-1.service");
    assert_eq!(fs::read_to_string(published).unwrap(), descriptor);

    let replay =
        decode_bootstrap_installer_response(&run_installer(&binary, &home.0, &request)).unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.generation, first.generation);
}
