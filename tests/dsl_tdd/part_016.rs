use super::*;

#[test]
fn smtp_rcpt_denied_path_does_not_match_success_rcpt_response() {
    let binding = compile_file(&dsl_fixture_path("smtp_rcpt_denied_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8309, 53023, "postfix-client"));
    session.ingest(route_fact(2, 8309, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8309, 1, 2, 53023, 25));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        8309,
        0x18,
        PacketDir::Ingress,
        Some(53023),
        Some(25),
        Some(0x32),
        Some(0x3232),
        Some(0x32323020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        8309,
        0x18,
        PacketDir::Egress,
        Some(53023),
        Some(25),
        Some(0x45),
        Some(0x4548),
        Some(0x45484c4f),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        6,
        8309,
        0x18,
        PacketDir::Ingress,
        Some(53023),
        Some(25),
        Some(0x32),
        Some(0x3235),
        Some(0x32353020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        7,
        8309,
        0x18,
        PacketDir::Egress,
        Some(53023),
        Some(25),
        Some(0x41),
        Some(0x4155),
        Some(0x41555448),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        8,
        8309,
        0x18,
        PacketDir::Ingress,
        Some(53023),
        Some(25),
        Some(0x32),
        Some(0x3233),
        Some(0x32333520),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        9,
        8309,
        0x18,
        PacketDir::Egress,
        Some(53023),
        Some(25),
        Some(0x4d),
        Some(0x4d41),
        Some(0x4d41494c),
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        10,
        8309,
        0x18,
        PacketDir::Ingress,
        Some(53023),
        Some(25),
        &[
            (0, 0x32),
            (1, 0x35),
            (2, 0x30),
            (3, 0x20),
            (4, 0x32),
            (5, 0x2e),
            (6, 0x31),
            (7, 0x2e),
        ],
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        11,
        8309,
        0x18,
        PacketDir::Egress,
        Some(53023),
        Some(25),
        Some(0x52),
        Some(0x5243),
        Some(0x52435054),
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        12,
        8309,
        0x18,
        PacketDir::Ingress,
        Some(53023),
        Some(25),
        &[
            (0, 0x32),
            (1, 0x35),
            (2, 0x30),
            (3, 0x20),
            (4, 0x32),
            (5, 0x2e),
            (6, 0x31),
            (7, 0x2e),
            (8, 0x35),
        ],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(110));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_rcpt_denied"))
    );
}

#[test]
fn ssh_auth_path_materializes_auth_request_and_success_phases() {
    let binding = compile_file(&dsl_fixture_path("ssh_auth_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8283, 53024, "ssh-client"));
    session.ingest(route_fact(2, 8283, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8283, 1, 2, 53024, 22));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        8283,
        0x18,
        PacketDir::Ingress,
        Some(53024),
        Some(22),
        Some(0x53),
        Some(0x5353),
        Some(0x5353482d),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        8283,
        0x18,
        PacketDir::Egress,
        Some(53024),
        Some(22),
        Some(0x53),
        Some(0x5353),
        Some(0x5353482d),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        6,
        8283,
        0x18,
        PacketDir::Egress,
        Some(53024),
        Some(22),
        Some(0x00),
        None,
        None,
        Some(0x10),
        Some(0x14),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        7,
        8283,
        0x18,
        PacketDir::Egress,
        Some(53024),
        Some(22),
        Some(0x00),
        None,
        None,
        Some(0x10),
        Some(0x32),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        8,
        8283,
        0x18,
        PacketDir::Ingress,
        Some(53024),
        Some(22),
        Some(0x00),
        None,
        None,
        Some(0x10),
        Some(0x34),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("ssh_auth".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_auth_request"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_auth_success"))
    );
}

#[test]
fn ssh_auth_denied_path_materializes_auth_denied_phase() {
    let binding = compile_file(&dsl_fixture_path("ssh_auth_denied_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8284, 53025, "ssh-client"));
    session.ingest(route_fact(2, 8284, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8284, 1, 2, 53025, 22));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        8284,
        0x18,
        PacketDir::Ingress,
        Some(53025),
        Some(22),
        Some(0x53),
        Some(0x5353),
        Some(0x5353482d),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        8284,
        0x18,
        PacketDir::Egress,
        Some(53025),
        Some(22),
        Some(0x53),
        Some(0x5353),
        Some(0x5353482d),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        6,
        8284,
        0x18,
        PacketDir::Egress,
        Some(53025),
        Some(22),
        Some(0x00),
        None,
        None,
        Some(0x10),
        Some(0x14),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        7,
        8284,
        0x18,
        PacketDir::Egress,
        Some(53025),
        Some(22),
        Some(0x00),
        None,
        None,
        Some(0x10),
        Some(0x32),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        8,
        8284,
        0x18,
        PacketDir::Ingress,
        Some(53025),
        Some(22),
        Some(0x00),
        None,
        None,
        Some(0x10),
        Some(0x33),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("ssh_auth_denied".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_auth_denied"))
    );
}

#[test]
fn ssh_channel_session_path_materializes_auth_and_channel_phases() {
    let binding = compile_file(&dsl_fixture_path("ssh_channel_session_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8286, 53027, "ssh-client"));
    session.ingest(route_fact(2, 8286, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8286, 1, 2, 53027, 22));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        8286,
        0x18,
        PacketDir::Ingress,
        Some(53027),
        Some(22),
        Some(0x53),
        Some(0x5353),
        Some(0x5353482d),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        8286,
        0x18,
        PacketDir::Egress,
        Some(53027),
        Some(22),
        Some(0x53),
        Some(0x5353),
        Some(0x5353482d),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        6,
        8286,
        0x18,
        PacketDir::Egress,
        Some(53027),
        Some(22),
        Some(0x00),
        None,
        None,
        Some(0x10),
        Some(0x14),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        7,
        8286,
        0x18,
        PacketDir::Egress,
        Some(53027),
        Some(22),
        Some(0x00),
        None,
        None,
        Some(0x10),
        Some(0x32),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        8,
        8286,
        0x18,
        PacketDir::Ingress,
        Some(53027),
        Some(22),
        Some(0x00),
        None,
        None,
        Some(0x10),
        Some(0x34),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        9,
        8286,
        0x18,
        PacketDir::Egress,
        Some(53027),
        Some(22),
        Some(0x00),
        None,
        None,
        Some(0x10),
        Some(0x5a),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        10,
        8286,
        0x18,
        PacketDir::Ingress,
        Some(53027),
        Some(22),
        Some(0x00),
        None,
        None,
        Some(0x10),
        Some(0x5b),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(90));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("ssh_channel_session".into())
    );
    for phase in [
        "send_auth_request",
        "receive_auth_success",
        "send_channel_open",
        "receive_channel_open_confirmation",
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

#[test]
fn smtp_session_path_does_not_match_wrong_banner_prefix() {
    let binding = compile_file(&dsl_fixture_path("smtp_session_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 828, 53010, "postfix-client"));
    session.ingest(route_fact(2, 828, 7));
    session.ingest(tcp_state_fact_with_ports(3, 828, 1, 2, 53010, 25));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        828,
        0x18,
        PacketDir::Ingress,
        Some(53010),
        Some(25),
        Some(0x35),
        Some(0x3535),
        Some(0x35353020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        828,
        0x18,
        PacketDir::Egress,
        Some(53010),
        Some(25),
        Some(0x45),
        Some(0x4548),
        Some(0x45484c4f),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(70));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_banner"))
    );
}
