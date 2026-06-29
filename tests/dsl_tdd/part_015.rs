use super::*;

#[test]
fn smtp_data_denied_path_materializes_denied_queue_phase() {
    let binding = compile_file(&dsl_fixture_path("smtp_data_denied_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8306, 53020, "postfix-client"));
    session.ingest(route_fact(2, 8306, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8306, 1, 2, 53020, 25));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        8306,
        0x18,
        PacketDir::Ingress,
        Some(53020),
        Some(25),
        Some(0x32),
        Some(0x3232),
        Some(0x32323020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        8306,
        0x18,
        PacketDir::Egress,
        Some(53020),
        Some(25),
        Some(0x45),
        Some(0x4548),
        Some(0x45484c4f),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        6,
        8306,
        0x18,
        PacketDir::Ingress,
        Some(53020),
        Some(25),
        Some(0x32),
        Some(0x3235),
        Some(0x32353020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        7,
        8306,
        0x18,
        PacketDir::Egress,
        Some(53020),
        Some(25),
        Some(0x41),
        Some(0x4155),
        Some(0x41555448),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        8,
        8306,
        0x18,
        PacketDir::Ingress,
        Some(53020),
        Some(25),
        Some(0x32),
        Some(0x3233),
        Some(0x32333520),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        9,
        8306,
        0x18,
        PacketDir::Egress,
        Some(53020),
        Some(25),
        Some(0x4d),
        Some(0x4d41),
        Some(0x4d41494c),
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        10,
        8306,
        0x18,
        PacketDir::Ingress,
        Some(53020),
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
        8306,
        0x18,
        PacketDir::Egress,
        Some(53020),
        Some(25),
        Some(0x52),
        Some(0x5243),
        Some(0x52435054),
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        12,
        8306,
        0x18,
        PacketDir::Ingress,
        Some(53020),
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
        8306,
        0x18,
        PacketDir::Egress,
        Some(53020),
        Some(25),
        Some(0x44),
        Some(0x4441),
        Some(0x44415441),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        14,
        8306,
        0x18,
        PacketDir::Ingress,
        Some(53020),
        Some(25),
        Some(0x33),
        Some(0x3335),
        Some(0x33353420),
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        15,
        8306,
        0x18,
        PacketDir::Egress,
        Some(53020),
        Some(25),
        &[(0, 0x0d), (1, 0x0a), (2, 0x2e), (3, 0x0d), (4, 0x0a)],
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        16,
        8306,
        0x18,
        PacketDir::Ingress,
        Some(53020),
        Some(25),
        Some(0x35),
        Some(0x3535),
        Some(0x35353020),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(150));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("smtp_data_denied".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_message_denied"))
    );
}

#[test]
fn smtp_data_denied_path_does_not_match_success_queue_response() {
    let binding = compile_file(&dsl_fixture_path("smtp_data_denied_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8307, 53021, "postfix-client"));
    session.ingest(route_fact(2, 8307, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8307, 1, 2, 53021, 25));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        8307,
        0x18,
        PacketDir::Ingress,
        Some(53021),
        Some(25),
        Some(0x32),
        Some(0x3232),
        Some(0x32323020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        8307,
        0x18,
        PacketDir::Egress,
        Some(53021),
        Some(25),
        Some(0x45),
        Some(0x4548),
        Some(0x45484c4f),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        6,
        8307,
        0x18,
        PacketDir::Ingress,
        Some(53021),
        Some(25),
        Some(0x32),
        Some(0x3235),
        Some(0x32353020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        7,
        8307,
        0x18,
        PacketDir::Egress,
        Some(53021),
        Some(25),
        Some(0x41),
        Some(0x4155),
        Some(0x41555448),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        8,
        8307,
        0x18,
        PacketDir::Ingress,
        Some(53021),
        Some(25),
        Some(0x32),
        Some(0x3233),
        Some(0x32333520),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        9,
        8307,
        0x18,
        PacketDir::Egress,
        Some(53021),
        Some(25),
        Some(0x4d),
        Some(0x4d41),
        Some(0x4d41494c),
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        10,
        8307,
        0x18,
        PacketDir::Ingress,
        Some(53021),
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
        8307,
        0x18,
        PacketDir::Egress,
        Some(53021),
        Some(25),
        Some(0x52),
        Some(0x5243),
        Some(0x52435054),
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        12,
        8307,
        0x18,
        PacketDir::Ingress,
        Some(53021),
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
        8307,
        0x18,
        PacketDir::Egress,
        Some(53021),
        Some(25),
        Some(0x44),
        Some(0x4441),
        Some(0x44415441),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        14,
        8307,
        0x18,
        PacketDir::Ingress,
        Some(53021),
        Some(25),
        Some(0x33),
        Some(0x3335),
        Some(0x33353420),
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        15,
        8307,
        0x18,
        PacketDir::Egress,
        Some(53021),
        Some(25),
        &[(0, 0x0d), (1, 0x0a), (2, 0x2e), (3, 0x0d), (4, 0x0a)],
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        16,
        8307,
        0x18,
        PacketDir::Ingress,
        Some(53021),
        Some(25),
        Some(0x32),
        Some(0x3235),
        Some(0x32353020),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(150));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_message_denied"))
    );
}

#[test]
fn smtp_rcpt_denied_path_materializes_recipient_denied_phase() {
    let binding = compile_file(&dsl_fixture_path("smtp_rcpt_denied_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8308, 53022, "postfix-client"));
    session.ingest(route_fact(2, 8308, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8308, 1, 2, 53022, 25));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        8308,
        0x18,
        PacketDir::Ingress,
        Some(53022),
        Some(25),
        Some(0x32),
        Some(0x3232),
        Some(0x32323020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        8308,
        0x18,
        PacketDir::Egress,
        Some(53022),
        Some(25),
        Some(0x45),
        Some(0x4548),
        Some(0x45484c4f),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        6,
        8308,
        0x18,
        PacketDir::Ingress,
        Some(53022),
        Some(25),
        Some(0x32),
        Some(0x3235),
        Some(0x32353020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        7,
        8308,
        0x18,
        PacketDir::Egress,
        Some(53022),
        Some(25),
        Some(0x41),
        Some(0x4155),
        Some(0x41555448),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        8,
        8308,
        0x18,
        PacketDir::Ingress,
        Some(53022),
        Some(25),
        Some(0x32),
        Some(0x3233),
        Some(0x32333520),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        9,
        8308,
        0x18,
        PacketDir::Egress,
        Some(53022),
        Some(25),
        Some(0x4d),
        Some(0x4d41),
        Some(0x4d41494c),
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        10,
        8308,
        0x18,
        PacketDir::Ingress,
        Some(53022),
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
        8308,
        0x18,
        PacketDir::Egress,
        Some(53022),
        Some(25),
        Some(0x52),
        Some(0x5243),
        Some(0x52435054),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        12,
        8308,
        0x18,
        PacketDir::Ingress,
        Some(53022),
        Some(25),
        Some(0x35),
        Some(0x3535),
        Some(0x35353020),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(110));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("smtp_rcpt_denied".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_rcpt_denied"))
    );
}
