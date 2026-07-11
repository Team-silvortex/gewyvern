use std::fs;
use std::path::Path;
use std::process::Command;

use serde_json::Value;

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

#[test]
fn list_json_matches_fixture() {
    let expected = read_fixture("docs/fixtures/gewyvern_validate_list.json");
    let (ok, stdout, stderr) = run_validate_json(&["--json", "list"]);

    assert!(ok, "list should succeed, stderr: {stderr}");
    assert!(stderr.trim().is_empty(), "unexpected stderr: {stderr}");
    assert_eq!(parse_single_json(&stdout), expected);
}

#[test]
fn minimal_release_gate_json_matches_fixture() {
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
fn minimal_release_gate_writes_release_artifact_index_files() {
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
    assert_eq!(parsed["schema_version"], 1);
    assert!(
        index.contains("\"juice_shop_container_validation\""),
        "expected optional practical target-lab entry in artifact index"
    );

    let summary = fs::read_to_string(&summary_path).unwrap_or_else(|err| {
        panic!("failed to read {}: {}", summary_path.display(), err);
    });
    assert!(summary.contains("release gate artifacts: ok"));
    assert!(summary.contains("juice_shop_container_validation"));
}

#[test]
fn invalid_cli_input_json_matches_fixture() {
    let expected = read_fixture("docs/fixtures/gewyvern_validate_invalid_cli_input.json");
    let (ok, stdout, stderr) = run_validate_json(&["--json", "release-gate", "--wat"]);

    assert!(!ok, "invalid cli input should fail");
    assert!(stdout.trim().is_empty(), "unexpected stdout: {stdout}");
    assert_eq!(parse_single_json(&stderr), expected);
}
