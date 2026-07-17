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
    let release_gate = read_repo_file("src/validation_harness/release_gate.rs");

    assert!(binary.contains("\"debugger-cross\""));
    assert!(binary.contains("\"container-operator-path-validation\""));
    assert!(binary.contains("\"container-protocol-validation\""));
    assert!(binary.contains("\"container-runtime-validation\""));
    assert!(binary.contains("\"container-validation-summary\""));
    assert!(binary.contains("\"ftp-denied-container-validation\""));
    assert!(binary.contains("\"ldap-bind-denied-container-validation\""));
    assert!(binary.contains("\"leserpent-aot\""));
    assert!(binary.contains("\"leserpent-benchmark\""));
    assert!(binary.contains("\"leserpent-parity-recovery\""));
    assert!(binary.contains("\"leserpent-transport\""));
    assert!(binary.contains("\"leserpent-accessibility\""));
    assert!(binary.contains("\"leselang-fuzz\""));
    assert!(binary.contains("\"package-install-smoke\""));
    assert!(binary.contains("\"remote-linux-host-validation\""));
    assert!(binary.contains("\"linux-attach-smoke\""));
    assert!(binary.contains("\"linux-kprobe-smoke\""));
    assert!(binary.contains("\"linux-tc-smoke\""));
    assert!(binary.contains("evidence-index.json"));
    assert!(binary.contains("\"field-smoke\""));
    assert!(binary.contains("\"high-frequency\""));
    assert!(binary.contains("\"juice-shop-container-validation\""));
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
    assert!(binary.contains("listed_commands"));
    assert!(binary.contains("STACK_COMMANDS"));
    assert!(binary.contains("--limit"));
    assert!(binary.contains("--json-out"));
    assert!(binary.contains("--json"));
    assert!(binary.contains("--json-errors"));
    assert!(binary.contains("TOP_LEVEL_COMMANDS"));
    assert!(binary.contains("unknown_command_error"));
    assert!(binary.contains("suggest_command"));
    assert!(binary.contains("levenshtein_distance"));
    assert!(binary.contains("print_help_json"));
    assert!(binary.contains("print_validation_report"));
    assert!(binary.contains("emit_json_payload"));
    assert!(binary.contains("JSON_SCHEMA_VERSION"));
    assert!(binary.contains("release_gate_summary_value"));
    assert!(binary.contains("remote_linux_host_summary_value"));
    assert!(binary.contains("summarize_remote_validation_posture"));
    assert!(binary.contains("parse_bool_string"));
    assert!(binary.contains("\"commands\""));
    assert!(binary.contains("\"evidence_dir\""));
    assert!(binary.contains("\"schema_version\""));
    assert!(binary.contains("\"preflight\""));
    assert!(binary.contains("\"ebpf\""));
    assert!(binary.contains("\"phase_timings\""));
    assert!(binary.contains("\"package_build_timings\""));
    assert!(binary.contains("\"total_seconds\""));
    assert!(binary.contains("\"slowest_phase_entries\""));
    assert!(binary.contains("\"budget_warnings\""));
    assert!(binary.contains("\"recent_ebpf_trend\""));
    assert!(binary.contains("\"recent_ebpf_lines\""));
    assert!(binary.contains("\"remote_ebpf_status_counts\""));
    assert!(binary.contains("\"remote_ebpf_reason_counts\""));
    assert!(binary.contains("\"remote_ebpf_matrix\""));
    assert!(binary.contains("\"validation_posture\""));
    assert!(binary.contains("\"release_gate_signal\""));
    assert!(binary.contains("\"coverage_incomplete\""));
    assert!(binary.contains("\"next_step\""));
    assert!(binary.contains("\"linux_proof_complete\""));
    assert!(binary.contains("\"requires_followup\""));
    assert!(binary.contains("\"gate_posture\""));
    assert!(binary.contains("\"ship_signal\""));
    assert!(binary.contains("summarize_release_gate_posture"));
    assert!(release_gate.contains("release-gate-artifacts.json"));
    assert!(release_gate.contains("release-gate-artifacts.txt"));
    assert!(binary.contains("\"workspace_sync\" => Some(WORKSPACE_SYNC_BUDGET_SECONDS)"));
    assert!(
        binary.contains("\"remote_package_smoke\" => Some(REMOTE_PACKAGE_SMOKE_BUDGET_SECONDS)")
    );
    assert!(
        binary.contains("\"remote_runtime_smoke\" => Some(REMOTE_RUNTIME_SMOKE_BUDGET_SECONDS)")
    );
    assert!(binary.contains("\"stages\""));
    assert!(binary.contains("\"remote\""));
    assert!(binary.contains("json_out_missing"));
    assert!(binary.contains("did you mean"));
    assert!(binary.contains("--remote-host-validation"));
    assert!(binary.contains("--skip-remote-build"));
    assert!(binary.contains("--keep-remote-dir"));
    assert!(mod_file.contains("run_debugger_cross_validation"));
    assert!(mod_file.contains("run_ftp_denied_container_validation"));
    assert!(mod_file.contains("run_ldap_bind_denied_container_validation"));
    assert!(mod_file.contains("run_leserpent_aot_validation"));
    assert!(mod_file.contains("run_leserpent_benchmark_validation"));
    assert!(mod_file.contains("run_leserpent_parity_recovery_validation"));
    assert!(mod_file.contains("run_leserpent_transport_validation"));
    assert!(mod_file.contains("run_leserpent_accessibility_validation"));
    assert!(mod_file.contains("run_leselang_fuzz_validation"));
    assert!(mod_file.contains("run_socket_roundtrip_demo"));
    assert!(mod_file.contains("run_training_dataset_roundtrip_demo"));
    assert!(mod_file.contains("run_external_engine_roundtrip_demo"));
    assert!(mod_file.contains("run_field_smoke_validation"));
    assert!(mod_file.contains("run_high_frequency_validation"));
    assert!(mod_file.contains("run_juice_shop_container_validation"));
    assert!(mod_file.contains("run_registry_validation"));
    assert!(mod_file.contains("run_resilience_log_evidence_validation"));
    assert!(mod_file.contains("run_resilience_roundtrip_validation"));
    assert!(mod_file.contains("run_resilience_bundle_validation"));
    assert!(mod_file.contains("run_resilience_emit_helper_validation"));
    assert!(mod_file.contains("run_resilience_drive_bad_json_validation"));
    assert!(mod_file.contains("run_runtime_lifecycle_validation"));
    assert!(mod_file.contains("run_runtime_operator_validation"));
    assert!(mod_file.contains("run_container_protocol_validation"));
    assert!(mod_file.contains("run_container_operator_path_validation"));
    assert!(mod_file.contains("run_container_runtime_validation"));
    assert!(mod_file.contains("run_container_validation_summary"));
    assert!(mod_file.contains("run_package_install_smoke"));
    assert!(mod_file.contains("run_remote_linux_host_validation"));
    assert!(mod_file.contains("run_linux_attach_smoke"));
    assert!(mod_file.contains("run_linux_kprobe_smoke"));
    assert!(mod_file.contains("run_linux_tc_smoke"));
    assert!(mod_file.contains("run_stack_probe_validation"));
    assert!(mod_file.contains("run_three_module_stack_smoke"));
    assert!(mod_file.contains("run_pathological_container_validation"));
    assert!(
        read_repo_file("src/validation_harness/stack_suites.rs").contains("evidence-index.json")
    );
    assert!(mod_file.contains("write_stack_resilience_summary"));
}

#[test]
fn leserpent_native_aot_proof_is_native_and_fail_closed() {
    let harness = read_repo_file("src/validation_harness/leserpent_aot.rs");
    let binary = read_repo_file("src/bin/gewyvern_validate.rs");

    assert!(harness.contains("--locked-mode"));
    assert!(harness.contains("--no-restore"));
    assert!(harness.contains("NativeMagic::Elf"));
    assert!(harness.contains("NativeMagic::MachO64"));
    assert!(!harness.contains("NativeMagic::Pe"));
    assert!(harness.contains("MAX_ARTIFACT_FILES"));
    assert!(harness.contains("renderer-debugger-conformance-v1.json"));
    assert!(harness.contains("initial_debugger_cancel_buttons=1"));
    assert!(harness.contains("remaining_debugger_cancel_buttons=0"));
    assert!(harness.contains("require_accessibility_proof"));
    assert!(harness.contains("artifact-manifest.json"));
    assert!(harness.contains("evidence-index.json"));
    assert!(!harness.contains("Command::new(\"sh\")"));
    assert!(!harness.contains("sudo"));
    assert!(binary.contains("print_leserpent_aot_help"));
    assert!(binary.contains("missing_native_aot_dependency"));
}

#[test]
fn leserpent_accessibility_proof_audits_real_controls_and_contrast() {
    let harness = read_repo_file("src/validation_harness/leserpent_accessibility.rs");
    let renderer = read_repo_file(
        "apps/leserpent-avalonia/src/Leserpent.Avalonia/AvaloniaDocumentRenderer.cs",
    );
    let binary = read_repo_file("src/bin/gewyvern_validate.rs");

    assert!(harness.contains("accessibility-summary.json"));
    assert!(harness.contains("unique_automation_ids"));
    assert!(harness.contains("wcag_aa_text_contrast"));
    assert!(harness.contains("minimum_contrast"));
    assert!(harness.contains("accessibility_valid=true"));
    assert!(renderer.contains("AuditAccessibility"));
    assert!(renderer.contains("AutomationProperties.GetAutomationId"));
    assert!(renderer.contains("AutomationProperties.GetName"));
    assert!(renderer.contains("AutomationProperties.GetHelpText"));
    assert!(renderer.contains("minimumContrast < 4.5"));
    assert!(renderer.contains("#C44D2D"));
    assert!(!harness.contains("sudo"));
    assert!(binary.contains("print_leserpent_accessibility_help"));
}

#[test]
fn leselang_fuzz_proof_is_named_deterministic_and_retained() {
    let harness = read_repo_file("src/validation_harness/leselang_fuzz.rs");
    let fuzz = read_repo_file("tests/leselang_fuzz_tdd.rs");
    let binary = read_repo_file("src/bin/gewyvern_validate.rs");

    assert!(harness.contains("leselang_fuzz_tdd"));
    assert!(harness.contains("fuzz-config.json"));
    assert!(harness.contains("evidence-index.json"));
    assert!(fuzz.contains("FUZZ_SEED"));
    assert!(fuzz.contains("SOURCE_CASES"));
    assert!(fuzz.contains("CONTINUATION_CASES"));
    assert!(fuzz.contains("decode_continuation"));
    assert!(fuzz.contains("assert_syntax_invariants"));
    assert!(!harness.contains("Command::new(\"sh\")"));
    assert!(binary.contains("print_leselang_fuzz_help"));
}

#[test]
fn leserpent_transport_proof_covers_contract_parity_and_real_ipc() {
    let harness = read_repo_file("src/validation_harness/leserpent_transport.rs");
    let binary = read_repo_file("src/bin/gewyvern_validate.rs");

    assert!(harness.contains("wire-v1-contract"));
    assert!(harness.contains("legacy-v1-compatibility"));
    assert!(harness.contains("cli-leselang-parity"));
    assert!(harness.contains("authenticated-ipc-vertical"));
    assert!(harness.contains("ipc-security-boundary"));
    assert!(harness.contains("invalid-token-rejection"));
    assert!(harness.contains("oversized-frame-rejection"));
    assert!(harness.contains("transport-summary.json"));
    assert!(harness.contains("evidence-index.json"));
    assert!(harness.contains("excluded_future_transports"));
    assert!(!harness.contains("Command::new(\"sh\")"));
    assert!(binary.contains("print_leserpent_transport_help"));
}

#[test]
fn leserpent_benchmark_proof_has_bounded_native_workloads() {
    let harness = read_repo_file("src/validation_harness/leserpent_benchmark.rs");
    let runtime = read_repo_file("crates/leserpent-runtime/examples/runtime_benchmark.rs");
    let ui = read_repo_file("crates/leselang-ui/examples/ui_benchmark.rs");
    let binary = read_repo_file("src/bin/gewyvern_validate.rs");

    assert!(harness.contains("COLD_OPEN_P95_BUDGET_MS"));
    assert!(harness.contains("EFFECT_ENQUEUE_MIN_PER_SECOND"));
    assert!(harness.contains("UI_PATCH_P50_BUDGET_MS"));
    assert!(harness.contains("RELEASE_BINARY_MAX_BYTES"));
    assert!(harness.contains("benchmark-summary.json"));
    assert!(harness.contains("evidence-index.json"));
    assert!(harness.contains("same_host_class_comparison_policy"));
    assert!(runtime.contains("EFFECT_COUNT: usize = 10_000"));
    assert!(runtime.contains("RUNTIME_COUNT: usize = 256"));
    assert!(ui.contains("RUNTIME_COUNT: usize = 256"));
    assert!(ui.contains("apply_patch"));
    assert!(!harness.contains("Command::new(\"sh\")"));
    assert!(binary.contains("print_leserpent_benchmark_help"));
}

#[test]
fn leserpent_parity_recovery_proof_is_non_vacuous_and_retained() {
    let harness = read_repo_file("src/validation_harness/leserpent_parity_recovery.rs");
    let binary = read_repo_file("src/bin/gewyvern_validate.rs");

    assert!(harness.contains("command-origin-lowering"));
    assert!(harness.contains("domain-authorization-idempotency"));
    assert!(harness.contains("debugger-confirmation-boundary"));
    assert!(harness.contains("cli-leselang-origin-parity"));
    assert!(harness.contains("vm-reentry-recovery"));
    assert!(harness.contains("runtime-recovery-injection"));
    assert!(harness.contains("expected_min_tests: 65"));
    assert!(harness.contains("expected_min_tests: 35"));
    assert!(harness.contains("passed_test_count"));
    assert!(harness.contains("proof-summary.json"));
    assert!(harness.contains("evidence-index.json"));
    assert!(harness.contains("worker-crash-final-attempt"));
    assert!(harness.contains("same-revision-workspace-composition"));
    assert!(harness.contains("endpoint-redacted-workspace-output"));
    assert!(harness.contains("workspace_atomic=true, logs_bounded=true, endpoint_retained=false"));
    assert!(!harness.contains("Command::new(\"sh\")"));
    assert!(binary.contains("print_leserpent_parity_recovery_help"));
}

#[test]
fn linux_ebpf_smokes_are_native_with_legacy_wrappers() {
    let smoke = read_repo_file("src/linux_ebpf_smoke.rs");
    let harness = read_repo_file("src/validation_harness/linux_ebpf.rs");
    let binary = read_repo_file("src/bin/gewyvern_validate.rs");
    let attach_script = read_repo_file("scripts/linux/linux_attach_smoke.sh");
    let kprobe_script = read_repo_file("scripts/linux/linux_kprobe_smoke.sh");
    let tc_script = read_repo_file("scripts/linux/linux_tc_smoke.sh");
    let entrypoints = read_repo_file("docs/script-entrypoints.md");
    let pinned_source_evidence: serde_json::Value = serde_json::from_str(&read_repo_file(
        "docs/fixtures/linux_attach_pinned_source_root.json",
    ))
    .expect("pinned Linux attach source-root evidence must be valid JSON");

    assert!(smoke.contains("run_tracepoint_attach_smoke"));
    assert!(smoke.contains("run_kprobe_attach_smoke"));
    assert!(smoke.contains("run_tc_attach_smoke"));
    assert!(smoke.contains("Command::new(\"clang\")"));
    assert!(smoke.contains("Command::new(\"cc\")"));
    assert!(smoke.contains("Command::new(\"tc\")"));
    assert!(smoke.contains("env!(\"CARGO_MANIFEST_DIR\")"));
    assert!(!smoke.contains("std::env::current_dir()"));
    assert_eq!(
        pinned_source_evidence["source_root_policy"],
        "build_time_cargo_manifest_dir"
    );
    assert_eq!(
        pinned_source_evidence["ambient_working_directory_sources"],
        false
    );
    assert_eq!(pinned_source_evidence["results"]["tracepoint_attach"], "ok");
    assert_eq!(pinned_source_evidence["results"]["kprobe_attach"], "ok");
    assert_eq!(pinned_source_evidence["results"]["tc_ingress_attach"], "ok");
    assert_eq!(pinned_source_evidence["matrix"]["ready"], false);
    assert!(smoke.contains("finalize_run_result"));
    assert!(smoke.contains("write_transcript"));
    assert!(harness.contains("run_linux_attach_smoke"));
    assert!(harness.contains("run_linux_kprobe_smoke"));
    assert!(harness.contains("run_linux_tc_smoke"));
    assert!(harness.contains("write_linux_smoke_evidence"));
    assert!(harness.contains("environment.txt"));
    assert!(harness.contains("evidence-index.json"));
    assert!(harness.contains("netdev.txt"));
    assert!(harness.contains("linux_environment_evidence"));
    assert!(harness.contains("bpftool_in_path"));
    assert!(harness.contains("/proc/self/status"));
    assert!(harness.contains("/sys/kernel/btf/vmlinux"));
    assert!(harness.contains("Operation not permitted"));
    assert!(binary.contains("linux-attach-smoke"));
    assert!(binary.contains("linux-kprobe-smoke"));
    assert!(binary.contains("linux-tc-smoke"));
    assert!(attach_script.contains("gewyvern_validate -- linux-attach-smoke"));
    assert!(kprobe_script.contains("gewyvern_validate -- linux-kprobe-smoke"));
    assert!(tc_script.contains("gewyvern_validate -- linux-tc-smoke"));
    assert!(
        entrypoints
            .contains("sudo cargo run --quiet --bin gewyvern_validate -- linux-attach-smoke")
    );
    assert!(entrypoints.contains("thin compatibility"));
    assert!(entrypoints.contains("environment.txt"));
    assert!(entrypoints.contains("evidence-index.json"));
}

#[test]
fn remote_host_validation_records_phase_timings() {
    let remote_host = read_repo_file("src/validation_harness/remote_host.rs");
    let entrypoints = read_repo_file("docs/script-entrypoints.md");

    assert!(remote_host.contains("struct PhaseTiming"));
    assert!(remote_host.contains("measure_phase(&mut phase_timings, \"remote_preflight\""));
    assert!(remote_host.contains("measure_phase(&mut phase_timings, \"workspace_sync\""));
    assert!(remote_host.contains("compute_local_workspace_sync_key()"));
    assert!(remote_host.contains("compute_git_workspace_sync_key"));
    assert!(remote_host.contains("compute_dirty_git_workspace_sync_key"));
    assert!(remote_host.contains("try_reuse_dirty_workspace_sync_key_cache"));
    assert!(remote_host.contains("local-workspace-sync-key-cache.txt"));
    assert!(
        remote_host.contains(".args([\"status\", \"--porcelain=v1\", \"--untracked-files=all\"])")
    );
    assert!(remote_host.contains("Ok(Some(format!(\"git:{head}\")))"));
    assert!(remote_host.contains("git-dirty:"));
    assert!(remote_host.contains("ssh_control_path_template"));
    assert!(remote_host.contains("ensure_ssh_control_master"));
    assert!(remote_host.contains("close_ssh_control_master"));
    assert!(remote_host.contains("ControlMaster=yes"));
    assert!(remote_host.contains(".arg(\"-fN\")"));
    assert!(remote_host.contains(".arg(\"check\")"));
    assert!(remote_host.contains(".arg(\"exit\")"));
    assert!(remote_host.contains("ControlMaster=auto"));
    assert!(remote_host.contains("ControlPersist=60"));
    assert!(remote_host.contains("ControlPath="));
    assert!(remote_host.contains("rsync_ssh_command"));
    assert!(remote_host.contains("remote_workspace_sync_key_matches"));
    assert!(remote_host.contains("write_remote_workspace_sync_key"));
    assert!(remote_host.contains(".gewy-workspace-sync-key"));
    assert!(remote_host.contains("workspace sync cache hit; skipping rsync"));
    assert!(remote_host.contains("measure_phase(&mut phase_timings, \"remote_package_build\""));
    assert!(remote_host.contains("measure_phase(&mut phase_timings, \"remote_package_smoke\""));
    assert!(remote_host.contains("measure_phase(&mut phase_timings, \"remote_runtime_smoke\""));
    assert!(remote_host.contains("\"remote_ebpf_validator_build\""));
    assert!(remote_host.contains("measure_phase(phase_timings, \"remote_ebpf_attach\""));
    assert!(
        remote_host.contains("measure_phase(&mut phase_timings, \"remote_workspace_materialize\"")
    );
    assert!(remote_host.contains("measure_phase(&mut phase_timings, \"remote_workspace_cleanup\""));
    assert!(remote_host.contains("remote-phase-timings.txt"));
    assert!(remote_host.contains("fn has_command(name: &str) -> bool"));
    assert!(remote_host.contains("env::split_paths(&path)"));
    assert!(!remote_host.contains(".arg(format!(\"command -v {name} >/dev/null 2>&1\"))"));
    assert!(remote_host.contains("checks.push(\"remote_phase_timings\".to_string())"));
    assert!(remote_host.contains("write_remote_ebpf_history"));
    assert!(remote_host.contains("remote-ebpf-history.jsonl"));
    assert!(remote_host.contains("remote-ebpf-history-rejected.jsonl"));
    assert!(remote_host.contains("remote-ebpf-latest.json"));
    assert!(remote_host.contains("remote-ebpf-recent.txt"));
    assert!(remote_host.contains("remote-ebpf-status-summary.json"));
    assert!(remote_host.contains("successful_kernel_counts"));
    assert!(remote_host.contains("MINIMUM_MATRIX_HOSTS"));
    assert!(remote_host.contains("MINIMUM_MATRIX_KERNELS"));
    assert!(remote_host.contains("HISTORY_RETENTION: usize = 32"));
    assert!(remote_host.contains("render_remote_ebpf_recent"));
    assert!(remote_host.contains("summarize_remote_ebpf_history"));
    assert!(remote_host.contains("atomic_write_evidence"));
    assert!(remote_host.contains("valid_remote_ebpf_history_entry"));
    assert!(remote_host.contains("acquire_remote_ebpf_history_lock"));
    assert!(remote_host.contains("acquire_remote_validation_run_lock"));
    assert!(remote_host.contains("remove_stale_remote_evidence_lock"));
    assert!(remote_host.contains("REMOTE_RUN_SEQUENCE"));
    assert!(remote_host.contains("total={:.3}"));
    assert!(remote_host.contains("requested remote workspace"));
    assert!(remote_host.contains("resolved remote workspace"));
    assert!(remote_host.contains("remote source cache"));
    assert!(remote_host.contains("remote cargo target cache"));
    assert!(entrypoints.contains("remote-phase-timings.txt"));
    assert!(entrypoints.contains("workspace sync cache"));
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
    let suites = read_repo_file("src/validation_harness/stack_suites.rs");
    let script = read_repo_file("scripts/validation/three_module_stack_smoke.sh");

    assert!(stack_probe.contains("resilience-healthy"));
    assert!(stack_probe.contains("leserpent-runtime-detail"));
    assert!(stack_cli.contains("stack-check-json"));
    assert!(stack_cli.contains("stack-register-runtime-json"));
    assert!(suites.contains("run_three_module_stack_smoke"));
    assert!(suites.contains("run_stack_probe_validation"));
    assert!(suites.contains("write_stack_resilience_summary"));
    assert!(suites.contains("127.0.0.1:{socket_port}:9000"));
    assert!(suites.contains("127.0.0.1:{api_port}:9100"));
    assert!(suites.contains("127.0.0.1:{}:4321"));
    assert!(script.contains("gewyvern_validate"));
    assert!(script.contains("three-module-stack-smoke"));
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
fn control_plane_environment_query_binding_is_explicit_for_runtime_filters() {
    let runtime_endpoints =
        read_repo_file("apps/leserpent/src/Leserpent/ProgramRuntimeEndpoints.cs");
    let fleet_endpoints = read_repo_file("apps/leserpent/src/Leserpent/ProgramFleetEndpoints.cs");

    assert!(runtime_endpoints.contains("using Microsoft.AspNetCore.Mvc;"));
    assert!(fleet_endpoints.contains("using Microsoft.AspNetCore.Mvc;"));
    assert!(runtime_endpoints.contains("[FromQuery(Name = \"environment\")]"));
    assert!(fleet_endpoints.contains("[FromQuery(Name = \"environment\")]"));
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
fn packaging_container_validations_are_native_with_legacy_wrappers() {
    let packaging = read_repo_file("src/validation_harness/container_packaging.rs");
    let build_packages = read_repo_file("scripts/packaging/build_packages.sh");
    let release_gate = read_repo_file("src/validation_harness/release_gate.rs");
    let smoke = read_repo_file("scripts/packaging/package_install_smoke.sh");
    let protocol = read_repo_file("scripts/packaging/container_protocol_validation.sh");
    let operator = read_repo_file("scripts/packaging/container_operator_path_validation.sh");
    let runtime = read_repo_file("scripts/packaging/container_runtime_validation.sh");
    let summary = read_repo_file("scripts/packaging/container_validation_summary.sh");

    assert!(packaging.contains("run_container_protocol_validation"));
    assert!(packaging.contains("run_container_operator_path_validation"));
    assert!(packaging.contains("run_container_runtime_validation"));
    assert!(packaging.contains("run_container_validation_summary"));
    assert!(packaging.contains("GEWY_DEB_PROTOCOL_IMAGE"));
    assert!(packaging.contains("GEWY_RPM_PROTOCOL_IMAGE"));
    assert!(packaging.contains("GEWY_DEB_OPERATOR_IMAGE"));
    assert!(packaging.contains("GEWY_RPM_OPERATOR_IMAGE"));
    assert!(packaging.contains("GEWY_DEB_RUNTIME_IMAGE"));
    assert!(packaging.contains("GEWY_RPM_RUNTIME_IMAGE"));
    assert!(packaging.contains("GEWY_DEB_SMOKE_IMAGE"));
    assert!(packaging.contains("GEWY_RPM_SMOKE_IMAGE"));
    assert!(packaging.contains("timeout_seconds"));
    assert!(packaging.contains("docker"));
    assert!(packaging.contains("run_package_install_smoke"));
    assert!(packaging.contains("package install smoke: ok"));
    assert!(packaging.contains("dpkg-deb -c"));
    assert!(packaging.contains("rpm -qpl"));
    assert!(packaging.contains("curl"));
    assert!(packaging.contains("wait_for_http_body"));
    assert!(packaging.contains("gewyvern_socket_send"));
    assert!(build_packages.contains("--bin gewyvern_validate"));
    assert!(build_packages.contains("-p gewyvern"));
    assert!(build_packages.contains("-p gewyc"));
    assert!(build_packages.contains("command -v ld.lld"));
    assert!(build_packages.contains("-C link-arg=-fuse-ld=lld"));
    assert!(build_packages.contains("build-manifest.txt"));
    assert!(build_packages.contains("record_manifest"));
    assert!(packaging.contains("fn has_command(name: &str) -> bool"));
    assert!(packaging.contains("env::split_paths(&path)"));
    assert!(!packaging.contains(".arg(format!(\"command -v {name} >/dev/null 2>&1\"))"));
    assert!(packaging.contains("container runtime validation: ok"));
    assert!(release_gate.contains("run_package_install_smoke(mode)?"));
    assert!(release_gate.contains("run_container_runtime_validation(mode)?"));
    assert!(release_gate.contains("run_container_validation_summary(mode)?"));
    assert!(release_gate.contains("run_remote_linux_host_validation"));
    assert!(release_gate.contains("run_debugger_cross_validation(None)?"));
    assert!(release_gate.contains("print_remote_release_gate_summary"));
    assert!(release_gate.contains("remote slowest phases"));
    assert!(release_gate.contains("remote eBPF summary"));
    assert!(release_gate.contains("validation-posture:"));
    assert!(release_gate.contains("release-gate-signal:"));
    assert!(release_gate.contains("remote Linux proof is partial"));
    assert!(release_gate.contains("remote budget warning:"));
    assert!(release_gate.contains("remote recent eBPF trend"));
    assert!(release_gate.contains("remote recent eBPF:"));
    assert!(release_gate.contains("remote history integrity:"));
    assert!(release_gate.contains("remote-ebpf-history-rejected.jsonl"));
    assert!(release_gate.contains("remote dir:"));
    assert!(release_gate.contains("covered packaged checks"));
    assert!(release_gate.contains("packaged release scope"));
    assert!(release_gate.contains("remote_linux_host_validation"));
    assert!(release_gate.contains("remote_ebpf_smoke"));
    assert!(release_gate.contains("remote_ebpf_smoke_skipped"));
    assert!(release_gate.contains("debugger_cross_validation"));
    assert!(smoke.contains("gewyvern_validate"));
    assert!(smoke.contains("package-install-smoke"));
    assert!(protocol.contains("gewyvern_validate"));
    assert!(protocol.contains("container-protocol-validation"));
    assert!(operator.contains("gewyvern_validate"));
    assert!(operator.contains("container-operator-path-validation"));
    assert!(runtime.contains("gewyvern_validate"));
    assert!(runtime.contains("container-runtime-validation"));
    assert!(summary.contains("gewyvern_validate"));
    assert!(summary.contains("container-validation-summary"));
    assert!(!summary.contains("run_mode_script"));
}

#[test]
fn remote_linux_host_validation_is_native_and_ssh_backed() {
    let remote = read_repo_file("src/validation_harness/remote_host.rs");
    let binary = read_repo_file("src/bin/gewyvern_validate.rs");
    let docs = read_repo_file("docs/script-entrypoints.md");

    assert!(remote.contains("RemoteLinuxHostOptions"));
    assert!(remote.contains("run_remote_linux_host_validation"));
    assert!(remote.contains("rsync"));
    assert!(remote.contains("ssh"));
    assert!(remote.contains("build_packages.sh --format all"));
    assert!(remote.contains("remote-preflight.txt"));
    assert!(remote.contains("remote-artifacts.txt"));
    assert!(remote.contains("remote-package-build-timings.txt"));
    assert!(remote.contains("remote-ebpf.txt"));
    assert!(remote.contains("build-manifest.txt"));
    assert!(remote.contains("collect_remote_preflight"));
    assert!(remote.contains("collect_remote_artifact_manifest"));
    assert!(remote.contains("collect_remote_package_build_timings"));
    assert!(remote.contains("collect_remote_ebpf_evidence"));
    assert!(remote.contains("sync_remote_ebpf_evidence"));
    assert!(remote.contains("release/gewyvern_validate"));
    assert!(remote.contains("cargo build --quiet --release --bin gewyvern_validate"));
    assert!(!remote.contains("if [ ! -x {validate_bin}"));
    assert!(remote.contains("CALLER_UID=\"$(id -u)\""));
    assert!(remote.contains("GEWY_EVIDENCE_UID=$CALLER_UID"));
    assert!(remote.contains("trap restore_evidence_owner EXIT"));
    assert!(remote.contains("chown -R \"$GEWY_EVIDENCE_UID:$GEWY_EVIDENCE_GID\""));
    assert!(remote.contains("command -v ld.lld"));
    assert!(remote.contains("-C link-arg=-fuse-ld=lld"));
    assert!(remote.contains(".arg(\"tests/\")"));
    assert!(remote.contains(".arg(\"apps/**/obj/\")"));
    assert!(remote.contains(".arg(\"apps/**/bin/\")"));
    assert!(remote.contains(".arg(\"**/__pycache__/\")"));
    assert!(remote.contains(".arg(\".DS_Store\")"));
    assert!(remote.contains("remote_source_cache_dir"));
    assert!(remote.contains(".cache/gewyvern/remote-source"));
    assert!(remote.contains("let validation_workspace = remote_source_cache.clone();"));
    assert!(remote.contains("materialize_remote_workspace"));
    assert!(remote.contains("remote_workspace_materialize"));
    assert!(remote.contains("ln -sfn {remote_source_cache} {remote_path}"));
    assert!(remote.contains("remote_workspace_materialized"));
    assert!(remote.contains("GEWY_REMOTE_EBPF_ADMIN_USER"));
    assert!(remote.contains("GEWY_REMOTE_EBPF_ADMIN_PASSWORD"));
    assert!(remote.contains("sshpass"));
    assert!(remote.contains("ssh_auth_target(host, &auth.user)"));
    assert!(remote.contains("fn ssh_auth_target(host: &str, user: &str) -> String"));
    assert!(!remote.contains(".arg(format!(\"{}@{}\", auth.user, host))"));
    assert!(remote.contains("remote_cargo_target_dir"));
    assert!(remote.contains(".cache/gewyvern/remote-target"));
    assert!(remote.contains(
        "CARGO_TARGET_DIR={target_dir} ./scripts/packaging/build_packages.sh --format all"
    ));
    assert!(remote.contains(
        "CARGO_TARGET_DIR={target_dir} cargo build --quiet --release --bin gewyvern_validate"
    ));
    assert!(remote.contains("remote_preflight"));
    assert!(remote.contains("remote_artifacts_present"));
    assert!(remote.contains("remote_ebpf_smoke"));
    assert!(remote.contains("remote_ebpf_validator_build"));
    assert!(remote.contains("remote_ebpf_attach"));
    assert!(remote.contains("remote_ebpf_evidence_synced"));
    assert!(remote.contains("remote_ebpf_smoke_skipped"));
    assert!(remote.contains("uname -s"));
    assert!(remote.contains("uname -m"));
    assert!(remote.contains("sudo_available"));
    assert!(remote.contains("default_route_device"));
    assert!(remote.contains("sudo_not_available"));
    assert!(remote.contains("all_smokes_passed_admin_ssh"));
    assert!(remote.contains("remote package smoke: ok"));
    assert!(remote.contains("remote_package_smoke_timings"));
    assert!(remote.contains("collect_remote_package_smoke_timings"));
    assert!(remote.contains("target/packages/package-smoke-timings.txt"));
    assert!(remote.contains("target/packages/.package-smoke/deb/$(basename \"$DEB\" .deb)"));
    assert!(remote.contains("target/packages/.package-smoke/rpm/$(basename \"$RPM\" .rpm)"));
    assert!(remote.contains("DEB_STAMP=\"$DEB_ROOT/.deb-sha256\""));
    assert!(remote.contains("RPM_STAMP=\"$RPM_ROOT/.rpm-sha256\""));
    assert!(remote.contains("record_timing deb_unpack_cache_refresh"));
    assert!(remote.contains("record_timing deb_verify"));
    assert!(remote.contains("record_timing rpm_unpack_cache_refresh"));
    assert!(remote.contains("record_timing rpm_verify"));
    assert!(remote.contains("remote runtime smoke: ok"));
    assert!(remote.contains("remote_runtime_smoke_timings"));
    assert!(remote.contains("collect_remote_runtime_smoke_timings"));
    assert!(remote.contains("target/packages/runtime-smoke-timings.txt"));
    assert!(remote.contains("record_timing tcp_boot_health"));
    assert!(remote.contains("record_timing tcp_summary"));
    assert!(remote.contains("record_timing tcp_health_after_bad"));
    assert!(remote.contains("record_timing tcp_analysis"));
    assert!(remote.contains("record_timing udp_boot_health"));
    assert!(remote.contains("record_timing udp_summary"));
    assert!(remote.contains("record_timing udp_analysis"));
    assert!(remote.contains("record_timing total"));
    assert!(remote.contains("awk -F= '/^deb=/{{print $2; exit}}' \"$MANIFEST\""));
    assert!(remote.contains("awk -F= '/^rpm=/{{print $2; exit}}' \"$MANIFEST\""));
    assert!(remote.contains("target/packages/.runtime-smoke/$(basename \"$DEB\" .deb)"));
    assert!(remote.contains("RUNTIME_STAMP=\"$RUNTIME_ROOT/.deb-sha256\""));
    assert!(remote.contains("EXPECTED_DEB_SHA=$(sha256sum \"$DEB\" | awk '{print $1}')"));
    assert!(remote.contains("[ \"$CURRENT_DEB_SHA\" != \"$EXPECTED_DEB_SHA\" ]"));
    assert!(remote.contains("rpm2cpio"));
    assert!(remote.contains("x86_64/amd64"));
    assert!(remote.contains("kyuubiki-lab"));
    assert!(binary.contains("remote-linux-host-validation"));
    assert!(binary.contains("--keep-remote-dir"));
    assert!(binary.contains("--skip-build"));
    assert!(binary.contains("Collect remote Linux/x86_64 preflight evidence"));
    assert!(binary.contains("print_remote_linux_host_validation_summary"));
    assert!(binary.contains("slowest-phases:"));
    assert!(binary.contains("source-cache:"));
    assert!(binary.contains("target-cache:"));
    assert!(binary.contains("remote-ebpf:"));
    assert!(binary.contains("print_release_container_check_summary"));
    assert!(binary.contains("release-mode:"));
    assert!(binary.contains("covered-checks:"));
    assert!(binary.contains("print_failure_guidance"));
    assert!(binary.contains("classify_failure"));
    assert!(binary.contains("enum FailureClass"));
    assert!(binary.contains("failure-class:"));
    assert!(binary.contains("failure-code:"));
    assert!(binary.contains("docker_unreachable"));
    assert!(binary.contains("remote_workspace_retained"));
    assert!(binary.contains("remote_host_not_linux"));
    assert!(binary.contains("remote_host_wrong_arch"));
    assert!(binary.contains("remote_admin_credentials_incomplete"));
    assert!(binary.contains("linux_ebpf_privilege_required"));
    assert!(binary.contains("missing_sshpass"));
    assert!(binary.contains("missing_system_command"));
    assert!(binary.contains("missing_package_artifact"));
    assert!(binary.contains("validation_timeout"));
    assert!(binary.contains("invalid_cli_input"));
    assert!(binary.contains("GlobalCliOptions"));
    assert!(binary.contains("parse_global_cli_options"));
    assert!(binary.contains("print_failure_guidance_json"));
    assert!(binary.contains("\"failure_class\""));
    assert!(binary.contains("\"failure_code\""));
    assert!(binary.contains("\"next_steps\""));
    assert!(binary.contains("\"timing_watch\""));
    assert!(binary.contains("docker daemon is not reachable"));
    assert!(binary.contains("remote workspace retained at "));
    assert!(binary.contains("Operation not permitted"));
    assert!(binary.contains("next-step:"));
    assert!(docs.contains("remote-linux-host-validation"));
    assert!(docs.contains("remote-preflight.txt"));
    assert!(docs.contains("remote-artifacts.txt"));
    assert!(docs.contains("remote-ebpf.txt"));
    assert!(docs.contains("target/validation/remote-linux-host-validation/remote-ebpf"));
    assert!(docs.contains("remote source cache"));
    assert!(docs.contains("repoints its requested remote"));
    assert!(docs.contains("slowest observed phases"));
}

#[test]
fn ftp_denied_container_validation_is_native_and_linux_only() {
    let stack = read_repo_file("src/validation_harness/stack_suites.rs");
    let binary = read_repo_file("src/bin/gewyvern_validate.rs");

    assert!(stack.contains("run_ftp_denied_container_validation"));
    assert!(stack.contains("ftp-denied container validation requires a Linux host"));
    assert!(stack.contains("fauria/vsftpd:latest"));
    assert!(stack.contains("wait_for_ftp_banner"));
    assert!(stack.contains("curl_capture_ftp_denied_exchange"));
    assert!(stack.contains("ftp-target-vsftpd.log"));
    assert!(stack.contains("530 Login incorrect."));
    assert!(stack.contains("Access denied: 530"));
    assert!(stack.contains("FAIL LOGIN"));
    assert!(stack.contains("PASV_ADDRESS=127.0.0.1"));
    assert!(stack.contains("linux-attach-smoke"));
    assert!(stack.contains("linux-kprobe-smoke"));
    assert!(stack.contains("linux-tc-smoke"));
    assert!(binary.contains("ftp-denied-container-validation"));
    assert!(binary.contains("print_ftp_denied_container_validation_help"));
    assert!(binary.contains("Usage: gewyvern_validate ftp-denied-container-validation"));
}

#[test]
fn ldap_bind_denied_container_validation_is_native_and_linux_only() {
    let stack = read_repo_file("src/validation_harness/stack_suites.rs");
    let binary = read_repo_file("src/bin/gewyvern_validate.rs");

    assert!(stack.contains("run_ldap_bind_denied_container_validation"));
    assert!(stack.contains("ldap-bind-denied container validation requires a Linux host"));
    assert!(stack.contains("osixia/openldap:1.5.0"));
    assert!(stack.contains("wait_for_ldap_bind_ready"));
    assert!(stack.contains("ldap_capture_bind_denied_exchange"));
    assert!(stack.contains("Invalid credentials (49)"));
    assert!(stack.contains("err=49"));
    assert!(stack.contains("BIND dn="));
    assert!(stack.contains("linux-attach-smoke"));
    assert!(stack.contains("linux-kprobe-smoke"));
    assert!(stack.contains("linux-tc-smoke"));
    assert!(binary.contains("ldap-bind-denied-container-validation"));
    assert!(binary.contains("print_ldap_bind_denied_container_validation_help"));
    assert!(binary.contains("Usage: gewyvern_validate ldap-bind-denied-container-validation"));
}

#[test]
fn stack_command_probes_avoid_shell_wrappers() {
    let stack = read_repo_file("src/validation_harness/stack_suites.rs");

    assert!(stack.contains("fn has_command(name: &str) -> bool"));
    assert!(stack.contains("env::split_paths(&path)"));
    assert!(!stack.contains(".arg(format!(\"command -v {name} >/dev/null 2>&1\"))"));
}

#[test]
fn docs_prefer_native_validation_entrypoints() {
    let entrypoints = read_repo_file("docs/script-entrypoints.md");
    let cli_recipes = read_repo_file("docs/cli-recipes.md");
    let release_checklist = read_repo_file("docs/release-checklist.md");
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

    assert!(
        entrypoints.contains("cargo run --quiet --bin gewyvern_validate -- --json release-gate")
    );
    assert!(entrypoints.contains("--skip-debugger-cross"));
    assert!(entrypoints.contains(
        "cargo run --quiet --bin gewyvern_validate -- --json remote-linux-host-validation"
    ));
    assert!(entrypoints.contains("slowest_phase_entries"));
    assert!(entrypoints.contains("total_seconds"));
    assert!(entrypoints.contains("budget_warnings"));
    assert!(entrypoints.contains("ship_signal = \"timing_watch\""));
    assert!(entrypoints.contains("workspace_sync"));
    assert!(entrypoints.contains("remote-ebpf-history.jsonl"));
    assert!(entrypoints.contains("remote-ebpf-latest.json"));
    assert!(entrypoints.contains("remote-ebpf-recent.txt"));
    assert!(entrypoints.contains("remote-ebpf-status-summary.json"));
    assert!(entrypoints.contains("remote-ebpf-history-rejected.jsonl"));
    assert!(entrypoints.contains("remote_ebpf_history_integrity"));
    assert!(entrypoints.contains("remote-package-build-timings.txt"));
    assert!(entrypoints.contains("remote-package-smoke-timings.txt"));
    assert!(entrypoints.contains("remote-runtime-smoke-timings.txt"));
    assert!(entrypoints.contains("recent_ebpf_trend"));
    assert!(entrypoints.contains("remote_ebpf_matrix"));
    assert!(entrypoints.contains("coverage_incomplete"));
    assert!(entrypoints.contains("recent_ebpf_lines"));
    assert!(entrypoints.contains("package_smoke_timings"));
    assert!(entrypoints.contains("runtime_smoke_timings"));
    assert!(entrypoints.contains("extra.stages"));
    assert!(entrypoints.contains("target/validation/release-gate-artifacts.json"));
    assert!(entrypoints.contains("target/validation/release-gate-artifacts.txt"));
    assert!(entrypoints.contains("--json-out <path>"));
    assert!(entrypoints.contains("Current JSON failure codes"));
    assert!(entrypoints.contains("remote_admin_credentials_incomplete"));
    assert!(entrypoints.contains("missing_package_artifact"));
    assert!(
        release_checklist
            .contains("cargo run --quiet --bin gewyvern_validate -- --json release-gate")
    );
    assert!(release_checklist.contains("--skip-debugger-cross"));
    assert!(release_checklist.contains("extra.remote.ebpf.status"));
    assert!(release_checklist.contains("extra.remote.total_seconds"));
    assert!(release_checklist.contains("extra.remote.budget_warnings"));
    assert!(release_checklist.contains("extra.ship_signal = \"timing_watch\""));
    assert!(release_checklist.contains("extra.remote.recent_ebpf_trend"));
    assert!(release_checklist.contains("target/validation/release-gate-artifacts.json"));
    assert!(release_checklist.contains("target/validation/release-gate-artifacts.txt"));
    assert!(cli_recipes.contains("## Validation JSON Recipes"));
    assert!(
        cli_recipes.contains("cargo run --quiet --bin gewyvern_validate -- --json release-gate")
    );
    assert!(cli_recipes.contains("--skip-debugger-cross"));
    assert!(cli_recipes.contains(
        "cargo run --quiet --bin gewyvern_validate -- --json remote-linux-host-validation"
    ));
    assert!(cli_recipes.contains("--json-out /tmp/gewyvern-release-gate.json"));
    assert!(cli_recipes.contains("jq '.extra.stages'"));
    assert!(cli_recipes.contains("jq '.extra.total_seconds'"));
    assert!(cli_recipes.contains("jq '.extra.package_build_timings'"));
    assert!(cli_recipes.contains("jq '.extra.package_smoke_timings'"));
    assert!(cli_recipes.contains("jq '.extra.runtime_smoke_timings'"));
    assert!(cli_recipes.contains("jq '.extra.ebpf.default_route_device'"));
    assert!(cli_recipes.contains("jq '.extra.budget_warnings // []'"));
    assert!(
        cli_recipes.contains("jq '.extra.recent_ebpf_trend, .extra.remote_ebpf_status_counts'")
    );
    assert!(cli_recipes.contains("remote-ebpf-history.jsonl"));
    assert!(cli_recipes.contains("remote-ebpf-latest.json"));
    assert!(cli_recipes.contains("remote-ebpf-recent.txt"));
    assert!(cli_recipes.contains("remote-ebpf-status-summary.json"));
    assert!(cli_recipes.contains("schema_version"));
    assert!(cli_recipes.contains("failure_code"));
    assert!(cli_recipes.contains("validation_timeout"));
    assert!(cli_recipes.contains("remote_host_wrong_arch"));
    assert!(entrypoints.contains("The evidence shelf now also writes `evidence-index.json`"));
    assert!(read_repo_file("docs/performance-baselines.md").contains("workspace_sync <= 8s"));
    assert!(
        read_repo_file("docs/fixtures/gewyvern_validate_list.json")
            .contains("\"schema_version\": 1")
    );
    assert!(
        read_repo_file("docs/fixtures/gewyvern_validate_release_gate_minimal.json")
            .contains("\"command\": \"release-gate\"")
    );
    assert!(
        read_repo_file("docs/fixtures/gewyvern_validate_invalid_cli_input.json")
            .contains("\"failure_code\": \"invalid_cli_input\"")
    );
}
