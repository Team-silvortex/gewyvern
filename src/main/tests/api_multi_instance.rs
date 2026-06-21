use super::*;

#[test]
fn capabilities_advertise_runtime_cluster_overview_surface() {
    let snapshot = ApiSnapshot::default();
    let (status, _, body) = api_response_for_request("/v1/capabilities", &snapshot);
    assert_eq!(status, 200);
    assert!(body.contains("\"runtime_cluster_overview\":true"));
    assert!(body.contains("\"runtime_cluster_attention\":true"));
    assert!(body.contains("\"runtime_cluster_attention_reasons\":true"));
    assert!(body.contains("\"runtime_cluster_attention_summary\":true"));
    assert!(body.contains("\"/v1/latest/runtime-cluster-overview.json\""));
    assert!(body.contains("\"/v1/latest/runtime-cluster-attention.json\""));
    assert!(body.contains("\"/v1/latest/runtime-cluster-attention-reasons.json\""));
    assert!(body.contains("\"/v1/latest/runtime-cluster-attention-summary.json\""));
}

#[test]
fn runtime_cluster_overview_groups_targets_by_protocol_cluster() {
    let state = Arc::new(Mutex::new(Arc::new(ApiSnapshot::default())));
    update_api_snapshot_for_scan(
        &state,
        vec![
            ApiRenderedTarget {
                name: "scan:http:request".into(),
                primary_module_family: "request-response".into(),
                evidence_posture: "direct_protocol_signal".into(),
                automation_outcome: "targeted_escalation".into(),
                summary_text: String::new(),
                summary_json: String::new(),
                findings_json: String::new(),
                analysis_json: String::new(),
                training_example_json: String::new(),
                has_external_sidecar_context: true,
                has_external_evidence_chain_enrichment: false,
                has_external_diagnostic_opinion: false,
                has_external_capability_profile: true,
                external_capability_status: Some("verified".into()),
                external_hint_status: None,
                external_context_status: Some("declared".into()),
                external_sidecar_trust_level: Some("trusted".into()),
                external_sidecar_consumption_mode: None,
                export_json: String::new(),
                report_json: String::new(),
                report_html: String::new(),
            },
            ApiRenderedTarget {
                name: "scan:redis:zadd".into(),
                primary_module_family: "cache".into(),
                evidence_posture: "heuristic_summary".into(),
                automation_outcome: "manual_review".into(),
                summary_text: String::new(),
                summary_json: String::new(),
                findings_json: String::new(),
                analysis_json: String::new(),
                training_example_json: String::new(),
                has_external_sidecar_context: false,
                has_external_evidence_chain_enrichment: false,
                has_external_diagnostic_opinion: false,
                has_external_capability_profile: false,
                external_capability_status: Some("unavailable".into()),
                external_hint_status: None,
                external_context_status: Some("unavailable".into()),
                external_sidecar_trust_level: Some("unverified".into()),
                external_sidecar_consumption_mode: None,
                export_json: String::new(),
                report_json: String::new(),
                report_html: String::new(),
            },
            ApiRenderedTarget {
                name: "dsl_demo".into(),
                primary_module_family: "request-response".into(),
                evidence_posture: "direct_protocol_signal".into(),
                automation_outcome: "targeted_escalation".into(),
                summary_text: String::new(),
                summary_json: String::new(),
                findings_json: String::new(),
                analysis_json: String::new(),
                training_example_json: String::new(),
                has_external_sidecar_context: false,
                has_external_evidence_chain_enrichment: false,
                has_external_diagnostic_opinion: false,
                has_external_capability_profile: false,
                external_capability_status: None,
                external_hint_status: None,
                external_context_status: None,
                external_sidecar_trust_level: None,
                external_sidecar_consumption_mode: None,
                export_json: String::new(),
                report_json: String::new(),
                report_html: String::new(),
            },
        ],
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
    );

    let snapshot = state.lock().unwrap().clone();
    let (status, content_type, body) =
        api_response_for_request("/v1/latest/runtime-cluster-overview.json", &snapshot);
    assert_eq!(status, 200);
    assert_eq!(content_type, "application/json; charset=utf-8");
    assert!(body.contains("\"surface\":\"runtime_cluster_overview\""));
    assert!(body.contains("\"cluster_count\":2"));
    assert!(body.contains("\"unclustered_target_count\":1"));
    assert!(body.contains("\"key\":\"web-proxy-request-response\""));
    assert!(body.contains("\"key\":\"cache-queue-stream\""));
    assert!(body.contains("\"name\":\"scan:http:request\""));
    assert!(body.contains("\"name\":\"scan:redis:zadd\""));
    assert!(body.contains("\"name\":\"dsl_demo\""));
    assert!(body.contains("\"sidecar_context_count\":1"));
    assert!(body.contains("\"capability_profile_count\":1"));
    assert!(body.contains("\"external_capability_status\":\"verified\""));
    assert!(body.contains("\"external_sidecar_trust_level\":\"unverified\""));
}

#[test]
fn runtime_cluster_attention_rollup_prioritizes_clusters_and_targets() {
    let state = Arc::new(Mutex::new(Arc::new(ApiSnapshot::default())));
    update_api_snapshot_for_scan(
        &state,
        vec![
            ApiRenderedTarget {
                name: "scan:http:request".into(),
                primary_module_family: "request-response".into(),
                evidence_posture: "direct_protocol_signal".into(),
                automation_outcome: "targeted_escalation".into(),
                summary_text: String::new(),
                summary_json: String::new(),
                findings_json: String::new(),
                analysis_json: String::new(),
                training_example_json: String::new(),
                has_external_sidecar_context: true,
                has_external_evidence_chain_enrichment: false,
                has_external_diagnostic_opinion: false,
                has_external_capability_profile: true,
                external_capability_status: Some("verified".into()),
                external_hint_status: None,
                external_context_status: Some("declared".into()),
                external_sidecar_trust_level: Some("trusted".into()),
                external_sidecar_consumption_mode: None,
                export_json: String::new(),
                report_json: String::new(),
                report_html: String::new(),
            },
            ApiRenderedTarget {
                name: "scan:redis:zadd".into(),
                primary_module_family: "cache".into(),
                evidence_posture: "heuristic_summary".into(),
                automation_outcome: "manual_review".into(),
                summary_text: String::new(),
                summary_json: String::new(),
                findings_json: String::new(),
                analysis_json: String::new(),
                training_example_json: String::new(),
                has_external_sidecar_context: false,
                has_external_evidence_chain_enrichment: false,
                has_external_diagnostic_opinion: false,
                has_external_capability_profile: false,
                external_capability_status: Some("unavailable".into()),
                external_hint_status: None,
                external_context_status: Some("unavailable".into()),
                external_sidecar_trust_level: Some("unverified".into()),
                external_sidecar_consumption_mode: None,
                export_json: String::new(),
                report_json: String::new(),
                report_html: String::new(),
            },
            ApiRenderedTarget {
                name: "dsl_demo".into(),
                primary_module_family: "request-response".into(),
                evidence_posture: "direct_protocol_signal".into(),
                automation_outcome: "targeted_escalation".into(),
                summary_text: String::new(),
                summary_json: String::new(),
                findings_json: String::new(),
                analysis_json: String::new(),
                training_example_json: String::new(),
                has_external_sidecar_context: false,
                has_external_evidence_chain_enrichment: false,
                has_external_diagnostic_opinion: false,
                has_external_capability_profile: false,
                external_capability_status: None,
                external_hint_status: None,
                external_context_status: None,
                external_sidecar_trust_level: None,
                external_sidecar_consumption_mode: None,
                export_json: String::new(),
                report_json: String::new(),
                report_html: String::new(),
            },
        ],
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
    );

    let snapshot = state.lock().unwrap().clone();
    let (status, content_type, body) =
        api_response_for_request("/v1/latest/runtime-cluster-attention.json", &snapshot);
    assert_eq!(status, 200);
    assert_eq!(content_type, "application/json; charset=utf-8");
    assert!(body.contains("\"surface\":\"runtime_cluster_attention\""));
    assert!(body.contains("\"attention_cluster_count\":2"));
    assert!(body.contains("\"attention_target_count\":3"));
    assert!(body.contains("\"key\":\"web-proxy-request-response\""));
    assert!(body.contains("\"key\":\"cache-queue-stream\""));
    assert!(body.contains("\"priority\":\"critical\""));
    assert!(body.contains("\"priority\":\"warning\""));
    assert!(body.contains("\"reason_tags\":[\"automation.targeted_escalation\"]"));
    assert!(body.contains("\"reason_tags\":[\"automation.manual_review\",\"sidecar.unverified\",\"capability.unavailable\"]"));
    assert!(body.contains("\"reason_catalog\":["));
    assert!(body.contains("\"name\":\"dsl_demo\""));
}

#[test]
fn runtime_cluster_attention_reason_catalog_lists_standardized_reason_specs() {
    let snapshot = ApiSnapshot::default();
    let (status, content_type, body) = api_response_for_request(
        "/v1/latest/runtime-cluster-attention-reasons.json",
        &snapshot,
    );
    assert_eq!(status, 200);
    assert_eq!(content_type, "application/json; charset=utf-8");
    assert!(body.contains("\"surface\":\"runtime_cluster_attention_reasons\""));
    assert!(body.contains("\"key\":\"automation.targeted_escalation\""));
    assert!(body.contains("\"priority\":\"critical\""));
    assert!(body.contains("\"key\":\"capability.unavailable\""));
    assert!(body.contains("\"priority\":\"warning\""));
    assert!(body.contains("\"key\":\"sidecar.context_without_profile\""));
    assert!(body.contains("\"priority\":\"observe\""));
}

#[test]
fn runtime_cluster_attention_summary_compacts_cluster_card_data() {
    let state = Arc::new(Mutex::new(Arc::new(ApiSnapshot::default())));
    update_api_snapshot_for_scan(
        &state,
        vec![
            ApiRenderedTarget {
                name: "scan:http:request".into(),
                primary_module_family: "request-response".into(),
                evidence_posture: "direct_protocol_signal".into(),
                automation_outcome: "targeted_escalation".into(),
                summary_text: String::new(),
                summary_json: String::new(),
                findings_json: String::new(),
                analysis_json: String::new(),
                training_example_json: String::new(),
                has_external_sidecar_context: false,
                has_external_evidence_chain_enrichment: false,
                has_external_diagnostic_opinion: false,
                has_external_capability_profile: false,
                external_capability_status: None,
                external_hint_status: None,
                external_context_status: None,
                external_sidecar_trust_level: None,
                external_sidecar_consumption_mode: None,
                export_json: String::new(),
                report_json: String::new(),
                report_html: String::new(),
            },
            ApiRenderedTarget {
                name: "scan:redis:zadd".into(),
                primary_module_family: "cache".into(),
                evidence_posture: "heuristic_summary".into(),
                automation_outcome: "manual_review".into(),
                summary_text: String::new(),
                summary_json: String::new(),
                findings_json: String::new(),
                analysis_json: String::new(),
                training_example_json: String::new(),
                has_external_sidecar_context: false,
                has_external_evidence_chain_enrichment: false,
                has_external_diagnostic_opinion: false,
                has_external_capability_profile: false,
                external_capability_status: Some("unavailable".into()),
                external_hint_status: None,
                external_context_status: None,
                external_sidecar_trust_level: Some("unverified".into()),
                external_sidecar_consumption_mode: None,
                export_json: String::new(),
                report_json: String::new(),
                report_html: String::new(),
            },
            ApiRenderedTarget {
                name: "dsl_demo".into(),
                primary_module_family: "request-response".into(),
                evidence_posture: "direct_protocol_signal".into(),
                automation_outcome: "targeted_escalation".into(),
                summary_text: String::new(),
                summary_json: String::new(),
                findings_json: String::new(),
                analysis_json: String::new(),
                training_example_json: String::new(),
                has_external_sidecar_context: false,
                has_external_evidence_chain_enrichment: false,
                has_external_diagnostic_opinion: false,
                has_external_capability_profile: false,
                external_capability_status: None,
                external_hint_status: None,
                external_context_status: None,
                external_sidecar_trust_level: None,
                external_sidecar_consumption_mode: None,
                export_json: String::new(),
                report_json: String::new(),
                report_html: String::new(),
            },
        ],
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
    );

    let snapshot = state.lock().unwrap().clone();
    let (status, content_type, body) = api_response_for_request(
        "/v1/latest/runtime-cluster-attention-summary.json",
        &snapshot,
    );
    assert_eq!(status, 200);
    assert_eq!(content_type, "application/json; charset=utf-8");
    assert!(body.contains("\"surface\":\"runtime_cluster_attention_summary\""));
    assert!(body.contains("\"attention_cluster_count\":2"));
    assert!(body.contains("\"attention_target_count\":3"));
    assert!(body.contains("\"key\":\"web-proxy-request-response\""));
    assert!(body.contains("\"attention_target_count\":1"));
    assert!(body.contains("\"reason_counts\":[{\"key\":\"automation.targeted_escalation\",\"priority\":\"critical\",\"count\":1}]"));
    assert!(body.contains("\"key\":\"cache-queue-stream\""));
    assert!(body.contains("\"key\":\"automation.manual_review\""));
    assert!(body.contains("\"key\":\"sidecar.unverified\""));
    assert!(body.contains("\"key\":\"capability.unavailable\""));
    assert!(body.contains("\"unclustered_attention_target_count\":1"));
}
