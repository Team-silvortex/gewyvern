use super::*;

#[cfg(target_family = "unix")]
#[test]
fn api_meta_marks_external_sidecar_context_presence() {
    with_fake_etragon_hook(
        "{\"augmentations\":[{\"kind\":\"ml-candidate\",\"name\":\"ml_candidate_targeted_escalation\",\"summary\":\"external engine suggests targeted escalation\",\"confidence\":\"candidate\",\"producer_stage\":\"candidate\",\"producer_pass\":\"fake_etragon\",\"data\":{\"module\":\"http_request_response\"}}],\"evidence_chain_enrichment\":{\"status\":\"reinforced\",\"primary_label\":\"targeted_escalation\",\"summary\":\"reinforced evidence chain\",\"handoff_readiness\":\"automation_worthy\",\"gewyvern_merge_hint\":\"augmentations_with_operator_guidance_support\"},\"diagnostic_opinion\":{\"status\":\"ready\",\"diagnosis_kind\":\"direct_protocol_failure\",\"label\":\"targeted_escalation\",\"summary\":\"direct protocol failure is now the most direct opinion\",\"handoff_readiness\":\"automation_worthy\",\"gewyvern_merge_hint\":\"operator_guidance_candidate\"}}",
        || {
            let binding =
                compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy")
                    .expect("http_request_path DSL should compile");
            let export = annotate_export_trust(
                run_binding_demo(binding),
                &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
            );
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
                    training_example_json: training_example_json("dsl_demo", &export),
                    has_external_sidecar_context: true,
                    has_external_evidence_chain_enrichment: true,
                    has_external_diagnostic_opinion: true,
                    has_external_capability_profile: true,
                    external_capability_status: Some("verified".into()),
                    external_hint_status: Some("declared".into()),
                    external_context_status: Some("declared".into()),
                    external_sidecar_trust_level: Some("trusted".into()),
                    external_sidecar_consumption_mode: Some("guidance_candidate".into()),
                    export_json: export.to_json(),
                    report_json: scan_report_json(&[("dsl_demo".to_string(), export.clone())]),
                    report_html: scan_report_html(&[("dsl_demo".to_string(), export.clone())]),
                },
            );
            let snapshot = state.lock().unwrap().clone();
            let meta = api_snapshot_meta_json(&snapshot);
            assert!(meta.contains("\"has_external_sidecar_context\":true"));
            assert!(meta.contains("\"has_external_evidence_chain_enrichment\":true"));
            assert!(meta.contains("\"has_external_diagnostic_opinion\":true"));
            assert!(meta.contains("\"has_external_capability_profile\":true"));
            assert!(meta.contains("\"external_capability_status\":\"verified\""));
            assert!(meta.contains("\"external_hint_status\":\"declared\""));
            assert!(meta.contains("\"external_context_status\":\"declared\""));
            assert!(meta.contains("\"external_sidecar_trust_level\":\"trusted\""));
            assert!(meta.contains("\"external_sidecar_consumption_mode\":\"guidance_candidate\""));
            assert!(meta.contains("\"primary_module_family\":\"request-response\""));
            assert!(meta.contains("\"evidence_posture\":"));
            assert!(meta.contains("\"automation_outcome\":"));
            let (_, _, targets_body) = api_response_for_request("/v1/latest/targets", &snapshot);
            assert!(targets_body.contains("\"has_external_sidecar_context\":true"));
            assert!(targets_body.contains("\"has_external_evidence_chain_enrichment\":true"));
            assert!(targets_body.contains("\"has_external_diagnostic_opinion\":true"));
            assert!(targets_body.contains("\"has_external_capability_profile\":true"));
            assert!(targets_body.contains("\"external_capability_status\":\"verified\""));
            assert!(targets_body.contains("\"external_hint_status\":\"declared\""));
            assert!(targets_body.contains("\"external_context_status\":\"declared\""));
            assert!(targets_body.contains("\"external_sidecar_trust_level\":\"trusted\""));
            assert!(
                targets_body
                    .contains("\"external_sidecar_consumption_mode\":\"guidance_candidate\"")
            );
            assert!(targets_body.contains("\"primary_module_family\":\"request-response\""));
        },
    );
}

#[cfg(target_family = "unix")]
#[test]
fn api_meta_marks_unverified_sidecar_trust_when_capability_profile_is_missing() {
    with_fake_etragon_hook_and_capabilities(
        "{\"augmentations\":[{\"kind\":\"ml-candidate\",\"name\":\"ml_candidate_targeted_escalation\",\"summary\":\"external engine suggests targeted escalation\",\"confidence\":\"candidate\",\"producer_stage\":\"candidate\",\"producer_pass\":\"fake_etragon\",\"data\":{\"module\":\"http_request_response\"}}],\"evidence_chain_enrichment\":{\"status\":\"reinforced\",\"primary_label\":\"targeted_escalation\",\"summary\":\"reinforced evidence chain\",\"handoff_readiness\":\"automation_worthy\",\"gewyvern_merge_hint\":\"augmentations_with_operator_guidance_support\"}}",
        None,
        || {
            let binding =
                compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy")
                    .expect("http_request_path DSL should compile");
            let export = annotate_export_trust(
                run_binding_demo(binding),
                &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
            );
            let analysis = analysis_snapshot(&export);
            let (
                has_external_sidecar_context,
                has_external_evidence_chain_enrichment,
                has_external_diagnostic_opinion,
            ) = crate::diagnosis_runtime::external_sidecar_presence(&analysis);
            let (
                has_external_capability_profile,
                external_capability_status,
                external_hint_status,
                external_context_status,
            ) = crate::diagnosis_runtime::external_capability_summary(&analysis);
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
                    training_example_json: training_example_json("dsl_demo", &export),
                    has_external_sidecar_context,
                    has_external_evidence_chain_enrichment,
                    has_external_diagnostic_opinion,
                    has_external_capability_profile,
                    external_capability_status,
                    external_hint_status,
                    external_context_status,
                    external_sidecar_trust_level:
                        crate::diagnosis_runtime::external_sidecar_trust_level(&analysis),
                    external_sidecar_consumption_mode:
                        crate::diagnosis_runtime::external_sidecar_consumption_mode(&analysis),
                    export_json: export.to_json(),
                    report_json: scan_report_json(&[("dsl_demo".to_string(), export.clone())]),
                    report_html: scan_report_html(&[("dsl_demo".to_string(), export.clone())]),
                },
            );
            let snapshot = state.lock().unwrap().clone();
            let meta = api_snapshot_meta_json(&snapshot);
            assert!(meta.contains("\"has_external_sidecar_context\":true"));
            assert!(meta.contains("\"has_external_capability_profile\":true"));
            assert!(meta.contains("\"external_capability_status\":\"unavailable\""));
            assert!(meta.contains("\"external_hint_status\":\"downgraded_unverified_profile\""));
            assert!(meta.contains("\"external_context_status\":\"unavailable\""));
            assert!(meta.contains("\"external_sidecar_trust_level\":\"unverified\""));
            assert!(meta.contains("\"external_sidecar_consumption_mode\":\"append_only\""));
        },
    );
}

#[test]
fn api_scan_meta_rolls_up_attention_first_diagnosis_spine() {
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
                name: "scan:redis:get".into(),
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
    let meta = api_snapshot_meta_json(&snapshot);
    assert!(meta.contains("\"primary_module_family\":\"request-response\""));
    assert!(meta.contains("\"evidence_posture\":\"direct_protocol_signal\""));
    assert!(meta.contains("\"automation_outcome\":\"targeted_escalation\""));
}
