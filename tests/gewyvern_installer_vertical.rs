use std::fs;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use gewyvern_install_contract::installer::{
    GewyvernInstallerRequest, GewyvernInstallerServiceState, decode_gewyvern_installer_response,
    encode_gewyvern_installer_request,
};
use leserpent_domain::RuntimeId;
use leserpent_domain::bootstrap::CredentialHandle;
use leserpent_domain::provisioning::ProvisioningId;
use ring::digest::{SHA256, digest};

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let root = fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "gewyvern-installer-vertical-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
}

impl Drop for TempDir {
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

fn invoke(binary: &str, install_root: &PathBuf, payload: &[u8]) -> std::process::Output {
    let mut child = Command::new(binary)
        .arg("gewyvern-install-v1")
        .env("GEWYVERN_INSTALL_ROOT", install_root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(payload).unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn native_installer_entrypoint_prepares_and_replays_a_private_generation() {
    let binary = env!("CARGO_BIN_EXE_gewyvern");
    let artifact = fs::read(binary).unwrap();
    let artifact_sha256 = hex(digest(&SHA256, &artifact).as_ref());
    let token = "0123456789abcdef0123456789abcdef";
    let request = GewyvernInstallerRequest::new(
        ProvisioningId::new("provision-vertical-1").unwrap(),
        RuntimeId::new("runtime-vertical-1").unwrap(),
        "https://runtime.example:9443",
        "test",
        artifact_sha256,
        CredentialHandle::new("vault:gewyvern:runtime-vertical-api").unwrap(),
        CredentialHandle::new("vault:gewyvern-ca:runtime-vertical-ca").unwrap(),
        token,
    )
    .unwrap();
    let payload = encode_gewyvern_installer_request(&request).unwrap();
    let temp = TempDir::new();
    let install_root = temp.0.join("install");

    let first = invoke(binary, &install_root, &payload);
    assert!(
        first.status.success(),
        "installer failed: {}",
        String::from_utf8_lossy(&first.stderr)
    );
    let first_response = decode_gewyvern_installer_response(&first.stdout).unwrap();
    assert_eq!(
        first_response.service_state,
        GewyvernInstallerServiceState::Installed
    );
    assert!(!first_response.replayed);
    assert!(!String::from_utf8_lossy(&first.stdout).contains(token));

    let generation = install_root
        .join("runtimes/runtime-vertical-1/generations")
        .join(&first_response.generation);
    assert_eq!(
        fs::read_to_string(generation.join("api.token")).unwrap(),
        token
    );
    assert_eq!(fs::read(generation.join("gewyvern")).unwrap(), artifact);
    #[cfg(unix)]
    assert_eq!(
        fs::metadata(generation.join("api.token"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );

    let replay = invoke(binary, &install_root, &payload);
    assert!(replay.status.success());
    let replay_response = decode_gewyvern_installer_response(&replay.stdout).unwrap();
    assert!(replay_response.replayed);
    assert_eq!(replay_response.generation, first_response.generation);
    assert_eq!(replay_response.tls_ca_sha256, first_response.tls_ca_sha256);
}

#[test]
fn native_installer_entrypoints_reject_extra_arguments_before_mutation() {
    let binary = env!("CARGO_BIN_EXE_gewyvern");
    let temp = TempDir::new();
    let install_root = temp.0.join("install");
    for entrypoint in [
        "gewyvern-install-v1",
        "gewyvern-activate-v1",
        "gewyvern-retire-v1",
        "gewyvern-service-v1",
    ] {
        let output = Command::new(binary)
            .args([entrypoint, "unexpected"])
            .env("GEWYVERN_INSTALL_ROOT", &install_root)
            .output()
            .unwrap();

        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("accepts no command-line arguments")
        );
    }
    assert!(!install_root.exists());
}
