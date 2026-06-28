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
use std::collections::BTreeMap;
use std::time::{Duration, SystemTime};

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
fn ospf_packet_fact(id: u64, dir: PacketDir, packet_type: u8) -> FactEnvelope {
    FactEnvelope {
        id: FactId(id),
        ts: SystemTime::UNIX_EPOCH + Duration::from_millis(id * 10),
        cpu: CpuId(0),
        ifindex: Some(2),
        session: SessionId(1),
        fragment_id: "udp_packet_meta_fragment".into(),
        kind: FactKind::PacketMeta(PacketMetaFact {
            netns: 1,
            sk_cookie: None,
            dir,
            local_port: None,
            remote_port: None,
            payload_byte0: Some(0x02),
            payload_byte1: Some(packet_type),
            payload_prefix2: Some(u16::from_be_bytes([0x02, packet_type])),
            payload_prefix4: None,
            payload_byte4: None,
            payload_byte5: None,
            payload_byte9: None,
            payload_byte10: None,
            payload_byte13: None,
            payload_bytes: BTreeMap::from([(1, packet_type)]),
            l3_proto: 0x0800,
            l4_proto: 89,
            tot_len: 64,
            tcp_flags: 0,
            seq: None,
            ack: None,
            window: None,
        }),
    }
}

#[test]
fn ospf_registry_entries_resolve_to_packaged_paths() {
    assert_eq!(
        protocol_dsl_path("ospf", Some("hello")),
        Some(protocol_fixture_path("ospf/hello").to_string())
    );
    assert_eq!(
        protocol_dsl_path("ospf-hello", None),
        Some(protocol_fixture_path("ospf/hello").to_string())
    );
    assert_eq!(
        protocol_dsl_path("ospf", Some("database-description")),
        Some(protocol_fixture_path("ospf/dbdesc").to_string())
    );
}

#[test]
fn ospf_default_entry_is_hello_with_dbdesc_available() {
    assert_eq!(protocol_default_entry("ospf"), Some("hello".to_string()));

    let entries = protocol_entries("ospf").expect("ospf entries should resolve");
    assert!(entries.contains(&"hello".to_string()));
    assert!(entries.contains(&"dbdesc".to_string()));
}

#[test]
fn ospf_surface_exposes_neighbor_and_database_semantics() {
    let hello = protocol_surface("ospf", "hello").expect("ospf hello surface should exist");
    assert_eq!(
        hello.shelf.expect("hello shelf should exist").key,
        "neighbor"
    );
    assert_eq!(
        hello
            .entry_semantics
            .expect("hello semantics should exist")
            .category,
        "link-state-neighbor-discovery"
    );

    let dbdesc = protocol_surface("ospf", "dbdesc").expect("ospf dbdesc surface should exist");
    assert_eq!(
        dbdesc.shelf.expect("dbdesc shelf should exist").key,
        "database"
    );
    assert_eq!(
        dbdesc
            .entry_semantics
            .expect("dbdesc semantics should exist")
            .typical_signal,
        Some("OSPF packet type 2 Database Description".to_string())
    );
}

#[test]
fn ospf_dsl_files_compile_into_expected_operations() {
    let hello = compile_file(&dsl_fixture_path("ospf_hello_path.gewy")).unwrap();
    assert_eq!(hello.template.id, "ospf_hello_path");
    assert_eq!(
        hello.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("ospf_hello".into())
    );

    let dbdesc = compile_file(&dsl_fixture_path("ospf_dbdesc_path.gewy")).unwrap();
    assert_eq!(dbdesc.template.id, "ospf_dbdesc_path");
    assert_eq!(
        dbdesc.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("ospf_dbdesc".into())
    );
}

#[test]
fn ospf_hello_runtime_path_materializes_neighbor_stages() {
    let binding = compile_file(&dsl_fixture_path("ospf_hello_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(ospf_packet_fact(1, PacketDir::Egress, 1));
    session.ingest(ospf_packet_fact(2, PacketDir::Ingress, 1));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("ospf_hello".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_hello"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_hello"))
    );
}

#[test]
fn ospf_dbdesc_runtime_path_materializes_database_sync_stages() {
    let binding = compile_file(&dsl_fixture_path("ospf_dbdesc_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(ospf_packet_fact(1, PacketDir::Egress, 2));
    session.ingest(ospf_packet_fact(2, PacketDir::Ingress, 2));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("ospf_dbdesc".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_dbdesc"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_dbdesc"))
    );
}
