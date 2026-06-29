use std::fs;
use std::path::Path;

fn read_repo_file(relative: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    fs::read_to_string(&path).unwrap_or_else(|err| {
        panic!("failed to read {}: {}", path.display(), err);
    })
}

#[test]
fn cross_validation_script_checks_positive_and_negative_debugger_surfaces() {
    let script = read_repo_file("scripts/validation/debugger_cross_validation.sh");
    let harness = read_repo_file("src/validation_harness/debugger_cross.rs");
    let binary = read_repo_file("src/bin/gewyvern_validate.rs");

    assert!(script.contains("gewyvern_validate"));
    assert!(script.contains("debugger-cross"));
    assert!(binary.contains("debugger-cross"));
    assert!(harness.contains("http_request_path.gewy"));
    assert!(harness.contains("http_connect_denied_path.gewy"));
    assert!(harness.contains("auth-connect-denied"));
    assert!(harness.contains("invalid.gewy"));
    assert!(harness.contains("primary_failure_basis"));
    assert!(harness.contains("missing_transition"));
    assert!(harness.contains("collect_more_runtime_evidence"));
    assert!(harness.contains("manual_review"));
    assert!(harness.contains("parse_ok"));
    assert!(harness.contains("validation_ok"));
    assert!(harness.contains("diagnostics_ok"));
}

#[test]
fn cross_validation_is_documented_as_a_debugger_readiness_gate() {
    let entrypoints = read_repo_file("docs/script-entrypoints.md");
    let runtime_surface = read_repo_file("docs/book/how-to-validate-runtime-surface.md");

    for doc in [&entrypoints, &runtime_surface] {
        assert!(doc.contains("gewyvern_validate"));
        assert!(doc.contains("scripts/validation/debugger_cross_validation.sh"));
        assert!(doc.contains("cross"));
        assert!(doc.contains("negative"));
    }
}
