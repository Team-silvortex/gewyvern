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

fn icmpv6_packet_fact(id: u64, cookie: u64, dir: PacketDir, type_byte: u8) -> FactEnvelope {
    packet_fact(id, Some(cookie), dir, type_byte)
}

fn ndp_packet_fact(id: u64, dir: PacketDir, type_byte: u8) -> FactEnvelope {
    packet_fact(id, None, dir, type_byte)
}

fn packet_fact(id: u64, cookie: Option<u64>, dir: PacketDir, type_byte: u8) -> FactEnvelope {
    FactEnvelope {
        id: FactId(id),
        ts: SystemTime::UNIX_EPOCH + Duration::from_millis(id * 10),
        cpu: CpuId(0),
        ifindex: Some(2),
        session: SessionId(1),
        fragment_id: "udp_packet_meta_fragment".into(),
        kind: FactKind::PacketMeta(PacketMetaFact {
            netns: 1,
            sk_cookie: cookie,
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
            l3_proto: 0x86dd,
            l4_proto: 58,
            tot_len: 96,
            tcp_flags: 0,
            seq: None,
            ack: None,
            window: None,
        }),
    }
}

#[test]
fn icmpv6_and_ndp_registry_entries_resolve_to_packaged_paths() {
    assert_eq!(
        protocol_dsl_path("icmpv6", Some("echo")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/icmpv6/echo".to_string())
    );
    assert_eq!(
        protocol_dsl_path("ping6", None),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/icmpv6/echo".to_string())
    );
    assert_eq!(
        protocol_dsl_path("icmpv6", Some("no-route")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/icmpv6/unreachable".to_string())
    );
    assert_eq!(
        protocol_dsl_path("ndp", Some("solicit")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/ndp/solicit".to_string())
    );
    assert_eq!(
        protocol_dsl_path("ndp", Some("na")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/ndp/advertise".to_string())
    );
}

#[test]
fn icmpv6_and_ndp_defaults_expose_expected_entries() {
    assert_eq!(protocol_default_entry("icmpv6"), Some("echo".to_string()));
    let icmpv6_entries = protocol_entries("icmpv6").expect("icmpv6 entries should resolve");
    assert!(icmpv6_entries.contains(&"echo".to_string()));
    assert!(icmpv6_entries.contains(&"unreachable".to_string()));

    assert_eq!(protocol_default_entry("ndp"), Some("solicit".to_string()));
    let ndp_entries = protocol_entries("ndp").expect("ndp entries should resolve");
    assert!(ndp_entries.contains(&"solicit".to_string()));
    assert!(ndp_entries.contains(&"advertise".to_string()));
}

#[test]
fn icmpv6_and_ndp_surfaces_expose_reachability_and_neighbor_semantics() {
    let echo = protocol_surface("icmpv6", "echo").expect("icmpv6 echo surface should exist");
    assert_eq!(echo.shelf.expect("echo shelf should exist").key, "echo");
    assert_eq!(
        echo.entry_semantics
            .expect("echo semantics should exist")
            .category,
        "reachability-path"
    );

    let unreachable =
        protocol_surface("icmpv6", "unreachable").expect("icmpv6 unreachable surface should exist");
    assert_eq!(
        unreachable
            .shelf
            .expect("unreachable shelf should exist")
            .key,
        "failure"
    );

    let solicit = protocol_surface("ndp", "solicit").expect("ndp solicit surface should exist");
    assert_eq!(
        solicit.shelf.expect("solicit shelf should exist").key,
        "solicit"
    );
    assert_eq!(
        solicit
            .entry_semantics
            .expect("solicit semantics should exist")
            .category,
        "neighbor-resolution-path"
    );

    let advertise =
        protocol_surface("ndp", "advertise").expect("ndp advertise surface should exist");
    assert_eq!(
        advertise
            .entry_semantics
            .expect("advertise semantics should exist")
            .typical_signal,
        Some("ICMPv6 type 136 neighbor advertisement".to_string())
    );
}

#[test]
fn icmpv6_and_ndp_dsl_files_compile_into_expected_operations() {
    let echo = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/icmpv6_echo_path.gewy").unwrap();
    assert_eq!(echo.template.id, "icmpv6_echo_path");
    assert_eq!(
        echo.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("icmpv6_echo".into())
    );

    let unreachable =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/icmpv6_unreachable_path.gewy").unwrap();
    assert_eq!(unreachable.template.id, "icmpv6_unreachable_path");
    assert_eq!(
        unreachable
            .template
            .program_model
            .as_ref()
            .unwrap()
            .operation,
        ProgramOperation::Custom("icmpv6_unreachable".into())
    );

    let solicit =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/ndp_solicit_path.gewy").unwrap();
    assert_eq!(solicit.template.id, "ndp_solicit_path");
    assert_eq!(
        solicit.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("ndp_solicit".into())
    );

    let advertise =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/ndp_advertise_path.gewy").unwrap();
    assert_eq!(advertise.template.id, "ndp_advertise_path");
    assert_eq!(
        advertise.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("ndp_advertise".into())
    );
}

#[test]
fn icmpv6_runtime_paths_materialize_echo_and_unreachable_stages() {
    let echo = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/icmpv6_echo_path.gewy").unwrap();
    let config = SessionConfig::for_binding(echo).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 9812, 0, "ping6"));
    session.ingest(route_fact(2, 9812, 7));
    session.ingest(icmpv6_packet_fact(3, 9812, PacketDir::Egress, 128));
    session.ingest(icmpv6_packet_fact(4, 9812, PacketDir::Ingress, 129));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("icmpv6_echo".into())
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

    let unreachable =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/icmpv6_unreachable_path.gewy").unwrap();
    let config = SessionConfig::for_binding(unreachable).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 9813, 0, "probe6"));
    session.ingest(route_fact(2, 9813, 7));
    session.ingest(icmpv6_packet_fact(3, 9813, PacketDir::Ingress, 1));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("icmpv6_unreachable".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_unreachable"))
    );
}

#[test]
fn ndp_runtime_paths_materialize_solicit_and_advertise_stages() {
    let solicit =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/ndp_solicit_path.gewy").unwrap();
    let config = SessionConfig::for_binding(solicit).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(ndp_packet_fact(1, PacketDir::Egress, 135));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("ndp_solicit".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_neighbor_solicitation"))
    );

    let advertise =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/ndp_advertise_path.gewy").unwrap();
    let config = SessionConfig::for_binding(advertise).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(ndp_packet_fact(1, PacketDir::Ingress, 136));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("ndp_advertise".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_neighbor_advertisement"))
    );
}
