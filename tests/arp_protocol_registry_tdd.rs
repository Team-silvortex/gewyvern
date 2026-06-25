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

fn arp_packet_fact(id: u64, dir: PacketDir, opcode: u8) -> FactEnvelope {
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
            payload_byte0: Some(0x00),
            payload_byte1: Some(0x01),
            payload_prefix2: Some(0x0001),
            payload_prefix4: Some(0x00010800),
            payload_byte4: Some(0x06),
            payload_byte5: Some(0x04),
            payload_byte9: None,
            payload_byte10: None,
            payload_byte13: None,
            payload_bytes: BTreeMap::from([(6, 0x00), (7, opcode)]),
            l3_proto: 0x0806,
            l4_proto: 0,
            tot_len: 28,
            tcp_flags: 0,
            seq: None,
            ack: None,
            window: None,
        }),
    }
}

#[test]
fn arp_registry_entries_resolve_to_packaged_paths() {
    assert_eq!(
        protocol_dsl_path("arp", Some("request")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/arp/request".to_string())
    );
    assert_eq!(
        protocol_dsl_path("arp-request", None),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/arp/request".to_string())
    );
    assert_eq!(
        protocol_dsl_path("arp", Some("is-at")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/arp/reply".to_string())
    );
}

#[test]
fn arp_default_entry_is_request_with_reply_available() {
    assert_eq!(protocol_default_entry("arp"), Some("request".to_string()));

    let entries = protocol_entries("arp").expect("arp entries should resolve");
    assert!(entries.contains(&"request".to_string()));
    assert!(entries.contains(&"reply".to_string()));
}

#[test]
fn arp_surface_exposes_neighbor_resolution_semantics() {
    let request = protocol_surface("arp", "request").expect("arp request surface should exist");
    assert_eq!(
        request.shelf.expect("request shelf should exist").key,
        "request"
    );
    assert_eq!(
        request
            .entry_semantics
            .expect("request semantics should exist")
            .category,
        "neighbor-resolution-path"
    );

    let reply = protocol_surface("arp", "reply").expect("arp reply surface should exist");
    assert_eq!(reply.shelf.expect("reply shelf should exist").key, "reply");
    assert_eq!(
        reply
            .entry_semantics
            .expect("reply semantics should exist")
            .typical_signal,
        Some("ARP opcode 2 is-at".to_string())
    );
}

#[test]
fn arp_dsl_files_compile_into_expected_operations() {
    let request =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/arp_request_path.gewy").unwrap();
    assert_eq!(request.template.id, "arp_request_path");
    assert_eq!(
        request.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("arp_request".into())
    );

    let reply = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/arp_reply_path.gewy").unwrap();
    assert_eq!(reply.template.id, "arp_reply_path");
    assert_eq!(
        reply.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("arp_reply".into())
    );
}

#[test]
fn arp_request_runtime_path_materializes_who_has_stage() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/arp_request_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(arp_packet_fact(1, PacketDir::Egress, 0x01));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("arp_request".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_who_has"))
    );
}

#[test]
fn arp_reply_runtime_path_materializes_is_at_stage() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/arp_reply_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(arp_packet_fact(1, PacketDir::Ingress, 0x02));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("arp_reply".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_is_at"))
    );
}
