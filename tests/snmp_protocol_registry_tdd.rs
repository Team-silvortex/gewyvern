mod support;

use gewyvern::dsl::compile_file;
use gewyvern::flow::ProgramOperation;
use gewyvern::ledger::{
    CpuId, FactEnvelope, FactId, FactKind, PacketDir, PacketMetaFact, SessionId,
};
use gewyvern::protocol_profiles::{
    protocol_default_entry, protocol_dsl_path, protocol_entries, protocol_surface,
};
use gewyvern::runtime::{RuntimeSession, SessionConfig};
use std::time::{Duration, SystemTime};
use support::{
    route_fact, sock_lineage_fact,
    udp_packet_fact_with_dir_and_ports_and_payload_prefix4_and_byte13,
};

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
fn snmp_registry_entries_resolve_to_packaged_paths() {
    assert_eq!(
        protocol_dsl_path("snmp", Some("bulk")),
        Some(protocol_fixture_path("snmp/bulk").to_string())
    );
    assert_eq!(
        protocol_dsl_path("snmp", Some("bulk-walk")),
        Some(protocol_fixture_path("snmp/bulk").to_string())
    );
    assert_eq!(
        protocol_dsl_path("snmp", Some("get-next")),
        Some(protocol_fixture_path("snmp/get-next").to_string())
    );
    assert_eq!(
        protocol_dsl_path("snmp", Some("walk")),
        Some(protocol_fixture_path("snmp/get-next").to_string())
    );
    assert_eq!(
        protocol_dsl_path("snmp", Some("set")),
        Some(protocol_fixture_path("snmp/set").to_string())
    );
    assert_eq!(
        protocol_dsl_path("snmp", Some("write")),
        Some(protocol_fixture_path("snmp/set").to_string())
    );
    assert_eq!(
        protocol_dsl_path("snmp", Some("trap")),
        Some(protocol_fixture_path("snmp/trap").to_string())
    );
    assert_eq!(
        protocol_dsl_path("snmp", Some("notify")),
        Some(protocol_fixture_path("snmp/trap").to_string())
    );
    assert_eq!(
        protocol_dsl_path("snmp", Some("inform")),
        Some(protocol_fixture_path("snmp/inform").to_string())
    );
    assert_eq!(
        protocol_dsl_path("snmp", Some("ack-notify")),
        Some(protocol_fixture_path("snmp/inform").to_string())
    );
    assert_eq!(
        protocol_dsl_path("snmp", Some("v3-auth")),
        Some(protocol_fixture_path("snmp/v3-auth").to_string())
    );
    assert_eq!(
        protocol_dsl_path("snmp", Some("auth-user")),
        Some(protocol_fixture_path("snmp/v3-auth").to_string())
    );
    assert_eq!(
        protocol_dsl_path("snmp", Some("v3-priv")),
        Some(protocol_fixture_path("snmp/v3-priv").to_string())
    );
    assert_eq!(
        protocol_dsl_path("snmp", Some("encrypted-session")),
        Some(protocol_fixture_path("snmp/v3-priv").to_string())
    );
}

#[test]
fn snmp_default_entry_stays_get_while_surface_grows() {
    assert_eq!(protocol_default_entry("snmp"), Some("get".to_string()));

    let entries = protocol_entries("snmp").expect("snmp entries should resolve");
    assert!(entries.contains(&"bulk".to_string()));
    assert!(entries.contains(&"get".to_string()));
    assert!(entries.contains(&"get-next".to_string()));
    assert!(entries.contains(&"inform".to_string()));
    assert!(entries.contains(&"set".to_string()));
    assert!(entries.contains(&"trap".to_string()));
    assert!(entries.contains(&"v3-auth".to_string()));
    assert!(entries.contains(&"v3-priv".to_string()));
}

#[test]
fn snmp_surface_keeps_generic_shelves_per_entry() {
    for (entry, key) in [
        ("bulk", "read"),
        ("get", "read"),
        ("get-next", "read"),
        ("inform", "notify"),
        ("set", "set"),
        ("trap", "notify"),
        ("v3-auth", "security"),
        ("v3-priv", "security"),
    ] {
        let surface = protocol_surface("snmp", entry).expect("snmp surface should exist");
        let shelf = surface.shelf.expect("snmp shelf should exist");
        assert_eq!(shelf.key, key);
    }
}

#[test]
fn snmp_dsl_files_compile_into_expected_operations() {
    for (path, template_id, operation) in [
        (
            dsl_fixture_path("snmp_bulk_path.gewy"),
            "snmp_bulk_path",
            "snmp_bulk",
        ),
        (
            dsl_fixture_path("snmp_get_next_path.gewy"),
            "snmp_get_next_path",
            "snmp_get_next",
        ),
        (
            dsl_fixture_path("snmp_set_path.gewy"),
            "snmp_set_path",
            "snmp_set",
        ),
        (
            dsl_fixture_path("snmp_trap_path.gewy"),
            "snmp_trap_path",
            "snmp_trap",
        ),
        (
            dsl_fixture_path("snmp_inform_path.gewy"),
            "snmp_inform_path",
            "snmp_inform",
        ),
        (
            dsl_fixture_path("snmp_v3_auth_path.gewy"),
            "snmp_v3_auth_path",
            "snmp_v3_auth",
        ),
        (
            dsl_fixture_path("snmp_v3_priv_path.gewy"),
            "snmp_v3_priv_path",
            "snmp_v3_priv",
        ),
    ] {
        let compiled = compile_file(&path).unwrap();
        assert_eq!(compiled.template.id, template_id);
        assert_eq!(
            compiled.template.program_model.as_ref().unwrap().operation,
            ProgramOperation::Custom(operation.into())
        );
    }
}

#[test]
fn snmp_bulk_runtime_path_materializes_request_and_response_datagrams() {
    let binding = compile_file(&dsl_fixture_path("snmp_bulk_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 2828, 54000, "snmpbulkget"));
    session.ingest(route_fact(2, 2828, 7));
    session.ingest(
        udp_packet_fact_with_dir_and_ports_and_payload_prefix4_and_byte13(
            3,
            2828,
            108,
            PacketDir::Egress,
            Some(54000),
            Some(161),
            Some(0x30),
            Some(0x302c),
            Some(0x302c0201),
            Some(0xa5),
        ),
    );
    session.ingest(
        udp_packet_fact_with_dir_and_ports_and_payload_prefix4_and_byte13(
            4,
            2828,
            128,
            PacketDir::Ingress,
            Some(54000),
            Some(161),
            Some(0x30),
            Some(0x3040),
            Some(0x30400201),
            Some(0xa2),
        ),
    );
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("snmp_bulk".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_bulk_request"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_bulk_response"))
    );
}

#[test]
fn snmp_get_next_runtime_path_materializes_request_and_response_datagrams() {
    let binding = compile_file(&dsl_fixture_path("snmp_get_next_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 2829, 54001, "snmpwalk"));
    session.ingest(route_fact(2, 2829, 7));
    session.ingest(
        udp_packet_fact_with_dir_and_ports_and_payload_prefix4_and_byte13(
            3,
            2829,
            96,
            PacketDir::Egress,
            Some(54001),
            Some(161),
            Some(0x30),
            Some(0x3026),
            Some(0x30260201),
            Some(0xa1),
        ),
    );
    session.ingest(
        udp_packet_fact_with_dir_and_ports_and_payload_prefix4_and_byte13(
            4,
            2829,
            104,
            PacketDir::Ingress,
            Some(54001),
            Some(161),
            Some(0x30),
            Some(0x3028),
            Some(0x30280201),
            Some(0xa2),
        ),
    );
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("snmp_get_next".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_get_next_request"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_get_next_response"))
    );
}

#[test]
fn snmp_set_runtime_path_rejects_wrong_response_pdu_type() {
    let binding = compile_file(&dsl_fixture_path("snmp_set_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 2830, 54002, "snmpset"));
    session.ingest(route_fact(2, 2830, 7));
    session.ingest(
        udp_packet_fact_with_dir_and_ports_and_payload_prefix4_and_byte13(
            3,
            2830,
            96,
            PacketDir::Egress,
            Some(54002),
            Some(161),
            Some(0x30),
            Some(0x3026),
            Some(0x30260201),
            Some(0xa3),
        ),
    );
    session.ingest(
        udp_packet_fact_with_dir_and_ports_and_payload_prefix4_and_byte13(
            4,
            2830,
            104,
            PacketDir::Ingress,
            Some(54002),
            Some(161),
            Some(0x30),
            Some(0x3028),
            Some(0x30280201),
            Some(0xa1),
        ),
    );
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_set_response"))
    );
}

#[test]
fn snmp_trap_runtime_path_materializes_one_way_notification_datagram() {
    let binding = compile_file(&dsl_fixture_path("snmp_trap_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 2831, 54003, "snmptrap"));
    session.ingest(route_fact(2, 2831, 7));
    session.ingest(
        udp_packet_fact_with_dir_and_ports_and_payload_prefix4_and_byte13(
            3,
            2831,
            112,
            PacketDir::Egress,
            Some(54003),
            Some(162),
            Some(0x30),
            Some(0x3030),
            Some(0x30300201),
            Some(0xa7),
        ),
    );
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("snmp_trap".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_trap_notification"))
    );
}

#[test]
fn snmp_inform_runtime_path_materializes_notification_and_acknowledgement() {
    let binding = compile_file(&dsl_fixture_path("snmp_inform_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 2832, 54004, "snmpinform"));
    session.ingest(route_fact(2, 2832, 7));
    session.ingest(
        udp_packet_fact_with_dir_and_ports_and_payload_prefix4_and_byte13(
            3,
            2832,
            112,
            PacketDir::Egress,
            Some(54004),
            Some(161),
            Some(0x30),
            Some(0x3030),
            Some(0x30300201),
            Some(0xa6),
        ),
    );
    session.ingest(
        udp_packet_fact_with_dir_and_ports_and_payload_prefix4_and_byte13(
            4,
            2832,
            120,
            PacketDir::Ingress,
            Some(54004),
            Some(161),
            Some(0x30),
            Some(0x3032),
            Some(0x30320201),
            Some(0xa2),
        ),
    );
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("snmp_inform".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_inform_notification"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_inform_response"))
    );
}

#[test]
fn snmp_v3_auth_runtime_path_materializes_authenticated_exchange() {
    let binding = compile_file(&dsl_fixture_path("snmp_v3_auth_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 2833, 54005, "snmpget"));
    session.ingest(route_fact(2, 2833, 7));
    session.ingest(snmp_v3_udp_packet_fact(
        3,
        2833,
        144,
        PacketDir::Egress,
        54005,
        161,
        &[
            (0, 0x30),
            (1, 0x40),
            (2, 0x02),
            (3, 0x01),
            (4, 0x03),
            (18, 0x01),
        ],
    ));
    session.ingest(snmp_v3_udp_packet_fact(
        4,
        2833,
        160,
        PacketDir::Ingress,
        54005,
        161,
        &[
            (0, 0x30),
            (1, 0x50),
            (2, 0x02),
            (3, 0x01),
            (4, 0x03),
            (18, 0x01),
        ],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("snmp_v3_auth".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_v3_auth_request"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_v3_auth_response"))
    );
}

#[test]
fn snmp_v3_priv_runtime_path_materializes_privacy_protected_exchange() {
    let binding = compile_file(&dsl_fixture_path("snmp_v3_priv_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 2834, 54006, "snmpget"));
    session.ingest(route_fact(2, 2834, 7));
    session.ingest(snmp_v3_udp_packet_fact(
        3,
        2834,
        176,
        PacketDir::Egress,
        54006,
        161,
        &[
            (0, 0x30),
            (1, 0x60),
            (2, 0x02),
            (3, 0x01),
            (4, 0x03),
            (18, 0x03),
        ],
    ));
    session.ingest(snmp_v3_udp_packet_fact(
        4,
        2834,
        192,
        PacketDir::Ingress,
        54006,
        161,
        &[
            (0, 0x30),
            (1, 0x70),
            (2, 0x02),
            (3, 0x01),
            (4, 0x03),
            (18, 0x03),
        ],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("snmp_v3_priv".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_v3_priv_request"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_v3_priv_response"))
    );
}
