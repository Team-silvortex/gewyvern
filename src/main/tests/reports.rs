use super::{
    Cli, analysis_snapshot, annotate_export_trust, dsl_fixture_path,
    push_synthetic_missing_stage_finding, render_report_outputs, run_binding_demo,
    scan_report_html, scan_report_json, scan_report_text,
    single_target_report_html_with_analysis, single_target_report_json_with_analysis, summary_json,
    synthesize_large_scan_outputs, with_fake_etragon_hook,
};
use gewyvern::dsl::compile_file;
use gewyvern::export::ExportBundle;
use gewyvern::flow::{ProgramFinding, ProgramFindingCause};

const TARGET_NAME: &str = "scan:http:request";

fn demo_reports_export() -> ExportBundle {
    let binding = compile_file(&dsl_fixture_path("http_request_path.gewy"))
        .expect("http_request_path DSL should compile");
    annotate_export_trust(
        run_binding_demo(binding),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
    )
}

#[test]
fn export_json_carries_ingest_trust_mode() {
    let export = demo_reports_export();
    let json = export.to_json();
    assert!(json.contains("\"ingest_trust_mode\":\"synthetic-demo\""));
}

#[test]
fn summary_json_carries_ingest_trust_mode() {
    let export = demo_reports_export();
    let json = summary_json(TARGET_NAME, &export);
    assert!(json.contains("\"ingest_mode\":\"demo\""));
    assert!(json.contains("\"ingest_mode_note\":\"synthetic demo mode: useful for exercising flows and reports, not for real process attribution\""));
    assert!(json.contains("\"ingest_trust_mode\":\"synthetic-demo\""));
}

#[test]
fn summary_json_marks_socket_ingest_as_unverified_local() {
    let cli = Cli::from_args(["--tcp-socket".to_string(), "127.0.0.1:9000".to_string()]).unwrap();
    let binding = compile_file(&dsl_fixture_path("http_request_path.gewy"))
        .expect("http_request_path DSL should compile");
    let export = annotate_export_trust(run_binding_demo(binding), &cli);
    let json = summary_json("socket_session", &export);
    assert!(json.contains("\"ingest_mode\":\"local-advisory\""));
    assert!(json.contains("\"ingest_mode_note\":\"local advisory mode: facts come from a local socket source, but lineage is still unverified\""));
    assert!(json.contains("\"ingest_trust_mode\":\"unverified-local\""));
    assert!(json.contains("\"pid_attribution_status\":\"unverified\""));
    assert!(json.contains(
            "\"pid_attribution_note\":\"pid-scoped conclusions are advisory only because ingest lineage is unverified\""
        ));
}

#[test]
fn summary_json_exposes_single_object_identity_fields() {
    let export = demo_reports_export();
    let json = summary_json(TARGET_NAME, &export);
    assert!(json.contains("\"kind\":\"single\""));
    assert!(json.contains("\"name\":\"scan:http:request\""));
    assert!(json.contains("\"demo\":\"scan:http:request\""));
}

#[test]
fn summary_json_includes_protocol_flow_progress_for_healthy_export() {
    let export = demo_reports_export();
    let json = summary_json(TARGET_NAME, &export);
    assert!(json.contains("\"protocol_flows\":["));
    assert!(json.contains("\"process_network_profiles\":["));
    assert!(json.contains("\"status\":\"healthy\""));
    assert!(json.contains("\"last_phase\":\"receive_response\""));
    assert!(json.contains("\"module_kinds\":[\"http_request_response\"]"));
}

#[test]
fn summary_json_contract_keeps_stable_top_level_fields() {
    let export = demo_reports_export();
    let json = summary_json(TARGET_NAME, &export);

    assert!(json.contains("\"kind\":\"single\""));
    assert!(json.contains("\"name\":\"scan:http:request\""));
    assert!(json.contains("\"primary_module_kind\":"));
    assert!(json.contains("\"primary_failure_stage\":"));
    assert!(json.contains("\"primary_failure_mode\":"));
    assert!(json.contains("\"primary_failure_detail\":"));
    assert!(json.contains("\"primary_failure_confidence\":"));
    assert!(json.contains("\"primary_failure_basis\":"));
    assert!(json.contains("\"operator_guidance_status\":"));
    assert!(json.contains("\"operator_guidance_action\":"));
    assert!(json.contains("\"operator_guidance_reason\":"));
    assert!(json.contains("\"operator_guidance_summary\":"));
    assert!(json.contains("\"ambiguous\":"));
    assert!(json.contains("\"competing_hypotheses\":["));
    assert!(json.contains("\"ingest_mode\":"));
    assert!(json.contains("\"ingest_mode_note\":"));
    assert!(json.contains("\"ingest_trust_mode\":"));
    assert!(json.contains("\"pid_attribution_status\":"));
    assert!(json.contains("\"pid_attribution_note\":"));
    assert!(json.contains("\"augmentations\":["));
}

#[test]
fn summary_json_contract_keeps_guidance_and_ambiguity_surface() {
    let export = demo_reports_export();
    let json = summary_json(TARGET_NAME, &export);

    assert!(json.contains("\"operator_guidance_status\":"));
    assert!(json.contains("\"operator_guidance_action\":"));
    assert!(json.contains("\"operator_guidance_reason\":"));
    assert!(json.contains("\"operator_guidance_summary\":"));
    assert!(json.contains("\"ambiguous\":false"));
    assert!(json.contains("\"competing_hypotheses\":[]"));
    assert!(json.contains("\"ingest_mode\":\"demo\""));
    assert!(json.contains("\"ingest_trust_mode\":\"synthetic-demo\""));
    assert!(json.contains("\"pid_attribution_status\":\"synthetic\""));
    assert!(json.contains("\"augmentations\":["));
}

#[test]
fn scan_report_json_summarizes_all_targets() {
    let binding = compile_file(&dsl_fixture_path("http_request_path.gewy"))
        .expect("http_request_path DSL should compile");
    let healthy_export = annotate_export_trust(
        run_binding_demo(binding.clone()),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
    );
    let mut attention_export = annotate_export_trust(
        run_binding_demo(binding),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
    );
    let flow = attention_export.program_flows[0].clone();
    attention_export.program_findings.push(ProgramFinding {
        program_flow: flow.id,
        process: flow.process.clone(),
        operation: flow.operation.clone(),
        module_label: "http_request_path".into(),
        network_module_kind: "http_request_response".into(),
        phase: Some("receive_response".into()),
        phase_kind: Some("receive_payload".into()),
        phase_transition: Some("send_request->receive_response".into()),
        phase_transition_kind: Some("emit_payload->receive_payload".into()),
        suspect_area: "transport_io".into(),
        cause: ProgramFindingCause::MissingCoreStage,
        summary: "synthetic missing response".into(),
        supporting_fragments: vec!["tcp_packet_meta_fragment".into()],
        evidence_trace: vec!["missing_signal:packet_observed".into()],
    });
    let report = scan_report_json(&[
        ("scan:http:request".to_string(), healthy_export),
        ("scan:http:response".to_string(), attention_export),
    ]);
    assert!(report.contains("\"kind\":\"scan\""));
    assert!(report.contains("\"name\":null"));
    assert!(report.contains("\"target_count\":2"));
    assert!(report.contains("\"scan_all\":true"));
    assert!(report.contains("\"total_targets\":2"));
    assert!(report.contains("\"healthy_targets\":1"));
    assert!(report.contains("\"attention_targets\":1"));
    assert!(report.contains("\"protocol_flow_count\":1"));
    assert!(report.contains("\"protocol_flows_omitted\":0"));
    assert!(report.contains("\"target\":\"scan:http:request\""));
    assert!(report.contains("\"target\":\"scan:http:response\""));
    assert!(report.contains("\"protocol_surface\":{\"protocol\":\"http\""));
    assert!(report.contains("\"entry\":\"request\""));
    assert!(report.contains("\"sibling_entries\":[\"auth-required\",\"auth-tunnel\",\"connect\",\"denied\",\"request\",\"response\"]"));
    assert!(report.contains("\"selected_overlay\":null"));
    assert!(report.contains(
        "\"reading_companions\":[{\"protocol\":\"dns\",\"entry\":\"tcp\",\"via_overlay\":\"doh\""
    ));
    assert!(report.contains("\"ingest_mode\":\"demo\""));
    assert!(report.contains("\"ingest_mode_note\":\"synthetic demo mode: useful for exercising flows and reports, not for real process attribution\""));
    assert!(report.contains("\"ingest_trust_mode\":\"synthetic-demo\""));
    assert!(report.contains("\"pid_attribution_status\":\"synthetic\""));
}

#[test]
fn scan_all_reports_compact_large_protocol_flow_details() {
    let outputs = synthesize_large_scan_outputs(2);
    let json = scan_report_json(&outputs);
    assert!(json.contains("\"protocol_flow_count\":256"));
    assert!(json.contains("\"protocol_flows_omitted\":224"));

    let text = scan_report_text(&outputs);
    assert!(text.contains("protocol_flow_count=256"));
    assert!(text.contains("protocol_flows_omitted=224"));

    let html = scan_report_html(&outputs);
    assert!(html.contains("224 additional protocol flow summaries omitted"));
}

#[test]
fn scan_report_html_renders_visual_summary() {
    let binding = compile_file(&dsl_fixture_path("http_request_path.gewy"))
        .expect("http_request_path DSL should compile");
    let export = annotate_export_trust(
        run_binding_demo(binding),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
    );
    let analysis = analysis_snapshot(&export);
    let report =
        single_target_report_html_with_analysis("scan:http:request", &export, &analysis);
    assert!(report.contains("<!DOCTYPE html>"));
    assert!(report.contains("gewyvern Scan Report"));
    assert!(report.contains("scan:http:request"));
    assert!(report.contains("Protocol Surface"));
    assert!(report.contains("default entry:</strong> request (selected)"));
    assert!(report.contains("sibling entries:</strong> auth-required | auth-tunnel | connect | denied | request | response"));
    assert!(report.contains("Process Profiles"));
    assert!(report.contains("primary module:"));
    assert!(report.contains("primary stage:"));
    assert!(report.contains("failure mode:"));
    assert!(report.contains("failure detail:"));
    assert!(report.contains("suspect modules:"));
    assert!(report.contains("mode:</strong> demo"));
    assert!(report.contains("Mode note:</strong> synthetic demo mode: useful for exercising flows and reports, not for real process attribution"));
    assert!(report.contains("trust:</strong> synthetic-demo"));
    assert!(report.contains("pid attribution:</strong> synthetic"));
    assert!(report.contains(
        "PID attribution note:</strong> pid-scoped conclusions come from synthetic demo lineage"
    ));
    assert!(report.contains("family-request-response"));
    assert!(report.contains("stage-request-response"));
    assert!(report.contains("failure-none"));
    assert!(report.contains("last_phase=receive_response"));
    assert!(report.contains("request-response</span> 1"));
    assert!(report.contains("attention targets are shown first"));
    assert!(report.contains("<details class=\"card status-healthy\">"));
}

#[test]
fn single_target_report_helpers_match_scan_report_rendering() {
    let binding = compile_file(&dsl_fixture_path("http_request_path.gewy"))
        .expect("http_request_path DSL should compile");
    let export = annotate_export_trust(
        run_binding_demo(binding),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
    );
    let analysis = analysis_snapshot(&export);
    let expected_json = scan_report_json(&[("scan:http:request".to_string(), export.clone())]);
    let expected_html = scan_report_html(&[("scan:http:request".to_string(), export.clone())]);

    assert_eq!(
        crate::report_runtime::single_target_report_json_with_analysis(
            "scan:http:request",
            &export,
            &analysis,
        ),
        expected_json
    );
    assert_eq!(
        crate::report_runtime::single_target_report_html_with_analysis(
            "scan:http:request",
            &export,
            &analysis,
        ),
        expected_html
    );
}

#[test]
fn single_target_html_report_renders_visual_summary() {
    let binding = compile_file(&dsl_fixture_path("mysql_query_session.gewy"))
        .expect("mysql_query_session DSL should compile");
    let export = annotate_export_trust(
        run_binding_demo(binding),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
    );
    let cli = Cli::from_args([
        "--protocol".to_string(),
        "mysql".to_string(),
        "--entry".to_string(),
        "session".to_string(),
        "--report-format".to_string(),
        "html".to_string(),
    ])
    .unwrap();
    let rendered = render_report_outputs(&cli, &[("scan:mysql:session".to_string(), export)]);
    assert!(rendered.contains("<!DOCTYPE html>"));
    assert!(rendered.contains("scan:mysql:session"));
}

#[test]
fn scan_report_html_expands_attention_targets_by_default() {
    let binding = compile_file(&dsl_fixture_path("http_request_path.gewy"))
        .expect("http_request_path DSL should compile");
    let mut attention_export = annotate_export_trust(
        run_binding_demo(binding),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
    );
    let flow = attention_export.program_flows[0].clone();
    attention_export.program_findings.push(ProgramFinding {
        program_flow: flow.id,
        process: flow.process.clone(),
        operation: flow.operation.clone(),
        module_label: "http_request_path".into(),
        network_module_kind: "http_request_response".into(),
        phase: Some("receive_response".into()),
        phase_kind: Some("receive_payload".into()),
        phase_transition: Some("send_request->receive_response".into()),
        phase_transition_kind: Some("emit_payload->receive_payload".into()),
        suspect_area: "transport_io".into(),
        cause: ProgramFindingCause::MissingCoreStage,
        summary: "synthetic missing response".into(),
        supporting_fragments: vec!["tcp_packet_meta_fragment".into()],
        evidence_trace: vec!["missing_signal:packet_observed".into()],
    });
    let analysis = analysis_snapshot(&attention_export);
    let report = single_target_report_html_with_analysis(
        "scan:http:attention",
        &attention_export,
        &analysis,
    );
    assert!(report.contains("<details class=\"card status-attention\" open>"));
    assert!(report.contains("scan:http:attention"));
}

#[test]
fn scan_report_text_includes_protocol_surface_summary() {
    let binding = compile_file(&dsl_fixture_path("mysql_query_session.gewy"))
        .expect("mysql_query_session DSL should compile");
    let export = annotate_export_trust(
        run_binding_demo(binding),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
    );
    let analysis = analysis_snapshot(&export);
    let expected_spine = format!(
        "diagnosis_spine=family={} posture={} outcome={}",
        analysis.primary_module_family, analysis.evidence_posture, analysis.automation_outcome
    );
    let report = scan_report_text(&[("scan:mysql:session".to_string(), export)]);
    assert!(report.contains(&expected_spine));
    assert!(report.contains("protocol_surface=mysql"));
    assert!(report.contains("entry=session"));
    assert!(report.contains("default=session"));
    assert!(report.contains("selected_default=true"));
    assert!(report.contains("entry_aliases=mysql-session | mysql_session"));
}

#[test]
fn scan_report_text_and_html_include_protocol_reading_companions() {
    let binding = compile_file(&dsl_fixture_path("tls_client_path.gewy"))
        .expect("tls_client_path DSL should compile");
    let export = annotate_export_trust(
        run_binding_demo(binding),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
    );
    let text = scan_report_text(&[("scan:tls:client".to_string(), export.clone())]);
    assert!(text.contains("selected_overlay=none"));
    assert!(text.contains("reading_companions=https:connect@https"));
    assert!(text.contains("dns:tcp@dot"));

    let analysis = analysis_snapshot(&export);
    let html = single_target_report_html_with_analysis("scan:tls:client", &export, &analysis);
    assert!(html.contains("selected overlay:</strong> none"));
    assert!(html.contains("reading companions:</strong> https:connect via https (HTTPS Over TLS)"));
    assert!(html.contains("dns:tcp via dot (DNS-Over-TLS)"));
}

#[cfg(target_family = "unix")]
#[test]
fn scan_report_html_rolls_up_sidecar_collaboration_counts() {
    with_fake_etragon_hook(
        "{\"augmentations\":[{\"kind\":\"ml-candidate\",\"name\":\"ml_candidate_targeted_escalation\",\"summary\":\"external engine suggests targeted escalation\",\"confidence\":\"candidate\",\"producer_stage\":\"candidate\",\"producer_pass\":\"fake_etragon\",\"data\":{\"module\":\"http_request_response\"}}],\"evidence_chain_enrichment\":{\"status\":\"reinforced\",\"primary_label\":\"targeted_escalation\",\"summary\":\"reinforced evidence chain\",\"handoff_readiness\":\"automation_worthy\",\"gewyvern_merge_hint\":\"augmentations_with_operator_guidance_support\"},\"diagnostic_opinion\":{\"status\":\"ready\",\"diagnosis_kind\":\"direct_protocol_failure\",\"label\":\"targeted_escalation\",\"summary\":\"direct protocol failure is now the most direct opinion\",\"handoff_readiness\":\"automation_worthy\",\"gewyvern_merge_hint\":\"operator_guidance_candidate\"}}",
        || {
            let binding = compile_file(&dsl_fixture_path("http_request_path.gewy"))
                .expect("http_request_path DSL should compile");
            let export = annotate_export_trust(
                run_binding_demo(binding),
                &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
            );
            let report = scan_report_html(&[
                ("scan:http:request".to_string(), export.clone()),
                ("scan:http:request-2".to_string(), export),
            ]);
            assert!(report.contains("automation-worthy sidecar targets:</strong> 2"));
        },
    );
}

#[test]
fn export_primary_conclusion_prefers_attention_process_profile() {
    let binding = compile_file(&dsl_fixture_path("http_request_path.gewy"))
        .expect("http_request_path DSL should compile");
    let mut export = annotate_export_trust(
        run_binding_demo(binding),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
    );

    let healthy_flow = export.program_flows[0].clone();
    let mut attention_flow = healthy_flow.clone();
    attention_flow.id = gewyvern::flow::ProgramFlowId(healthy_flow.id.0 + 1000);
    if let Some(process) = &mut attention_flow.process {
        process.pid = 4242;
        process.comm = "apt".into();
    }
    export.program_flows.push(attention_flow.clone());
    export.program_findings.push(ProgramFinding {
        program_flow: attention_flow.id,
        process: attention_flow.process.clone(),
        operation: attention_flow.operation.clone(),
        module_label: "http_request_path".into(),
        network_module_kind: "http_request_response".into(),
        phase: Some("receive_response".into()),
        phase_kind: Some("receive_payload".into()),
        phase_transition: Some("send_request->receive_response".into()),
        phase_transition_kind: Some("emit_payload->receive_payload".into()),
        suspect_area: "transport_io".into(),
        cause: ProgramFindingCause::MissingCoreStage,
        summary: "synthetic missing response".into(),
        supporting_fragments: vec!["tcp_packet_meta_fragment".into()],
        evidence_trace: vec!["missing_signal:packet_observed".into()],
    });

    assert_eq!(
        crate::primary_module_kind_for_export(&export),
        "http_request_response"
    );
    assert_eq!(
        crate::primary_failure_stage_for_export(&export),
        "send_request->receive_response"
    );
    assert_eq!(
        crate::primary_failure_mode_for_export(&export),
        "no_response"
    );
    assert_eq!(
        crate::primary_failure_detail_for_export(&export),
        "request_sent_no_reply"
    );
    assert_eq!(
        crate::suspect_modules_for_export(&export),
        "http_request_path"
    );
}

#[test]
fn single_target_json_report_wraps_protocol_result() {
    let binding = compile_file(&dsl_fixture_path("mysql_query_session.gewy"))
        .expect("mysql_query_session DSL should compile");
    let export = annotate_export_trust(
        run_binding_demo(binding),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
    );
    let cli = Cli::from_args([
        "--protocol".to_string(),
        "mysql".to_string(),
        "--entry".to_string(),
        "session".to_string(),
        "--report-format".to_string(),
        "json".to_string(),
    ])
    .unwrap();
    let rendered = render_report_outputs(&cli, &[("scan:mysql:session".to_string(), export)]);
    assert!(rendered.contains("\"scan_all\":true"));
    assert!(rendered.contains("\"target\":\"scan:mysql:session\""));
}

#[test]
fn summary_json_marks_protocol_flow_attention_and_missing_transition() {
    let binding = compile_file(&dsl_fixture_path("http_request_path.gewy"))
        .expect("http_request_path DSL should compile");
    let mut export = annotate_export_trust(
        run_binding_demo(binding),
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
    let json = summary_json(TARGET_NAME, &export);
    assert!(json.contains("\"status\":\"attention\""));
    assert!(json.contains("\"network_module_kind\":\"http_request_response\""));
    assert!(json.contains("\"network_module_kinds\":[\"http_request_response\"]"));
    assert!(json.contains("\"process_network_profiles\":["));
    assert!(json.contains("\"missing_transitions\":[\"send_request->receive_response\"]"));
    assert!(json.contains("\"attention_flows\":1"));
    assert!(json.contains("\"missing_transitions\":[\"send_request->receive_response\"]"));
    assert!(json.contains("\"suspect_areas\":[\"transport_io\"]"));
    assert!(json.contains("\"primary_module_kind\":\"http_request_response\""));
    assert!(json.contains("\"primary_failure_stage\":\"send_request->receive_response\""));
    assert!(
        json.contains("\"primary_failure_mode\":\"no_response\""),
        "json={}",
        json
    );
    assert!(
        json.contains("\"primary_failure_detail\":\"request_sent_no_reply\""),
        "json={}",
        json
    );
    assert!(
        json.contains("\"primary_failure_confidence\":\"medium\""),
        "json={}",
        json
    );
    assert!(
        json.contains("\"primary_failure_basis\":\"missing_transition\""),
        "json={}",
        json
    );
}

#[test]
fn scan_report_json_promotes_top_level_diagnosis_aggregates() {
    let binding = compile_file(&dsl_fixture_path("http_request_path.gewy"))
        .expect("http_request_path DSL should compile");
    let mut export = annotate_export_trust(
        run_binding_demo(binding),
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
    let analysis = analysis_snapshot(&export);
    let json = single_target_report_json_with_analysis(TARGET_NAME, &export, &analysis);
    assert!(json.contains("\"primary_module_family\":\"request-response\""));
    assert!(json.contains("\"evidence_posture\":\"missing_transition\""));
    assert!(json.contains("\"automation_outcome\":\"collect_more_evidence\""));
    assert!(json.contains("\"operations\":[\"http_request\"]"));
    assert!(json.contains(
        "\"phases\":[\"bind\",\"resolve_upstream\",\"connect\",\"establish\",\"send_request\",\"receive_response\"]"
    ));
    assert!(json.contains("\"missing_transitions\":[\"send_request->receive_response\"]"));
    assert!(json.contains("\"suspect_areas\":[\"transport_io\"]"));
}
