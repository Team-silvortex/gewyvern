use super::*;

#[test]
fn summary_json_carries_modern_protocol_failure_detail() {
    let binding = compile_file(&dsl_fixture_path("http3_request_path.gewy"))
        .expect("http3_request_path DSL should compile");
    let mut export = annotate_export_trust(
        run_binding_demo(binding),
        &Cli::from_args(["--demo".to_string(), "udp".to_string()]).unwrap(),
    );
    let flow = export.program_flows[0].clone();
    push_synthetic_missing_stage_finding(
        &mut export,
        &flow,
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
    let json = summary_json("dsl_demo", &export);
    assert!(json.contains("\"primary_module_kind\":\"http3_request_response\""));
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
    assert!(json.contains("\"failure_detail\":\"request_sent_no_reply\""));
}

#[test]
fn summary_json_carries_tls_handshake_incomplete_detail() {
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
    let json = summary_json("dsl_demo", &export);
    assert!(json.contains("\"primary_module_kind\":\"tls_handshake\""));
    assert!(
        json.contains("\"primary_failure_mode\":\"setup_incomplete\""),
        "json={}",
        json
    );
    assert!(json.contains("\"primary_failure_detail\":\"handshake_incomplete\""));
}

#[test]
fn summary_json_carries_ssh_banner_timeout_detail() {
    let binding = compile_file(&dsl_fixture_path("ssh_session_path.gewy"))
        .expect("ssh_session_path DSL should compile");
    let mut export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 8281, 53022, "ssh-client"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    8281,
                    7,
                    SessionId(1),
                ),
                tcp_state_fact_with_ports_for_tests(3, 8281, 1, 2, 53022, 22),
            ],
        ),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
    );
    let flow = export.program_flows[0].clone();
    push_synthetic_missing_stage_finding(
        &mut export,
        &flow,
        "ssh_session_path",
        "remote_access_session",
        "receive_server_banner",
        "receive_payload",
        "connect->receive_server_banner",
        "initiate_connection->receive_payload",
        "transport_io",
        "synthetic missing ssh server banner",
        "tcp_packet_meta_fragment",
        "missing_signal:packet_observed",
    );
    let json = summary_json("dsl_demo", &export);
    assert!(json.contains("\"primary_module_kind\":\"remote_access_session\""));
    assert!(json.contains("\"primary_failure_mode\":\"setup_incomplete\""));
    assert!(json.contains("\"primary_failure_detail\":\"handshake_incomplete\""));
    assert!(json.contains("\"primary_failure_confidence\":\"medium\""));
    assert!(json.contains("\"primary_failure_basis\":\"missing_transition\""));
}

#[test]
fn summary_json_carries_ssh_kex_followup_detail() {
    let binding = compile_file(&dsl_fixture_path("ssh_session_path.gewy"))
        .expect("ssh_session_path DSL should compile");
    let export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 8282, 53023, "ssh-client"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    8282,
                    7,
                    SessionId(1),
                ),
                tcp_state_fact_with_ports_for_tests(3, 8282, 1, 2, 53023, 22),
                packet_fact_with_dir_and_payload_for_tests(
                    4,
                    8282,
                    0x18,
                    PacketDir::Ingress,
                    Some(53023),
                    Some(22),
                    Some(0x53),
                    Some(0x5353),
                    Some(0x5353482d),
                ),
                packet_fact_with_dir_and_payload_for_tests(
                    5,
                    8282,
                    0x18,
                    PacketDir::Egress,
                    Some(53023),
                    Some(22),
                    Some(0x53),
                    Some(0x5353),
                    Some(0x5353482d),
                ),
            ],
        ),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
    );
    let json = summary_json("dsl_demo", &export);
    assert!(json.contains("\"primary_module_kind\":\"remote_access_session\""));
    assert!(
        json.contains("\"primary_failure_mode\":\"not_sent\""),
        "json={}",
        json
    );
    assert!(json.contains("\"primary_failure_detail\":\"followup_not_sent\""));
    assert!(json.contains("\"primary_failure_confidence\":\"medium\""));
    assert!(json.contains("\"primary_failure_basis\":\"missing_transition\""));
}

#[test]
fn summary_json_carries_ssh_auth_timeout_detail() {
    let binding = compile_file(&dsl_fixture_path("ssh_auth_path.gewy"))
        .expect("ssh_auth_path DSL should compile");
    let mut export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 8289, 53028, "ssh-client"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    8289,
                    7,
                    SessionId(1),
                ),
                tcp_state_fact_with_ports_for_tests(3, 8289, 1, 2, 53028, 22),
                packet_fact_with_dir_and_payload_for_tests(
                    4,
                    8289,
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
                    8289,
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
                    8289,
                    0x18,
                    PacketDir::Egress,
                    Some(53028),
                    Some(22),
                    &[(0, 0x00), (4, 0x10), (5, 0x14)],
                ),
                packet_fact_with_dir_and_payload_bytes_for_tests(
                    7,
                    8289,
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
    let json = summary_json("dsl_demo", &export);
    assert!(json.contains("\"primary_module_kind\":\"remote_access_authentication\""));
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
fn summary_json_carries_ssh_channel_timeout_detail() {
    let binding = compile_file(&dsl_fixture_path("ssh_channel_session_path.gewy"))
        .expect("ssh_channel_session_path DSL should compile");
    let mut export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 8290, 53029, "ssh-client"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    8290,
                    7,
                    SessionId(1),
                ),
                tcp_state_fact_with_ports_for_tests(3, 8290, 1, 2, 53029, 22),
                packet_fact_with_dir_and_payload_for_tests(
                    4,
                    8290,
                    0x18,
                    PacketDir::Ingress,
                    Some(53029),
                    Some(22),
                    Some(0x53),
                    Some(0x5353),
                    Some(0x5353482d),
                ),
                packet_fact_with_dir_and_payload_for_tests(
                    5,
                    8290,
                    0x18,
                    PacketDir::Egress,
                    Some(53029),
                    Some(22),
                    Some(0x53),
                    Some(0x5353),
                    Some(0x5353482d),
                ),
                packet_fact_with_dir_and_payload_bytes_for_tests(
                    6,
                    8290,
                    0x18,
                    PacketDir::Egress,
                    Some(53029),
                    Some(22),
                    &[(0, 0x00), (4, 0x10), (5, 0x14)],
                ),
                packet_fact_with_dir_and_payload_bytes_for_tests(
                    7,
                    8290,
                    0x18,
                    PacketDir::Egress,
                    Some(53029),
                    Some(22),
                    &[(0, 0x00), (4, 0x10), (5, 0x32)],
                ),
                packet_fact_with_dir_and_payload_bytes_for_tests(
                    8,
                    8290,
                    0x18,
                    PacketDir::Ingress,
                    Some(53029),
                    Some(22),
                    &[(0, 0x00), (4, 0x10), (5, 0x34)],
                ),
                packet_fact_with_dir_and_payload_bytes_for_tests(
                    9,
                    8290,
                    0x18,
                    PacketDir::Egress,
                    Some(53029),
                    Some(22),
                    &[(0, 0x00), (4, 0x10), (5, 0x5a)],
                ),
            ],
        ),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
    );
    let flow = export.program_flows[0].clone();
    push_synthetic_missing_stage_finding(
        &mut export,
        &flow,
        "ssh_channel_session_path",
        "remote_access_session",
        "receive_channel_open_confirmation",
        "receive_payload",
        "send_channel_open->receive_channel_open_confirmation",
        "emit_payload->receive_payload",
        "transport_io",
        "synthetic missing ssh channel open confirmation",
        "tcp_packet_meta_fragment",
        "missing_signal:packet_observed",
    );
    let json = summary_json("dsl_demo", &export);
    assert!(json.contains("\"primary_module_kind\":\"remote_access_session\""));
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
