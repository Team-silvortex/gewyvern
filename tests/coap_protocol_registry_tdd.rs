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
fn coap_registry_entries_resolve_to_packaged_paths() {
    assert_eq!(
        protocol_dsl_path("coap", Some("post")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/coap/post".to_string())
    );
    assert_eq!(
        protocol_dsl_path("coap", Some("write")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/coap/post".to_string())
    );
    assert_eq!(
        protocol_dsl_path("coap", Some("put")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/coap/put".to_string())
    );
    assert_eq!(
        protocol_dsl_path("coap", Some("update")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/coap/put".to_string())
    );
    assert_eq!(
        protocol_dsl_path("coap", Some("delete")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/coap/delete".to_string())
    );
    assert_eq!(
        protocol_dsl_path("coap", Some("remove")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/coap/delete".to_string())
    );
}

#[test]
fn coap_default_entry_stays_get_while_surface_grows() {
    assert_eq!(protocol_default_entry("coap"), Some("get".to_string()));

    let entries = protocol_entries("coap").expect("coap entries should resolve");
    assert!(entries.contains(&"get".to_string()));
    assert!(entries.contains(&"post".to_string()));
    assert!(entries.contains(&"put".to_string()));
    assert!(entries.contains(&"delete".to_string()));
}

#[test]
fn coap_surface_keeps_generic_shelves_per_entry() {
    for (entry, key) in [
        ("get", "get"),
        ("post", "write"),
        ("put", "write"),
        ("delete", "write"),
    ] {
        let surface = protocol_surface("coap", entry).expect("coap surface should exist");
        let shelf = surface.shelf.expect("coap shelf should exist");
        assert_eq!(shelf.key, key);
    }
}

#[test]
fn coap_dsl_files_compile_into_expected_operations() {
    let post = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/coap_post_path.gewy").unwrap();
    assert_eq!(post.template.id, "coap_post_path");
    assert_eq!(
        post.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("coap_post".into())
    );

    let put = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/coap_put_path.gewy").unwrap();
    assert_eq!(put.template.id, "coap_put_path");
    assert_eq!(
        put.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("coap_put".into())
    );

    let delete =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/coap_delete_path.gewy").unwrap();
    assert_eq!(delete.template.id, "coap_delete_path");
    assert_eq!(
        delete.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("coap_delete".into())
    );
}

#[test]
fn coap_post_runtime_path_materializes_send_and_created_response() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/coap_post_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 1811, 6100, "coap-client"));
    session.ingest(route_fact(2, 1811, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        1811,
        64,
        PacketDir::Egress,
        Some(56001),
        Some(5683),
        Some(0x40),
        Some(0x4002),
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        4,
        1811,
        80,
        PacketDir::Ingress,
        Some(56001),
        Some(5683),
        Some(0x60),
        Some(0x6041),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("coap_post".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_request"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_created"))
    );
}

#[test]
fn coap_delete_runtime_path_rejects_wrong_response_code() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/coap_delete_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 1812, 6101, "coap-client"));
    session.ingest(route_fact(2, 1812, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        1812,
        64,
        PacketDir::Egress,
        Some(56002),
        Some(5683),
        Some(0x40),
        Some(0x4004),
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        4,
        1812,
        80,
        PacketDir::Ingress,
        Some(56002),
        Some(5683),
        Some(0x60),
        Some(0x6045),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_deleted"))
    );
}
