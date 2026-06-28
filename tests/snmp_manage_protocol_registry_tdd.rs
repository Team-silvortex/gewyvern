mod support;

use gewyvern::dsl::compile_file;
use gewyvern::flow::ProgramOperation;
use gewyvern::ledger::{
    CpuId, FactEnvelope, FactId, FactKind, PacketDir, PacketMetaFact, SessionId,
};
use gewyvern::protocol_profiles::{protocol_dsl_path, protocol_surface};
use gewyvern::runtime::{RuntimeSession, SessionConfig};
use std::time::{Duration, SystemTime};
use support::{route_fact, sock_lineage_fact};

fn dsl_fixture_path(name: &str) -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("dsl")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

fn protocol_fixture_path(relative: &str) -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("protocols")
        .join(relative)
        .to_string_lossy()
        .into_owned()
}
fn snmp_v3_udp_packet_fact(
    id: u64,
    cookie: u64,
    tot_len: u32,
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
            payload_byte1: byte_at(1),
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
            tot_len,
            tcp_flags: 0,
            seq: None,
            ack: None,
            window: None,
        }),
    }
}

#[test]
fn snmp_manage_registry_aliases_resolve_to_packaged_paths() {
    assert_eq!(
        protocol_dsl_path("snmp", Some("engine-sync")),
        Some(protocol_fixture_path("snmp/engine-sync").to_string())
    );
    assert_eq!(
        protocol_dsl_path("snmp", Some("engine-discovery")),
        Some(protocol_fixture_path("snmp/engine-sync").to_string())
    );
    assert_eq!(
        protocol_dsl_path("snmp", Some("trap-recv")),
        Some(protocol_fixture_path("snmp/trap-recv").to_string())
    );
    assert_eq!(
        protocol_dsl_path("snmp", Some("trap-listener")),
        Some(protocol_fixture_path("snmp/trap-recv").to_string())
    );
}

#[test]
fn snmp_manage_surface_uses_manage_shelf() {
    for entry in ["engine-sync", "trap-recv"] {
        let surface = protocol_surface("snmp", entry).expect("snmp manage surface should exist");
        let shelf = surface.shelf.expect("snmp manage shelf should exist");
        assert_eq!(shelf.key, "manage");
        assert_eq!(shelf.label, "Manage");
    }
}

#[test]
fn snmp_manage_dsl_files_compile_into_expected_operations() {
    let engine_sync = compile_file(&dsl_fixture_path("snmp_engine_sync_path.gewy")).unwrap();
    assert_eq!(engine_sync.template.id, "snmp_engine_sync_path");
    assert_eq!(
        engine_sync
            .template
            .program_model
            .as_ref()
            .unwrap()
            .operation,
        ProgramOperation::Custom("snmp_engine_sync".into())
    );

    let trap_recv = compile_file(&dsl_fixture_path("snmp_trap_recv_path.gewy")).unwrap();
    assert_eq!(trap_recv.template.id, "snmp_trap_recv_path");
    assert_eq!(
        trap_recv.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("snmp_trap_recv".into())
    );
}

#[test]
fn snmp_engine_sync_runtime_path_materializes_probe_and_report() {
    let binding = compile_file(&dsl_fixture_path("snmp_engine_sync_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 2840, 54010, "snmpget"));
    session.ingest(route_fact(2, 2840, 7));
    session.ingest(snmp_v3_udp_packet_fact(
        3,
        2840,
        144,
        PacketDir::Egress,
        54010,
        161,
        &[
            (0, 0x30),
            (1, 0x40),
            (2, 0x02),
            (3, 0x01),
            (4, 0x03),
            (13, 0xa0),
            (18, 0x04),
        ],
    ));
    session.ingest(snmp_v3_udp_packet_fact(
        4,
        2840,
        160,
        PacketDir::Ingress,
        54010,
        161,
        &[
            (0, 0x30),
            (1, 0x50),
            (2, 0x02),
            (3, 0x01),
            (4, 0x03),
            (13, 0xa8),
            (18, 0x04),
        ],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("snmp_engine_sync".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_engine_sync_probe"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_engine_sync_report"))
    );
}

#[test]
fn snmp_trap_recv_runtime_path_materializes_receive_only_notification() {
    let binding = compile_file(&dsl_fixture_path("snmp_trap_recv_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 2841, 162, "snmptrapd"));
    session.ingest(route_fact(2, 2841, 7));
    session.ingest(snmp_v3_udp_packet_fact(
        3,
        2841,
        120,
        PacketDir::Ingress,
        162,
        161,
        &[(0, 0x30), (1, 0x32), (2, 0x02), (3, 0x01), (13, 0xa7)],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("snmp_trap_recv".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_trap_notification"))
    );
}
