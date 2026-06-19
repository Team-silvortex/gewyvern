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
fn snmp_result_registry_aliases_resolve_to_packaged_paths() {
    assert_eq!(
        protocol_dsl_path("snmp", Some("report")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/snmp/report".to_string())
    );
    assert_eq!(
        protocol_dsl_path("snmp", Some("engine-report")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/snmp/report".to_string())
    );
    assert_eq!(
        protocol_dsl_path("snmp", Some("unauthorized")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/snmp/unauthorized".to_string())
    );
    assert_eq!(
        protocol_dsl_path("snmp", Some("auth-failed")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/snmp/unauthorized".to_string())
    );
}

#[test]
fn snmp_result_surface_uses_result_shelf() {
    for entry in ["report", "unauthorized"] {
        let surface = protocol_surface("snmp", entry).expect("snmp result surface should exist");
        let shelf = surface.shelf.expect("snmp result shelf should exist");
        assert_eq!(shelf.key, "result");
        assert_eq!(shelf.label, "Result");
    }
}

#[test]
fn snmp_result_dsl_files_compile_into_expected_operations() {
    let report =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/snmp_report_path.gewy").unwrap();
    assert_eq!(report.template.id, "snmp_report_path");
    assert_eq!(
        report.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("snmp_report".into())
    );

    let unauthorized =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/snmp_unauthorized_path.gewy")
            .unwrap();
    assert_eq!(unauthorized.template.id, "snmp_unauthorized_path");
    assert_eq!(
        unauthorized.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("snmp_unauthorized".into())
    );
}

#[test]
fn snmp_report_runtime_path_materializes_generic_report_response() {
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/snmp_report_path.gewy")
        .unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 2850, 54020, "snmpget"));
    session.ingest(route_fact(2, 2850, 7));
    session.ingest(snmp_v3_udp_packet_fact(
        3,
        2850,
        160,
        PacketDir::Ingress,
        54020,
        161,
        &[(0, 0x30), (1, 0x50), (2, 0x02), (3, 0x01), (4, 0x03), (13, 0xa8), (18, 0x04)],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("snmp_report".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_report_pdu"))
    );
}

#[test]
fn snmp_unauthorized_runtime_path_materializes_auth_failure_report() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/snmp_unauthorized_path.gewy")
            .unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 2851, 54021, "snmpget"));
    session.ingest(route_fact(2, 2851, 7));
    session.ingest(snmp_v3_udp_packet_fact(
        3,
        2851,
        168,
        PacketDir::Ingress,
        54021,
        161,
        &[(0, 0x30), (1, 0x58), (2, 0x02), (3, 0x01), (4, 0x03), (13, 0xa8), (18, 0x05)],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("snmp_unauthorized".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_authorization_failure_report"))
    );
}
