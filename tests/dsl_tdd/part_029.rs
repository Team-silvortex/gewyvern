use super::*;

#[test]
fn postgres_auth_path_materializes_auth_password_and_ready_phases() {
    let binding = compile_file(&dsl_fixture_path("postgres_auth_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 508, 7783, "psql"));
    session.ingest(tcp_state_fact_with_ports(2, 508, 1, 2, 43127, 5432));
    session.ingest(tcp_state_fact_with_ports(3, 508, 2, 3, 43127, 5432));
    session.ingest(route_fact(4, 508, 6));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        508,
        0,
        PacketDir::Ingress,
        Some(43127),
        Some(5432),
        Some(0x52),
        None,
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        6,
        508,
        0,
        PacketDir::Egress,
        Some(43127),
        Some(5432),
        Some(0x70),
        None,
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        7,
        508,
        0,
        PacketDir::Ingress,
        Some(43127),
        Some(5432),
        Some(0x5a),
        None,
        None,
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("postgres_auth".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_auth"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_password"))
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
    assert!(phase_kinds.contains(&"receive_payload".to_string()));
    assert!(phase_kinds.contains(&"emit_payload".to_string()));
    assert_eq!(export.module_findings.len(), 0);
}

#[test]
fn postgres_auth_path_does_not_match_wrong_auth_message_type() {
    let binding = compile_file(&dsl_fixture_path("postgres_auth_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 509, 7784, "psql"));
    session.ingest(tcp_state_fact_with_ports(2, 509, 1, 2, 43128, 5432));
    session.ingest(tcp_state_fact_with_ports(3, 509, 2, 3, 43128, 5432));
    session.ingest(route_fact(4, 509, 6));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        509,
        0,
        PacketDir::Ingress,
        Some(43128),
        Some(5432),
        Some(0x45),
        None,
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        6,
        509,
        0,
        PacketDir::Egress,
        Some(43128),
        Some(5432),
        Some(0x70),
        None,
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        7,
        509,
        0,
        PacketDir::Ingress,
        Some(43128),
        Some(5432),
        Some(0x5a),
        None,
        None,
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_auth"))
    );
}

#[test]
fn postgres_query_error_path_materializes_query_and_error_phases() {
    let binding = compile_file(&dsl_fixture_path("postgres_query_error_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 510, 7785, "psql"));
    session.ingest(tcp_state_fact_with_ports(2, 510, 1, 2, 43129, 5432));
    session.ingest(tcp_state_fact_with_ports(3, 510, 2, 3, 43129, 5432));
    session.ingest(route_fact(4, 510, 6));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        510,
        0,
        PacketDir::Egress,
        Some(43129),
        Some(5432),
        Some(0x51),
        None,
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        6,
        510,
        0,
        PacketDir::Ingress,
        Some(43129),
        Some(5432),
        Some(0x45),
        None,
        None,
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(70));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("postgres_query_error".into())
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
            .any(|stage| stage.phase.as_deref() == Some("receive_error"))
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
fn postgres_query_error_path_does_not_match_ready_message() {
    let binding = compile_file(&dsl_fixture_path("postgres_query_error_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 511, 7786, "psql"));
    session.ingest(tcp_state_fact_with_ports(2, 511, 1, 2, 43130, 5432));
    session.ingest(tcp_state_fact_with_ports(3, 511, 2, 3, 43130, 5432));
    session.ingest(route_fact(4, 511, 6));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        511,
        0,
        PacketDir::Egress,
        Some(43130),
        Some(5432),
        Some(0x51),
        None,
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        6,
        511,
        0,
        PacketDir::Ingress,
        Some(43130),
        Some(5432),
        Some(0x5a),
        None,
        None,
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
fn mysql_simple_query_path_materializes_connect_query_and_ok_phases() {
    let binding = compile_file(&dsl_fixture_path("mysql_simple_query_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 512, 7787, "mysql"));
    session.ingest(tcp_state_fact_with_ports(2, 512, 1, 2, 43131, 3306));
    session.ingest(tcp_state_fact_with_ports(3, 512, 2, 3, 43131, 3306));
    session.ingest(route_fact(4, 512, 6));
    session.ingest(packet_fact_with_dir_and_payload_and_byte4(
        5,
        512,
        0,
        PacketDir::Egress,
        Some(43131),
        Some(3306),
        None,
        None,
        None,
        Some(0x03),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_byte4(
        6,
        512,
        0,
        PacketDir::Ingress,
        Some(43131),
        Some(3306),
        None,
        None,
        None,
        Some(0x00),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(70));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("mysql_simple_query".into())
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
            .any(|stage| stage.phase.as_deref() == Some("receive_ok"))
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
fn mysql_simple_query_path_does_not_match_error_packet_as_ok() {
    let binding = compile_file(&dsl_fixture_path("mysql_simple_query_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 513, 7788, "mysql"));
    session.ingest(tcp_state_fact_with_ports(2, 513, 1, 2, 43132, 3306));
    session.ingest(tcp_state_fact_with_ports(3, 513, 2, 3, 43132, 3306));
    session.ingest(route_fact(4, 513, 6));
    session.ingest(packet_fact_with_dir_and_payload_and_byte4(
        5,
        513,
        0,
        PacketDir::Egress,
        Some(43132),
        Some(3306),
        None,
        None,
        None,
        Some(0x03),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_byte4(
        6,
        513,
        0,
        PacketDir::Ingress,
        Some(43132),
        Some(3306),
        None,
        None,
        None,
        Some(0xff),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(70));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_ok"))
    );
}

#[test]
fn mysql_query_session_can_span_connect_query_and_ok_in_one_module() {
    let binding = compile_file(&dsl_fixture_path("mysql_query_session.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 514, 7789, "mysql-session"));
    session.ingest(route_fact(2, 514, 6));
    session.ingest(tcp_state_fact_with_ports(3, 514, 1, 2, 43133, 3306));
    session.ingest(tcp_state_fact_with_ports(4, 514, 2, 3, 43133, 3306));
    session.ingest(packet_fact_with_dir_and_payload_and_byte4(
        5,
        514,
        0,
        PacketDir::Egress,
        Some(43133),
        Some(3306),
        None,
        None,
        None,
        Some(0x03),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_byte4(
        6,
        514,
        0,
        PacketDir::Ingress,
        Some(43133),
        Some(3306),
        None,
        None,
        None,
        Some(0x00),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("mysql_query_session".into())
    );
    let phases = export.program_flows[0]
        .stages
        .iter()
        .filter_map(|stage| stage.phase.clone())
        .collect::<Vec<_>>();
    assert!(phases.contains(&"connect".to_string()));
    assert!(phases.contains(&"establish".to_string()));
    assert!(phases.contains(&"send_query".to_string()));
    assert!(phases.contains(&"receive_ok".to_string()));
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
fn mysql_query_session_missing_response_produces_query_to_ok_transition() {
    let binding = compile_file(&dsl_fixture_path("mysql_query_session.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 515, 7790, "mysql-session"));
    session.ingest(route_fact(2, 515, 6));
    session.ingest(tcp_state_fact_with_ports(3, 515, 1, 2, 43134, 3306));
    session.ingest(tcp_state_fact_with_ports(4, 515, 2, 3, 43134, 3306));
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
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert!(export.program_findings.iter().any(|finding| {
        finding.module_label == "mysql_query_session"
            && finding.network_module_kind == "database_query"
            && finding.phase.as_deref() == Some("receive_ok")
            && finding.phase_transition.as_deref() == Some("send_query->receive_ok")
    }));
    assert!(export.module_findings.iter().any(|finding| {
        finding
            .phase_transitions
            .contains(&"send_query->receive_ok".to_string())
    }));
}

#[test]
fn mysql_query_error_path_materializes_query_and_error_phases() {
    let binding = compile_file(&dsl_fixture_path("mysql_query_error_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 514, 7789, "mysql"));
    session.ingest(tcp_state_fact_with_ports(2, 514, 1, 2, 43133, 3306));
    session.ingest(tcp_state_fact_with_ports(3, 514, 2, 3, 43133, 3306));
    session.ingest(route_fact(4, 514, 6));
    session.ingest(packet_fact_with_dir_and_payload_and_byte4(
        5,
        514,
        0,
        PacketDir::Egress,
        Some(43133),
        Some(3306),
        None,
        None,
        None,
        Some(0x03),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_byte4(
        6,
        514,
        0,
        PacketDir::Ingress,
        Some(43133),
        Some(3306),
        None,
        None,
        None,
        Some(0xff),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(70));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("mysql_query_error".into())
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
            .any(|stage| stage.phase.as_deref() == Some("receive_error"))
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
