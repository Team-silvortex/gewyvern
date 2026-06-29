use super::*;

#[test]
fn ftp_retr_path_materializes_pasv_and_retr_transfer_phases() {
    let binding = compile_file(&dsl_fixture_path("ftp_retr_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8295, 53036, "ftp-client"));
    session.ingest(route_fact(2, 8295, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8295, 1, 2, 53036, 21));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        8295,
        0x18,
        PacketDir::Ingress,
        Some(53036),
        Some(21),
        Some(0x32),
        Some(0x3232),
        Some(0x32323020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        8295,
        0x18,
        PacketDir::Egress,
        Some(53036),
        Some(21),
        Some(0x55),
        Some(0x5553),
        Some(0x55534552),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        6,
        8295,
        0x18,
        PacketDir::Ingress,
        Some(53036),
        Some(21),
        Some(0x33),
        Some(0x3333),
        Some(0x33333120),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        7,
        8295,
        0x18,
        PacketDir::Egress,
        Some(53036),
        Some(21),
        Some(0x50),
        Some(0x5041),
        Some(0x50415353),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        8,
        8295,
        0x18,
        PacketDir::Ingress,
        Some(53036),
        Some(21),
        Some(0x32),
        Some(0x3233),
        Some(0x32333020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        9,
        8295,
        0x18,
        PacketDir::Egress,
        Some(53036),
        Some(21),
        Some(0x50),
        Some(0x5041),
        Some(0x50415356),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        10,
        8295,
        0x18,
        PacketDir::Ingress,
        Some(53036),
        Some(21),
        Some(0x32),
        Some(0x3232),
        Some(0x32323720),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        11,
        8295,
        0x18,
        PacketDir::Egress,
        Some(53036),
        Some(21),
        Some(0x52),
        Some(0x5245),
        Some(0x52455452),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        12,
        8295,
        0x18,
        PacketDir::Ingress,
        Some(53036),
        Some(21),
        Some(0x31),
        Some(0x3135),
        Some(0x31353020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        13,
        8295,
        0x18,
        PacketDir::Ingress,
        Some(53036),
        Some(21),
        Some(0x32),
        Some(0x3232),
        Some(0x32323620),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("ftp_retr".into())
    );
    for phase in [
        "send_pasv",
        "receive_pasv_ready",
        "send_retr",
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
fn ftp_retr_path_does_not_match_wrong_transfer_open_code() {
    let binding = compile_file(&dsl_fixture_path("ftp_retr_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8296, 53037, "ftp-client"));
    session.ingest(route_fact(2, 8296, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8296, 1, 2, 53037, 21));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        8296,
        0x18,
        PacketDir::Ingress,
        Some(53037),
        Some(21),
        Some(0x32),
        Some(0x3232),
        Some(0x32323020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        8296,
        0x18,
        PacketDir::Egress,
        Some(53037),
        Some(21),
        Some(0x55),
        Some(0x5553),
        Some(0x55534552),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        6,
        8296,
        0x18,
        PacketDir::Ingress,
        Some(53037),
        Some(21),
        Some(0x33),
        Some(0x3333),
        Some(0x33333120),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        7,
        8296,
        0x18,
        PacketDir::Egress,
        Some(53037),
        Some(21),
        Some(0x50),
        Some(0x5041),
        Some(0x50415353),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        8,
        8296,
        0x18,
        PacketDir::Ingress,
        Some(53037),
        Some(21),
        Some(0x32),
        Some(0x3233),
        Some(0x32333020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        9,
        8296,
        0x18,
        PacketDir::Egress,
        Some(53037),
        Some(21),
        Some(0x50),
        Some(0x5041),
        Some(0x50415356),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        10,
        8296,
        0x18,
        PacketDir::Ingress,
        Some(53037),
        Some(21),
        Some(0x32),
        Some(0x3232),
        Some(0x32323720),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        11,
        8296,
        0x18,
        PacketDir::Egress,
        Some(53037),
        Some(21),
        Some(0x52),
        Some(0x5245),
        Some(0x52455452),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        12,
        8296,
        0x18,
        PacketDir::Ingress,
        Some(53037),
        Some(21),
        Some(0x35),
        Some(0x3535),
        Some(0x35353020),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_transfer_open"))
    );
}

#[test]
fn ftp_stor_path_materializes_pasv_and_stor_transfer_phases() {
    let binding = compile_file(&dsl_fixture_path("ftp_stor_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8297, 53038, "ftp-client"));
    session.ingest(route_fact(2, 8297, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8297, 1, 2, 53038, 21));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        8297,
        0x18,
        PacketDir::Ingress,
        Some(53038),
        Some(21),
        Some(0x32),
        Some(0x3232),
        Some(0x32323020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        8297,
        0x18,
        PacketDir::Egress,
        Some(53038),
        Some(21),
        Some(0x55),
        Some(0x5553),
        Some(0x55534552),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        6,
        8297,
        0x18,
        PacketDir::Ingress,
        Some(53038),
        Some(21),
        Some(0x33),
        Some(0x3333),
        Some(0x33333120),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        7,
        8297,
        0x18,
        PacketDir::Egress,
        Some(53038),
        Some(21),
        Some(0x50),
        Some(0x5041),
        Some(0x50415353),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        8,
        8297,
        0x18,
        PacketDir::Ingress,
        Some(53038),
        Some(21),
        Some(0x32),
        Some(0x3233),
        Some(0x32333020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        9,
        8297,
        0x18,
        PacketDir::Egress,
        Some(53038),
        Some(21),
        Some(0x50),
        Some(0x5041),
        Some(0x50415356),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        10,
        8297,
        0x18,
        PacketDir::Ingress,
        Some(53038),
        Some(21),
        Some(0x32),
        Some(0x3232),
        Some(0x32323720),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        11,
        8297,
        0x18,
        PacketDir::Egress,
        Some(53038),
        Some(21),
        Some(0x53),
        Some(0x5354),
        Some(0x53544f52),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        12,
        8297,
        0x18,
        PacketDir::Ingress,
        Some(53038),
        Some(21),
        Some(0x31),
        Some(0x3135),
        Some(0x31353020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        13,
        8297,
        0x18,
        PacketDir::Ingress,
        Some(53038),
        Some(21),
        Some(0x32),
        Some(0x3232),
        Some(0x32323620),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("ftp_stor".into())
    );
    for phase in [
        "send_pasv",
        "receive_pasv_ready",
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
