mod support;

use gewyvern::dsl::compile_file;
use gewyvern::flow::ProgramOperation;
use gewyvern::ledger::PacketDir;
use gewyvern::protocol_profiles::{
    protocol_default_entry, protocol_dsl_path, protocol_entries, protocol_surface,
};
use gewyvern::runtime::{RuntimeSession, SessionConfig};
use std::time::{Duration, SystemTime};
use support::{
    route_fact, sock_lineage_fact, udp_packet_fact_with_dir_and_ports_and_payload,
    udp_quic_meta_fact,
};

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
#[test]
fn quic_retry_registry_entry_resolves_to_packaged_path() {
    assert_eq!(
        protocol_dsl_path("quic", Some("address-validation")),
        Some(protocol_fixture_path("quic/retry").to_string())
    );
}

#[test]
fn quic_default_entry_stays_initial_while_surface_grows() {
    assert_eq!(protocol_default_entry("quic"), Some("initial".to_string()));

    let entries = protocol_entries("quic").expect("quic entries should resolve");
    assert!(entries.contains(&"initial".to_string()));
    assert!(entries.contains(&"retry".to_string()));
    assert!(entries.contains(&"close".to_string()));
    assert!(entries.contains(&"local-close".to_string()));
    assert!(entries.contains(&"crypto".to_string()));
}

#[test]
fn quic_surface_uses_split_shelves_per_entry() {
    for (entry, key) in [
        ("initial", "initial"),
        ("retry", "retry"),
        ("crypto", "crypto"),
        ("close", "close"),
        ("local-close", "local-close"),
        ("stream", "stream"),
        ("bidi", "bidi"),
    ] {
        let surface = protocol_surface("quic", entry).expect("quic surface should exist");
        let shelf = surface.shelf.expect("quic shelf should exist");
        assert_eq!(shelf.key, key);
    }
}

#[test]
fn quic_retry_dsl_file_compiles_into_expected_operation() {
    let retry = compile_file(&dsl_fixture_path("quic_retry_path.gewy")).unwrap();
    assert_eq!(retry.template.id, "quic_retry_path");
    assert_eq!(
        retry.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("quic_retry_validation".into())
    );
}

#[test]
fn quic_close_dsl_file_compiles_into_expected_operation() {
    let close = compile_file(&dsl_fixture_path("quic_close_path.gewy")).unwrap();
    assert_eq!(close.template.id, "quic_close_path");
    assert_eq!(
        close.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("quic_close_observation".into())
    );
}

#[test]
fn quic_local_close_dsl_file_compiles_into_expected_operation() {
    let close = compile_file(&dsl_fixture_path("quic_local_close_path.gewy")).unwrap();
    assert_eq!(close.template.id, "quic_local_close_path");
    assert_eq!(
        close.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("quic_local_close_observation".into())
    );
}

#[test]
fn quic_retry_runtime_path_materializes_initial_and_retry_datagrams() {
    let binding = compile_file(&dsl_fixture_path("quic_retry_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 5901, 4433, "quic-client"));
    session.ingest(route_fact(2, 5901, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        5901,
        1280,
        PacketDir::Egress,
        Some(53000),
        Some(443),
        Some(0xc3),
        Some(0xc300),
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        4,
        5901,
        160,
        PacketDir::Ingress,
        Some(53000),
        Some(443),
        Some(0xf0),
        None,
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("quic_retry_validation".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_retry"))
    );
}

#[test]
fn quic_close_runtime_path_materializes_handshake_and_close_frames() {
    let binding = compile_file(&dsl_fixture_path("quic_close_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 5902, 4433, "quic-client"));
    session.ingest(route_fact(2, 5902, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        5902,
        1280,
        PacketDir::Egress,
        Some(53001),
        Some(443),
        Some(0xc3),
        Some(0xc300),
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        4,
        5902,
        220,
        PacketDir::Ingress,
        Some(53001),
        Some(443),
        Some(0xe0),
        None,
    ));
    session.ingest(udp_quic_meta_fact(
        5,
        5902,
        PacketDir::Egress,
        Some(53001),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Initial),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_quic_meta_fact(
        6,
        5902,
        PacketDir::Ingress,
        Some(53001),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Handshake),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_quic_meta_fact(
        7,
        5902,
        PacketDir::Ingress,
        Some(53001),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::ConnectionClose],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(70));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("quic_close_observation".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_close"))
    );
}

#[test]
fn quic_local_close_runtime_path_materializes_handshake_and_local_close_frames() {
    let binding = compile_file(&dsl_fixture_path("quic_local_close_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 5903, 8443, "quic-server"));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        2,
        5903,
        1280,
        PacketDir::Ingress,
        Some(443),
        Some(53002),
        Some(0xc3),
        Some(0xc300),
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        5903,
        220,
        PacketDir::Egress,
        Some(443),
        Some(53002),
        Some(0xe0),
        None,
    ));
    session.ingest(udp_quic_meta_fact(
        4,
        5903,
        PacketDir::Ingress,
        Some(443),
        Some(53002),
        true,
        Some(gewyvern::ir::QuicPacketType::Initial),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_quic_meta_fact(
        5,
        5903,
        PacketDir::Egress,
        Some(443),
        Some(53002),
        true,
        Some(gewyvern::ir::QuicPacketType::Handshake),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_quic_meta_fact(
        6,
        5903,
        PacketDir::Egress,
        Some(443),
        Some(53002),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::ConnectionClose],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(70));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("quic_local_close_observation".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_close"))
    );
}
