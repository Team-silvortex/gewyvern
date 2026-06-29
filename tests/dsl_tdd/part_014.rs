use super::*;

#[test]
fn smtp_mail_path_materializes_auth_and_mail_from_phases() {
    let binding = compile_file(&dsl_fixture_path("smtp_mail_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8300, 53014, "postfix-client"));
    session.ingest(route_fact(2, 8300, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8300, 1, 2, 53014, 25));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        8300,
        0x18,
        PacketDir::Ingress,
        Some(53014),
        Some(25),
        Some(0x32),
        Some(0x3232),
        Some(0x32323020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        8300,
        0x18,
        PacketDir::Egress,
        Some(53014),
        Some(25),
        Some(0x45),
        Some(0x4548),
        Some(0x45484c4f),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        6,
        8300,
        0x18,
        PacketDir::Ingress,
        Some(53014),
        Some(25),
        Some(0x32),
        Some(0x3235),
        Some(0x32353020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        7,
        8300,
        0x18,
        PacketDir::Egress,
        Some(53014),
        Some(25),
        Some(0x41),
        Some(0x4155),
        Some(0x41555448),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        8,
        8300,
        0x18,
        PacketDir::Ingress,
        Some(53014),
        Some(25),
        Some(0x32),
        Some(0x3233),
        Some(0x32333520),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        9,
        8300,
        0x18,
        PacketDir::Egress,
        Some(53014),
        Some(25),
        Some(0x4d),
        Some(0x4d41),
        Some(0x4d41494c),
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        10,
        8300,
        0x18,
        PacketDir::Ingress,
        Some(53014),
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
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(90));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("smtp_mail".into())
    );
    for phase in ["receive_auth_ok", "send_mail_from", "receive_mail_ok"] {
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
fn smtp_rcpt_path_materializes_mail_and_rcpt_phases() {
    let binding = compile_file(&dsl_fixture_path("smtp_rcpt_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8302, 53016, "postfix-client"));
    session.ingest(route_fact(2, 8302, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8302, 1, 2, 53016, 25));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        8302,
        0x18,
        PacketDir::Ingress,
        Some(53016),
        Some(25),
        Some(0x32),
        Some(0x3232),
        Some(0x32323020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        8302,
        0x18,
        PacketDir::Egress,
        Some(53016),
        Some(25),
        Some(0x45),
        Some(0x4548),
        Some(0x45484c4f),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        6,
        8302,
        0x18,
        PacketDir::Ingress,
        Some(53016),
        Some(25),
        Some(0x32),
        Some(0x3235),
        Some(0x32353020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        7,
        8302,
        0x18,
        PacketDir::Egress,
        Some(53016),
        Some(25),
        Some(0x41),
        Some(0x4155),
        Some(0x41555448),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        8,
        8302,
        0x18,
        PacketDir::Ingress,
        Some(53016),
        Some(25),
        Some(0x32),
        Some(0x3233),
        Some(0x32333520),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        9,
        8302,
        0x18,
        PacketDir::Egress,
        Some(53016),
        Some(25),
        Some(0x4d),
        Some(0x4d41),
        Some(0x4d41494c),
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        10,
        8302,
        0x18,
        PacketDir::Ingress,
        Some(53016),
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
        8302,
        0x18,
        PacketDir::Egress,
        Some(53016),
        Some(25),
        Some(0x52),
        Some(0x5243),
        Some(0x52435054),
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        12,
        8302,
        0x18,
        PacketDir::Ingress,
        Some(53016),
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
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("smtp_rcpt".into())
    );
    for phase in ["receive_mail_ok", "send_rcpt_to", "receive_rcpt_ok"] {
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
fn smtp_data_path_materializes_data_and_queue_phases() {
    let binding = compile_file(&dsl_fixture_path("smtp_data_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8304, 53018, "postfix-client"));
    session.ingest(route_fact(2, 8304, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8304, 1, 2, 53018, 25));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        8304,
        0x18,
        PacketDir::Ingress,
        Some(53018),
        Some(25),
        Some(0x32),
        Some(0x3232),
        Some(0x32323020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        8304,
        0x18,
        PacketDir::Egress,
        Some(53018),
        Some(25),
        Some(0x45),
        Some(0x4548),
        Some(0x45484c4f),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        6,
        8304,
        0x18,
        PacketDir::Ingress,
        Some(53018),
        Some(25),
        Some(0x32),
        Some(0x3235),
        Some(0x32353020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        7,
        8304,
        0x18,
        PacketDir::Egress,
        Some(53018),
        Some(25),
        Some(0x41),
        Some(0x4155),
        Some(0x41555448),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        8,
        8304,
        0x18,
        PacketDir::Ingress,
        Some(53018),
        Some(25),
        Some(0x32),
        Some(0x3233),
        Some(0x32333520),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        9,
        8304,
        0x18,
        PacketDir::Egress,
        Some(53018),
        Some(25),
        Some(0x4d),
        Some(0x4d41),
        Some(0x4d41494c),
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        10,
        8304,
        0x18,
        PacketDir::Ingress,
        Some(53018),
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
        8304,
        0x18,
        PacketDir::Egress,
        Some(53018),
        Some(25),
        Some(0x52),
        Some(0x5243),
        Some(0x52435054),
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        12,
        8304,
        0x18,
        PacketDir::Ingress,
        Some(53018),
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
    session.ingest(packet_fact_with_dir_and_payload(
        13,
        8304,
        0x18,
        PacketDir::Egress,
        Some(53018),
        Some(25),
        Some(0x44),
        Some(0x4441),
        Some(0x44415441),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        14,
        8304,
        0x18,
        PacketDir::Ingress,
        Some(53018),
        Some(25),
        Some(0x33),
        Some(0x3335),
        Some(0x33353420),
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        15,
        8304,
        0x18,
        PacketDir::Egress,
        Some(53018),
        Some(25),
        &[(0, 0x0d), (1, 0x0a), (2, 0x2e), (3, 0x0d), (4, 0x0a)],
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        16,
        8304,
        0x18,
        PacketDir::Ingress,
        Some(53018),
        Some(25),
        &[
            (0, 0x32),
            (1, 0x35),
            (2, 0x30),
            (3, 0x20),
            (4, 0x32),
            (5, 0x2e),
            (6, 0x30),
            (7, 0x2e),
            (8, 0x30),
        ],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(150));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("smtp_data".into())
    );
    for phase in [
        "receive_rcpt_ok",
        "send_data",
        "receive_data_ready",
        "send_message_body",
        "receive_message_queued",
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
