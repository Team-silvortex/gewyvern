use super::*;

#[test]
fn ssh_channel_session_path_does_not_treat_auth_success_as_channel_open_confirmation() {
    let binding = compile_file(&dsl_fixture_path("ssh_channel_session_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8287, 53028, "ssh-client"));
    session.ingest(route_fact(2, 8287, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8287, 1, 2, 53028, 22));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        8287,
        0x18,
        PacketDir::Ingress,
        Some(53028),
        Some(22),
        Some(0x53),
        Some(0x5353),
        Some(0x5353482d),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        8287,
        0x18,
        PacketDir::Egress,
        Some(53028),
        Some(22),
        Some(0x53),
        Some(0x5353),
        Some(0x5353482d),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        6,
        8287,
        0x18,
        PacketDir::Egress,
        Some(53028),
        Some(22),
        Some(0x00),
        None,
        None,
        Some(0x10),
        Some(0x14),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        7,
        8287,
        0x18,
        PacketDir::Egress,
        Some(53028),
        Some(22),
        Some(0x00),
        None,
        None,
        Some(0x10),
        Some(0x32),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        8,
        8287,
        0x18,
        PacketDir::Ingress,
        Some(53028),
        Some(22),
        Some(0x00),
        None,
        None,
        Some(0x10),
        Some(0x34),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(90));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_channel_open_confirmation"))
    );
}

#[test]
fn socks5_session_path_materializes_method_and_connect_phases() {
    let binding = compile_file(&dsl_fixture_path("socks5_session_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8283, 53180, "proxy-client"));
    session.ingest(route_fact(2, 8283, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8283, 1, 2, 53180, 1080));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        8283,
        0x18,
        PacketDir::Egress,
        Some(53180),
        Some(1080),
        Some(0x05),
        Some(0x0501),
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        8283,
        0x18,
        PacketDir::Ingress,
        Some(53180),
        Some(1080),
        Some(0x05),
        Some(0x0500),
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        6,
        8283,
        0x18,
        PacketDir::Egress,
        Some(53180),
        Some(1080),
        &[(0, 0x05), (1, 0x01), (2, 0x00), (3, 0x03)],
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        7,
        8283,
        0x18,
        PacketDir::Ingress,
        Some(53180),
        Some(1080),
        &[(0, 0x05), (1, 0x00), (2, 0x00), (3, 0x01)],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(90));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("socks5_session".into())
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
            .any(|stage| stage.phase.as_deref() == Some("send_method_greeting"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_method_selection"))
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
            .any(|stage| stage.phase.as_deref() == Some("receive_connect_success"))
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
fn socks5_session_path_does_not_treat_failed_reply_as_connect_success() {
    let binding = compile_file(&dsl_fixture_path("socks5_session_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8284, 53180, "proxy-client"));
    session.ingest(route_fact(2, 8284, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8284, 1, 2, 53180, 1080));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        8284,
        0x18,
        PacketDir::Egress,
        Some(53180),
        Some(1080),
        Some(0x05),
        Some(0x0501),
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        8284,
        0x18,
        PacketDir::Ingress,
        Some(53180),
        Some(1080),
        Some(0x05),
        Some(0x0500),
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        6,
        8284,
        0x18,
        PacketDir::Egress,
        Some(53180),
        Some(1080),
        &[(0, 0x05), (1, 0x01), (2, 0x00), (3, 0x03)],
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        7,
        8284,
        0x18,
        PacketDir::Ingress,
        Some(53180),
        Some(1080),
        &[(0, 0x05), (1, 0x05), (2, 0x00), (3, 0x01)],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(90));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_connect_success"))
    );
}

#[test]
fn socks5_auth_path_materializes_auth_and_connect_phases() {
    let binding = compile_file(&dsl_fixture_path("socks5_auth_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8295, 53134, "proxy-client"));
    session.ingest(route_fact(2, 8295, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8295, 1, 2, 53134, 1080));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        4,
        8295,
        0x18,
        PacketDir::Egress,
        Some(53134),
        Some(1080),
        &[(0, 0x05), (1, 0x01), (2, 0x02)],
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        5,
        8295,
        0x18,
        PacketDir::Ingress,
        Some(53134),
        Some(1080),
        &[(0, 0x05), (1, 0x02)],
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        6,
        8295,
        0x18,
        PacketDir::Egress,
        Some(53134),
        Some(1080),
        &[(0, 0x01), (1, 0x01)],
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        7,
        8295,
        0x18,
        PacketDir::Ingress,
        Some(53134),
        Some(1080),
        &[(0, 0x01), (1, 0x00)],
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        8,
        8295,
        0x18,
        PacketDir::Egress,
        Some(53134),
        Some(1080),
        &[(0, 0x05), (1, 0x01), (2, 0x00), (3, 0x03)],
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        9,
        8295,
        0x18,
        PacketDir::Ingress,
        Some(53134),
        Some(1080),
        &[(0, 0x05), (1, 0x00), (2, 0x00), (3, 0x01)],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("socks5_auth".into())
    );
    for phase in [
        "send_method_greeting",
        "receive_method_selection",
        "send_auth_request",
        "receive_auth_ok",
        "send_connect_request",
        "receive_connect_success",
    ] {
        assert!(
            export.program_flows[0]
                .stages
                .iter()
                .any(|stage| stage.phase.as_deref() == Some(phase)),
            "missing phase {phase:?}"
        );
    }
    assert_eq!(export.module_findings.len(), 0);
}

#[test]
fn socks5_auth_path_does_not_treat_failed_auth_reply_as_auth_ok() {
    let binding = compile_file(&dsl_fixture_path("socks5_auth_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8296, 53135, "proxy-client"));
    session.ingest(route_fact(2, 8296, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8296, 1, 2, 53135, 1080));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        4,
        8296,
        0x18,
        PacketDir::Egress,
        Some(53135),
        Some(1080),
        &[(0, 0x05), (1, 0x01), (2, 0x02)],
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        5,
        8296,
        0x18,
        PacketDir::Ingress,
        Some(53135),
        Some(1080),
        &[(0, 0x05), (1, 0x02)],
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        6,
        8296,
        0x18,
        PacketDir::Egress,
        Some(53135),
        Some(1080),
        &[(0, 0x01), (1, 0x01)],
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        7,
        8296,
        0x18,
        PacketDir::Ingress,
        Some(53135),
        Some(1080),
        &[(0, 0x01), (1, 0x01)],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_auth_ok"))
    );
}

#[test]
fn socks5_auth_denied_path_materializes_auth_denied_phase() {
    let binding = compile_file(&dsl_fixture_path("socks5_auth_denied_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8297, 53136, "proxy-client"));
    session.ingest(route_fact(2, 8297, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8297, 1, 2, 53136, 1080));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        4,
        8297,
        0x18,
        PacketDir::Egress,
        Some(53136),
        Some(1080),
        &[(0, 0x05), (1, 0x01), (2, 0x02)],
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        5,
        8297,
        0x18,
        PacketDir::Ingress,
        Some(53136),
        Some(1080),
        &[(0, 0x05), (1, 0x02)],
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        6,
        8297,
        0x18,
        PacketDir::Egress,
        Some(53136),
        Some(1080),
        &[(0, 0x01), (1, 0x01)],
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        7,
        8297,
        0x18,
        PacketDir::Ingress,
        Some(53136),
        Some(1080),
        &[(0, 0x01), (1, 0x01)],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_auth_denied"))
    );
    assert_eq!(export.module_findings.len(), 0);
}

#[test]
fn socks5_auth_connect_denied_path_materializes_denied_connect_after_auth() {
    let binding = compile_file(&dsl_fixture_path("socks5_auth_connect_denied_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 82975, 53137, "proxy-client"));
    session.ingest(route_fact(2, 82975, 7));
    session.ingest(tcp_state_fact_with_ports(3, 82975, 1, 2, 53137, 1080));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        4,
        82975,
        0x18,
        PacketDir::Egress,
        Some(53137),
        Some(1080),
        &[(0, 0x05), (1, 0x01), (2, 0x02)],
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        5,
        82975,
        0x18,
        PacketDir::Ingress,
        Some(53137),
        Some(1080),
        &[(0, 0x05), (1, 0x02)],
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        6,
        82975,
        0x18,
        PacketDir::Egress,
        Some(53137),
        Some(1080),
        &[(0, 0x01), (1, 0x01)],
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        7,
        82975,
        0x18,
        PacketDir::Ingress,
        Some(53137),
        Some(1080),
        &[(0, 0x01), (1, 0x00)],
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        8,
        82975,
        0x18,
        PacketDir::Egress,
        Some(53137),
        Some(1080),
        &[(0, 0x05), (1, 0x01), (2, 0x00), (3, 0x03)],
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        9,
        82975,
        0x18,
        PacketDir::Ingress,
        Some(53137),
        Some(1080),
        &[(0, 0x05), (1, 0x05), (2, 0x00), (3, 0x01)],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(90));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("socks5_auth_connect_denied".into())
    );
    for phase in [
        "send_auth_request",
        "receive_auth_ok",
        "send_connect_request",
        "receive_connect_denied",
    ] {
        assert!(
            export.program_flows[0]
                .stages
                .iter()
                .any(|stage| stage.phase.as_deref() == Some(phase)),
            "missing phase {phase:?}"
        );
    }
}
