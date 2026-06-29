use super::*;

#[test]
fn ftp_active_retr_path_does_not_match_wrong_port_ready_code() {
    let binding = compile_file(&dsl_fixture_path("ftp_active_retr_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8302, 53043, "ftp-client"));
    session.ingest(route_fact(2, 8302, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8302, 1, 2, 53043, 21));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        8302,
        0x18,
        PacketDir::Ingress,
        Some(53043),
        Some(21),
        Some(0x32),
        Some(0x3232),
        Some(0x32323020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        8302,
        0x18,
        PacketDir::Egress,
        Some(53043),
        Some(21),
        Some(0x55),
        Some(0x5553),
        Some(0x55534552),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        6,
        8302,
        0x18,
        PacketDir::Ingress,
        Some(53043),
        Some(21),
        Some(0x33),
        Some(0x3333),
        Some(0x33333120),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        7,
        8302,
        0x18,
        PacketDir::Egress,
        Some(53043),
        Some(21),
        Some(0x50),
        Some(0x5041),
        Some(0x50415353),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        8,
        8302,
        0x18,
        PacketDir::Ingress,
        Some(53043),
        Some(21),
        Some(0x32),
        Some(0x3233),
        Some(0x32333020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        9,
        8302,
        0x18,
        PacketDir::Egress,
        Some(53043),
        Some(21),
        Some(0x50),
        Some(0x504f),
        Some(0x504f5254),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        10,
        8302,
        0x18,
        PacketDir::Ingress,
        Some(53043),
        Some(21),
        Some(0x35),
        Some(0x3530),
        Some(0x35303020),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_port_ready"))
    );
}

#[test]
fn ftp_active_stor_path_materializes_port_and_stor_transfer_phases() {
    let binding = compile_file(&dsl_fixture_path("ftp_active_stor_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8303, 53044, "ftp-client"));
    session.ingest(route_fact(2, 8303, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8303, 1, 2, 53044, 21));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        8303,
        0x18,
        PacketDir::Ingress,
        Some(53044),
        Some(21),
        Some(0x32),
        Some(0x3232),
        Some(0x32323020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        8303,
        0x18,
        PacketDir::Egress,
        Some(53044),
        Some(21),
        Some(0x55),
        Some(0x5553),
        Some(0x55534552),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        6,
        8303,
        0x18,
        PacketDir::Ingress,
        Some(53044),
        Some(21),
        Some(0x33),
        Some(0x3333),
        Some(0x33333120),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        7,
        8303,
        0x18,
        PacketDir::Egress,
        Some(53044),
        Some(21),
        Some(0x50),
        Some(0x5041),
        Some(0x50415353),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        8,
        8303,
        0x18,
        PacketDir::Ingress,
        Some(53044),
        Some(21),
        Some(0x32),
        Some(0x3233),
        Some(0x32333020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        9,
        8303,
        0x18,
        PacketDir::Egress,
        Some(53044),
        Some(21),
        Some(0x50),
        Some(0x504f),
        Some(0x504f5254),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        10,
        8303,
        0x18,
        PacketDir::Ingress,
        Some(53044),
        Some(21),
        Some(0x32),
        Some(0x3230),
        Some(0x32303020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        11,
        8303,
        0x18,
        PacketDir::Egress,
        Some(53044),
        Some(21),
        Some(0x53),
        Some(0x5354),
        Some(0x53544f52),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        12,
        8303,
        0x18,
        PacketDir::Ingress,
        Some(53044),
        Some(21),
        Some(0x31),
        Some(0x3135),
        Some(0x31353020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        13,
        8303,
        0x18,
        PacketDir::Ingress,
        Some(53044),
        Some(21),
        Some(0x32),
        Some(0x3232),
        Some(0x32323620),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("ftp_active_stor".into())
    );
    for phase in [
        "send_port",
        "receive_port_ready",
        "send_stor",
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
fn ftp_active_stor_path_does_not_match_wrong_port_ready_code() {
    let binding = compile_file(&dsl_fixture_path("ftp_active_stor_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8304, 53045, "ftp-client"));
    session.ingest(route_fact(2, 8304, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8304, 1, 2, 53045, 21));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        8304,
        0x18,
        PacketDir::Ingress,
        Some(53045),
        Some(21),
        Some(0x32),
        Some(0x3232),
        Some(0x32323020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        8304,
        0x18,
        PacketDir::Egress,
        Some(53045),
        Some(21),
        Some(0x55),
        Some(0x5553),
        Some(0x55534552),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        6,
        8304,
        0x18,
        PacketDir::Ingress,
        Some(53045),
        Some(21),
        Some(0x33),
        Some(0x3333),
        Some(0x33333120),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        7,
        8304,
        0x18,
        PacketDir::Egress,
        Some(53045),
        Some(21),
        Some(0x50),
        Some(0x5041),
        Some(0x50415353),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        8,
        8304,
        0x18,
        PacketDir::Ingress,
        Some(53045),
        Some(21),
        Some(0x32),
        Some(0x3233),
        Some(0x32333020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        9,
        8304,
        0x18,
        PacketDir::Egress,
        Some(53045),
        Some(21),
        Some(0x50),
        Some(0x504f),
        Some(0x504f5254),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        10,
        8304,
        0x18,
        PacketDir::Ingress,
        Some(53045),
        Some(21),
        Some(0x35),
        Some(0x3530),
        Some(0x35303020),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_port_ready"))
    );
}

#[test]
fn ssh_session_path_does_not_treat_wrong_message_code_as_key_exchange_init() {
    let binding = compile_file(&dsl_fixture_path("ssh_session_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8282, 53022, "ssh-client"));
    session.ingest(route_fact(2, 8282, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8282, 1, 2, 53022, 22));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        8282,
        0x18,
        PacketDir::Ingress,
        Some(53022),
        Some(22),
        Some(0x53),
        Some(0x5353),
        Some(0x5353482d),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        8282,
        0x18,
        PacketDir::Egress,
        Some(53022),
        Some(22),
        Some(0x53),
        Some(0x5353),
        Some(0x5353482d),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        6,
        8282,
        0x18,
        PacketDir::Egress,
        Some(53022),
        Some(22),
        Some(0x00),
        None,
        None,
        Some(0x10),
        Some(0x15),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("send_key_exchange_init"))
    );
}

#[test]
fn ssh_auth_path_does_not_treat_auth_failure_as_success() {
    let binding = compile_file(&dsl_fixture_path("ssh_auth_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8285, 53026, "ssh-client"));
    session.ingest(route_fact(2, 8285, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8285, 1, 2, 53026, 22));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        8285,
        0x18,
        PacketDir::Ingress,
        Some(53026),
        Some(22),
        Some(0x53),
        Some(0x5353),
        Some(0x5353482d),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        8285,
        0x18,
        PacketDir::Egress,
        Some(53026),
        Some(22),
        Some(0x53),
        Some(0x5353),
        Some(0x5353482d),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        6,
        8285,
        0x18,
        PacketDir::Egress,
        Some(53026),
        Some(22),
        Some(0x00),
        None,
        None,
        Some(0x10),
        Some(0x14),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        7,
        8285,
        0x18,
        PacketDir::Egress,
        Some(53026),
        Some(22),
        Some(0x00),
        None,
        None,
        Some(0x10),
        Some(0x32),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        8,
        8285,
        0x18,
        PacketDir::Ingress,
        Some(53026),
        Some(22),
        Some(0x00),
        None,
        None,
        Some(0x10),
        Some(0x33),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_auth_success"))
    );
}
