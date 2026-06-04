use super::*;

#[test]
fn summary_json_carries_http3_request_timeout_detail() {
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http3_request_path.gewy")
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
    assert!(json.contains("\"primary_failure_mode\":\"no_response\""));
    assert!(json.contains("\"primary_failure_detail\":\"request_sent_no_reply\""));
}

#[test]
fn summary_json_carries_smtp_auth_timeout_detail() {
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/smtp_auth_path.gewy")
        .expect("smtp_auth_path DSL should compile");
    let mut export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 82911, 53013, "postfix-client"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    82911,
                    7,
                    SessionId(1),
                ),
                tcp_state_fact_with_ports_for_tests(3, 82911, 1, 2, 53013, 25),
                packet_fact_with_dir_and_payload_for_tests(
                    4,
                    82911,
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
                    82911,
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
                    82911,
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
                    82911,
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
    let json = summary_json("dsl_demo", &export);
    assert!(json.contains("\"primary_module_kind\":\"authentication_exchange\""));
    assert!(json.contains("\"primary_failure_mode\":\"no_response\""));
    assert!(json.contains("\"primary_failure_detail\":\"request_sent_no_reply\""));
    assert!(json.contains("\"primary_failure_confidence\":\"medium\""));
    assert!(json.contains("\"primary_failure_basis\":\"missing_transition\""));
}

#[test]
fn summary_json_carries_imap_auth_timeout_detail() {
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/imap_auth_path.gewy")
        .expect("imap_auth_path DSL should compile");
    let mut export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 82921, 53041, "imap-client"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    82921,
                    7,
                    SessionId(1),
                ),
                tcp_state_fact_with_ports_for_tests(3, 82921, 1, 2, 53041, 143),
                packet_fact_with_dir_and_payload_for_tests(
                    4,
                    82921,
                    0x18,
                    PacketDir::Ingress,
                    Some(53041),
                    Some(143),
                    Some(0x2a),
                    Some(0x2a20),
                    Some(0x2a204f4b),
                ),
                packet_fact_with_dir_and_payload_bytes_for_tests(
                    5,
                    82921,
                    0x18,
                    PacketDir::Egress,
                    Some(53041),
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
            ],
        ),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
    );
    let flow = export.program_flows[0].clone();
    push_synthetic_missing_stage_finding(
        &mut export,
        &flow,
        "imap_auth_path",
        "authentication_exchange",
        "receive_auth_ok",
        "receive_payload",
        "send_auth_request->receive_auth_ok",
        "emit_payload->receive_payload",
        "transport_io",
        "synthetic missing imap auth ok",
        "tcp_packet_meta_fragment",
        "missing_signal:packet_observed",
    );
    let json = summary_json("dsl_demo", &export);
    assert!(json.contains("\"primary_module_kind\":\"authentication_exchange\""));
    assert!(json.contains("\"primary_failure_mode\":\"no_response\""));
    assert!(json.contains("\"primary_failure_detail\":\"request_sent_no_reply\""));
}

#[test]
fn summary_json_carries_smtp_mail_timeout_detail() {
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/smtp_mail_path.gewy")
        .expect("smtp_mail_path DSL should compile");
    let mut export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 82912, 53016, "postfix-client"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    82912,
                    7,
                    SessionId(1),
                ),
                tcp_state_fact_with_ports_for_tests(3, 82912, 1, 2, 53016, 25),
                packet_fact_with_dir_and_payload_for_tests(
                    4,
                    82912,
                    0x18,
                    PacketDir::Ingress,
                    Some(53016),
                    Some(25),
                    Some(0x32),
                    Some(0x3232),
                    Some(0x32323020),
                ),
                packet_fact_with_dir_and_payload_for_tests(
                    5,
                    82912,
                    0x18,
                    PacketDir::Egress,
                    Some(53016),
                    Some(25),
                    Some(0x45),
                    Some(0x4548),
                    Some(0x45484c4f),
                ),
                packet_fact_with_dir_and_payload_for_tests(
                    6,
                    82912,
                    0x18,
                    PacketDir::Ingress,
                    Some(53016),
                    Some(25),
                    Some(0x32),
                    Some(0x3235),
                    Some(0x32353020),
                ),
                packet_fact_with_dir_and_payload_for_tests(
                    7,
                    82912,
                    0x18,
                    PacketDir::Egress,
                    Some(53016),
                    Some(25),
                    Some(0x41),
                    Some(0x4155),
                    Some(0x41555448),
                ),
                packet_fact_with_dir_and_payload_for_tests(
                    8,
                    82912,
                    0x18,
                    PacketDir::Ingress,
                    Some(53016),
                    Some(25),
                    Some(0x32),
                    Some(0x3233),
                    Some(0x32333520),
                ),
                packet_fact_with_dir_and_payload_for_tests(
                    9,
                    82912,
                    0x18,
                    PacketDir::Egress,
                    Some(53016),
                    Some(25),
                    Some(0x4d),
                    Some(0x4d41),
                    Some(0x4d41494c),
                ),
            ],
        ),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
    );
    let flow = export.program_flows[0].clone();
    push_synthetic_missing_stage_finding(
        &mut export,
        &flow,
        "smtp_mail_path",
        "mail_session",
        "receive_mail_ok",
        "receive_payload",
        "send_mail_from->receive_mail_ok",
        "emit_payload->receive_payload",
        "transport_io",
        "synthetic missing smtp mail ok",
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
