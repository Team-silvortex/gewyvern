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
#[test]
fn stun_registry_entries_resolve_to_packaged_paths() {
    assert_eq!(
        protocol_dsl_path("stun", Some("allocate")),
        Some(protocol_fixture_path("stun/allocate").to_string())
    );
    assert_eq!(
        protocol_dsl_path("stun", Some("relay")),
        Some(protocol_fixture_path("stun/allocate").to_string())
    );
    assert_eq!(
        protocol_dsl_path("stun", Some("refresh")),
        Some(protocol_fixture_path("stun/refresh").to_string())
    );
    assert_eq!(
        protocol_dsl_path("stun", Some("keepalive")),
        Some(protocol_fixture_path("stun/refresh").to_string())
    );
}

#[test]
fn stun_default_entry_stays_binding_while_surface_grows() {
    assert_eq!(protocol_default_entry("stun"), Some("binding".to_string()));

    let entries = protocol_entries("stun").expect("stun entries should resolve");
    assert!(entries.contains(&"binding".to_string()));
    assert!(entries.contains(&"allocate".to_string()));
    assert!(entries.contains(&"refresh".to_string()));
}

#[test]
fn stun_surface_keeps_generic_shelves_per_entry() {
    for (entry, key) in [
        ("binding", "binding"),
        ("allocate", "relay"),
        ("refresh", "relay"),
    ] {
        let surface = protocol_surface("stun", entry).expect("stun surface should exist");
        let shelf = surface.shelf.expect("stun shelf should exist");
        assert_eq!(shelf.key, key);
    }
}

#[test]
fn stun_dsl_files_compile_into_expected_operations() {
    let allocate = compile_file(&dsl_fixture_path("stun_allocate_path.gewy")).unwrap();
    assert_eq!(allocate.template.id, "stun_allocate_path");
    assert_eq!(
        allocate.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("stun_allocate".into())
    );

    let refresh = compile_file(&dsl_fixture_path("stun_refresh_path.gewy")).unwrap();
    assert_eq!(refresh.template.id, "stun_refresh_path");
    assert_eq!(
        refresh.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("stun_refresh".into())
    );
}

#[test]
fn stun_allocate_runtime_path_materializes_request_and_response_datagrams() {
    let binding = compile_file(&dsl_fixture_path("stun_allocate_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 3809, 5001, "turn-client"));
    session.ingest(route_fact(2, 3809, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        3809,
        120,
        PacketDir::Egress,
        Some(54010),
        Some(3478),
        Some(0x00),
        Some(0x0003),
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        4,
        3809,
        140,
        PacketDir::Ingress,
        Some(54010),
        Some(3478),
        Some(0x01),
        Some(0x0103),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("stun_allocate".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_allocate_request"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_allocate_response"))
    );
}

#[test]
fn stun_refresh_runtime_path_rejects_wrong_response_type() {
    let binding = compile_file(&dsl_fixture_path("stun_refresh_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 3810, 5002, "turn-client"));
    session.ingest(route_fact(2, 3810, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        3810,
        120,
        PacketDir::Egress,
        Some(54011),
        Some(3478),
        Some(0x00),
        Some(0x0004),
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        4,
        3810,
        140,
        PacketDir::Ingress,
        Some(54011),
        Some(3478),
        Some(0x01),
        Some(0x0101),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_refresh_response"))
    );
}
