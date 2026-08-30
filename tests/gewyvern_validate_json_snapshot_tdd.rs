use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

static RELEASE_GATE_EVIDENCE_LOCK: Mutex<()> = Mutex::new(());

struct ReleaseGateEvidenceGuard {
    _lock: MutexGuard<'static, ()>,
    files: Vec<(PathBuf, Option<Vec<u8>>)>,
}

impl ReleaseGateEvidenceGuard {
    fn acquire() -> Self {
        let lock = RELEASE_GATE_EVIDENCE_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("validation");
        let paths = [
            root.join("release-gate-artifacts.json"),
            root.join("release-gate-artifacts.txt"),
            root.join("leserpent-macos-release-preflight")
                .join("release-gate-preflight.json"),
        ];
        let files = paths
            .into_iter()
            .map(|path| {
                let contents = path.is_file().then(|| fs::read(&path).unwrap());
                (path, contents)
            })
            .collect();
        Self { _lock: lock, files }
    }
}

impl Drop for ReleaseGateEvidenceGuard {
    fn drop(&mut self) {
        for (path, contents) in &self.files {
            if let Some(contents) = contents {
                if let Some(parent) = path.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                let _ = fs::write(path, contents);
            } else if path.is_file() || path.is_symlink() {
                let _ = fs::remove_file(path);
            }
        }
    }
}

fn read_fixture(relative: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    let contents = fs::read_to_string(&path).unwrap_or_else(|err| {
        panic!("failed to read {}: {}", path.display(), err);
    });
    serde_json::from_str(&contents).unwrap_or_else(|err| {
        panic!("failed to parse fixture {}: {}", path.display(), err);
    })
}

fn run_validate_json(args: &[&str]) -> (bool, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_gewyvern_validate"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args(args)
        .output()
        .unwrap_or_else(|err| panic!("failed to run gewyvern_validate: {err}"));

    (
        output.status.success(),
        String::from_utf8(output.stdout).unwrap_or_else(|err| {
            panic!("stdout was not utf-8: {err}");
        }),
        String::from_utf8(output.stderr).unwrap_or_else(|err| {
            panic!("stderr was not utf-8: {err}");
        }),
    )
}

fn parse_single_json(body: &str) -> Value {
    serde_json::from_str(body.trim()).unwrap_or_else(|err| {
        panic!("failed to parse json output `{}`: {}", body, err);
    })
}

fn normalize_release_artifact_shape(index: &Value) -> Value {
    let artifacts = index["artifacts"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|entry| {
            serde_json::json!({
                "expectation": entry["expectation"],
                "key": entry["key"],
                "kind": entry["kind"],
                "note": entry["note"],
                "producer": entry["producer"],
            })
        })
        .collect::<Vec<_>>();

    serde_json::json!({
        "schema_version": index["schema_version"],
        "kind": index["kind"],
        "name": index["name"],
        "artifacts": artifacts,
    })
}

#[test]
fn list_json_matches_fixture() {
    let expected = read_fixture("docs/fixtures/gewyvern_validate_list.json");
    let (ok, stdout, stderr) = run_validate_json(&["--json", "list"]);

    assert!(ok, "list should succeed, stderr: {stderr}");
    assert!(stderr.trim().is_empty(), "unexpected stderr: {stderr}");
    assert_eq!(parse_single_json(&stdout), expected);
}

#[test]
fn help_json_matches_fixture() {
    let expected = read_fixture("docs/fixtures/gewyvern_validate_help.json");
    let (ok, stdout, stderr) = run_validate_json(&["--json", "help"]);

    assert!(ok, "help should succeed, stderr: {stderr}");
    assert!(stderr.trim().is_empty(), "unexpected stderr: {stderr}");
    assert_eq!(parse_single_json(&stdout), expected);
}

#[test]
fn json_out_alone_writes_the_success_payload() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "gewyvern-validate-json-out-{}-{unique}.json",
        std::process::id()
    ));
    let output = Command::new(env!("CARGO_BIN_EXE_gewyvern_validate"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .arg("--json-out")
        .arg(&path)
        .arg("list")
        .output()
        .unwrap_or_else(|err| panic!("failed to run gewyvern_validate: {err}"));

    let written = fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
    let _ = fs::remove_file(&path);

    assert!(output.status.success());
    assert_eq!(
        parse_single_json(&written),
        read_fixture("docs/fixtures/gewyvern_validate_list.json")
    );
    assert_eq!(
        parse_single_json(&String::from_utf8(output.stdout).unwrap()),
        parse_single_json(&written)
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn json_out_write_failure_is_never_reported_as_success() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let parent = std::env::temp_dir().join(format!(
        "gewyvern-validate-json-parent-{}-{unique}",
        std::process::id()
    ));
    fs::write(&parent, b"not a directory").unwrap();
    let path = parent.join("result.json");
    let output = Command::new(env!("CARGO_BIN_EXE_gewyvern_validate"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .arg("--json-out")
        .arg(&path)
        .arg("list")
        .output()
        .unwrap_or_else(|err| panic!("failed to run gewyvern_validate: {err}"));
    let _ = fs::remove_file(&parent);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("cannot write JSON output")
    );
}

#[test]
fn minimal_release_gate_json_matches_fixture() {
    let _evidence = ReleaseGateEvidenceGuard::acquire();
    let expected = read_fixture("docs/fixtures/gewyvern_validate_release_gate_minimal.json");
    let (ok, stdout, stderr) = run_validate_json(&[
        "--json",
        "release-gate",
        "--skip-build",
        "--skip-release-check",
        "--skip-stack",
        "--skip-debugger-cross",
        "--skip-pathology",
    ]);

    assert!(ok, "release-gate should succeed, stderr: {stderr}");
    assert!(stderr.trim().is_empty(), "unexpected stderr: {stderr}");
    assert_eq!(parse_single_json(&stdout), expected);
}

#[test]
fn release_gate_retains_valid_blocked_macos_preflight() {
    let _evidence = ReleaseGateEvidenceGuard::acquire();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("docs/fixtures/leserpent_macos_release_preflight.json");
    let fixture = fixture.to_str().unwrap();
    let (ok, stdout, stderr) = run_validate_json(&[
        "--json",
        "release-gate",
        "--skip-build",
        "--skip-release-check",
        "--skip-stack",
        "--skip-debugger-cross",
        "--skip-pathology",
        "--macos-release-preflight",
        fixture,
    ]);

    assert!(ok, "blocked preflight is valid evidence, stderr: {stderr}");
    assert!(stderr.trim().is_empty(), "unexpected stderr: {stderr}");
    let report = parse_single_json(&stdout);
    assert_eq!(
        report["checks"],
        serde_json::json!(["macos_release_preflight_blocked"])
    );
    assert_eq!(report["extra"]["stages"]["macos_release_preflight"], true);
    assert_eq!(
        report["extra"]["stages"]["macos_release_preflight_blocked"],
        true
    );
    assert_eq!(
        report["extra"]["stages"]["macos_release_preflight_ready"],
        false
    );
    assert!(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target/validation/leserpent-macos-release-preflight/release-gate-preflight.json")
            .is_file()
    );
}

#[test]
fn minimal_release_gate_writes_release_artifact_index_files() {
    let _evidence = ReleaseGateEvidenceGuard::acquire();
    let expected_shape =
        read_fixture("docs/fixtures/gewyvern_validate_release_gate_artifact_shape.json");
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("validation");
    let index_path = root.join("release-gate-artifacts.json");
    let summary_path = root.join("release-gate-artifacts.txt");

    let (ok, _stdout, stderr) = run_validate_json(&[
        "--json",
        "release-gate",
        "--skip-build",
        "--skip-release-check",
        "--skip-stack",
        "--skip-debugger-cross",
        "--skip-pathology",
    ]);

    assert!(ok, "release-gate should succeed, stderr: {stderr}");
    assert!(
        index_path.is_file(),
        "expected release artifact index at {}",
        index_path.display()
    );
    assert!(
        summary_path.is_file(),
        "expected release artifact summary at {}",
        summary_path.display()
    );

    let index = fs::read_to_string(&index_path).unwrap_or_else(|err| {
        panic!("failed to read {}: {}", index_path.display(), err);
    });
    let parsed = parse_single_json(&index);
    assert_eq!(parsed["kind"], "release_artifact_index");
    assert_eq!(parsed["schema_version"], 2);
    let publication_id = parsed["publication_id"]
        .as_str()
        .expect("release artifact index must carry a publication ID");
    assert!(publication_id.len() <= 128);
    assert_eq!(normalize_release_artifact_shape(&parsed), expected_shape);
    assert!(
        index.contains("\"juice_shop_container_validation\""),
        "expected optional practical target-lab entry in artifact index"
    );
    assert!(
        index.contains("\"ftp_denied_container_validation\""),
        "expected FTP denied practical target-lab entry in artifact index"
    );
    assert!(
        index.contains("\"ldap_bind_denied_container_validation\""),
        "expected LDAP denied practical target-lab entry in artifact index"
    );

    let summary = fs::read_to_string(&summary_path).unwrap_or_else(|err| {
        panic!("failed to read {}: {}", summary_path.display(), err);
    });
    assert!(summary.contains("release gate artifacts: ok"));
    assert!(summary.contains(&format!("publication_id={publication_id}\n")));
    assert!(summary.contains("juice_shop_container_validation"));
    assert!(summary.contains("ftp_denied_container_validation"));
    assert!(summary.contains("ldap_bind_denied_container_validation"));
}

#[test]
fn invalid_cli_input_json_matches_fixture() {
    let expected = read_fixture("docs/fixtures/gewyvern_validate_invalid_cli_input.json");
    let (ok, stdout, stderr) = run_validate_json(&["--json", "release-gate", "--wat"]);

    assert!(!ok, "invalid cli input should fail");
    assert!(stdout.trim().is_empty(), "unexpected stdout: {stdout}");
    assert_eq!(parse_single_json(&stderr), expected);
}
