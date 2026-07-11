use super::{
    Cli, annotate_export_trust, coerce_export_process, dsl_fixture_path, merge_exports_for_tests,
    push_synthetic_missing_stage_finding, run_binding_demo, summary_json, synthetic_process_view,
};
use gewyvern::dsl::compile_file;
use gewyvern::flow::{ProgramFinding, ProgramFindingCause};

const HTTP_REQUEST_TARGET_NAME: &str = "scan:http:request";
const HTTP3_REQUEST_TARGET_NAME: &str = "scan:http3:request";

#[test]
fn process_profiles_lower_confidence_for_competing_missing_transition_hypotheses() {
    let binding = compile_file(&dsl_fixture_path("http_request_path.gewy"))
        .expect("http_request_path DSL should compile");
    let mut export = annotate_export_trust(
        run_binding_demo(binding),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
    );
    let primary_flow = export.program_flows[0].clone();
    push_synthetic_missing_stage_finding(
        &mut export,
        &primary_flow,
        "http_request_path",
        "http_request_response",
        "receive_response",
        "receive_payload",
        "send_request->receive_response",
        "emit_payload->receive_payload",
        "transport_io",
        "synthetic missing http response",
        "tcp_packet_meta_fragment",
        "missing_signal:packet_observed",
    );

    let mut competing_flow = primary_flow.clone();
    competing_flow.id = gewyvern::flow::ProgramFlowId(primary_flow.id.0 + 5000);
    export.program_flows.push(competing_flow.clone());
    push_synthetic_missing_stage_finding(
        &mut export,
        &competing_flow,
        "http_connect_authenticated_tunnel_path",
        "proxy_authentication",
        "receive_auth_ok",
        "receive_payload",
        "send_auth_request->receive_auth_ok",
        "emit_payload->receive_payload",
        "transport_io",
        "synthetic missing proxy auth response",
        "tcp_packet_meta_fragment",
        "missing_signal:packet_observed",
    );

    let json = crate::process_network_profiles_json(&export);
    assert!(
        json.contains("\"module_kinds\":[\"http_request_response\",\"proxy_authentication\"]"),
        "json={}",
        json
    );
    assert!(
            json.contains(
                "\"missing_transitions\":[\"send_auth_request->receive_auth_ok\",\"send_request->receive_response\"]"
            ),
            "json={}",
            json
        );
    assert!(
        json.contains("\"primary_failure_confidence\":\"low\""),
        "json={}",
        json
    );
    assert!(
        json.contains("\"primary_failure_basis\":\"missing_transition\""),
        "json={}",
        json
    );
    assert!(json.contains("\"ambiguous\":true"), "json={}", json);
    assert!(json.contains("\"competing_hypotheses\":["), "json={}", json);
    assert!(
        json.contains("\"module:proxy_authentication\""),
        "json={}",
        json
    );
    assert!(
        json.contains("\"transition:send_request->receive_response\"")
            || json.contains("\"transition:send_auth_request->receive_auth_ok\""),
        "json={}",
        json
    );
}

#[test]
fn process_profiles_lower_direct_signal_confidence_for_competing_module_hypotheses() {
    let binding = compile_file(&dsl_fixture_path("http_request_path.gewy"))
        .expect("http_request_path DSL should compile");
    let mut export = annotate_export_trust(
        run_binding_demo(binding),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
    );
    let flow = export.program_flows[0].clone();
    export.program_findings.push(ProgramFinding {
        program_flow: flow.id,
        process: flow.process.clone(),
        operation: flow.operation.clone(),
        module_label: "http_connect_auth_required_path".into(),
        network_module_kind: "proxy_authentication".into(),
        phase: Some("receive_auth_required".into()),
        phase_kind: Some("receive_payload".into()),
        phase_transition: None,
        phase_transition_kind: None,
        suspect_area: "authentication".into(),
        cause: ProgramFindingCause::MissingCoreStage,
        summary: "synthetic competing proxy auth requirement".into(),
        supporting_fragments: vec!["tcp_packet_meta_fragment".into()],
        evidence_trace: vec!["synthetic:direct_protocol_signal".into()],
    });

    let json = crate::process_network_profiles_json(&export);
    assert!(
        json.contains("\"module_kinds\":[\"http_request_response\",\"proxy_authentication\"]"),
        "json={}",
        json
    );
    assert!(
        json.contains("\"primary_failure_confidence\":\"medium\""),
        "json={}",
        json
    );
    assert!(
        json.contains("\"primary_failure_basis\":\"direct_protocol_signal\""),
        "json={}",
        json
    );
    assert!(json.contains("\"ambiguous\":true"), "json={}", json);
}

#[test]
fn summary_json_exposes_ambiguous_competing_hypotheses() {
    let binding = compile_file(&dsl_fixture_path("http_request_path.gewy"))
        .expect("http_request_path DSL should compile");
    let mut export = annotate_export_trust(
        run_binding_demo(binding),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
    );
    let primary_flow = export.program_flows[0].clone();
    push_synthetic_missing_stage_finding(
        &mut export,
        &primary_flow,
        "http_request_path",
        "http_request_response",
        "receive_response",
        "receive_payload",
        "send_request->receive_response",
        "emit_payload->receive_payload",
        "transport_io",
        "synthetic missing http response",
        "tcp_packet_meta_fragment",
        "missing_signal:packet_observed",
    );
    let mut competing_flow = primary_flow.clone();
    competing_flow.id = gewyvern::flow::ProgramFlowId(primary_flow.id.0 + 6000);
    export.program_flows.push(competing_flow.clone());
    push_synthetic_missing_stage_finding(
        &mut export,
        &competing_flow,
        "http_connect_authenticated_tunnel_path",
        "proxy_authentication",
        "receive_auth_ok",
        "receive_payload",
        "send_auth_request->receive_auth_ok",
        "emit_payload->receive_payload",
        "transport_io",
        "synthetic missing proxy auth response",
        "tcp_packet_meta_fragment",
        "missing_signal:packet_observed",
    );

    let json = summary_json(HTTP_REQUEST_TARGET_NAME, &export);
    assert!(json.contains("\"ambiguous\":true"), "json={}", json);
    assert!(json.contains("\"competing_hypotheses\":["), "json={}", json);
    assert!(
        json.contains("\"module:proxy_authentication\""),
        "json={}",
        json
    );
}

#[test]
fn mixed_dns_tls_http_profile_stays_ambiguous_and_low_confidence() {
    let process = synthetic_process_view(7001, "curl");
    let dns_export = coerce_export_process(
        annotate_export_trust(
            run_binding_demo(
                compile_file(&dsl_fixture_path("dns_udp_process.gewy"))
                    .expect("dns_udp_process DSL should compile"),
            ),
            &Cli::from_args(["--demo".to_string(), "udp".to_string()]).unwrap(),
        ),
        &process,
    );
    let tls_export = coerce_export_process(
        annotate_export_trust(
            run_binding_demo(
                compile_file(&dsl_fixture_path("tls_client_path.gewy"))
                    .expect("tls_client_path DSL should compile"),
            ),
            &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
        ),
        &process,
    );
    let mut http_export = coerce_export_process(
        annotate_export_trust(
            run_binding_demo(
                compile_file(&dsl_fixture_path("http_request_path.gewy"))
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
        "synthetic missing http response",
        "tcp_packet_meta_fragment",
        "missing_signal:packet_observed",
    );

    let export = merge_exports_for_tests(vec![dns_export, tls_export, http_export]);
    let json = summary_json(HTTP_REQUEST_TARGET_NAME, &export);
    assert!(json.contains("\"primary_module_kind\":\"http_request_response\""));
    assert!(json.contains("\"ambiguous\":true"), "json={}", json);
    assert!(
        json.contains("\"primary_failure_confidence\":\"low\""),
        "json={}",
        json
    );
    assert!(
        json.contains("\"operator_guidance_status\":\"ambiguous\""),
        "json={}",
        json
    );
    assert!(
        json.contains("\"operator_guidance_action\":\"keep_multiple_hypotheses\""),
        "json={}",
        json
    );
    assert!(json.contains("\"module:name_resolution\""), "json={}", json);
    assert!(json.contains("\"module:tls_handshake\""), "json={}", json);
}

#[test]
fn mixed_proxy_tunnel_and_upstream_request_exposes_competing_hypotheses() {
    let process = synthetic_process_view(7002, "apt");
    let proxy_export = coerce_export_process(
        annotate_export_trust(
            run_binding_demo(
                compile_file(&dsl_fixture_path(
                    "http_connect_authenticated_tunnel_path.gewy",
                ))
                .expect("http_connect_authenticated_tunnel_path DSL should compile"),
            ),
            &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
        ),
        &process,
    );
    let mut http_export = coerce_export_process(
        annotate_export_trust(
            run_binding_demo(
                compile_file(&dsl_fixture_path("http_request_path.gewy"))
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
        "synthetic missing upstream http response",
        "tcp_packet_meta_fragment",
        "missing_signal:packet_observed",
    );

    let export = merge_exports_for_tests(vec![proxy_export, http_export]);
    let json = summary_json(HTTP3_REQUEST_TARGET_NAME, &export);
    assert!(json.contains("\"ambiguous\":true"), "json={}", json);
    assert!(
        json.contains("\"primary_failure_confidence\":\"low\""),
        "json={}",
        json
    );
    assert!(
        json.contains("\"operator_guidance_status\":\"ambiguous\""),
        "json={}",
        json
    );
    assert!(
        json.contains("\"operator_guidance_action\":\"keep_multiple_hypotheses\""),
        "json={}",
        json
    );
    assert!(
        json.contains("\"module:proxy_authentication\""),
        "json={}",
        json
    );
    assert!(
        json.contains("\"transition:send_request->receive_response\""),
        "json={}",
        json
    );
}

#[test]
fn mixed_quic_http3_hy2_profile_stays_conservative() {
    let process = synthetic_process_view(7003, "proxy");
    let quic_export = coerce_export_process(
        annotate_export_trust(
            run_binding_demo(
                compile_file(&dsl_fixture_path("quic_stream_session_path.gewy"))
                    .expect("quic_stream_session_path DSL should compile"),
            ),
            &Cli::from_args(["--demo".to_string(), "udp".to_string()]).unwrap(),
        ),
        &process,
    );
    let mut http3_export = coerce_export_process(
        annotate_export_trust(
            run_binding_demo(
                compile_file(&dsl_fixture_path("http3_request_path.gewy"))
                    .expect("http3_request_path DSL should compile"),
            ),
            &Cli::from_args(["--demo".to_string(), "udp".to_string()]).unwrap(),
        ),
        &process,
    );
    let http3_flow = http3_export.program_flows[0].clone();
    push_synthetic_missing_stage_finding(
        &mut http3_export,
        &http3_flow,
        "http3_request_path",
        "http3_request_response",
        "receive_response_stream",
        "receive_payload",
        "send_request_stream->receive_response_stream",
        "emit_payload->receive_payload",
        "transport_io",
        "synthetic missing http3 response",
        "quic_frame_meta_fragment",
        "missing_signal:quic_frame_observed",
    );
    let hy2_export = coerce_export_process(
        annotate_export_trust(
            run_binding_demo(
                compile_file(&dsl_fixture_path("hy2_auth_path.gewy"))
                    .expect("hy2_auth_path DSL should compile"),
            ),
            &Cli::from_args(["--demo".to_string(), "udp".to_string()]).unwrap(),
        ),
        &process,
    );

    let export = merge_exports_for_tests(vec![quic_export, http3_export, hy2_export]);
    let json = summary_json(HTTP3_REQUEST_TARGET_NAME, &export);
    assert!(json.contains("\"primary_module_kind\":\"http3_request_response\""));
    assert!(json.contains("\"ambiguous\":true"), "json={}", json);
    assert!(
        json.contains("\"primary_failure_confidence\":\"low\""),
        "json={}",
        json
    );
    assert!(
        json.contains("\"operator_guidance_status\":\"ambiguous\""),
        "json={}",
        json
    );
    assert!(
        json.contains("\"operator_guidance_action\":\"keep_multiple_hypotheses\""),
        "json={}",
        json
    );
    assert!(
        json.contains("\"module:quic_stream_session\""),
        "json={}",
        json
    );
    assert!(
        json.contains("\"module:proxy_authentication\""),
        "json={}",
        json
    );
}
