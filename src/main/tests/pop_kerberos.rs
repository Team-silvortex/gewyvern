use super::*;

const POP3_AUTH_TARGET_NAME: &str = "scan:pop3:auth";
const POP3_LIST_TARGET_NAME: &str = "scan:pop3:list";
const KERBEROS_AS_TARGET_NAME: &str = "scan:kerberos:as";
const RTSP_SETUP_TARGET_NAME: &str = "scan:rtsp:setup";

#[test]
fn summary_json_carries_pop3_auth_timeout_detail() {
    let binding = compile_file(&dsl_fixture_path("pop3_auth_path.gewy"))
        .expect("pop3_auth_path DSL should compile");
    let mut export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 82923, 53043, "pop3-client"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    82923,
                    7,
                    SessionId(1),
                ),
                tcp_state_fact_with_ports_for_tests(3, 82923, 1, 2, 53043, 110),
                packet_fact_with_dir_and_payload_bytes_for_tests(
                    4,
                    82923,
                    0x18,
                    PacketDir::Ingress,
                    Some(53043),
                    Some(110),
                    &[
                        (0, 0x2b),
                        (1, 0x4f),
                        (2, 0x4b),
                        (3, 0x20),
                        (5, 0x50),
                        (6, 0x4f),
                        (7, 0x50),
                        (8, 0x33),
                    ],
                ),
                packet_fact_with_dir_and_payload_bytes_for_tests(
                    5,
                    82923,
                    0x18,
                    PacketDir::Egress,
                    Some(53043),
                    Some(110),
                    &[(0, 0x55), (1, 0x53), (2, 0x45), (3, 0x52)],
                ),
                packet_fact_with_dir_and_payload_bytes_for_tests(
                    6,
                    82923,
                    0x18,
                    PacketDir::Ingress,
                    Some(53043),
                    Some(110),
                    &[
                        (0, 0x2b),
                        (1, 0x4f),
                        (2, 0x4b),
                        (3, 0x20),
                        (5, 0x55),
                        (6, 0x73),
                        (7, 0x65),
                        (8, 0x72),
                    ],
                ),
                packet_fact_with_dir_and_payload_bytes_for_tests(
                    7,
                    82923,
                    0x18,
                    PacketDir::Egress,
                    Some(53043),
                    Some(110),
                    &[(0, 0x50), (1, 0x41), (2, 0x53), (3, 0x53)],
                ),
            ],
        ),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
    );
    let flow = export.program_flows[0].clone();
    push_synthetic_missing_stage_finding(
        &mut export,
        &flow,
        "pop3_auth_path",
        "authentication_exchange",
        "receive_auth_ok",
        "receive_payload",
        "send_auth_pass->receive_auth_ok",
        "emit_payload->receive_payload",
        "transport_io",
        "synthetic missing pop3 auth ok",
        "tcp_packet_meta_fragment",
        "missing_signal:packet_observed",
    );
    let json = summary_json(POP3_AUTH_TARGET_NAME, &export);
    assert!(json.contains("\"primary_module_kind\":\"authentication_exchange\""));
    assert!(json.contains("\"primary_failure_mode\":\"no_response\""));
    assert!(json.contains("\"primary_failure_detail\":\"request_sent_no_reply\""));
    assert!(json.contains("\"primary_failure_confidence\":\"medium\""));
    assert!(json.contains("\"primary_failure_basis\":\"missing_transition\""));
}

#[test]
fn summary_json_carries_pop3_auth_denied_detail() {
    let binding = compile_file(&dsl_fixture_path("pop3_auth_denied_path.gewy"))
        .expect("pop3_auth_denied_path DSL should compile");
    let export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 82924, 53044, "pop3-client"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    82924,
                    7,
                    SessionId(1),
                ),
                tcp_state_fact_with_ports_for_tests(3, 82924, 1, 2, 53044, 110),
                packet_fact_with_dir_and_payload_bytes_for_tests(
                    4,
                    82924,
                    0x18,
                    PacketDir::Ingress,
                    Some(53044),
                    Some(110),
                    &[
                        (0, 0x2b),
                        (1, 0x4f),
                        (2, 0x4b),
                        (3, 0x20),
                        (5, 0x50),
                        (6, 0x4f),
                        (7, 0x50),
                        (8, 0x33),
                    ],
                ),
                packet_fact_with_dir_and_payload_bytes_for_tests(
                    5,
                    82924,
                    0x18,
                    PacketDir::Egress,
                    Some(53044),
                    Some(110),
                    &[(0, 0x55), (1, 0x53), (2, 0x45), (3, 0x52)],
                ),
                packet_fact_with_dir_and_payload_bytes_for_tests(
                    6,
                    82924,
                    0x18,
                    PacketDir::Ingress,
                    Some(53044),
                    Some(110),
                    &[
                        (0, 0x2b),
                        (1, 0x4f),
                        (2, 0x4b),
                        (3, 0x20),
                        (5, 0x55),
                        (6, 0x73),
                        (7, 0x65),
                        (8, 0x72),
                    ],
                ),
                packet_fact_with_dir_and_payload_bytes_for_tests(
                    7,
                    82924,
                    0x18,
                    PacketDir::Egress,
                    Some(53044),
                    Some(110),
                    &[(0, 0x50), (1, 0x41), (2, 0x53), (3, 0x53)],
                ),
                packet_fact_with_dir_and_payload_bytes_for_tests(
                    8,
                    82924,
                    0x18,
                    PacketDir::Ingress,
                    Some(53044),
                    Some(110),
                    &[
                        (0, 0x2d),
                        (1, 0x45),
                        (2, 0x52),
                        (3, 0x52),
                        (5, 0x61),
                        (6, 0x75),
                        (7, 0x74),
                        (8, 0x68),
                    ],
                ),
            ],
        ),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
    );
    let json = summary_json(POP3_AUTH_TARGET_NAME, &export);
    assert!(json.contains("\"primary_module_kind\":\"authentication_exchange\""));
    assert!(json.contains("\"primary_failure_mode\":\"server_denied\""));
    assert!(json.contains("\"primary_failure_detail\":\"access_denied\""));
    assert!(json.contains("\"primary_failure_confidence\":\"high\""));
    assert!(json.contains("\"primary_failure_basis\":\"direct_protocol_signal\""));
}

#[test]
fn summary_json_carries_pop3_list_timeout_detail() {
    let binding = compile_file(&dsl_fixture_path("pop3_list_path.gewy"))
        .expect("pop3_list_path DSL should compile");
    let mut export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 82929, 53051, "pop3-client"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    82929,
                    7,
                    SessionId(1),
                ),
                tcp_state_fact_with_ports_for_tests(3, 82929, 1, 2, 53051, 110),
                packet_fact_with_dir_and_payload_bytes_for_tests(
                    4,
                    82929,
                    0x18,
                    PacketDir::Ingress,
                    Some(53051),
                    Some(110),
                    &[
                        (0, 0x2b),
                        (1, 0x4f),
                        (2, 0x4b),
                        (3, 0x20),
                        (5, 0x50),
                        (6, 0x4f),
                        (7, 0x50),
                        (8, 0x33),
                    ],
                ),
                packet_fact_with_dir_and_payload_bytes_for_tests(
                    5,
                    82929,
                    0x18,
                    PacketDir::Egress,
                    Some(53051),
                    Some(110),
                    &[(0, 0x55), (1, 0x53), (2, 0x45), (3, 0x52)],
                ),
                packet_fact_with_dir_and_payload_bytes_for_tests(
                    6,
                    82929,
                    0x18,
                    PacketDir::Ingress,
                    Some(53051),
                    Some(110),
                    &[
                        (0, 0x2b),
                        (1, 0x4f),
                        (2, 0x4b),
                        (3, 0x20),
                        (5, 0x55),
                        (6, 0x73),
                        (7, 0x65),
                        (8, 0x72),
                    ],
                ),
                packet_fact_with_dir_and_payload_bytes_for_tests(
                    7,
                    82929,
                    0x18,
                    PacketDir::Egress,
                    Some(53051),
                    Some(110),
                    &[(0, 0x50), (1, 0x41), (2, 0x53), (3, 0x53)],
                ),
                packet_fact_with_dir_and_payload_bytes_for_tests(
                    8,
                    82929,
                    0x18,
                    PacketDir::Ingress,
                    Some(53051),
                    Some(110),
                    &[
                        (0, 0x2b),
                        (1, 0x4f),
                        (2, 0x4b),
                        (3, 0x20),
                        (5, 0x4d),
                        (6, 0x61),
                        (7, 0x69),
                        (8, 0x6c),
                    ],
                ),
                packet_fact_with_dir_and_payload_bytes_for_tests(
                    9,
                    82929,
                    0x18,
                    PacketDir::Egress,
                    Some(53051),
                    Some(110),
                    &[(0, 0x4c), (1, 0x49), (2, 0x53), (3, 0x54)],
                ),
            ],
        ),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
    );
    let flow = export.program_flows[0].clone();
    push_synthetic_missing_stage_finding(
        &mut export,
        &flow,
        "pop3_list_path",
        "mail_session",
        "receive_list_ready",
        "receive_payload",
        "send_list->receive_list_ready",
        "emit_payload->receive_payload",
        "transport_io",
        "synthetic missing pop3 list ready",
        "tcp_packet_meta_fragment",
        "missing_signal:packet_observed",
    );
    let json = summary_json(POP3_LIST_TARGET_NAME, &export);
    assert!(json.contains("\"primary_module_kind\":\"mail_session\""));
    assert!(json.contains("\"primary_failure_mode\":\"no_response\""));
    assert!(json.contains("\"primary_failure_detail\":\"request_sent_no_reply\""));
    assert!(json.contains("\"primary_failure_confidence\":\"medium\""));
    assert!(json.contains("\"primary_failure_basis\":\"missing_transition\""));
}

#[test]
fn summary_json_carries_kerberos_as_timeout_detail() {
    let binding = compile_file(&dsl_fixture_path("kerberos_as_path.gewy"))
        .expect("kerberos_as_path DSL should compile");
    let mut export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 82925, 53045, "kinit"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    82925,
                    7,
                    SessionId(1),
                ),
                udp_packet_fact_with_dir_and_ports_and_payload_for_tests(
                    3,
                    82925,
                    120,
                    PacketDir::Egress,
                    Some(53045),
                    Some(88),
                    Some(0x6a),
                    None,
                ),
            ],
        ),
        &Cli::from_args(["--demo".to_string(), "udp".to_string()]).unwrap(),
    );
    let flow = export.program_flows[0].clone();
    push_synthetic_missing_stage_finding(
        &mut export,
        &flow,
        "kerberos_as_path",
        "authentication_exchange",
        "receive_as_reply",
        "receive_datagram",
        "send_as_request->receive_as_reply",
        "emit_datagram->receive_datagram",
        "transport_io",
        "synthetic missing kerberos as reply",
        "udp_packet_meta_fragment",
        "missing_signal:datagram_observed",
    );
    let json = summary_json(KERBEROS_AS_TARGET_NAME, &export);
    assert!(json.contains("\"primary_module_kind\":\"authentication_exchange\""));
    assert!(json.contains("\"primary_failure_mode\":\"no_response\""));
    assert!(json.contains("\"primary_failure_detail\":\"request_sent_no_reply\""));
}

#[test]
fn summary_json_carries_kerberos_as_error_detail() {
    let binding = compile_file(&dsl_fixture_path("kerberos_as_error_path.gewy"))
        .expect("kerberos_as_error_path DSL should compile");
    let export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 82926, 53046, "kinit"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    82926,
                    7,
                    SessionId(1),
                ),
                udp_packet_fact_with_dir_and_ports_and_payload_for_tests(
                    3,
                    82926,
                    120,
                    PacketDir::Egress,
                    Some(53046),
                    Some(88),
                    Some(0x6a),
                    None,
                ),
                udp_packet_fact_with_dir_and_ports_and_payload_for_tests(
                    4,
                    82926,
                    100,
                    PacketDir::Ingress,
                    Some(53046),
                    Some(88),
                    Some(0x7e),
                    None,
                ),
            ],
        ),
        &Cli::from_args(["--demo".to_string(), "udp".to_string()]).unwrap(),
    );
    let json = summary_json(KERBEROS_AS_TARGET_NAME, &export);
    assert!(json.contains("\"primary_module_kind\":\"authentication_exchange\""));
    assert!(json.contains("\"primary_failure_mode\":\"semantic_error\""));
    assert!(json.contains("\"primary_failure_detail\":\"protocol_error\""));
    assert!(json.contains("\"primary_failure_confidence\":\"high\""));
    assert!(json.contains("\"primary_failure_basis\":\"direct_protocol_signal\""));
}

#[test]
fn summary_json_carries_rtsp_setup_timeout_detail() {
    let binding = compile_file(&dsl_fixture_path("rtsp_setup_path.gewy"))
        .expect("rtsp_setup_path DSL should compile");
    let mut export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 82931, 53053, "vlc"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    82931,
                    7,
                    SessionId(1),
                ),
                tcp_state_fact_with_ports_for_tests(3, 82931, 1, 2, 53053, 554),
                tcp_state_fact_with_ports_for_tests(4, 82931, 2, 3, 53053, 554),
                packet_fact_with_dir_and_payload_bytes_for_tests(
                    5,
                    82931,
                    0x18,
                    PacketDir::Egress,
                    Some(53053),
                    Some(554),
                    &[(0, 0x4f), (1, 0x50), (2, 0x54), (3, 0x49)],
                ),
                packet_fact_with_dir_and_payload_bytes_for_tests(
                    6,
                    82931,
                    0x18,
                    PacketDir::Ingress,
                    Some(53053),
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
                    82931,
                    0x18,
                    PacketDir::Egress,
                    Some(53053),
                    Some(554),
                    &[(0, 0x53), (1, 0x45), (2, 0x54), (3, 0x55)],
                ),
            ],
        ),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
    );
    let flow = export.program_flows[0].clone();
    push_synthetic_missing_stage_finding(
        &mut export,
        &flow,
        "rtsp_setup_path",
        "signaling_session",
        "receive_setup_ok",
        "receive_payload",
        "send_setup->receive_setup_ok",
        "emit_payload->receive_payload",
        "transport_io",
        "synthetic missing rtsp setup ok",
        "tcp_packet_meta_fragment",
        "missing_signal:packet_observed",
    );
    let json = summary_json(RTSP_SETUP_TARGET_NAME, &export);
    assert!(json.contains("\"primary_module_kind\":\"signaling_session\""));
    assert!(json.contains("\"primary_failure_mode\":\"not_sent\""));
    assert!(json.contains("\"primary_failure_detail\":\"followup_not_sent\""));
    assert!(json.contains("\"primary_failure_confidence\":\"low\""));
    assert!(json.contains("\"primary_failure_basis\":\"missing_transition\""));
}
