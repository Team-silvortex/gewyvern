use std::fs;
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use gewyvern::project_status::{StatusCatalog, StatusCatalogLoadError};

struct Sandbox {
    root: PathBuf,
}

impl Sandbox {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock must be after the Unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "gewyvern-machine-error-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("sandbox root must be created");
        Self { root }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    fn runtime_roots(&self) -> [PathBuf; 4] {
        [
            self.path("config"),
            self.path("data"),
            self.path("state"),
            self.path("cache"),
        ]
    }

    fn gewyvern_command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_gewyvern"));
        for key in [
            "GEWY_CONFIG_FILE",
            "GEWY_CERTIFICATE_ROOT",
            "GEWY_TRUST_ROOT",
            "GEWY_AUTHORITY_ROOT",
            "GEWY_IDENTITY_ROOT",
            "GEWY_CERTIFICATE_STATE_ROOT",
            "GEWY_PROTOCOL_REGISTRY_ROOT",
            "GEWY_SHARE_ROOT",
        ] {
            command.env_remove(key);
        }
        command
            .env("HOME", self.path("home"))
            .env("USERPROFILE", self.path("home"))
            .env("GEWY_CONFIG_HOME", self.path("config"))
            .env("GEWY_DATA_HOME", self.path("data"))
            .env("GEWY_STATE_HOME", self.path("state"))
            .env("GEWY_CACHE_HOME", self.path("cache"));
        command
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn assert_machine_error(
    output: &Output,
    expected_code: &str,
    expected_category: &str,
    expected_retryable: bool,
    expected_exit_code: i64,
) {
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    let payload: serde_json::Value = serde_json::from_str(stderr.trim())
        .unwrap_or_else(|error| panic!("stderr must be one JSON document: {error}\n{stderr}"));
    assert_eq!(payload["schema_version"], 1);
    assert_eq!(payload["ok"], false);
    assert_eq!(payload["error"]["code"], expected_code);
    assert_eq!(payload["error"]["category"], expected_category);
    assert_eq!(payload["error"]["retryable"], expected_retryable);
    assert_eq!(payload["error"]["exit_code"], expected_exit_code);
    assert!(
        payload["error"]["message"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
    );
}

fn assert_runtime_roots_absent(sandbox: &Sandbox) {
    for root in sandbox.runtime_roots() {
        assert!(
            !root.exists(),
            "preflight-only command unexpectedly created '{}'",
            root.display()
        );
    }
}

#[test]
fn help_is_side_effect_free_before_runtime_bootstrap() {
    let sandbox = Sandbox::new("help");
    let output = sandbox
        .gewyvern_command()
        .arg("--help")
        .output()
        .expect("gewyvern help must run");

    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("usage:"));
    assert_runtime_roots_absent(&sandbox);
}

#[test]
fn invalid_cli_is_machine_readable_and_does_not_prepare_runtime_roots() {
    let sandbox = Sandbox::new("invalid-cli");
    let output = sandbox
        .gewyvern_command()
        .args(["--json", "--definitely-invalid"])
        .output()
        .expect("invalid gewyvern command must run");

    assert_machine_error(&output, "cli_invalid", "input", false, 2);
    assert_runtime_roots_absent(&sandbox);
}

#[test]
fn invalid_certificate_command_is_rejected_before_runtime_layout_side_effects() {
    let sandbox = Sandbox::new("invalid-certificate-cli");
    let output = sandbox
        .gewyvern_command()
        .args(["certificate-state", "unknown", "--json"])
        .output()
        .expect("invalid certificate command must run");

    assert_machine_error(&output, "certificate_state_cli_invalid", "input", false, 2);
    assert_runtime_roots_absent(&sandbox);
}

#[test]
fn output_failure_is_one_machine_readable_error_document() {
    let sandbox = Sandbox::new("output-failure");
    let output_directory = sandbox.path("not-a-file");
    fs::create_dir_all(&output_directory).expect("output directory must exist");
    let output = sandbox
        .gewyvern_command()
        .args(["--list-protocols", "--json", "--out"])
        .arg(&output_directory)
        .output()
        .expect("gewyvern output failure must run");

    assert_machine_error(&output, "output_write_failed", "io", true, 1);
}

#[cfg(unix)]
#[test]
fn output_replaces_a_symlink_without_touching_its_target() {
    use std::os::unix::fs::symlink;

    let sandbox = Sandbox::new("output-symlink");
    let outside = sandbox.path("outside.json");
    let output_path = sandbox.path("protocols.json");
    fs::write(&outside, "outside\n").expect("outside file must be writable");
    symlink(&outside, &output_path).expect("output symlink must be created");

    let output = sandbox
        .gewyvern_command()
        .args(["--list-protocols", "--json", "--out"])
        .arg(&output_path)
        .output()
        .expect("gewyvern output command must run");

    assert!(output.status.success());
    assert_eq!(
        fs::read_to_string(&outside).expect("outside file must remain readable"),
        "outside\n"
    );
    assert!(
        !fs::symlink_metadata(&output_path)
            .expect("output must exist")
            .file_type()
            .is_symlink()
    );
    let document: serde_json::Value = serde_json::from_slice(
        &fs::read(&output_path).expect("output must contain the protocol catalog"),
    )
    .expect("output must remain valid JSON");
    let protocols = document
        .as_array()
        .expect("protocol catalog output must be an array");
    assert!(!protocols.is_empty());
    assert!(
        protocols
            .iter()
            .all(|protocol| protocol["protocol"].is_string())
    );
}

#[test]
fn service_listener_failure_returns_to_the_machine_error_boundary() {
    let sandbox = Sandbox::new("service-bind");
    let occupied = TcpListener::bind("127.0.0.1:0").expect("test listener must bind");
    let address = occupied
        .local_addr()
        .expect("test listener address must resolve")
        .to_string();
    let output = sandbox
        .gewyvern_command()
        .args([
            "--serve",
            "--tcp-socket",
            &address,
            "--max-sessions",
            "1",
            "--json",
        ])
        .output()
        .expect("service bind failure must run");

    assert_machine_error(
        &output,
        "socket_listener_bind_failed",
        "environment",
        true,
        1,
    );
}

#[test]
fn status_cli_reports_catalog_read_failure_with_the_shared_contract() {
    let sandbox = Sandbox::new("status-read");
    let output = Command::new(env!("CARGO_BIN_EXE_gewyvern_status"))
        .args(["validate", "--json", "--catalog"])
        .arg(sandbox.path("missing-status.json"))
        .output()
        .expect("status command must run");

    assert_machine_error(&output, "status_catalog_read_failed", "io", true, 1);
}

#[test]
fn typed_status_loader_distinguishes_retryable_io_from_invalid_content() {
    let sandbox = Sandbox::new("typed-status");
    let missing_path = sandbox.path("missing.json");
    let read_error = StatusCatalog::load_typed(&missing_path)
        .expect_err("missing status catalog must be rejected");
    assert!(matches!(&read_error, StatusCatalogLoadError::Read { .. }));
    assert_eq!(read_error.code(), "status_catalog_read_failed");
    assert_eq!(
        read_error.category(),
        gewyvern::machine_error::ErrorCategory::Io
    );
    assert!(read_error.retryable());

    let malformed_path = sandbox.path("malformed.json");
    fs::write(&malformed_path, "{not-json").expect("malformed catalog must be written");
    let decode_error = StatusCatalog::load_typed(&malformed_path)
        .expect_err("malformed status catalog must be rejected");
    assert!(matches!(
        &decode_error,
        StatusCatalogLoadError::Decode { .. }
    ));
    assert_eq!(decode_error.code(), "status_catalog_decode_failed");
    assert_eq!(
        decode_error.category(),
        gewyvern::machine_error::ErrorCategory::Configuration
    );
    assert!(!decode_error.retryable());
    let machine_error = gewyvern::machine_error::MachineError::from(decode_error);
    assert_eq!(machine_error.code, "status_catalog_decode_failed");
    assert_eq!(
        machine_error.category,
        gewyvern::machine_error::ErrorCategory::Configuration
    );
}

#[test]
fn typed_status_loader_rejects_oversized_catalogs_before_decoding() {
    let sandbox = Sandbox::new("typed-status-oversized");
    let oversized_path = sandbox.path("oversized.json");
    let oversized = fs::File::create(&oversized_path).expect("oversized catalog must be created");
    oversized
        .set_len(gewyvern::project_status::MAX_STATUS_CATALOG_BYTES + 1)
        .expect("oversized catalog length must be set");

    let error = StatusCatalog::load_typed(&oversized_path)
        .expect_err("oversized status catalog must be rejected");
    assert!(matches!(&error, StatusCatalogLoadError::Read { .. }));
    assert_eq!(error.code(), "status_catalog_read_failed");
}
