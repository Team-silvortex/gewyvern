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

fn gre_packet_fact(id: u64, dir: PacketDir, prefix2: u16) -> FactEnvelope {
    let [byte0, byte1] = prefix2.to_be_bytes();
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
            payload_byte0: Some(byte0),
            payload_byte1: Some(byte1),
            payload_prefix2: Some(prefix2),
            payload_prefix4: None,
            payload_byte4: None,
            payload_byte5: None,
            payload_byte9: None,
            payload_byte10: None,
            payload_byte13: None,
            payload_bytes: BTreeMap::from([(0, byte0), (1, byte1)]),
            l3_proto: 0x0800,
            l4_proto: 47,
            tot_len: 64,
            tcp_flags: 0,
            seq: None,
            ack: None,
            window: None,
        }),
    }
}

#[test]
fn gre_registry_entries_resolve_to_packaged_paths() {
    assert_eq!(
        protocol_dsl_path("gre", Some("encap")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/gre/encap".to_string())
    );
    assert_eq!(
        protocol_dsl_path("gre-tunnel", None),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/gre/encap".to_string())
    );
    assert_eq!(
        protocol_dsl_path("gre", Some("keep-alive")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/gre/keepalive".to_string())
    );
}

#[test]
fn gre_default_entry_is_encap_with_keepalive_available() {
    assert_eq!(protocol_default_entry("gre"), Some("encap".to_string()));

    let entries = protocol_entries("gre").expect("gre entries should resolve");
    assert!(entries.contains(&"encap".to_string()));
    assert!(entries.contains(&"keepalive".to_string()));
}

#[test]
fn gre_surface_exposes_tunnel_semantics() {
    let encap = protocol_surface("gre", "encap").expect("gre encap surface should exist");
    assert_eq!(encap.shelf.expect("encap shelf should exist").key, "tunnel");
    assert_eq!(
        encap
            .entry_semantics
            .expect("encap semantics should exist")
            .category,
        "tunnel-encapsulation-path"
    );

    let keepalive =
        protocol_surface("gre", "keepalive").expect("gre keepalive surface should exist");
    assert_eq!(
        keepalive
            .entry_semantics
            .expect("keepalive semantics should exist")
            .typical_signal,
        Some("GRE flags/version prefix 0x0000".to_string())
    );
}

#[test]
fn gre_dsl_files_compile_into_expected_operations() {
    let encap = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/gre_encap_path.gewy").unwrap();
    assert_eq!(encap.template.id, "gre_encap_path");
    assert_eq!(
        encap.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("gre_encap".into())
    );

    let keepalive =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/gre_keepalive_path.gewy").unwrap();
    assert_eq!(keepalive.template.id, "gre_keepalive_path");
    assert_eq!(
        keepalive.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("gre_keepalive".into())
    );
}

#[test]
fn gre_encap_runtime_path_materializes_tunnel_stages() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/gre_encap_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(gre_packet_fact(1, PacketDir::Egress, 0x2000));
    session.ingest(gre_packet_fact(2, PacketDir::Ingress, 0x2000));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("gre_encap".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_encapsulated_packet"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_encapsulated_packet"))
    );
}

#[test]
fn gre_keepalive_runtime_path_materializes_liveness_stages() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/gre_keepalive_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(gre_packet_fact(1, PacketDir::Egress, 0x0000));
    session.ingest(gre_packet_fact(2, PacketDir::Ingress, 0x0000));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("gre_keepalive".into())
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
