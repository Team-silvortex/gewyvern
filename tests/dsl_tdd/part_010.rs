use super::*;

#[test]
fn coap_get_path_materializes_request_and_response_datagrams() {
    let binding = compile_file(&dsl_fixture_path("coap_get_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 811, 6000, "coap-client"));
    session.ingest(route_fact(2, 811, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        811,
        64,
        PacketDir::Egress,
        Some(56000),
        Some(5683),
        Some(0x40),
        Some(0x4001),
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        4,
        811,
        80,
        PacketDir::Ingress,
        Some(56000),
        Some(5683),
        Some(0x60),
        Some(0x6045),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("coap_get".into())
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
fn coap_get_path_does_not_match_wrong_response_code() {
    let binding = compile_file(&dsl_fixture_path("coap_get_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 812, 6000, "coap-client"));
    session.ingest(route_fact(2, 812, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        812,
        64,
        PacketDir::Egress,
        Some(56000),
        Some(5683),
        Some(0x40),
        Some(0x4001),
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        4,
        812,
        80,
        PacketDir::Ingress,
        Some(56000),
        Some(5683),
        Some(0x60),
        Some(0x6050),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_response"))
    );
}

#[test]
fn ntp_client_path_materializes_request_and_response_datagrams() {
    let binding = compile_file(&dsl_fixture_path("ntp_client_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 813, 7000, "chrony-client"));
    session.ingest(route_fact(2, 813, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        813,
        48,
        PacketDir::Egress,
        Some(53000),
        Some(123),
        Some(0x23),
        Some(0x2300),
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        4,
        813,
        48,
        PacketDir::Ingress,
        Some(53000),
        Some(123),
        Some(0x24),
        Some(0x2400),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("ntp_client".into())
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
fn ntp_client_path_does_not_match_wrong_response_mode() {
    let binding = compile_file(&dsl_fixture_path("ntp_client_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 814, 7000, "chrony-client"));
    session.ingest(route_fact(2, 814, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        814,
        48,
        PacketDir::Egress,
        Some(53000),
        Some(123),
        Some(0x23),
        Some(0x2300),
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        4,
        814,
        48,
        PacketDir::Ingress,
        Some(53000),
        Some(123),
        Some(0x25),
        Some(0x2500),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_response"))
    );
}

#[test]
fn gtpu_echo_path_materializes_request_and_response_datagrams() {
    let binding = compile_file(&dsl_fixture_path("gtpu_echo_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 813, 6001, "upf-agent"));
    session.ingest(route_fact(2, 813, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        813,
        64,
        PacketDir::Egress,
        Some(2152),
        Some(2152),
        Some(0x30),
        Some(0x3001),
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        4,
        813,
        64,
        PacketDir::Ingress,
        Some(2152),
        Some(2152),
        Some(0x30),
        Some(0x3002),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("gtpu_echo".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_echo_request"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_echo_response"))
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
fn gtpu_echo_path_does_not_match_wrong_response_type() {
    let binding = compile_file(&dsl_fixture_path("gtpu_echo_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 814, 6002, "upf-agent"));
    session.ingest(route_fact(2, 814, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        814,
        64,
        PacketDir::Egress,
        Some(2152),
        Some(2152),
        Some(0x30),
        Some(0x3001),
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        4,
        814,
        64,
        PacketDir::Ingress,
        Some(2152),
        Some(2152),
        Some(0x30),
        Some(0x3003),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_echo_response"))
    );
}

#[test]
fn dhcp_client_path_materializes_request_and_response_datagrams() {
    let binding = compile_file(&dsl_fixture_path("dhcp_client_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 815, 68, "dhclient"));
    session.ingest(route_fact(2, 815, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        815,
        300,
        PacketDir::Egress,
        Some(68),
        Some(67),
        Some(0x01),
        Some(0x0101),
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        4,
        815,
        300,
        PacketDir::Ingress,
        Some(68),
        Some(67),
        Some(0x02),
        Some(0x0201),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("dhcp_client".into())
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
fn dhcp_client_path_does_not_match_wrong_reply_opcode() {
    let binding = compile_file(&dsl_fixture_path("dhcp_client_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 816, 68, "dhclient"));
    session.ingest(route_fact(2, 816, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        816,
        300,
        PacketDir::Egress,
        Some(68),
        Some(67),
        Some(0x01),
        Some(0x0101),
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        4,
        816,
        300,
        PacketDir::Ingress,
        Some(68),
        Some(67),
        Some(0x01),
        Some(0x0101),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_offer"))
    );
}

#[test]
fn wireguard_handshake_path_materializes_initiation_and_response_datagrams() {
    let binding = compile_file(&dsl_fixture_path("wireguard_handshake_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 817, 53000, "wg-quick"));
    session.ingest(route_fact(2, 817, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        817,
        148,
        PacketDir::Egress,
        Some(53000),
        Some(51820),
        Some(0x01),
        Some(0x0100),
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        4,
        817,
        92,
        PacketDir::Ingress,
        Some(53000),
        Some(51820),
        Some(0x02),
        Some(0x0200),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("wireguard_handshake".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_initiation"))
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
fn wireguard_handshake_path_does_not_match_wrong_response_type() {
    let binding = compile_file(&dsl_fixture_path("wireguard_handshake_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 818, 53000, "wg-quick"));
    session.ingest(route_fact(2, 818, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        818,
        148,
        PacketDir::Egress,
        Some(53000),
        Some(51820),
        Some(0x01),
        Some(0x0100),
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        4,
        818,
        64,
        PacketDir::Ingress,
        Some(53000),
        Some(51820),
        Some(0x04),
        Some(0x0400),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_response"))
    );
}
