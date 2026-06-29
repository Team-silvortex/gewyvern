mod command;
mod debugger_cross;
mod demo_roundtrip;
mod field_smoke;
mod high_frequency;
mod registry;
mod resilience;
mod runtime_lifecycle;
mod runtime_operator;
mod stack_probe;

pub use command::{ValidationError, ValidationReport, repo_root};
pub use debugger_cross::run_debugger_cross_validation;
pub use demo_roundtrip::{
    run_external_engine_roundtrip_demo, run_socket_roundtrip_demo,
    run_training_dataset_roundtrip_demo,
};
pub use field_smoke::run_field_smoke_validation;
pub use high_frequency::run_high_frequency_validation;
pub use registry::run_registry_validation;
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
