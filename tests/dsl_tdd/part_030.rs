use super::*;

#[test]
fn mysql_query_error_path_does_not_match_ok_packet() {
    let binding = compile_file(&dsl_fixture_path("mysql_query_error_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 515, 7790, "mysql"));
    session.ingest(tcp_state_fact_with_ports(2, 515, 1, 2, 43134, 3306));
    session.ingest(tcp_state_fact_with_ports(3, 515, 2, 3, 43134, 3306));
    session.ingest(route_fact(4, 515, 6));
    session.ingest(packet_fact_with_dir_and_payload_and_byte4(
        5,
        515,
        0,
        PacketDir::Egress,
        Some(43134),
        Some(3306),
        None,
        None,
        None,
        Some(0x03),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_byte4(
        6,
        515,
        0,
        PacketDir::Ingress,
        Some(43134),
        Some(3306),
        None,
        None,
        None,
        Some(0x00),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(70));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_error"))
    );
}

#[test]
fn memcached_get_path_materializes_connect_get_and_value_phases() {
    let binding = compile_file(&dsl_fixture_path("memcached_get_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 516, 7791, "memcached-client"));
    session.ingest(tcp_state_fact_with_ports(2, 516, 1, 2, 43135, 11211));
    session.ingest(tcp_state_fact_with_ports(3, 516, 2, 3, 43135, 11211));
    session.ingest(route_fact(4, 516, 6));
    session.ingest(packet_fact_with_dir_and_payload_and_byte1(
        5,
        516,
        0,
        PacketDir::Egress,
        Some(43135),
        Some(11211),
        Some(0x80),
        Some(0x00),
        None,
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_byte1(
        6,
        516,
        0,
        PacketDir::Ingress,
        Some(43135),
        Some(11211),
        Some(0x81),
        Some(0x00),
        None,
        None,
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(70));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("memcached_get".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_get"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_value"))
    );
    let phase_kinds = export.program_flows[0]
        .stages
        .iter()
        .filter_map(|stage| stage.phase_kind.clone())
        .collect::<Vec<_>>();
    assert!(phase_kinds.contains(&"emit_payload".to_string()));
    assert!(phase_kinds.contains(&"receive_payload".to_string()));
    assert_eq!(export.module_findings.len(), 0);
}

#[test]
fn memcached_get_path_does_not_match_set_opcode() {
    let binding = compile_file(&dsl_fixture_path("memcached_get_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 517, 7792, "memcached-client"));
    session.ingest(tcp_state_fact_with_ports(2, 517, 1, 2, 43136, 11211));
    session.ingest(tcp_state_fact_with_ports(3, 517, 2, 3, 43136, 11211));
    session.ingest(route_fact(4, 517, 6));
    session.ingest(packet_fact_with_dir_and_payload_and_byte1(
        5,
        517,
        0,
        PacketDir::Egress,
        Some(43136),
        Some(11211),
        Some(0x80),
        Some(0x01),
        None,
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_byte1(
        6,
        517,
        0,
        PacketDir::Ingress,
        Some(43136),
        Some(11211),
        Some(0x81),
        Some(0x01),
        None,
        None,
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(70));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_value"))
    );
}

#[test]
fn memcached_set_path_materializes_connect_set_and_stored_phases() {
    let binding = compile_file(&dsl_fixture_path("memcached_set_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 518, 7793, "memcached-client"));
    session.ingest(tcp_state_fact_with_ports(2, 518, 1, 2, 43137, 11211));
    session.ingest(tcp_state_fact_with_ports(3, 518, 2, 3, 43137, 11211));
    session.ingest(route_fact(4, 518, 6));
    session.ingest(packet_fact_with_dir_and_payload_and_byte1(
        5,
        518,
        0,
        PacketDir::Egress,
        Some(43137),
        Some(11211),
        Some(0x80),
        Some(0x01),
        None,
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_byte1(
        6,
        518,
        0,
        PacketDir::Ingress,
        Some(43137),
        Some(11211),
        Some(0x81),
        Some(0x01),
        None,
        None,
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(70));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("memcached_set".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_set"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_stored"))
    );
    let phase_kinds = export.program_flows[0]
        .stages
        .iter()
        .filter_map(|stage| stage.phase_kind.clone())
        .collect::<Vec<_>>();
    assert!(phase_kinds.contains(&"emit_payload".to_string()));
    assert!(phase_kinds.contains(&"receive_payload".to_string()));
    assert_eq!(export.module_findings.len(), 0);
}

#[test]
fn memcached_set_path_does_not_match_get_opcode() {
    let binding = compile_file(&dsl_fixture_path("memcached_set_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 519, 7794, "memcached-client"));
    session.ingest(tcp_state_fact_with_ports(2, 519, 1, 2, 43138, 11211));
    session.ingest(tcp_state_fact_with_ports(3, 519, 2, 3, 43138, 11211));
    session.ingest(route_fact(4, 519, 6));
    session.ingest(packet_fact_with_dir_and_payload_and_byte1(
        5,
        519,
        0,
        PacketDir::Egress,
        Some(43138),
        Some(11211),
        Some(0x80),
        Some(0x00),
        None,
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_byte1(
        6,
        519,
        0,
        PacketDir::Ingress,
        Some(43138),
        Some(11211),
        Some(0x81),
        Some(0x00),
        None,
        None,
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(70));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_stored"))
    );
}

#[test]
fn amqp_connection_start_path_materializes_header_start_and_start_ok_phases() {
    let binding = compile_file(&dsl_fixture_path("amqp_connection_start_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 520, 7795, "amqp-client"));
    session.ingest(tcp_state_fact_with_ports(2, 520, 1, 2, 43139, 5672));
    session.ingest(tcp_state_fact_with_ports(3, 520, 2, 3, 43139, 5672));
    session.ingest(route_fact(4, 520, 6));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        520,
        0,
        PacketDir::Egress,
        Some(43139),
        Some(5672),
        None,
        None,
        Some(0x414d5150),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_byte10(
        6,
        520,
        0,
        PacketDir::Ingress,
        Some(43139),
        Some(5672),
        Some(0x01),
        Some(0x0a),
        None,
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_byte10(
        7,
        520,
        0,
        PacketDir::Egress,
        Some(43139),
        Some(5672),
        Some(0x01),
        Some(0x0b),
        None,
        None,
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("amqp_connection_start".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_protocol_header"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_start"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_start_ok"))
    );
    let phase_kinds = export.program_flows[0]
        .stages
        .iter()
        .filter_map(|stage| stage.phase_kind.clone())
        .collect::<Vec<_>>();
    assert!(phase_kinds.contains(&"emit_payload".to_string()));
    assert!(phase_kinds.contains(&"receive_payload".to_string()));
    assert_eq!(export.module_findings.len(), 0);
}

#[test]
fn amqp_connection_start_path_does_not_match_wrong_server_method_id() {
    let binding = compile_file(&dsl_fixture_path("amqp_connection_start_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 521, 7796, "amqp-client"));
    session.ingest(tcp_state_fact_with_ports(2, 521, 1, 2, 43140, 5672));
    session.ingest(tcp_state_fact_with_ports(3, 521, 2, 3, 43140, 5672));
    session.ingest(route_fact(4, 521, 6));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        521,
        0,
        PacketDir::Egress,
        Some(43140),
        Some(5672),
        None,
        None,
        Some(0x414d5150),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_byte10(
        6,
        521,
        0,
        PacketDir::Ingress,
        Some(43140),
        Some(5672),
        Some(0x01),
        Some(0x14),
        None,
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_byte10(
        7,
        521,
        0,
        PacketDir::Egress,
        Some(43140),
        Some(5672),
        Some(0x01),
        Some(0x0b),
        None,
        None,
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_start"))
    );
}

#[test]
fn amqp_basic_publish_path_materializes_publish_and_ack_phases() {
    let binding = compile_file(&dsl_fixture_path("amqp_basic_publish_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 522, 7797, "amqp-publisher"));
    session.ingest(tcp_state_fact_with_ports(2, 522, 1, 2, 43141, 5672));
    session.ingest(tcp_state_fact_with_ports(3, 522, 2, 3, 43141, 5672));
    session.ingest(route_fact(4, 522, 6));
    session.ingest(packet_fact_with_dir_and_payload_and_byte10(
        5,
        522,
        0,
        PacketDir::Egress,
        Some(43141),
        Some(5672),
        Some(0x01),
        Some(0x28),
        None,
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_byte10(
        6,
        522,
        0,
        PacketDir::Ingress,
        Some(43141),
        Some(5672),
        Some(0x01),
        Some(0x50),
        None,
        None,
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(70));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("amqp_basic_publish".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_publish"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_ack"))
    );
    let phase_kinds = export.program_flows[0]
        .stages
        .iter()
        .filter_map(|stage| stage.phase_kind.clone())
        .collect::<Vec<_>>();
    assert!(phase_kinds.contains(&"emit_payload".to_string()));
    assert!(phase_kinds.contains(&"receive_payload".to_string()));
    assert_eq!(export.module_findings.len(), 0);
}

#[test]
fn amqp_basic_publish_path_does_not_match_wrong_ack_method_id() {
    let binding = compile_file(&dsl_fixture_path("amqp_basic_publish_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 523, 7798, "amqp-publisher"));
    session.ingest(tcp_state_fact_with_ports(2, 523, 1, 2, 43142, 5672));
    session.ingest(tcp_state_fact_with_ports(3, 523, 2, 3, 43142, 5672));
    session.ingest(route_fact(4, 523, 6));
    session.ingest(packet_fact_with_dir_and_payload_and_byte10(
        5,
        523,
        0,
        PacketDir::Egress,
        Some(43142),
        Some(5672),
        Some(0x01),
        Some(0x28),
        None,
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_byte10(
        6,
        523,
        0,
        PacketDir::Ingress,
        Some(43142),
        Some(5672),
        Some(0x01),
        Some(0x0a),
        None,
        None,
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(70));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_ack"))
    );
}
