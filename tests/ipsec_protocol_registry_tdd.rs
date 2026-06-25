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

fn ipsec_packet_fact(id: u64, dir: PacketDir, l4_proto: u8) -> FactEnvelope {
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
            payload_byte0: Some(0x12),
            payload_byte1: Some(0x34),
            payload_prefix2: Some(0x1234),
            payload_prefix4: None,
            payload_byte4: None,
            payload_byte5: None,
            payload_byte9: None,
            payload_byte10: None,
            payload_byte13: None,
            payload_bytes: BTreeMap::from([(0, 0x12), (1, 0x34)]),
            l3_proto: 0x0800,
            l4_proto,
            tot_len: 96,
            tcp_flags: 0,
            seq: None,
            ack: None,
            window: None,
        }),
    }
}

#[test]
fn ipsec_registry_entries_resolve_to_packaged_paths() {
    assert_eq!(
        protocol_dsl_path("ipsec", Some("esp")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/ipsec/esp".to_string())
    );
    assert_eq!(
        protocol_dsl_path("ipsec-esp", None),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/ipsec/esp".to_string())
    );
    assert_eq!(
        protocol_dsl_path("ipsec", Some("auth-header")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/ipsec/ah".to_string())
    );
}

#[test]
fn ipsec_default_entry_is_esp_with_ah_available() {
    assert_eq!(protocol_default_entry("ipsec"), Some("esp".to_string()));

    let entries = protocol_entries("ipsec").expect("ipsec entries should resolve");
    assert!(entries.contains(&"esp".to_string()));
    assert!(entries.contains(&"ah".to_string()));
}

#[test]
fn ipsec_surface_exposes_security_semantics() {
    let esp = protocol_surface("ipsec", "esp").expect("ipsec esp surface should exist");
    assert_eq!(esp.shelf.expect("esp shelf should exist").key, "security");
    assert_eq!(
        esp.entry_semantics
            .expect("esp semantics should exist")
            .category,
        "secure-encapsulation-path"
    );

    let ah = protocol_surface("ipsec", "ah").expect("ipsec ah surface should exist");
    assert_eq!(
        ah.entry_semantics
            .expect("ah semantics should exist")
            .typical_signal,
        Some("IP protocol 51 AH packet".to_string())
    );
}

#[test]
fn ipsec_dsl_files_compile_into_expected_operations() {
    let esp = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/ipsec_esp_path.gewy").unwrap();
    assert_eq!(esp.template.id, "ipsec_esp_path");
    assert_eq!(
        esp.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("ipsec_esp".into())
    );

    let ah = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/ipsec_ah_path.gewy").unwrap();
    assert_eq!(ah.template.id, "ipsec_ah_path");
    assert_eq!(
        ah.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("ipsec_ah".into())
    );
}

#[test]
fn ipsec_esp_runtime_path_materializes_secure_path_stages() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/ipsec_esp_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(ipsec_packet_fact(1, PacketDir::Egress, 50));
    session.ingest(ipsec_packet_fact(2, PacketDir::Ingress, 50));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("ipsec_esp".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_esp_packet"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_esp_packet"))
    );
}

#[test]
fn ipsec_ah_runtime_path_materializes_authenticated_path_stages() {
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/ipsec_ah_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(ipsec_packet_fact(1, PacketDir::Egress, 51));
    session.ingest(ipsec_packet_fact(2, PacketDir::Ingress, 51));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("ipsec_ah".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_ah_packet"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_ah_packet"))
    );
}
