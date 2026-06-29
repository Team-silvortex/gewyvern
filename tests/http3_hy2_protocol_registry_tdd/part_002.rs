use super::*;

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
