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
    assert!(binary.contains("\"high-frequency\""));
    assert!(binary.contains("\"registry\""));
    assert!(binary.contains("\"runtime-lifecycle\""));
    assert!(binary.contains("--limit"));
    assert!(mod_file.contains("run_debugger_cross_validation"));
    assert!(mod_file.contains("run_high_frequency_validation"));
    assert!(mod_file.contains("run_registry_validation"));
    assert!(mod_file.contains("run_runtime_lifecycle_validation"));
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
fn docs_prefer_native_validation_entrypoints() {
    let entrypoints = read_repo_file("docs/script-entrypoints.md");
    let runtime_surface = read_repo_file("docs/book/how-to-validate-runtime-surface.md");

    for doc in [&entrypoints, &runtime_surface] {
        assert!(doc.contains("cargo run --quiet --bin gewyvern_validate -- registry"));
        assert!(doc.contains("cargo run --quiet --bin gewyvern_validate -- high-frequency"));
        assert!(doc.contains("cargo run --quiet --bin gewyvern_validate -- debugger-cross"));
        assert!(doc.contains("cargo run --quiet --bin gewyvern_validate -- runtime-lifecycle"));
        assert!(doc.contains("legacy"));
    }
}
