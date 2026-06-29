use super::*;

#[test]
fn hy2_tcp_relay_path_materializes_auth_and_tcp_stream_stages() {
    let binding = compile_file(&dsl_fixture_path("hy2_tcp_relay_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 818, 4242, "hysteria"));
    session.ingest(route_fact(2, 818, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        818,
        1280,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        Some(0xc3),
        Some(0xc300),
    ));
    session.ingest(udp_quic_meta_fact(
        4,
        818,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Initial),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        5,
        818,
        220,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        Some(0xe0),
        None,
    ));
    session.ingest(udp_quic_meta_fact(
        6,
        818,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Handshake),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_quic_meta_fact(
        7,
        818,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Stream],
    ));
    session.ingest(udp_quic_meta_fact(
        8,
        818,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Stream],
    ));
    session.ingest(udp_quic_meta_fact_with_payload_bytes(
        9,
        818,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Stream],
        &[(0, 0x44), (1, 0x01)],
    ));
    session.ingest(udp_quic_meta_fact_with_payload_bytes(
        10,
        818,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Stream],
        &[(0, 0x00)],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(130));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("hy2_tcp_relay".into())
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
    assert_eq!(export.module_findings.len(), 0);
}

#[test]
fn hy2_tcp_relay_path_does_not_treat_auth_stream_as_tcp_request_stream() {
    let binding = compile_file(&dsl_fixture_path("hy2_tcp_relay_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 819, 4242, "hysteria"));
    session.ingest(route_fact(2, 819, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        819,
        1280,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        Some(0xc3),
        Some(0xc300),
    ));
    session.ingest(udp_quic_meta_fact(
        4,
        819,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Initial),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        5,
        819,
        220,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        Some(0xe0),
        None,
    ));
    session.ingest(udp_quic_meta_fact(
        6,
        819,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Handshake),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_quic_meta_fact(
        7,
        819,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Stream],
    ));
    session.ingest(udp_quic_meta_fact(
        8,
        819,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Stream],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(100));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("send_tcp_request_stream"))
    );
}

#[test]
fn quic_client_initial_path_missing_handshake_produces_datagram_transition() {
    let binding = compile_file(&dsl_fixture_path("quic_client_initial_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 804, 4242, "curl"));
    session.ingest(route_fact(2, 804, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        804,
        1280,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        Some(0xc1),
        Some(0xc300),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(50));

    let export = session.export_bundle();
    assert!(export.program_findings.iter().any(|finding| {
        finding.phase.as_deref() == Some("receive_handshake")
            && finding.phase_transition.as_deref() == Some("send_initial->receive_handshake")
            && finding.phase_transition_kind.as_deref() == Some("emit_datagram->receive_datagram")
    }));
}

#[test]
fn quic_client_initial_path_does_not_match_non_quic_udp_ports() {
    let binding = compile_file(&dsl_fixture_path("quic_client_initial_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 805, 4242, "curl"));
    session.ingest(route_fact(2, 805, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        805,
        1280,
        PacketDir::Egress,
        Some(42310),
        Some(53),
        Some(0xc0),
        Some(0xc300),
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        4,
        805,
        220,
        PacketDir::Ingress,
        Some(42310),
        Some(53),
        Some(0xe0),
        None,
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("send_initial"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_handshake"))
    );
}

#[test]
fn quic_client_initial_path_does_not_treat_small_quic_port_datagrams_as_initial() {
    let binding = compile_file(&dsl_fixture_path("quic_client_initial_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 806, 4242, "curl"));
    session.ingest(route_fact(2, 806, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        806,
        200,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        Some(0xc0),
        Some(0xc300),
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        4,
        806,
        220,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        Some(0xe0),
        None,
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("send_initial"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_handshake"))
    );
}

#[test]
fn quic_client_initial_path_does_not_treat_wrong_first_byte_as_initial() {
    let binding = compile_file(&dsl_fixture_path("quic_client_initial_path.gewy")).unwrap();
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
        Some(0x40),
        Some(0x4000),
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        4,
        807,
        220,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        Some(0xe0),
        None,
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("send_initial"))
    );
}

#[test]
fn quic_client_initial_path_does_not_treat_wrong_quic_packet_type_as_initial() {
    let binding = compile_file(&dsl_fixture_path("quic_client_initial_path.gewy")).unwrap();
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
        Some(0xd0),
        None,
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        4,
        808,
        220,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        Some(0xe0),
        None,
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("send_initial"))
    );
}

#[test]
fn stun_binding_path_materializes_request_and_response_datagrams() {
    let binding = compile_file(&dsl_fixture_path("stun_binding_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 809, 5000, "webrtc-app"));
    session.ingest(route_fact(2, 809, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        809,
        120,
        PacketDir::Egress,
        Some(54000),
        Some(3478),
        Some(0x00),
        Some(0x0001),
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        4,
        809,
        140,
        PacketDir::Ingress,
        Some(54000),
        Some(3478),
        Some(0x01),
        Some(0x0101),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("stun_binding".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_request"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_response"))
    );
    let phase_kinds = export.program_flows[0]
        .stages
        .iter()
        .filter_map(|stage| stage.phase_kind.clone())
        .collect::<Vec<_>>();
    assert!(phase_kinds.contains(&"emit_datagram".to_string()));
    assert!(phase_kinds.contains(&"receive_datagram".to_string()));
    assert_eq!(export.module_findings.len(), 0);
}

#[test]
fn stun_binding_path_does_not_match_wrong_message_type() {
    let binding = compile_file(&dsl_fixture_path("stun_binding_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 810, 5000, "webrtc-app"));
    session.ingest(route_fact(2, 810, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        810,
        120,
        PacketDir::Egress,
        Some(54000),
        Some(3478),
        Some(0x00),
        Some(0x0002),
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        4,
        810,
        140,
        PacketDir::Ingress,
        Some(54000),
        Some(3478),
        Some(0x01),
        Some(0x0101),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("send_request"))
    );
}
