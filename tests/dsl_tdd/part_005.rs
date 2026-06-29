use super::*;

#[test]
fn dns_dsl_does_not_treat_ingress_udp_as_lookup_request() {
    let binding = compile_file(&dsl_fixture_path("dns_udp_process.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 304, 5353, "dig"));
    session.ingest(route_fact(2, 304, 7));
    session.ingest(udp_packet_fact_with_dir(3, 304, 96, PacketDir::Ingress));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(40));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("send_request"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_reply"))
    );
    assert!(
        export.program_flows[0]
            .narrative
            .iter()
            .all(|line| line != "program emitted a UDP datagram")
    );
    assert!(export.program_findings.iter().any(|finding| {
        finding.phase.as_deref() == Some("send_request")
            && finding.phase_transition.as_deref() == Some("resolve->send_request")
    }));
}

#[test]
fn dns_dsl_missing_reply_produces_send_request_to_receive_reply_transition() {
    let binding = compile_file(&dsl_fixture_path("dns_udp_process.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 305, 5353, "dig"));
    session.ingest(route_fact(2, 305, 7));
    session.ingest(udp_packet_fact_with_dir(3, 305, 96, PacketDir::Egress));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(40));

    let export = session.export_bundle();
    assert!(export.program_findings.iter().any(|finding| {
        finding.phase.as_deref() == Some("receive_reply")
            && finding.phase_transition.as_deref() == Some("send_request->receive_reply")
    }));
    assert!(export.module_findings.iter().any(|finding| {
        finding
            .phase_transitions
            .contains(&"send_request->receive_reply".to_string())
    }));
}

#[test]
fn http_request_path_can_span_connect_and_request_response_phases_in_one_module() {
    let binding = compile_file(&dsl_fixture_path("http_request_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 601, 4242, "curl"));
    session.ingest(route_fact(2, 601, 7));
    session.ingest(tcp_state_fact_with_ports(3, 601, 1, 2, 42000, 443));
    session.ingest(tcp_state_fact_with_ports(4, 601, 2, 3, 42000, 443));
    session.ingest(packet_fact_with_dir(5, 601, 0x18, PacketDir::Egress));
    session.ingest(packet_fact_with_dir(6, 601, 0x18, PacketDir::Ingress));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("http_request".into())
    );
    let phases = export.program_flows[0]
        .stages
        .iter()
        .filter_map(|stage| stage.phase.clone())
        .collect::<Vec<_>>();
    assert!(phases.contains(&"bind".to_string()));
    assert!(phases.contains(&"resolve_upstream".to_string()));
    assert!(phases.contains(&"connect".to_string()));
    assert!(phases.contains(&"establish".to_string()));
    assert!(phases.contains(&"send_request".to_string()));
    assert!(phases.contains(&"receive_response".to_string()));
    let phase_kinds = export.program_flows[0]
        .stages
        .iter()
        .filter_map(|stage| stage.phase_kind.clone())
        .collect::<Vec<_>>();
    assert!(phase_kinds.contains(&"resolve_route".to_string()));
    assert!(phase_kinds.contains(&"initiate_connection".to_string()));
    assert!(phase_kinds.contains(&"establish_connection".to_string()));
    assert!(phase_kinds.contains(&"emit_payload".to_string()));
    assert!(phase_kinds.contains(&"receive_payload".to_string()));
    assert_eq!(export.module_findings.len(), 0);
}

#[test]
fn http_request_path_missing_establish_produces_connect_to_establish_transition() {
    let binding = compile_file(&dsl_fixture_path("http_request_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 602, 4242, "curl"));
    session.ingest(route_fact(2, 602, 7));
    session.ingest(tcp_state_fact_with_ports(3, 602, 1, 2, 42000, 443));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(50));

    let export = session.export_bundle();
    assert!(export.program_findings.iter().any(|finding| {
        finding.module_label == "http_request_path"
            && finding.phase.as_deref() == Some("establish")
            && finding.phase_transition.as_deref() == Some("connect->establish")
    }));
}

#[test]
fn http_request_path_missing_response_produces_request_to_response_transition() {
    let binding = compile_file(&dsl_fixture_path("http_request_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 603, 4242, "curl"));
    session.ingest(route_fact(2, 603, 7));
    session.ingest(tcp_state_fact_with_ports(3, 603, 1, 2, 42000, 443));
    session.ingest(tcp_state_fact_with_ports(4, 603, 2, 3, 42000, 443));
    session.ingest(packet_fact_with_dir(5, 603, 0x18, PacketDir::Egress));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(70));

    let export = session.export_bundle();
    assert!(export.program_findings.iter().any(|finding| {
        finding.module_label == "http_request_path"
            && finding.network_module_kind == "http_request_response"
            && finding.phase.as_deref() == Some("receive_response")
            && finding.phase_transition.as_deref() == Some("send_request->receive_response")
    }));
}

#[test]
fn http_server_response_path_can_span_accept_request_and_response_phases() {
    let binding = compile_file(&dsl_fixture_path("http_server_response_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 604, 8080, "nginx"));
    session.ingest(tcp_state_fact_with_ports(2, 604, 1, 2, 80, 53000));
    session.ingest(tcp_state_fact_with_ports(3, 604, 2, 3, 80, 53000));
    session.ingest(packet_fact_with_dir(4, 604, 0x18, PacketDir::Ingress));
    session.ingest(packet_fact_with_dir(5, 604, 0x18, PacketDir::Egress));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(70));

    let export = session.export_bundle();
    let phases = export.program_flows[0]
        .stages
        .iter()
        .filter_map(|stage| stage.phase.clone())
        .collect::<Vec<_>>();
    assert!(phases.contains(&"accept".to_string()));
    assert!(phases.contains(&"establish".to_string()));
    assert!(phases.contains(&"receive_request".to_string()));
    assert!(phases.contains(&"send_response".to_string()));
    assert_eq!(export.module_findings.len(), 0);
}

#[test]
fn http_server_response_path_missing_response_produces_request_to_response_transition() {
    let binding = compile_file(&dsl_fixture_path("http_server_response_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 605, 8080, "nginx"));
    session.ingest(tcp_state_fact_with_ports(2, 605, 1, 2, 80, 53000));
    session.ingest(tcp_state_fact_with_ports(3, 605, 2, 3, 80, 53000));
    session.ingest(packet_fact_with_dir(4, 605, 0x18, PacketDir::Ingress));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert!(export.program_findings.iter().any(|finding| {
        finding.module_label == "http_server_response_path"
            && finding.phase.as_deref() == Some("send_response")
            && finding.phase_transition.as_deref() == Some("receive_request->send_response")
    }));
}

#[test]
fn tls_client_path_materializes_transport_packet_phase() {
    let binding = compile_file(&dsl_fixture_path("tls_client_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 801, 4242, "curl"));
    session.ingest(route_fact(2, 801, 7));
    session.ingest(tcp_state_fact_with_ports(3, 801, 1, 2, 42310, 443));
    session.ingest(tcp_state_fact_with_ports(4, 801, 2, 3, 42310, 443));
    session.ingest(packet_fact(5, 801, 0x18));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("tls_client".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_client_hello"))
    );
    assert!(
        export.program_flows[0]
            .narrative
            .iter()
            .any(|line| line == "program sent transport payload on this network flow")
    );
    assert_eq!(export.module_findings.len(), 0);
}

#[test]
fn tls_client_path_missing_packet_phase_produces_establish_transition() {
    let binding = compile_file(&dsl_fixture_path("tls_client_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 802, 4242, "curl"));
    session.ingest(route_fact(2, 802, 7));
    session.ingest(tcp_state_fact_with_ports(3, 802, 1, 2, 42310, 443));
    session.ingest(tcp_state_fact_with_ports(4, 802, 2, 3, 42310, 443));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(50));

    let export = session.export_bundle();
    assert!(export.program_findings.iter().any(|finding| {
        finding.phase.as_deref() == Some("send_client_hello")
            && finding.phase_transition.as_deref() == Some("establish->send_client_hello")
    }));
}

#[test]
fn built_in_tls_server_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("tls_server_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "tls_server_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("tls_server".into())
    );
}

#[test]
fn tls_server_path_materializes_accept_and_server_hello_phases() {
    let binding = compile_file(&dsl_fixture_path("tls_server_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 811, 8443, "nginx"));
    session.ingest(tcp_state_fact_with_ports(2, 811, 1, 2, 443, 53000));
    session.ingest(tcp_state_fact_with_ports(3, 811, 2, 3, 443, 53000));
    session.ingest(packet_fact_with_dir(4, 811, 0x18, PacketDir::Ingress));
    session.ingest(packet_fact_with_dir(5, 811, 0x18, PacketDir::Egress));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(70));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("tls_server".into())
    );
    let phases = export.program_flows[0]
        .stages
        .iter()
        .filter_map(|stage| stage.phase.clone())
        .collect::<Vec<_>>();
    assert!(phases.contains(&"accept".to_string()));
    assert!(phases.contains(&"establish".to_string()));
    assert!(phases.contains(&"receive_client_hello".to_string()));
    assert!(phases.contains(&"send_server_hello".to_string()));
    assert_eq!(export.module_findings.len(), 0);
}

#[test]
fn tls_server_path_missing_server_hello_produces_receive_to_send_transition() {
    let binding = compile_file(&dsl_fixture_path("tls_server_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 812, 8443, "nginx"));
    session.ingest(tcp_state_fact_with_ports(2, 812, 1, 2, 443, 53000));
    session.ingest(tcp_state_fact_with_ports(3, 812, 2, 3, 443, 53000));
    session.ingest(packet_fact_with_dir(4, 812, 0x18, PacketDir::Ingress));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert!(export.program_findings.iter().any(|finding| {
        finding.phase.as_deref() == Some("send_server_hello")
            && finding.phase_transition.as_deref()
                == Some("receive_client_hello->send_server_hello")
    }));
}

#[test]
fn quic_client_initial_path_materializes_initial_and_handshake_datagrams() {
    let binding = compile_file(&dsl_fixture_path("quic_client_initial_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 803, 4242, "curl"));
    session.ingest(route_fact(2, 803, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        803,
        1280,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        Some(0xc3),
        Some(0xc300),
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        4,
        803,
        220,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        Some(0xe0),
        None,
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("quic_client_initial".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_initial"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_handshake"))
    );
    let phase_kinds = export.program_flows[0]
        .stages
        .iter()
        .filter_map(|stage| stage.phase_kind.clone())
        .collect::<Vec<_>>();
    assert!(phase_kinds.contains(&"emit_datagram".to_string()));
    assert!(phase_kinds.contains(&"receive_datagram".to_string()));
    assert!(
        export.program_flows[0]
            .narrative
            .iter()
            .any(|line| line == "program emitted a UDP datagram")
    );
    assert!(
        export.program_flows[0]
            .narrative
            .iter()
            .any(|line| line == "program received a UDP datagram")
    );
    assert_eq!(export.module_findings.len(), 0);
}

#[test]
fn quic_crypto_handshake_path_materializes_quic_crypto_stages() {
    let binding = compile_file(&dsl_fixture_path("quic_crypto_handshake_path.gewy")).unwrap();
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
        Some(0xc3),
        Some(0xc300),
    ));
    session.ingest(udp_quic_meta_fact(
        4,
        804,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Initial),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        5,
        804,
        220,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        Some(0xe0),
        None,
    ));
    session.ingest(udp_quic_meta_fact(
        6,
        804,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Handshake),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("quic_crypto_handshake".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_crypto"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_crypto"))
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
fn quic_crypto_handshake_path_does_not_treat_non_crypto_frames_as_crypto() {
    let binding = compile_file(&dsl_fixture_path("quic_crypto_handshake_path.gewy")).unwrap();
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
        Some(443),
        Some(0xc3),
        Some(0xc300),
    ));
    session.ingest(udp_quic_meta_fact(
        4,
        805,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Initial),
        vec![gewyvern::ir::QuicFrameType::Ack],
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        5,
        805,
        220,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        Some(0xe0),
        None,
    ));
    session.ingest(udp_quic_meta_fact(
        6,
        805,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Handshake),
        vec![gewyvern::ir::QuicFrameType::Ack],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("send_crypto"))
    );
}
