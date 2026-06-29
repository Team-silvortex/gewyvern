use super::*;

#[test]
fn quic_stream_session_path_materializes_stream_and_close_stages() {
    let binding = compile_file(&dsl_fixture_path("quic_stream_session_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 806, 4242, "curl"));
    session.ingest(route_fact(2, 806, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        806,
        1280,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        Some(0xc3),
        Some(0xc300),
    ));
    session.ingest(udp_quic_meta_fact(
        4,
        806,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Initial),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        5,
        806,
        220,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        Some(0xe0),
        None,
    ));
    session.ingest(udp_quic_meta_fact(
        6,
        806,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Handshake),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_quic_meta_fact(
        7,
        806,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Stream],
    ));
    session.ingest(udp_quic_meta_fact(
        8,
        806,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::ConnectionClose],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(100));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("quic_stream_session".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_stream"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_close"))
    );
    let phase_kinds = export.program_flows[0]
        .stages
        .iter()
        .filter_map(|stage| stage.phase_kind.clone())
        .collect::<Vec<_>>();
    assert!(phase_kinds.contains(&"emit_payload".to_string()));
    assert!(phase_kinds.contains(&"receive_payload".to_string()));
    assert_eq!(export.module_findings.len(), 0);
}

#[test]
fn quic_stream_session_path_does_not_treat_ack_as_stream() {
    let binding = compile_file(&dsl_fixture_path("quic_stream_session_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 807, 4242, "curl"));
    session.ingest(route_fact(2, 807, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        807,
        1280,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        Some(0xc3),
        Some(0xc300),
    ));
    session.ingest(udp_quic_meta_fact(
        4,
        807,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Initial),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        5,
        807,
        220,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        Some(0xe0),
        None,
    ));
    session.ingest(udp_quic_meta_fact(
        6,
        807,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Handshake),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_quic_meta_fact(
        7,
        807,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Ack],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(90));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("send_stream"))
    );
}

#[test]
fn quic_bidi_stream_path_materializes_request_response_and_close_stages() {
    let binding = compile_file(&dsl_fixture_path("quic_bidi_stream_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 808, 4242, "curl"));
    session.ingest(route_fact(2, 808, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        808,
        1280,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        Some(0xc3),
        Some(0xc300),
    ));
    session.ingest(udp_quic_meta_fact(
        4,
        808,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Initial),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        5,
        808,
        220,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        Some(0xe0),
        None,
    ));
    session.ingest(udp_quic_meta_fact(
        6,
        808,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Handshake),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_quic_meta_fact(
        7,
        808,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Stream],
    ));
    session.ingest(udp_quic_meta_fact(
        8,
        808,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Stream],
    ));
    session.ingest(udp_quic_meta_fact(
        9,
        808,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::ConnectionClose],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(110));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("quic_bidi_stream".into())
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
            .any(|stage| stage.phase.as_deref() == Some("receive_response_stream"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_close"))
    );
    assert_eq!(export.module_findings.len(), 0);
}

#[test]
fn quic_bidi_stream_path_does_not_treat_close_as_response_stream() {
    let binding = compile_file(&dsl_fixture_path("quic_bidi_stream_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 809, 4242, "curl"));
    session.ingest(route_fact(2, 809, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        809,
        1280,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        Some(0xc3),
        Some(0xc300),
    ));
    session.ingest(udp_quic_meta_fact(
        4,
        809,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Initial),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        5,
        809,
        220,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        Some(0xe0),
        None,
    ));
    session.ingest(udp_quic_meta_fact(
        6,
        809,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Handshake),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_quic_meta_fact(
        7,
        809,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Stream],
    ));
    session.ingest(udp_quic_meta_fact(
        8,
        809,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::ConnectionClose],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(100));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_response_stream"))
    );
}

#[test]
fn http3_request_path_materializes_request_response_and_close_stages() {
    let binding = compile_file(&dsl_fixture_path("http3_request_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 810, 4242, "curl"));
    session.ingest(route_fact(2, 810, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        810,
        1280,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        Some(0xc3),
        Some(0xc300),
    ));
    session.ingest(udp_quic_meta_fact(
        4,
        810,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Initial),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        5,
        810,
        220,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        Some(0xe0),
        None,
    ));
    session.ingest(udp_quic_meta_fact(
        6,
        810,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Handshake),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_quic_meta_fact(
        7,
        810,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Stream],
    ));
    session.ingest(udp_quic_meta_fact(
        8,
        810,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Stream],
    ));
    session.ingest(udp_quic_meta_fact(
        9,
        810,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::ConnectionClose],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(110));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("http3_request".into())
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
            .any(|stage| stage.phase.as_deref() == Some("receive_response_stream"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_close"))
    );
    assert_eq!(export.module_findings.len(), 0);
}
