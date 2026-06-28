use super::*;

#[test]
fn compile_findings_report_str_surfaces_parse_failures() {
    let report = compile_findings_report_str(
        r#"
template(:broken)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> oops(:true)
"#,
    );
    assert_eq!(report.findings.len(), 1);
    assert_eq!(report.findings[0].stage, CompilerFindingStage::Parse);
    assert_eq!(report.findings[0].code, "GEWYC-PARSE-INVALID-VALUE");
    assert_eq!(report.findings[0].severity, CompilerFindingSeverity::Error);
    assert_eq!(report.findings[0].line, Some(6));
    assert!(
        report.findings[0]
            .message
            .contains("unknown pipeline DSL step 'oops'")
    );
}

#[test]
fn compile_findings_report_str_surfaces_validation_failures() {
    let report = compile_findings_report_str(
        r#"
template(:broken_validation)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:route_meta_fragment)
|> program_model(:broken_validation_model)
|> operation(:dns_lookup)
|> program_rule(predicate: "datagram_observed:udp", stage: :datagram_observed, narrative: "static:udp seen", dedupe: true)
"#,
    );
    assert_eq!(report.findings.len(), 1);
    assert_eq!(report.findings[0].stage, CompilerFindingStage::Validation);
    assert_eq!(
        report.findings[0].code,
        "GEWYC-VALIDATE-MISSING-RULE-EVIDENCE"
    );
    assert_eq!(report.findings[0].severity, CompilerFindingSeverity::Error);
    assert_eq!(report.findings[0].line, None);
    assert!(report.findings[0].message.contains("MissingRuleEvidence"));
}

#[test]
fn compile_findings_report_str_surfaces_unsupported_payload_offset_failures() {
    let report = compile_findings_report_str(
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
    assert_eq!(report.findings.len(), 1);
    assert_eq!(report.findings[0].stage, CompilerFindingStage::Validation);
    assert_eq!(
        report.findings[0].code,
        "GEWYC-VALIDATE-UNSUPPORTED-PAYLOAD-OFFSETS"
    );
    assert_eq!(report.findings[0].severity, CompilerFindingSeverity::Error);
    assert_eq!(report.findings[0].line, None);
    assert!(
        report.findings[0]
            .message
            .contains("UnsupportedRulePayloadOffsets")
    );
}

#[test]
fn compile_findings_report_str_is_empty_when_pipeline_succeeds() {
    let input = crate::dsl::read_file(&dsl_fixture_path("udp_process_debug.gewy")).unwrap();
    let report = compile_findings_report_str(&input);
    assert!(report.findings.is_empty());
}

#[test]
fn compile_findings_report_str_uses_specific_code_for_unknown_pipeline_function() {
    let report = compile_findings_report_str(
        r#"
template(:broken_pipeline_use)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> use(:missing_core)
"#,
    );
    assert_eq!(report.findings.len(), 1);
    assert_eq!(
        report.findings[0].code,
        "GEWYC-PARSE-UNKNOWN-PIPELINE-FUNCTION"
    );
    assert_eq!(report.findings[0].line, Some(5));
    assert!(
        report.findings[0]
            .message
            .contains("unknown pipeline function 'missing_core'")
    );
}

#[test]
fn compile_findings_report_file_uses_specific_code_for_unknown_package_dependency() {
    let package_dir =
        std::env::temp_dir().join(format!("gewyc-missing-dependency-{}", std::process::id()));
    std::fs::create_dir_all(&package_dir).unwrap();
    std::fs::write(
        package_dir.join("gewy.pkg"),
        "name=missing_dependency_pkg\nversion=0.1.0\nentry=main.gewy\n",
    )
    .unwrap();
    std::fs::write(
        package_dir.join("main.gewy"),
        r#"
template(:missing_dependency_pkg)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> include("missing_dep:module.gewy")
"#,
    )
    .unwrap();

    let report = compile_findings_report_file(package_dir.to_str().unwrap());
    assert_eq!(report.findings.len(), 1);
    assert_eq!(
        report.findings[0].code,
        "GEWYC-PARSE-UNKNOWN-PACKAGE-DEPENDENCY"
    );
    assert_eq!(report.findings[0].line, Some(5));
    assert!(
        report.findings[0]
            .message
            .contains("unknown package dependency 'missing_dep'")
    );
}

#[test]
fn compile_findings_report_str_uses_specific_code_for_nonfilesystem_include() {
    let report = compile_findings_report_str(
        r#"
template(:include_without_package)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> include("./module.gewy")
"#,
    );
    assert_eq!(report.findings.len(), 1);
    assert_eq!(
        report.findings[0].code,
        "GEWYC-PARSE-INCLUDE-NONFILESYSTEM-ENTRY"
    );
    assert_eq!(report.findings[0].line, Some(5));
}

#[test]
fn compile_findings_report_str_uses_specific_code_for_invalid_function_body() {
    let report = compile_findings_report_str(
        r#"
fn udp_core() {
  fragment(:udp_packet_meta_fragment)
}

template(:invalid_function_body)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> use(:udp_core)
"#,
    );
    assert_eq!(report.findings.len(), 1);
    assert_eq!(report.findings[0].code, "GEWYC-PARSE-INVALID-FUNCTION-BODY");
    assert_eq!(report.findings[0].line, Some(3));
}

#[test]
fn compile_findings_report_str_uses_specific_code_for_unclosed_function_block() {
    let report = compile_findings_report_str(
        r#"
fn udp_core() {
  |> fragment(:udp_packet_meta_fragment)
"#,
    );
    assert_eq!(report.findings.len(), 1);
    assert_eq!(
        report.findings[0].code,
        "GEWYC-PARSE-UNCLOSED-FUNCTION-BLOCK"
    );
    assert_eq!(report.findings[0].line, Some(2));
}

#[test]
fn findings_json_includes_code_severity_and_line() {
    let report = compile_findings_report_str(
        r#"
template(:broken)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> oops(:true)
"#,
    );
    let json = render_findings_report(&report, RenderFormat::Json);
    assert!(json.contains("\"surface_id\":\"gewyc.findings\""));
    assert!(json.contains(
        "\"schema_hint\":{\"family\":\"gewyc\",\"surface\":\"findings\",\"schema_version\":1}"
    ));
    assert!(json.contains("\"contract_hint\":{\"stability\":\"candidate\",\"compatibility\":\"grouped_payload_preferred\",\"legacy_fields\":\"retained_in_payload\"}"));
    assert!(json.contains("\"code\":\"GEWYC-PARSE-INVALID-VALUE\""));
    assert!(json.contains("\"severity\":\"error\""));
    assert!(json.contains("\"line\":6"));
}

#[test]
fn stage_local_finding_json_matches_standalone_findings_shape() {
    let stages = compile_stages_report_str(
        r#"
template(:broken)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> oops(:true)
"#,
    );
    let standalone = compile_findings_report_str(
        r#"
template(:broken)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> oops(:true)
"#,
    );
    let standalone_finding = standalone.findings.first().unwrap();
    let stages_json = render_stages_report(&stages, RenderFormat::Json);
    let expected = finding_json_record(standalone_finding);
    assert!(stages_json.contains(&format!("\"finding\":{expected}")));
}

#[test]
fn stage_local_finding_keeps_specific_frontend_parse_code() {
    let stages = compile_stages_report_str(
        r#"
template(:broken_pipeline_use)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> use(:missing_core)
"#,
    );
    assert_eq!(
        stages
            .parse
            .finding
            .as_ref()
            .map(|finding| finding.code.as_str()),
        Some("GEWYC-PARSE-UNKNOWN-PIPELINE-FUNCTION")
    );
    assert_eq!(
        stages
            .parse
            .finding
            .as_ref()
            .and_then(|finding| finding.column),
        None
    );
}

#[test]
fn parse_findings_surface_column_for_invalid_function_signature() {
    let report = compile_findings_report_str(
        r#"
fn broken =
template(:broken)
|> use(:broken)
"#,
    );
    let finding = report.findings.first().expect("parse finding");
    assert_eq!(finding.line, Some(2));
    assert_eq!(finding.column, Some(10));
    let text = render_findings_report(&report, RenderFormat::Text);
    let json = render_findings_report(&report, RenderFormat::Json);
    assert!(text.contains("line=2 column=10"));
    assert!(json.contains("\"line\":2"));
    assert!(json.contains("\"column\":10"));
}

#[test]
fn parse_findings_surface_column_for_invalid_let_binding() {
    let report = compile_findings_report_str(
        r#"
fn demo() =
  let op
template(:demo)
|> use(:demo)
"#,
    );
    let finding = report.findings.first().expect("parse finding");
    assert_eq!(finding.line, Some(3));
    assert_eq!(finding.column, Some(9));
    let text = render_findings_report(&report, RenderFormat::Text);
    let json = render_findings_report(&report, RenderFormat::Json);
    assert!(text.contains("line=3 column=9"));
    assert!(json.contains("\"line\":3"));
    assert!(json.contains("\"column\":9"));
}

#[test]
fn parse_findings_surface_column_for_window_keyword_error() {
    let report = compile_findings_report_str(
        r#"
template(:demo)
|> window(duration_ms: 5000)
|> reason(:udp_datagram_l1)
"#,
    );
    let finding = report.findings.first().expect("parse finding");
    assert_eq!(finding.line, Some(3));
    assert_eq!(finding.column, Some(4));
}

#[test]
fn parse_findings_surface_column_for_program_rule_keyword_error() {
    let report = compile_findings_report_str(
        r#"
template(:demo)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> operation(:datagram_exchange)
|> program_model(:demo_model)
|> program_rule(predicate: "process_bound", stage: :connect_flow, dedupe: true)
"#,
    );
    let finding = report.findings.first().expect("parse finding");
    assert_eq!(finding.line, Some(8));
    assert_eq!(finding.column, Some(4));
}

#[test]
fn parse_findings_surface_column_for_program_rule_invalid_stage_value() {
    let input = r#"
template(:demo)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> operation(:datagram_exchange)
|> program_model(:demo_model)
|> program_rule(predicate: "process_bound", stage: :not_a_stage, narrative: "static:test", dedupe: true)
"#;
    let report = compile_findings_report_str(input);
    let finding = report.findings.first().expect("parse finding");
    let line = input.lines().nth(7).unwrap();
    let expected_column = line.find(":not_a_stage").unwrap() + 1;
    assert_eq!(finding.line, Some(8));
    assert_eq!(finding.column, Some(expected_column));
}

#[test]
fn parse_findings_surface_column_for_program_rule_invalid_predicate_value() {
    let input = r#"
template(:demo)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> operation(:datagram_exchange)
|> program_model(:demo_model)
|> program_rule(predicate: "packet_observed:tcp:remote:notaport", stage: :connect_flow, narrative: "static:test", dedupe: true)
"#;
    let report = compile_findings_report_str(input);
    let finding = report.findings.first().expect("parse finding");
    let line = input.lines().nth(7).unwrap();
    let expected_column = line.find("\"packet_observed").unwrap() + 1;
    assert_eq!(finding.line, Some(8));
    assert_eq!(finding.column, Some(expected_column));
}

#[test]
fn parse_findings_surface_column_for_packet_byte_at_qualifier_error() {
    let input = r#"
template(:demo)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> operation(:datagram_exchange)
|> program_model(:demo_model)
|> program_rule(predicate: "packet_observed:tcp:remote:mysql:byte_at:not_u16:255:1", stage: :connect_flow, narrative: "static:test", dedupe: true)
"#;
    let report = compile_findings_report_str(input);
    let finding = report.findings.first().expect("parse finding");
    let line = input.lines().nth(7).unwrap();
    let expected_column = line.find("byte_at").unwrap();
    assert_eq!(finding.line, Some(8));
    assert_eq!(finding.column, Some(expected_column));
}

#[test]
fn parse_findings_surface_column_for_datagram_bytes_at_missing_sequence() {
    let input = r#"
template(:demo)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> operation(:datagram_exchange)
|> program_model(:demo_model)
|> program_rule(predicate: "datagram_observed:udp:remote:snmp:bytes_at:8", stage: :datagram_observed, narrative: "static:test", dedupe: true)
"#;
    let report = compile_findings_report_str(input);
    let finding = report.findings.first().expect("parse finding");
    let line = input.lines().nth(7).unwrap();
    let expected_column = line.find("bytes_at").unwrap();
    assert_eq!(finding.line, Some(8));
    assert_eq!(finding.column, Some(expected_column));
}

#[test]
fn parse_findings_surface_column_for_socket_state_invalid_port() {
    let input = r#"
template(:demo)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> operation(:datagram_exchange)
|> program_model(:demo_model)
|> program_rule(predicate: "socket_state_observed:remote:notaport", stage: :socket_state_transition, narrative: "static:test", dedupe: true)
"#;
    let report = compile_findings_report_str(input);
    let finding = report.findings.first().expect("parse finding");
    let line = input.lines().nth(7).unwrap();
    let expected_column = line.find("notaport").unwrap();
    assert_eq!(finding.line, Some(8));
    assert_eq!(finding.column, Some(expected_column));
}

#[test]
fn parse_findings_surface_column_for_quic_packet_invalid_type() {
    let input = r#"
template(:demo)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> operation(:datagram_exchange)
|> program_model(:demo_model)
|> program_rule(predicate: "quic_packet_observed:remote:quic:type:not_a_type", stage: :datagram_observed, narrative: "static:test", dedupe: true)
"#;
    let report = compile_findings_report_str(input);
    let finding = report.findings.first().expect("parse finding");
    let line = input.lines().nth(7).unwrap();
    let expected_column = line.find("not_a_type").unwrap();
    assert_eq!(finding.line, Some(8));
    assert_eq!(finding.column, Some(expected_column));
}

#[test]
fn parse_findings_surface_column_for_quic_frame_byte_at_error() {
    let input = r#"
template(:demo)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> operation(:datagram_exchange)
|> program_model(:demo_model)
|> program_rule(predicate: "quic_frame_observed:remote:quic:frame:crypto:byte_at:not_u16:0xff:0xa0", stage: :datagram_observed, narrative: "static:test", dedupe: true)
"#;
    let report = compile_findings_report_str(input);
    let finding = report.findings.first().expect("parse finding");
    let line = input.lines().nth(7).unwrap();
    let expected_column = line.find("byte_at").unwrap();
    assert_eq!(finding.line, Some(8));
    assert_eq!(finding.column, Some(expected_column));
}

#[test]
fn parse_findings_surface_column_for_all_predicate_child_error() {
    let input = r#"
template(:demo)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> operation(:datagram_exchange)
|> program_model(:demo_model)
|> program_rule(predicate: "all(process_bound, packet_observed:tcp:remote:mysql:byte_at:not_u16:255:1)", stage: :connect_flow, narrative: "static:test", dedupe: true)
"#;
    let report = compile_findings_report_str(input);
    let finding = report.findings.first().expect("parse finding");
    let line = input.lines().nth(7).unwrap();
    let expected_column = line.find("byte_at").unwrap();
    assert_eq!(finding.line, Some(8));
    assert_eq!(finding.column, Some(expected_column));
}

#[test]
fn parse_findings_surface_column_for_any_predicate_child_error() {
    let input = r#"
template(:demo)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> operation(:datagram_exchange)
|> program_model(:demo_model)
|> program_rule(predicate: "any(process_bound, quic_packet_observed:remote:quic:type:not_a_type)", stage: :connect_flow, narrative: "static:test", dedupe: true)
"#;
    let report = compile_findings_report_str(input);
    let finding = report.findings.first().expect("parse finding");
    let line = input.lines().nth(7).unwrap();
    let expected_column = line.find("not_a_type").unwrap();
    assert_eq!(finding.line, Some(8));
    assert_eq!(finding.column, Some(expected_column));
}

#[test]
fn stage_local_finding_without_column_stays_shape_compatible() {
    let stages = compile_stages_report_str(
        r#"
template(:broken_pipeline_use)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> use(:missing_core)
"#,
    );
    assert_eq!(
        stages
            .parse
            .finding
            .as_ref()
            .and_then(|finding| finding.line),
        Some(5)
    );
}
