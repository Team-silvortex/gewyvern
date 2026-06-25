mod support;

use gewyvern::dsl::compile_file;
use gewyvern::flow::ProgramOperation;
use gewyvern::ledger::{FactEnvelope, PacketDir};
use gewyvern::protocol_profiles::{
    protocol_default_entry, protocol_dsl_path, protocol_entries, protocol_surface,
};
use gewyvern::runtime::{RuntimeSession, SessionConfig};
use std::time::{Duration, SystemTime};
use support::{packet_fact_with_dir_and_payload_bytes, route_fact, sock_lineage_fact};

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

#[test]
fn bgp_registry_entries_resolve_to_packaged_paths() {
    assert_eq!(
        protocol_dsl_path("bgp", Some("open")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/bgp/open".to_string())
    );
    assert_eq!(
        protocol_dsl_path("bgp-open", None),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/bgp/open".to_string())
    );
    assert_eq!(
        protocol_dsl_path("bgp", Some("keep-alive")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/bgp/keepalive".to_string())
    );
}

#[test]
fn bgp_default_entry_is_open_with_keepalive_available() {
    assert_eq!(protocol_default_entry("bgp"), Some("open".to_string()));

    let entries = protocol_entries("bgp").expect("bgp entries should resolve");
    assert!(entries.contains(&"open".to_string()));
    assert!(entries.contains(&"keepalive".to_string()));
}

#[test]
fn bgp_surface_exposes_routing_control_session_semantics() {
    let open = protocol_surface("bgp", "open").expect("bgp open surface should exist");
    assert_eq!(open.shelf.expect("open shelf should exist").key, "session");
    assert_eq!(
        open.entry_semantics
            .expect("open semantics should exist")
            .category,
        "routing-control-session"
    );

    let keepalive =
        protocol_surface("bgp", "keepalive").expect("bgp keepalive surface should exist");
    assert_eq!(
        keepalive
            .entry_semantics
            .expect("keepalive semantics should exist")
            .typical_signal,
        Some("BGP message type 4 KEEPALIVE".to_string())
    );
}

#[test]
fn bgp_dsl_files_compile_into_expected_operations() {
    let open = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/bgp_open_path.gewy").unwrap();
    assert_eq!(open.template.id, "bgp_open_path");
    assert_eq!(
        open.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("bgp_open".into())
    );

    let keepalive =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/bgp_keepalive_path.gewy").unwrap();
    assert_eq!(keepalive.template.id, "bgp_keepalive_path");
    assert_eq!(
        keepalive.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("bgp_keepalive".into())
    );
}

#[test]
fn bgp_open_runtime_path_materializes_peer_open_stages() {
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/bgp_open_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 9179, 179, "bgpd"));
    session.ingest(route_fact(2, 9179, 7));
    session.ingest(bgp_packet_fact(3, 9179, PacketDir::Egress, 1));
    session.ingest(bgp_packet_fact(4, 9179, PacketDir::Ingress, 1));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("bgp_open".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_open"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_open"))
    );
}

#[test]
fn bgp_keepalive_runtime_path_materializes_liveness_stages() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/bgp_keepalive_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 9180, 179, "bgpd"));
    session.ingest(bgp_packet_fact(2, 9180, PacketDir::Egress, 4));
    session.ingest(bgp_packet_fact(3, 9180, PacketDir::Ingress, 4));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("bgp_keepalive".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_keepalive"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_keepalive"))
    );
}
