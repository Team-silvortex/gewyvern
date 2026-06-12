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
