use super::*;

#[test]
fn explain_report_rejects_stage_inference_mismatches_for_pipeline_arguments() {
    let report = compile_explain_report_str(
        r#"
fn stage_module(stage_value = :process_bound) =
  |> program_model(:stage_model)
  |> operation(:datagram_exchange)
  |> program_rule(predicate: :process_bound, stage: ${stage_value}, narrative: :process_bound, dedupe: true, module: :stage_module, phase: :bind)

template(:frontend_defaults)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> use(:stage_module, stage_value: :not_a_real_stage)
"#,
    );
    assert!(!report.ok);
    let text = render_explain_report(&report, RenderFormat::Text);
    assert!(text.contains("expects stage-compatible value"));
    assert!(text.contains("stage_value"));
}

#[test]
fn explain_report_rejects_key_event_inference_mismatches_for_pipeline_arguments() {
    let report = compile_explain_report_str(
        r#"
fn reason_module(event_value = :process_identified) =
  |> reason_model(:reason_model)
  |> reason_rule(predicate: :process_bound, key_event: ${event_value}, narrative: :process_bound, dedupe: true, module: :reason_module, phase: :bind)

template(:frontend_defaults)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> use(:reason_module, event_value: :not_a_real_event)
"#,
    );
    assert!(!report.ok);
    let text = render_explain_report(&report, RenderFormat::Text);
    assert!(text.contains("expects key_event-compatible value"));
    assert!(text.contains("event_value"));
}

#[test]
fn explain_report_rejects_phase_inference_mismatches_for_pipeline_arguments() {
    let report = compile_explain_report_str(
        r#"
fn phase_module(phase_value = :send_request) =
  |> program_model(:phase_model)
  |> operation(:datagram_exchange)
  |> program_rule(predicate: :process_bound, stage: :process_bound, narrative: :process_bound, dedupe: true, module: :phase_module, phase: ${phase_value})

template(:frontend_defaults)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> use(:phase_module, phase_value: :send-request)
"#,
    );
    assert!(!report.ok);
    let text = render_explain_report(&report, RenderFormat::Text);
    assert!(text.contains("expects phase-compatible value"));
    assert!(text.contains("phase_value"));
    assert!(text.contains("snake_case"));
}

#[test]
fn stages_report_summarizes_payload_offset_support() {
    let report = compile_stages_report_file(&dsl_fixture_path("snmp_get_path.gewy")).unwrap();
    assert_eq!(
        report.validation.sampled_payload_offsets,
        vec![0, 1, 4, 5, 9, 10, 13]
    );
    assert_eq!(report.validation.required_payload_offsets, vec![13]);
    assert_eq!(
        report.validation.unsupported_payload_offsets,
        Vec::<u16>::new()
    );
}

#[test]
fn envelope_json_is_valid_for_stable_subset_entry() {
    let report = compile_envelope_file(&dsl_fixture_path("http_request_path.gewy")).unwrap();
    let json = render_envelope_report(&report, RenderFormat::Json);
    assert_valid_json_document(&json);
}

#[test]
fn envelope_json_is_valid_for_registry_amqp_publish_entry() {
    let report = compile_envelope_file(&protocol_fixture_path("amqp/publish/main.gewy")).unwrap();
    let json = render_envelope_report(&report, RenderFormat::Json);
    assert_valid_json_document(&json);
}
