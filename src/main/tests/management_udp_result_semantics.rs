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
fn ntp_query_response_keeps_management_query_healthy() {
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/ntp_query_path.gewy")
        .expect("ntp_query_path DSL should compile");
    let export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 93001, 45001, "chronyd"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    93001,
                    7,
                    SessionId(1),
                ),
                udp_packet_fact_with_payload_bytes_for_tests(
                    3,
                    93001,
                    PacketDir::Egress,
                    54020,
                    123,
                    &[(0, 0x23), (1, 0x00)],
                ),
                udp_packet_fact_with_payload_bytes_for_tests(
                    4,
                    93001,
                    PacketDir::Ingress,
                    54020,
                    123,
                    &[(0, 0x24), (1, 0x02)],
                ),
            ],
        ),
        &Cli::from_args(["--demo".to_string(), "udp".to_string()]).unwrap(),
    );
    let json = summary_json("dsl_demo", &export);
    assert!(export.program_findings.is_empty());
    assert!(export.module_findings.is_empty());
    assert!(json.contains("\"status\":\"healthy\""), "json={}", json);
    assert!(
        json.contains("\"primary_failure_mode\":\"none\""),
        "json={}",
        json
    );
    assert!(json.contains("\"operations\":[\"ntp_query\"]"), "json={}", json);
    assert!(
        json.contains("\"last_phase\":\"receive_response\""),
        "json={}",
        json
    );
}

#[test]
fn dhcp_offer_result_keeps_discover_surface_healthy() {
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/dhcp_discover_path.gewy")
        .expect("dhcp_discover_path DSL should compile");
    let export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 93002, 68, "dhclient"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    93002,
                    7,
                    SessionId(1),
                ),
                udp_packet_fact_with_payload_bytes_for_tests(
                    3,
                    93002,
                    PacketDir::Egress,
                    68,
                    67,
                    &[(0, 0x01), (1, 0x01), (242, 0x01)],
                ),
                udp_packet_fact_with_payload_bytes_for_tests(
                    4,
                    93002,
                    PacketDir::Ingress,
                    68,
                    67,
                    &[(0, 0x02), (1, 0x01), (242, 0x02)],
                ),
            ],
        ),
        &Cli::from_args(["--demo".to_string(), "udp".to_string()]).unwrap(),
    );
    let json = summary_json("dsl_demo", &export);
    assert!(export.program_findings.is_empty());
    assert!(json.contains("\"status\":\"healthy\""), "json={}", json);
    assert!(
        json.contains("\"primary_failure_mode\":\"none\""),
        "json={}",
        json
    );
    assert!(
        json.contains("\"operations\":[\"dhcp_discover\"]"),
        "json={}",
        json
    );
    assert!(json.contains("\"last_phase\":\"receive_offer\""), "json={}", json);
}

#[test]
fn dhcp_offer_result_keeps_client_surface_healthy() {
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/dhcp_client_path.gewy")
        .expect("dhcp_client_path DSL should compile");
    let export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 93012, 68, "dhclient"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    93012,
                    7,
                    SessionId(1),
                ),
                udp_packet_fact_with_payload_bytes_for_tests(
                    3,
                    93012,
                    PacketDir::Egress,
                    68,
                    67,
                    &[(0, 0x01), (1, 0x01)],
                ),
                udp_packet_fact_with_payload_bytes_for_tests(
                    4,
                    93012,
                    PacketDir::Ingress,
                    68,
                    67,
                    &[(0, 0x02), (1, 0x01)],
                ),
            ],
        ),
        &Cli::from_args(["--demo".to_string(), "udp".to_string()]).unwrap(),
    );
    let json = summary_json("dsl_demo", &export);
    assert!(export.program_findings.is_empty());
    assert!(json.contains("\"status\":\"healthy\""), "json={}", json);
    assert!(
        json.contains("\"primary_failure_mode\":\"none\""),
        "json={}",
        json
    );
    assert!(json.contains("\"operations\":[\"dhcp_client\"]"), "json={}", json);
    assert!(json.contains("\"last_phase\":\"receive_offer\""), "json={}", json);
}

#[test]
fn dhcp_ack_result_keeps_request_surface_healthy() {
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/dhcp_request_path.gewy")
        .expect("dhcp_request_path DSL should compile");
    let export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 93003, 68, "dhclient"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    93003,
                    7,
                    SessionId(1),
                ),
                udp_packet_fact_with_payload_bytes_for_tests(
                    3,
                    93003,
                    PacketDir::Egress,
                    68,
                    67,
                    &[(0, 0x01), (1, 0x01), (242, 0x03)],
                ),
                udp_packet_fact_with_payload_bytes_for_tests(
                    4,
                    93003,
                    PacketDir::Ingress,
                    68,
                    67,
                    &[(0, 0x02), (1, 0x01), (242, 0x05)],
                ),
            ],
        ),
        &Cli::from_args(["--demo".to_string(), "udp".to_string()]).unwrap(),
    );
    let json = summary_json("dsl_demo", &export);
    assert!(export.program_findings.is_empty());
    assert!(json.contains("\"status\":\"healthy\""), "json={}", json);
    assert!(
        json.contains("\"primary_failure_mode\":\"none\""),
        "json={}",
        json
    );
    assert!(
        json.contains("\"operations\":[\"dhcp_request\"]"),
        "json={}",
        json
    );
    assert!(json.contains("\"last_phase\":\"receive_ack\""), "json={}", json);
}

#[test]
fn ntp_response_keeps_client_surface_healthy() {
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/ntp_client_path.gewy")
        .expect("ntp_client_path DSL should compile");
    let export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 93013, 45013, "chronyd"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    93013,
                    7,
                    SessionId(1),
                ),
                udp_packet_fact_with_payload_bytes_for_tests(
                    3,
                    93013,
                    PacketDir::Egress,
                    54021,
                    123,
                    &[(0, 0x23), (1, 0x00)],
                ),
                udp_packet_fact_with_payload_bytes_for_tests(
                    4,
                    93013,
                    PacketDir::Ingress,
                    54021,
                    123,
                    &[(0, 0x24), (1, 0x02)],
                ),
            ],
        ),
        &Cli::from_args(["--demo".to_string(), "udp".to_string()]).unwrap(),
    );
    let json = summary_json("dsl_demo", &export);
    assert!(export.program_findings.is_empty());
    assert!(json.contains("\"status\":\"healthy\""), "json={}", json);
    assert!(
        json.contains("\"primary_failure_mode\":\"none\""),
        "json={}",
        json
    );
    assert!(json.contains("\"operations\":[\"ntp_client\"]"), "json={}", json);
    assert!(
        json.contains("\"last_phase\":\"receive_response\""),
        "json={}",
        json
    );
}

#[test]
fn stun_binding_response_keeps_surface_healthy() {
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/stun_binding_path.gewy")
        .expect("stun_binding_path DSL should compile");
    let export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 93004, 45001, "stun-client"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    93004,
                    7,
                    SessionId(1),
                ),
                udp_packet_fact_with_payload_bytes_for_tests(
                    3,
                    93004,
                    PacketDir::Egress,
                    45001,
                    3478,
                    &[(0, 0x00), (1, 0x01)],
                ),
                udp_packet_fact_with_payload_bytes_for_tests(
                    4,
                    93004,
                    PacketDir::Ingress,
                    45001,
                    3478,
                    &[(0, 0x01), (1, 0x01)],
                ),
            ],
        ),
        &Cli::from_args(["--demo".to_string(), "udp".to_string()]).unwrap(),
    );
    let json = summary_json("dsl_demo", &export);
    assert!(export.program_findings.is_empty());
    assert!(json.contains("\"status\":\"healthy\""), "json={}", json);
    assert!(
        json.contains("\"primary_failure_mode\":\"none\""),
        "json={}",
        json
    );
    assert!(
        json.contains("\"operations\":[\"stun_binding\"]"),
        "json={}",
        json
    );
    assert!(
        json.contains("\"last_phase\":\"receive_response\""),
        "json={}",
        json
    );
}

#[test]
fn stun_allocate_response_keeps_relay_surface_healthy() {
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/stun_allocate_path.gewy")
        .expect("stun_allocate_path DSL should compile");
    let export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 93005, 45001, "turn-client"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    93005,
                    7,
                    SessionId(1),
                ),
                udp_packet_fact_with_payload_bytes_for_tests(
                    3,
                    93005,
                    PacketDir::Egress,
                    45001,
                    3478,
                    &[(0, 0x00), (1, 0x03)],
                ),
                udp_packet_fact_with_payload_bytes_for_tests(
                    4,
                    93005,
                    PacketDir::Ingress,
                    45001,
                    3478,
                    &[(0, 0x01), (1, 0x03)],
                ),
            ],
        ),
        &Cli::from_args(["--demo".to_string(), "udp".to_string()]).unwrap(),
    );
    let json = summary_json("dsl_demo", &export);
    assert!(export.program_findings.is_empty());
    assert!(json.contains("\"status\":\"healthy\""), "json={}", json);
    assert!(
        json.contains("\"primary_failure_mode\":\"none\""),
        "json={}",
        json
    );
    assert!(
        json.contains("\"operations\":[\"stun_allocate\"]"),
        "json={}",
        json
    );
    assert!(
        json.contains("\"last_phase\":\"receive_allocate_response\""),
        "json={}",
        json
    );
}
