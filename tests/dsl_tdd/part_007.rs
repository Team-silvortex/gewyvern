use super::*;

#[test]
fn http3_request_path_does_not_treat_close_as_response_stream() {
    let binding = compile_file(&dsl_fixture_path("http3_request_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 811, 4242, "curl"));
    session.ingest(route_fact(2, 811, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        811,
        1280,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        Some(0xc3),
        Some(0xc300),
    ));
    session.ingest(udp_quic_meta_fact(
        4,
        811,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Initial),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        5,
        811,
        220,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        Some(0xe0),
        None,
    ));
    session.ingest(udp_quic_meta_fact(
        6,
        811,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Handshake),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_quic_meta_fact(
        7,
        811,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Stream],
    ));
    session.ingest(udp_quic_meta_fact(
        8,
        811,
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
fn http3_server_response_path_materializes_request_response_and_close_stages() {
    let binding = compile_file(&dsl_fixture_path("http3_server_response_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 812, 8080, "nginx"));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        2,
        812,
        1280,
        PacketDir::Ingress,
        Some(443),
        Some(53000),
        Some(0xc3),
        Some(0xc300),
    ));
    session.ingest(udp_quic_meta_fact(
        3,
        812,
        PacketDir::Ingress,
        Some(443),
        Some(53000),
        true,
        Some(gewyvern::ir::QuicPacketType::Initial),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        4,
        812,
        220,
        PacketDir::Egress,
        Some(443),
        Some(53000),
        Some(0xe0),
        None,
    ));
    session.ingest(udp_quic_meta_fact(
        5,
        812,
        PacketDir::Egress,
        Some(443),
        Some(53000),
        true,
        Some(gewyvern::ir::QuicPacketType::Handshake),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_quic_meta_fact(
        6,
        812,
        PacketDir::Ingress,
        Some(443),
        Some(53000),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Stream],
    ));
    session.ingest(udp_quic_meta_fact(
        7,
        812,
        PacketDir::Egress,
        Some(443),
        Some(53000),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Stream],
    ));
    session.ingest(udp_quic_meta_fact(
        8,
        812,
        PacketDir::Egress,
        Some(443),
        Some(53000),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::ConnectionClose],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(100));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("http3_server_response".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_request_stream"))
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
    assert_eq!(export.module_findings.len(), 0);
}

#[test]
fn http3_server_response_path_does_not_treat_close_as_request_stream() {
    let binding = compile_file(&dsl_fixture_path("http3_server_response_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 813, 8080, "nginx"));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        2,
        813,
        1280,
        PacketDir::Ingress,
        Some(443),
        Some(53000),
        Some(0xc3),
        Some(0xc300),
    ));
    session.ingest(udp_quic_meta_fact(
        3,
        813,
        PacketDir::Ingress,
        Some(443),
        Some(53000),
        true,
        Some(gewyvern::ir::QuicPacketType::Initial),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        4,
        813,
        220,
        PacketDir::Egress,
        Some(443),
        Some(53000),
        Some(0xe0),
        None,
    ));
    session.ingest(udp_quic_meta_fact(
        5,
        813,
        PacketDir::Egress,
        Some(443),
        Some(53000),
        true,
        Some(gewyvern::ir::QuicPacketType::Handshake),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_quic_meta_fact(
        6,
        813,
        PacketDir::Egress,
        Some(443),
        Some(53000),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::ConnectionClose],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(90));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_request_stream"))
    );
}

#[test]
fn hy2_auth_path_materializes_auth_request_and_ok_stages() {
    let binding = compile_file(&dsl_fixture_path("hy2_auth_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 814, 4242, "hysteria"));
    session.ingest(route_fact(2, 814, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        814,
        1280,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        Some(0xc3),
        Some(0xc300),
    ));
    session.ingest(udp_quic_meta_fact(
        4,
        814,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Initial),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        5,
        814,
        220,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        Some(0xe0),
        None,
    ));
    session.ingest(udp_quic_meta_fact(
        6,
        814,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Handshake),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_quic_meta_fact(
        7,
        814,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Stream],
    ));
    session.ingest(udp_quic_meta_fact(
        8,
        814,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Stream],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(100));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("hy2_auth".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_auth_request_stream"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_auth_ok_stream"))
    );
    assert_eq!(export.module_findings.len(), 0);
}

#[test]
fn hy2_auth_path_does_not_treat_close_as_auth_ok_stream() {
    let binding = compile_file(&dsl_fixture_path("hy2_auth_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 815, 4242, "hysteria"));
    session.ingest(route_fact(2, 815, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        815,
        1280,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        Some(0xc3),
        Some(0xc300),
    ));
    session.ingest(udp_quic_meta_fact(
        4,
        815,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Initial),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        5,
        815,
        220,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        Some(0xe0),
        None,
    ));
    session.ingest(udp_quic_meta_fact(
        6,
        815,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Handshake),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_quic_meta_fact(
        7,
        815,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Stream],
    ));
    session.ingest(udp_quic_meta_fact(
        8,
        815,
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
            .all(|stage| stage.phase.as_deref() != Some("receive_auth_ok_stream"))
    );
}

#[test]
fn hy2_auth_operation_maps_to_proxy_authentication_module_kind() {
    assert_eq!(
        gewyvern::flow::infer_network_module_kind(
            &ProgramOperation::Custom("hy2_auth".into()),
            Some("receive_auth_ok_stream"),
            Some("send_auth_request_stream->receive_auth_ok_stream"),
            "transport_io",
        ),
        "proxy_authentication"
    );
}

#[test]
fn ssh_session_operation_maps_to_remote_access_session_module_kind() {
    assert_eq!(
        gewyvern::flow::infer_network_module_kind(
            &ProgramOperation::Custom("ssh_session".into()),
            Some("send_key_exchange_init"),
            Some("receive_server_banner->send_key_exchange_init"),
            "transport_io",
        ),
        "remote_access_session"
    );
}

#[test]
fn ssh_channel_session_operation_maps_to_remote_access_session_module_kind() {
    assert_eq!(
        gewyvern::flow::infer_network_module_kind(
            &ProgramOperation::Custom("ssh_channel_session".into()),
            Some("receive_channel_open_confirmation"),
            Some("send_channel_open->receive_channel_open_confirmation"),
            "transport_io",
        ),
        "remote_access_session"
    );
}

#[test]
fn smtp_auth_operation_maps_to_authentication_exchange_module_kind() {
    assert_eq!(
        gewyvern::flow::infer_network_module_kind(
            &ProgramOperation::Custom("smtp_auth".into()),
            Some("receive_auth_ok"),
            Some("send_auth_request->receive_auth_ok"),
            "transport_io",
        ),
        "authentication_exchange"
    );
}

#[test]
fn imap_auth_operation_maps_to_authentication_exchange_module_kind() {
    assert_eq!(
        gewyvern::flow::infer_network_module_kind(
            &ProgramOperation::Custom("imap_auth".into()),
            Some("receive_auth_ok"),
            Some("send_auth_request->receive_auth_ok"),
            "transport_io",
        ),
        "authentication_exchange"
    );
}

#[test]
fn imap_auth_denied_operation_maps_to_authentication_exchange_module_kind() {
    assert_eq!(
        gewyvern::flow::infer_network_module_kind(
            &ProgramOperation::Custom("imap_auth_denied".into()),
            Some("receive_auth_denied"),
            Some("send_auth_request->receive_auth_denied"),
            "transport_io",
        ),
        "authentication_exchange"
    );
}

#[test]
fn imap_select_operation_maps_to_mail_session_module_kind() {
    assert_eq!(
        gewyvern::flow::infer_network_module_kind(
            &ProgramOperation::Custom("imap_select".into()),
            Some("receive_mailbox_selected"),
            Some("send_select->receive_mailbox_selected"),
            "transport_io",
        ),
        "mail_session"
    );
}

#[test]
fn pop3_auth_operation_maps_to_authentication_exchange_module_kind() {
    assert_eq!(
        gewyvern::flow::infer_network_module_kind(
            &ProgramOperation::Custom("pop3_auth".into()),
            Some("receive_auth_ok"),
            Some("send_auth_pass->receive_auth_ok"),
            "transport_io",
        ),
        "authentication_exchange"
    );
}
