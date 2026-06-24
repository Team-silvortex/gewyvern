use super::*;
use crate::render_utils::extract_json_string_field;

fn demo_matrix_export() -> ExportBundle {
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy")
        .expect("http_request_path DSL should compile");
    annotate_export_trust(
        run_binding_demo(binding),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
    )
}

fn single_target_snapshot(
    target_name: &str,
) -> (ApiSnapshot, String, String, String, String, String, String) {
    let export = demo_matrix_export();
    let analysis = analysis_snapshot(&export);
    let summary = summary_json(target_name, &export);
    let analysis_json = analysis_snapshot_json(&analysis);
    let training = training_example_json_with_analysis(target_name, &export, &analysis);
    let export_json = export.to_json();
    let report_json = scan_report_json(&[(target_name.to_string(), export.clone())]);
    let report_html = scan_report_html(&[(target_name.to_string(), export.clone())]);
    let state = Arc::new(Mutex::new(Arc::new(ApiSnapshot::default())));

    update_api_snapshot_for_single(
        &state,
        ApiRenderedTarget {
            name: target_name.into(),
            primary_module_family: analysis.primary_module_family.clone(),
            evidence_posture: analysis.evidence_posture.clone(),
            automation_outcome: analysis.automation_outcome.clone(),
            summary_text: summary_line(target_name, &export),
            summary_json: summary.clone(),
            findings_json: findings_json(target_name, &export),
            analysis_json: analysis_json.clone(),
            training_example_json: training.clone(),
            has_external_sidecar_context: false,
            has_external_evidence_chain_enrichment: false,
            has_external_diagnostic_opinion: false,
            has_external_capability_profile: false,
            external_capability_status: None,
            external_hint_status: None,
            external_context_status: None,
            external_sidecar_trust_level: None,
            external_sidecar_consumption_mode: None,
            export_json: export_json.clone(),
            report_json: report_json.clone(),
            report_html: report_html.clone(),
        },
    );

    (
        state.lock().unwrap().as_ref().clone(),
        summary,
        analysis_json,
        training,
        export_json,
        report_json,
        report_html,
    )
}

#[test]
fn machine_surface_roundtrip_keeps_summary_analysis_training_and_export_in_sync() {
    let _guard = test_guard();
    set_external_analysis_config(None);
    let target_name = "dsl_demo";
    let (snapshot, _, _, _, _, _, _) = single_target_snapshot(target_name);

    let (summary_status, _, summary_body) =
        api_response_for_request("/v1/latest/summary.json", &snapshot);
    let (analysis_status, _, analysis_body) =
        api_response_for_request("/v1/latest/analysis.json", &snapshot);
    let (training_status, _, training_body) =
        api_response_for_request("/v1/latest/training-example.json", &snapshot);
    let (dataset_status, _, dataset_body) =
        api_response_for_request("/v1/latest/training-dataset.json", &snapshot);
    let (export_status, _, export_body) =
        api_response_for_request("/v1/latest/export.json", &snapshot);
    let (targets_status, _, targets_body) =
        api_response_for_request("/v1/latest/targets", &snapshot);

    assert_eq!(summary_status, 200);
    assert_eq!(analysis_status, 200);
    assert_eq!(training_status, 200);
    assert_eq!(dataset_status, 200);
    assert_eq!(export_status, 200);
    assert_eq!(targets_status, 200);

    let summary_module = extract_json_string_field(&summary_body, "primary_module_kind")
        .expect("summary should expose primary_module_kind");
    let analysis_module = extract_json_string_field(&analysis_body, "primary_module_kind")
        .expect("analysis should expose primary_module_kind");
    let summary_mode = extract_json_string_field(&summary_body, "primary_failure_mode")
        .expect("summary should expose primary_failure_mode");
    let analysis_mode = extract_json_string_field(&analysis_body, "primary_failure_mode")
        .expect("analysis should expose primary_failure_mode");
    let sample_id = extract_json_string_field(&training_body, "sample_id")
        .expect("training example should expose sample_id");
    let manifest_sample_id = extract_json_string_field(&dataset_body, "sample_id")
        .expect("training dataset manifest should expose sample_id");

    assert_eq!(summary_module, analysis_module);
    assert_eq!(summary_mode, analysis_mode);
    assert_eq!(sample_id, manifest_sample_id);
    assert!(training_body.contains("\"input\":{"));
    assert!(training_body.contains("\"supervision\":{"));
    assert!(training_body.contains("\"provenance\":{"));
    assert!(dataset_body.contains("\"sample_format\":\"training_example_json\""));
    assert!(dataset_body.contains("\"supervision_heads\":{\"diagnosis\""));
    assert!(export_body.contains("\"template_id\""));
    assert!(export_body.contains("\"fragment_inventory\""));
    assert!(targets_body.contains("\"targets\":[\"dsl_demo\"]"));
}

#[test]
fn machine_surface_roundtrip_keeps_capabilities_and_runtime_certificate_surfaces_alive() {
    let _guard = test_guard();
    set_external_analysis_config(None);
    let (snapshot, _, _, _, _, _, _) = single_target_snapshot("dsl_demo");

    let (cap_status, _, cap_body) = api_response_for_request("/v1/capabilities", &snapshot);
    let (cert_status, _, cert_body) =
        api_response_for_request("/v1/runtime/certificates.json", &snapshot);
    let (policy_status, _, policy_body) =
        api_response_for_request("/v1/runtime/certificate-policy.json", &snapshot);
    let (state_status, _, state_body) =
        api_response_for_request("/v1/runtime/certificate-state.json", &snapshot);
    let (digest_status, _, digest_body) =
        api_response_for_request("/v1/latest/runtime-capability-digest.json", &snapshot);

    assert_eq!(cap_status, 200);
    assert_eq!(cert_status, 200);
    assert_eq!(policy_status, 200);
    assert_eq!(state_status, 200);
    assert_eq!(digest_status, 200);

    assert!(cap_body.contains("\"runtime_certificates\":true"));
    assert!(cap_body.contains("\"runtime_certificate_policy\":true"));
    assert!(cap_body.contains("\"runtime_certificate_state\":true"));
    assert!(cap_body.contains("\"training_dataset_manifest\":true"));
    assert!(cert_body.contains("\"surface\":\"runtime_certificates\""));
    assert!(cert_body.contains("\"policy\":{"));
    assert!(cert_body.contains("\"state\":{"));
    assert!(policy_body.contains("\"surface\":\"runtime_certificate_policy\""));
    assert!(policy_body.contains("\"reasons\":["));
    assert!(state_body.contains("\"surface\":\"runtime_certificate_state\""));
    assert!(state_body.contains("\"summary\":{"));
    assert!(digest_body.contains("\"surface\":\"runtime_capability_digest\""));
    assert!(digest_body.contains("\"targets_without_protocol_surface\":1"));
}
