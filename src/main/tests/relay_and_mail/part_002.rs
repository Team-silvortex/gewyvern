use super::*;

#[test]
fn summary_json_carries_imap_select_timeout_detail() {
    let binding = compile_file(&dsl_fixture_path("imap_select_path.gewy"))
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
