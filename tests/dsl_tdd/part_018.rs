use super::*;

#[test]
fn smtp_data_path_does_not_treat_failed_queue_response_as_message_queued() {
    let binding = compile_file(&dsl_fixture_path("smtp_data_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8305, 53019, "postfix-client"));
    session.ingest(route_fact(2, 8305, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8305, 1, 2, 53019, 25));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        8305,
        0x18,
        PacketDir::Ingress,
        Some(53019),
        Some(25),
        Some(0x32),
        Some(0x3232),
        Some(0x32323020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        8305,
        0x18,
        PacketDir::Egress,
        Some(53019),
        Some(25),
        Some(0x45),
        Some(0x4548),
        Some(0x45484c4f),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        6,
        8305,
        0x18,
        PacketDir::Ingress,
        Some(53019),
        Some(25),
        Some(0x32),
        Some(0x3235),
        Some(0x32353020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        7,
        8305,
        0x18,
        PacketDir::Egress,
        Some(53019),
        Some(25),
        Some(0x41),
        Some(0x4155),
        Some(0x41555448),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        8,
        8305,
        0x18,
        PacketDir::Ingress,
        Some(53019),
        Some(25),
        Some(0x32),
        Some(0x3233),
        Some(0x32333520),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        9,
        8305,
        0x18,
        PacketDir::Egress,
        Some(53019),
        Some(25),
        Some(0x4d),
        Some(0x4d41),
        Some(0x4d41494c),
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        10,
        8305,
        0x18,
        PacketDir::Ingress,
        Some(53019),
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
        8305,
        0x18,
        PacketDir::Egress,
        Some(53019),
        Some(25),
        Some(0x52),
        Some(0x5243),
        Some(0x52435054),
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        12,
        8305,
        0x18,
        PacketDir::Ingress,
        Some(53019),
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
        8305,
        0x18,
        PacketDir::Egress,
        Some(53019),
        Some(25),
        Some(0x44),
        Some(0x4441),
        Some(0x44415441),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        14,
        8305,
        0x18,
        PacketDir::Ingress,
        Some(53019),
        Some(25),
        Some(0x33),
        Some(0x3335),
        Some(0x33353420),
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        15,
        8305,
        0x18,
        PacketDir::Egress,
        Some(53019),
        Some(25),
        &[(0, 0x0d), (1, 0x0a), (2, 0x2e), (3, 0x0d), (4, 0x0a)],
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        16,
        8305,
        0x18,
        PacketDir::Ingress,
        Some(53019),
        Some(25),
        &[
            (0, 0x34),
            (1, 0x35),
            (2, 0x31),
            (3, 0x20),
            (4, 0x34),
            (5, 0x2e),
            (6, 0x33),
            (7, 0x2e),
            (8, 0x30),
        ],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(150));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_message_queued"))
    );
}

#[test]
fn ftp_session_operation_maps_to_authentication_exchange_module_kind() {
    assert_eq!(
        gewyvern::flow::infer_network_module_kind(
            &ProgramOperation::Custom("ftp_session".into()),
            Some("receive_auth_ok"),
            Some("send_auth_user->receive_auth_ok"),
            "transport_io"
        ),
        "authentication_exchange"
    );
}

#[test]
fn ftp_passive_list_operation_maps_to_file_transfer_session_module_kind() {
    assert_eq!(
        gewyvern::flow::infer_network_module_kind(
            &ProgramOperation::Custom("ftp_passive_list".into()),
            Some("receive_transfer_complete"),
            Some("send_list->receive_transfer_complete"),
            "transport_io"
        ),
        "file_transfer_session"
    );
}

#[test]
fn ftp_retr_operation_maps_to_file_transfer_session_module_kind() {
    assert_eq!(
        gewyvern::flow::infer_network_module_kind(
            &ProgramOperation::Custom("ftp_retr".into()),
            Some("receive_transfer_complete"),
            Some("send_retr->receive_transfer_complete"),
            "transport_io"
        ),
        "file_transfer_session"
    );
}

#[test]
fn ftp_stor_operation_maps_to_file_transfer_session_module_kind() {
    assert_eq!(
        gewyvern::flow::infer_network_module_kind(
            &ProgramOperation::Custom("ftp_stor".into()),
            Some("receive_transfer_complete"),
            Some("send_stor->receive_transfer_complete"),
            "transport_io"
        ),
        "file_transfer_session"
    );
}

#[test]
fn ftp_active_list_operation_maps_to_file_transfer_session_module_kind() {
    assert_eq!(
        gewyvern::flow::infer_network_module_kind(
            &ProgramOperation::Custom("ftp_active_list".into()),
            Some("receive_transfer_complete"),
            Some("send_list->receive_transfer_complete"),
            "transport_io"
        ),
        "file_transfer_session"
    );
}

#[test]
fn ftp_active_retr_operation_maps_to_file_transfer_session_module_kind() {
    assert_eq!(
        gewyvern::flow::infer_network_module_kind(
            &ProgramOperation::Custom("ftp_active_retr".into()),
            Some("receive_transfer_complete"),
            Some("send_retr->receive_transfer_complete"),
            "transport_io"
        ),
        "file_transfer_session"
    );
}

#[test]
fn ftp_active_stor_operation_maps_to_file_transfer_session_module_kind() {
    assert_eq!(
        gewyvern::flow::infer_network_module_kind(
            &ProgramOperation::Custom("ftp_active_stor".into()),
            Some("receive_transfer_complete"),
            Some("send_stor->receive_transfer_complete"),
            "transport_io"
        ),
        "file_transfer_session"
    );
}

#[test]
fn ssh_session_path_materializes_banner_and_key_exchange_phases() {
    let binding = compile_file(&dsl_fixture_path("ssh_session_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8281, 53022, "ssh-client"));
    session.ingest(route_fact(2, 8281, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8281, 1, 2, 53022, 22));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        8281,
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
        8281,
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
        8281,
        0x18,
        PacketDir::Egress,
        Some(53022),
        Some(22),
        Some(0x00),
        None,
        None,
        Some(0x10),
        Some(0x14),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("ssh_session".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("connect"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_server_banner"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_client_banner"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_key_exchange_init"))
    );
    let phase_kinds = export.program_flows[0]
        .stages
        .iter()
        .filter_map(|stage| stage.phase_kind.clone())
        .collect::<Vec<_>>();
    assert!(phase_kinds.contains(&"initiate_connection".to_string()));
    assert!(phase_kinds.contains(&"receive_payload".to_string()));
    assert!(phase_kinds.contains(&"emit_payload".to_string()));
    assert_eq!(export.module_findings.len(), 0);
}

#[test]
fn ftp_session_path_materializes_banner_and_auth_phases() {
    let binding = compile_file(&dsl_fixture_path("ftp_session_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8289, 53030, "ftp-client"));
    session.ingest(route_fact(2, 8289, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8289, 1, 2, 53030, 21));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        8289,
        0x18,
        PacketDir::Ingress,
        Some(53030),
        Some(21),
        Some(0x32),
        Some(0x3232),
        Some(0x32323020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        8289,
        0x18,
        PacketDir::Egress,
        Some(53030),
        Some(21),
        Some(0x55),
        Some(0x5553),
        Some(0x55534552),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        6,
        8289,
        0x18,
        PacketDir::Ingress,
        Some(53030),
        Some(21),
        Some(0x33),
        Some(0x3333),
        Some(0x33333120),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        7,
        8289,
        0x18,
        PacketDir::Egress,
        Some(53030),
        Some(21),
        Some(0x50),
        Some(0x5041),
        Some(0x50415353),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        8,
        8289,
        0x18,
        PacketDir::Ingress,
        Some(53030),
        Some(21),
        Some(0x32),
        Some(0x3233),
        Some(0x32333020),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("ftp_session".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_banner"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_auth_user"))
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
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_auth_ok"))
    );
    assert_eq!(export.module_findings.len(), 0);
}
