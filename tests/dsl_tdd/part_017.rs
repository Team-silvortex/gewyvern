use super::*;

#[test]
fn smtp_auth_path_does_not_treat_failed_auth_response_as_auth_ok() {
    let binding = compile_file(&dsl_fixture_path("smtp_auth_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8299, 53012, "postfix-client"));
    session.ingest(route_fact(2, 8299, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8299, 1, 2, 53012, 25));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        8299,
        0x18,
        PacketDir::Ingress,
        Some(53012),
        Some(25),
        Some(0x32),
        Some(0x3232),
        Some(0x32323020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        8299,
        0x18,
        PacketDir::Egress,
        Some(53012),
        Some(25),
        Some(0x45),
        Some(0x4548),
        Some(0x45484c4f),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        6,
        8299,
        0x18,
        PacketDir::Ingress,
        Some(53012),
        Some(25),
        Some(0x32),
        Some(0x3235),
        Some(0x32353020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        7,
        8299,
        0x18,
        PacketDir::Egress,
        Some(53012),
        Some(25),
        Some(0x41),
        Some(0x4155),
        Some(0x41555448),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        8,
        8299,
        0x18,
        PacketDir::Ingress,
        Some(53012),
        Some(25),
        Some(0x35),
        Some(0x3533),
        Some(0x35333420),
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
fn imap_auth_path_does_not_treat_denied_response_as_auth_ok() {
    let binding = compile_file(&dsl_fixture_path("imap_auth_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8403, 53033, "imap-client"));
    session.ingest(route_fact(2, 8403, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8403, 1, 2, 53033, 143));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        8403,
        0x18,
        PacketDir::Ingress,
        Some(53033),
        Some(143),
        Some(0x2a),
        Some(0x2a20),
        Some(0x2a204f4b),
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        5,
        8403,
        0x18,
        PacketDir::Egress,
        Some(53033),
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
        8403,
        0x18,
        PacketDir::Ingress,
        Some(53033),
        Some(143),
        &[
            (0, 0x41),
            (1, 0x30),
            (2, 0x30),
            (3, 0x31),
            (5, 0x4e),
            (6, 0x4f),
        ],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(70));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_auth_ok"))
    );
}

#[test]
fn imap_select_path_does_not_treat_login_ok_as_mailbox_selected() {
    let binding = compile_file(&dsl_fixture_path("imap_select_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8404, 53034, "imap-client"));
    session.ingest(route_fact(2, 8404, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8404, 1, 2, 53034, 143));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        8404,
        0x18,
        PacketDir::Ingress,
        Some(53034),
        Some(143),
        Some(0x2a),
        Some(0x2a20),
        Some(0x2a204f4b),
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        5,
        8404,
        0x18,
        PacketDir::Egress,
        Some(53034),
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
        8404,
        0x18,
        PacketDir::Ingress,
        Some(53034),
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
        8404,
        0x18,
        PacketDir::Egress,
        Some(53034),
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
        8404,
        0x18,
        PacketDir::Ingress,
        Some(53034),
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
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_mailbox_selected"))
    );
}

#[test]
fn smtp_mail_path_does_not_treat_failed_mail_response_as_mail_ok() {
    let binding = compile_file(&dsl_fixture_path("smtp_mail_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8301, 53015, "postfix-client"));
    session.ingest(route_fact(2, 8301, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8301, 1, 2, 53015, 25));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        8301,
        0x18,
        PacketDir::Ingress,
        Some(53015),
        Some(25),
        Some(0x32),
        Some(0x3232),
        Some(0x32323020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        8301,
        0x18,
        PacketDir::Egress,
        Some(53015),
        Some(25),
        Some(0x45),
        Some(0x4548),
        Some(0x45484c4f),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        6,
        8301,
        0x18,
        PacketDir::Ingress,
        Some(53015),
        Some(25),
        Some(0x32),
        Some(0x3235),
        Some(0x32353020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        7,
        8301,
        0x18,
        PacketDir::Egress,
        Some(53015),
        Some(25),
        Some(0x41),
        Some(0x4155),
        Some(0x41555448),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        8,
        8301,
        0x18,
        PacketDir::Ingress,
        Some(53015),
        Some(25),
        Some(0x32),
        Some(0x3233),
        Some(0x32333520),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        9,
        8301,
        0x18,
        PacketDir::Egress,
        Some(53015),
        Some(25),
        Some(0x4d),
        Some(0x4d41),
        Some(0x4d41494c),
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        10,
        8301,
        0x18,
        PacketDir::Ingress,
        Some(53015),
        Some(25),
        &[
            (0, 0x35),
            (1, 0x35),
            (2, 0x30),
            (3, 0x20),
            (4, 0x35),
            (5, 0x2e),
            (6, 0x31),
            (7, 0x2e),
        ],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(90));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_mail_ok"))
    );
}

#[test]
fn smtp_rcpt_path_does_not_treat_failed_rcpt_response_as_rcpt_ok() {
    let binding = compile_file(&dsl_fixture_path("smtp_rcpt_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8303, 53017, "postfix-client"));
    session.ingest(route_fact(2, 8303, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8303, 1, 2, 53017, 25));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        8303,
        0x18,
        PacketDir::Ingress,
        Some(53017),
        Some(25),
        Some(0x32),
        Some(0x3232),
        Some(0x32323020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        8303,
        0x18,
        PacketDir::Egress,
        Some(53017),
        Some(25),
        Some(0x45),
        Some(0x4548),
        Some(0x45484c4f),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        6,
        8303,
        0x18,
        PacketDir::Ingress,
        Some(53017),
        Some(25),
        Some(0x32),
        Some(0x3235),
        Some(0x32353020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        7,
        8303,
        0x18,
        PacketDir::Egress,
        Some(53017),
        Some(25),
        Some(0x41),
        Some(0x4155),
        Some(0x41555448),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        8,
        8303,
        0x18,
        PacketDir::Ingress,
        Some(53017),
        Some(25),
        Some(0x32),
        Some(0x3233),
        Some(0x32333520),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        9,
        8303,
        0x18,
        PacketDir::Egress,
        Some(53017),
        Some(25),
        Some(0x4d),
        Some(0x4d41),
        Some(0x4d41494c),
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        10,
        8303,
        0x18,
        PacketDir::Ingress,
        Some(53017),
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
        8303,
        0x18,
        PacketDir::Egress,
        Some(53017),
        Some(25),
        Some(0x52),
        Some(0x5243),
        Some(0x52435054),
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        12,
        8303,
        0x18,
        PacketDir::Ingress,
        Some(53017),
        Some(25),
        &[
            (0, 0x35),
            (1, 0x35),
            (2, 0x30),
            (3, 0x20),
            (4, 0x35),
            (5, 0x2e),
            (6, 0x31),
            (7, 0x2e),
        ],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(110));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_rcpt_ok"))
    );
}
