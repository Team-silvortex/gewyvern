use super::*;

#[test]
fn summary_json_carries_smtp_data_denied_detail() {
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/smtp_data_denied_path.gewy")
        .expect("smtp_data_denied_path DSL should compile");
    let export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 82915, 53020, "postfix-client"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    82915,
                    7,
                    SessionId(1),
                ),
                tcp_state_fact_with_ports_for_tests(3, 82915, 1, 2, 53020, 25),
                packet_fact_with_dir_and_payload_for_tests(
                    4,
                    82915,
                    0x18,
                    PacketDir::Ingress,
                    Some(53020),
                    Some(25),
                    Some(0x32),
                    Some(0x3232),
                    Some(0x32323020),
                ),
                packet_fact_with_dir_and_payload_for_tests(
                    5,
                    82915,
                    0x18,
                    PacketDir::Egress,
                    Some(53020),
                    Some(25),
                    Some(0x45),
                    Some(0x4548),
                    Some(0x45484c4f),
                ),
                packet_fact_with_dir_and_payload_for_tests(
                    6,
                    82915,
                    0x18,
                    PacketDir::Ingress,
                    Some(53020),
                    Some(25),
                    Some(0x32),
                    Some(0x3235),
                    Some(0x32353020),
                ),
                packet_fact_with_dir_and_payload_for_tests(
                    7,
                    82915,
                    0x18,
                    PacketDir::Egress,
                    Some(53020),
                    Some(25),
                    Some(0x41),
                    Some(0x4155),
                    Some(0x41555448),
                ),
                packet_fact_with_dir_and_payload_for_tests(
                    8,
                    82915,
                    0x18,
                    PacketDir::Ingress,
                    Some(53020),
                    Some(25),
                    Some(0x32),
                    Some(0x3233),
                    Some(0x32333520),
                ),
                packet_fact_with_dir_and_payload_for_tests(
                    9,
                    82915,
                    0x18,
                    PacketDir::Egress,
                    Some(53020),
                    Some(25),
                    Some(0x4d),
                    Some(0x4d41),
                    Some(0x4d41494c),
                ),
                packet_fact_with_dir_and_payload_bytes_for_tests(
                    10,
                    82915,
                    0x18,
                    PacketDir::Ingress,
                    Some(53020),
                    Some(25),
                    &[
                        (0, 0x32),
                        (1, 0x35),
                        (2, 0x30),
                        (3, 0x20),
                        (4, 0x32),
                        (5, 0x2e),
                        (6, 0x31),
                        (7, 0x2e),
                    ],
                ),
                packet_fact_with_dir_and_payload_for_tests(
                    11,
                    82915,
                    0x18,
                    PacketDir::Egress,
                    Some(53020),
                    Some(25),
                    Some(0x52),
                    Some(0x5243),
                    Some(0x52435054),
                ),
                packet_fact_with_dir_and_payload_bytes_for_tests(
                    12,
                    82915,
                    0x18,
                    PacketDir::Ingress,
                    Some(53020),
                    Some(25),
                    &[
                        (0, 0x32),
                        (1, 0x35),
                        (2, 0x30),
                        (3, 0x20),
                        (4, 0x32),
                        (5, 0x2e),
                        (6, 0x31),
                        (7, 0x2e),
                        (8, 0x35),
                    ],
                ),
                packet_fact_with_dir_and_payload_for_tests(
                    13,
                    82915,
                    0x18,
                    PacketDir::Egress,
                    Some(53020),
                    Some(25),
                    Some(0x44),
                    Some(0x4441),
                    Some(0x44415441),
                ),
                packet_fact_with_dir_and_payload_for_tests(
                    14,
                    82915,
                    0x18,
                    PacketDir::Ingress,
                    Some(53020),
                    Some(25),
                    Some(0x33),
                    Some(0x3335),
                    Some(0x33353420),
                ),
                packet_fact_with_dir_and_payload_bytes_for_tests(
                    15,
                    82915,
                    0x18,
                    PacketDir::Egress,
                    Some(53020),
                    Some(25),
                    &[(0, 0x0d), (1, 0x0a), (2, 0x2e), (3, 0x0d), (4, 0x0a)],
                ),
                packet_fact_with_dir_and_payload_for_tests(
                    16,
                    82915,
                    0x18,
                    PacketDir::Ingress,
                    Some(53020),
                    Some(25),
                    Some(0x35),
                    Some(0x3535),
                    Some(0x35353020),
                ),
            ],
        ),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
    );
    let json = summary_json("dsl_demo", &export);
    assert!(
        json.contains("\"primary_module_kind\":\"mail_session\""),
        "json={}",
        json
    );
    assert!(
        json.contains("\"primary_failure_mode\":\"server_denied\""),
        "json={}",
        json
    );
    assert!(
        json.contains("\"primary_failure_detail\":\"access_denied\""),
        "json={}",
        json
    );
    assert!(
        json.contains("\"primary_failure_confidence\":\"high\""),
        "json={}",
        json
    );
    assert!(
        json.contains("\"primary_failure_basis\":\"direct_protocol_signal\""),
        "json={}",
        json
    );
}

#[test]
fn summary_json_carries_hy2_auth_timeout_detail() {
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/hy2_auth_path.gewy")
        .expect("hy2_auth_path DSL should compile");
    let mut export = annotate_export_trust(
        run_binding_demo(binding),
        &Cli::from_args(["--demo".to_string(), "udp".to_string()]).unwrap(),
    );
    let flow = export.program_flows[0].clone();
    push_synthetic_missing_stage_finding(
        &mut export,
        &flow,
        "hy2_auth_path",
        "proxy_authentication",
        "receive_auth_ok_stream",
        "receive_payload",
        "send_auth_request_stream->receive_auth_ok_stream",
        "emit_payload->receive_payload",
        "transport_io",
        "synthetic missing hy2 auth ok",
        "quic_frame_meta_fragment",
        "missing_signal:quic_frame_observed",
    );
    let json = summary_json("dsl_demo", &export);
    assert!(json.contains("\"primary_module_kind\":\"proxy_authentication\""));
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
}

#[test]
fn summary_json_carries_hy2_tcp_relay_timeout_detail() {
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/hy2_tcp_relay_path.gewy")
        .expect("hy2_tcp_relay_path DSL should compile");
    let mut export = annotate_export_trust(
        run_binding_demo(binding),
        &Cli::from_args(["--demo".to_string(), "udp".to_string()]).unwrap(),
    );
    let flow = export.program_flows[0].clone();
    push_synthetic_missing_stage_finding(
        &mut export,
        &flow,
        "hy2_tcp_relay_path",
        "proxy_tcp_relay",
        "receive_tcp_response_stream",
        "receive_payload",
        "send_tcp_request_stream->receive_tcp_response_stream",
        "emit_payload->receive_payload",
        "transport_io",
        "synthetic missing hy2 tcp response",
        "quic_frame_meta_fragment",
        "missing_signal:quic_frame_observed",
    );
    let json = summary_json("dsl_demo", &export);
    assert!(json.contains("\"primary_module_kind\":\"proxy_tcp_relay\""));
    assert!(json.contains("\"primary_failure_mode\":\"no_response\""));
    assert!(json.contains("\"primary_failure_detail\":\"request_sent_no_reply\""));
    assert!(json.contains("\"primary_failure_confidence\":\"medium\""));
    assert!(json.contains("\"primary_failure_basis\":\"missing_transition\""));
}

#[test]
fn summary_json_carries_socks5_timeout_detail() {
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/socks5_session_path.gewy")
        .expect("socks5_session_path DSL should compile");
    let mut export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 8283, 53180, "proxy-client"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    8283,
                    7,
                    SessionId(1),
                ),
                tcp_state_fact_with_ports_for_tests(3, 8283, 1, 2, 53180, 1080),
                packet_fact_with_dir_and_payload_for_tests(
                    4,
                    8283,
                    0x18,
                    PacketDir::Egress,
                    Some(53180),
                    Some(1080),
                    Some(0x05),
                    Some(0x0501),
                    None,
                ),
                packet_fact_with_dir_and_payload_for_tests(
                    5,
                    8283,
                    0x18,
                    PacketDir::Ingress,
                    Some(53180),
                    Some(1080),
                    Some(0x05),
                    Some(0x0500),
                    None,
                ),
                packet_fact_with_dir_and_payload_bytes_for_tests(
                    6,
                    8283,
                    0x18,
                    PacketDir::Egress,
                    Some(53180),
                    Some(1080),
                    &[(0, 0x05), (1, 0x01), (2, 0x00), (3, 0x03)],
                ),
            ],
        ),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
    );
    let flow = export.program_flows[0].clone();
    push_synthetic_missing_stage_finding(
        &mut export,
        &flow,
        "socks5_session_path",
        "proxy_negotiation",
        "receive_connect_success",
        "receive_payload",
        "send_connect_request->receive_connect_success",
        "emit_payload->receive_payload",
        "transport_io",
        "synthetic missing socks5 connect success",
        "tcp_packet_meta_fragment",
        "missing_signal:packet_observed",
    );
    let json = summary_json("dsl_demo", &export);
    assert!(json.contains("\"primary_module_kind\":\"proxy_negotiation\""));
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
}

#[test]
fn summary_json_carries_socks5_auth_connect_denied_detail() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/socks5_auth_connect_denied_path.gewy")
            .expect("socks5_auth_connect_denied_path DSL should compile");
    let export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 82831, 53186, "proxy-client"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    82831,
                    7,
                    SessionId(1),
                ),
                tcp_state_fact_with_ports_for_tests(3, 82831, 1, 2, 53186, 1080),
                packet_fact_with_dir_and_payload_bytes_for_tests(
                    4,
                    82831,
                    0x18,
                    PacketDir::Egress,
                    Some(53186),
                    Some(1080),
                    &[(0, 0x05), (1, 0x01), (2, 0x02)],
                ),
                packet_fact_with_dir_and_payload_bytes_for_tests(
                    5,
                    82831,
                    0x18,
                    PacketDir::Ingress,
                    Some(53186),
                    Some(1080),
                    &[(0, 0x05), (1, 0x02)],
                ),
                packet_fact_with_dir_and_payload_bytes_for_tests(
                    6,
                    82831,
                    0x18,
                    PacketDir::Egress,
                    Some(53186),
                    Some(1080),
                    &[(0, 0x01), (1, 0x01)],
                ),
                packet_fact_with_dir_and_payload_bytes_for_tests(
                    7,
                    82831,
                    0x18,
                    PacketDir::Ingress,
                    Some(53186),
                    Some(1080),
                    &[(0, 0x01), (1, 0x00)],
                ),
                packet_fact_with_dir_and_payload_bytes_for_tests(
                    8,
                    82831,
                    0x18,
                    PacketDir::Egress,
                    Some(53186),
                    Some(1080),
                    &[(0, 0x05), (1, 0x01), (2, 0x00), (3, 0x03)],
                ),
                packet_fact_with_dir_and_payload_bytes_for_tests(
                    9,
                    82831,
                    0x18,
                    PacketDir::Ingress,
                    Some(53186),
                    Some(1080),
                    &[(0, 0x05), (1, 0x05), (2, 0x00), (3, 0x01)],
                ),
            ],
        ),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
    );
    let json = summary_json("dsl_demo", &export);
    assert!(json.contains("\"primary_module_kind\":\"proxy_negotiation\""));
    assert!(json.contains("\"primary_failure_mode\":\"server_denied\""));
    assert!(json.contains("\"primary_failure_detail\":\"access_denied\""));
    assert!(json.contains("\"primary_failure_confidence\":\"high\""));
    assert!(json.contains("\"primary_failure_basis\":\"direct_protocol_signal\""));
}

#[test]
fn summary_json_carries_imap_auth_denied_detail() {
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/imap_auth_denied_path.gewy")
        .expect("imap_auth_denied_path DSL should compile");
    let export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 82922, 53042, "imap-client"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    82922,
                    7,
                    SessionId(1),
                ),
                tcp_state_fact_with_ports_for_tests(3, 82922, 1, 2, 53042, 143),
                packet_fact_with_dir_and_payload_for_tests(
                    4,
                    82922,
                    0x18,
                    PacketDir::Ingress,
                    Some(53042),
                    Some(143),
                    Some(0x2a),
                    Some(0x2a20),
                    Some(0x2a204f4b),
                ),
                packet_fact_with_dir_and_payload_bytes_for_tests(
                    5,
                    82922,
                    0x18,
                    PacketDir::Egress,
                    Some(53042),
                    Some(143),
                    &[
                        (0, 0x41),
                        (1, 0x30),
                        (2, 0x30),
                        (3, 0x31),
                        (5, 0x4c),
                        (6, 0x4f),
                        (7, 0x47),
                        (8, 0x49),
                        (9, 0x4e),
                    ],
                ),
                packet_fact_with_dir_and_payload_bytes_for_tests(
                    6,
                    82922,
                    0x18,
                    PacketDir::Ingress,
                    Some(53042),
                    Some(143),
                    &[
                        (0, 0x41),
                        (1, 0x30),
                        (2, 0x30),
                        (3, 0x31),
                        (5, 0x4e),
                        (6, 0x4f),
                    ],
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

#[test]
fn summary_json_carries_imap_select_timeout_detail() {
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/imap_select_path.gewy")
        .expect("imap_select_path DSL should compile");
    let mut export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 82928, 53050, "imap-client"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    82928,
                    7,
                    SessionId(1),
                ),
                tcp_state_fact_with_ports_for_tests(3, 82928, 1, 2, 53050, 143),
                packet_fact_with_dir_and_payload_for_tests(
                    4,
                    82928,
                    0x18,
                    PacketDir::Ingress,
                    Some(53050),
                    Some(143),
                    Some(0x2a),
                    Some(0x2a20),
                    Some(0x2a204f4b),
                ),
                packet_fact_with_dir_and_payload_bytes_for_tests(
                    5,
                    82928,
                    0x18,
                    PacketDir::Egress,
                    Some(53050),
                    Some(143),
                    &[
                        (0, 0x41),
                        (1, 0x30),
                        (2, 0x30),
                        (3, 0x31),
                        (5, 0x4c),
                        (6, 0x4f),
                        (7, 0x47),
                        (8, 0x49),
                        (9, 0x4e),
                    ],
                ),
                packet_fact_with_dir_and_payload_bytes_for_tests(
                    6,
                    82928,
                    0x18,
                    PacketDir::Ingress,
                    Some(53050),
                    Some(143),
                    &[
                        (0, 0x41),
                        (1, 0x30),
                        (2, 0x30),
                        (3, 0x31),
                        (5, 0x4f),
                        (6, 0x4b),
                    ],
                ),
                packet_fact_with_dir_and_payload_bytes_for_tests(
                    7,
                    82928,
                    0x18,
                    PacketDir::Egress,
                    Some(53050),
                    Some(143),
                    &[
                        (0, 0x41),
                        (1, 0x30),
                        (2, 0x30),
                        (3, 0x32),
                        (5, 0x53),
                        (6, 0x45),
                        (7, 0x4c),
                        (8, 0x45),
                        (9, 0x43),
                        (10, 0x54),
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
        "imap_select_path",
        "mail_session",
        "receive_mailbox_selected",
        "receive_payload",
        "send_select->receive_mailbox_selected",
        "emit_payload->receive_payload",
        "transport_io",
        "synthetic missing imap select ok",
        "tcp_packet_meta_fragment",
        "missing_signal:packet_observed",
    );
    let json = summary_json("dsl_demo", &export);
    assert!(json.contains("\"primary_module_kind\":\"mail_session\""));
    assert!(json.contains("\"primary_failure_mode\":\"no_response\""));
    assert!(json.contains("\"primary_failure_detail\":\"request_sent_no_reply\""));
    assert!(json.contains("\"primary_failure_confidence\":\"medium\""));
    assert!(json.contains("\"primary_failure_basis\":\"missing_transition\""));
}
