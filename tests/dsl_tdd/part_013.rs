use super::*;

#[test]
fn pop3_auth_path_does_not_treat_denied_response_as_auth_ok() {
    let binding = compile_file(&dsl_fixture_path("pop3_auth_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8407, 53037, "pop3-client"));
    session.ingest(route_fact(2, 8407, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8407, 1, 2, 53037, 110));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        4,
        8407,
        0x18,
        PacketDir::Ingress,
        Some(53037),
        Some(110),
        &[
            (0, 0x2b),
            (1, 0x4f),
            (2, 0x4b),
            (3, 0x20),
            (5, 0x50),
            (6, 0x4f),
            (7, 0x50),
            (8, 0x33),
        ],
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        5,
        8407,
        0x18,
        PacketDir::Egress,
        Some(53037),
        Some(110),
        &[(0, 0x55), (1, 0x53), (2, 0x45), (3, 0x52)],
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        6,
        8407,
        0x18,
        PacketDir::Ingress,
        Some(53037),
        Some(110),
        &[
            (0, 0x2b),
            (1, 0x4f),
            (2, 0x4b),
            (3, 0x20),
            (5, 0x55),
            (6, 0x73),
            (7, 0x65),
            (8, 0x72),
        ],
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        7,
        8407,
        0x18,
        PacketDir::Egress,
        Some(53037),
        Some(110),
        &[(0, 0x50), (1, 0x41), (2, 0x53), (3, 0x53)],
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        8,
        8407,
        0x18,
        PacketDir::Ingress,
        Some(53037),
        Some(110),
        &[
            (0, 0x2d),
            (1, 0x45),
            (2, 0x52),
            (3, 0x52),
            (5, 0x61),
            (6, 0x75),
            (7, 0x74),
            (8, 0x68),
        ],
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
fn pop3_list_path_does_not_treat_auth_ok_as_list_ready() {
    let binding = compile_file(&dsl_fixture_path("pop3_list_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8408, 53038, "pop3-client"));
    session.ingest(route_fact(2, 8408, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8408, 1, 2, 53038, 110));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        4,
        8408,
        0x18,
        PacketDir::Ingress,
        Some(53038),
        Some(110),
        &[
            (0, 0x2b),
            (1, 0x4f),
            (2, 0x4b),
            (3, 0x20),
            (5, 0x50),
            (6, 0x4f),
            (7, 0x50),
            (8, 0x33),
        ],
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        5,
        8408,
        0x18,
        PacketDir::Egress,
        Some(53038),
        Some(110),
        &[(0, 0x55), (1, 0x53), (2, 0x45), (3, 0x52)],
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        6,
        8408,
        0x18,
        PacketDir::Ingress,
        Some(53038),
        Some(110),
        &[
            (0, 0x2b),
            (1, 0x4f),
            (2, 0x4b),
            (3, 0x20),
            (5, 0x55),
            (6, 0x73),
            (7, 0x65),
            (8, 0x72),
        ],
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        7,
        8408,
        0x18,
        PacketDir::Egress,
        Some(53038),
        Some(110),
        &[(0, 0x50), (1, 0x41), (2, 0x53), (3, 0x53)],
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        8,
        8408,
        0x18,
        PacketDir::Ingress,
        Some(53038),
        Some(110),
        &[
            (0, 0x2b),
            (1, 0x4f),
            (2, 0x4b),
            (3, 0x20),
            (5, 0x4d),
            (6, 0x61),
            (7, 0x69),
            (8, 0x6c),
        ],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_list_ready"))
    );
}

#[test]
fn kerberos_as_path_materializes_request_and_reply_datagrams() {
    let binding = compile_file(&dsl_fixture_path("kerberos_as_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8409, 53039, "kinit"));
    session.ingest(route_fact(2, 8409, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        8409,
        120,
        PacketDir::Egress,
        Some(53039),
        Some(88),
        Some(0x6a),
        None,
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        4,
        8409,
        140,
        PacketDir::Ingress,
        Some(53039),
        Some(88),
        Some(0x6b),
        None,
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("kerberos_as".into())
    );
    for phase in ["send_as_request", "receive_as_reply"] {
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
fn kerberos_tgs_path_materializes_request_and_reply_datagrams() {
    let binding = compile_file(&dsl_fixture_path("kerberos_tgs_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8410, 53040, "kvno"));
    session.ingest(route_fact(2, 8410, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        8410,
        120,
        PacketDir::Egress,
        Some(53040),
        Some(88),
        Some(0x6c),
        None,
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        4,
        8410,
        140,
        PacketDir::Ingress,
        Some(53040),
        Some(88),
        Some(0x6d),
        None,
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("kerberos_tgs".into())
    );
    for phase in ["send_tgs_request", "receive_tgs_reply"] {
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
fn kerberos_as_path_does_not_treat_error_as_as_reply() {
    let binding = compile_file(&dsl_fixture_path("kerberos_as_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8411, 53041, "kinit"));
    session.ingest(route_fact(2, 8411, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        8411,
        120,
        PacketDir::Egress,
        Some(53041),
        Some(88),
        Some(0x6a),
        None,
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        4,
        8411,
        100,
        PacketDir::Ingress,
        Some(53041),
        Some(88),
        Some(0x7e),
        None,
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_as_reply"))
    );
}

#[test]
fn rtsp_setup_path_materializes_options_describe_and_setup_phases() {
    let binding = compile_file(&dsl_fixture_path("rtsp_setup_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8412, 53047, "vlc"));
    session.ingest(route_fact(2, 8412, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8412, 1, 2, 53047, 554));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        4,
        8412,
        0x18,
        PacketDir::Egress,
        Some(53047),
        Some(554),
        &[(0, 0x4f), (1, 0x50), (2, 0x54), (3, 0x49)],
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        5,
        8412,
        0x18,
        PacketDir::Ingress,
        Some(53047),
        Some(554),
        &[
            (0, 0x52),
            (1, 0x54),
            (2, 0x53),
            (3, 0x50),
            (9, 0x32),
            (10, 0x30),
            (11, 0x30),
            (17, 0x50),
            (18, 0x75),
            (19, 0x62),
            (20, 0x6c),
        ],
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        6,
        8412,
        0x18,
        PacketDir::Egress,
        Some(53047),
        Some(554),
        &[(0, 0x44), (1, 0x45), (2, 0x53), (3, 0x43)],
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        7,
        8412,
        0x18,
        PacketDir::Ingress,
        Some(53047),
        Some(554),
        &[
            (0, 0x52),
            (1, 0x54),
            (2, 0x53),
            (3, 0x50),
            (9, 0x32),
            (10, 0x30),
            (11, 0x30),
            (17, 0x43),
            (18, 0x6f),
            (19, 0x6e),
            (20, 0x74),
        ],
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        8,
        8412,
        0x18,
        PacketDir::Egress,
        Some(53047),
        Some(554),
        &[(0, 0x53), (1, 0x45), (2, 0x54), (3, 0x55)],
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        9,
        8412,
        0x18,
        PacketDir::Ingress,
        Some(53047),
        Some(554),
        &[
            (0, 0x52),
            (1, 0x54),
            (2, 0x53),
            (3, 0x50),
            (9, 0x32),
            (10, 0x30),
            (11, 0x30),
            (17, 0x53),
            (18, 0x65),
            (19, 0x73),
            (20, 0x73),
        ],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(90));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("rtsp_setup".into())
    );
    for phase in [
        "send_options",
        "receive_options_ok",
        "send_describe",
        "receive_describe_ok",
        "send_setup",
        "receive_setup_ok",
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
fn rtsp_setup_path_does_not_treat_describe_ok_as_setup_ok() {
    let binding = compile_file(&dsl_fixture_path("rtsp_setup_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8413, 53048, "vlc"));
    session.ingest(route_fact(2, 8413, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8413, 1, 2, 53048, 554));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        4,
        8413,
        0x18,
        PacketDir::Egress,
        Some(53048),
        Some(554),
        &[(0, 0x4f), (1, 0x50), (2, 0x54), (3, 0x49)],
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        5,
        8413,
        0x18,
        PacketDir::Ingress,
        Some(53048),
        Some(554),
        &[
            (0, 0x52),
            (1, 0x54),
            (2, 0x53),
            (3, 0x50),
            (9, 0x32),
            (10, 0x30),
            (11, 0x30),
            (17, 0x50),
            (18, 0x75),
            (19, 0x62),
            (20, 0x6c),
        ],
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        6,
        8413,
        0x18,
        PacketDir::Egress,
        Some(53048),
        Some(554),
        &[(0, 0x44), (1, 0x45), (2, 0x53), (3, 0x43)],
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        7,
        8413,
        0x18,
        PacketDir::Ingress,
        Some(53048),
        Some(554),
        &[
            (0, 0x52),
            (1, 0x54),
            (2, 0x53),
            (3, 0x50),
            (9, 0x32),
            (10, 0x30),
            (11, 0x30),
            (17, 0x43),
            (18, 0x6f),
            (19, 0x6e),
            (20, 0x74),
        ],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(70));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_setup_ok"))
    );
}
