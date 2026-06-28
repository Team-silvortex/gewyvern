use super::*;

#[test]
fn summary_json_carries_rtsp_describe_timeout_detail() {
    let binding = compile_file(&dsl_fixture_path("rtsp_describe_path.gewy"))
        .expect("rtsp_describe_path DSL should compile");
    let mut export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 82930, 53052, "vlc"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    82930,
                    7,
                    SessionId(1),
                ),
                tcp_state_fact_with_ports_for_tests(3, 82930, 1, 2, 53052, 554),
                tcp_state_fact_with_ports_for_tests(4, 82930, 2, 3, 53052, 554),
                packet_fact_with_dir_and_payload_bytes_for_tests(
                    5,
                    82930,
                    0x18,
                    PacketDir::Egress,
                    Some(53052),
                    Some(554),
                    &[(0, 0x4f), (1, 0x50), (2, 0x54), (3, 0x49)],
                ),
                packet_fact_with_dir_and_payload_bytes_for_tests(
                    6,
                    82930,
                    0x18,
                    PacketDir::Ingress,
                    Some(53052),
                    Some(554),
                    &[
                        (0, 0x52),
                        (1, 0x54),
                        (2, 0x53),
                        (3, 0x50),
                        (9, 0x32),
                        (10, 0x30),
                        (11, 0x30),
                        (17, 0x50),
                        (18, 0x75),
                        (19, 0x62),
                        (20, 0x6c),
                    ],
                ),
                packet_fact_with_dir_and_payload_bytes_for_tests(
                    7,
                    82930,
                    0x18,
                    PacketDir::Egress,
                    Some(53052),
                    Some(554),
                    &[(0, 0x44), (1, 0x45), (2, 0x53), (3, 0x43)],
                ),
            ],
        ),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
    );
    let flow = export.program_flows[0].clone();
    push_synthetic_missing_stage_finding(
        &mut export,
        &flow,
        "rtsp_describe_path",
        "signaling_session",
        "receive_describe_ok",
        "receive_payload",
        "send_describe->receive_describe_ok",
        "emit_payload->receive_payload",
        "transport_io",
        "synthetic missing rtsp describe ok",
        "tcp_packet_meta_fragment",
        "missing_signal:packet_observed",
    );
    let json = summary_json("dsl_demo", &export);
    assert!(json.contains("\"primary_module_kind\":\"signaling_session\""));
    assert!(json.contains("\"primary_failure_mode\":\"no_response\""));
    assert!(json.contains("\"primary_failure_detail\":\"request_sent_no_reply\""));
    assert!(json.contains("\"primary_failure_confidence\":\"medium\""));
    assert!(json.contains("\"primary_failure_basis\":\"missing_transition\""));
}

#[test]
fn summary_json_carries_http_connect_timeout_detail() {
    let binding = compile_file(&dsl_fixture_path("http_connect_tunnel_path.gewy"))
        .expect("http_connect_tunnel_path DSL should compile");
    let mut export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 8284, 53181, "proxy-client"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    8284,
                    7,
                    SessionId(1),
                ),
                tcp_state_fact_with_ports_for_tests(3, 8284, 1, 2, 53181, 8080),
                packet_fact_with_dir_and_payload_for_tests(
                    4,
                    8284,
                    0x18,
                    PacketDir::Egress,
                    Some(53181),
                    Some(8080),
                    Some(0x43),
                    Some(0x434f),
                    Some(0x434f4e4e),
                ),
            ],
        ),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
    );
    let flow = export.program_flows[0].clone();
    push_synthetic_missing_stage_finding(
        &mut export,
        &flow,
        "http_connect_tunnel_path",
        "proxy_tunnel_establishment",
        "receive_connect_established",
        "receive_payload",
        "send_connect_request->receive_connect_established",
        "emit_payload->receive_payload",
        "transport_io",
        "synthetic missing http connect established",
        "tcp_packet_meta_fragment",
        "missing_signal:packet_observed",
    );
    let json = summary_json("dsl_demo", &export);
    assert!(json.contains("\"primary_module_kind\":\"proxy_tunnel_establishment\""));
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
    assert!(json.contains("\"primary_failure_confidence\":\"medium\""));
    assert!(json.contains("\"primary_failure_basis\":\"missing_transition\""));
}

#[test]
fn summary_json_carries_http_connect_auth_required_detail() {
    let binding = compile_file(&dsl_fixture_path("http_connect_auth_required_path.gewy"))
        .expect("http_connect_auth_required_path DSL should compile");
    let export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 82840, 53185, "proxy-client"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    82840,
                    7,
                    SessionId(1),
                ),
                tcp_state_fact_with_ports_for_tests(3, 82840, 1, 2, 53185, 8080),
                packet_fact_with_dir_and_payload_for_tests(
                    4,
                    82840,
                    0x18,
                    PacketDir::Egress,
                    Some(53185),
                    Some(8080),
                    Some(0x43),
                    Some(0x434f),
                    Some(0x434f4e4e),
                ),
                packet_fact_with_dir_and_payload_for_tests(
                    5,
                    82840,
                    0x18,
                    PacketDir::Ingress,
                    Some(53185),
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
    assert!(json.contains("\"primary_module_kind\":\"proxy_authentication\""));
    assert!(json.contains("\"primary_failure_mode\":\"server_denied\""));
    assert!(json.contains("\"primary_failure_detail\":\"auth_required\""));
    assert!(json.contains("\"primary_failure_confidence\":\"high\""));
    assert!(json.contains("\"primary_failure_basis\":\"direct_protocol_signal\""));
}

#[test]
fn summary_json_carries_http_connect_authenticated_tunnel_pending_auth_detail() {
    let binding = compile_file(&dsl_fixture_path(
        "http_connect_authenticated_tunnel_path.gewy",
    ))
    .expect("http_connect_authenticated_tunnel_path DSL should compile");
    let mut export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 82841, 53187, "proxy-client"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    82841,
                    7,
                    SessionId(1),
                ),
                tcp_state_fact_with_ports_for_tests(3, 82841, 1, 2, 53187, 8080),
                packet_fact_with_dir_and_payload_for_tests(
                    4,
                    82841,
                    0x18,
                    PacketDir::Egress,
                    Some(53187),
                    Some(8080),
                    Some(0x43),
                    Some(0x434f),
                    Some(0x434f4e4e),
                ),
                packet_fact_with_dir_and_payload_for_tests(
                    5,
                    82841,
                    0x18,
                    PacketDir::Ingress,
                    Some(53187),
                    Some(8080),
                    Some(0x34),
                    Some(0x3430),
                    Some(0x34303720),
                ),
                packet_fact_with_dir_and_payload_for_tests(
                    6,
                    82841,
                    0x18,
                    PacketDir::Egress,
                    Some(53187),
                    Some(8080),
                    Some(0x43),
                    Some(0x434f),
                    Some(0x434f4e4e),
                ),
            ],
        ),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
    );
    let flow = export.program_flows[0].clone();
    push_synthetic_missing_stage_finding(
        &mut export,
        &flow,
        "http_connect_authenticated_tunnel_path",
        "proxy_authentication",
        "receive_connect_established",
        "receive_payload",
        "send_connect_request->receive_connect_established",
        "emit_payload->receive_payload",
        "transport_io",
        "synthetic missing authenticated http connect established",
        "tcp_packet_meta_fragment",
        "missing_signal:packet_observed",
    );
    let json = summary_json("dsl_demo", &export);
    assert!(json.contains("\"primary_module_kind\":\"proxy_authentication\""));
    assert!(
        json.contains("\"primary_failure_mode\":\"server_denied\""),
        "json={}",
        json
    );
    assert!(
        json.contains("\"primary_failure_detail\":\"auth_required\""),
        "json={}",
        json
    );
}

#[test]
fn summary_json_carries_http3_server_timeout_detail() {
    let binding = compile_file(&dsl_fixture_path("http3_server_response_path.gewy"))
        .expect("http3_server_response_path DSL should compile");
    let mut export = annotate_export_trust(
        run_binding_demo(binding),
        &Cli::from_args(["--demo".to_string(), "udp".to_string()]).unwrap(),
    );
    let flow = export.program_flows[0].clone();
    push_synthetic_missing_stage_finding(
        &mut export,
        &flow,
        "http3_server_response_path",
        "http3_request_response",
        "send_response_stream",
        "emit_payload",
        "receive_request_stream->send_response_stream",
        "receive_payload->emit_payload",
        "transport_io",
        "synthetic missing http3 server response",
        "quic_frame_meta_fragment",
        "missing_signal:quic_frame_observed",
    );
    let json = summary_json("dsl_demo", &export);
    assert!(json.contains("\"primary_module_kind\":\"http3_request_response\""));
    assert!(json.contains("\"primary_failure_mode\":\"not_sent\""));
    assert!(json.contains("\"primary_failure_detail\":\"followup_not_sent\""));
}

#[test]
fn summary_json_carries_tls_route_blocked_detail() {
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
        "establish",
        "establish_connection",
        "connect->establish",
        "initiate_connection->establish_connection",
        "route_io",
        "synthetic blocked tls route/connect",
        "route_meta_fragment",
        "missing_signal:route_resolution",
    );
    let json = summary_json("dsl_demo", &export);
    assert!(json.contains("\"primary_module_kind\":\"tls_handshake\""));
    assert!(json.contains("\"primary_failure_mode\":\"setup_incomplete\""));
    assert!(json.contains("\"primary_failure_detail\":\"route_or_connect_blocked\""));
}

#[test]
fn findings_json_carries_network_module_classification() {
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

    let json = findings_json("dsl_demo", &export);
    assert!(json.contains("\"module_findings\":["), "json={}", json);
    assert!(json.contains("\"program_findings\":["), "json={}", json);
    assert!(
        json.contains("\"network_module_kind\":\"http_request_response\""),
        "json={}",
        json
    );
    assert!(
        json.contains("\"suspect_area\":\"transport_io\""),
        "json={}",
        json
    );
}
