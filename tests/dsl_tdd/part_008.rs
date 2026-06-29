use super::*;

#[test]
fn pop3_auth_denied_operation_maps_to_authentication_exchange_module_kind() {
    assert_eq!(
        gewyvern::flow::infer_network_module_kind(
            &ProgramOperation::Custom("pop3_auth_denied".into()),
            Some("receive_auth_denied"),
            Some("send_auth_pass->receive_auth_denied"),
            "transport_io",
        ),
        "authentication_exchange"
    );
}

#[test]
fn pop3_list_operation_maps_to_mail_session_module_kind() {
    assert_eq!(
        gewyvern::flow::infer_network_module_kind(
            &ProgramOperation::Custom("pop3_list".into()),
            Some("receive_list_ready"),
            Some("send_list->receive_list_ready"),
            "transport_io",
        ),
        "mail_session"
    );
}

#[test]
fn kerberos_as_operation_maps_to_authentication_exchange_module_kind() {
    assert_eq!(
        gewyvern::flow::infer_network_module_kind(
            &ProgramOperation::Custom("kerberos_as".into()),
            Some("receive_as_reply"),
            Some("send_as_request->receive_as_reply"),
            "transport_io",
        ),
        "authentication_exchange"
    );
}

#[test]
fn kerberos_as_error_operation_maps_to_authentication_exchange_module_kind() {
    assert_eq!(
        gewyvern::flow::infer_network_module_kind(
            &ProgramOperation::Custom("kerberos_as_error".into()),
            Some("receive_error"),
            Some("send_as_request->receive_error"),
            "transport_io",
        ),
        "authentication_exchange"
    );
}

#[test]
fn kerberos_tgs_operation_maps_to_ticket_granting_module_kind() {
    assert_eq!(
        gewyvern::flow::infer_network_module_kind(
            &ProgramOperation::Custom("kerberos_tgs".into()),
            Some("receive_tgs_reply"),
            Some("send_tgs_request->receive_tgs_reply"),
            "transport_io",
        ),
        "ticket_granting"
    );
}

#[test]
fn rtsp_options_operation_maps_to_signaling_session_module_kind() {
    assert_eq!(
        gewyvern::flow::infer_network_module_kind(
            &ProgramOperation::Custom("rtsp_options".into()),
            Some("receive_options_ok"),
            Some("send_options->receive_options_ok"),
            "transport_io",
        ),
        "signaling_session"
    );
}

#[test]
fn rtsp_describe_operation_maps_to_signaling_session_module_kind() {
    assert_eq!(
        gewyvern::flow::infer_network_module_kind(
            &ProgramOperation::Custom("rtsp_describe".into()),
            Some("receive_describe_ok"),
            Some("send_describe->receive_describe_ok"),
            "transport_io",
        ),
        "signaling_session"
    );
}

#[test]
fn rtsp_setup_operation_maps_to_signaling_session_module_kind() {
    assert_eq!(
        gewyvern::flow::infer_network_module_kind(
            &ProgramOperation::Custom("rtsp_setup".into()),
            Some("receive_setup_ok"),
            Some("send_setup->receive_setup_ok"),
            "transport_io",
        ),
        "signaling_session"
    );
}

#[test]
fn smtp_mail_operation_maps_to_mail_session_module_kind() {
    assert_eq!(
        gewyvern::flow::infer_network_module_kind(
            &ProgramOperation::Custom("smtp_mail".into()),
            Some("receive_mail_ok"),
            Some("send_mail_from->receive_mail_ok"),
            "transport_io",
        ),
        "mail_session"
    );
}

#[test]
fn smtp_rcpt_operation_maps_to_mail_session_module_kind() {
    assert_eq!(
        gewyvern::flow::infer_network_module_kind(
            &ProgramOperation::Custom("smtp_rcpt".into()),
            Some("receive_rcpt_ok"),
            Some("send_rcpt_to->receive_rcpt_ok"),
            "transport_io",
        ),
        "mail_session"
    );
}

#[test]
fn smtp_data_operation_maps_to_mail_session_module_kind() {
    assert_eq!(
        gewyvern::flow::infer_network_module_kind(
            &ProgramOperation::Custom("smtp_data".into()),
            Some("receive_message_queued"),
            Some("send_message_body->receive_message_queued"),
            "transport_io",
        ),
        "mail_session"
    );
}

#[test]
fn smtp_data_denied_operation_maps_to_mail_session_module_kind() {
    assert_eq!(
        gewyvern::flow::infer_network_module_kind(
            &ProgramOperation::Custom("smtp_data_denied".into()),
            Some("receive_message_denied"),
            Some("send_message_body->receive_message_denied"),
            "transport_io",
        ),
        "mail_session"
    );
}

#[test]
fn smtp_rcpt_denied_operation_maps_to_mail_session_module_kind() {
    assert_eq!(
        gewyvern::flow::infer_network_module_kind(
            &ProgramOperation::Custom("smtp_rcpt_denied".into()),
            Some("receive_rcpt_denied"),
            Some("send_rcpt_to->receive_rcpt_denied"),
            "transport_io",
        ),
        "mail_session"
    );
}

#[test]
fn ssh_auth_operation_maps_to_remote_access_authentication_module_kind() {
    assert_eq!(
        gewyvern::flow::infer_network_module_kind(
            &ProgramOperation::Custom("ssh_auth".into()),
            Some("receive_auth_success"),
            Some("send_auth_request->receive_auth_success"),
            "transport_io",
        ),
        "remote_access_authentication"
    );
}

#[test]
fn socks5_session_operation_maps_to_proxy_negotiation_module_kind() {
    assert_eq!(
        gewyvern::flow::infer_network_module_kind(
            &ProgramOperation::Custom("socks5_session".into()),
            Some("receive_connect_success"),
            Some("send_connect_request->receive_connect_success"),
            "transport_io",
        ),
        "proxy_negotiation"
    );
}

#[test]
fn socks5_auth_operation_maps_to_proxy_authentication_module_kind() {
    assert_eq!(
        gewyvern::flow::infer_network_module_kind(
            &ProgramOperation::Custom("socks5_auth".into()),
            Some("receive_auth_ok"),
            Some("send_auth_request->receive_auth_ok"),
            "transport_io"
        ),
        "proxy_authentication"
    );
}

#[test]
fn socks5_auth_connect_denied_operation_maps_to_proxy_negotiation_module_kind() {
    assert_eq!(
        gewyvern::flow::infer_network_module_kind(
            &ProgramOperation::Custom("socks5_auth_connect_denied".into()),
            Some("receive_connect_denied"),
            Some("send_connect_request->receive_connect_denied"),
            "transport_io"
        ),
        "proxy_negotiation"
    );
}

#[test]
fn http_connect_tunnel_operation_maps_to_proxy_tunnel_establishment_module_kind() {
    assert_eq!(
        gewyvern::flow::infer_network_module_kind(
            &ProgramOperation::Custom("http_connect_tunnel".into()),
            Some("receive_connect_established"),
            Some("send_connect_request->receive_connect_established"),
            "transport_io",
        ),
        "proxy_tunnel_establishment"
    );
}

#[test]
fn http_connect_auth_required_operation_maps_to_proxy_authentication_module_kind() {
    assert_eq!(
        gewyvern::flow::infer_network_module_kind(
            &ProgramOperation::Custom("http_connect_auth_required".into()),
            Some("receive_auth_required"),
            Some("send_connect_request->receive_auth_required"),
            "transport_io",
        ),
        "proxy_authentication"
    );
}

#[test]
fn http_connect_authenticated_tunnel_operation_maps_to_proxy_authentication_module_kind() {
    assert_eq!(
        gewyvern::flow::infer_network_module_kind(
            &ProgramOperation::Custom("http_connect_authenticated_tunnel".into()),
            Some("receive_connect_established"),
            Some("send_connect_request->receive_connect_established"),
            "transport_io",
        ),
        "proxy_authentication"
    );
}

#[test]
fn hy2_udp_relay_path_materializes_auth_and_datagram_stages() {
    let binding = compile_file(&dsl_fixture_path("hy2_udp_relay_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 816, 4242, "hysteria"));
    session.ingest(route_fact(2, 816, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        816,
        1280,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        Some(0xc3),
        Some(0xc300),
    ));
    session.ingest(udp_quic_meta_fact(
        4,
        816,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Initial),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        5,
        816,
        220,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        Some(0xe0),
        None,
    ));
    session.ingest(udp_quic_meta_fact(
        6,
        816,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Handshake),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_quic_meta_fact(
        7,
        816,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Stream],
    ));
    session.ingest(udp_quic_meta_fact(
        8,
        816,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Stream],
    ));
    session.ingest(udp_quic_meta_fact(
        9,
        816,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Datagram],
    ));
    session.ingest(udp_quic_meta_fact(
        10,
        816,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Datagram],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(120));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("hy2_udp_relay".into())
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
fn hy2_udp_relay_path_does_not_treat_stream_as_udp_datagram() {
    let binding = compile_file(&dsl_fixture_path("hy2_udp_relay_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 817, 4242, "hysteria"));
    session.ingest(route_fact(2, 817, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        817,
        1280,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        Some(0xc3),
        Some(0xc300),
    ));
    session.ingest(udp_quic_meta_fact(
        4,
        817,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Initial),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        5,
        817,
        220,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        Some(0xe0),
        None,
    ));
    session.ingest(udp_quic_meta_fact(
        6,
        817,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Handshake),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_quic_meta_fact(
        7,
        817,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Stream],
    ));
    session.ingest(udp_quic_meta_fact(
        8,
        817,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Stream],
    ));
    session.ingest(udp_quic_meta_fact(
        9,
        817,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Stream],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(110));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_udp_relay_datagram"))
    );
}
