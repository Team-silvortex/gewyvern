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
fn snmp_v3_auth_response_keeps_security_surface_healthy() {
    let binding = compile_file(&dsl_fixture_path("snmp_v3_auth_path.gewy"))
        .expect("snmp_v3_auth_path DSL should compile");
    let export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 93021, 45021, "snmpget"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    93021,
                    7,
                    SessionId(1),
                ),
                udp_packet_fact_with_payload_bytes_for_tests(
                    3,
                    93021,
                    PacketDir::Egress,
                    49021,
                    161,
                    &[(0, 0x30), (4, 0x03), (18, 0x01)],
                ),
                udp_packet_fact_with_payload_bytes_for_tests(
                    4,
                    93021,
                    PacketDir::Ingress,
                    49021,
                    161,
                    &[(0, 0x30), (4, 0x03), (18, 0x01)],
                ),
            ],
        ),
        &Cli::from_args(["--demo".to_string(), "udp".to_string()]).unwrap(),
    );
    let json = summary_json("dsl_demo", &export);
    assert!(
        export.program_findings.is_empty(),
        "json={} findings={:#?}",
        json,
        export.program_findings
    );
    assert!(json.contains("\"status\":\"healthy\""), "json={}", json);
    assert!(
        json.contains("\"primary_failure_mode\":\"none\""),
        "json={}",
        json
    );
    assert!(
        json.contains("\"operations\":[\"snmp_v3_auth\"]"),
        "json={}",
        json
    );
    assert!(
        json.contains("\"last_phase\":\"receive_v3_auth_response\""),
        "json={}",
        json
    );
}

#[test]
fn snmp_v3_priv_response_keeps_security_surface_healthy() {
    let binding = compile_file(&dsl_fixture_path("snmp_v3_priv_path.gewy"))
        .expect("snmp_v3_priv_path DSL should compile");
    let export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 93022, 45022, "snmpget"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    93022,
                    7,
                    SessionId(1),
                ),
                udp_packet_fact_with_payload_bytes_for_tests(
                    3,
                    93022,
                    PacketDir::Egress,
                    49022,
                    161,
                    &[(0, 0x30), (4, 0x03), (18, 0x03)],
                ),
                udp_packet_fact_with_payload_bytes_for_tests(
                    4,
                    93022,
                    PacketDir::Ingress,
                    49022,
                    161,
                    &[(0, 0x30), (4, 0x03), (18, 0x03)],
                ),
            ],
        ),
        &Cli::from_args(["--demo".to_string(), "udp".to_string()]).unwrap(),
    );
    let json = summary_json("dsl_demo", &export);
    assert!(
        export.program_findings.is_empty(),
        "json={} findings={:#?}",
        json,
        export.program_findings
    );
    assert!(json.contains("\"status\":\"healthy\""), "json={}", json);
    assert!(
        json.contains("\"primary_failure_mode\":\"none\""),
        "json={}",
        json
    );
    assert!(
        json.contains("\"operations\":[\"snmp_v3_priv\"]"),
        "json={}",
        json
    );
    assert!(
        json.contains("\"last_phase\":\"receive_v3_priv_response\""),
        "json={}",
        json
    );
}

#[test]
fn snmp_trap_send_keeps_notify_surface_healthy() {
    let binding = compile_file(&dsl_fixture_path("snmp_trap_path.gewy"))
        .expect("snmp_trap_path DSL should compile");
    let export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 93023, 45023, "snmptrap"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    93023,
                    7,
                    SessionId(1),
                ),
                udp_packet_fact_with_payload_bytes_for_tests(
                    3,
                    93023,
                    PacketDir::Egress,
                    49162,
                    162,
                    &[(0, 0x30), (13, 0xa7)],
                ),
            ],
        ),
        &Cli::from_args(["--demo".to_string(), "udp".to_string()]).unwrap(),
    );
    let json = summary_json("dsl_demo", &export);
    assert!(
        export.program_findings.is_empty(),
        "json={} findings={:#?}",
        json,
        export.program_findings
    );
    assert!(json.contains("\"status\":\"healthy\""), "json={}", json);
    assert!(
        json.contains("\"primary_failure_mode\":\"none\""),
        "json={}",
        json
    );
    assert!(
        json.contains("\"operations\":[\"snmp_trap\"]"),
        "json={}",
        json
    );
    assert!(
        json.contains("\"last_phase\":\"send_trap_notification\""),
        "json={}",
        json
    );
}

#[test]
fn snmp_trap_recv_keeps_manage_surface_healthy() {
    let binding = compile_file(&dsl_fixture_path("snmp_trap_recv_path.gewy"))
        .expect("snmp_trap_recv_path DSL should compile");
    let export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 93024, 162, "snmptrapd"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    93024,
                    7,
                    SessionId(1),
                ),
                udp_packet_fact_with_payload_bytes_for_tests(
                    3,
                    93024,
                    PacketDir::Ingress,
                    162,
                    49162,
                    &[(0, 0x30), (13, 0xa7)],
                ),
            ],
        ),
        &Cli::from_args(["--demo".to_string(), "udp".to_string()]).unwrap(),
    );
    let json = summary_json("dsl_demo", &export);
    assert!(
        export.program_findings.is_empty(),
        "json={} findings={:#?}",
        json,
        export.program_findings
    );
    assert!(json.contains("\"status\":\"healthy\""), "json={}", json);
    assert!(
        json.contains("\"primary_failure_mode\":\"none\""),
        "json={}",
        json
    );
    assert!(
        json.contains("\"operations\":[\"snmp_trap_recv\"]"),
        "json={}",
        json
    );
    assert!(
        json.contains("\"last_phase\":\"receive_trap_notification\""),
        "json={}",
        json
    );
}
