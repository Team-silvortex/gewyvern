use super::*;

fn udp_debug_path() -> String {
    dsl_fixture_path("udp_process_debug.gewy")
}

fn amqp_publish_path() -> String {
    protocol_fixture_path("amqp/publish/main.gewy")
}

#[test]
fn blessed_wrapper_fields_exist_for_binding_surface() {
    let binding = compile_binding_file(&udp_debug_path()).unwrap();
    let expected_fingerprint = binding_report(&binding).fingerprint();
    let json = render_binding(&binding, RenderFormat::Json);
    let document: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert_valid_json_document(&json);
    assert!(json.contains("\"surface_id\":\"gewyc.binding\""));
    assert!(json.contains(
        "\"schema_hint\":{\"family\":\"gewyc\",\"surface\":\"binding\",\"schema_version\":1}"
    ));
    assert!(json.contains("\"contract_hint\":{\"stability\":\"candidate\",\"compatibility\":\"grouped_payload_preferred\",\"legacy_fields\":\"retained_in_payload\"}"));
    assert!(json.contains("\"payload\":{"));
    assert!(json.contains(
        "\"language_contract\":{\"language\":\"gewylang\",\"syntax_version\":1,\"stage\":\"binding_ir\",\"stage_version\":1}"
    ));
    assert!(json.contains("\"template_id\":\"udp_process_debug\""));
    assert!(json.contains("\"window\":{\"id\":\"inline\""));
    assert!(json.contains("\"duration_ms\":5000"));
    assert!(json.contains("\"lateness_ms\":200"));
    assert!(json.contains("\"reason_profile\":{\"kind\":\"builtin\""));
    assert!(json.contains("\"id\":\"udp_datagram_l1\""));
    assert_eq!(document["payload"]["fingerprint"]["algorithm"], "sha256");
    assert_eq!(document["payload"]["fingerprint"]["encoding_version"], 1);
    assert_eq!(
        document["payload"]["fingerprint"]["digest"],
        expected_fingerprint.digest_hex()
    );
}

#[test]
fn blessed_grouped_fields_exist_for_explain_surface() {
    let report = compile_explain_report_file(&udp_debug_path()).unwrap();
    let json = render_explain_report(&report, RenderFormat::Json);

    assert_valid_json_document(&json);
    assert!(json.contains("\"surface_id\":\"gewyc.explain\""));
    assert!(json.contains("\"payload\":{"));
    assert!(json.contains("\"summary\":{"));
    assert!(
        json.contains("\"stage_status\":{\"parse\":true,\"validation\":true,\"diagnostics\":true}")
    );
    assert!(json.contains("\"analysis\":{"));
    assert!(json.contains("\"shape_notes\":{"));
    assert!(json.contains("\"excerpts\":{"));
    assert!(json.contains("\"next_step\""));
}

#[test]
fn blessed_grouped_fields_exist_for_ir_history_surface() {
    let report = compile_explain_report_file(&amqp_publish_path()).unwrap();
    let ir_report = report.ir_report.as_ref().expect("ir report should exist");
    let expected_fingerprint = ir_report.fingerprint();
    let json = render_ir_history_snapshot(ir_report, RenderFormat::Json);
    let document: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert_valid_json_document(&json);
    assert!(json.contains("\"surface_id\":\"gewyc.ir_history_snapshot\""));
    assert!(json.contains("\"payload\":{"));
    assert!(json.contains(
        "\"language_contract\":{\"language\":\"gewylang\",\"syntax_version\":1,\"stage\":\"analysis_ir\",\"stage_version\":1}"
    ));
    assert!(json.contains("\"template_id\":\"amqp_basic_publish_path\""));
    assert!(json.contains("\"operation\":\"amqp_basic_publish\""));
    assert!(json.contains("\"program_model\":{"));
    assert!(json.contains("\"reason_model\":{"));
    assert!(json.contains("\"model_compare\":{"));
    assert_eq!(
        document["payload"]["source_ir_fingerprint"]["digest"],
        expected_fingerprint.digest_hex()
    );
}

#[test]
fn direct_ir_surface_has_a_distinct_analysis_contract() {
    let report = compile_ir_report_file(&amqp_publish_path()).unwrap();
    let expected_fingerprint = report.fingerprint();
    let json = render_ir_report(&report, RenderFormat::Json);
    let text = render_ir_report(&report, RenderFormat::Text);
    let document: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert_valid_json_document(&json);
    assert!(json.contains("\"surface_id\":\"gewyc.ir\""));
    assert!(json.contains(
        "\"language_contract\":{\"language\":\"gewylang\",\"syntax_version\":1,\"stage\":\"analysis_ir\",\"stage_version\":1}"
    ));
    assert!(text.starts_with(
        "language_contract=gewylang syntax_version=1 stage=analysis_ir stage_version=1"
    ));
    assert_eq!(
        document["payload"]["fingerprint"]["digest"],
        expected_fingerprint.digest_hex()
    );
    assert!(text.contains(&format!("fingerprint={expected_fingerprint}")));
}

#[test]
fn compat_binding_fields_remain_available_inside_payload() {
    let binding = compile_binding_file(&udp_debug_path()).unwrap();
    let json = render_binding(&binding, RenderFormat::Json);

    assert!(json.contains("\"template_id\":\"udp_process_debug\""));
    assert!(json.contains("\"window\":{\"id\":\"inline\""));
    assert!(json.contains("\"duration_ms\":5000"));
    assert!(json.contains("\"lateness_ms\":200"));
    assert!(json.contains("\"reason_profile\":{\"kind\":\"builtin\""));
    assert!(json.contains("\"id\":\"udp_datagram_l1\""));
    assert!(json.contains("\"fragments\":[\"udp_packet_meta_fragment\",\"route_meta_fragment\",\"sock_lineage_fragment\"]"));
    assert!(json.contains("\"program_model\":"));
}

#[test]
fn compat_explain_fields_remain_available_inside_payload() {
    let report = compile_explain_report_file(&udp_debug_path()).unwrap();
    let json = render_explain_report(&report, RenderFormat::Json);

    assert!(json.contains("\"summary\":{"));
    assert!(json.contains("\"ok\":true"));
    assert!(
        json.contains(
            "\"summary\":{\"parse_ok\":true,\"validation_ok\":true,\"diagnostics_ok\":true"
        )
    );
    assert!(json.contains("\"analysis\":{\"authoring_context\":"));
    assert!(
        json.contains(
            "\"excerpts\":{\"parse_source\":null,\"validation\":null,\"diagnostics\":null}"
        )
    );
    assert!(json.contains("\"authoring_context\":"));
}

#[test]
fn compat_ir_history_fields_remain_available_inside_payload() {
    let report = compile_explain_report_file(&amqp_publish_path()).unwrap();
    let ir_report = report.ir_report.as_ref().expect("ir report should exist");
    let json = render_ir_history_snapshot(ir_report, RenderFormat::Json);

    assert!(json.contains("\"template_id\":\"amqp_basic_publish_path\""));
    assert!(json.contains("\"program_model\":{\"id\":\"amqp_basic_publish_path_dsl_model\""));
    assert!(json.contains("\"kind\":\"program_model\""));
    assert!(json.contains("\"reason_model\":{\"id\":\"amqp_basic_publish_path_reason\""));
    assert!(json.contains("\"model_compare\":{"));
    assert!(json.contains("\"shared_modules\":"));
}
