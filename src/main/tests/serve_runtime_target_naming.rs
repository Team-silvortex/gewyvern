use super::{
    Cli, annotate_export_trust, dsl_fixture_path, protocol_fixture_path, run_binding_demo,
};
use crate::serve_runtime::{SOCKET_SESSION_TARGET_NAME, single_runtime_target_name};
use gewyvern::dsl::compile_file;

#[test]
fn single_runtime_target_name_uses_protocol_target_for_builtin_template() {
    let binding = compile_file(&dsl_fixture_path("http_request_path.gewy"))
        .expect("http_request_path DSL should compile");
    let export = annotate_export_trust(
        run_binding_demo(binding),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
    );
    assert_eq!(single_runtime_target_name(&export), "scan:http:request");
}

#[test]
fn single_runtime_target_name_uses_protocol_target_for_packaged_template() {
    let binding = compile_file(&protocol_fixture_path("redis/auth-required"))
        .expect("redis auth-required package should compile");
    let export = annotate_export_trust(
        run_binding_demo(binding),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
    );
    assert_eq!(
        single_runtime_target_name(&export),
        "scan:redis:auth-required"
    );
}

#[test]
fn single_runtime_target_name_falls_back_for_unknown_template() {
    let binding = compile_file(&dsl_fixture_path("udp_process_debug.gewy"))
        .expect("udp_process_debug DSL should compile");
    let export = annotate_export_trust(
        run_binding_demo(binding),
        &Cli::from_args(["--demo".to_string(), "udp".to_string()]).unwrap(),
    );
    assert_eq!(single_runtime_target_name(&export), SOCKET_SESSION_TARGET_NAME);
}
