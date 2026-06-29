use super::*;

#[test]
fn ftp_session_path_does_not_match_wrong_login_success_code() {
    let binding = compile_file(&dsl_fixture_path("ftp_session_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8290, 53031, "ftp-client"));
    session.ingest(route_fact(2, 8290, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8290, 1, 2, 53031, 21));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        8290,
        0x18,
        PacketDir::Ingress,
        Some(53031),
        Some(21),
        Some(0x32),
        Some(0x3232),
        Some(0x32323020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        8290,
        0x18,
        PacketDir::Egress,
        Some(53031),
        Some(21),
        Some(0x55),
        Some(0x5553),
        Some(0x55534552),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        6,
        8290,
        0x18,
        PacketDir::Ingress,
        Some(53031),
        Some(21),
        Some(0x33),
        Some(0x3333),
        Some(0x33333120),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        7,
        8290,
        0x18,
        PacketDir::Egress,
        Some(53031),
        Some(21),
        Some(0x50),
        Some(0x5041),
        Some(0x50415353),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        8,
        8290,
        0x18,
        PacketDir::Ingress,
        Some(53031),
        Some(21),
        Some(0x35),
        Some(0x3533),
        Some(0x35333020),
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
fn ftp_denied_path_materializes_auth_denied_phase() {
    let binding = compile_file(&dsl_fixture_path("ftp_denied_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8291, 53032, "ftp-client"));
    session.ingest(route_fact(2, 8291, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8291, 1, 2, 53032, 21));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        8291,
        0x18,
        PacketDir::Ingress,
        Some(53032),
        Some(21),
        Some(0x32),
        Some(0x3232),
        Some(0x32323020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        8291,
        0x18,
        PacketDir::Egress,
        Some(53032),
        Some(21),
        Some(0x55),
        Some(0x5553),
        Some(0x55534552),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        6,
        8291,
        0x18,
        PacketDir::Ingress,
        Some(53032),
        Some(21),
        Some(0x33),
        Some(0x3333),
        Some(0x33333120),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        7,
        8291,
        0x18,
        PacketDir::Egress,
        Some(53032),
        Some(21),
        Some(0x50),
        Some(0x5041),
        Some(0x50415353),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        8,
        8291,
        0x18,
        PacketDir::Ingress,
        Some(53032),
        Some(21),
        Some(0x35),
        Some(0x3533),
        Some(0x35333020),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_auth_denied"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_password_required"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_auth_pass"))
    );
    assert_eq!(export.module_findings.len(), 0);
}

#[test]
fn ftp_denied_path_does_not_match_success_code() {
    let binding = compile_file(&dsl_fixture_path("ftp_denied_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8292, 53033, "ftp-client"));
    session.ingest(route_fact(2, 8292, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8292, 1, 2, 53033, 21));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        8292,
        0x18,
        PacketDir::Ingress,
        Some(53033),
        Some(21),
        Some(0x32),
        Some(0x3232),
        Some(0x32323020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        8292,
        0x18,
        PacketDir::Egress,
        Some(53033),
        Some(21),
        Some(0x55),
        Some(0x5553),
        Some(0x55534552),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        6,
        8292,
        0x18,
        PacketDir::Ingress,
        Some(53033),
        Some(21),
        Some(0x33),
        Some(0x3333),
        Some(0x33333120),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        7,
        8292,
        0x18,
        PacketDir::Egress,
        Some(53033),
        Some(21),
        Some(0x50),
        Some(0x5041),
        Some(0x50415353),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        8,
        8292,
        0x18,
        PacketDir::Ingress,
        Some(53033),
        Some(21),
        Some(0x32),
        Some(0x3233),
        Some(0x32333020),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_auth_denied"))
    );
}

#[test]
fn ftp_passive_list_path_materializes_pasv_and_list_transfer_phases() {
    let binding = compile_file(&dsl_fixture_path("ftp_passive_list_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8293, 53034, "ftp-client"));
    session.ingest(route_fact(2, 8293, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8293, 1, 2, 53034, 21));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        8293,
        0x18,
        PacketDir::Ingress,
        Some(53034),
        Some(21),
        Some(0x32),
        Some(0x3232),
        Some(0x32323020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        8293,
        0x18,
        PacketDir::Egress,
        Some(53034),
        Some(21),
        Some(0x55),
        Some(0x5553),
        Some(0x55534552),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        6,
        8293,
        0x18,
        PacketDir::Ingress,
        Some(53034),
        Some(21),
        Some(0x33),
        Some(0x3333),
        Some(0x33333120),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        7,
        8293,
        0x18,
        PacketDir::Egress,
        Some(53034),
        Some(21),
        Some(0x50),
        Some(0x5041),
        Some(0x50415353),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        8,
        8293,
        0x18,
        PacketDir::Ingress,
        Some(53034),
        Some(21),
        Some(0x32),
        Some(0x3233),
        Some(0x32333020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        9,
        8293,
        0x18,
        PacketDir::Egress,
        Some(53034),
        Some(21),
        Some(0x50),
        Some(0x5041),
        Some(0x50415356),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        10,
        8293,
        0x18,
        PacketDir::Ingress,
        Some(53034),
        Some(21),
        Some(0x32),
        Some(0x3232),
        Some(0x32323720),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        11,
        8293,
        0x18,
        PacketDir::Egress,
        Some(53034),
        Some(21),
        Some(0x4c),
        Some(0x4c49),
        Some(0x4c495354),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        12,
        8293,
        0x18,
        PacketDir::Ingress,
        Some(53034),
        Some(21),
        Some(0x31),
        Some(0x3135),
        Some(0x31353020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        13,
        8293,
        0x18,
        PacketDir::Ingress,
        Some(53034),
        Some(21),
        Some(0x32),
        Some(0x3232),
        Some(0x32323620),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("ftp_passive_list".into())
    );
    for phase in [
        "send_pasv",
        "receive_pasv_ready",
        "send_list",
        "receive_transfer_open",
        "receive_transfer_complete",
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
fn ftp_passive_list_path_does_not_match_wrong_pasv_reply_code() {
    let binding = compile_file(&dsl_fixture_path("ftp_passive_list_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8294, 53035, "ftp-client"));
    session.ingest(route_fact(2, 8294, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8294, 1, 2, 53035, 21));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        8294,
        0x18,
        PacketDir::Ingress,
        Some(53035),
        Some(21),
        Some(0x32),
        Some(0x3232),
        Some(0x32323020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        8294,
        0x18,
        PacketDir::Egress,
        Some(53035),
        Some(21),
        Some(0x55),
        Some(0x5553),
        Some(0x55534552),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        6,
        8294,
        0x18,
        PacketDir::Ingress,
        Some(53035),
        Some(21),
        Some(0x33),
        Some(0x3333),
        Some(0x33333120),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        7,
        8294,
        0x18,
        PacketDir::Egress,
        Some(53035),
        Some(21),
        Some(0x50),
        Some(0x5041),
        Some(0x50415353),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        8,
        8294,
        0x18,
        PacketDir::Ingress,
        Some(53035),
        Some(21),
        Some(0x32),
        Some(0x3233),
        Some(0x32333020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        9,
        8294,
        0x18,
        PacketDir::Egress,
        Some(53035),
        Some(21),
        Some(0x50),
        Some(0x5041),
        Some(0x50415356),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        10,
        8294,
        0x18,
        PacketDir::Ingress,
        Some(53035),
        Some(21),
        Some(0x35),
        Some(0x3533),
        Some(0x35333020),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_pasv_ready"))
    );
}
