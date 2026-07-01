use std::fs;
use std::path::Path;

fn read_repo_file(relative: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    fs::read_to_string(&path).unwrap_or_else(|err| {
        panic!("failed to read {}: {}", path.display(), err);
    })
}

#[test]
fn native_validation_harness_exposes_registry_and_debugger_commands() {
    let binary = read_repo_file("src/bin/gewyvern_validate.rs");
    let mod_file = read_repo_file("src/validation_harness.rs");

    assert!(binary.contains("\"debugger-cross\""));
    assert!(binary.contains("evidence-index.json"));
    assert!(binary.contains("\"field-smoke\""));
    assert!(binary.contains("\"high-frequency\""));
    assert!(binary.contains("\"registry\""));
    assert!(binary.contains("\"socket-roundtrip\""));
    assert!(binary.contains("\"training-roundtrip\""));
    assert!(binary.contains("\"external-engine-roundtrip\""));
    assert!(binary.contains("\"resilience-log-evidence\""));
    assert!(binary.contains("\"resilience-roundtrip\""));
    assert!(binary.contains("\"resilience-bundle\""));
    assert!(binary.contains("\"resilience-emit-helper\""));
    assert!(binary.contains("\"resilience-drive-bad-json\""));
    assert!(binary.contains("\"runtime-lifecycle\""));
    assert!(binary.contains("\"runtime-operator\""));
    assert!(binary.contains("run_stack_command"));
    assert!(binary.contains("print_stack_list"));
    assert!(binary.contains("--limit"));
    assert!(binary.contains("--json-out"));
    assert!(mod_file.contains("run_debugger_cross_validation"));
    assert!(mod_file.contains("run_socket_roundtrip_demo"));
    assert!(mod_file.contains("run_training_dataset_roundtrip_demo"));
    assert!(mod_file.contains("run_external_engine_roundtrip_demo"));
    assert!(mod_file.contains("run_field_smoke_validation"));
    assert!(mod_file.contains("run_high_frequency_validation"));
    assert!(mod_file.contains("run_registry_validation"));
    assert!(mod_file.contains("run_resilience_log_evidence_validation"));
    assert!(mod_file.contains("run_resilience_roundtrip_validation"));
    assert!(mod_file.contains("run_resilience_bundle_validation"));
    assert!(mod_file.contains("run_resilience_emit_helper_validation"));
    assert!(mod_file.contains("run_resilience_drive_bad_json_validation"));
    assert!(mod_file.contains("run_runtime_lifecycle_validation"));
    assert!(mod_file.contains("run_runtime_operator_validation"));
    assert!(mod_file.contains("run_stack_probe_validation"));
    assert!(mod_file.contains("write_stack_resilience_summary"));
}

#[test]
fn field_smoke_validation_has_native_assertions_and_legacy_wrapper() {
    let field_smoke = read_repo_file("src/validation_harness/field_smoke.rs");
    let script = read_repo_file("scripts/validation/field_validation_smoke.sh");

    assert!(field_smoke.contains("--demo"));
    assert!(field_smoke.contains("http_request_path.gewy"));
    assert!(field_smoke.contains("explain"));
    assert!(field_smoke.contains("gewyvern-field-validation-{}.sock"));
    assert!(field_smoke.contains("--scan-all"));
    assert!(script.contains("gewyvern_validate"));
    assert!(script.contains("field-smoke"));
}

#[test]
fn demo_roundtrips_are_native_with_legacy_wrappers() {
    let demo = read_repo_file("src/validation_harness/demo_roundtrip.rs");
    let socket_script = read_repo_file("scripts/demos/socket_roundtrip_demo.sh");
    let training_script = read_repo_file("scripts/demos/training_dataset_roundtrip_demo.sh");
    let external_script = read_repo_file("scripts/demos/external_engine_roundtrip_demo.sh");

    assert!(demo.contains("run_socket_roundtrip_demo"));
    assert!(demo.contains("run_training_dataset_roundtrip_demo"));
    assert!(demo.contains("run_external_engine_roundtrip_demo"));
    assert!(demo.contains("training-dataset.json"));
    assert!(demo.contains("analyze-url"));
    assert!(demo.contains("sample_ids_verified"));
    assert!(demo.contains("gewyvern_socket_send"));
    assert!(socket_script.contains("socket-roundtrip"));
    assert!(training_script.contains("training-roundtrip"));
    assert!(external_script.contains("external-engine-roundtrip"));
    assert!(!training_script.contains("python3"));
    assert!(!training_script.contains("curl"));
    assert!(!external_script.contains("curl"));
}

#[test]
fn registry_validation_has_native_assertions_and_legacy_wrapper() {
    let registry = read_repo_file("src/validation_harness/registry.rs");
    let script = read_repo_file("scripts/validation/registry_validation.sh");

    assert!(registry.contains("protocols"));
    assert!(registry.contains("gewy.pkg"));
    assert!(registry.contains("main.gewy"));
    assert!(registry.contains("parse_ok"));
    assert!(registry.contains("validation_ok"));
    assert!(registry.contains("diagnostics_ok"));
    assert!(registry.contains("GEWY_REGISTRY_LIMIT"));
    assert!(script.contains("gewyvern_validate"));
    assert!(script.contains("registry"));
}

#[test]
fn high_frequency_validation_has_native_assertions_and_legacy_wrapper() {
    let high_frequency = read_repo_file("src/validation_harness/high_frequency.rs");
    let script = read_repo_file("scripts/validation/high_frequency_validation.sh");

    assert!(high_frequency.contains("http_request_path.gewy"));
    assert!(high_frequency.contains("tls_client_path.gewy"));
    assert!(high_frequency.contains("ssh_session_path.gewy"));
    assert!(high_frequency.contains("socks5_auth_path.gewy"));
    assert!(high_frequency.contains("postgres"));
    assert!(high_frequency.contains("primary_module_kind"));
    assert!(high_frequency.contains("operator_guidance_action"));
    assert!(high_frequency.contains("mixed_dns_tls_http_profile"));
    assert!(script.contains("gewyvern_validate"));
    assert!(script.contains("high-frequency"));
}

#[test]
fn runtime_lifecycle_validation_manages_processes_natively() {
    let lifecycle = read_repo_file("src/validation_harness/runtime_lifecycle.rs");
    let script = read_repo_file("scripts/validation/runtime_lifecycle_validation.sh");

    assert!(lifecycle.contains("start_gewyvern"));
    assert!(lifecycle.contains("wait_for_http_body"));
    assert!(lifecycle.contains("send_invalid_session"));
    assert!(lifecycle.contains("expect_http_unreachable"));
    assert!(lifecycle.contains("expect_socket_send_fails"));
    assert!(lifecycle.contains("socket_service_recovered"));
    assert!(script.contains("gewyvern_validate"));
    assert!(script.contains("runtime-lifecycle"));
}

#[test]
fn runtime_operator_validation_moves_live_operator_checks_into_rust() {
    let operator = read_repo_file("src/validation_harness/runtime_operator.rs");
    let script = read_repo_file("scripts/validation/runtime_operator_validation.sh");

    assert!(operator.contains("validate_tcp_operator_path"));
    assert!(operator.contains("validate_udp_operator_path"));
    assert!(operator.contains("training_dataset_sample_ids_roundtrip"));
    assert!(operator.contains("/v1/latest/training-dataset.json"));
    assert!(operator.contains("send_invalid_session"));
    assert!(operator.contains("requires_operator_confirmation"));
    assert!(script.contains("gewyvern_validate"));
    assert!(script.contains("runtime-operator"));
    assert!(!script.contains("curl -fsS"));
    assert!(!script.contains("python3"));
}

#[test]
fn three_module_stack_smoke_uses_native_stack_probe_for_json_readiness() {
    let stack_probe = read_repo_file("src/validation_harness/stack_probe.rs");
    let stack_cli = read_repo_file("src/validation_harness_cli_stack.rs");
    let script = read_repo_file("scripts/validation/three_module_stack_smoke.sh");

    assert!(stack_probe.contains("resilience-healthy"));
    assert!(stack_probe.contains("leserpent-runtime-detail"));
    assert!(stack_cli.contains("stack-check-json"));
    assert!(stack_cli.contains("stack-register-runtime-json"));
    assert!(script.contains("stack-probe"));
    assert!(script.contains("stack-resilience-summary"));
    assert!(script.contains("127.0.0.1:${socket_port}:9000"));
    assert!(script.contains("127.0.0.1:${api_port}:9100"));
    assert!(script.contains("127.0.0.1:${ET_A_API_PORT}:4321"));
    assert!(!script.contains("wait_for_json_python"));
    assert!(!script.contains("assert_json_python"));
    assert!(!script.contains("python3 -c"));
}

#[test]
fn external_engine_roundtrip_rejects_shell_command_bridge() {
    let harness = read_repo_file("src/validation_harness/demo_roundtrip.rs");
    let docs = read_repo_file("docs/book/how-to-wire-etragon-sidecar.md");

    assert!(harness.contains("validate_external_engine_command"));
    assert!(harness.contains("remove_stale_unix_socket"));
    assert!(harness.contains("symlink_metadata"));
    assert!(harness.contains("is_socket"));
    assert!(!harness.contains("Command::new(\"sh\")"));
    assert!(!harness.contains("arg(\"-c\")"));
    assert!(docs.contains("single executable path"));
}

#[test]
fn control_plane_security_limits_large_persistence_imports() {
    let security =
        read_repo_file("apps/leserpent/src/Leserpent/ControlPlane/ControlPlaneSecurityPolicy.cs");

    assert!(security.contains("PersistenceImportBodyLimitBytes"));
    assert!(security.contains("IHttpMaxRequestBodySizeFeature"));
    assert!(security.contains("ApplyPersistenceImportLimit"));
    assert!(security.contains("MaxRequestBodySize = PersistenceImportBodyLimitBytes"));
    assert!(security.contains("Status413PayloadTooLarge"));
    assert!(security.contains("persistence_import_too_large"));
}

#[test]
fn control_plane_state_defaults_avoid_source_tree_runtime_state() {
    let store =
        read_repo_file("apps/leserpent/src/Leserpent/ControlPlane/ControlPlaneStateStore.cs");
    let gitignore = read_repo_file(".gitignore");
    let sample =
        read_repo_file("apps/leserpent/src/Leserpent/data/control-plane-state.sample.json");

    assert!(store.contains("Environment.SpecialFolder.LocalApplicationData"));
    assert!(store.contains("LESERPENT_STATE_PATH"));
    assert!(!store.contains("ContentRootPath, \"data\", \"control-plane-state.json\""));
    assert!(gitignore.contains("apps/leserpent/src/Leserpent/data/control-plane-state.json"));
    assert!(sample.contains("\"runtimes\": []"));
    assert!(sample.contains("\"sessions\": []"));
}

#[test]
fn resilience_validation_bundle_is_native_with_legacy_wrappers() {
    let resilience = read_repo_file("src/validation_harness/resilience.rs");
    let fault = read_repo_file("scripts/validation/runtime_resilience_fault_injection.sh");
    let evidence = read_repo_file("scripts/validation/runtime_resilience_log_evidence.sh");
    let roundtrip = read_repo_file("scripts/validation/runtime_resilience_roundtrip.sh");
    let bundle = read_repo_file("scripts/validation/runtime_resilience_validation.sh");

    assert!(resilience.contains("run_resilience_log_evidence_validation"));
    assert!(resilience.contains("write_roundtrip_artifacts"));
    assert!(resilience.contains("run_resilience_bundle_validation"));
    assert!(resilience.contains("run_resilience_emit_helper_validation"));
    assert!(resilience.contains("run_resilience_drive_bad_json_validation"));
    assert!(resilience.contains("TcpStream::connect"));
    assert!(resilience.contains("external_analysis_circuit_open"));
    assert!(resilience.contains("backoff_ms="));
    assert!(fault.contains("resilience-emit-helper"));
    assert!(fault.contains("resilience-drive-bad-json"));
    assert!(evidence.contains("resilience-log-evidence"));
    assert!(roundtrip.contains("resilience-roundtrip"));
    assert!(bundle.contains("resilience-bundle"));
}

#[test]
fn docs_prefer_native_validation_entrypoints() {
    let entrypoints = read_repo_file("docs/script-entrypoints.md");
    let runtime_surface = read_repo_file("docs/book/how-to-validate-runtime-surface.md");

    for doc in [&entrypoints, &runtime_surface] {
        assert!(doc.contains("cargo run --quiet --bin gewyvern_validate -- registry"));
        assert!(doc.contains("cargo run --quiet --bin gewyvern_validate -- field-smoke"));
        assert!(doc.contains("cargo run --quiet --bin gewyvern_validate -- high-frequency"));
        assert!(doc.contains("cargo run --quiet --bin gewyvern_validate -- debugger-cross"));
        assert!(doc.contains("cargo run --quiet --bin gewyvern_validate -- runtime-lifecycle"));
        assert!(doc.contains("cargo run --quiet --bin gewyvern_validate -- runtime-operator"));
        assert!(doc.contains("cargo run --quiet --bin gewyvern_validate -- socket-roundtrip"));
        assert!(doc.contains("cargo run --quiet --bin gewyvern_validate -- training-roundtrip"));
        assert!(
            doc.contains("cargo run --quiet --bin gewyvern_validate -- external-engine-roundtrip")
        );
        assert!(doc.contains("legacy"));
    }
}
