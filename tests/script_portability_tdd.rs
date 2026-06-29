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
    let script = read_repo_file("scripts/validation/field_validation_smoke.sh");

    assert!(script.contains("ROUNDTRIP_SOCKET=\"/tmp/gewyvern-field-validation-$$.sock\""));
    assert!(!script.contains("/private/tmp/gewyvern-field-validation.sock"));
    assert!(!script.contains("ROUNDTRIP_SOCKET=\"${TMP_DIR}/gewyvern-field-validation.sock\""));
}

#[test]
fn resilience_log_evidence_does_not_require_ripgrep() {
    let script = read_repo_file("scripts/validation/runtime_resilience_log_evidence.sh");

    assert!(script.contains("command -v rg"));
    assert!(script.contains("grep -E"));
    assert!(script.contains("backoff_ms="));
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
