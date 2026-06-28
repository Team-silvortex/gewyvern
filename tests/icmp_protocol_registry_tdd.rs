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
use support::{route_fact, sock_lineage_fact};

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
fn icmp_packet_fact(id: u64, cookie: u64, dir: PacketDir, type_byte: u8) -> FactEnvelope {
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
            local_port: None,
            remote_port: None,
            payload_byte0: Some(type_byte),
            payload_byte1: Some(0),
            payload_prefix2: Some(u16::from_be_bytes([type_byte, 0])),
            payload_prefix4: None,
            payload_byte4: None,
            payload_byte5: None,
            payload_byte9: None,
            payload_byte10: None,
            payload_byte13: None,
            payload_bytes: BTreeMap::new(),
            l3_proto: 0x0800,
            l4_proto: 1,
            tot_len: 84,
            tcp_flags: 0,
            seq: None,
            ack: None,
            window: None,
        }),
    }
}

#[test]
fn icmp_registry_entries_resolve_to_packaged_paths() {
    assert_eq!(
        protocol_dsl_path("icmp", Some("echo")),
        Some(protocol_fixture_path("icmp/echo").to_string())
    );
    assert_eq!(
        protocol_dsl_path("ping", None),
        Some(protocol_fixture_path("icmp/echo").to_string())
    );
    assert_eq!(
        protocol_dsl_path("icmp", Some("port-unreachable")),
        Some(protocol_fixture_path("icmp/unreachable").to_string())
    );
}

#[test]
fn icmp_default_entry_is_echo_with_failure_surface_available() {
    assert_eq!(protocol_default_entry("icmp"), Some("echo".to_string()));

    let entries = protocol_entries("icmp").expect("icmp entries should resolve");
    assert!(entries.contains(&"echo".to_string()));
    assert!(entries.contains(&"unreachable".to_string()));
}

#[test]
fn icmp_surface_exposes_reachability_and_failure_shelves() {
    let echo = protocol_surface("icmp", "echo").expect("icmp echo surface should exist");
    assert_eq!(echo.shelf.expect("echo shelf should exist").key, "echo");
    assert_eq!(
        echo.entry_semantics
            .expect("echo semantics should exist")
            .category,
        "reachability-path"
    );

    let unreachable =
        protocol_surface("icmp", "unreachable").expect("icmp unreachable surface should exist");
    assert_eq!(
        unreachable
            .shelf
            .expect("unreachable shelf should exist")
            .key,
        "failure"
    );
    assert_eq!(
        unreachable
            .entry_semantics
            .expect("unreachable semantics should exist")
            .primary_failure_mode,
        Some("network_unreachable".to_string())
    );
}

#[test]
fn icmp_dsl_files_compile_into_expected_operations() {
    let echo = compile_file(&dsl_fixture_path("icmp_echo_path.gewy")).unwrap();
    assert_eq!(echo.template.id, "icmp_echo_path");
    assert_eq!(
        echo.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("icmp_echo".into())
    );

    let unreachable = compile_file(&dsl_fixture_path("icmp_unreachable_path.gewy")).unwrap();
    assert_eq!(unreachable.template.id, "icmp_unreachable_path");
    assert_eq!(
        unreachable
            .template
            .program_model
            .as_ref()
            .unwrap()
            .operation,
        ProgramOperation::Custom("icmp_unreachable".into())
    );
}

#[test]
fn icmp_echo_runtime_path_materializes_request_and_reply() {
    let binding = compile_file(&dsl_fixture_path("icmp_echo_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 9810, 0, "ping"));
    session.ingest(route_fact(2, 9810, 7));
    session.ingest(icmp_packet_fact(3, 9810, PacketDir::Egress, 8));
    session.ingest(icmp_packet_fact(4, 9810, PacketDir::Ingress, 0));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("icmp_echo".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_echo_request"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_echo_reply"))
    );
}

#[test]
fn icmp_unreachable_runtime_path_materializes_failure_signal() {
    let binding = compile_file(&dsl_fixture_path("icmp_unreachable_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 9811, 0, "probe"));
    session.ingest(route_fact(2, 9811, 7));
    session.ingest(icmp_packet_fact(3, 9811, PacketDir::Ingress, 3));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("icmp_unreachable".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_unreachable"))
    );
}
