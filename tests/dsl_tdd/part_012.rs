use super::*;

#[test]
fn smtp_auth_path_materializes_banner_ehlo_and_auth_phases() {
    let binding = compile_file(&dsl_fixture_path("smtp_auth_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8298, 53011, "postfix-client"));
    session.ingest(route_fact(2, 8298, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8298, 1, 2, 53011, 25));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        8298,
        0x18,
        PacketDir::Ingress,
        Some(53011),
        Some(25),
        Some(0x32),
        Some(0x3232),
        Some(0x32323020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        8298,
        0x18,
        PacketDir::Egress,
        Some(53011),
        Some(25),
        Some(0x45),
        Some(0x4548),
        Some(0x45484c4f),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        6,
        8298,
        0x18,
        PacketDir::Ingress,
        Some(53011),
        Some(25),
        Some(0x32),
        Some(0x3235),
        Some(0x32353020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        7,
        8298,
        0x18,
        PacketDir::Egress,
        Some(53011),
        Some(25),
        Some(0x41),
        Some(0x4155),
        Some(0x41555448),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        8,
        8298,
        0x18,
        PacketDir::Ingress,
        Some(53011),
        Some(25),
        Some(0x32),
        Some(0x3233),
        Some(0x32333520),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("smtp_auth".into())
    );
    for phase in [
        "receive_banner",
        "send_ehlo",
        "receive_ehlo_ok",
        "send_auth_request",
        "receive_auth_ok",
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
fn imap_auth_path_materializes_banner_and_login_phases() {
    let binding = compile_file(&dsl_fixture_path("imap_auth_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8401, 53031, "imap-client"));
    session.ingest(route_fact(2, 8401, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8401, 1, 2, 53031, 143));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        8401,
        0x18,
        PacketDir::Ingress,
        Some(53031),
        Some(143),
        Some(0x2a),
        Some(0x2a20),
        Some(0x2a204f4b),
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        5,
        8401,
        0x18,
        PacketDir::Egress,
        Some(53031),
        Some(143),
        &[
            (0, 0x41),
            (1, 0x30),
            (2, 0x30),
            (3, 0x31),
            (5, 0x4c),
            (6, 0x4f),
            (7, 0x47),
            (8, 0x49),
            (9, 0x4e),
        ],
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        6,
        8401,
        0x18,
        PacketDir::Ingress,
        Some(53031),
        Some(143),
        &[
            (0, 0x41),
            (1, 0x30),
            (2, 0x30),
            (3, 0x31),
            (5, 0x4f),
            (6, 0x4b),
        ],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(70));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("imap_auth".into())
    );
    for phase in ["receive_banner", "send_auth_request", "receive_auth_ok"] {
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
fn imap_select_path_materializes_login_and_select_phases() {
    let binding = compile_file(&dsl_fixture_path("imap_select_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8402, 53032, "imap-client"));
    session.ingest(route_fact(2, 8402, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8402, 1, 2, 53032, 143));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        8402,
        0x18,
        PacketDir::Ingress,
        Some(53032),
        Some(143),
        Some(0x2a),
        Some(0x2a20),
        Some(0x2a204f4b),
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        5,
        8402,
        0x18,
        PacketDir::Egress,
        Some(53032),
        Some(143),
        &[
            (0, 0x41),
            (1, 0x30),
            (2, 0x30),
            (3, 0x31),
            (5, 0x4c),
            (6, 0x4f),
            (7, 0x47),
            (8, 0x49),
            (9, 0x4e),
        ],
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        6,
        8402,
        0x18,
        PacketDir::Ingress,
        Some(53032),
        Some(143),
        &[
            (0, 0x41),
            (1, 0x30),
            (2, 0x30),
            (3, 0x31),
            (5, 0x4f),
            (6, 0x4b),
        ],
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        7,
        8402,
        0x18,
        PacketDir::Egress,
        Some(53032),
        Some(143),
        &[
            (0, 0x41),
            (1, 0x30),
            (2, 0x30),
            (3, 0x32),
            (5, 0x53),
            (6, 0x45),
            (7, 0x4c),
            (8, 0x45),
            (9, 0x43),
            (10, 0x54),
        ],
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        8,
        8402,
        0x18,
        PacketDir::Ingress,
        Some(53032),
        Some(143),
        &[
            (0, 0x41),
            (1, 0x30),
            (2, 0x30),
            (3, 0x32),
            (5, 0x4f),
            (6, 0x4b),
        ],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("imap_select".into())
    );
    for phase in [
        "receive_banner",
        "send_auth_request",
        "receive_auth_ok",
        "send_select",
        "receive_mailbox_selected",
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
fn pop3_auth_path_materializes_banner_and_auth_phases() {
    let binding = compile_file(&dsl_fixture_path("pop3_auth_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8405, 53035, "pop3-client"));
    session.ingest(route_fact(2, 8405, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8405, 1, 2, 53035, 110));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        4,
        8405,
        0x18,
        PacketDir::Ingress,
        Some(53035),
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
        8405,
        0x18,
        PacketDir::Egress,
        Some(53035),
        Some(110),
        &[(0, 0x55), (1, 0x53), (2, 0x45), (3, 0x52)],
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        6,
        8405,
        0x18,
        PacketDir::Ingress,
        Some(53035),
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
        8405,
        0x18,
        PacketDir::Egress,
        Some(53035),
        Some(110),
        &[(0, 0x50), (1, 0x41), (2, 0x53), (3, 0x53)],
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        8,
        8405,
        0x18,
        PacketDir::Ingress,
        Some(53035),
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
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("pop3_auth".into())
    );
    for phase in [
        "receive_banner",
        "send_user",
        "receive_user_ok",
        "send_auth_pass",
        "receive_auth_ok",
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
fn pop3_list_path_materializes_auth_and_list_phases() {
    let binding = compile_file(&dsl_fixture_path("pop3_list_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8406, 53036, "pop3-client"));
    session.ingest(route_fact(2, 8406, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8406, 1, 2, 53036, 110));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        4,
        8406,
        0x18,
        PacketDir::Ingress,
        Some(53036),
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
        8406,
        0x18,
        PacketDir::Egress,
        Some(53036),
        Some(110),
        &[(0, 0x55), (1, 0x53), (2, 0x45), (3, 0x52)],
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        6,
        8406,
        0x18,
        PacketDir::Ingress,
        Some(53036),
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
        8406,
        0x18,
        PacketDir::Egress,
        Some(53036),
        Some(110),
        &[(0, 0x50), (1, 0x41), (2, 0x53), (3, 0x53)],
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        8,
        8406,
        0x18,
        PacketDir::Ingress,
        Some(53036),
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
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        9,
        8406,
        0x18,
        PacketDir::Egress,
        Some(53036),
        Some(110),
        &[(0, 0x4c), (1, 0x49), (2, 0x53), (3, 0x54)],
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        10,
        8406,
        0x18,
        PacketDir::Ingress,
        Some(53036),
        Some(110),
        &[
            (0, 0x2b),
            (1, 0x4f),
            (2, 0x4b),
            (3, 0x20),
            (5, 0x6d),
            (6, 0x65),
            (7, 0x73),
            (8, 0x73),
        ],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(90));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("pop3_list".into())
    );
    for phase in [
        "receive_banner",
        "send_user",
        "receive_user_ok",
        "send_auth_pass",
        "receive_auth_ok",
        "send_list",
        "receive_list_ready",
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
