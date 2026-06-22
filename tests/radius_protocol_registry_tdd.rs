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
fn radius_registry_entries_resolve_to_packaged_paths() {
    assert_eq!(
        protocol_dsl_path("radius", Some("challenge")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/radius/challenge".to_string())
    );
    assert_eq!(
        protocol_dsl_path("radius", Some("mfa")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/radius/challenge".to_string())
    );
    assert_eq!(
        protocol_dsl_path("radius", Some("denied")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/radius/denied".to_string())
    );
    assert_eq!(
        protocol_dsl_path("radius", Some("reject")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/radius/denied".to_string())
    );
}

#[test]
fn radius_default_entry_stays_access_while_surface_grows() {
    assert_eq!(protocol_default_entry("radius"), Some("access".to_string()));

    let entries = protocol_entries("radius").expect("radius entries should resolve");
    assert!(entries.contains(&"access".to_string()));
    assert!(entries.contains(&"challenge".to_string()));
    assert!(entries.contains(&"denied".to_string()));
}

#[test]
fn radius_surface_uses_split_shelves_per_entry() {
    for (entry, key) in [
        ("access", "access"),
        ("challenge", "challenge"),
        ("denied", "denied"),
    ] {
        let surface = protocol_surface("radius", entry).expect("radius surface should exist");
        let shelf = surface.shelf.expect("radius shelf should exist");
        assert_eq!(shelf.key, key);
    }
}

#[test]
fn radius_dsl_files_compile_into_expected_operations() {
    let challenge =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/radius_challenge_path.gewy").unwrap();
    assert_eq!(challenge.template.id, "radius_challenge_path");
    assert_eq!(
        challenge.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("radius_challenge".into())
    );

    let denied =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/radius_denied_path.gewy").unwrap();
    assert_eq!(denied.template.id, "radius_denied_path");
    assert_eq!(
        denied.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("radius_denied".into())
    );
}

#[test]
fn radius_challenge_runtime_path_materializes_request_and_challenge() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/radius_challenge_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 4825, 53000, "wpa_supplicant"));
    session.ingest(route_fact(2, 4825, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        4825,
        96,
        PacketDir::Egress,
        Some(53000),
        Some(1812),
        Some(0x01),
        None,
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        4,
        4825,
        96,
        PacketDir::Ingress,
        Some(53000),
        Some(1812),
        Some(0x0b),
        None,
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("radius_challenge".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_access_challenge"))
    );
}

#[test]
fn radius_denied_runtime_path_materializes_request_and_reject() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/radius_denied_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 4826, 53001, "wpa_supplicant"));
    session.ingest(route_fact(2, 4826, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        4826,
        96,
        PacketDir::Egress,
        Some(53001),
        Some(1812),
        Some(0x01),
        None,
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        4,
        4826,
        96,
        PacketDir::Ingress,
        Some(53001),
        Some(1812),
        Some(0x03),
        None,
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("radius_denied".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_access_reject"))
    );
}
