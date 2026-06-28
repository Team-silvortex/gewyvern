mod support;

use gewyvern::dsl::compile_file;
use gewyvern::flow::ProgramOperation;
use gewyvern::ledger::PacketDir;
use gewyvern::runtime::{RuntimeSession, SessionConfig};
use std::time::{Duration, SystemTime};
use support::{
    route_fact, sock_lineage_fact, udp_packet_fact_with_dir_and_ports_and_payload,
    udp_quic_meta_fact, udp_quic_meta_fact_with_payload_bytes,
};

fn dsl_fixture_path(name: &str) -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("dsl")
        .join(name)
        .to_string_lossy()
        .into_owned()
}
#[test]
fn http3_close_dsl_file_compiles_into_expected_operation() {
    let binding = compile_file(&dsl_fixture_path("http3_close_path.gewy"))
        .expect("http3 close DSL should compile");
    assert_eq!(binding.template.id, "http3_close_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("http3_close_observation".into())
    );
}

#[test]
fn http3_server_close_dsl_file_compiles_into_expected_operation() {
    let binding = compile_file(&dsl_fixture_path("http3_server_close_path.gewy"))
        .expect("http3 server-close DSL should compile");
    assert_eq!(binding.template.id, "http3_server_close_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("http3_server_close_observation".into())
    );
}

#[test]
fn http3_server_close_runtime_path_materializes_response_and_close_stages() {
    let binding = compile_file(&dsl_fixture_path("http3_server_close_path.gewy"))
        .expect("http3 server-close DSL should compile");
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 6912, 8443, "nginx"));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        2,
        6912,
        1280,
        PacketDir::Ingress,
        Some(443),
        Some(53112),
        Some(0xc3),
        Some(0xc300),
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        6912,
        220,
        PacketDir::Egress,
        Some(443),
        Some(53112),
        Some(0xe0),
        None,
    ));
    session.ingest(udp_quic_meta_fact(
        4,
        6912,
        PacketDir::Ingress,
        Some(443),
        Some(53112),
        true,
        Some(gewyvern::ir::QuicPacketType::Initial),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_quic_meta_fact(
        5,
        6912,
        PacketDir::Egress,
        Some(443),
        Some(53112),
        true,
        Some(gewyvern::ir::QuicPacketType::Handshake),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_quic_meta_fact(
        6,
        6912,
        PacketDir::Ingress,
        Some(443),
        Some(53112),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Stream],
    ));
    session.ingest(udp_quic_meta_fact(
        7,
        6912,
        PacketDir::Egress,
        Some(443),
        Some(53112),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Stream],
    ));
    session.ingest(udp_quic_meta_fact(
        8,
        6912,
        PacketDir::Egress,
        Some(443),
        Some(53112),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::ConnectionClose],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(110));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("http3_server_close_observation".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_response_stream"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_close"))
    );
}

#[test]
fn http3_close_runtime_path_materializes_request_and_close_stages() {
    let binding = compile_file(&dsl_fixture_path("http3_close_path.gewy"))
        .expect("http3 close DSL should compile");
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 6910, 4433, "curl"));
    session.ingest(route_fact(2, 6910, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        6910,
        1280,
        PacketDir::Egress,
        Some(53110),
        Some(443),
        Some(0xc3),
        Some(0xc300),
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        4,
        6910,
        220,
        PacketDir::Ingress,
        Some(53110),
        Some(443),
        Some(0xe0),
        None,
    ));
    session.ingest(udp_quic_meta_fact(
        5,
        6910,
        PacketDir::Egress,
        Some(53110),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Initial),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_quic_meta_fact(
        6,
        6910,
        PacketDir::Ingress,
        Some(53110),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Handshake),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_quic_meta_fact(
        7,
        6910,
        PacketDir::Egress,
        Some(53110),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Stream],
    ));
    session.ingest(udp_quic_meta_fact(
        8,
        6910,
        PacketDir::Ingress,
        Some(53110),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::ConnectionClose],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(90));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("http3_close_observation".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_request_stream"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_close"))
    );
}

#[test]
fn hy2_close_dsl_file_compiles_into_expected_operation() {
    let binding = compile_file(&dsl_fixture_path("hy2_close_path.gewy"))
        .expect("hy2 close DSL should compile");
    assert_eq!(binding.template.id, "hy2_close_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("hy2_close_observation".into())
    );
}

#[test]
fn hy2_tcp_close_dsl_file_compiles_into_expected_operation() {
    let binding = compile_file(&dsl_fixture_path("hy2_tcp_close_path.gewy"))
        .expect("hy2 tcp-close DSL should compile");
    assert_eq!(binding.template.id, "hy2_tcp_close_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("hy2_tcp_close_observation".into())
    );
}

#[test]
fn hy2_tcp_close_runtime_path_materializes_tcp_relay_and_close_stages() {
    let binding = compile_file(&dsl_fixture_path("hy2_tcp_close_path.gewy"))
        .expect("hy2 tcp-close DSL should compile");
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 6913, 4433, "hysteria"));
    session.ingest(route_fact(2, 6913, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        6913,
        1280,
        PacketDir::Egress,
        Some(53113),
        Some(443),
        Some(0xc3),
        Some(0xc300),
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        4,
        6913,
        220,
        PacketDir::Ingress,
        Some(53113),
        Some(443),
        Some(0xe0),
        None,
    ));
    session.ingest(udp_quic_meta_fact(
        5,
        6913,
        PacketDir::Egress,
        Some(53113),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Initial),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_quic_meta_fact(
        6,
        6913,
        PacketDir::Ingress,
        Some(53113),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Handshake),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_quic_meta_fact(
        7,
        6913,
        PacketDir::Egress,
        Some(53113),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Stream],
    ));
    session.ingest(udp_quic_meta_fact(
        8,
        6913,
        PacketDir::Ingress,
        Some(53113),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Stream],
    ));
    session.ingest(udp_quic_meta_fact_with_payload_bytes(
        9,
        6913,
        PacketDir::Egress,
        Some(53113),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Stream],
        &[(0, 0x44), (1, 0x01)],
    ));
    session.ingest(udp_quic_meta_fact_with_payload_bytes(
        10,
        6913,
        PacketDir::Egress,
        Some(53113),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Stream],
        &[(0, 0x44), (1, 0x01)],
    ));
    session.ingest(udp_quic_meta_fact_with_payload_bytes(
        11,
        6913,
        PacketDir::Ingress,
        Some(53113),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Stream],
        &[(0, 0x00)],
    ));
    session.ingest(udp_quic_meta_fact_with_payload_bytes(
        12,
        6913,
        PacketDir::Ingress,
        Some(53113),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Stream],
        &[(0, 0x00)],
    ));
    session.ingest(udp_quic_meta_fact(
        13,
        6913,
        PacketDir::Ingress,
        Some(53113),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::ConnectionClose],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(120));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("hy2_tcp_close_observation".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_tcp_request_stream"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_tcp_response_stream"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_close"))
    );
}

#[test]
fn hy2_udp_close_dsl_file_compiles_into_expected_operation() {
    let binding = compile_file(&dsl_fixture_path("hy2_udp_close_path.gewy"))
        .expect("hy2 udp-close DSL should compile");
    assert_eq!(binding.template.id, "hy2_udp_close_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("hy2_udp_close_observation".into())
    );
}

#[test]
fn hy2_udp_close_runtime_path_materializes_udp_relay_and_close_stages() {
    let binding = compile_file(&dsl_fixture_path("hy2_udp_close_path.gewy"))
        .expect("hy2 udp-close DSL should compile");
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 6914, 4433, "hysteria"));
    session.ingest(route_fact(2, 6914, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        6914,
        1280,
        PacketDir::Egress,
        Some(53114),
        Some(443),
        Some(0xc3),
        Some(0xc300),
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        4,
        6914,
        220,
        PacketDir::Ingress,
        Some(53114),
        Some(443),
        Some(0xe0),
        None,
    ));
    session.ingest(udp_quic_meta_fact(
        5,
        6914,
        PacketDir::Egress,
        Some(53114),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Initial),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_quic_meta_fact(
        6,
        6914,
        PacketDir::Ingress,
        Some(53114),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Handshake),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_quic_meta_fact(
        7,
        6914,
        PacketDir::Egress,
        Some(53114),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Stream],
    ));
    session.ingest(udp_quic_meta_fact(
        8,
        6914,
        PacketDir::Ingress,
        Some(53114),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Stream],
    ));
    session.ingest(udp_quic_meta_fact(
        9,
        6914,
        PacketDir::Egress,
        Some(53114),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Datagram],
    ));
    session.ingest(udp_quic_meta_fact(
        10,
        6914,
        PacketDir::Ingress,
        Some(53114),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Datagram],
    ));
    session.ingest(udp_quic_meta_fact(
        11,
        6914,
        PacketDir::Ingress,
        Some(53114),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::ConnectionClose],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(120));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("hy2_udp_close_observation".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_udp_relay_datagram"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_udp_relay_datagram"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_close"))
    );
}

#[test]
fn hy2_close_runtime_path_materializes_auth_ok_and_close_stages() {
    let binding = compile_file(&dsl_fixture_path("hy2_close_path.gewy"))
        .expect("hy2 close DSL should compile");
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 6911, 4433, "hysteria"));
    session.ingest(route_fact(2, 6911, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        6911,
        1280,
        PacketDir::Egress,
        Some(53111),
        Some(443),
        Some(0xc3),
        Some(0xc300),
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        4,
        6911,
        220,
        PacketDir::Ingress,
        Some(53111),
        Some(443),
        Some(0xe0),
        None,
    ));
    session.ingest(udp_quic_meta_fact(
        5,
        6911,
        PacketDir::Egress,
        Some(53111),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Initial),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_quic_meta_fact(
        6,
        6911,
        PacketDir::Ingress,
        Some(53111),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Handshake),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_quic_meta_fact(
        7,
        6911,
        PacketDir::Egress,
        Some(53111),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Stream],
    ));
    session.ingest(udp_quic_meta_fact(
        8,
        6911,
        PacketDir::Ingress,
        Some(53111),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Stream],
    ));
    session.ingest(udp_quic_meta_fact(
        9,
        6911,
        PacketDir::Ingress,
        Some(53111),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::ConnectionClose],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(100));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("hy2_close_observation".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_auth_ok_stream"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_close"))
    );
}
