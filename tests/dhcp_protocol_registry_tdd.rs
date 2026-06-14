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

fn udp_packet_fact_with_payload_bytes(
    id: u64,
    cookie: u64,
    tot_len: u32,
    dir: PacketDir,
    local_port: u16,
    remote_port: u16,
    payload_bytes: &[(u16, u8)],
) -> FactEnvelope {
    let byte_at = |target: u16| {
        payload_bytes
            .iter()
            .find_map(|(offset, value)| (*offset == target).then_some(*value))
    };
    let payload_byte0 = byte_at(0);
    let payload_byte1 = byte_at(1);
    let payload_byte2 = byte_at(2);
    let payload_byte3 = byte_at(3);
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
            local_port: Some(local_port),
            remote_port: Some(remote_port),
            payload_byte0,
            payload_byte1: byte_at(1),
            payload_prefix2: payload_byte0
                .zip(payload_byte1)
                .map(|(b0, b1)| u16::from_be_bytes([b0, b1])),
            payload_prefix4: payload_byte0
                .zip(payload_byte1)
                .zip(payload_byte2)
                .zip(payload_byte3)
                .map(|(((b0, b1), b2), b3)| u32::from_be_bytes([b0, b1, b2, b3])),
            payload_byte4: byte_at(4),
            payload_byte5: byte_at(5),
            payload_byte9: byte_at(9),
            payload_byte10: byte_at(10),
            payload_byte13: byte_at(13),
            payload_bytes: payload_bytes.iter().copied().collect::<BTreeMap<_, _>>(),
            l3_proto: 0x0800,
            l4_proto: 17,
            tot_len,
            tcp_flags: 0,
            seq: None,
            ack: None,
            window: None,
        }),
    }
}

#[test]
fn dhcp_registry_entries_resolve_to_packaged_paths() {
    assert_eq!(
        protocol_dsl_path("dhcp", Some("discover")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/dhcp/discover".to_string())
    );
    assert_eq!(
        protocol_dsl_path("dhcp", Some("offer-probe")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/dhcp/discover".to_string())
    );
    assert_eq!(
        protocol_dsl_path("dhcp", Some("request")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/dhcp/request".to_string())
    );
    assert_eq!(
        protocol_dsl_path("dhcp", Some("renew")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/dhcp/request".to_string())
    );
}

#[test]
fn dhcp_default_entry_stays_client_while_surface_grows() {
    assert_eq!(protocol_default_entry("dhcp"), Some("client".to_string()));

    let entries = protocol_entries("dhcp").expect("dhcp entries should resolve");
    assert!(entries.contains(&"client".to_string()));
    assert!(entries.contains(&"discover".to_string()));
    assert!(entries.contains(&"request".to_string()));
}

#[test]
fn dhcp_surface_keeps_generic_shelves_per_entry() {
    for (entry, key) in [
        ("client", "client"),
        ("discover", "discover"),
        ("request", "request"),
    ] {
        let surface = protocol_surface("dhcp", entry).expect("dhcp surface should exist");
        let shelf = surface.shelf.expect("dhcp shelf should exist");
        assert_eq!(shelf.key, key);
    }
}

#[test]
fn dhcp_dsl_files_compile_into_expected_operations() {
    let discover =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/dhcp_discover_path.gewy").unwrap();
    assert_eq!(discover.template.id, "dhcp_discover_path");
    assert_eq!(
        discover.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("dhcp_discover".into())
    );

    let request =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/dhcp_request_path.gewy").unwrap();
    assert_eq!(request.template.id, "dhcp_request_path");
    assert_eq!(
        request.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("dhcp_request".into())
    );
}

#[test]
fn dhcp_discover_runtime_path_materializes_discover_and_offer() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/dhcp_discover_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 4815, 68, "dhclient"));
    session.ingest(route_fact(2, 4815, 7));
    session.ingest(udp_packet_fact_with_payload_bytes(
        3,
        4815,
        300,
        PacketDir::Egress,
        68,
        67,
        &[(0, 0x01), (1, 0x01), (242, 0x01)],
    ));
    session.ingest(udp_packet_fact_with_payload_bytes(
        4,
        4815,
        300,
        PacketDir::Ingress,
        68,
        67,
        &[(0, 0x02), (1, 0x01), (242, 0x02)],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("dhcp_discover".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_discover"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_offer"))
    );
}

#[test]
fn dhcp_request_runtime_path_rejects_offer_when_ack_is_expected() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/dhcp_request_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 4816, 68, "dhclient"));
    session.ingest(route_fact(2, 4816, 7));
    session.ingest(udp_packet_fact_with_payload_bytes(
        3,
        4816,
        300,
        PacketDir::Egress,
        68,
        67,
        &[(0, 0x01), (1, 0x01), (242, 0x03)],
    ));
    session.ingest(udp_packet_fact_with_payload_bytes(
        4,
        4816,
        300,
        PacketDir::Ingress,
        68,
        67,
        &[(0, 0x02), (1, 0x01), (242, 0x02)],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_ack"))
    );
}
