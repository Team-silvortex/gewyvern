use super::*;

const HTTP_TARGET_NAME: &str = "scan:http:request";

#[test]
fn anomaly_flow_route_highlights_missing_transition_breakpoint() {
    let _guard = test_guard();
    set_external_analysis_config(None);
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
            report_json: single_target_report_json_with_analysis(
                "scan:http:request",
                &export,
                &analysis,
            ),
            report_html: single_target_report_html_with_analysis(
                "scan:http:request",
                &export,
                &analysis,
            ),
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
            name: HTTP_TARGET_NAME.into(),
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
    let (status, _, body) = api_response_for_request(
        "/v1/latest/targets/scan:http:request/anomaly-flow.json",
        &snapshot,
    );
    assert_eq!(status, 404);
    assert!(body.contains("no anomaly flow view available for target"));
}

#[test]
fn anomaly_flow_route_uses_tls_specific_phase_hint() {
    let _guard = test_guard();
    set_external_analysis_config(None);
    let binding = compile_file(&dsl_fixture_path("tls_client_path.gewy"))
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
            report_json: single_target_report_json_with_analysis(
                "scan:tls:client",
                &export,
                &analysis,
            ),
            report_html: single_target_report_html_with_analysis(
                "scan:tls:client",
                &export,
                &analysis,
            ),
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
    let binding = compile_file(&dsl_fixture_path("dns_tcp_query_path.gewy"))
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
            report_json: single_target_report_json_with_analysis("scan:dns:tcp", &export, &analysis),
            report_html: single_target_report_html_with_analysis("scan:dns:tcp", &export, &analysis),
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

    let dns_binding = compile_file(&dsl_fixture_path("dns_tcp_query_path.gewy"))
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
            report_json: single_target_report_json_with_analysis(
                "scan:dot:tcp",
                &dns_export,
                &dns_analysis,
            ),
            report_html: single_target_report_html_with_analysis(
                "scan:dot:tcp",
                &dns_export,
                &dns_analysis,
            ),
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

    let http_binding = compile_file(&dsl_fixture_path("http_request_path.gewy"))
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
            report_json: single_target_report_json_with_analysis(
                "scan:doh:request",
                &http_export,
                &http_analysis,
            ),
            report_html: single_target_report_html_with_analysis(
                "scan:doh:request",
                &http_export,
                &http_analysis,
            ),
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

#[test]
fn anomaly_flow_route_uses_rip_specific_phase_hints() {
    let _guard = test_guard();
    set_external_analysis_config(None);
    let binding = compile_file(&dsl_fixture_path("rip_request_path.gewy"))
        .expect("rip_request_path DSL should compile");
    let mut export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 93001, 520, "rip-query"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    93001,
                    7,
                    SessionId(1),
                ),
                udp_packet_fact_with_payload_bytes_for_tests(
                    3,
                    93001,
                    PacketDir::Egress,
                    44020,
                    520,
                    &[(0, 0x01), (1, 0x02)],
                ),
            ],
        ),
        &Cli::from_args(["--demo".to_string(), "udp".to_string()]).unwrap(),
    );
    let flow = export.program_flows[0].clone();
    push_synthetic_missing_stage_finding(
        &mut export,
        &flow,
        "rip_request_path",
        "rip_request",
        "receive_response",
        "receive_payload",
        "send_request->receive_response",
        "emit_payload->receive_payload",
        "transport_io",
        "synthetic missing rip response",
        "udp_packet_meta_fragment",
        "missing_signal:packet_observed",
    );
    let analysis = analysis_snapshot(&export);
    let state = Arc::new(Mutex::new(Arc::new(ApiSnapshot::default())));
    update_api_snapshot_for_single(
        &state,
        ApiRenderedTarget {
            name: "scan:rip:request".into(),
            primary_module_family: analysis.primary_module_family.clone(),
            evidence_posture: analysis.evidence_posture.clone(),
            automation_outcome: analysis.automation_outcome.clone(),
            summary_text: summary_line("scan:rip:request", &export),
            summary_json: summary_json("scan:rip:request", &export),
            findings_json: findings_json("scan:rip:request", &export),
            analysis_json: analysis_snapshot_json(&analysis),
            training_example_json: training_example_json("scan:rip:request", &export),
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
            report_json: single_target_report_json_with_analysis(
                "scan:rip:request",
                &export,
                &analysis,
            ),
            report_html: single_target_report_html_with_analysis(
                "scan:rip:request",
                &export,
                &analysis,
            ),
        },
    );
    let snapshot = state.lock().unwrap().clone();
    let (status, _, body) = api_response_for_request(
        "/v1/latest/targets/scan:rip:request/anomaly-flow.json",
        &snapshot,
    );
    assert_eq!(status, 200);
    assert!(body.contains("\"protocol\":\"rip\""), "body={}", body);
    assert!(body.contains("\"entry\":\"request\""), "body={}", body);
    assert!(
        body.contains("the RIP route-table request should leave here"),
        "body={}",
        body
    );
    assert!(
        body.contains(
            "the RIP neighbor route should resolve here before distance-vector exchange begins"
        ),
        "body={}",
        body
    );
}

#[test]
fn anomaly_flow_route_uses_bgp_specific_phase_hints() {
    let _guard = test_guard();
    set_external_analysis_config(None);
    let binding = compile_file(&dsl_fixture_path("bgp_open_path.gewy"))
        .expect("bgp_open_path DSL should compile");
    let mut export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 93179, 179, "bgpd"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    93179,
                    7,
                    SessionId(1),
                ),
                tcp_state_fact_with_ports_for_tests(3, 93179, 1, 2, 50179, 179),
                tcp_state_fact_with_ports_for_tests(4, 93179, 2, 3, 50179, 179),
                packet_fact_with_dir_and_payload_bytes_for_tests(
                    5,
                    93179,
                    0x18,
                    PacketDir::Egress,
                    Some(50179),
                    Some(179),
                    &[
                        (0, 0xff),
                        (1, 0xff),
                        (2, 0xff),
                        (3, 0xff),
                        (16, 0x00),
                        (17, 0x13),
                        (18, 0x01),
                    ],
                ),
            ],
        ),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
    );
    let flow = export.program_flows[0].clone();
    push_synthetic_missing_stage_finding(
        &mut export,
        &flow,
        "bgp_open_path",
        "bgp_open",
        "receive_open",
        "receive_payload",
        "send_open->receive_open",
        "emit_payload->receive_payload",
        "transport_io",
        "synthetic missing bgp open",
        "tcp_packet_meta_fragment",
        "missing_signal:packet_observed",
    );
    let analysis = analysis_snapshot(&export);
    let state = Arc::new(Mutex::new(Arc::new(ApiSnapshot::default())));
    update_api_snapshot_for_single(
        &state,
        ApiRenderedTarget {
            name: "scan:bgp:open".into(),
            primary_module_family: analysis.primary_module_family.clone(),
            evidence_posture: analysis.evidence_posture.clone(),
            automation_outcome: analysis.automation_outcome.clone(),
            summary_text: summary_line("scan:bgp:open", &export),
            summary_json: summary_json("scan:bgp:open", &export),
            findings_json: findings_json("scan:bgp:open", &export),
            analysis_json: analysis_snapshot_json(&analysis),
            training_example_json: training_example_json("scan:bgp:open", &export),
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
            report_json: single_target_report_json_with_analysis("scan:bgp:open", &export, &analysis),
            report_html: single_target_report_html_with_analysis("scan:bgp:open", &export, &analysis),
        },
    );
    let snapshot = state.lock().unwrap().clone();
    let (status, _, body) = api_response_for_request(
        "/v1/latest/targets/scan:bgp:open/anomaly-flow.json",
        &snapshot,
    );
    assert_eq!(status, 200);
    assert!(body.contains("\"protocol\":\"bgp\""), "body={}", body);
    assert!(body.contains("\"entry\":\"open\""), "body={}", body);
    assert!(
        body.contains("the BGP OPEN message should leave here"),
        "body={}",
        body
    );
    assert!(
        body.contains("the BGP peer route should resolve here before session bring-up begins"),
        "body={}",
        body
    );
}

#[test]
fn anomaly_flow_route_uses_ospf_specific_phase_hints() {
    let _guard = test_guard();
    set_external_analysis_config(None);
    let binding = compile_file(&dsl_fixture_path("ospf_hello_path.gewy"))
        .expect("ospf_hello_path DSL should compile");
    let mut export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![packet_fact_with_l4_proto_and_payload_bytes_for_tests(
                1,
                None,
                PacketDir::Egress,
                89,
                &[(0, 0x02), (1, 0x01)],
            )],
        ),
        &Cli::from_args(["--demo".to_string(), "udp".to_string()]).unwrap(),
    );
    let flow = export.program_flows[0].clone();
    push_synthetic_missing_stage_finding(
        &mut export,
        &flow,
        "ospf_hello_path",
        "ospf_hello",
        "receive_hello",
        "receive_payload",
        "send_hello->receive_hello",
        "emit_payload->receive_payload",
        "transport_io",
        "synthetic missing ospf hello",
        "udp_packet_meta_fragment",
        "missing_signal:packet_observed",
    );
    let analysis = analysis_snapshot(&export);
    let state = Arc::new(Mutex::new(Arc::new(ApiSnapshot::default())));
    update_api_snapshot_for_single(
        &state,
        ApiRenderedTarget {
            name: "scan:ospf:hello".into(),
            primary_module_family: analysis.primary_module_family.clone(),
            evidence_posture: analysis.evidence_posture.clone(),
            automation_outcome: analysis.automation_outcome.clone(),
            summary_text: summary_line("scan:ospf:hello", &export),
            summary_json: summary_json("scan:ospf:hello", &export),
            findings_json: findings_json("scan:ospf:hello", &export),
            analysis_json: analysis_snapshot_json(&analysis),
            training_example_json: training_example_json("scan:ospf:hello", &export),
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
            report_json: single_target_report_json_with_analysis(
                "scan:ospf:hello",
                &export,
                &analysis,
            ),
            report_html: single_target_report_html_with_analysis(
                "scan:ospf:hello",
                &export,
                &analysis,
            ),
        },
    );
    let snapshot = state.lock().unwrap().clone();
    let (status, _, body) = api_response_for_request(
        "/v1/latest/targets/scan:ospf:hello/anomaly-flow.json",
        &snapshot,
    );
    assert_eq!(status, 200);
    assert!(body.contains("\"protocol\":\"ospf\""), "body={}", body);
    assert!(body.contains("\"entry\":\"hello\""), "body={}", body);
    assert!(
        body.contains("the OSPF hello packet should leave here"),
        "body={}",
        body
    );
}

#[test]
fn anomaly_flow_route_uses_ntp_specific_phase_hints() {
    let _guard = test_guard();
    set_external_analysis_config(None);
    let binding = compile_file(&dsl_fixture_path("ntp_query_path.gewy"))
        .expect("ntp_query_path DSL should compile");
    let mut export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 94001, 44001, "chrony-query"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    94001,
                    7,
                    SessionId(1),
                ),
                udp_packet_fact_with_payload_bytes_for_tests(
                    3,
                    94001,
                    PacketDir::Egress,
                    54020,
                    123,
                    &[(0, 0x23), (1, 0x00)],
                ),
            ],
        ),
        &Cli::from_args(["--demo".to_string(), "udp".to_string()]).unwrap(),
    );
    let flow = export.program_flows[0].clone();
    push_synthetic_missing_stage_finding(
        &mut export,
        &flow,
        "ntp_query_path",
        "ntp_query",
        "receive_response",
        "receive_payload",
        "send_query->receive_response",
        "emit_payload->receive_payload",
        "transport_io",
        "synthetic missing ntp response",
        "udp_packet_meta_fragment",
        "missing_signal:packet_observed",
    );
    let analysis = analysis_snapshot(&export);
    let state = Arc::new(Mutex::new(Arc::new(ApiSnapshot::default())));
    update_api_snapshot_for_single(
        &state,
        ApiRenderedTarget {
            name: "scan:ntp:query".into(),
            primary_module_family: analysis.primary_module_family.clone(),
            evidence_posture: analysis.evidence_posture.clone(),
            automation_outcome: analysis.automation_outcome.clone(),
            summary_text: summary_line("scan:ntp:query", &export),
            summary_json: summary_json("scan:ntp:query", &export),
            findings_json: findings_json("scan:ntp:query", &export),
            analysis_json: analysis_snapshot_json(&analysis),
            training_example_json: training_example_json("scan:ntp:query", &export),
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
            report_json: single_target_report_json_with_analysis("scan:ntp:query", &export, &analysis),
            report_html: single_target_report_html_with_analysis("scan:ntp:query", &export, &analysis),
        },
    );
    let snapshot = state.lock().unwrap().clone();
    let (status, _, body) = api_response_for_request(
        "/v1/latest/targets/scan:ntp:query/anomaly-flow.json",
        &snapshot,
    );
    assert_eq!(status, 200);
    assert!(body.contains("\"protocol\":\"ntp\""), "body={}", body);
    assert!(body.contains("\"entry\":\"query\""), "body={}", body);
    assert!(
        body.contains("the NTP query should leave here"),
        "body={}",
        body
    );
}

#[test]
fn anomaly_flow_route_uses_dhcp_specific_phase_hints() {
    let _guard = test_guard();
    set_external_analysis_config(None);
    let binding = compile_file(&dsl_fixture_path("dhcp_discover_path.gewy"))
        .expect("dhcp_discover_path DSL should compile");
    let mut export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 94002, 68, "dhclient"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    94002,
                    7,
                    SessionId(1),
                ),
                udp_packet_fact_with_payload_bytes_for_tests(
                    3,
                    94002,
                    PacketDir::Egress,
                    68,
                    67,
                    &[(0, 0x01), (1, 0x01), (242, 0x01)],
                ),
            ],
        ),
        &Cli::from_args(["--demo".to_string(), "udp".to_string()]).unwrap(),
    );
    let flow = export.program_flows[0].clone();
    push_synthetic_missing_stage_finding(
        &mut export,
        &flow,
        "dhcp_discover_path",
        "dhcp_discover",
        "receive_offer",
        "receive_payload",
        "send_discover->receive_offer",
        "emit_payload->receive_payload",
        "transport_io",
        "synthetic missing dhcp offer",
        "udp_packet_meta_fragment",
        "missing_signal:packet_observed",
    );
    let analysis = analysis_snapshot(&export);
    let state = Arc::new(Mutex::new(Arc::new(ApiSnapshot::default())));
    update_api_snapshot_for_single(
        &state,
        ApiRenderedTarget {
            name: "scan:dhcp:client".into(),
            primary_module_family: analysis.primary_module_family.clone(),
            evidence_posture: analysis.evidence_posture.clone(),
            automation_outcome: analysis.automation_outcome.clone(),
            summary_text: summary_line("scan:dhcp:client", &export),
            summary_json: summary_json("scan:dhcp:client", &export),
            findings_json: findings_json("scan:dhcp:client", &export),
            analysis_json: analysis_snapshot_json(&analysis),
            training_example_json: training_example_json("scan:dhcp:client", &export),
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
            report_json: single_target_report_json_with_analysis(
                "scan:dhcp:client",
                &export,
                &analysis,
            ),
            report_html: single_target_report_html_with_analysis(
                "scan:dhcp:client",
                &export,
                &analysis,
            ),
        },
    );
    let snapshot = state.lock().unwrap().clone();
    let (status, _, body) = api_response_for_request(
        "/v1/latest/targets/scan:dhcp:client/anomaly-flow.json",
        &snapshot,
    );
    assert_eq!(status, 200);
    assert!(body.contains("\"protocol\":\"dhcp\""), "body={}", body);
    assert!(body.contains("\"entry\":\"client\""), "body={}", body);
    assert!(
        body.contains("the DHCP DISCOVER should leave here"),
        "body={}",
        body
    );
}

#[test]
fn anomaly_flow_route_uses_stun_specific_phase_hints() {
    let _guard = test_guard();
    set_external_analysis_config(None);
    let binding = compile_file(&dsl_fixture_path("stun_binding_path.gewy"))
        .expect("stun_binding_path DSL should compile");
    let mut export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 94003, 45001, "stun-client"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    94003,
                    7,
                    SessionId(1),
                ),
                udp_packet_fact_with_payload_bytes_for_tests(
                    3,
                    94003,
                    PacketDir::Egress,
                    54030,
                    3478,
                    &[(0, 0x00), (1, 0x01)],
                ),
            ],
        ),
        &Cli::from_args(["--demo".to_string(), "udp".to_string()]).unwrap(),
    );
    let flow = export.program_flows[0].clone();
    push_synthetic_missing_stage_finding(
        &mut export,
        &flow,
        "stun_binding_path",
        "stun_binding",
        "receive_response",
        "receive_payload",
        "send_request->receive_response",
        "emit_payload->receive_payload",
        "transport_io",
        "synthetic missing stun response",
        "udp_packet_meta_fragment",
        "missing_signal:packet_observed",
    );
    let analysis = analysis_snapshot(&export);
    let state = Arc::new(Mutex::new(Arc::new(ApiSnapshot::default())));
    update_api_snapshot_for_single(
        &state,
        ApiRenderedTarget {
            name: "scan:stun:binding".into(),
            primary_module_family: analysis.primary_module_family.clone(),
            evidence_posture: analysis.evidence_posture.clone(),
            automation_outcome: analysis.automation_outcome.clone(),
            summary_text: summary_line("scan:stun:binding", &export),
            summary_json: summary_json("scan:stun:binding", &export),
            findings_json: findings_json("scan:stun:binding", &export),
            analysis_json: analysis_snapshot_json(&analysis),
            training_example_json: training_example_json("scan:stun:binding", &export),
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
            report_json: single_target_report_json_with_analysis(
                "scan:stun:binding",
                &export,
                &analysis,
            ),
            report_html: single_target_report_html_with_analysis(
                "scan:stun:binding",
                &export,
                &analysis,
            ),
        },
    );
    let snapshot = state.lock().unwrap().clone();
    let (status, _, body) = api_response_for_request(
        "/v1/latest/targets/scan:stun:binding/anomaly-flow.json",
        &snapshot,
    );
    assert_eq!(status, 200);
    assert!(body.contains("\"protocol\":\"stun\""), "body={}", body);
    assert!(body.contains("\"entry\":\"binding\""), "body={}", body);
    assert!(
        body.contains("the STUN binding request should leave here"),
        "body={}",
        body
    );
}

#[test]
fn anomaly_flow_route_uses_snmp_specific_phase_hints() {
    let _guard = test_guard();
    set_external_analysis_config(None);
    let binding = compile_file(&dsl_fixture_path("snmp_get_path.gewy"))
        .expect("snmp_get_path DSL should compile");
    let mut export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 94004, 43001, "snmpget"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    94004,
                    7,
                    SessionId(1),
                ),
                udp_packet_fact_with_payload_bytes_for_tests(
                    3,
                    94004,
                    PacketDir::Egress,
                    49001,
                    161,
                    &[(0, 0x30), (1, 0x2a), (4, 0x02), (13, 0xa0)],
                ),
            ],
        ),
        &Cli::from_args(["--demo".to_string(), "udp".to_string()]).unwrap(),
    );
    let flow = export.program_flows[0].clone();
    push_synthetic_missing_stage_finding(
        &mut export,
        &flow,
        "snmp_get_path",
        "snmp_get",
        "receive_get_response",
        "receive_payload",
        "send_get_request->receive_get_response",
        "emit_payload->receive_payload",
        "transport_io",
        "synthetic missing snmp get response",
        "udp_packet_meta_fragment",
        "missing_signal:packet_observed",
    );
    let analysis = analysis_snapshot(&export);
    let state = Arc::new(Mutex::new(Arc::new(ApiSnapshot::default())));
    update_api_snapshot_for_single(
        &state,
        ApiRenderedTarget {
            name: "scan:snmp:get".into(),
            primary_module_family: analysis.primary_module_family.clone(),
            evidence_posture: analysis.evidence_posture.clone(),
            automation_outcome: analysis.automation_outcome.clone(),
            summary_text: summary_line("scan:snmp:get", &export),
            summary_json: summary_json("scan:snmp:get", &export),
            findings_json: findings_json("scan:snmp:get", &export),
            analysis_json: analysis_snapshot_json(&analysis),
            training_example_json: training_example_json("scan:snmp:get", &export),
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
            report_json: single_target_report_json_with_analysis("scan:snmp:get", &export, &analysis),
            report_html: single_target_report_html_with_analysis("scan:snmp:get", &export, &analysis),
        },
    );
    let snapshot = state.lock().unwrap().clone();
    let (status, _, body) = api_response_for_request(
        "/v1/latest/targets/scan:snmp:get/anomaly-flow.json",
        &snapshot,
    );
    assert_eq!(status, 200);
    assert!(body.contains("\"protocol\":\"snmp\""), "body={}", body);
    assert!(body.contains("\"entry\":\"get\""), "body={}", body);
    assert!(
        body.contains("the SNMP PDU should be emitted here"),
        "body={}",
        body
    );
}

#[test]
fn anomaly_flow_route_uses_mdns_specific_phase_hints() {
    let _guard = test_guard();
    set_external_analysis_config(None);
    let binding = compile_file(&dsl_fixture_path("mdns_query_path.gewy"))
        .expect("mdns_query_path DSL should compile");
    let mut export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 94005, 5353, "systemd-resolve"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    94005,
                    7,
                    SessionId(1),
                ),
                udp_packet_fact_with_payload_bytes_for_tests(
                    3,
                    94005,
                    PacketDir::Egress,
                    5353,
                    5353,
                    &[(0, 0x00), (1, 0x00), (2, 0x00), (3, 0x00)],
                ),
            ],
        ),
        &Cli::from_args(["--demo".to_string(), "udp".to_string()]).unwrap(),
    );
    let flow = export.program_flows[0].clone();
    push_synthetic_missing_stage_finding(
        &mut export,
        &flow,
        "mdns_query_path",
        "mdns_query",
        "receive_response",
        "receive_payload",
        "send_query->receive_response",
        "emit_payload->receive_payload",
        "transport_io",
        "synthetic missing mdns response",
        "udp_packet_meta_fragment",
        "missing_signal:packet_observed",
    );
    let analysis = analysis_snapshot(&export);
    let state = Arc::new(Mutex::new(Arc::new(ApiSnapshot::default())));
    update_api_snapshot_for_single(
        &state,
        ApiRenderedTarget {
            name: "scan:mdns:query".into(),
            primary_module_family: analysis.primary_module_family.clone(),
            evidence_posture: analysis.evidence_posture.clone(),
            automation_outcome: analysis.automation_outcome.clone(),
            summary_text: summary_line("scan:mdns:query", &export),
            summary_json: summary_json("scan:mdns:query", &export),
            findings_json: findings_json("scan:mdns:query", &export),
            analysis_json: analysis_snapshot_json(&analysis),
            training_example_json: training_example_json("scan:mdns:query", &export),
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
            report_json: single_target_report_json_with_analysis(
                "scan:mdns:query",
                &export,
                &analysis,
            ),
            report_html: single_target_report_html_with_analysis(
                "scan:mdns:query",
                &export,
                &analysis,
            ),
        },
    );
    let snapshot = state.lock().unwrap().clone();
    let (status, _, body) = api_response_for_request(
        "/v1/latest/targets/scan:mdns:query/anomaly-flow.json",
        &snapshot,
    );
    assert_eq!(status, 200);
    assert!(body.contains("\"protocol\":\"mdns\""), "body={}", body);
    assert!(body.contains("\"entry\":\"query\""), "body={}", body);
    assert!(
        body.contains("the multicast DNS query should leave here"),
        "body={}",
        body
    );
}

#[test]
fn anomaly_flow_route_uses_ssh_specific_phase_hints() {
    let _guard = test_guard();
    set_external_analysis_config(None);
    let binding = compile_file(&dsl_fixture_path("ssh_auth_path.gewy"))
        .expect("ssh_auth_path DSL should compile");
    let mut export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 94006, 53028, "ssh-client"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    94006,
                    7,
                    SessionId(1),
                ),
                tcp_state_fact_with_ports_for_tests(3, 94006, 1, 2, 53028, 22),
                packet_fact_with_dir_and_payload_for_tests(
                    4,
                    94006,
                    0x18,
                    PacketDir::Ingress,
                    Some(53028),
                    Some(22),
                    Some(0x53),
                    Some(0x5353),
                    Some(0x5353482d),
                ),
                packet_fact_with_dir_and_payload_for_tests(
                    5,
                    94006,
                    0x18,
                    PacketDir::Egress,
                    Some(53028),
                    Some(22),
                    Some(0x53),
                    Some(0x5353),
                    Some(0x5353482d),
                ),
                packet_fact_with_dir_and_payload_bytes_for_tests(
                    6,
                    94006,
                    0x18,
                    PacketDir::Egress,
                    Some(53028),
                    Some(22),
                    &[(0, 0x00), (4, 0x10), (5, 0x14)],
                ),
                packet_fact_with_dir_and_payload_bytes_for_tests(
                    7,
                    94006,
                    0x18,
                    PacketDir::Egress,
                    Some(53028),
                    Some(22),
                    &[(0, 0x00), (4, 0x10), (5, 0x32)],
                ),
            ],
        ),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
    );
    let flow = export.program_flows[0].clone();
    push_synthetic_missing_stage_finding(
        &mut export,
        &flow,
        "ssh_auth_path",
        "remote_access_authentication",
        "receive_auth_success",
        "receive_payload",
        "send_auth_request->receive_auth_success",
        "emit_payload->receive_payload",
        "transport_io",
        "synthetic missing ssh auth success",
        "tcp_packet_meta_fragment",
        "missing_signal:packet_observed",
    );
    let analysis = analysis_snapshot(&export);
    let state = Arc::new(Mutex::new(Arc::new(ApiSnapshot::default())));
    update_api_snapshot_for_single(
        &state,
        ApiRenderedTarget {
            name: "scan:ssh:auth".into(),
            primary_module_family: analysis.primary_module_family.clone(),
            evidence_posture: analysis.evidence_posture.clone(),
            automation_outcome: analysis.automation_outcome.clone(),
            summary_text: summary_line("scan:ssh:auth", &export),
            summary_json: summary_json("scan:ssh:auth", &export),
            findings_json: findings_json("scan:ssh:auth", &export),
            analysis_json: analysis_snapshot_json(&analysis),
            training_example_json: training_example_json("scan:ssh:auth", &export),
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
            report_json: single_target_report_json_with_analysis("scan:ssh:auth", &export, &analysis),
            report_html: single_target_report_html_with_analysis("scan:ssh:auth", &export, &analysis),
        },
    );
    let snapshot = state.lock().unwrap().clone();
    let (status, _, body) = api_response_for_request(
        "/v1/latest/targets/scan:ssh:auth/anomaly-flow.json",
        &snapshot,
    );
    assert_eq!(status, 200);
    assert!(body.contains("\"protocol\":\"ssh\""), "body={}", body);
    assert!(body.contains("\"entry\":\"auth\""), "body={}", body);
    assert!(
        body.contains("authentication material should be sent here"),
        "body={}",
        body
    );
}

#[test]
fn anomaly_flow_route_uses_smtp_specific_phase_hints() {
    let _guard = test_guard();
    set_external_analysis_config(None);
    let binding = compile_file(&dsl_fixture_path("smtp_auth_path.gewy"))
        .expect("smtp_auth_path DSL should compile");
    let mut export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 94007, 53013, "postfix-client"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    94007,
                    7,
                    SessionId(1),
                ),
                tcp_state_fact_with_ports_for_tests(3, 94007, 1, 2, 53013, 25),
                packet_fact_with_dir_and_payload_for_tests(
                    4,
                    94007,
                    0x18,
                    PacketDir::Ingress,
                    Some(53013),
                    Some(25),
                    Some(0x32),
                    Some(0x3232),
                    Some(0x32323020),
                ),
                packet_fact_with_dir_and_payload_for_tests(
                    5,
                    94007,
                    0x18,
                    PacketDir::Egress,
                    Some(53013),
                    Some(25),
                    Some(0x45),
                    Some(0x4548),
                    Some(0x45484c4f),
                ),
                packet_fact_with_dir_and_payload_for_tests(
                    6,
                    94007,
                    0x18,
                    PacketDir::Ingress,
                    Some(53013),
                    Some(25),
                    Some(0x32),
                    Some(0x3235),
                    Some(0x32353020),
                ),
                packet_fact_with_dir_and_payload_for_tests(
                    7,
                    94007,
                    0x18,
                    PacketDir::Egress,
                    Some(53013),
                    Some(25),
                    Some(0x41),
                    Some(0x4155),
                    Some(0x41555448),
                ),
            ],
        ),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
    );
    let flow = export.program_flows[0].clone();
    push_synthetic_missing_stage_finding(
        &mut export,
        &flow,
        "smtp_auth_path",
        "authentication_exchange",
        "receive_auth_ok",
        "receive_payload",
        "send_auth_request->receive_auth_ok",
        "emit_payload->receive_payload",
        "transport_io",
        "synthetic missing smtp auth ok",
        "tcp_packet_meta_fragment",
        "missing_signal:packet_observed",
    );
    let analysis = analysis_snapshot(&export);
    let state = Arc::new(Mutex::new(Arc::new(ApiSnapshot::default())));
    update_api_snapshot_for_single(
        &state,
        ApiRenderedTarget {
            name: "scan:smtp:auth".into(),
            primary_module_family: analysis.primary_module_family.clone(),
            evidence_posture: analysis.evidence_posture.clone(),
            automation_outcome: analysis.automation_outcome.clone(),
            summary_text: summary_line("scan:smtp:auth", &export),
            summary_json: summary_json("scan:smtp:auth", &export),
            findings_json: findings_json("scan:smtp:auth", &export),
            analysis_json: analysis_snapshot_json(&analysis),
            training_example_json: training_example_json("scan:smtp:auth", &export),
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
            report_json: single_target_report_json_with_analysis("scan:smtp:auth", &export, &analysis),
            report_html: single_target_report_html_with_analysis("scan:smtp:auth", &export, &analysis),
        },
    );
    let snapshot = state.lock().unwrap().clone();
    let (status, _, body) = api_response_for_request(
        "/v1/latest/targets/scan:smtp:auth/anomaly-flow.json",
        &snapshot,
    );
    assert_eq!(status, 200);
    assert!(body.contains("\"protocol\":\"smtp\""), "body={}", body);
    assert!(body.contains("\"entry\":\"auth\""), "body={}", body);
    assert!(
        body.contains("mail authentication should be attempted here"),
        "body={}",
        body
    );
}

#[test]
fn anomaly_flow_route_uses_redis_specific_phase_hints() {
    let _guard = test_guard();
    set_external_analysis_config(None);
    let binding = compile_file(&dsl_fixture_path("redis_get_path.gewy"))
        .expect("redis_get_path DSL should compile");
    let mut export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 94008, 43079, "redis-cli"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    94008,
                    7,
                    SessionId(1),
                ),
                tcp_state_fact_with_ports_for_tests(3, 94008, 1, 2, 43079, 6379),
                tcp_state_fact_with_ports_for_tests(4, 94008, 2, 3, 43079, 6379),
                packet_fact_with_dir_and_payload_bytes_for_tests(
                    5,
                    94008,
                    0,
                    PacketDir::Egress,
                    Some(43079),
                    Some(6379),
                    &[
                        (0, 0x2a),
                        (1, 0x32),
                        (2, 0x0d),
                        (3, 0x0a),
                        (8, 0x47),
                        (9, 0x45),
                        (10, 0x54),
                    ],
                ),
                packet_fact_with_dir_and_payload_bytes_for_tests(
                    6,
                    94008,
                    0,
                    PacketDir::Ingress,
                    Some(43079),
                    Some(6379),
                    &[(0, 0x24), (1, 0x35), (2, 0x0d), (3, 0x0a)],
                ),
            ],
        ),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
    );
    let flow = export.program_flows[0].clone();
    push_synthetic_missing_stage_finding(
        &mut export,
        &flow,
        "redis_get_path",
        "redis_get",
        "decode_value",
        "interpret_payload",
        "receive_bulk->decode_value",
        "receive_payload->interpret_payload",
        "transport_io",
        "synthetic missing redis bulk decode",
        "tcp_packet_meta_fragment",
        "missing_signal:packet_observed",
    );
    let analysis = analysis_snapshot(&export);
    let state = Arc::new(Mutex::new(Arc::new(ApiSnapshot::default())));
    update_api_snapshot_for_single(
        &state,
        ApiRenderedTarget {
            name: "scan:redis:get".into(),
            primary_module_family: analysis.primary_module_family.clone(),
            evidence_posture: analysis.evidence_posture.clone(),
            automation_outcome: analysis.automation_outcome.clone(),
            summary_text: summary_line("scan:redis:get", &export),
            summary_json: summary_json("scan:redis:get", &export),
            findings_json: findings_json("scan:redis:get", &export),
            analysis_json: analysis_snapshot_json(&analysis),
            training_example_json: training_example_json("scan:redis:get", &export),
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
            report_json: single_target_report_json_with_analysis("scan:redis:get", &export, &analysis),
            report_html: single_target_report_html_with_analysis("scan:redis:get", &export, &analysis),
        },
    );
    let snapshot = state.lock().unwrap().clone();
    let (status, _, body) = api_response_for_request(
        "/v1/latest/targets/scan:redis:get/anomaly-flow.json",
        &snapshot,
    );
    assert_eq!(status, 200);
    assert!(body.contains("\"protocol\":\"redis\""), "body={}", body);
    assert!(body.contains("\"entry\":\"get\""), "body={}", body);
    assert!(
        body.contains("bulk reply bytes should arrive here"),
        "body={}",
        body
    );
}

#[test]
fn debugger_console_rolls_up_targets_with_attention_first_focus() {
    let state = Arc::new(Mutex::new(Arc::new(ApiSnapshot::default())));
    update_api_snapshot_for_scan(
        &state,
        vec![
            debugger_target(
                "scan:http:response",
                "request-response",
                "heuristic_summary",
                "manual_review",
                "healthy",
                "receive_response",
                "none",
                "",
                "manual_review",
                "keep watching response posture",
                None,
            ),
            debugger_target(
                "scan:http:request",
                "request-response",
                "missing_transition",
                "collect_more_evidence",
                "attention",
                "send_request->receive_response",
                "no_response",
                "synthetic missing response",
                "collect_more_evidence",
                "collect packet evidence around the missing response",
                Some("send_request->receive_response"),
            ),
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
        api_response_for_request("/v1/latest/debugger-console.json", &snapshot);
    assert_eq!(status, 200);
    assert_eq!(content_type, "application/json; charset=utf-8");
    assert!(body.contains("\"surface\":\"debugger_console\""));
    assert!(body.contains("\"attention_count\":1"));
    assert!(body.contains("\"recommended_focus\":{\"name\":\"scan:http:request\""));
    assert!(body.contains("\"first_missing_transition\":\"send_request->receive_response\""));
    assert!(
        body.contains(
            "\"anomaly_flow\":\"/v1/latest/targets/scan:http:request/anomaly-flow.json\""
        )
    );

    let (cap_status, _, caps) = api_response_for_request("/v1/capabilities", &snapshot);
    assert_eq!(cap_status, 200);
    assert!(caps.contains("\"debugger_console\":true"));
    assert!(caps.contains("\"/v1/latest/debugger-console.json\""));
}

fn debugger_target(
    name: &str,
    family: &str,
    evidence: &str,
    outcome: &str,
    status: &str,
    stage: &str,
    mode: &str,
    detail: &str,
    guidance_action: &str,
    guidance_summary: &str,
    missing_transition: Option<&str>,
) -> ApiRenderedTarget {
    let missing = missing_transition
        .map(|value| format!("\"{value}\""))
        .unwrap_or_default();
    ApiRenderedTarget {
        name: name.into(),
        primary_module_family: family.into(),
        evidence_posture: evidence.into(),
        automation_outcome: outcome.into(),
        summary_text: String::new(),
        summary_json: String::new(),
        findings_json: String::new(),
        analysis_json: format!(
            "{{\"target_status\":\"{status}\",\"primary_failure_stage\":\"{stage}\",\"primary_failure_mode\":\"{mode}\",\"primary_failure_detail\":\"{detail}\",\"operator_guidance_action\":\"{guidance_action}\",\"operator_guidance_summary\":\"{guidance_summary}\",\"missing_transitions\":[{missing}]}}"
        ),
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
    }
}
