use super::*;

#[test]
fn explain_report_compact_ir_focus_includes_model_compare_summary() {
    let report =
        compile_explain_report_file(&protocol_fixture_path("amqp/publish/main.gewy")).unwrap();
    let text = render_explain_report_with_options(
        &report,
        RenderFormat::Text,
        Some(ExplainFocus::Ir),
        true,
    );

    assert!(text.contains("focus=ir"));
    assert!(text.contains("ir program_rules=6 reason_rules=6"));
    assert!(text.contains("ir_compare rule_delta=0 supported_delta=0"));
    assert!(text.contains("shared_modules=amqp_basic_publish_path"));
    assert!(text.contains(
        "shared_phases=bind,connect,establish,receive_ack,resolve_upstream,send_publish"
    ));
}
