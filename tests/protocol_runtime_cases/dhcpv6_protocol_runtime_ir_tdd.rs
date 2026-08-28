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
use support::{route_fact, sock_lineage_fact};

fn dsl_fixture_path(name: &str) -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("dsl")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

#[test]
fn dhcpv6_solicit_runtime_path_materializes_ipv6_lease_ir() {
    let export = run_dhcpv6_path(
        "dhcpv6_solicit_path.gewy",
        0xd601,
        &[(PacketDir::Egress, 0x01), (PacketDir::Ingress, 0x02)],
    );

    assert_operation(&export, "dhcpv6_solicit");
    assert_stage(&export, "send_solicit");
    assert_stage(&export, "receive_advertise");

    let ir = protocol_ir(&export, "dhcpv6_solicit");
    assert_surface(
        ir,
        "dhcpv6",
        "solicit",
        "lease",
        "network-control-discovery",
    );
    assert_eq!(
        ir.semantics_category.as_deref(),
        Some("ipv6-lease-discovery-path")
    );
    assert_json_replay(&export);
}

#[test]
fn dhcpv6_request_runtime_path_materializes_reply_ir() {
    let export = run_dhcpv6_path(
        "dhcpv6_request_path.gewy",
        0xd602,
        &[(PacketDir::Egress, 0x03), (PacketDir::Ingress, 0x07)],
    );

    assert_operation(&export, "dhcpv6_request");
    assert_stage(&export, "send_request");
    assert_stage(&export, "receive_reply");

    let ir = protocol_ir(&export, "dhcpv6_request");
    assert_surface(
        ir,
        "dhcpv6",
        "request",
        "lease",
        "network-control-discovery",
    );
    assert_eq!(
        ir.semantics_category.as_deref(),
        Some("ipv6-lease-request-path")
    );
    assert_json_replay(&export);
}

#[test]
fn dhcpv6_request_runtime_ir_does_not_materialize_for_advertise() {
    let export = run_dhcpv6_path(
        "dhcpv6_request_path.gewy",
        0xd603,
        &[(PacketDir::Egress, 0x03), (PacketDir::Ingress, 0x02)],
    );

    assert_no_stage(&export, "receive_reply");
    assert_no_protocol_ir(&export, "dhcpv6_request");
    assert_json_replay(&export);
}

#[test]
fn dhcpv6_release_runtime_path_materializes_lifecycle_ir() {
    let export = run_dhcpv6_path(
        "dhcpv6_release_path.gewy",
        0xd604,
        &[(PacketDir::Egress, 0x08)],
    );

    assert_operation(&export, "dhcpv6_release");
    assert_stage(&export, "send_release");

    let ir = protocol_ir(&export, "dhcpv6_release");
    assert_surface(
        ir,
        "dhcpv6",
        "release",
        "lifecycle",
        "network-control-discovery",
    );
    assert_eq!(
        ir.semantics_category.as_deref(),
        Some("ipv6-lease-release-path")
    );
    assert_json_replay(&export);
}

fn run_dhcpv6_path(fixture: &str, cookie: u64, packets: &[(PacketDir, u8)]) -> ExportBundle {
    let binding = compile_file(&dsl_fixture_path(fixture)).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, cookie, 8701, "dhclient"));
    session.ingest(route_fact(2, cookie, 7));
    for (index, (dir, byte0)) in packets.iter().enumerate() {
        session.ingest(udp_payload_fact(3 + index as u64, cookie, *dir, *byte0));
    }
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(100));
    session.export_bundle()
}

fn udp_payload_fact(id: u64, cookie: u64, dir: PacketDir, byte0: u8) -> FactEnvelope {
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
            local_port: Some(546),
            remote_port: Some(547),
            payload_byte0: Some(byte0),
            payload_byte1: None,
            payload_prefix2: None,
            payload_prefix4: None,
            payload_byte4: None,
            payload_byte5: None,
            payload_byte9: None,
            payload_byte10: None,
            payload_byte13: None,
            payload_bytes: BTreeMap::from([(0, byte0)]),
            l3_proto: 0x86dd,
            l4_proto: 17,
            tot_len: 180,
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
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some(phase)),
        "unexpected stage {phase}"
    );
}

fn assert_json_replay(export: &ExportBundle) {
    let replayed = ExportBundle::from_json(&export.to_json()).expect("export json should replay");
    assert_eq!(replayed.protocol_ir, export.protocol_ir);
}

fn assert_no_protocol_ir(export: &ExportBundle, operation: &str) {
    assert!(
        export
            .protocol_ir
            .iter()
            .all(|item| item.operation != operation),
        "unexpected protocol IR for {operation}"
    );
}

fn protocol_ir<'a>(export: &'a ExportBundle, operation: &str) -> &'a ProtocolIr {
    export
        .protocol_ir
        .iter()
        .find(|item| item.operation == operation)
        .expect("protocol IR should materialize")
}
