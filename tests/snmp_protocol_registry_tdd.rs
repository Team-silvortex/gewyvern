mod support;

use gewyvern::dsl::compile_file;
use gewyvern::flow::ProgramOperation;
use gewyvern::ledger::PacketDir;
use gewyvern::protocol_profiles::{
    protocol_default_entry, protocol_dsl_path, protocol_entries, protocol_surface,
};
use gewyvern::runtime::{RuntimeSession, SessionConfig};
use std::time::{Duration, SystemTime};
use support::{
    route_fact, sock_lineage_fact,
    udp_packet_fact_with_dir_and_ports_and_payload_prefix4_and_byte13,
};

#[test]
fn snmp_registry_entries_resolve_to_packaged_paths() {
    assert_eq!(
        protocol_dsl_path("snmp", Some("get-next")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/snmp/get-next".to_string())
    );
    assert_eq!(
        protocol_dsl_path("snmp", Some("walk")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/snmp/get-next".to_string())
    );
    assert_eq!(
        protocol_dsl_path("snmp", Some("set")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/snmp/set".to_string())
    );
    assert_eq!(
        protocol_dsl_path("snmp", Some("write")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/snmp/set".to_string())
    );
}

#[test]
fn snmp_default_entry_stays_get_while_surface_grows() {
    assert_eq!(protocol_default_entry("snmp"), Some("get".to_string()));

    let entries = protocol_entries("snmp").expect("snmp entries should resolve");
    assert!(entries.contains(&"get".to_string()));
    assert!(entries.contains(&"get-next".to_string()));
    assert!(entries.contains(&"set".to_string()));
}

#[test]
fn snmp_surface_keeps_generic_shelves_per_entry() {
    for (entry, key) in [("get", "get"), ("get-next", "get-next"), ("set", "set")] {
        let surface = protocol_surface("snmp", entry).expect("snmp surface should exist");
        let shelf = surface.shelf.expect("snmp shelf should exist");
        assert_eq!(shelf.key, key);
    }
}

#[test]
fn snmp_dsl_files_compile_into_expected_operations() {
    let get_next =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/snmp_get_next_path.gewy").unwrap();
    assert_eq!(get_next.template.id, "snmp_get_next_path");
    assert_eq!(
        get_next.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("snmp_get_next".into())
    );

    let set = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/snmp_set_path.gewy").unwrap();
    assert_eq!(set.template.id, "snmp_set_path");
    assert_eq!(
        set.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("snmp_set".into())
    );
}

#[test]
fn snmp_get_next_runtime_path_materializes_request_and_response_datagrams() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/snmp_get_next_path.gewy").unwrap();
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
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/snmp_set_path.gewy").unwrap();
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
