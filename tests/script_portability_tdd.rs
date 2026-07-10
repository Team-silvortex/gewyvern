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
        "scripts/validation/pathological_container_validation.sh",
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

#[test]
fn markdown_docs_do_not_embed_local_checkout_paths() {
    let roots = [
        Path::new(env!("CARGO_MANIFEST_DIR")).join("README.md"),
        Path::new(env!("CARGO_MANIFEST_DIR")).join("ROADMAP.md"),
        Path::new(env!("CARGO_MANIFEST_DIR")).join("docs"),
        Path::new(env!("CARGO_MANIFEST_DIR")).join("apps/leserpent/README.md"),
    ];
    let forbidden = [
        local_path(&["Users", "Shared", "chroot", "dev", "gewyvern"]),
        local_path(&["Users", "Shared", "chroot", "dev", "etragon"]),
        local_path(&["Users", "Shared", "chroot", "dev", "leserpent"]),
        local_path(&["Users", "seis"]),
        local_path(&["var", "folders"]),
        local_path(&["home", "chiharukiryu", "work", "gewyvern-server-test"]),
        local_path(&["home", "gewyvern-lab", "work", "gewyvern"]),
        local_path(&["home", "user"]),
        local_path(&["absolute", "path"]),
        local_path(&["path", "to"]),
    ];
    let mut failures = Vec::new();

    for root in roots {
        collect_markdown_path_failures(&root, &forbidden, &mut failures);
    }

    assert!(
        failures.is_empty(),
        "markdown docs should not embed local checkout paths:\n{}",
        failures.join("\n")
    );
}

#[test]
fn release_docs_prefer_native_validation_entrypoints() {
    let field_validation = read_repo_file("docs/field-validation.md");
    let field_findings = read_repo_file("docs/field-findings.md");
    let v020 = read_repo_file("docs/history/v0.20.x.md");

    assert!(
        field_validation
            .contains("cargo run --quiet --bin gewyvern_validate -- release-container-check")
    );
    assert!(
        field_validation
            .contains("cargo run --quiet --bin gewyvern_validate -- container-runtime-validation")
    );
    assert!(
        field_findings
            .contains("cargo run --quiet --bin gewyvern_validate -- release-gate --skip-build")
    );
    assert!(v020.contains("cargo run --quiet --bin gewyvern_validate -- release-container-check"));
    assert!(
        v020.contains("cargo run --quiet --bin gewyvern_validate -- release-gate --skip-build")
    );
}

fn local_path(parts: &[&str]) -> String {
    format!("/{}", parts.join("/"))
}

fn collect_markdown_path_failures(path: &Path, forbidden: &[String], failures: &mut Vec<String>) {
    if path.is_dir() {
        for entry in fs::read_dir(path)
            .unwrap_or_else(|err| panic!("failed to read directory {}: {}", path.display(), err))
        {
            let entry = entry.unwrap_or_else(|err| panic!("failed to read dir entry: {err}"));
            collect_markdown_path_failures(&entry.path(), forbidden, failures);
        }
        return;
    }

    if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
        return;
    }

    let body = fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("failed to read {}: {}", path.display(), err));
    for needle in forbidden {
        if body.contains(needle) {
            failures.push(format!("{} contains {}", path.display(), needle));
        }
    }
}
