use super::*;

#[test]
fn binding_json_mentions_template_id() {
    let binding = compile_binding_file(&dsl_fixture_path("udp_process_debug.gewy")).unwrap();
    let json = render_binding(&binding, RenderFormat::Json);
    assert!(json.contains("\"surface_id\":\"gewyc.binding\""));
    assert!(json.contains(
        "\"schema_hint\":{\"family\":\"gewyc\",\"surface\":\"binding\",\"schema_version\":1}"
    ));
    assert!(json.contains("\"contract_hint\":{\"stability\":\"candidate\",\"compatibility\":\"grouped_payload_preferred\",\"legacy_fields\":\"retained_in_payload\"}"));
    assert!(json.contains("\"template_id\":\"udp_process_debug\""));
    assert!(json.contains("\"program_model\""));
}

#[test]
fn diagnostics_text_mentions_program_rule() {
    let binding = compile_binding_file(&dsl_fixture_path("udp_process_debug.gewy")).unwrap();
    let diagnostics = collect_binding_diagnostics(&binding).unwrap();
    let text = render_diagnostics(&binding, &diagnostics, RenderFormat::Text);
    assert!(text.contains("program_model="));
    assert!(text.contains("program_rule["));
}

#[test]
fn binding_report_is_owned_and_stable() {
    let binding = compile_binding_file(&dsl_fixture_path("udp_process_debug.gewy")).unwrap();
    let report = binding_report(&binding);
    assert_eq!(report.template_id, "udp_process_debug");
    assert!(
        report
            .fragments
            .contains(&"udp_packet_meta_fragment".to_string())
    );
    assert!(
        report
            .fragment_params
            .iter()
            .any(|param| param.fragment == "sock_lineage_fragment" && param.key == "capture_comm")
    );
}

#[test]
fn compile_diagnostics_report_file_materializes_reason_and_program_models() {
    let report =
        compile_diagnostics_report_file(&dsl_fixture_path("udp_process_debug.gewy")).unwrap();
    assert_eq!(report.template_id, "udp_process_debug");
    assert!(
        report
            .fragments
            .contains(&"udp_packet_meta_fragment".to_string())
    );
    assert!(report.program_model.is_some());
}

#[test]
fn compile_envelope_str_collects_all_frontend_surfaces() {
    let input = crate::dsl::read_file(&dsl_fixture_path("udp_process_debug.gewy")).unwrap();
    let envelope = compile_envelope_str(&input);
    assert_eq!(
        envelope
            .binding
            .as_ref()
            .map(|report| report.template_id.as_str()),
        Some("udp_process_debug")
    );
    assert_eq!(
        envelope
            .diagnostics
            .as_ref()
            .and_then(|report| report.program_model.as_ref())
            .map(|_| true),
        Some(true)
    );
    assert!(envelope.findings.findings.is_empty());
    assert!(envelope.stages.parse.ok);
    assert!(envelope.stages.validation.ok);
    assert!(envelope.stages.diagnostics.ok);
}

#[test]
fn compile_envelope_str_keeps_findings_and_stages_in_sync_for_parse_failure() {
    let envelope = compile_envelope_str(
        r#"
template(:broken)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> oops(:true)
"#,
    );
    assert!(envelope.binding.is_none());
    assert!(envelope.diagnostics.is_none());
    assert_eq!(envelope.findings.findings.len(), 1);
    assert_eq!(
        envelope.findings.findings[0],
        envelope.stages.parse.finding.clone().unwrap()
    );
}

#[test]
fn compile_stages_report_file_separates_binding_and_diagnostics_reports() {
    let report = compile_stages_report_file(&dsl_fixture_path("udp_process_debug.gewy")).unwrap();
    assert!(report.parse.ok);
    assert!(report.parse.finding.is_none());
    assert_eq!(
        report.parse.report.as_ref().unwrap().template_id,
        "udp_process_debug"
    );
    assert!(report.diagnostics.ok);
    assert_eq!(
        report.diagnostics.report.as_ref().unwrap().template_id,
        "udp_process_debug"
    );
    assert!(report.validation.ok);
    assert!(report.validation.finding.is_none());
    assert_eq!(report.validation.registry, "builtin");
    assert_eq!(report.validation.fragment_count, 3);
    assert!(report.validation.program_rule_count > 0);
    assert!(
        report
            .validation
            .checks
            .contains(&"rule_evidence".to_string())
    );
    assert!(
        report
            .parse
            .report
            .as_ref()
            .unwrap()
            .program_model
            .is_some()
    );
    assert!(
        report
            .diagnostics
            .report
            .as_ref()
            .unwrap()
            .program_model
            .is_some()
    );
}

#[test]
fn compile_stages_report_str_keeps_parse_failure_as_stage_finding() {
    let report = compile_stages_report_str(
        r#"
template(:broken)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> oops(:true)
"#,
    );
    assert!(!report.parse.ok);
    assert!(report.parse.report.is_none());
    assert_eq!(
        report
            .parse
            .finding
            .as_ref()
            .map(|finding| finding.code.as_str()),
        Some("GEWYC-PARSE-INVALID-VALUE")
    );
    assert!(!report.validation.ok);
    assert!(report.validation.finding.is_none());
    assert!(!report.diagnostics.ok);
    assert!(report.diagnostics.finding.is_none());
}

#[test]
fn compile_stages_report_file_keeps_partial_report_on_validation_failure() {
    let path = "/tmp/gewyc-validation-failure.gewy";
    std::fs::write(
        path,
        r#"
template(:broken_offset_validation)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> program_model(:broken_offset_validation_model)
|> operation(:snmp_get)
|> program_rule(predicate: "datagram_observed:udp:remote:snmp:byte_at:8:0xff:0xa0", stage: :datagram_observed, narrative: "static:snmp seen", dedupe: true)
"#,
    )
    .unwrap();
    let report = compile_stages_report_file(path).unwrap();
    assert!(!report.validation.ok);
    assert_eq!(
        report
            .validation
            .finding
            .as_ref()
            .map(|finding| finding.code.as_str()),
        Some("GEWYC-VALIDATE-UNSUPPORTED-PAYLOAD-OFFSETS")
    );
    assert!(report.diagnostics.ok);
    let diagnostics = report.diagnostics.report.as_ref().unwrap();
    let program_model = diagnostics.program_model.as_ref().unwrap();
    assert_eq!(program_model.rules[0].unsupported_payload_offsets, vec![8]);
}
