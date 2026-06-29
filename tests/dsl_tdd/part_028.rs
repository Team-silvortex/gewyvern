use super::*;

#[test]
fn snmp_get_path_materializes_request_and_response_datagrams() {
    let binding = compile_file(&dsl_fixture_path("snmp_get_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 829, 54000, "snmpwalk"));
    session.ingest(route_fact(2, 829, 7));
    session.ingest(
        udp_packet_fact_with_dir_and_ports_and_payload_prefix4_and_byte13(
            3,
            829,
            96,
            PacketDir::Egress,
            Some(54000),
            Some(161),
            Some(0x30),
            Some(0x3026),
            Some(0x30260201),
            Some(0xa0),
        ),
    );
    session.ingest(
        udp_packet_fact_with_dir_and_ports_and_payload_prefix4_and_byte13(
            4,
            829,
            104,
            PacketDir::Ingress,
            Some(54000),
            Some(161),
            Some(0x30),
            Some(0x3028),
            Some(0x30280201),
            Some(0xa2),
        ),
    );
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("snmp_get".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_get_request"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_get_response"))
    );
    let phase_kinds = export.program_flows[0]
        .stages
        .iter()
        .filter_map(|stage| stage.phase_kind.clone())
        .collect::<Vec<_>>();
    assert!(phase_kinds.contains(&"emit_datagram".to_string()));
    assert!(phase_kinds.contains(&"receive_datagram".to_string()));
    assert_eq!(export.module_findings.len(), 0);
}

#[test]
fn snmp_get_path_does_not_match_wrong_response_pdu_type() {
    let binding = compile_file(&dsl_fixture_path("snmp_get_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 830, 54000, "snmpwalk"));
    session.ingest(route_fact(2, 830, 7));
    session.ingest(
        udp_packet_fact_with_dir_and_ports_and_payload_prefix4_and_byte13(
            3,
            830,
            96,
            PacketDir::Egress,
            Some(54000),
            Some(161),
            Some(0x30),
            Some(0x3026),
            Some(0x30260201),
            Some(0xa0),
        ),
    );
    session.ingest(
        udp_packet_fact_with_dir_and_ports_and_payload_prefix4_and_byte13(
            4,
            830,
            104,
            PacketDir::Ingress,
            Some(54000),
            Some(161),
            Some(0x30),
            Some(0x3028),
            Some(0x30280201),
            Some(0xa1),
        ),
    );
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_get_response"))
    );
}

#[test]
fn mqtt_connect_path_does_not_match_wrong_connack_prefix() {
    let binding = compile_file(&dsl_fixture_path("mqtt_connect_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 825, 53002, "mosquitto-pub"));
    session.ingest(route_fact(2, 825, 7));
    session.ingest(packet_fact_with_dir_and_payload(
        3,
        825,
        0x18,
        PacketDir::Egress,
        Some(53002),
        Some(1883),
        Some(0x10),
        Some(0x1016),
        Some(0x10160004),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        825,
        0x18,
        PacketDir::Ingress,
        Some(53002),
        Some(1883),
        Some(0x20),
        Some(0x2002),
        Some(0x20020001),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_connack"))
    );
}

#[test]
fn dns_tcp_query_path_materializes_request_and_response_payload_phases() {
    let binding = compile_file(&dsl_fixture_path("dns_tcp_query_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 825, 53053, "dig"));
    session.ingest(route_fact(2, 825, 7));
    session.ingest(packet_fact_with_dir_and_payload_and_byte4(
        3,
        825,
        0x18,
        PacketDir::Egress,
        Some(53053),
        Some(53),
        Some(0x00),
        Some(0x001c),
        Some(0x001c1234),
        Some(0x01),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_byte4(
        4,
        825,
        0x18,
        PacketDir::Ingress,
        Some(53053),
        Some(53),
        Some(0x00),
        Some(0x001c),
        Some(0x001c1234),
        Some(0x81),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("dns_tcp_query".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_query"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_response"))
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
fn dns_tcp_query_path_does_not_match_wrong_response_qr_bit() {
    let binding = compile_file(&dsl_fixture_path("dns_tcp_query_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 826, 53053, "dig"));
    session.ingest(route_fact(2, 826, 7));
    session.ingest(packet_fact_with_dir_and_payload_and_byte4(
        3,
        826,
        0x18,
        PacketDir::Egress,
        Some(53053),
        Some(53),
        Some(0x00),
        Some(0x001c),
        Some(0x001c1234),
        Some(0x01),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_byte4(
        4,
        826,
        0x18,
        PacketDir::Ingress,
        Some(53053),
        Some(53),
        Some(0x00),
        Some(0x001c),
        Some(0x001c1234),
        Some(0x01),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_response"))
    );
}

#[test]
fn https_connect_dsl_uses_destination_port_to_model_connect_path() {
    let binding = compile_file(&dsl_fixture_path("https_connect_process.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 401, 9001, "curl"));
    session.ingest(tcp_state_fact_with_ports(2, 401, 1, 2, 42310, 443));
    session.ingest(route_fact(3, 401, 5));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(40));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("https_connect".into())
    );
    assert!(
        export.program_flows[0]
            .narrative
            .iter()
            .any(|line| line.contains("HTTPS socket state transition"))
    );
    assert!(
        export.reasons[0]
            .l3
            .narrative
            .iter()
            .any(|line| line.text.contains("tcp state 1 -> 2"))
    );
}

#[test]
fn https_connect_dsl_does_not_treat_other_ports_as_https_connect() {
    let binding = compile_file(&dsl_fixture_path("https_connect_process.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 402, 9002, "curl"));
    session.ingest(tcp_state_fact_with_ports(2, 402, 1, 2, 42310, 80));
    session.ingest(route_fact(3, 402, 5));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(40));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .narrative
            .iter()
            .all(|line| !line.contains("HTTPS socket state transition"))
    );
    assert!(
        export.reasons[0]
            .l3
            .narrative
            .iter()
            .all(|line| !line.text.contains("tcp state 1 -> 2"))
    );
}

#[test]
fn postgres_connect_dsl_uses_named_port_alias() {
    let binding = compile_file(&dsl_fixture_path("postgres_connect_process.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 501, 7777, "psql"));
    session.ingest(tcp_state_fact_with_ports(2, 501, 1, 2, 43123, 5432));
    session.ingest(tcp_state_fact_with_ports(3, 501, 2, 3, 43123, 5432));
    session.ingest(route_fact(4, 501, 6));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(50));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("postgres_connect".into())
    );
    assert!(
        export.program_flows[0]
            .narrative
            .iter()
            .any(|line| line.contains("PostgreSQL socket state transition"))
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
            .any(|stage| stage.phase.as_deref() == Some("establish"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("resolve"))
    );
    assert_eq!(export.module_findings.len(), 0);
}

#[test]
fn postgres_simple_query_path_materializes_connect_query_and_ready_phases() {
    let binding = compile_file(&dsl_fixture_path("postgres_simple_query_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 506, 7781, "psql"));
    session.ingest(tcp_state_fact_with_ports(2, 506, 1, 2, 43125, 5432));
    session.ingest(tcp_state_fact_with_ports(3, 506, 2, 3, 43125, 5432));
    session.ingest(route_fact(4, 506, 6));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        506,
        0,
        PacketDir::Egress,
        Some(43125),
        Some(5432),
        Some(0x51),
        None,
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        6,
        506,
        0,
        PacketDir::Ingress,
        Some(43125),
        Some(5432),
        Some(0x5a),
        None,
        None,
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(70));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("postgres_simple_query".into())
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
            .any(|stage| stage.phase.as_deref() == Some("establish"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_query"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_ready"))
    );
    let phase_kinds = export.program_flows[0]
        .stages
        .iter()
        .filter_map(|stage| stage.phase_kind.clone())
        .collect::<Vec<_>>();
    assert!(phase_kinds.contains(&"initiate_connection".to_string()));
    assert!(phase_kinds.contains(&"establish_connection".to_string()));
    assert!(phase_kinds.contains(&"emit_payload".to_string()));
    assert!(phase_kinds.contains(&"receive_payload".to_string()));
    assert_eq!(export.module_findings.len(), 0);
}

#[test]
fn postgres_simple_query_path_does_not_match_wrong_server_message_type() {
    let binding = compile_file(&dsl_fixture_path("postgres_simple_query_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 507, 7782, "psql"));
    session.ingest(tcp_state_fact_with_ports(2, 507, 1, 2, 43126, 5432));
    session.ingest(tcp_state_fact_with_ports(3, 507, 2, 3, 43126, 5432));
    session.ingest(route_fact(4, 507, 6));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        507,
        0,
        PacketDir::Egress,
        Some(43126),
        Some(5432),
        Some(0x51),
        None,
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        6,
        507,
        0,
        PacketDir::Ingress,
        Some(43126),
        Some(5432),
        Some(0x45),
        None,
        None,
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(70));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_ready"))
    );
}
