mod support;

use gewyvern::dsl::compile_file;
use gewyvern::export::{ExportBundle, ProtocolIr};
use gewyvern::flow::ProgramOperation;
use gewyvern::ledger::{
    CpuId, FactEnvelope, FactId, FactKind, PacketDir, PacketMetaFact, SessionId,
};
use gewyvern::runtime::{RuntimeSession, SessionConfig};
use std::collections::BTreeMap;
use std::time::{Duration, SystemTime};
use support::{route_fact, sock_lineage_fact, udp_packet_fact_with_dir_and_ports_and_payload};

fn dsl_fixture_path(name: &str) -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("dsl")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

#[test]
fn dhcp_discover_runtime_path_materializes_lease_ir() {
    let export = run_payload_udp_path(
        "dhcp_discover_path.gewy",
        0xd4c1,
        "dhclient",
        68,
        67,
        &[
            (PacketDir::Egress, &[(0, 0x01), (1, 0x01), (242, 0x01)][..]),
            (PacketDir::Ingress, &[(0, 0x02), (1, 0x01), (242, 0x02)][..]),
        ],
    );

    assert_operation(&export, "dhcp_discover");
    assert_stage(&export, "send_discover");
    assert_stage(&export, "receive_offer");

    let ir = protocol_ir(&export, "dhcp_discover");
    assert_surface(ir, "dhcp", "discover", "lease", "network-control-discovery");
    assert_eq!(
        ir.semantics_category.as_deref(),
        Some("lease-discovery-path")
    );
    assert_json_replay(&export);
}

#[test]
fn dhcp_request_runtime_path_materializes_lease_ack_ir() {
    let export = run_payload_udp_path(
        "dhcp_request_path.gewy",
        0xd4c2,
        "dhclient",
        68,
        67,
        &[
            (PacketDir::Egress, &[(0, 0x01), (1, 0x01), (242, 0x03)][..]),
            (PacketDir::Ingress, &[(0, 0x02), (1, 0x01), (242, 0x05)][..]),
        ],
    );

    assert_operation(&export, "dhcp_request");
    assert_stage(&export, "send_request");
    assert_stage(&export, "receive_ack");

    let ir = protocol_ir(&export, "dhcp_request");
    assert_surface(ir, "dhcp", "request", "lease", "network-control-discovery");
    assert_eq!(ir.semantics_category.as_deref(), Some("lease-request-path"));
    assert_eq!(ir.typical_signal.as_deref(), Some("DHCPREQUEST + DHCPACK"));
    assert_json_replay(&export);
}

#[test]
fn dhcp_nak_runtime_path_materializes_lease_denied_ir() {
    let export = run_payload_udp_path(
        "dhcp_nak_path.gewy",
        0xd4c3,
        "dhclient",
        68,
        67,
        &[
            (PacketDir::Egress, &[(0, 0x01), (1, 0x01), (242, 0x03)][..]),
            (PacketDir::Ingress, &[(0, 0x02), (1, 0x01), (242, 0x06)][..]),
        ],
    );

    assert_operation(&export, "dhcp_nak");
    assert_stage(&export, "send_request");
    assert_stage(&export, "receive_nak");

    let ir = protocol_ir(&export, "dhcp_nak");
    assert_surface(ir, "dhcp", "nak", "lease", "network-control-discovery");
    assert_eq!(ir.semantics_category.as_deref(), Some("lease-denied-path"));
    assert_eq!(ir.typical_signal.as_deref(), Some("DHCPNAK"));
    assert_json_replay(&export);
}

#[test]
fn ntp_query_runtime_path_materializes_time_ir() {
    let export = run_simple_udp_path(
        "ntp_query_path.gewy",
        0x9170,
        "chrony-query",
        54020,
        123,
        &[
            (PacketDir::Egress, 0x23, 0x2300),
            (PacketDir::Ingress, 0x24, 0x2400),
        ],
    );

    assert_operation(&export, "ntp_query");
    assert_stage(&export, "send_query");
    assert_stage(&export, "receive_response");

    let ir = protocol_ir(&export, "ntp_query");
    assert_surface(ir, "ntp", "query", "query", "network-control-discovery");
    assert_eq!(ir.semantics_category.as_deref(), Some("time-query-path"));
    assert_json_replay(&export);
}

#[test]
fn coap_post_runtime_path_materializes_constrained_write_ir() {
    let export = run_simple_udp_path(
        "coap_post_path.gewy",
        0xc0a9,
        "coap-client",
        56001,
        5683,
        &[
            (PacketDir::Egress, 0x40, 0x4002),
            (PacketDir::Ingress, 0x60, 0x6041),
        ],
    );

    assert_operation(&export, "coap_post");
    assert_stage(&export, "send_request");
    assert_stage(&export, "receive_created");

    let ir = protocol_ir(&export, "coap_post");
    assert_surface(ir, "coap", "post", "write", "network-control-discovery");
    assert_eq!(
        ir.semantics_category.as_deref(),
        Some("constrained-resource-create-path")
    );
    assert_json_replay(&export);
}

#[test]
fn stun_allocate_runtime_path_materializes_relay_ir() {
    let export = run_simple_udp_path(
        "stun_allocate_path.gewy",
        0x5711,
        "turn-client",
        54010,
        3478,
        &[
            (PacketDir::Egress, 0x00, 0x0003),
            (PacketDir::Ingress, 0x01, 0x0103),
        ],
    );

    assert_operation(&export, "stun_allocate");
    assert_stage(&export, "send_allocate_request");
    assert_stage(&export, "receive_allocate_response");

    let ir = protocol_ir(&export, "stun_allocate");
    assert_surface(ir, "stun", "allocate", "relay", "network-control-discovery");
    assert_eq!(
        ir.semantics_category.as_deref(),
        Some("relay-allocation-path")
    );
    assert_json_replay(&export);
}

#[test]
fn dns_error_runtime_path_materializes_name_resolution_failure_ir() {
    let export = run_payload_udp_path(
        "dns_error_path.gewy",
        0xd053,
        "dig",
        53000,
        53,
        &[(PacketDir::Ingress, &[(2, 0x80), (3, 0x03)][..])],
    );

    assert_operation(&export, "dns_error");
    assert_stage(&export, "receive_nxdomain");

    let ir = protocol_ir(&export, "dns_error");
    assert_surface(ir, "dns", "error", "error", "network-control-discovery");
    assert_eq!(
        ir.semantics_category.as_deref(),
        Some("name-resolution-error")
    );
    assert_eq!(
        ir.typical_signal.as_deref(),
        Some("DNS QR response with non-zero rcode")
    );
    assert_json_replay(&export);
}

fn run_simple_udp_path(
    fixture: &str,
    cookie: u64,
    process_name: &str,
    local_port: u16,
    remote_port: u16,
    packets: &[(PacketDir, u8, u16)],
) -> ExportBundle {
    let binding = compile_file(&dsl_fixture_path(fixture)).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, cookie, 8700, process_name));
    session.ingest(route_fact(2, cookie, 7));

    for (index, (dir, byte0, prefix2)) in packets.iter().enumerate() {
        session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
            3 + index as u64,
            cookie,
            120,
            *dir,
            Some(local_port),
            Some(remote_port),
            Some(*byte0),
            Some(*prefix2),
        ));
    }

    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(100));
    session.export_bundle()
}

fn run_payload_udp_path(
    fixture: &str,
    cookie: u64,
    process_name: &str,
    local_port: u16,
    remote_port: u16,
    packets: &[(PacketDir, &[(u16, u8)])],
) -> ExportBundle {
    let binding = compile_file(&dsl_fixture_path(fixture)).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, cookie, 8701, process_name));
    session.ingest(route_fact(2, cookie, 7));

    for (index, (dir, payload_bytes)) in packets.iter().enumerate() {
        session.ingest(udp_payload_fact(
            3 + index as u64,
            cookie,
            *dir,
            local_port,
            remote_port,
            payload_bytes,
        ));
    }

    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(100));
    session.export_bundle()
}

fn udp_payload_fact(
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
            payload_bytes: payload_bytes.iter().copied().collect::<BTreeMap<_, _>>(),
            l3_proto: 0x0800,
            l4_proto: 17,
            tot_len: 300,
            tcp_flags: 0,
            seq: None,
            ack: None,
            window: None,
        }),
    }
}

fn assert_operation(export: &ExportBundle, operation: &str) {
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom(operation.into())
    );
}

fn assert_surface(ir: &ProtocolIr, protocol: &str, entry: &str, shelf: &str, cluster: &str) {
    assert_eq!(ir.protocol, protocol);
    assert_eq!(ir.entry, entry);
    assert_eq!(ir.shelf_key.as_deref(), Some(shelf));
    assert_eq!(ir.cluster_key.as_deref(), Some(cluster));
}

fn assert_stage(export: &ExportBundle, phase: &str) {
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some(phase)),
        "missing stage {phase}"
    );
}

fn assert_json_replay(export: &ExportBundle) {
    let replayed = ExportBundle::from_json(&export.to_json()).expect("export json should replay");
    assert_eq!(replayed.protocol_ir, export.protocol_ir);
}

fn protocol_ir<'a>(export: &'a ExportBundle, operation: &str) -> &'a ProtocolIr {
    export
        .protocol_ir
        .iter()
        .find(|item| item.operation == operation)
        .expect("protocol IR should materialize")
}
