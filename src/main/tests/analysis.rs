use super::*;

#[test]
fn analysis_snapshot_supports_composable_augmenters() {
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy")
        .expect("http_request_path DSL should compile");
    let export = annotate_export_trust(
        run_binding_demo(binding),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
    );
    let augmenter = MlHookAugmenter;
    let snapshot = analysis_snapshot_with_augmenters(&export, &[&augmenter]);

    assert_eq!(snapshot.primary_failure_confidence, "ml-candidate");
    assert!(
        snapshot
            .competing_hypotheses
            .contains(&"augmenter:ml_rerank_hook".to_string()),
        "augmenters should be able to enrich the shared analysis snapshot",
    );
    assert!(
        snapshot
            .augmentations
            .iter()
            .any(|item| item.name == "ml_rerank_hook"),
        "external augmenters should append custom machine-readable annotations"
    );
    assert!(
        snapshot
            .augmentations
            .iter()
            .any(|item| item.name == "automation_recommendation"),
        "built-in augmenters should remain active when external augmenters are composed"
    );
    let json = analysis_snapshot_json(&snapshot);
    assert!(json.contains("\"augmentations\":["));
    assert!(json.contains("\"name\":\"ml_rerank_hook\""));
}

#[cfg(target_family = "unix")]
#[test]
fn analysis_snapshot_merges_external_etragon_augmentations() {
    with_fake_etragon_hook(
        "{\"augmentations\":[{\"kind\":\"ml-candidate\",\"name\":\"ml_candidate_targeted_escalation\",\"summary\":\"external engine suggests targeted escalation\",\"confidence\":\"candidate\",\"producer_stage\":\"candidate\",\"producer_pass\":\"fake_etragon\",\"data\":{\"module\":\"http_request_response\"}}]}",
        || {
            let binding =
                compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy")
                    .expect("http_request_path DSL should compile");
            let export = annotate_export_trust(
                run_binding_demo(binding),
                &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
            );
            let snapshot = analysis_snapshot(&export);
            let json = analysis_snapshot_json(&snapshot);
            assert!(
                snapshot
                    .augmentations
                    .iter()
                    .any(|item| item.name == "ml_candidate_targeted_escalation")
            );
            assert!(json.contains("\"producer_pass\":\"fake_etragon\""));
            assert!(json.contains("\"name\":\"ml_candidate_targeted_escalation\""));
        },
    );
}

#[cfg(target_family = "unix")]
#[test]
fn summary_and_findings_json_expose_external_augmentations() {
    with_fake_etragon_hook(
        "{\"augmentations\":[{\"kind\":\"ml-candidate\",\"name\":\"ml_candidate_manual_review\",\"summary\":\"external engine suggests manual review\",\"confidence\":\"candidate\",\"producer_stage\":\"candidate\",\"producer_pass\":\"fake_etragon\",\"data\":{\"module\":\"connection_establishment\"}}],\"evidence_chain_enrichment\":{\"status\":\"reinforced\",\"primary_label\":\"manual_review\",\"summary\":\"manual review is still the safest evidence-chain reading\",\"handoff_readiness\":\"mergeable\",\"gewyvern_merge_hint\":\"augmentations_and_guidance_context\"},\"diagnostic_opinion\":{\"status\":\"advisory\",\"diagnosis_kind\":\"manual_review_required\",\"label\":\"manual_review\",\"summary\":\"manual review remains the safest top-level opinion\",\"handoff_readiness\":\"advisory_only\",\"gewyvern_merge_hint\":\"sidecar_only_opinion\"}}",
        || {
            let binding =
                compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy")
                    .expect("http_request_path DSL should compile");
            let export = annotate_export_trust(
                run_binding_demo(binding),
                &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
            );
            let summary = summary_json("dsl_demo", &export);
            let findings = findings_json("dsl_demo", &export);
            assert!(summary.contains("\"augmentations\":["));
            assert!(summary.contains("\"name\":\"ml_candidate_manual_review\""));
            assert!(summary.contains("\"external_sidecar_context\":{"));
            assert!(summary.contains("\"merge_hint\":\"augmentations_and_guidance_context\""));
            assert!(summary.contains("\"consumption_mode\":\"guidance_context\""));
            assert!(summary.contains("\"has_external_capability_profile\":true"));
            assert!(summary.contains("\"external_sidecar_trust_level\":\"trusted\""));
            assert!(summary.contains("\"external_context_status\":\"declared\""));
            assert!(summary.contains("\"external_sidecar_consumption_mode\":\"operator_review\""));
            assert!(findings.contains("\"augmentations\":["));
            assert!(findings.contains("\"producer_pass\":\"fake_etragon\""));
            assert!(findings.contains("\"external_sidecar_context\":{"));
            assert!(findings.contains("\"merge_hint\":\"sidecar_only_opinion\""));
            assert!(findings.contains("\"consumption_mode\":\"operator_review\""));
            assert!(findings.contains("\"has_external_capability_profile\":true"));
            assert!(findings.contains("\"external_sidecar_trust_level\":\"trusted\""));
            assert!(findings.contains("\"external_context_status\":\"declared\""));
            assert!(findings.contains("\"external_sidecar_consumption_mode\":\"operator_review\""));
        },
    );
}

#[cfg(target_family = "unix")]
#[test]
fn summary_line_and_html_surface_external_sidecar_hints() {
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
            let summary = summary_line("dsl_demo", &export);
            let html = scan_report_html(&[("dsl_demo".to_string(), export)]);
            assert!(summary.contains(
                    "external_enrichment_hint=automation_worthy+augmentations_with_operator_guidance_support"
                ));
            assert!(summary.contains(
                "external_diagnostic_opinion_hint=automation_worthy+operator_guidance_candidate"
            ));
            assert!(
                summary.contains("external_collaboration_state=automation_worthy_sidecar_opinion")
            );
            assert!(
                summary.contains("external_operator_guidance_support=operator_guidance_candidate")
            );
            assert!(html.contains("external_evidence_chain_enrichment"));
            assert!(html.contains("handoff=automation_worthy"));
            assert!(html.contains("merge_hint=augmentations_with_operator_guidance_support"));
            assert!(html.contains("external_diagnostic_opinion"));
            assert!(html.contains("merge_hint=operator_guidance_candidate"));
            assert!(html.contains("External sidecar context:"));
            assert!(html.contains("External operator-guidance support:"));
            assert!(html.contains("automation_worthy_sidecar_opinion"));
            assert!(html.contains("operator_guidance_candidate"));
        },
    );
}

#[cfg(target_family = "unix")]
#[test]
fn summary_line_and_html_mark_advisory_only_sidecar_context() {
    with_fake_etragon_hook(
        "{\"augmentations\":[],\"evidence_chain_enrichment\":{\"status\":\"reinforced\",\"primary_label\":\"manual_review\",\"summary\":\"manual review is still the safest evidence-chain reading\",\"handoff_readiness\":\"advisory_only\",\"gewyvern_merge_hint\":\"augmentations_only\"},\"diagnostic_opinion\":{\"status\":\"advisory\",\"diagnosis_kind\":\"manual_review_required\",\"label\":\"manual_review\",\"summary\":\"manual review remains the safest top-level opinion\",\"handoff_readiness\":\"advisory_only\",\"gewyvern_merge_hint\":\"sidecar_only_opinion\"}}",
        || {
            let binding =
                compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy")
                    .expect("http_request_path DSL should compile");
            let export = annotate_export_trust(
                run_binding_demo(binding),
                &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
            );
            let summary = summary_line("dsl_demo", &export);
            let html = scan_report_html(&[("dsl_demo".to_string(), export)]);
            assert!(summary.contains("external_collaboration_state=advisory_only_sidecar_context"));
            assert!(summary.contains("external_operator_guidance_support=none"));
            assert!(html.contains("advisory_only_sidecar_context"));
            assert!(html.contains("should not be treated as a direct merged conclusion"));
            assert!(!html.contains("External operator-guidance support:"));
        },
    );
}

#[cfg(target_family = "unix")]
#[test]
fn analysis_snapshot_merges_external_sidecar_context_hints() {
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
            let snapshot = analysis_snapshot(&export);
            let json = analysis_snapshot_json(&snapshot);
            let augmentation_names = snapshot
                .augmentations
                .iter()
                .map(|item| item.name.clone())
                .collect::<Vec<_>>();
            assert!(
                snapshot
                    .augmentations
                    .iter()
                    .any(|item| item.name == "external_evidence_chain_enrichment"),
                "missing synthetic enrichment augmentation in {:?}",
                augmentation_names
            );
            assert!(
                snapshot
                    .augmentations
                    .iter()
                    .any(|item| item.name == "external_diagnostic_opinion"),
                "missing synthetic diagnostic opinion augmentation in {:?}",
                augmentation_names
            );
            assert!(json.contains("\"name\":\"external_evidence_chain_enrichment\""));
            assert!(json.contains("\"name\":\"external_diagnostic_opinion\""));
            assert!(json.contains(
                "\"external_merge_hint\":\"augmentations_with_operator_guidance_support\""
            ));
            assert!(json.contains("\"external_merge_hint\":\"operator_guidance_candidate\""));
            assert!(json.contains("\"external_sidecar_context\":{"));
            assert!(json.contains("\"has_external_capability_profile\":true"));
            assert!(json.contains("\"external_capability_status\":\"verified\""));
            assert!(json.contains("\"external_hint_status\":\"declared\""));
            assert!(json.contains("\"external_context_status\":\"declared\""));
            assert!(json.contains("\"external_sidecar_trust_level\":\"trusted\""));
            assert!(json.contains("\"external_sidecar_consumption_mode\":\"guidance_candidate\""));
            assert!(json.contains(
                "\"evidence_chain_enrichment\":{\"summary\":\"reinforced evidence chain\""
            ));
            assert!(json.contains("\"diagnostic_opinion\":{\"summary\":\"direct protocol failure is now the most direct opinion\""));
            assert!(json.contains("\"handoff_readiness\":\"automation_worthy\""));
            assert!(json.contains("\"merge_hint\":\"operator_guidance_candidate\""));
            assert!(json.contains("\"consumption_mode\":\"operator_guidance_support\""));
            assert!(json.contains("\"consumption_mode\":\"guidance_candidate\""));
        },
    );
}

#[cfg(target_family = "unix")]
#[test]
fn analysis_snapshot_downgrades_sidecar_hints_without_capability_profile() {
    with_fake_etragon_hook_and_capabilities(
        "{\"augmentations\":[{\"kind\":\"ml-candidate\",\"name\":\"ml_candidate_targeted_escalation\",\"summary\":\"external engine suggests targeted escalation\",\"confidence\":\"candidate\",\"producer_stage\":\"candidate\",\"producer_pass\":\"fake_etragon\",\"data\":{\"module\":\"http_request_response\"}}],\"evidence_chain_enrichment\":{\"status\":\"reinforced\",\"primary_label\":\"targeted_escalation\",\"summary\":\"reinforced evidence chain\",\"handoff_readiness\":\"automation_worthy\",\"gewyvern_merge_hint\":\"augmentations_with_operator_guidance_support\"},\"diagnostic_opinion\":{\"status\":\"ready\",\"diagnosis_kind\":\"direct_protocol_failure\",\"label\":\"targeted_escalation\",\"summary\":\"direct protocol failure is now the most direct opinion\",\"handoff_readiness\":\"automation_worthy\",\"gewyvern_merge_hint\":\"operator_guidance_candidate\"}}",
        None,
        || {
            let binding =
                compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy")
                    .expect("http_request_path DSL should compile");
            let export = annotate_export_trust(
                run_binding_demo(binding),
                &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
            );
            let snapshot = analysis_snapshot(&export);
            let json = analysis_snapshot_json(&snapshot);
            assert!(json.contains("\"name\":\"external_capability_profile\""));
            assert!(json.contains("\"compatibility_status\":\"unavailable\""));
            assert!(json.contains("\"context_status\":\"unavailable\""));
            assert!(json.contains(
                "\"evidence_chain_enrichment\":{\"summary\":\"reinforced evidence chain\""
            ));
            assert!(json.contains("\"diagnostic_opinion\":{\"summary\":\"direct protocol failure is now the most direct opinion\""));
            assert!(json.contains("\"handoff_readiness\":\"advisory_only\""));
            assert!(json.contains("\"merge_hint\":\"augmentations_only\""));
            assert!(json.contains("\"merge_hint\":\"sidecar_only_opinion\""));
            assert!(json.contains("\"consumption_mode\":\"append_only\""));
            assert!(json.contains("\"consumption_mode\":\"operator_review\""));
            assert!(json.contains("\"external_context_status\":\"unavailable\""));
            assert!(json.contains("\"external_sidecar_consumption_mode\":\"operator_review\""));
        },
    );
}

#[test]
fn analysis_snapshot_adds_unverified_ingest_augmentation() {
    let cli = Cli::from_args(["--tcp-socket".to_string(), "127.0.0.1:9000".to_string()]).unwrap();
    let export = annotate_export_trust(
        run_binding_demo(
            compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy")
                .expect("http_request_path DSL should compile"),
        ),
        &cli,
    );
    let snapshot = analysis_snapshot(&export);
    let json = analysis_snapshot_json(&snapshot);
    assert!(
        snapshot
            .augmentations
            .iter()
            .any(|item| item.name == "unverified_ingest_lineage"),
        "snapshot should expose an advisory trust augmentation"
    );
    assert!(json.contains("\"name\":\"unverified_ingest_lineage\""));
    assert!(json.contains("\"kind\":\"trust\""));
    assert!(json.contains("\"name\":\"automation_recommendation\""));
    assert!(json.contains("\"action\":\"avoid_pid_strong_actions\""));
    assert!(json.contains("\"operator_guidance_status\":\"advisory_only\""));
    assert!(json.contains("\"operator_guidance_action\":\"avoid_pid_strong_actions\""));
    assert!(json.contains("\"operator_guidance_reason\":\"unverified_ingest_lineage\""));
}

#[test]
fn analysis_snapshot_adds_competing_hypotheses_augmentation() {
    let process = synthetic_process_view(9101, "curl");
    let dns_export = coerce_export_process(
        annotate_export_trust(
            run_binding_demo(
                compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/dns_udp_process.gewy")
                    .expect("dns_udp_process DSL should compile"),
            ),
            &Cli::from_args(["--demo".to_string(), "udp".to_string()]).unwrap(),
        ),
        &process,
    );
    let mut http_export = coerce_export_process(
        annotate_export_trust(
            run_binding_demo(
                compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy")
                    .expect("http_request_path DSL should compile"),
            ),
            &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
        ),
        &process,
    );
    let http_flow = http_export.program_flows[0].clone();
    push_synthetic_missing_stage_finding(
        &mut http_export,
        &http_flow,
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
    let export = merge_exports_for_tests(vec![dns_export, http_export]);
    let snapshot = analysis_snapshot(&export);
    let json = analysis_snapshot_json(&snapshot);
    assert!(
        snapshot
            .augmentations
            .iter()
            .any(|item| item.name == "competing_hypotheses"),
        "snapshot should expose an advisory ambiguity augmentation"
    );
    assert!(json.contains("\"name\":\"competing_hypotheses\""));
    assert!(json.contains("\"kind\":\"analysis\""));
    assert!(json.contains("\"name\":\"automation_recommendation\""));
    assert!(json.contains("\"action\":\"keep_multiple_hypotheses\""));
    assert!(json.contains("\"operator_guidance_status\":\"ambiguous\""));
    assert!(json.contains("\"operator_guidance_action\":\"keep_multiple_hypotheses\""));
    assert!(json.contains("\"operator_guidance_reason\":\"competing_hypotheses\""));
}

#[test]
fn analysis_snapshot_adds_missing_transition_recommendation() {
    let mut export = annotate_export_trust(
        run_binding_demo(
            compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy")
                .expect("http_request_path DSL should compile"),
        ),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
    );
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
    let snapshot = analysis_snapshot(&export);
    let json = analysis_snapshot_json(&snapshot);
    assert!(
        snapshot
            .augmentations
            .iter()
            .any(|item| item.name == "automation_recommendation"),
        "snapshot should expose an automation-friendly recommendation augmentation"
    );
    assert!(json.contains("\"action\":\"collect_more_runtime_evidence\""));
    assert!(json.contains("\"reason\":\"missing_transition\""));
    assert!(json.contains("\"operator_guidance_status\":\"observe_more\""));
    assert!(json.contains("\"operator_guidance_action\":\"collect_more_runtime_evidence\""));
    assert!(json.contains("\"operator_guidance_reason\":\"missing_transition\""));
}

#[test]
fn summary_json_carries_operator_guidance_for_direct_protocol_signal() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_connect_auth_required_path.gewy")
            .expect("http_connect_auth_required_path DSL should compile");
    let export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 99001, 53100, "proxy-client"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    99001,
                    7,
                    SessionId(1),
                ),
                tcp_state_fact_with_ports_for_tests(3, 99001, 1, 2, 53100, 8080),
                packet_fact_with_dir_and_payload_for_tests(
                    4,
                    99001,
                    0x18,
                    PacketDir::Egress,
                    Some(53100),
                    Some(8080),
                    Some(0x43),
                    Some(0x434f),
                    Some(0x434f4e4e),
                ),
                packet_fact_with_dir_and_payload_for_tests(
                    5,
                    99001,
                    0x18,
                    PacketDir::Ingress,
                    Some(53100),
                    Some(8080),
                    Some(0x34),
                    Some(0x3430),
                    Some(0x34303720),
                ),
            ],
        ),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
    );
    let json = summary_json("dsl_demo", &export);
    assert!(json.contains("\"operator_guidance_status\":\"targeted_ready\""));
    assert!(json.contains("\"operator_guidance_action\":\"safe_to_escalate_protocol_signal\""));
    assert!(json.contains("\"operator_guidance_reason\":\"direct_protocol_signal\""));
}

#[test]
fn analysis_and_findings_json_wrap_object_arrays_correctly() {
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy")
        .expect("http_request_path DSL should compile");
    let export = annotate_export_trust(
        run_binding_demo(binding),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
    );
    let snapshot = analysis_snapshot(&export);
    let analysis = analysis_snapshot_json(&snapshot);
    let findings = findings_json_with_analysis("dsl_demo", &export, &snapshot);

    assert!(analysis.contains("\"augmentations\":["));
    assert!(analysis.contains("\"process_network_profiles\":["));
    assert!(analysis.contains("\"protocol_flows\":["));
    assert!(!analysis.contains("\"augmentations\":[["));
    assert!(!analysis.contains("\"process_network_profiles\":[["));
    assert!(!analysis.contains("\"protocol_flows\":[["));

    assert!(findings.contains("\"module_findings\":["));
    assert!(findings.contains("\"program_findings\":["));
    assert!(!findings.contains("\"module_findings\":[["));
    assert!(!findings.contains("\"program_findings\":[["));
}
