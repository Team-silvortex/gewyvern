use super::*;

#[test]
fn ldap_directory_session_can_span_bind_and_search_in_one_module() {
    let binding = compile_file(&dsl_fixture_path("ldap_directory_session.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 835, 54022, "ldap-directory-client"));
    session.ingest(route_fact(2, 835, 7));
    session.ingest(tcp_state_fact_with_ports(3, 835, 1, 2, 54022, 389));
    session.ingest(tcp_state_fact_with_ports(4, 835, 2, 3, 54022, 389));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        5,
        835,
        0x18,
        PacketDir::Egress,
        Some(54022),
        Some(389),
        Some(0x30),
        Some(0x300c),
        Some(0x300c0201),
        Some(0x01),
        Some(0x60),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        6,
        835,
        0x18,
        PacketDir::Ingress,
        Some(54022),
        Some(389),
        Some(0x30),
        Some(0x300c),
        Some(0x300c0201),
        Some(0x01),
        Some(0x61),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        7,
        835,
        0x18,
        PacketDir::Egress,
        Some(54022),
        Some(389),
        Some(0x30),
        Some(0x3010),
        Some(0x30100201),
        Some(0x01),
        Some(0x63),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        8,
        835,
        0x18,
        PacketDir::Ingress,
        Some(54022),
        Some(389),
        Some(0x30),
        Some(0x300c),
        Some(0x300c0201),
        Some(0x01),
        Some(0x65),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(100));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("ldap_directory_session".into())
    );
    let phases = export.program_flows[0]
        .stages
        .iter()
        .filter_map(|stage| stage.phase.clone())
        .collect::<Vec<_>>();
    assert!(phases.contains(&"connect".to_string()));
    assert!(phases.contains(&"establish".to_string()));
    assert!(phases.contains(&"send_bind".to_string()));
    assert!(phases.contains(&"receive_bind_response".to_string()));
    assert!(phases.contains(&"send_search".to_string()));
    assert!(phases.contains(&"receive_search_result".to_string()));
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
fn ldap_directory_write_session_can_span_bind_and_modify_in_one_module() {
    let binding = compile_file(&dsl_fixture_path("ldap_directory_write_session.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 838, 54024, "ldap-directory-writer"));
    session.ingest(route_fact(2, 838, 7));
    session.ingest(tcp_state_fact_with_ports(3, 838, 1, 2, 54024, 389));
    session.ingest(tcp_state_fact_with_ports(4, 838, 2, 3, 54024, 389));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        5,
        838,
        0x18,
        PacketDir::Egress,
        Some(54024),
        Some(389),
        Some(0x30),
        Some(0x300c),
        Some(0x300c0201),
        Some(0x01),
        Some(0x60),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        6,
        838,
        0x18,
        PacketDir::Ingress,
        Some(54024),
        Some(389),
        Some(0x30),
        Some(0x300c),
        Some(0x300c0201),
        Some(0x01),
        Some(0x61),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        7,
        838,
        0x18,
        PacketDir::Egress,
        Some(54024),
        Some(389),
        Some(0x30),
        Some(0x3012),
        Some(0x30120201),
        Some(0x01),
        Some(0x66),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_5_and9(
        8,
        838,
        0x18,
        PacketDir::Ingress,
        Some(54024),
        Some(389),
        Some(0x30),
        Some(0x300c),
        Some(0x300c0201),
        Some(0x01),
        Some(0x67),
        Some(0x00),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(100));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("ldap_directory_write_session".into())
    );
    let phases = export.program_flows[0]
        .stages
        .iter()
        .filter_map(|stage| stage.phase.clone())
        .collect::<Vec<_>>();
    assert!(phases.contains(&"connect".to_string()));
    assert!(phases.contains(&"establish".to_string()));
    assert!(phases.contains(&"send_bind".to_string()));
    assert!(phases.contains(&"receive_bind_response".to_string()));
    assert!(phases.contains(&"send_modify".to_string()));
    assert!(phases.contains(&"receive_modify_response".to_string()));
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
fn ldap_directory_sync_session_can_span_bind_search_and_modify_in_one_module() {
    let binding = compile_file(&dsl_fixture_path("ldap_directory_sync_session.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 839, 54025, "ldap-directory-sync"));
    session.ingest(route_fact(2, 839, 7));
    session.ingest(tcp_state_fact_with_ports(3, 839, 1, 2, 54025, 389));
    session.ingest(tcp_state_fact_with_ports(4, 839, 2, 3, 54025, 389));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        5,
        839,
        0x18,
        PacketDir::Egress,
        Some(54025),
        Some(389),
        Some(0x30),
        Some(0x300c),
        Some(0x300c0201),
        Some(0x01),
        Some(0x60),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        6,
        839,
        0x18,
        PacketDir::Ingress,
        Some(54025),
        Some(389),
        Some(0x30),
        Some(0x300c),
        Some(0x300c0201),
        Some(0x01),
        Some(0x61),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        7,
        839,
        0x18,
        PacketDir::Egress,
        Some(54025),
        Some(389),
        Some(0x30),
        Some(0x3010),
        Some(0x30100201),
        Some(0x01),
        Some(0x63),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        8,
        839,
        0x18,
        PacketDir::Ingress,
        Some(54025),
        Some(389),
        Some(0x30),
        Some(0x300c),
        Some(0x300c0201),
        Some(0x01),
        Some(0x65),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_5_and9(
        9,
        839,
        0x18,
        PacketDir::Egress,
        Some(54025),
        Some(389),
        Some(0x30),
        Some(0x3012),
        Some(0x30120201),
        Some(0x01),
        Some(0x66),
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_5_and9(
        10,
        839,
        0x18,
        PacketDir::Ingress,
        Some(54025),
        Some(389),
        Some(0x30),
        Some(0x300c),
        Some(0x300c0201),
        Some(0x01),
        Some(0x67),
        Some(0x00),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(120));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("ldap_directory_sync_session".into())
    );
    let phases = export.program_flows[0]
        .stages
        .iter()
        .filter_map(|stage| stage.phase.clone())
        .collect::<Vec<_>>();
    assert!(phases.contains(&"connect".to_string()));
    assert!(phases.contains(&"establish".to_string()));
    assert!(phases.contains(&"send_bind".to_string()));
    assert!(phases.contains(&"receive_bind_response".to_string()));
    assert!(phases.contains(&"send_search".to_string()));
    assert!(phases.contains(&"receive_search_result".to_string()));
    assert!(phases.contains(&"send_modify".to_string()));
    assert!(phases.contains(&"receive_modify_response".to_string()));
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
fn ldap_directory_sync_session_missing_modify_produces_search_to_modify_transition() {
    let binding = compile_file(&dsl_fixture_path("ldap_directory_sync_session.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 840, 54026, "ldap-directory-sync"));
    session.ingest(route_fact(2, 840, 7));
    session.ingest(tcp_state_fact_with_ports(3, 840, 1, 2, 54026, 389));
    session.ingest(tcp_state_fact_with_ports(4, 840, 2, 3, 54026, 389));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        5,
        840,
        0x18,
        PacketDir::Egress,
        Some(54026),
        Some(389),
        Some(0x30),
        Some(0x300c),
        Some(0x300c0201),
        Some(0x01),
        Some(0x60),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        6,
        840,
        0x18,
        PacketDir::Ingress,
        Some(54026),
        Some(389),
        Some(0x30),
        Some(0x300c),
        Some(0x300c0201),
        Some(0x01),
        Some(0x61),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        7,
        840,
        0x18,
        PacketDir::Egress,
        Some(54026),
        Some(389),
        Some(0x30),
        Some(0x3010),
        Some(0x30100201),
        Some(0x01),
        Some(0x63),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        8,
        840,
        0x18,
        PacketDir::Ingress,
        Some(54026),
        Some(389),
        Some(0x30),
        Some(0x300c),
        Some(0x300c0201),
        Some(0x01),
        Some(0x65),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(100));

    let export = session.export_bundle();
    assert!(export.program_findings.iter().any(|finding| {
        finding.module_label == "ldap_directory_sync_session"
            && finding.phase.as_deref() == Some("send_modify")
            && finding.phase_transition.as_deref() == Some("receive_search_result->send_modify")
    }));
    assert!(export.module_findings.iter().any(|finding| {
        finding
            .phase_transitions
            .contains(&"receive_search_result->send_modify".to_string())
    }));
}

#[test]
fn ldap_directory_sync_session_failed_modify_response_produces_modify_transition() {
    let binding = compile_file(&dsl_fixture_path("ldap_directory_sync_session.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 841, 54027, "ldap-directory-sync"));
    session.ingest(route_fact(2, 841, 7));
    session.ingest(tcp_state_fact_with_ports(3, 841, 1, 2, 54027, 389));
    session.ingest(tcp_state_fact_with_ports(4, 841, 2, 3, 54027, 389));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        5,
        841,
        0x18,
        PacketDir::Egress,
        Some(54027),
        Some(389),
        Some(0x30),
        Some(0x300c),
        Some(0x300c0201),
        Some(0x01),
        Some(0x60),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        6,
        841,
        0x18,
        PacketDir::Ingress,
        Some(54027),
        Some(389),
        Some(0x30),
        Some(0x300c),
        Some(0x300c0201),
        Some(0x01),
        Some(0x61),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        7,
        841,
        0x18,
        PacketDir::Egress,
        Some(54027),
        Some(389),
        Some(0x30),
        Some(0x3010),
        Some(0x30100201),
        Some(0x01),
        Some(0x63),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        8,
        841,
        0x18,
        PacketDir::Ingress,
        Some(54027),
        Some(389),
        Some(0x30),
        Some(0x300c),
        Some(0x300c0201),
        Some(0x01),
        Some(0x65),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_5_and9(
        9,
        841,
        0x18,
        PacketDir::Egress,
        Some(54027),
        Some(389),
        Some(0x30),
        Some(0x3012),
        Some(0x30120201),
        Some(0x01),
        Some(0x66),
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_5_and9(
        10,
        841,
        0x18,
        PacketDir::Ingress,
        Some(54027),
        Some(389),
        Some(0x30),
        Some(0x300c),
        Some(0x300c0201),
        Some(0x01),
        Some(0x67),
        Some(0x31),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(120));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_modify_response"))
    );
    assert!(export.program_findings.iter().any(|finding| {
        finding.module_label == "ldap_directory_sync_session"
            && finding.phase.as_deref() == Some("receive_modify_response")
            && finding.phase_transition.as_deref() == Some("send_modify->receive_modify_response")
    }));
    assert!(export.module_findings.iter().any(|finding| {
        finding
            .phase_transitions
            .contains(&"send_modify->receive_modify_response".to_string())
    }));
}
