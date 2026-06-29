use super::*;

#[test]
fn ftp_stor_path_does_not_match_wrong_transfer_open_code() {
    let binding = compile_file(&dsl_fixture_path("ftp_stor_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8298, 53039, "ftp-client"));
    session.ingest(route_fact(2, 8298, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8298, 1, 2, 53039, 21));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        8298,
        0x18,
        PacketDir::Ingress,
        Some(53039),
        Some(21),
        Some(0x32),
        Some(0x3232),
        Some(0x32323020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        8298,
        0x18,
        PacketDir::Egress,
        Some(53039),
        Some(21),
        Some(0x55),
        Some(0x5553),
        Some(0x55534552),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        6,
        8298,
        0x18,
        PacketDir::Ingress,
        Some(53039),
        Some(21),
        Some(0x33),
        Some(0x3333),
        Some(0x33333120),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        7,
        8298,
        0x18,
        PacketDir::Egress,
        Some(53039),
        Some(21),
        Some(0x50),
        Some(0x5041),
        Some(0x50415353),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        8,
        8298,
        0x18,
        PacketDir::Ingress,
        Some(53039),
        Some(21),
        Some(0x32),
        Some(0x3233),
        Some(0x32333020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        9,
        8298,
        0x18,
        PacketDir::Egress,
        Some(53039),
        Some(21),
        Some(0x50),
        Some(0x5041),
        Some(0x50415356),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        10,
        8298,
        0x18,
        PacketDir::Ingress,
        Some(53039),
        Some(21),
        Some(0x32),
        Some(0x3232),
        Some(0x32323720),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        11,
        8298,
        0x18,
        PacketDir::Egress,
        Some(53039),
        Some(21),
        Some(0x53),
        Some(0x5354),
        Some(0x53544f52),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        12,
        8298,
        0x18,
        PacketDir::Ingress,
        Some(53039),
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
fn ftp_active_list_path_materializes_port_and_list_transfer_phases() {
    let binding = compile_file(&dsl_fixture_path("ftp_active_list_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8299, 53040, "ftp-client"));
    session.ingest(route_fact(2, 8299, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8299, 1, 2, 53040, 21));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        8299,
        0x18,
        PacketDir::Ingress,
        Some(53040),
        Some(21),
        Some(0x32),
        Some(0x3232),
        Some(0x32323020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        8299,
        0x18,
        PacketDir::Egress,
        Some(53040),
        Some(21),
        Some(0x55),
        Some(0x5553),
        Some(0x55534552),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        6,
        8299,
        0x18,
        PacketDir::Ingress,
        Some(53040),
        Some(21),
        Some(0x33),
        Some(0x3333),
        Some(0x33333120),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        7,
        8299,
        0x18,
        PacketDir::Egress,
        Some(53040),
        Some(21),
        Some(0x50),
        Some(0x5041),
        Some(0x50415353),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        8,
        8299,
        0x18,
        PacketDir::Ingress,
        Some(53040),
        Some(21),
        Some(0x32),
        Some(0x3233),
        Some(0x32333020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        9,
        8299,
        0x18,
        PacketDir::Egress,
        Some(53040),
        Some(21),
        Some(0x50),
        Some(0x504f),
        Some(0x504f5254),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        10,
        8299,
        0x18,
        PacketDir::Ingress,
        Some(53040),
        Some(21),
        Some(0x32),
        Some(0x3230),
        Some(0x32303020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        11,
        8299,
        0x18,
        PacketDir::Egress,
        Some(53040),
        Some(21),
        Some(0x4c),
        Some(0x4c49),
        Some(0x4c495354),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        12,
        8299,
        0x18,
        PacketDir::Ingress,
        Some(53040),
        Some(21),
        Some(0x31),
        Some(0x3135),
        Some(0x31353020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        13,
        8299,
        0x18,
        PacketDir::Ingress,
        Some(53040),
        Some(21),
        Some(0x32),
        Some(0x3232),
        Some(0x32323620),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("ftp_active_list".into())
    );
    for phase in [
        "send_port",
        "receive_port_ready",
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
fn ftp_active_list_path_does_not_match_wrong_port_ready_code() {
    let binding = compile_file(&dsl_fixture_path("ftp_active_list_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8300, 53041, "ftp-client"));
    session.ingest(route_fact(2, 8300, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8300, 1, 2, 53041, 21));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        8300,
        0x18,
        PacketDir::Ingress,
        Some(53041),
        Some(21),
        Some(0x32),
        Some(0x3232),
        Some(0x32323020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        8300,
        0x18,
        PacketDir::Egress,
        Some(53041),
        Some(21),
        Some(0x55),
        Some(0x5553),
        Some(0x55534552),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        6,
        8300,
        0x18,
        PacketDir::Ingress,
        Some(53041),
        Some(21),
        Some(0x33),
        Some(0x3333),
        Some(0x33333120),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        7,
        8300,
        0x18,
        PacketDir::Egress,
        Some(53041),
        Some(21),
        Some(0x50),
        Some(0x5041),
        Some(0x50415353),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        8,
        8300,
        0x18,
        PacketDir::Ingress,
        Some(53041),
        Some(21),
        Some(0x32),
        Some(0x3233),
        Some(0x32333020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        9,
        8300,
        0x18,
        PacketDir::Egress,
        Some(53041),
        Some(21),
        Some(0x50),
        Some(0x504f),
        Some(0x504f5254),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        10,
        8300,
        0x18,
        PacketDir::Ingress,
        Some(53041),
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
fn ftp_active_retr_path_materializes_port_and_retr_transfer_phases() {
    let binding = compile_file(&dsl_fixture_path("ftp_active_retr_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8301, 53042, "ftp-client"));
    session.ingest(route_fact(2, 8301, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8301, 1, 2, 53042, 21));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        8301,
        0x18,
        PacketDir::Ingress,
        Some(53042),
        Some(21),
        Some(0x32),
        Some(0x3232),
        Some(0x32323020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        8301,
        0x18,
        PacketDir::Egress,
        Some(53042),
        Some(21),
        Some(0x55),
        Some(0x5553),
        Some(0x55534552),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        6,
        8301,
        0x18,
        PacketDir::Ingress,
        Some(53042),
        Some(21),
        Some(0x33),
        Some(0x3333),
        Some(0x33333120),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        7,
        8301,
        0x18,
        PacketDir::Egress,
        Some(53042),
        Some(21),
        Some(0x50),
        Some(0x5041),
        Some(0x50415353),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        8,
        8301,
        0x18,
        PacketDir::Ingress,
        Some(53042),
        Some(21),
        Some(0x32),
        Some(0x3233),
        Some(0x32333020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        9,
        8301,
        0x18,
        PacketDir::Egress,
        Some(53042),
        Some(21),
        Some(0x50),
        Some(0x504f),
        Some(0x504f5254),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        10,
        8301,
        0x18,
        PacketDir::Ingress,
        Some(53042),
        Some(21),
        Some(0x32),
        Some(0x3230),
        Some(0x32303020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        11,
        8301,
        0x18,
        PacketDir::Egress,
        Some(53042),
        Some(21),
        Some(0x52),
        Some(0x5245),
        Some(0x52455452),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        12,
        8301,
        0x18,
        PacketDir::Ingress,
        Some(53042),
        Some(21),
        Some(0x31),
        Some(0x3135),
        Some(0x31353020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        13,
        8301,
        0x18,
        PacketDir::Ingress,
        Some(53042),
        Some(21),
        Some(0x32),
        Some(0x3232),
        Some(0x32323620),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("ftp_active_retr".into())
    );
    for phase in [
        "send_port",
        "receive_port_ready",
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
