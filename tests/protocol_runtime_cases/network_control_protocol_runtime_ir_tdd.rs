use crate::support;

use gewyvern::dsl::compile_file;
use gewyvern::export::{ExportBundle, ProtocolIr};
use gewyvern::flow::ProgramOperation;
use gewyvern::ledger::{
    CpuId, FactEnvelope, FactId, FactKind, PacketDir, PacketMetaFact, SessionId,
};
use gewyvern::runtime::{RuntimeSession, SessionConfig};
use std::collections::BTreeMap;
use std::time::{Duration, SystemTime};
use support::{packet_fact_with_dir_and_payload_bytes, route_fact, sock_lineage_fact};

fn dsl_fixture_path(name: &str) -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("dsl")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

#[test]
fn arp_request_runtime_path_materializes_neighbor_ir() {
    let binding = compile_file(&dsl_fixture_path("arp_request_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(arp_packet_fact(1, PacketDir::Egress, 0x01));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_operation(&export, "arp_request");
    assert_stage(&export, "send_who_has");

    let ir = protocol_ir(&export, "arp_request");
    assert_surface(ir, "arp", "request", "request", "network-control-discovery");
    assert_eq!(
        ir.semantics_category.as_deref(),
        Some("neighbor-resolution-path")
    );
    assert_json_replay(&export);
}

#[test]
fn icmp_unreachable_runtime_path_materializes_failure_ir() {
    let binding = compile_file(&dsl_fixture_path("icmp_unreachable_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 0x1c11, 0, "probe"));
    session.ingest(route_fact(2, 0x1c11, 7));
    session.ingest(icmp_packet_fact(3, 0x1c11, PacketDir::Ingress, 3));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_operation(&export, "icmp_unreachable");
    assert_stage(&export, "receive_unreachable");

    let ir = protocol_ir(&export, "icmp_unreachable");
    assert_surface(
        ir,
        "icmp",
        "unreachable",
        "failure",
        "network-control-discovery",
    );
    assert_eq!(ir.semantics_category.as_deref(), Some("failure-path"));
    assert_eq!(ir.typical_signal.as_deref(), Some("type 3 unreachable"));
    assert_json_replay(&export);
}

#[test]
fn icmp_unreachable_runtime_ir_does_not_materialize_for_echo_reply() {
    let binding = compile_file(&dsl_fixture_path("icmp_unreachable_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 0x1c12, 0, "probe"));
    session.ingest(route_fact(2, 0x1c12, 7));
    session.ingest(icmp_packet_fact(3, 0x1c12, PacketDir::Ingress, 0));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_operation(&export, "icmp_unreachable");
    assert_no_stage(&export, "receive_unreachable");
    assert_no_protocol_ir(&export, "icmp_unreachable");
    assert_json_replay(&export);
}

#[test]
fn icmpv6_echo_runtime_path_materializes_reachability_ir() {
    let binding = compile_file(&dsl_fixture_path("icmpv6_echo_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 0x6c11, 0, "ping6"));
    session.ingest(route_fact(2, 0x6c11, 7));
    session.ingest(icmpv6_packet_fact(3, 0x6c11, PacketDir::Egress, 128));
    session.ingest(icmpv6_packet_fact(4, 0x6c11, PacketDir::Ingress, 129));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_operation(&export, "icmpv6_echo");
    assert_stage(&export, "send_echo_request");
    assert_stage(&export, "receive_echo_reply");

    let ir = protocol_ir(&export, "icmpv6_echo");
    assert_surface(ir, "icmpv6", "echo", "echo", "network-control-discovery");
    assert_eq!(ir.semantics_category.as_deref(), Some("reachability-path"));
    assert_eq!(
        ir.typical_signal.as_deref(),
        Some("type 128 request / type 129 reply")
    );
    assert_json_replay(&export);
}

#[test]
fn icmpv6_unreachable_runtime_path_materializes_failure_ir() {
    let binding = compile_file(&dsl_fixture_path("icmpv6_unreachable_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 0x6c12, 0, "probe6"));
    session.ingest(route_fact(2, 0x6c12, 7));
    session.ingest(icmpv6_packet_fact(3, 0x6c12, PacketDir::Ingress, 1));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_operation(&export, "icmpv6_unreachable");
    assert_stage(&export, "receive_unreachable");

    let ir = protocol_ir(&export, "icmpv6_unreachable");
    assert_surface(
        ir,
        "icmpv6",
        "unreachable",
        "failure",
        "network-control-discovery",
    );
    assert_eq!(ir.semantics_category.as_deref(), Some("failure-path"));
    assert_eq!(
        ir.typical_signal.as_deref(),
        Some("type 1 destination unreachable")
    );
    assert_json_replay(&export);
}

#[test]
fn icmpv6_unreachable_runtime_ir_does_not_materialize_for_echo_reply() {
    let binding = compile_file(&dsl_fixture_path("icmpv6_unreachable_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 0x6c13, 0, "probe6"));
    session.ingest(route_fact(2, 0x6c13, 7));
    session.ingest(icmpv6_packet_fact(3, 0x6c13, PacketDir::Ingress, 129));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_operation(&export, "icmpv6_unreachable");
    assert_no_stage(&export, "receive_unreachable");
    assert_no_protocol_ir(&export, "icmpv6_unreachable");
    assert_json_replay(&export);
}

#[test]
fn bgp_open_runtime_path_materializes_routing_ir() {
    let binding = compile_file(&dsl_fixture_path("bgp_open_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 0xb917, 179, "bgpd"));
    session.ingest(route_fact(2, 0xb917, 7));
    session.ingest(bgp_packet_fact(3, 0xb917, PacketDir::Egress, 1));
    session.ingest(bgp_packet_fact(4, 0xb917, PacketDir::Ingress, 1));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert_operation(&export, "bgp_open");
    assert_stage(&export, "send_open");
    assert_stage(&export, "receive_open");

    let ir = protocol_ir(&export, "bgp_open");
    assert_surface(ir, "bgp", "open", "session", "network-control-discovery");
    assert_eq!(
        ir.semantics_category.as_deref(),
        Some("routing-control-session")
    );
    assert_json_replay(&export);
}

#[test]
fn ospf_dbdesc_runtime_path_materializes_link_state_ir() {
    let binding = compile_file(&dsl_fixture_path("ospf_dbdesc_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(ospf_packet_fact(1, PacketDir::Egress, 2));
    session.ingest(ospf_packet_fact(2, PacketDir::Ingress, 2));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_operation(&export, "ospf_dbdesc");
    assert_stage(&export, "send_dbdesc");
    assert_stage(&export, "receive_dbdesc");

    let ir = protocol_ir(&export, "ospf_dbdesc");
    assert_surface(
        ir,
        "ospf",
        "dbdesc",
        "database",
        "network-control-discovery",
    );
    assert_eq!(
        ir.semantics_category.as_deref(),
        Some("link-state-database-sync")
    );
    assert_json_replay(&export);
}

fn arp_packet_fact(id: u64, dir: PacketDir, opcode: u8) -> FactEnvelope {
    FactEnvelope {
        id: FactId(id),
        ts: SystemTime::UNIX_EPOCH + Duration::from_millis(id * 10),
        cpu: CpuId(0),
        ifindex: Some(2),
        session: SessionId(1),
        fragment_id: "udp_packet_meta_fragment".into(),
        kind: FactKind::PacketMeta(PacketMetaFact {
            netns: 1,
            sk_cookie: None,
            dir,
            local_port: None,
            remote_port: None,
            payload_byte0: Some(0x00),
            payload_byte1: Some(0x01),
            payload_prefix2: Some(0x0001),
            payload_prefix4: Some(0x00010800),
            payload_byte4: Some(0x06),
            payload_byte5: Some(0x04),
            payload_byte9: None,
            payload_byte10: None,
            payload_byte13: None,
            payload_bytes: BTreeMap::from([(6, 0x00), (7, opcode)]),
            l3_proto: 0x0806,
            l4_proto: 0,
            tot_len: 28,
            tcp_flags: 0,
            seq: None,
            ack: None,
            window: None,
        }),
    }
}

fn icmp_packet_fact(id: u64, cookie: u64, dir: PacketDir, type_byte: u8) -> FactEnvelope {
    reachability_packet_fact(id, cookie, dir, type_byte, 0x0800, 1, 84)
}

fn icmpv6_packet_fact(id: u64, cookie: u64, dir: PacketDir, type_byte: u8) -> FactEnvelope {
    reachability_packet_fact(id, cookie, dir, type_byte, 0x86dd, 58, 96)
}

fn reachability_packet_fact(
    id: u64,
    cookie: u64,
    dir: PacketDir,
    type_byte: u8,
    l3_proto: u16,
    l4_proto: u8,
    tot_len: u32,
) -> FactEnvelope {
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
            local_port: None,
            remote_port: None,
            payload_byte0: Some(type_byte),
            payload_byte1: Some(0),
            payload_prefix2: Some(u16::from_be_bytes([type_byte, 0])),
            payload_prefix4: None,
            payload_byte4: None,
            payload_byte5: None,
            payload_byte9: None,
            payload_byte10: None,
            payload_byte13: None,
            payload_bytes: BTreeMap::new(),
            l3_proto,
            l4_proto,
            tot_len,
            tcp_flags: 0,
            seq: None,
            ack: None,
            window: None,
        }),
    }
}

fn bgp_packet_fact(id: u64, cookie: u64, dir: PacketDir, msg_type: u8) -> FactEnvelope {
    packet_fact_with_dir_and_payload_bytes(
        id,
        cookie,
        0x18,
        dir,
        Some(50179),
        Some(179),
        &[
            (0, 0xff),
            (1, 0xff),
            (2, 0xff),
            (3, 0xff),
            (16, 0x00),
            (17, 0x13),
            (18, msg_type),
        ],
    )
}

fn ospf_packet_fact(id: u64, dir: PacketDir, packet_type: u8) -> FactEnvelope {
    FactEnvelope {
        id: FactId(id),
        ts: SystemTime::UNIX_EPOCH + Duration::from_millis(id * 10),
        cpu: CpuId(0),
        ifindex: Some(2),
        session: SessionId(1),
        fragment_id: "udp_packet_meta_fragment".into(),
        kind: FactKind::PacketMeta(PacketMetaFact {
            netns: 1,
            sk_cookie: None,
            dir,
            local_port: None,
            remote_port: None,
            payload_byte0: Some(0x02),
            payload_byte1: Some(packet_type),
            payload_prefix2: Some(u16::from_be_bytes([0x02, packet_type])),
            payload_prefix4: None,
            payload_byte4: None,
            payload_byte5: None,
            payload_byte9: None,
            payload_byte10: None,
            payload_byte13: None,
            payload_bytes: BTreeMap::from([(1, packet_type)]),
            l3_proto: 0x0800,
            l4_proto: 89,
            tot_len: 64,
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

fn assert_no_stage(export: &ExportBundle, phase: &str) {
    assert!(
        !export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some(phase)),
        "unexpected stage {phase}"
    );
}

fn assert_no_protocol_ir(export: &ExportBundle, operation: &str) {
    assert!(
        !export
            .protocol_ir
            .iter()
            .any(|item| item.operation == operation),
        "unexpected protocol IR for {operation}"
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
