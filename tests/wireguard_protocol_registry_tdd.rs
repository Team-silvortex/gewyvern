mod support;

use gewyvern::dsl::compile_file;
use gewyvern::flow::ProgramOperation;
use gewyvern::ledger::PacketDir;
use gewyvern::protocol_profiles::{
    protocol_default_entry, protocol_dsl_path, protocol_entries, protocol_surface,
};
use gewyvern::runtime::{RuntimeSession, SessionConfig};
use std::time::{Duration, SystemTime};
use support::{route_fact, sock_lineage_fact, udp_packet_fact_with_dir_and_ports_and_payload};

#[test]
fn wireguard_registry_entries_resolve_to_packaged_paths() {
    assert_eq!(
        protocol_dsl_path("wireguard", Some("cookie-reply")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/wireguard/cookie".to_string())
    );
    assert_eq!(
        protocol_dsl_path("wireguard", Some("data")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/wireguard/transport".to_string())
    );
}

#[test]
fn wireguard_default_entry_stays_handshake_while_surface_grows() {
    assert_eq!(
        protocol_default_entry("wireguard"),
        Some("handshake".to_string())
    );

    let entries = protocol_entries("wireguard").expect("wireguard entries should resolve");
    assert!(entries.contains(&"handshake".to_string()));
    assert!(entries.contains(&"cookie".to_string()));
    assert!(entries.contains(&"transport".to_string()));
}

#[test]
fn wireguard_surface_uses_split_shelves_per_entry() {
    for (entry, key) in [
        ("handshake", "handshake"),
        ("cookie", "cookie"),
        ("transport", "transport"),
    ] {
        let surface = protocol_surface("wireguard", entry).expect("wireguard surface should exist");
        let shelf = surface.shelf.expect("wireguard shelf should exist");
        assert_eq!(shelf.key, key);
    }
}

#[test]
fn wireguard_dsl_files_compile_into_expected_operations() {
    let cookie =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/wireguard_cookie_path.gewy").unwrap();
    assert_eq!(cookie.template.id, "wireguard_cookie_path");
    assert_eq!(
        cookie.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("wireguard_cookie".into())
    );

    let transport = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/wireguard_transport_path.gewy")
        .unwrap();
    assert_eq!(transport.template.id, "wireguard_transport_path");
    assert_eq!(
        transport.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("wireguard_transport".into())
    );
}

#[test]
fn wireguard_cookie_runtime_path_materializes_initiation_and_cookie_reply() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/wireguard_cookie_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 5801, 51820, "wg"));
    session.ingest(route_fact(2, 5801, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        5801,
        148,
        PacketDir::Egress,
        Some(51820),
        Some(51820),
        Some(0x01),
        None,
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        4,
        5801,
        92,
        PacketDir::Ingress,
        Some(51820),
        Some(51820),
        Some(0x03),
        None,
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("wireguard_cookie".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_cookie_reply"))
    );
}

#[test]
fn wireguard_transport_runtime_path_materializes_bidirectional_data() {
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/wireguard_transport_path.gewy")
        .unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 5802, 51820, "wg"));
    session.ingest(route_fact(2, 5802, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        5802,
        120,
        PacketDir::Egress,
        Some(51820),
        Some(51820),
        Some(0x04),
        None,
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        4,
        5802,
        120,
        PacketDir::Ingress,
        Some(51820),
        Some(51820),
        Some(0x04),
        None,
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("wireguard_transport".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_transport_data"))
    );
}
