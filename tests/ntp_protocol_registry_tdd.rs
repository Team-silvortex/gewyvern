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
fn ntp_registry_entries_resolve_to_packaged_paths() {
    assert_eq!(
        protocol_dsl_path("ntp", Some("query")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/ntp/query".to_string())
    );
    assert_eq!(
        protocol_dsl_path("ntp", Some("probe")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/ntp/query".to_string())
    );
    assert_eq!(
        protocol_dsl_path("ntp", Some("sync")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/ntp/sync".to_string())
    );
    assert_eq!(
        protocol_dsl_path("ntp", Some("clock-sync")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/ntp/sync".to_string())
    );
}

#[test]
fn ntp_default_entry_stays_client_while_surface_grows() {
    assert_eq!(protocol_default_entry("ntp"), Some("client".to_string()));

    let entries = protocol_entries("ntp").expect("ntp entries should resolve");
    assert!(entries.contains(&"client".to_string()));
    assert!(entries.contains(&"query".to_string()));
    assert!(entries.contains(&"sync".to_string()));
}

#[test]
fn ntp_surface_keeps_generic_shelves_per_entry() {
    for (entry, key) in [("client", "client"), ("query", "query"), ("sync", "sync")] {
        let surface = protocol_surface("ntp", entry).expect("ntp surface should exist");
        let shelf = surface.shelf.expect("ntp shelf should exist");
        assert_eq!(shelf.key, key);
    }
}

#[test]
fn ntp_dsl_files_compile_into_expected_operations() {
    let query = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/ntp_query_path.gewy").unwrap();
    assert_eq!(query.template.id, "ntp_query_path");
    assert_eq!(
        query.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("ntp_query".into())
    );

    let sync = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/ntp_sync_path.gewy").unwrap();
    assert_eq!(sync.template.id, "ntp_sync_path");
    assert_eq!(
        sync.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("ntp_sync".into())
    );
}

#[test]
fn ntp_query_runtime_path_materializes_query_and_response() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/ntp_query_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 5813, 7000, "chrony-query"));
    session.ingest(route_fact(2, 5813, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        5813,
        90,
        PacketDir::Egress,
        Some(54020),
        Some(123),
        Some(0x23),
        Some(0x2300),
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        4,
        5813,
        90,
        PacketDir::Ingress,
        Some(54020),
        Some(123),
        Some(0x24),
        Some(0x2400),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("ntp_query".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_query"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_response"))
    );
}

#[test]
fn ntp_sync_runtime_path_rejects_query_byte_as_sync_request() {
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/ntp_sync_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 5814, 7001, "chronyd"));
    session.ingest(route_fact(2, 5814, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        5814,
        90,
        PacketDir::Egress,
        Some(54021),
        Some(123),
        Some(0x23),
        Some(0x2300),
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        4,
        5814,
        90,
        PacketDir::Ingress,
        Some(54021),
        Some(123),
        Some(0x24),
        Some(0x2400),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("send_sync_request"))
    );
}
