use super::*;

fn udp_packet_fact_with_payload_bytes_for_tests(
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
            tot_len: 128,
            tcp_flags: 0,
            seq: None,
            ack: None,
            window: None,
        }),
    }
}

#[test]
fn summary_json_carries_ntp_query_timeout_detail() {
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/ntp_query_path.gewy")
        .expect("ntp_query_path DSL should compile");
    let mut export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 92001, 44001, "chrony-query"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    92001,
                    7,
                    SessionId(1),
                ),
                udp_packet_fact_with_payload_bytes_for_tests(
                    3,
                    92001,
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
    let json = summary_json("dsl_demo", &export);
    assert!(
        json.contains("\"operations\":[\"ntp_query\"]"),
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
        json.contains("\"missing_transitions\":[\"send_query->receive_response\"]"),
        "json={}",
        json
    );
}

#[test]
fn summary_json_carries_dhcp_discover_timeout_detail() {
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/dhcp_discover_path.gewy")
        .expect("dhcp_discover_path DSL should compile");
    let mut export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 92002, 68, "dhclient"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    92002,
                    7,
                    SessionId(1),
                ),
                udp_packet_fact_with_payload_bytes_for_tests(
                    3,
                    92002,
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
    let json = summary_json("dsl_demo", &export);
    assert!(
        json.contains("\"operations\":[\"dhcp_discover\"]"),
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
        json.contains("\"missing_transitions\":[\"send_discover->receive_offer\"]"),
        "json={}",
        json
    );
}

#[test]
fn summary_json_carries_dhcp_request_timeout_detail() {
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/dhcp_request_path.gewy")
        .expect("dhcp_request_path DSL should compile");
    let mut export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 92003, 68, "dhclient"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    92003,
                    7,
                    SessionId(1),
                ),
                udp_packet_fact_with_payload_bytes_for_tests(
                    3,
                    92003,
                    PacketDir::Egress,
                    68,
                    67,
                    &[(0, 0x01), (1, 0x01), (242, 0x03)],
                ),
            ],
        ),
        &Cli::from_args(["--demo".to_string(), "udp".to_string()]).unwrap(),
    );
    let flow = export.program_flows[0].clone();
    push_synthetic_missing_stage_finding(
        &mut export,
        &flow,
        "dhcp_request_path",
        "dhcp_request",
        "receive_ack",
        "receive_payload",
        "send_request->receive_ack",
        "emit_payload->receive_payload",
        "transport_io",
        "synthetic missing dhcp ack",
        "udp_packet_meta_fragment",
        "missing_signal:packet_observed",
    );
    let json = summary_json("dsl_demo", &export);
    assert!(
        json.contains("\"operations\":[\"dhcp_request\"]"),
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
        json.contains("\"missing_transitions\":[\"send_request->receive_ack\"]"),
        "json={}",
        json
    );
}

#[test]
fn summary_json_carries_stun_binding_timeout_detail() {
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/stun_binding_path.gewy")
        .expect("stun_binding_path DSL should compile");
    let mut export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 92004, 45001, "stun-client"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    92004,
                    7,
                    SessionId(1),
                ),
                udp_packet_fact_with_payload_bytes_for_tests(
                    3,
                    92004,
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
        "synthetic missing stun binding response",
        "udp_packet_meta_fragment",
        "missing_signal:packet_observed",
    );
    let json = summary_json("dsl_demo", &export);
    assert!(
        json.contains("\"operations\":[\"stun_binding\"]"),
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
        json.contains("\"missing_transitions\":[\"send_request->receive_response\"]"),
        "json={}",
        json
    );
}

#[test]
fn summary_json_carries_ntp_sync_timeout_detail() {
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/ntp_sync_path.gewy")
        .expect("ntp_sync_path DSL should compile");
    let mut export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 92005, 44002, "chronyd"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    92005,
                    7,
                    SessionId(1),
                ),
                udp_packet_fact_with_payload_bytes_for_tests(
                    3,
                    92005,
                    PacketDir::Egress,
                    54021,
                    123,
                    &[(0, 0x1b), (1, 0x00)],
                ),
            ],
        ),
        &Cli::from_args(["--demo".to_string(), "udp".to_string()]).unwrap(),
    );
    let flow = export.program_flows[0].clone();
    push_synthetic_missing_stage_finding(
        &mut export,
        &flow,
        "ntp_sync_path",
        "ntp_sync",
        "receive_sync_response",
        "receive_payload",
        "send_sync_request->receive_sync_response",
        "emit_payload->receive_payload",
        "transport_io",
        "synthetic missing ntp sync response",
        "udp_packet_meta_fragment",
        "missing_signal:packet_observed",
    );
    let json = summary_json("dsl_demo", &export);
    assert!(
        json.contains("\"operations\":[\"ntp_sync\"]"),
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
        json.contains("\"missing_transitions\":[\"send_sync_request->receive_sync_response\"]"),
        "json={}",
        json
    );
}

#[test]
fn summary_json_carries_stun_allocate_timeout_detail() {
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/stun_allocate_path.gewy")
        .expect("stun_allocate_path DSL should compile");
    let mut export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 92006, 45002, "turn-client"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    92006,
                    7,
                    SessionId(1),
                ),
                udp_packet_fact_with_payload_bytes_for_tests(
                    3,
                    92006,
                    PacketDir::Egress,
                    54031,
                    3478,
                    &[(0, 0x00), (1, 0x03)],
                ),
            ],
        ),
        &Cli::from_args(["--demo".to_string(), "udp".to_string()]).unwrap(),
    );
    let flow = export.program_flows[0].clone();
    push_synthetic_missing_stage_finding(
        &mut export,
        &flow,
        "stun_allocate_path",
        "stun_allocate",
        "receive_allocate_response",
        "receive_payload",
        "send_allocate_request->receive_allocate_response",
        "emit_payload->receive_payload",
        "transport_io",
        "synthetic missing stun allocate response",
        "udp_packet_meta_fragment",
        "missing_signal:packet_observed",
    );
    let json = summary_json("dsl_demo", &export);
    assert!(
        json.contains("\"operations\":[\"stun_allocate\"]"),
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
        json.contains(
            "\"missing_transitions\":[\"send_allocate_request->receive_allocate_response\"]"
        ),
        "json={}",
        json
    );
}

#[test]
fn summary_json_carries_stun_refresh_timeout_detail() {
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/stun_refresh_path.gewy")
        .expect("stun_refresh_path DSL should compile");
    let mut export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 92007, 45003, "turn-client"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    92007,
                    7,
                    SessionId(1),
                ),
                udp_packet_fact_with_payload_bytes_for_tests(
                    3,
                    92007,
                    PacketDir::Egress,
                    54032,
                    3478,
                    &[(0, 0x00), (1, 0x04)],
                ),
            ],
        ),
        &Cli::from_args(["--demo".to_string(), "udp".to_string()]).unwrap(),
    );
    let flow = export.program_flows[0].clone();
    push_synthetic_missing_stage_finding(
        &mut export,
        &flow,
        "stun_refresh_path",
        "stun_refresh",
        "receive_refresh_response",
        "receive_payload",
        "send_refresh_request->receive_refresh_response",
        "emit_payload->receive_payload",
        "transport_io",
        "synthetic missing stun refresh response",
        "udp_packet_meta_fragment",
        "missing_signal:packet_observed",
    );
    let json = summary_json("dsl_demo", &export);
    assert!(
        json.contains("\"operations\":[\"stun_refresh\"]"),
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
        json.contains(
            "\"missing_transitions\":[\"send_refresh_request->receive_refresh_response\"]"
        ),
        "json={}",
        json
    );
}
