use super::*;

#[test]
fn compile_explain_report_file_materializes_human_summary_surface() {
    let report =
        compile_explain_report_file("/Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy")
            .unwrap();
    assert!(report.ok);
    assert!(report.binding.is_some());
    assert!(report.frontend.is_some());
    assert!(report.findings.findings.is_empty());
    let text = render_explain_report(&report, RenderFormat::Text);
    let json = render_explain_report(&report, RenderFormat::Json);
    assert!(text.contains("surface=explain"));
    assert!(text.contains("validation:"));
    assert!(text.contains("next_step="));
    assert!(json.contains("\"surface_id\":\"gewyc.explain\""));
    assert!(json.contains(
        "\"schema_hint\":{\"family\":\"gewyc\",\"surface\":\"explain\",\"schema_version\":1}"
    ));
    assert!(json.contains("\"contract_hint\":{\"stability\":\"candidate\",\"compatibility\":\"grouped_payload_preferred\",\"legacy_fields\":\"retained_in_payload\"}"));
    assert!(json.contains("\"summary\""));
    assert!(json.contains("\"next_step\""));
}

#[test]
fn explain_report_suggests_frontend_for_parse_failure() {
    let report = compile_explain_report_str(
        r#"
template(:broken_parse)
|> window(:default_5s)
|> use(:missing_function)
"#,
    );
    let text = render_explain_report(&report, RenderFormat::Text);
    assert!(text.contains("next_step=fix the parse finding first"));
    assert!(text.contains("gewyc frontend"));
}

#[test]
fn explain_report_includes_parse_source_excerpt() {
    let report = compile_explain_report_str(
        r#"
template(:demo)
fn broken( =
  |> fragment(:udp_packet_meta_fragment)
"#,
    );
    let text = render_explain_report(&report, RenderFormat::Text);
    let json = render_explain_report(&report, RenderFormat::Json);
    assert!(text.contains("parse_source_excerpt=fn broken( ="));
    assert!(text.contains("parse_source_marker="));
    assert!(json.contains("\"parse_source_excerpt\""));
    assert!(json.contains("\"line_text\":\"fn broken( =\""));
}

#[test]
fn explain_report_suggests_unsupported_offsets_for_validation_failure() {
    let report = compile_explain_report_str(
        r#"
template(:broken_offsets)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> operation(:snmp_query)
|> program_model(:broken_offsets_model)
|> program_rule(predicate: "packet_observed:tcp:remote:mysql:byte_at:42:255:1", stage: :packet_observed, narrative: "static:test", dedupe: true)
"#,
    );
    let text = render_explain_report(&report, RenderFormat::Text);
    let json = render_explain_report(&report, RenderFormat::Json);
    assert!(text.contains("unsupported_payload_offsets"));
    assert!(text.contains("validation_excerpt=model:broken_offsets_model rule:0"));
    assert!(text.contains("validation_note="));
    assert!(text.contains("adjust fragment coverage or payload matchers"));
    assert!(json.contains("unsupported_payload_offsets"));
    assert!(json.contains("\"validation_excerpt\""));
    assert!(json.contains("\"validation_shape_note\""));
    assert!(json.contains("\"model\":\"broken_offsets_model\""));
}

#[test]
fn explain_report_includes_diagnostics_excerpt_for_rule_support_failures() {
    let report = compile_explain_report_str(
        r#"
template(:broken_offset_validation)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> program_model(:broken_offset_validation_model)
|> operation(:snmp_get)
|> program_rule(predicate: "datagram_observed:udp:remote:snmp:byte_at:8:0xff:0xa0", stage: :datagram_observed, narrative: "static:snmp seen", dedupe: true)
"#,
    );
    let text = render_explain_report_with_focus(
        &report,
        RenderFormat::Text,
        Some(ExplainFocus::Diagnostics),
    );
    let json = render_explain_report(&report, RenderFormat::Json);
    assert!(text.contains("diagnostics_excerpt=model:broken_offset_validation_model"));
    assert!(text.contains("diagnostics_note="));
    assert!(text.contains("offsets:[8]"));
    assert!(json.contains("\"diagnostics_excerpt\""));
    assert!(json.contains("\"diagnostics_shape_note\""));
    assert!(json.contains("\"model\":\"broken_offset_validation_model\""));
}

#[test]
fn explain_report_surfaces_frontend_docs_in_summary_and_focus() {
    let report = compile_explain_report_str(
        r#"
//! UDP authoring demo
//! Keeps the module intent obvious
/// Shared UDP rules
fn udp_rules() =
  |> operation(:datagram_exchange)
  |> program_model(:frontend_docs_model)
  |> program_rule(predicate: :process_bound, stage: :process_bound, narrative: :process_bound, dedupe: true, module: :frontend_docs, phase: :bind)

/// Entry template for frontend docs
template(:frontend_docs)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> fragment(:route_meta_fragment)
|> fragment(:sock_lineage_fragment)
|> use(:udp_rules)
"#,
    );

    let text = render_explain_report(&report, RenderFormat::Text);
    let compact = render_explain_report_with_options(
        &report,
        RenderFormat::Text,
        Some(ExplainFocus::Frontend),
        true,
    );
    let focused =
        render_explain_report_with_focus(&report, RenderFormat::Text, Some(ExplainFocus::Frontend));
    let json = render_explain_report(&report, RenderFormat::Json);

    assert!(text.contains(
        "authoring=module_doc=UDP authoring demo / Keeps the module intent obvious ; template_doc=Entry template for frontend docs ; documented_functions=udp_rules"
    ));
    assert!(text.contains("- module_doc=UDP authoring demo / Keeps the module intent obvious"));
    assert!(text.contains("- template_doc=Entry template for frontend docs"));
    assert!(text.contains("- documented_functions=udp_rules"));
    assert!(compact.contains("module_doc=UDP authoring demo / Keeps the module intent obvious"));
    assert!(compact.contains("template_doc=Entry template for frontend docs"));
    assert!(compact.contains("documented_functions=udp_rules"));
    assert!(focused.contains("module_doc=UDP authoring demo / Keeps the module intent obvious"));
    assert!(focused.contains("template_doc=Entry template for frontend docs"));
    assert!(focused.contains("doc: Shared UDP rules"));
    assert!(json.contains("\"authoring_context\":{\"module_doc\":\"UDP authoring demo\\nKeeps the module intent obvious\",\"template_doc\":\"Entry template for frontend docs\",\"documented_functions\":[\"udp_rules\"]}"));
    assert!(
        json.contains("\"stage_status\":{\"parse\":true,\"validation\":true,\"diagnostics\":true}")
    );
    assert!(json.contains("\"analysis\":{\"authoring_context\":{\"module_doc\":\"UDP authoring demo\\nKeeps the module intent obvious\",\"template_doc\":\"Entry template for frontend docs\",\"documented_functions\":[\"udp_rules\"]}"));
    assert!(json.contains("\"shape_notes\":{\"binding\":"));
    assert!(
        json.contains(
            "\"excerpts\":{\"parse_source\":null,\"validation\":null,\"diagnostics\":null}"
        )
    );
    assert!(
        json.contains("\"module_doc\":\"UDP authoring demo\\nKeeps the module intent obvious\"")
    );
    assert!(json.contains("\"template_doc\":\"Entry template for frontend docs\""));
}

#[test]
fn explain_focus_json_uses_structured_groups() {
    let report = compile_explain_report_str(
        r#"
//! Focus JSON demo
/// Shared UDP rules
fn udp_rules() =
  |> operation(:datagram_exchange)
  |> program_model(:focus_json_model)
  |> program_rule(predicate: :process_bound, stage: :process_bound, narrative: :process_bound, dedupe: true, module: :focus_json, phase: :bind)

template(:focus_json_demo)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> use(:udp_rules)
"#,
    );

    let frontend_json =
        render_explain_report_with_focus(&report, RenderFormat::Json, Some(ExplainFocus::Frontend));
    let binding_json =
        render_explain_report_with_focus(&report, RenderFormat::Json, Some(ExplainFocus::Binding));

    assert!(frontend_json.contains("\"focused_report\":{\"kind\":\"frontend\""));
    assert!(frontend_json.contains("\"status\":{\"present\":true}"));
    assert!(
        frontend_json
            .contains("\"analysis\":{\"authoring_context\":{\"module_doc\":\"Focus JSON demo\"")
    );
    assert!(binding_json.contains("\"focused_report\":{\"kind\":\"binding\""));
    assert!(binding_json.contains("\"analysis\":{\"lowered_binding_summary\":"));
    assert!(binding_json.contains("\"shape_notes\":{\"binding\":"));
}

#[test]
fn binding_and_diagnostics_json_surface_status_and_counts() {
    let report =
        compile_explain_report_file("/Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy")
            .unwrap();
    let binding = report
        .binding
        .as_ref()
        .expect("binding report should exist");
    let diagnostics = report
        .diagnostics
        .as_ref()
        .expect("diagnostics report should exist");

    let binding_json = render_binding_report(binding, RenderFormat::Json);
    let diagnostics_json = render_diagnostics_report(diagnostics, RenderFormat::Json);

    assert!(binding_json.contains("\"status\":{\"has_window\":true"));
    assert!(binding_json.contains("\"counts\":{\"fragments\":3"));
    assert!(diagnostics_json.contains("\"status\":{\"has_program_model\":true"));
    assert!(diagnostics_json.contains("\"counts\":{\"fragments\":3"));
}
