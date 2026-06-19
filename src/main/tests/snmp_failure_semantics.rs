use super::*;

fn snmp_udp_packet_fact(
    id: u64,
    cookie: u64,
    dir: PacketDir,
    local_port: u16,
    remote_port: u16,
    payload_bytes: &[(u16, u8)],
) -> FactEnvelope {
    let byte_at = |target: u16| {
        payload_bytes
            .iter()
            .find_map(|(offset, value)| (*offset == target).then_some(*value))
    };
    let payload_byte0 = byte_at(0);
    let payload_byte1 = byte_at(1);
    let payload_byte2 = byte_at(2);
    let payload_byte3 = byte_at(3);
    FactEnvelope {
        id: FactId(id),
        ts: SystemTime::UNIX_EPOCH + Duration::from_millis(id * 10),
        cpu: CpuId(0),
        ifindex: Some(2),
        session: SessionId(1),
        fragment_id: "udp_packet_meta_fragment".into(),
        kind: FactKind::PacketMeta(PacketMetaFact {
            netns: 1,
            sk_cookie: Some(cookie),
            dir,
            local_port: Some(local_port),
            remote_port: Some(remote_port),
            payload_byte0,
            payload_byte1,
            payload_prefix2: payload_byte0
                .zip(payload_byte1)
                .map(|(b0, b1)| u16::from_be_bytes([b0, b1])),
            payload_prefix4: payload_byte0
                .zip(payload_byte1)
                .zip(payload_byte2)
                .zip(payload_byte3)
                .map(|(((b0, b1), b2), b3)| u32::from_be_bytes([b0, b1, b2, b3])),
            payload_byte4: byte_at(4),
            payload_byte5: byte_at(5),
            payload_byte9: byte_at(9),
            payload_byte10: byte_at(10),
            payload_byte13: byte_at(13),
            payload_bytes: payload_bytes.iter().copied().collect(),
            l3_proto: 0x0800,
            l4_proto: 17,
            tot_len: 96,
            tcp_flags: 0,
            seq: None,
            ack: None,
            window: None,
        }),
    }
}

#[test]
fn summary_json_carries_snmp_get_timeout_detail() {
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/snmp_get_path.gewy")
        .expect("snmp_get_path DSL should compile");
    let mut export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 91001, 43001, "snmpget"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    91001,
                    7,
                    SessionId(1),
                ),
                snmp_udp_packet_fact(
                    3,
                    91001,
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
    let json = summary_json("dsl_demo", &export);
    assert!(
        json.contains("\"primary_module_kind\":\"management_query\""),
        "json={}",
        json
    );
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
    assert!(
        json.contains("\"primary_failure_basis\":\"missing_transition\""),
        "json={}",
        json
    );
    assert!(
        json.contains("\"missing_transitions\":[\"send_get_request->receive_get_response\"]"),
        "json={}",
        json
    );
}

#[test]
fn summary_json_carries_snmp_bulk_timeout_detail() {
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/snmp_bulk_path.gewy")
        .expect("snmp_bulk_path DSL should compile");
    let mut export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 91002, 43002, "snmpbulkwalk"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    91002,
                    7,
                    SessionId(1),
                ),
                snmp_udp_packet_fact(
                    3,
                    91002,
                    PacketDir::Egress,
                    49002,
                    161,
                    &[(0, 0x30), (1, 0x2d), (4, 0x02), (13, 0xa5)],
                ),
            ],
        ),
        &Cli::from_args(["--demo".to_string(), "udp".to_string()]).unwrap(),
    );
    let flow = export.program_flows[0].clone();
    push_synthetic_missing_stage_finding(
        &mut export,
        &flow,
        "snmp_bulk_path",
        "snmp_bulk",
        "receive_bulk_response",
        "receive_payload",
        "send_bulk_request->receive_bulk_response",
        "emit_payload->receive_payload",
        "transport_io",
        "synthetic missing snmp bulk response",
        "udp_packet_meta_fragment",
        "missing_signal:packet_observed",
    );
    let json = summary_json("dsl_demo", &export);
    assert!(
        json.contains("\"primary_module_kind\":\"management_query\""),
        "json={}",
        json
    );
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
    assert!(
        json.contains("\"primary_failure_basis\":\"missing_transition\""),
        "json={}",
        json
    );
    assert!(
        json.contains("\"missing_transitions\":[\"send_bulk_request->receive_bulk_response\"]"),
        "json={}",
        json
    );
}

#[test]
fn summary_json_carries_snmp_unauthorized_denied_detail() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/snmp_unauthorized_path.gewy")
            .expect("snmp_unauthorized_path DSL should compile");
    let export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 91003, 43003, "snmpd"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    91003,
                    7,
                    SessionId(1),
                ),
                snmp_udp_packet_fact(
                    3,
                    91003,
                    PacketDir::Ingress,
                    49003,
                    161,
                    &[(0, 0x30), (1, 0x2f), (4, 0x03), (13, 0xa8), (18, 0x05)],
                ),
            ],
        ),
        &Cli::from_args(["--demo".to_string(), "udp".to_string()]).unwrap(),
    );
    let json = summary_json("dsl_demo", &export);
    assert!(
        json.contains("\"primary_module_kind\":\"management_query\""),
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
        json.contains("\"primary_failure_basis\":\"direct_protocol_signal\""),
        "json={}",
        json
    );
}

#[test]
fn summary_json_carries_snmp_set_timeout_detail() {
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/snmp_set_path.gewy")
        .expect("snmp_set_path DSL should compile");
    let mut export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 91004, 43004, "snmpset"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    91004,
                    7,
                    SessionId(1),
                ),
                snmp_udp_packet_fact(
                    3,
                    91004,
                    PacketDir::Egress,
                    49004,
                    161,
                    &[(0, 0x30), (1, 0x31), (4, 0x02), (13, 0xa3)],
                ),
            ],
        ),
        &Cli::from_args(["--demo".to_string(), "udp".to_string()]).unwrap(),
    );
    let flow = export.program_flows[0].clone();
    push_synthetic_missing_stage_finding(
        &mut export,
        &flow,
        "snmp_set_path",
        "snmp_set",
        "receive_set_response",
        "receive_payload",
        "send_set_request->receive_set_response",
        "emit_payload->receive_payload",
        "transport_io",
        "synthetic missing snmp set response",
        "udp_packet_meta_fragment",
        "missing_signal:packet_observed",
    );
    let json = summary_json("dsl_demo", &export);
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
    assert!(
        json.contains("\"missing_transitions\":[\"send_set_request->receive_set_response\"]"),
        "json={}",
        json
    );
}

#[test]
fn summary_json_carries_snmp_inform_timeout_detail() {
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/snmp_inform_path.gewy")
        .expect("snmp_inform_path DSL should compile");
    let mut export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 91005, 43005, "snmpinform"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    91005,
                    7,
                    SessionId(1),
                ),
                snmp_udp_packet_fact(
                    3,
                    91005,
                    PacketDir::Egress,
                    49005,
                    161,
                    &[(0, 0x30), (1, 0x33), (4, 0x02), (13, 0xa6)],
                ),
            ],
        ),
        &Cli::from_args(["--demo".to_string(), "udp".to_string()]).unwrap(),
    );
    let flow = export.program_flows[0].clone();
    push_synthetic_missing_stage_finding(
        &mut export,
        &flow,
        "snmp_inform_path",
        "snmp_inform",
        "receive_inform_response",
        "receive_payload",
        "send_inform_notification->receive_inform_response",
        "emit_payload->receive_payload",
        "transport_io",
        "synthetic missing snmp inform response",
        "udp_packet_meta_fragment",
        "missing_signal:packet_observed",
    );
    let json = summary_json("dsl_demo", &export);
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
    assert!(
        json.contains(
            "\"missing_transitions\":[\"send_inform_notification->receive_inform_response\"]"
        ),
        "json={}",
        json
    );
}

#[test]
fn summary_json_carries_snmp_engine_sync_timeout_detail() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/snmp_engine_sync_path.gewy")
            .expect("snmp_engine_sync_path DSL should compile");
    let mut export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 91006, 43006, "snmpv3-sync"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    91006,
                    7,
                    SessionId(1),
                ),
                snmp_udp_packet_fact(
                    3,
                    91006,
                    PacketDir::Egress,
                    49006,
                    161,
                    &[(0, 0x30), (1, 0x39), (4, 0x03), (13, 0xa0), (18, 0x04)],
                ),
            ],
        ),
        &Cli::from_args(["--demo".to_string(), "udp".to_string()]).unwrap(),
    );
    let flow = export.program_flows[0].clone();
    push_synthetic_missing_stage_finding(
        &mut export,
        &flow,
        "snmp_engine_sync_path",
        "snmp_engine_sync",
        "receive_engine_sync_report",
        "receive_payload",
        "send_engine_sync_probe->receive_engine_sync_report",
        "emit_payload->receive_payload",
        "transport_io",
        "synthetic missing snmp engine sync report",
        "udp_packet_meta_fragment",
        "missing_signal:packet_observed",
    );
    let json = summary_json("dsl_demo", &export);
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
    assert!(
        json.contains(
            "\"missing_transitions\":[\"send_engine_sync_probe->receive_engine_sync_report\"]"
        ),
        "json={}",
        json
    );
}
