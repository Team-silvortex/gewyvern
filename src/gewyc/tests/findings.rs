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
    assert_eq!(report.findings[0].code, "GEWYC-PARSE-UNKNOWN-PIPELINE-STEP");
    assert_eq!(report.findings[0].severity, CompilerFindingSeverity::Error);
    assert_eq!(report.findings[0].line, Some(6));
    assert!(
        report.findings[0]
            .message
            .contains("unknown pipeline DSL step 'oops'")
    );
}

#[test]
fn compile_findings_report_str_uses_specific_code_for_unknown_parameter_kind() {
    let report = compile_findings_report_str(
        r#"
fn udp_core(model_name: mystery) =
  |> program_model $model_name

template :unknown_kind
|> window :default_5s
|> reason :udp_datagram_l1
|> use :udp_core, :udp_model
"#,
    );
    let finding = report.findings.first().expect("parse finding");
    assert_eq!(finding.code, "GEWYC-PARSE-UNKNOWN-PARAMETER-KIND");
    assert_eq!(finding.line, Some(2));
    assert!(
        finding
            .message
            .contains("unknown pipeline parameter kind 'mystery'")
    );
}

#[test]
fn compile_findings_report_str_rejects_duplicate_declarations() {
    let report = compile_findings_report_str(
        r#"
fn udp_core() =
  |> fragment(:udp_packet_meta_fragment)

fn udp_core() =
  |> fragment(:route_meta_fragment)

template(:duplicate_function)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
"#,
    );
    let finding = report.findings.first().expect("parse finding");
    assert_eq!(finding.code, "GEWYC-PARSE-DUPLICATE-FUNCTION");
    assert_eq!(finding.line, Some(5));

    let report = compile_findings_report_str(
        r#"
fn udp_core(model: atom, model: atom) =
  |> program_model($model)

template(:duplicate_parameter)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
"#,
    );
    let finding = report.findings.first().expect("parse finding");
    assert_eq!(finding.code, "GEWYC-PARSE-DUPLICATE-PARAMETER");
    assert_eq!(finding.line, Some(2));
}

#[test]
fn compile_findings_report_str_classifies_declaration_topology() {
    let report = compile_findings_report_str(
        r#"
fn udp_core() =
  |> fragment(:udp_packet_meta_fragment)
"#,
    );
    assert_eq!(report.findings[0].code, "GEWYC-PARSE-MISSING-TEMPLATE-HEAD");

    let report = compile_findings_report_str(
        r#"
template(:first)
|> template(:second)
"#,
    );
    assert_eq!(
        report.findings[0].code,
        "GEWYC-PARSE-DUPLICATE-TEMPLATE-HEAD"
    );

    let report = compile_findings_report_str(
        r#"
template(:demo)
window(:default_5s)
"#,
    );
    assert_eq!(
        report.findings[0].code,
        "GEWYC-PARSE-MISSING-PIPELINE-PREFIX"
    );

    let report = compile_findings_report_str("template=legacy\nwindow=default_5s\n");
    assert_eq!(report.findings[0].code, "GEWYC-PARSE-UNSUPPORTED-SYNTAX");
}

#[test]
fn compile_findings_report_str_classifies_function_declaration_shape() {
    let cases = [
        (
            "fn core(required = :value, late) =\n  |> fragment(:udp_packet_meta_fragment)\n",
            "GEWYC-PARSE-INVALID-PARAMETER-ORDER",
        ),
        (
            "fn core(value =) =\n  |> fragment(:udp_packet_meta_fragment)\n",
            "GEWYC-PARSE-MISSING-PARAMETER-DEFAULT",
        ),
        (
            "fn core(:) =\n  |> fragment(:udp_packet_meta_fragment)\n",
            "GEWYC-PARSE-INVALID-PARAMETER-NAME",
        ),
        (
            "fn bad.name() =\n  |> fragment(:udp_packet_meta_fragment)\n",
            "GEWYC-PARSE-INVALID-FUNCTION-NAME",
        ),
        (
            "fn core(bad.name) =\n  |> fragment(:udp_packet_meta_fragment)\n",
            "GEWYC-PARSE-INVALID-PARAMETER-NAME",
        ),
        (
            "fn core() trailing =\n  |> fragment(:udp_packet_meta_fragment)\n",
            "GEWYC-PARSE-INVALID-FUNCTION-SIGNATURE",
        ),
        (
            "fn core() =\n  let value = :first\n  let value = :second\n  |> fragment(:udp_packet_meta_fragment)\n",
            "GEWYC-PARSE-DUPLICATE-LOCAL-BINDING",
        ),
    ];
    for (input, code) in cases {
        let report = compile_findings_report_str(input);
        assert_eq!(report.findings[0].code, code, "input: {input}");
    }
}

#[test]
fn compile_findings_report_str_rejects_unclosed_strings() {
    let input = r#"
template :unclosed_string
|> window :default_5s
|> reason "udp_datagram_l1
"#;
    let report = compile_findings_report_str(input);
    let finding = report.findings.first().expect("parse finding");
    let line = input.lines().nth(3).unwrap();
    assert_eq!(finding.code, "GEWYC-PARSE-UNCLOSED-STRING");
    assert_eq!(finding.line, Some(4));
    assert_eq!(finding.column, Some(line.find('"').unwrap() + 1));
}

#[test]
fn dsl_invalid_value_codes_classify_package_graph_failures() {
    let cases = [
        (
            "pipeline include cycle detected at '/tmp/module.gewy'",
            "GEWYC-PARSE-INCLUDE-CYCLE",
        ),
        (
            "pipeline use cycle detected at function 'core'",
            "GEWYC-PARSE-USE-CYCLE",
        ),
        (
            "unknown package source 'local'",
            "GEWYC-PARSE-UNKNOWN-PACKAGE-SOURCE",
        ),
        (
            "invalid source dependency 'local/pkg', expected source:<name>/<package>",
            "GEWYC-PARSE-INVALID-SOURCE-DEPENDENCY",
        ),
        (
            "included path '/tmp/outside.gewy' escapes package root '/tmp/app'",
            "GEWYC-PARSE-INCLUDE-ESCAPES-PACKAGE",
        ),
        (
            "pipeline include() should be resolved before lowering",
            "GEWYC-PARSE-UNRESOLVED-INCLUDE",
        ),
    ];
    for (message, code) in cases {
        assert_eq!(dsl_invalid_value_code(message), code, "message: {message}");
    }
    assert_eq!(
        dsl_invalid_value_code(
            "pipeline placeholder expansion exceeded 32 substitutions while test expansion"
        ),
        "GEWYC-PARSE-PLACEHOLDER-EXPANSION-LIMIT"
    );
}

#[test]
fn compile_findings_report_str_uses_specific_code_for_parameter_kind_conflict() {
    let report = compile_findings_report_str(
        r#"
fn udp_core(model_name: bool) =
  |> program_model $model_name

template :kind_conflict
|> window :default_5s
|> reason :udp_datagram_l1
|> use :udp_core, true
"#,
    );
    let finding = report.findings.first().expect("parse finding");
    assert_eq!(finding.code, "GEWYC-PARSE-PARAMETER-KIND-CONFLICT");
    assert_eq!(finding.line, Some(2));
    assert!(finding.message.contains("declares kind 'bool'"));
}

#[test]
fn compile_findings_report_str_uses_specific_code_for_argument_type_mismatch() {
    let report = compile_findings_report_str(
        r#"
fn rule_module(predicate_name: predicate) =
  |> program_model :predicate_model
  |> operation :datagram_exchange
  |> program_rule pred: $predicate_name, stage: :process_bound, narr: :process_bound, dedupe: true

template :argument_mismatch
|> window :default_5s
|> reason :udp_datagram_l1
|> use :rule_module, :not_a_real_predicate
"#,
    );
    let finding = report.findings.first().expect("parse finding");
    assert_eq!(finding.code, "GEWYC-PARSE-ARGUMENT-TYPE-MISMATCH");
    assert_eq!(finding.line, Some(10));
    assert!(
        finding
            .message
            .contains("expects predicate-compatible value")
    );
}

#[test]
fn compile_findings_report_str_uses_specific_code_for_unknown_placeholder() {
    let report = compile_findings_report_str(
        r#"
fn udp_core(model_name: atom) =
  |> program_model $model_nam

template :unknown_placeholder
|> window :default_5s
|> reason :udp_datagram_l1
|> use :udp_core, :udp_model
"#,
    );
    let finding = report.findings.first().expect("parse finding");
    assert_eq!(finding.code, "GEWYC-PARSE-UNKNOWN-PLACEHOLDER");
    assert_eq!(finding.line, Some(3));
    assert!(
        finding
            .message
            .contains("unknown pipeline placeholder '$model_nam'")
    );
}

#[test]
fn compile_findings_report_str_rejects_unclosed_placeholders() {
    let report = compile_findings_report_str(
        r#"
fn reason_core(model) =
  |> reason_model("${model")

template :unclosed_placeholder
|> window :default_5s
|> reason :udp_datagram_l1
|> use :reason_core, :demo_model
"#,
    );
    let finding = report.findings.first().expect("parse finding");
    assert_eq!(finding.code, "GEWYC-PARSE-UNCLOSED-PLACEHOLDER");
    assert_eq!(finding.line, Some(3));
}

#[test]
fn compile_findings_report_str_uses_specific_code_for_unknown_named_argument() {
    let report = compile_findings_report_str(
        r#"
fn udp_core(model_name: atom) =
  |> program_model $model_name

template :unknown_named_argument
|> window :default_5s
|> reason :udp_datagram_l1
|> use :udp_core, mode_name: :udp_model
"#,
    );
    let finding = report.findings.first().expect("parse finding");
    assert_eq!(finding.code, "GEWYC-PARSE-UNKNOWN-NAMED-ARGUMENT");
    assert_eq!(finding.line, Some(8));
    assert!(
        finding
            .message
            .contains("unknown named parameter 'mode_name'")
    );
}

#[test]
fn compile_findings_report_str_uses_specific_code_for_duplicate_argument() {
    let report = compile_findings_report_str(
        r#"
fn udp_core(model_name: atom) =
  |> program_model $model_name

template :duplicate_argument
|> window :default_5s
|> reason :udp_datagram_l1
|> use :udp_core, model_name: :first, model_name: :second
"#,
    );
    let finding = report.findings.first().expect("parse finding");
    assert_eq!(finding.code, "GEWYC-PARSE-DUPLICATE-ARGUMENT");
    assert_eq!(finding.line, Some(8));
    assert!(
        finding
            .message
            .contains("duplicate named argument 'model_name'")
    );
}

#[test]
fn compile_findings_report_str_uses_specific_code_for_argument_order() {
    let report = compile_findings_report_str(
        r#"
fn udp_core(model_name: atom, dedupe: bool) =
  |> program_model $model_name
  |> program_rule pred: :process_bound, stage: :process_bound, narr: :process_bound, dedupe: $dedupe

template :argument_order
|> window :default_5s
|> reason :udp_datagram_l1
|> use :udp_core, model_name: :udp_model, true
"#,
    );
    let finding = report.findings.first().expect("parse finding");
    assert_eq!(finding.code, "GEWYC-PARSE-ARGUMENT-ORDER");
    assert_eq!(finding.line, Some(9));
    assert!(
        finding
            .message
            .contains("positional arguments after named arguments")
    );
}

#[test]
fn compile_findings_report_str_uses_specific_code_for_function_arity() {
    let report = compile_findings_report_str(
        r#"
fn udp_core(model_name: atom) =
  |> program_model $model_name

template :function_arity
|> window :default_5s
|> reason :udp_datagram_l1
|> use :udp_core
"#,
    );
    let finding = report.findings.first().expect("parse finding");
    assert_eq!(finding.code, "GEWYC-PARSE-FUNCTION-ARITY");
    assert_eq!(finding.line, Some(8));
    assert!(finding.message.contains("expected 1 args, got 0"));
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
    assert!(json.contains("\"summary\":{\"finding_count\":1"));
    assert!(json.contains("\"next_step\":\"fix the parse finding first"));
    assert!(json.contains("\"code\":\"GEWYC-PARSE-UNKNOWN-PIPELINE-STEP\""));
    assert!(json.contains("\"severity\":\"error\""));
    assert!(json.contains("\"line\":6"));
}

#[test]
fn findings_text_includes_count_and_next_step_hint() {
    let report = compile_findings_report_str(
        r#"
template(:broken)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> oops(:true)
"#,
    );
    let text = render_findings_report(&report, RenderFormat::Text);
    assert!(text.contains("finding_count=1"));
    assert!(text.contains("next_step=fix the parse finding first"));
    assert!(text.contains("findings:"));
    assert!(text.contains("- stage=parse severity=error"));
}

#[test]
fn findings_text_empty_surface_keeps_guidance() {
    let report = compile_findings_report_str(
        &crate::dsl::read_file(&dsl_fixture_path("udp_process_debug.gewy")).unwrap(),
    );
    let text = render_findings_report(&report, RenderFormat::Text);
    assert!(text.contains("finding_count=0"));
    assert!(text.contains("findings=none"));
    assert!(text.contains("next_step=findings are clear"));
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
    assert_eq!(finding.code, "GEWYC-PARSE-INVALID-FUNCTION-SIGNATURE");
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
    assert_eq!(finding.code, "GEWYC-PARSE-INVALID-LET-BINDING");
    assert_eq!(finding.line, Some(3));
    assert_eq!(finding.column, Some(9));
    let text = render_findings_report(&report, RenderFormat::Text);
    let json = render_findings_report(&report, RenderFormat::Json);
    assert!(text.contains("line=3 column=9"));
    assert!(json.contains("\"line\":3"));
    assert!(json.contains("\"column\":9"));
}

#[test]
fn parse_findings_use_specific_code_for_invalid_pipeline_call() {
    let report = compile_findings_report_str(
        r#"
template :invalid_call
|> window(:default_5s
"#,
    );
    let finding = report.findings.first().expect("parse finding");
    assert_eq!(finding.code, "GEWYC-PARSE-INVALID-PIPELINE-CALL");
    assert_eq!(finding.line, Some(3));
}

#[test]
fn parse_findings_reject_unknown_rule_fields_before_missing_fields() {
    let input = r#"
template :unknown_rule_field
|> window :default_5s
|> reason :udp_datagram_l1
|> fragment :udp_packet_meta_fragment
|> operation :datagram_exchange
|> program_model :demo_model
|> program_rule pred: :process_bound, stgae: :process_bound, narr: :process_bound, dedupe: true
"#;
    let report = compile_findings_report_str(input);
    let finding = report.findings.first().expect("parse finding");
    let line = input.lines().nth(7).unwrap();
    assert_eq!(finding.code, "GEWYC-PARSE-UNKNOWN-RULE-FIELD");
    assert_eq!(finding.line, Some(8));
    assert_eq!(finding.column, Some(line.find("stgae:").unwrap() + 8));
    assert!(finding.message.contains("unknown field 'stgae'"));
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
    assert_eq!(finding.code, "GEWYC-PARSE-UNKNOWN-STAGE");
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
    assert_eq!(finding.code, "GEWYC-PARSE-INVALID-PREDICATE-QUALIFIER");
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
    assert_eq!(finding.code, "GEWYC-PARSE-INVALID-PREDICATE-QUALIFIER");
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
    assert_eq!(finding.code, "GEWYC-PARSE-MISSING-PREDICATE-QUALIFIER");
    assert_eq!(finding.line, Some(8));
    assert_eq!(finding.column, Some(expected_column));
}

#[test]
fn parse_findings_classify_reason_profile_vocabulary_errors() {
    let input = r#"
template(:demo)
|> window(:default_5s)
|> reason(:not_a_profile)
"#;
    let report = compile_findings_report_str(input);
    let finding = report.findings.first().expect("parse finding");
    let line = input.lines().nth(3).unwrap();
    assert_eq!(finding.code, "GEWYC-PARSE-UNKNOWN-REASON-PROFILE");
    assert_eq!(finding.line, Some(4));
    assert_eq!(
        finding.column,
        Some(line.find(":not_a_profile").unwrap() + 1)
    );
}

#[test]
fn parse_findings_classify_window_profile_and_scalar_errors() {
    let input = r#"
template(:demo)
|> window(:not_a_window)
"#;
    let report = compile_findings_report_str(input);
    let finding = report.findings.first().expect("parse finding");
    let line = input.lines().nth(2).unwrap();
    assert_eq!(finding.code, "GEWYC-PARSE-UNKNOWN-WINDOW-PROFILE");
    assert_eq!(finding.line, Some(3));
    assert_eq!(
        finding.column,
        Some(line.find(":not_a_window").unwrap() + 1)
    );

    let report = compile_findings_report_str(
        r#"
template(:demo)
|> window(duration_ms: not_an_integer, lateness_ms: 200)
"#,
    );
    let finding = report.findings.first().expect("parse finding");
    assert_eq!(finding.code, "GEWYC-PARSE-INVALID-INTEGER");

    let report = compile_findings_report_str(
        r#"
template(:demo)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> program_rule(predicate: :process_bound, stage: :process_bound, narrative: :process_bound, dedupe: maybe)
"#,
    );
    let finding = report.findings.first().expect("parse finding");
    assert_eq!(finding.code, "GEWYC-PARSE-INVALID-BOOLEAN");
}

#[test]
fn parse_findings_classify_pipeline_step_shape_errors() {
    let report = compile_findings_report_str(
        r#"
template(:demo)
|> window(:default_5s)
|> reason(:udp_datagram_l1, :udp_datagram_l1)
"#,
    );
    let finding = report.findings.first().expect("parse finding");
    assert_eq!(finding.code, "GEWYC-PARSE-INVALID-STEP-ARITY");

    let report = compile_findings_report_str(
        r#"
template(:demo)
|> window(duration_ms: 5000, broken)
"#,
    );
    let finding = report.findings.first().expect("parse finding");
    assert_eq!(finding.code, "GEWYC-PARSE-MALFORMED-ARGUMENT");

    let report = compile_findings_report_str(
        r#"
template(:demo)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> program_rule(predicate: :process_bound, stage: :process_bound, narrative: :process_bound, dedupe: true, phase: :bind)
"#,
    );
    let finding = report.findings.first().expect("parse finding");
    assert_eq!(finding.code, "GEWYC-PARSE-RULE-PHASE-WITHOUT-MODULE");
}

#[test]
fn parse_findings_classify_evidence_vocabulary_errors() {
    let report = compile_findings_report_str(
        r#"
template(:demo)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> evidence(:not_a_fact, :core_requirement)
"#,
    );
    let finding = report.findings.first().expect("parse finding");
    assert_eq!(finding.code, "GEWYC-PARSE-UNKNOWN-EVIDENCE-FACT-KIND");

    let report = compile_findings_report_str(
        r#"
template(:demo)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> evidence(:sock_lineage, :not_a_tier)
"#,
    );
    let finding = report.findings.first().expect("parse finding");
    assert_eq!(finding.code, "GEWYC-PARSE-UNKNOWN-EVIDENCE-TIER");
}

#[test]
fn parse_findings_classify_fragment_param_target_errors() {
    let report = compile_findings_report_str(
        r#"
template(:demo)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> param(:missing_separator, true)
"#,
    );
    let finding = report.findings.first().expect("parse finding");
    assert_eq!(finding.code, "GEWYC-PARSE-INVALID-FRAGMENT-PARAM-TARGET");
}

#[test]
fn parse_findings_classify_reason_key_event_vocabulary_errors() {
    let report = compile_findings_report_str(
        r#"
template(:demo)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> reason_rule(predicate: :process_bound, key_event: :not_an_event, narrative: :process_bound, dedupe: true)
"#,
    );
    let finding = report.findings.first().expect("parse finding");
    assert_eq!(finding.code, "GEWYC-PARSE-UNKNOWN-KEY-EVENT");
}

#[test]
fn parse_findings_classify_predicate_vocabulary_errors() {
    let report = compile_findings_report_str(
        r#"
template(:demo)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> program_rule(predicate: "packet_observed:not_a_proto", stage: :connect_flow, narrative: "static:test", dedupe: true)
"#,
    );
    let finding = report.findings.first().expect("parse finding");
    assert_eq!(finding.code, "GEWYC-PARSE-UNKNOWN-TRANSPORT-PROTOCOL");

    let report = compile_findings_report_str(
        r#"
template(:demo)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> program_rule(predicate: "not_a_predicate", stage: :connect_flow, narrative: "static:test", dedupe: true)
"#,
    );
    let finding = report.findings.first().expect("parse finding");
    assert_eq!(finding.code, "GEWYC-PARSE-UNKNOWN-PREDICATE");
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
    assert_eq!(finding.code, "GEWYC-PARSE-INVALID-PREDICATE-QUALIFIER");
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
