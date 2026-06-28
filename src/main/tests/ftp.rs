use super::*;

#[test]
fn summary_json_carries_ftp_banner_timeout_detail() {
    let binding = compile_file(&dsl_fixture_path("ftp_session_path.gewy"))
        .expect("ftp_session_path DSL should compile");
    let mut export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 8285, 53182, "ftp-client"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    8285,
                    7,
                    SessionId(1),
                ),
                tcp_state_fact_with_ports_for_tests(3, 8285, 1, 2, 53182, 21),
            ],
        ),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
    );
    let flow = export.program_flows[0].clone();
    push_synthetic_missing_stage_finding(
        &mut export,
        &flow,
        "ftp_session_path",
        "authentication_exchange",
        "receive_banner",
        "receive_payload",
        "connect->receive_banner",
        "initiate_connection->receive_payload",
        "transport_io",
        "synthetic missing ftp banner",
        "tcp_packet_meta_fragment",
        "missing_signal:packet_observed",
    );
    let json = summary_json("dsl_demo", &export);
    assert!(json.contains("\"primary_module_kind\":\"authentication_exchange\""));
    assert!(
        json.contains("\"primary_failure_mode\":\"setup_incomplete\""),
        "json={}",
        json
    );
    assert!(
        json.contains("\"primary_failure_detail\":\"handshake_incomplete\""),
        "json={}",
        json
    );
    assert!(json.contains("\"primary_failure_confidence\":\"medium\""));
    assert!(json.contains("\"primary_failure_basis\":\"missing_transition\""));
}

#[test]
fn summary_json_carries_ftp_auth_timeout_detail() {
    let binding = compile_file(&dsl_fixture_path("ftp_session_path.gewy"))
        .expect("ftp_session_path DSL should compile");
    let mut export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 8286, 53183, "ftp-client"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    8286,
                    7,
                    SessionId(1),
                ),
                tcp_state_fact_with_ports_for_tests(3, 8286, 1, 2, 53183, 21),
                packet_fact_with_dir_and_payload_for_tests(
                    4,
                    8286,
                    0x18,
                    PacketDir::Ingress,
                    Some(53183),
                    Some(21),
                    Some(0x32),
                    Some(0x3232),
                    Some(0x32323020),
                ),
                packet_fact_with_dir_and_payload_for_tests(
                    5,
                    8286,
                    0x18,
                    PacketDir::Egress,
                    Some(53183),
                    Some(21),
                    Some(0x55),
                    Some(0x5553),
                    Some(0x55534552),
                ),
                packet_fact_with_dir_and_payload_for_tests(
                    6,
                    8286,
                    0x18,
                    PacketDir::Ingress,
                    Some(53183),
                    Some(21),
                    Some(0x33),
                    Some(0x3333),
                    Some(0x33333120),
                ),
                packet_fact_with_dir_and_payload_for_tests(
                    7,
                    8286,
                    0x18,
                    PacketDir::Egress,
                    Some(53183),
                    Some(21),
                    Some(0x50),
                    Some(0x5041),
                    Some(0x50415353),
                ),
            ],
        ),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
    );
    let flow = export.program_flows[0].clone();
    push_synthetic_missing_stage_finding(
        &mut export,
        &flow,
        "ftp_session_path",
        "authentication_exchange",
        "receive_auth_ok",
        "receive_payload",
        "send_auth_pass->receive_auth_ok",
        "emit_payload->receive_payload",
        "transport_io",
        "synthetic missing ftp auth ok",
        "tcp_packet_meta_fragment",
        "missing_signal:packet_observed",
    );
    let json = summary_json("dsl_demo", &export);
    assert!(json.contains("\"primary_module_kind\":\"authentication_exchange\""));
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
fn summary_json_carries_ftp_auth_followup_missing_detail() {
    let binding = compile_file(&dsl_fixture_path("ftp_session_path.gewy"))
        .expect("ftp_session_path DSL should compile");
    let mut export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 8287, 53184, "ftp-client"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    8287,
                    7,
                    SessionId(1),
                ),
                tcp_state_fact_with_ports_for_tests(3, 8287, 1, 2, 53184, 21),
                packet_fact_with_dir_and_payload_for_tests(
                    4,
                    8287,
                    0x18,
                    PacketDir::Ingress,
                    Some(53184),
                    Some(21),
                    Some(0x32),
                    Some(0x3232),
                    Some(0x32323020),
                ),
                packet_fact_with_dir_and_payload_for_tests(
                    5,
                    8287,
                    0x18,
                    PacketDir::Egress,
                    Some(53184),
                    Some(21),
                    Some(0x55),
                    Some(0x5553),
                    Some(0x55534552),
                ),
                packet_fact_with_dir_and_payload_for_tests(
                    6,
                    8287,
                    0x18,
                    PacketDir::Ingress,
                    Some(53184),
                    Some(21),
                    Some(0x33),
                    Some(0x3333),
                    Some(0x33333120),
                ),
            ],
        ),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
    );
    let flow = export.program_flows[0].clone();
    push_synthetic_missing_stage_finding(
        &mut export,
        &flow,
        "ftp_session_path",
        "authentication_exchange",
        "send_auth_pass",
        "emit_payload",
        "receive_password_required->send_auth_pass",
        "receive_payload->emit_payload",
        "transport_io",
        "synthetic missing ftp auth pass",
        "tcp_packet_meta_fragment",
        "missing_signal:packet_observed",
    );
    let json = summary_json("dsl_demo", &export);
    assert!(json.contains("\"primary_module_kind\":\"authentication_exchange\""));
    assert!(
        json.contains("\"primary_failure_mode\":\"not_sent\""),
        "json={}",
        json
    );
    assert!(
        json.contains("\"primary_failure_detail\":\"followup_not_sent\""),
        "json={}",
        json
    );
    assert!(json.contains("\"primary_failure_confidence\":\"medium\""));
    assert!(json.contains("\"primary_failure_basis\":\"missing_transition\""));
}

#[test]
fn summary_json_carries_ftp_list_timeout_detail() {
    let binding = compile_file(&dsl_fixture_path("ftp_passive_list_path.gewy"))
        .expect("ftp_passive_list_path DSL should compile");
    let mut export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 8288, 53185, "ftp-client"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    8288,
                    7,
                    SessionId(1),
                ),
                tcp_state_fact_with_ports_for_tests(3, 8288, 1, 2, 53185, 21),
                packet_fact_with_dir_and_payload_for_tests(
                    4,
                    8288,
                    0x18,
                    PacketDir::Ingress,
                    Some(53185),
                    Some(21),
                    Some(0x32),
                    Some(0x3232),
                    Some(0x32323020),
                ),
                packet_fact_with_dir_and_payload_for_tests(
                    5,
                    8288,
                    0x18,
                    PacketDir::Egress,
                    Some(53185),
                    Some(21),
                    Some(0x55),
                    Some(0x5553),
                    Some(0x55534552),
                ),
                packet_fact_with_dir_and_payload_for_tests(
                    6,
                    8288,
                    0x18,
                    PacketDir::Ingress,
                    Some(53185),
                    Some(21),
                    Some(0x33),
                    Some(0x3333),
                    Some(0x33333120),
                ),
                packet_fact_with_dir_and_payload_for_tests(
                    7,
                    8288,
                    0x18,
                    PacketDir::Egress,
                    Some(53185),
                    Some(21),
                    Some(0x50),
                    Some(0x5041),
                    Some(0x50415353),
                ),
                packet_fact_with_dir_and_payload_for_tests(
                    8,
                    8288,
                    0x18,
                    PacketDir::Ingress,
                    Some(53185),
                    Some(21),
                    Some(0x32),
                    Some(0x3233),
                    Some(0x32333020),
                ),
                packet_fact_with_dir_and_payload_for_tests(
                    9,
                    8288,
                    0x18,
                    PacketDir::Egress,
                    Some(53185),
                    Some(21),
                    Some(0x50),
                    Some(0x5041),
                    Some(0x50415356),
                ),
                packet_fact_with_dir_and_payload_for_tests(
                    10,
                    8288,
                    0x18,
                    PacketDir::Ingress,
                    Some(53185),
                    Some(21),
                    Some(0x32),
                    Some(0x3232),
                    Some(0x32323720),
                ),
                packet_fact_with_dir_and_payload_for_tests(
                    11,
                    8288,
                    0x18,
                    PacketDir::Egress,
                    Some(53185),
                    Some(21),
                    Some(0x4c),
                    Some(0x4c49),
                    Some(0x4c495354),
                ),
            ],
        ),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
    );
    let flow = export.program_flows[0].clone();
    push_synthetic_missing_stage_finding(
        &mut export,
        &flow,
        "ftp_passive_list_path",
        "file_transfer_session",
        "receive_transfer_open",
        "receive_payload",
        "send_list->receive_transfer_open",
        "emit_payload->receive_payload",
        "transport_io",
        "synthetic missing ftp transfer open",
        "tcp_packet_meta_fragment",
        "missing_signal:packet_observed",
    );
    let json = summary_json("dsl_demo", &export);
    assert!(json.contains("\"primary_module_kind\":\"file_transfer_session\""));
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
fn summary_json_carries_ftp_active_port_timeout_detail() {
    let binding = compile_file(&dsl_fixture_path("ftp_active_list_path.gewy"))
        .expect("ftp_active_list_path DSL should compile");
    let mut export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 8301, 53042, "ftp-client"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    8301,
                    7,
                    SessionId(1),
                ),
                tcp_state_fact_with_ports_for_tests(3, 8301, 1, 2, 53042, 21),
                packet_fact_with_dir_and_payload_for_tests(
                    4,
                    8301,
                    0x18,
                    PacketDir::Ingress,
                    Some(53042),
                    Some(21),
                    Some(0x32),
                    Some(0x3232),
                    Some(0x32323020),
                ),
                packet_fact_with_dir_and_payload_for_tests(
                    5,
                    8301,
                    0x18,
                    PacketDir::Egress,
                    Some(53042),
                    Some(21),
                    Some(0x55),
                    Some(0x5553),
                    Some(0x55534552),
                ),
                packet_fact_with_dir_and_payload_for_tests(
                    6,
                    8301,
                    0x18,
                    PacketDir::Ingress,
                    Some(53042),
                    Some(21),
                    Some(0x33),
                    Some(0x3333),
                    Some(0x33333120),
                ),
                packet_fact_with_dir_and_payload_for_tests(
                    7,
                    8301,
                    0x18,
                    PacketDir::Egress,
                    Some(53042),
                    Some(21),
                    Some(0x50),
                    Some(0x5041),
                    Some(0x50415353),
                ),
                packet_fact_with_dir_and_payload_for_tests(
                    8,
                    8301,
                    0x18,
                    PacketDir::Ingress,
                    Some(53042),
                    Some(21),
                    Some(0x32),
                    Some(0x3233),
                    Some(0x32333020),
                ),
                packet_fact_with_dir_and_payload_for_tests(
                    9,
                    8301,
                    0x18,
                    PacketDir::Egress,
                    Some(53042),
                    Some(21),
                    Some(0x50),
                    Some(0x504f),
                    Some(0x504f5254),
                ),
            ],
        ),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
    );
    let flow = export.program_flows[0].clone();
    push_synthetic_missing_stage_finding(
        &mut export,
        &flow,
        "ftp_active_list_path",
        "file_transfer_session",
        "receive_port_ready",
        "receive_payload",
        "send_port->receive_port_ready",
        "emit_payload->receive_payload",
        "transport_io",
        "synthetic missing ftp port ready",
        "tcp_packet_meta_fragment",
        "missing_signal:packet_observed",
    );
    let json = summary_json("dsl_demo", &export);
    assert!(json.contains("\"primary_module_kind\":\"file_transfer_session\""));
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
fn summary_json_carries_ftp_denied_detail() {
    let binding = compile_file(&dsl_fixture_path("ftp_denied_path.gewy"))
        .expect("ftp_denied_path DSL should compile");
    let export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 8302, 53043, "ftp-client"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    8302,
                    7,
                    SessionId(1),
                ),
                tcp_state_fact_with_ports_for_tests(3, 8302, 1, 2, 53043, 21),
                packet_fact_with_dir_and_payload_for_tests(
                    4,
                    8302,
                    0x18,
                    PacketDir::Ingress,
                    Some(53043),
                    Some(21),
                    Some(0x32),
                    Some(0x3232),
                    Some(0x32323020),
                ),
                packet_fact_with_dir_and_payload_for_tests(
                    5,
                    8302,
                    0x18,
                    PacketDir::Egress,
                    Some(53043),
                    Some(21),
                    Some(0x55),
                    Some(0x5553),
                    Some(0x55534552),
                ),
                packet_fact_with_dir_and_payload_for_tests(
                    6,
                    8302,
                    0x18,
                    PacketDir::Ingress,
                    Some(53043),
                    Some(21),
                    Some(0x33),
                    Some(0x3333),
                    Some(0x33333120),
                ),
                packet_fact_with_dir_and_payload_for_tests(
                    7,
                    8302,
                    0x18,
                    PacketDir::Egress,
                    Some(53043),
                    Some(21),
                    Some(0x50),
                    Some(0x5041),
                    Some(0x50415353),
                ),
                packet_fact_with_dir_and_payload_for_tests(
                    8,
                    8302,
                    0x18,
                    PacketDir::Ingress,
                    Some(53043),
                    Some(21),
                    Some(0x35),
                    Some(0x3533),
                    Some(0x35333020),
                ),
            ],
        ),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
    );
    let json = summary_json("dsl_demo", &export);
    assert!(json.contains("\"primary_module_kind\":\"authentication_exchange\""));
    assert!(json.contains("\"primary_failure_mode\":\"server_denied\""));
    assert!(json.contains("\"primary_failure_detail\":\"access_denied\""));
    assert!(json.contains("\"primary_failure_confidence\":\"high\""));
    assert!(json.contains("\"primary_failure_basis\":\"direct_protocol_signal\""));
}
