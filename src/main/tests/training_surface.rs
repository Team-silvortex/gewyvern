use super::support_facts::push_synthetic_missing_stage_finding;
use super::*;
use crate::data_api::training_sample_id;
use crate::render_utils::extract_json_string_field;

fn demo_training_export() -> ExportBundle {
    let binding = compile_file(&dsl_fixture_path("http_request_path.gewy"))
        .expect("http_request_path DSL should compile");
    annotate_export_trust(
        run_binding_demo(binding),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
    )
}

#[test]
fn training_example_json_exposes_input_supervision_and_provenance() {
    let _guard = test_guard();
    set_external_analysis_config(None);
    let export = demo_training_export();
    let body = training_example_json("dsl_demo", &export);
    assert!(body.contains("\"kind\":\"training_example\""));
    assert!(body.contains("\"sample_id\":\"gewy:"));
    assert!(body.contains("\"input\":{"));
    assert!(body.contains("\"supervision\":{"));
    assert!(body.contains("\"provenance\":{"));
    assert!(body.contains("\"schema_version\":1"));
    assert!(body.contains("\"primary_failure_mode\""));
    assert!(body.contains("\"operator_guidance_action\""));
    assert!(body.contains("\"targets\":{\"diagnosis\":{"));
    assert!(body.contains("\"automation\":{\"posture\":"));
    assert!(body.contains("\"ranking\":{\"attention_priority\":"));
    assert!(body.contains("\"augmentations\":["));
}

#[test]
fn training_example_array_supports_scan_level_export() {
    let _guard = test_guard();
    set_external_analysis_config(None);
    let outputs = synthesize_large_scan_outputs(2);
    let analyses = outputs
        .iter()
        .map(|(_, export)| analysis_snapshot(export))
        .collect::<Vec<_>>();
    let body = training_example_json_array(&outputs, &analyses);
    assert!(body.starts_with('['));
    assert!(body.contains("\"kind\":\"training_example\""));
    assert!(body.contains("\"sample_id\":\"gewy:"));
    assert!(body.contains("\"name\":\"scan:http:request:0\""));
    assert!(body.contains("\"name\":\"scan:http:request:1\""));
}

#[test]
fn training_example_promotes_top_level_diagnosis_aggregates() {
    let _guard = test_guard();
    set_external_analysis_config(None);
    let mut export = demo_training_export();
    let flow = export.program_flows[0].clone();
    push_synthetic_missing_stage_finding(
        &mut export,
        &flow,
        "http_request_path",
        "http_request_response",
        "receive_response",
        "receive_payload",
        "send_request->receive_response",
        "emit_payload->receive_payload",
        "transport_io",
        "synthetic missing response",
        "tcp_packet_meta_fragment",
        "missing_signal:packet_observed",
    );
    let body = training_example_json("dsl_demo", &export);
    assert!(body.contains("\"primary_module_family\":\"request-response\""));
    assert!(body.contains("\"evidence_posture\":\"missing_transition\""));
    assert!(body.contains("\"automation_outcome\":\"collect_more_evidence\""));
    assert!(body.contains("\"operations\":[\"http_request\"]"));
    assert!(body.contains(
        "\"phases\":[\"bind\",\"resolve_upstream\",\"connect\",\"establish\",\"send_request\",\"receive_response\"]"
    ));
    assert!(body.contains("\"missing_transitions\":[\"send_request->receive_response\"]"));
    assert!(body.contains("\"suspect_areas\":[\"transport_io\"]"));
}

#[test]
fn training_example_contract_keeps_stable_top_level_and_input_fields() {
    let _guard = test_guard();
    set_external_analysis_config(None);
    let export = demo_training_export();
    let body = training_example_json("dsl_demo", &export);
    let sample_id = extract_json_string_field(&body, "sample_id")
        .expect("training example should expose sample_id");

    assert!(body.contains("\"kind\":\"training_example\""));
    assert!(body.contains("\"schema_version\":1"));
    assert!(body.contains("\"name\":\"dsl_demo\""));
    assert_eq!(sample_id, training_sample_id("dsl_demo"));
    assert!(body.contains("\"template_id\":"));
    assert!(body.contains("\"input\":{"));
    assert!(body.contains("\"target_status\":"));
    assert!(body.contains("\"primary_module_kind\":"));
    assert!(body.contains("\"primary_failure_stage\":"));
    assert!(body.contains("\"primary_failure_mode\":"));
    assert!(body.contains("\"primary_failure_detail\":"));
    assert!(body.contains("\"primary_failure_confidence\":"));
    assert!(body.contains("\"primary_failure_basis\":"));
    assert!(body.contains("\"ambiguous\":"));
    assert!(body.contains("\"competing_hypotheses\":"));
    assert!(body.contains("\"suspect_modules\":"));
    assert!(body.contains("\"protocol_flows\":["));
    assert!(body.contains("\"process_network_profiles\":["));
    assert!(body.contains("\"augmentations\":["));
    assert!(body.contains("\"external_sidecar_context\":"));
    assert!(body.contains("\"has_external_capability_profile\":"));
    assert!(body.contains("\"external_capability_status\":"));
    assert!(body.contains("\"external_hint_status\":"));
    assert!(body.contains("\"external_context_status\":"));
    assert!(body.contains("\"external_sidecar_trust_level\":"));
    assert!(body.contains("\"external_sidecar_consumption_mode\":"));
}

#[test]
fn training_example_contract_keeps_supervision_and_provenance_fields() {
    let _guard = test_guard();
    set_external_analysis_config(None);
    let export = demo_training_export();
    let body = training_example_json("dsl_demo", &export);

    assert!(body.contains("\"supervision\":{"));
    assert!(body.contains("\"operator_guidance_status\":"));
    assert!(body.contains("\"operator_guidance_action\":"));
    assert!(body.contains("\"operator_guidance_reason\":"));
    assert!(body.contains("\"operator_guidance_summary\":"));
    assert!(body.contains("\"targets\":{\"diagnosis\":{"));
    assert!(body.contains("\"guidance\":{\"status\":"));
    assert!(body.contains("\"automation\":{\"posture\":"));
    assert!(body.contains("\"ranking\":{\"attention_priority\":"));
    assert!(body.contains("\"provenance\":{"));
    assert!(body.contains("\"ingest_mode\":"));
    assert!(body.contains("\"ingest_mode_note\":"));
    assert!(body.contains("\"ingest_trust_mode\":"));
    assert!(body.contains("\"pid_attribution_status\":"));
    assert!(body.contains("\"fragments_loaded\":"));
    assert!(body.contains("\"flows\":"));
    assert!(body.contains("\"program_findings\":"));
    assert!(body.contains("\"module_findings\":"));
}

#[test]
fn api_training_example_routes_cover_single_export() {
    let _guard = test_guard();
    set_external_analysis_config(None);
    let export = demo_training_export();
    let analysis = analysis_snapshot(&export);
    let state = Arc::new(Mutex::new(Arc::new(ApiSnapshot::default())));
    update_api_snapshot_for_single(
        &state,
        ApiRenderedTarget {
            name: "dsl_demo".into(),
            primary_module_family: analysis.primary_module_family.clone(),
            evidence_posture: analysis.evidence_posture.clone(),
            automation_outcome: analysis.automation_outcome.clone(),
            summary_text: summary_line("dsl_demo", &export),
            summary_json: summary_json("dsl_demo", &export),
            findings_json: findings_json("dsl_demo", &export),
            analysis_json: analysis_snapshot_json(&analysis),
            training_example_json: training_example_json_with_analysis(
                "dsl_demo", &export, &analysis,
            ),
            has_external_sidecar_context: false,
            has_external_evidence_chain_enrichment: false,
            has_external_diagnostic_opinion: false,
            has_external_capability_profile: false,
            external_capability_status: None,
            external_hint_status: None,
            external_context_status: None,
            external_sidecar_trust_level: None,
            external_sidecar_consumption_mode: None,
            export_json: export.to_json(),
            report_json: single_target_report_json_with_analysis("dsl_demo", &export, &analysis),
            report_html: single_target_report_html_with_analysis("dsl_demo", &export, &analysis),
        },
    );
    let snapshot = state.lock().unwrap().clone();
    let meta = api_snapshot_meta_json(&snapshot);
    assert!(meta.contains("\"has_training_example_json\":true"));
    let (_, _, latest_body) =
        api_response_for_request("/v1/latest/training-example.json", &snapshot);
    assert!(latest_body.contains("\"kind\":\"training_example\""));
    assert!(latest_body.contains("\"sample_id\":\"gewy:"));
    let (_, _, dataset_body) =
        api_response_for_request("/v1/latest/training-dataset.json", &snapshot);
    assert!(dataset_body.contains("\"kind\":\"training_dataset_manifest\""));
    assert!(dataset_body.contains("\"sample_format\":\"training_example_json\""));
    assert!(dataset_body.contains("\"split_policies\":{\"default\":\"name_bucket_mod_10\""));
    assert!(dataset_body.contains("\"supervision_heads\":{\"diagnosis\""));
    assert!(dataset_body.contains("\"sample_id\":\"gewy:"));
    assert!(dataset_body.contains("\"split_hints\":{\"name_bucket_mod_10\":"));
    assert!(dataset_body.contains("\"group_key\":\"unknown\""));
    let (_, _, target_body) = api_response_for_request(
        "/v1/latest/targets/dsl_demo/training-example.json",
        &snapshot,
    );
    assert!(target_body.contains("\"name\":\"dsl_demo\""));
    let (_, _, target_dataset_body) = api_response_for_request(
        "/v1/latest/targets/dsl_demo/training-dataset.json",
        &snapshot,
    );
    assert!(target_dataset_body.contains("\"kind\":\"training_dataset_manifest\""));
    assert!(
        target_dataset_body
            .contains("\"sample_path\":\"/v1/latest/targets/dsl_demo/training-example.json\"")
    );
    assert!(target_dataset_body.contains("\"sample_id\":\"gewy:"));
    assert!(target_dataset_body.contains("\"split_hints\":{\"name_bucket_mod_10\":"));
    let sample_id = extract_json_string_field(&target_body, "sample_id")
        .expect("training example should expose sample_id");
    let manifest_sample_id = extract_json_string_field(&target_dataset_body, "sample_id")
        .expect("training dataset manifest should expose sample_id");
    assert_eq!(sample_id, manifest_sample_id);
}
