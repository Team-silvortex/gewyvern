mod command;
mod container_packaging;
mod debugger_cross;
mod demo_roundtrip;
mod field_smoke;
mod high_frequency;
mod linux_ebpf;
mod registry;
mod release_gate;
mod remote_host;
mod resilience;
mod runtime_lifecycle;
mod runtime_operator;
mod stack_probe;
mod stack_suites;

pub use command::{ValidationError, ValidationReport, repo_root};
pub use container_packaging::{
    run_container_operator_path_validation, run_container_protocol_validation,
    run_container_runtime_validation, run_container_validation_summary, run_package_install_smoke,
};
pub use debugger_cross::run_debugger_cross_validation;
pub use demo_roundtrip::{
    run_external_engine_roundtrip_demo, run_socket_roundtrip_demo,
    run_training_dataset_roundtrip_demo,
};
pub use field_smoke::run_field_smoke_validation;
pub use high_frequency::run_high_frequency_validation;
pub use linux_ebpf::{run_linux_attach_smoke, run_linux_kprobe_smoke, run_linux_tc_smoke};
pub use registry::run_registry_validation;
pub use release_gate::{
    ReleaseCheckMode, ReleaseGateOptions, run_release_container_check, run_release_gate,
};
pub use remote_host::{RemoteLinuxHostOptions, run_remote_linux_host_validation};
pub use resilience::{
    run_resilience_bundle_validation, run_resilience_drive_bad_json_validation,
    run_resilience_emit_helper_validation, run_resilience_log_evidence_validation,
    run_resilience_roundtrip_validation,
};
pub use runtime_lifecycle::run_runtime_lifecycle_validation;
pub use runtime_operator::run_runtime_operator_validation;
pub use stack_probe::{
    run_stack_json_file_validation, run_stack_probe_validation, run_stack_register_runtime_json,
    write_stack_resilience_summary,
};
pub use stack_suites::{run_pathological_container_validation, run_three_module_stack_smoke};
