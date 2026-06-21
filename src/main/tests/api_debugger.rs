use super::*;

#[test]
fn anomaly_flow_route_highlights_missing_transition_breakpoint() {
    let _guard = test_guard();
    set_external_analysis_config(None);
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy")
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
    let state = Arc::new(Mutex::new(Arc::new(ApiSnapshot::default())));
    update_api_snapshot_for_single(
        &state,
        ApiRenderedTarget {
            name: "scan:http:request".into(),
            primary_module_family: analysis.primary_module_family.clone(),
            evidence_posture: analysis.evidence_posture.clone(),
            automation_outcome: analysis.automation_outcome.clone(),
            summary_text: summary_line("scan:http:request", &export),
            summary_json: summary_json("scan:http:request", &export),
            findings_json: findings_json("scan:http:request", &export),
            analysis_json: analysis_snapshot_json(&analysis),
            training_example_json: training_example_json("scan:http:request", &export),
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
            report_json: scan_report_json(&[("scan:http:request".to_string(), export.clone())]),
            report_html: scan_report_html(&[("scan:http:request".to_string(), export.clone())]),
        },
    );
    let snapshot = state.lock().unwrap().clone();
    let (status, content_type, body) = api_response_for_request(
        "/v1/latest/targets/scan:http:request/anomaly-flow.json",
        &snapshot,
    );
    assert_eq!(status, 200);
    assert_eq!(content_type, "application/json; charset=utf-8");
    assert!(body.contains("\"surface\":\"anomaly_flow_view\""));
    assert!(body.contains("\"target\":\"scan:http:request\""));
    assert!(body.contains("\"protocol\":\"http\""));
    assert!(body.contains("\"entry\":\"request\""));
    assert!(body.contains("\"primary_failure_stage\":\"send_request->receive_response\""));
    assert!(body.contains("\"evidence_posture\":\"missing_transition\""));
    assert!(body.contains("\"automation_outcome\":\"collect_more_evidence\""));
    assert!(body.contains("\"breakpoint_transition\":\"send_request->receive_response\""));
    assert!(body.contains("\"next_debug_step\":\"inspect evidence around missing transition send_request->receive_response\""));
    assert!(body.contains("\"breakpoint_hint\":\""));
    assert!(body.contains("\"attention_flows\":[{"));
    assert!(body.contains("\"failure_mode\":\"no_response\""));
    assert!(body.contains("\"last_phase_hint\":\""));
    assert!(body.contains("\"phase_hints\":["));
}

#[test]
fn anomaly_flow_route_returns_404_without_analysis_snapshot() {
    let state = Arc::new(Mutex::new(Arc::new(ApiSnapshot::default())));
    update_api_snapshot_for_single(
        &state,
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
    );
    let snapshot = state.lock().unwrap().clone();
    let (status, _, body) =
        api_response_for_request("/v1/latest/targets/dsl_demo/anomaly-flow.json", &snapshot);
    assert_eq!(status, 404);
    assert!(body.contains("no anomaly flow view available for target"));
}

#[test]
fn anomaly_flow_route_uses_tls_specific_phase_hint() {
    let _guard = test_guard();
    set_external_analysis_config(None);
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/tls_client_path.gewy")
        .expect("tls_client_path DSL should compile");
    let mut export = annotate_export_trust(
        run_binding_demo(binding),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
    );
    let flow = export.program_flows[0].clone();
    push_synthetic_missing_stage_finding(
        &mut export,
        &flow,
        "tls_client_path",
        "tls_handshake",
        "receive_server_hello",
        "receive_payload",
        "send_client_hello->receive_server_hello",
        "emit_payload->receive_payload",
        "transport_io",
        "synthetic missing tls server hello",
        "tcp_packet_meta_fragment",
        "missing_signal:packet_observed",
    );
    let analysis = analysis_snapshot(&export);
    let state = Arc::new(Mutex::new(Arc::new(ApiSnapshot::default())));
    update_api_snapshot_for_single(
        &state,
        ApiRenderedTarget {
            name: "scan:tls:client".into(),
            primary_module_family: analysis.primary_module_family.clone(),
            evidence_posture: analysis.evidence_posture.clone(),
            automation_outcome: analysis.automation_outcome.clone(),
            summary_text: summary_line("scan:tls:client", &export),
            summary_json: summary_json("scan:tls:client", &export),
            findings_json: findings_json("scan:tls:client", &export),
            analysis_json: analysis_snapshot_json(&analysis),
            training_example_json: training_example_json("scan:tls:client", &export),
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
            report_json: scan_report_json(&[("scan:tls:client".to_string(), export.clone())]),
            report_html: scan_report_html(&[("scan:tls:client".to_string(), export.clone())]),
        },
    );
    let snapshot = state.lock().unwrap().clone();
    let (status, _, body) = api_response_for_request(
        "/v1/latest/targets/scan:tls:client/anomaly-flow.json",
        &snapshot,
    );
    assert_eq!(status, 200);
    assert!(body.contains("\"protocol\":\"tls\""), "body={}", body);
    assert!(
        body.contains("TLS client hello should be emitted here"),
        "body={}",
        body
    );
}

#[test]
fn anomaly_flow_route_uses_dns_tcp_specific_phase_hint() {
    let _guard = test_guard();
    set_external_analysis_config(None);
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/dns_tcp_query_path.gewy")
        .expect("dns_tcp_query_path DSL should compile");
    let mut export = annotate_export_trust(
        run_binding_demo(binding),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
    );
    let flow = export.program_flows[0].clone();
    push_synthetic_missing_stage_finding(
        &mut export,
        &flow,
        "dns_tcp_query_path",
        "dns_tcp_query",
        "receive_response",
        "receive_payload",
        "send_query->receive_response",
        "emit_payload->receive_payload",
        "transport_io",
        "synthetic missing dns tcp response",
        "tcp_packet_meta_fragment",
        "missing_signal:packet_observed",
    );
    let analysis = analysis_snapshot(&export);
    let state = Arc::new(Mutex::new(Arc::new(ApiSnapshot::default())));
    update_api_snapshot_for_single(
        &state,
        ApiRenderedTarget {
            name: "scan:dns:tcp".into(),
            primary_module_family: analysis.primary_module_family.clone(),
            evidence_posture: analysis.evidence_posture.clone(),
            automation_outcome: analysis.automation_outcome.clone(),
            summary_text: summary_line("scan:dns:tcp", &export),
            summary_json: summary_json("scan:dns:tcp", &export),
            findings_json: findings_json("scan:dns:tcp", &export),
            analysis_json: analysis_snapshot_json(&analysis),
            training_example_json: training_example_json("scan:dns:tcp", &export),
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
            report_json: scan_report_json(&[("scan:dns:tcp".to_string(), export.clone())]),
            report_html: scan_report_html(&[("scan:dns:tcp".to_string(), export.clone())]),
        },
    );
    let snapshot = state.lock().unwrap().clone();
    let (status, _, body) = api_response_for_request(
        "/v1/latest/targets/scan:dns:tcp/anomaly-flow.json",
        &snapshot,
    );
    assert_eq!(status, 200);
    assert!(body.contains("\"protocol\":\"dns\""), "body={}", body);
    assert!(body.contains("\"entry\":\"tcp\""), "body={}", body);
    assert!(
        body.contains("DNS resolver target should be selected here"),
        "body={}",
        body
    );
}

#[test]
fn anomaly_flow_route_uses_dot_and_doh_specific_phase_hints() {
    let _guard = test_guard();
    set_external_analysis_config(None);

    let dns_binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/dns_tcp_query_path.gewy")
        .expect("dns_tcp_query_path DSL should compile");
    let mut dns_export = annotate_export_trust(
        run_binding_demo(dns_binding),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
    );
    let dns_flow = dns_export.program_flows[0].clone();
    push_synthetic_missing_stage_finding(
        &mut dns_export,
        &dns_flow,
        "dns_tcp_query_path",
        "dns_tcp_query",
        "receive_response",
        "receive_payload",
        "send_query->receive_response",
        "emit_payload->receive_payload",
        "transport_io",
        "synthetic missing dot response",
        "tcp_packet_meta_fragment",
        "missing_signal:packet_observed",
    );
    let dns_analysis = analysis_snapshot(&dns_export);
    let dot_state = Arc::new(Mutex::new(Arc::new(ApiSnapshot::default())));
    update_api_snapshot_for_single(
        &dot_state,
        ApiRenderedTarget {
            name: "scan:dot:tcp".into(),
            primary_module_family: dns_analysis.primary_module_family.clone(),
            evidence_posture: dns_analysis.evidence_posture.clone(),
            automation_outcome: dns_analysis.automation_outcome.clone(),
            summary_text: summary_line("scan:dot:tcp", &dns_export),
            summary_json: summary_json("scan:dot:tcp", &dns_export),
            findings_json: findings_json("scan:dot:tcp", &dns_export),
            analysis_json: analysis_snapshot_json(&dns_analysis),
            training_example_json: training_example_json("scan:dot:tcp", &dns_export),
            has_external_sidecar_context: false,
            has_external_evidence_chain_enrichment: false,
            has_external_diagnostic_opinion: false,
            has_external_capability_profile: false,
            external_capability_status: None,
            external_hint_status: None,
            external_context_status: None,
            external_sidecar_trust_level: None,
            external_sidecar_consumption_mode: None,
            export_json: dns_export.to_json(),
            report_json: scan_report_json(&[("scan:dot:tcp".to_string(), dns_export.clone())]),
            report_html: scan_report_html(&[("scan:dot:tcp".to_string(), dns_export.clone())]),
        },
    );
    let dot_snapshot = dot_state.lock().unwrap().clone();
    let (dot_status, _, dot_body) = api_response_for_request(
        "/v1/latest/targets/scan:dot:tcp/anomaly-flow.json",
        &dot_snapshot,
    );
    assert_eq!(dot_status, 200);
    assert!(
        dot_body.contains("\"protocol\":\"dns\""),
        "body={}",
        dot_body
    );
    assert!(
        dot_body.contains("DNS-over-TLS resolver should be selected here"),
        "body={}",
        dot_body
    );

    let http_binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy")
        .expect("http_request_path DSL should compile");
    let mut http_export = annotate_export_trust(
        run_binding_demo(http_binding),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
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
        "synthetic missing doh response",
        "tcp_packet_meta_fragment",
        "missing_signal:packet_observed",
    );
    let http_analysis = analysis_snapshot(&http_export);
    let doh_state = Arc::new(Mutex::new(Arc::new(ApiSnapshot::default())));
    update_api_snapshot_for_single(
        &doh_state,
        ApiRenderedTarget {
            name: "scan:doh:request".into(),
            primary_module_family: http_analysis.primary_module_family.clone(),
            evidence_posture: http_analysis.evidence_posture.clone(),
            automation_outcome: http_analysis.automation_outcome.clone(),
            summary_text: summary_line("scan:doh:request", &http_export),
            summary_json: summary_json("scan:doh:request", &http_export),
            findings_json: findings_json("scan:doh:request", &http_export),
            analysis_json: analysis_snapshot_json(&http_analysis),
            training_example_json: training_example_json("scan:doh:request", &http_export),
            has_external_sidecar_context: false,
            has_external_evidence_chain_enrichment: false,
            has_external_diagnostic_opinion: false,
            has_external_capability_profile: false,
            external_capability_status: None,
            external_hint_status: None,
            external_context_status: None,
            external_sidecar_trust_level: None,
            external_sidecar_consumption_mode: None,
            export_json: http_export.to_json(),
            report_json: scan_report_json(&[("scan:doh:request".to_string(), http_export.clone())]),
            report_html: scan_report_html(&[("scan:doh:request".to_string(), http_export.clone())]),
        },
    );
    let doh_snapshot = doh_state.lock().unwrap().clone();
    let (doh_status, _, doh_body) = api_response_for_request(
        "/v1/latest/targets/scan:doh:request/anomaly-flow.json",
        &doh_snapshot,
    );
    assert_eq!(doh_status, 200);
    assert!(
        doh_body.contains("\"protocol\":\"http\""),
        "body={}",
        doh_body
    );
    assert!(
        doh_body.contains("DNS-over-HTTPS request should leave here"),
        "body={}",
        doh_body
    );
}
