use super::*;

#[test]
fn explain_ir_focus_reports_builtin_reason_and_lowered_program_shape() {
    let report =
        compile_explain_report_file("/Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy")
            .unwrap();
    let text =
        render_explain_report_with_focus(&report, RenderFormat::Text, Some(ExplainFocus::Ir));
    let json =
        render_explain_report_with_focus(&report, RenderFormat::Json, Some(ExplainFocus::Ir));

    assert!(text.contains("focus=ir"));
    assert!(text.contains("ir_delta.frontend_functions=1"));
    assert!(text.contains("ir_delta.lowered_program_rules=3"));
    assert!(text.contains("ir_delta.lowered_reason_rules=0"));
    assert!(text.contains("program_model=udp_process_debug_dsl_model"));
    assert!(text.contains("program_model_operation=datagram_exchange"));
    assert!(text.contains("reason_model=udp_datagram_l1 kind=builtin_reason_profile rules=0"));
    assert!(json.contains("\"kind\":\"ir\""));
    assert!(json.contains("\"ir_lowering_delta\":{"));
    assert!(json.contains("\"frontend_function_count\":1"));
    assert!(json.contains("\"id\":\"udp_process_debug_dsl_model\""));
    assert!(json.contains("\"kind\":\"builtin_reason_profile\""));
}

#[test]
fn explain_ir_focus_reports_protocol_phase_and_support_details() {
    let report = compile_explain_report_file(
        "/Users/Shared/chroot/dev/gewyvern/protocols/amqp/publish/main.gewy",
    )
    .unwrap();
    let text =
        render_explain_report_with_focus(&report, RenderFormat::Text, Some(ExplainFocus::Ir));
    let json =
        render_explain_report_with_focus(&report, RenderFormat::Json, Some(ExplainFocus::Ir));

    assert!(text.contains("focus=ir"));
    assert!(text.contains("ir_delta.lowered_modules=amqp_basic_publish_path"));
    assert!(text.contains(
        "ir_delta.lowered_phases=bind,connect,establish,receive_ack,resolve_upstream,send_publish"
    ));
    assert!(text.contains(
        "ir_delta.lowered_phase_kinds=bind_process,emit_payload,establish_connection,initiate_connection,receive_payload,resolve_route"
    ));
    assert!(text.contains("program_model_operation=amqp_basic_publish"));
    assert!(text.contains("phase=send_publish phase_kind=emit_payload"));
    assert!(text.contains("supporting=tcp_packet_meta_fragment"));
    assert!(text.contains(
        "reason_model=amqp_basic_publish_path_reason kind=declarative_reason_model rules=6"
    ));
    assert!(text.contains("ir_delta.model.program_model.id=amqp_basic_publish_path_dsl_model"));
    assert!(text.contains("ir_delta.model.program_model.modules=amqp_basic_publish_path"));
    assert!(text.contains("ir_delta.model.reason_model.id=amqp_basic_publish_path_reason"));
    assert!(json.contains("\"lowered_modules\":[\"amqp_basic_publish_path\"]"));
    assert!(json.contains(
        "\"lowered_phase_kinds\":[\"bind_process\",\"emit_payload\",\"establish_connection\",\"initiate_connection\",\"receive_payload\",\"resolve_route\"]"
    ));
    assert!(json.contains("\"lowered_models\":[{"));
    assert!(json.contains("\"label\":\"program_model\""));
    assert!(json.contains("\"supported_rule_count\":6"));
    assert!(json.contains("\"label\":\"reason_model\""));
    assert!(json.contains("\"phase_kind\":\"emit_payload\""));
    assert!(json.contains("\"supporting_fragments\":[\"tcp_packet_meta_fragment\"]"));
    assert!(json.contains("\"kind\":\"declarative_reason_model\""));
}
