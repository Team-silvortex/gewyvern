use super::*;

#[test]
fn socks5_auth_connect_denied_path_does_not_treat_auth_failure_as_connect_denied() {
    let binding = compile_file(&dsl_fixture_path("socks5_auth_connect_denied_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 82976, 53138, "proxy-client"));
    session.ingest(route_fact(2, 82976, 7));
    session.ingest(tcp_state_fact_with_ports(3, 82976, 1, 2, 53138, 1080));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        4,
        82976,
        0x18,
        PacketDir::Egress,
        Some(53138),
        Some(1080),
        &[(0, 0x05), (1, 0x01), (2, 0x02)],
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        5,
        82976,
        0x18,
        PacketDir::Ingress,
        Some(53138),
        Some(1080),
        &[(0, 0x05), (1, 0x02)],
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        6,
        82976,
        0x18,
        PacketDir::Egress,
        Some(53138),
        Some(1080),
        &[(0, 0x01), (1, 0x01)],
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        7,
        82976,
        0x18,
        PacketDir::Ingress,
        Some(53138),
        Some(1080),
        &[(0, 0x01), (1, 0x01)],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(90));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_connect_denied"))
    );
}

#[test]
fn socks5_denied_path_materializes_denied_connect_phase() {
    let binding = compile_file(&dsl_fixture_path("socks5_denied_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8287, 53182, "proxy-client"));
    session.ingest(route_fact(2, 8287, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8287, 1, 2, 53182, 1080));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        8287,
        0x18,
        PacketDir::Egress,
        Some(53182),
        Some(1080),
        Some(0x05),
        Some(0x0501),
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        8287,
        0x18,
        PacketDir::Ingress,
        Some(53182),
        Some(1080),
        Some(0x05),
        Some(0x0500),
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        6,
        8287,
        0x18,
        PacketDir::Egress,
        Some(53182),
        Some(1080),
        &[(0, 0x05), (1, 0x01), (2, 0x00), (3, 0x03)],
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        7,
        8287,
        0x18,
        PacketDir::Ingress,
        Some(53182),
        Some(1080),
        &[(0, 0x05), (1, 0x05), (2, 0x00), (3, 0x01)],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(90));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("socks5_denied".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_connect_request"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_connect_denied"))
    );
    assert_eq!(export.module_findings.len(), 1);
}

#[test]
fn socks5_denied_path_does_not_match_success_reply() {
    let binding = compile_file(&dsl_fixture_path("socks5_denied_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8288, 53182, "proxy-client"));
    session.ingest(route_fact(2, 8288, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8288, 1, 2, 53182, 1080));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        8288,
        0x18,
        PacketDir::Egress,
        Some(53182),
        Some(1080),
        Some(0x05),
        Some(0x0501),
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        8288,
        0x18,
        PacketDir::Ingress,
        Some(53182),
        Some(1080),
        Some(0x05),
        Some(0x0500),
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        6,
        8288,
        0x18,
        PacketDir::Egress,
        Some(53182),
        Some(1080),
        &[(0, 0x05), (1, 0x01), (2, 0x00), (3, 0x03)],
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        7,
        8288,
        0x18,
        PacketDir::Ingress,
        Some(53182),
        Some(1080),
        &[(0, 0x05), (1, 0x00), (2, 0x00), (3, 0x01)],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(90));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_connect_denied"))
    );
}

#[test]
fn http_connect_tunnel_path_materializes_connect_request_and_established_response() {
    let binding = compile_file(&dsl_fixture_path("http_connect_tunnel_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8285, 53181, "proxy-client"));
    session.ingest(route_fact(2, 8285, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8285, 1, 2, 53181, 8080));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        8285,
        0x18,
        PacketDir::Egress,
        Some(53181),
        Some(8080),
        Some(0x43),
        Some(0x434f),
        Some(0x434f4e4e),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        8285,
        0x18,
        PacketDir::Ingress,
        Some(53181),
        Some(8080),
        Some(0x32),
        Some(0x3230),
        Some(0x32303020),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("http_connect_tunnel".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("connect"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_connect_request"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_connect_established"))
    );
    let phase_kinds = export.program_flows[0]
        .stages
        .iter()
        .filter_map(|stage| stage.phase_kind.clone())
        .collect::<Vec<_>>();
    assert!(phase_kinds.contains(&"initiate_connection".to_string()));
    assert!(phase_kinds.contains(&"receive_payload".to_string()));
    assert!(phase_kinds.contains(&"emit_payload".to_string()));
    assert_eq!(export.module_findings.len(), 0);
}

#[test]
fn http_connect_tunnel_path_does_not_treat_non_200_response_as_established() {
    let binding = compile_file(&dsl_fixture_path("http_connect_tunnel_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8286, 53181, "proxy-client"));
    session.ingest(route_fact(2, 8286, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8286, 1, 2, 53181, 8080));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        8286,
        0x18,
        PacketDir::Egress,
        Some(53181),
        Some(8080),
        Some(0x43),
        Some(0x434f),
        Some(0x434f4e4e),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        8286,
        0x18,
        PacketDir::Ingress,
        Some(53181),
        Some(8080),
        Some(0x34),
        Some(0x3430),
        Some(0x34303320),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_connect_established"))
    );
}

#[test]
fn http_connect_denied_path_materializes_denied_tunnel_phase() {
    let binding = compile_file(&dsl_fixture_path("http_connect_denied_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8289, 53183, "proxy-client"));
    session.ingest(route_fact(2, 8289, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8289, 1, 2, 53183, 8080));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        8289,
        0x18,
        PacketDir::Egress,
        Some(53183),
        Some(8080),
        Some(0x43),
        Some(0x434f),
        Some(0x434f4e4e),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        8289,
        0x18,
        PacketDir::Ingress,
        Some(53183),
        Some(8080),
        Some(0x34),
        Some(0x3430),
        Some(0x34303320),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("http_connect_denied".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_connect_request"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_connect_denied"))
    );
    assert_eq!(export.module_findings.len(), 0);
}

#[test]
fn http_connect_denied_path_does_not_match_200_response() {
    let binding = compile_file(&dsl_fixture_path("http_connect_denied_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8290, 53183, "proxy-client"));
    session.ingest(route_fact(2, 8290, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8290, 1, 2, 53183, 8080));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        8290,
        0x18,
        PacketDir::Egress,
        Some(53183),
        Some(8080),
        Some(0x43),
        Some(0x434f),
        Some(0x434f4e4e),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        8290,
        0x18,
        PacketDir::Ingress,
        Some(53183),
        Some(8080),
        Some(0x32),
        Some(0x3230),
        Some(0x32303020),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_connect_denied"))
    );
}

#[test]
fn http_connect_auth_required_path_materializes_proxy_auth_phase() {
    let binding = compile_file(&dsl_fixture_path("http_connect_auth_required_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8291, 53184, "proxy-client"));
    session.ingest(route_fact(2, 8291, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8291, 1, 2, 53184, 8080));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        8291,
        0x18,
        PacketDir::Egress,
        Some(53184),
        Some(8080),
        Some(0x43),
        Some(0x434f),
        Some(0x434f4e4e),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        8291,
        0x18,
        PacketDir::Ingress,
        Some(53184),
        Some(8080),
        Some(0x34),
        Some(0x3430),
        Some(0x34303720),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("http_connect_auth_required".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_auth_required"))
    );
    assert_eq!(export.module_findings.len(), 0);
}

#[test]
fn http_connect_auth_required_path_does_not_match_403_response() {
    let binding = compile_file(&dsl_fixture_path("http_connect_auth_required_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8292, 53184, "proxy-client"));
    session.ingest(route_fact(2, 8292, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8292, 1, 2, 53184, 8080));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        8292,
        0x18,
        PacketDir::Egress,
        Some(53184),
        Some(8080),
        Some(0x43),
        Some(0x434f),
        Some(0x434f4e4e),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        8292,
        0x18,
        PacketDir::Ingress,
        Some(53184),
        Some(8080),
        Some(0x34),
        Some(0x3430),
        Some(0x34303320),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_auth_required"))
    );
}
