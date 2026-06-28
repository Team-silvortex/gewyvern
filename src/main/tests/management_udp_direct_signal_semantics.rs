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
fn summary_json_carries_dhcp_nak_denied_detail() {
    let binding = compile_file(&dsl_fixture_path("dhcp_nak_path.gewy"))
        .expect("dhcp_nak_path DSL should compile");
    let export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 94001, 68, "dhclient"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    94001,
                    7,
                    SessionId(1),
                ),
                udp_packet_fact_with_payload_bytes_for_tests(
                    3,
                    94001,
                    PacketDir::Egress,
                    68,
                    67,
                    &[(0, 0x01), (1, 0x01), (242, 0x03)],
                ),
                udp_packet_fact_with_payload_bytes_for_tests(
                    4,
                    94001,
                    PacketDir::Ingress,
                    68,
                    67,
                    &[(0, 0x02), (1, 0x01), (242, 0x06)],
                ),
            ],
        ),
        &Cli::from_args(["--demo".to_string(), "udp".to_string()]).unwrap(),
    );
    let json = summary_json("dsl_demo", &export);
    assert!(
        json.contains("\"operations\":[\"dhcp_nak\"]"),
        "json={}",
        json
    );
    assert!(
        json.contains("\"primary_failure_mode\":\"server_denied\""),
        "json={}",
        json
    );
    assert!(
        json.contains("\"primary_failure_detail\":\"request_rejected\""),
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
fn summary_json_carries_stun_binding_error_semantic_detail() {
    let binding = compile_file(&dsl_fixture_path("stun_binding_error_path.gewy"))
        .expect("stun_binding_error_path DSL should compile");
    let export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 94002, 45001, "stun-client"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    94002,
                    7,
                    SessionId(1),
                ),
                udp_packet_fact_with_payload_bytes_for_tests(
                    3,
                    94002,
                    PacketDir::Egress,
                    45001,
                    3478,
                    &[(0, 0x00), (1, 0x01)],
                ),
                udp_packet_fact_with_payload_bytes_for_tests(
                    4,
                    94002,
                    PacketDir::Ingress,
                    45001,
                    3478,
                    &[(0, 0x01), (1, 0x11)],
                ),
            ],
        ),
        &Cli::from_args(["--demo".to_string(), "udp".to_string()]).unwrap(),
    );
    let json = summary_json("dsl_demo", &export);
    assert!(
        json.contains("\"operations\":[\"stun_binding_error\"]"),
        "json={}",
        json
    );
    assert!(
        json.contains("\"primary_failure_mode\":\"semantic_error\""),
        "json={}",
        json
    );
    assert!(
        json.contains("\"primary_failure_detail\":\"protocol_error\""),
        "json={}",
        json
    );
    assert!(
        json.contains("\"primary_failure_basis\":\"direct_protocol_signal\""),
        "json={}",
        json
    );
}
