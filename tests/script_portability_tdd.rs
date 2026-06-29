use std::fs;
use std::path::Path;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

fn read_repo_file(relative: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    fs::read_to_string(&path).unwrap_or_else(|err| {
        panic!("failed to read {}: {}", path.display(), err);
    })
}

#[test]
fn field_validation_socket_roundtrip_uses_short_cross_platform_socket_path() {
    let harness = read_repo_file("src/validation_harness/field_smoke.rs");
    let script = read_repo_file("scripts/validation/field_validation_smoke.sh");

    assert!(harness.contains("/tmp/gewyvern-field-validation-{}.sock"));
    assert!(!script.contains("/private/tmp/gewyvern-field-validation.sock"));
    assert!(!script.contains("ROUNDTRIP_SOCKET=\"${TMP_DIR}/gewyvern-field-validation.sock\""));
}

#[test]
fn resilience_log_evidence_no_longer_depends_on_shell_grep() {
    let harness = read_repo_file("src/validation_harness/resilience.rs");
    let script = read_repo_file("scripts/validation/runtime_resilience_log_evidence.sh");
    let fault = read_repo_file("scripts/validation/runtime_resilience_fault_injection.sh");

    assert!(harness.contains("line_has_resilience_signal"));
    assert!(harness.contains("backoff_ms="));
    assert!(harness.contains("TcpStream::connect"));
    assert!(script.contains("gewyvern_validate"));
    assert!(script.contains("resilience-log-evidence"));
    assert!(fault.contains("resilience-drive-bad-json"));
    assert!(!script.contains("command -v rg"));
    assert!(!script.contains("grep -E"));
    assert!(!fault.contains("command -v nc"));
    assert!(!fault.contains(" nc "));
}

#[test]
fn demo_roundtrip_wrappers_do_not_require_curl_or_python() {
    let socket = read_repo_file("scripts/demos/socket_roundtrip_demo.sh");
    let training = read_repo_file("scripts/demos/training_dataset_roundtrip_demo.sh");
    let external = read_repo_file("scripts/demos/external_engine_roundtrip_demo.sh");

    assert!(socket.contains("socket-roundtrip"));
    assert!(training.contains("training-roundtrip"));
    assert!(external.contains("external-engine-roundtrip"));
    assert!(!socket.contains("curl"));
    assert!(!training.contains("curl"));
    assert!(!external.contains("curl"));
    assert!(!training.contains("python3"));
    assert!(!external.contains("python3"));
}

#[test]
fn linux_probe_docs_call_out_required_privileges() {
    let entrypoints = read_repo_file("docs/script-entrypoints.md");
    let cli_recipes = read_repo_file("docs/cli-recipes.md");

    for doc in [&entrypoints, &cli_recipes] {
        assert!(doc.contains("sudo"));
        assert!(doc.contains("BPF attach privileges"));
        assert!(doc.contains("Operation not permitted"));
    }
}

#[cfg(unix)]
#[test]
fn documented_shell_entrypoints_are_executable() {
    let entrypoints = [
        "scripts/demos/external_engine_roundtrip_demo.sh",
        "scripts/demos/socket_roundtrip_demo.sh",
        "scripts/demos/training_dataset_roundtrip_demo.sh",
        "scripts/linux/linux_attach_smoke.sh",
        "scripts/linux/linux_kprobe_smoke.sh",
        "scripts/linux/linux_tc_smoke.sh",
        "scripts/packaging/container_operator_path_validation.sh",
        "scripts/packaging/container_protocol_validation.sh",
        "scripts/packaging/container_runtime_validation.sh",
        "scripts/packaging/container_validation_summary.sh",
        "scripts/packaging/package_install_smoke.sh",
        "scripts/packaging/release_container_check.sh",
        "scripts/packaging/release_gate.sh",
        "scripts/validation/debugger_cross_validation.sh",
        "scripts/validation/field_validation_smoke.sh",
        "scripts/validation/high_frequency_validation.sh",
        "scripts/validation/registry_validation.sh",
        "scripts/validation/runtime_lifecycle_validation.sh",
        "scripts/validation/runtime_operator_validation.sh",
        "scripts/validation/runtime_resilience_fault_injection.sh",
        "scripts/validation/runtime_resilience_log_evidence.sh",
        "scripts/validation/runtime_resilience_roundtrip.sh",
        "scripts/validation/runtime_resilience_validation.sh",
        "scripts/validation/three_module_stack_smoke.sh",
    ];

    for relative in entrypoints {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
        let mode = fs::metadata(&path)
            .unwrap_or_else(|err| panic!("failed to stat {}: {}", path.display(), err))
            .permissions()
            .mode();
        assert_ne!(mode & 0o111, 0, "{} should be executable", relative);
    }
}
