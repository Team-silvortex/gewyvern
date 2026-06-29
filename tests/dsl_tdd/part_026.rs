use super::*;

#[test]
fn ldap_modify_path_materializes_connect_modify_and_response_phases() {
    let binding = compile_file(&dsl_fixture_path("ldap_modify_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 836, 54023, "ldapmodify"));
    session.ingest(route_fact(2, 836, 7));
    session.ingest(tcp_state_fact_with_ports(3, 836, 1, 2, 54023, 389));
    session.ingest(tcp_state_fact_with_ports(4, 836, 2, 3, 54023, 389));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_5_and9(
        5,
        836,
        0x18,
        PacketDir::Egress,
        Some(54023),
        Some(389),
        Some(0x30),
        Some(0x3012),
        Some(0x30120201),
        Some(0x01),
        Some(0x66),
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_5_and9(
        6,
        836,
        0x18,
        PacketDir::Ingress,
        Some(54023),
        Some(389),
        Some(0x30),
        Some(0x300c),
        Some(0x300c0201),
        Some(0x01),
        Some(0x67),
        Some(0x00),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("ldap_modify".into())
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
            .any(|stage| stage.phase.as_deref() == Some("send_modify"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_modify_response"))
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
fn ldap_modify_path_does_not_match_wrong_response_op_tag() {
    let binding = compile_file(&dsl_fixture_path("ldap_modify_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 837, 54023, "ldapmodify"));
    session.ingest(route_fact(2, 837, 7));
    session.ingest(tcp_state_fact_with_ports(3, 837, 1, 2, 54023, 389));
    session.ingest(tcp_state_fact_with_ports(4, 837, 2, 3, 54023, 389));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_5_and9(
        5,
        837,
        0x18,
        PacketDir::Egress,
        Some(54023),
        Some(389),
        Some(0x30),
        Some(0x3012),
        Some(0x30120201),
        Some(0x01),
        Some(0x66),
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_5_and9(
        6,
        837,
        0x18,
        PacketDir::Ingress,
        Some(54023),
        Some(389),
        Some(0x30),
        Some(0x300c),
        Some(0x300c0201),
        Some(0x01),
        Some(0x67),
        Some(0x31),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_modify_response"))
    );
}

#[test]
fn ldap_bind_denied_path_materializes_denied_bind_phase() {
    let binding = compile_file(&dsl_fixture_path("ldap_bind_denied_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 841, 54027, "ldapbind"));
    session.ingest(route_fact(2, 841, 7));
    session.ingest(tcp_state_fact_with_ports(3, 841, 1, 2, 54027, 389));
    session.ingest(tcp_state_fact_with_ports(4, 841, 2, 3, 54027, 389));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_5_and9(
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
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_5_and9(
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
        Some(0x31),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("ldap_bind_denied".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_bind"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_bind_denied"))
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
fn ldap_bind_denied_path_does_not_match_success_result_code() {
    let binding = compile_file(&dsl_fixture_path("ldap_bind_denied_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8411, 54027, "ldapbind"));
    session.ingest(route_fact(2, 8411, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8411, 1, 2, 54027, 389));
    session.ingest(tcp_state_fact_with_ports(4, 8411, 2, 3, 54027, 389));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_5_and9(
        5,
        8411,
        0x18,
        PacketDir::Egress,
        Some(54027),
        Some(389),
        Some(0x30),
        Some(0x300c),
        Some(0x300c0201),
        Some(0x01),
        Some(0x60),
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_5_and9(
        6,
        8411,
        0x18,
        PacketDir::Ingress,
        Some(54027),
        Some(389),
        Some(0x30),
        Some(0x300c),
        Some(0x300c0201),
        Some(0x01),
        Some(0x61),
        Some(0x00),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_bind_denied"))
    );
}

#[test]
fn ldap_modify_denied_path_materializes_denied_modify_phase() {
    let binding = compile_file(&dsl_fixture_path("ldap_modify_denied_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 842, 54028, "ldapmodify"));
    session.ingest(route_fact(2, 842, 7));
    session.ingest(tcp_state_fact_with_ports(3, 842, 1, 2, 54028, 389));
    session.ingest(tcp_state_fact_with_ports(4, 842, 2, 3, 54028, 389));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_5_and9(
        5,
        842,
        0x18,
        PacketDir::Egress,
        Some(54028),
        Some(389),
        Some(0x30),
        Some(0x3012),
        Some(0x30120201),
        Some(0x01),
        Some(0x66),
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_5_and9(
        6,
        842,
        0x18,
        PacketDir::Ingress,
        Some(54028),
        Some(389),
        Some(0x30),
        Some(0x300c),
        Some(0x300c0201),
        Some(0x01),
        Some(0x67),
        Some(0x32),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("ldap_modify_denied".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_modify"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_modify_denied"))
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
fn ldap_modify_denied_path_does_not_match_success_result_code() {
    let binding = compile_file(&dsl_fixture_path("ldap_modify_denied_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 843, 54028, "ldapmodify"));
    session.ingest(route_fact(2, 843, 7));
    session.ingest(tcp_state_fact_with_ports(3, 843, 1, 2, 54028, 389));
    session.ingest(tcp_state_fact_with_ports(4, 843, 2, 3, 54028, 389));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_5_and9(
        5,
        843,
        0x18,
        PacketDir::Egress,
        Some(54028),
        Some(389),
        Some(0x30),
        Some(0x3012),
        Some(0x30120201),
        Some(0x01),
        Some(0x66),
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_5_and9(
        6,
        843,
        0x18,
        PacketDir::Ingress,
        Some(54028),
        Some(389),
        Some(0x30),
        Some(0x300c),
        Some(0x300c0201),
        Some(0x01),
        Some(0x67),
        Some(0x00),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_modify_denied"))
    );
}

#[test]
fn ldap_modify_constraint_path_materializes_constraint_violation_phase() {
    let binding = compile_file(&dsl_fixture_path("ldap_modify_constraint_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 844, 54029, "ldapmodify"));
    session.ingest(route_fact(2, 844, 7));
    session.ingest(tcp_state_fact_with_ports(3, 844, 1, 2, 54029, 389));
    session.ingest(tcp_state_fact_with_ports(4, 844, 2, 3, 54029, 389));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_5_and9(
        5,
        844,
        0x18,
        PacketDir::Egress,
        Some(54029),
        Some(389),
        Some(0x30),
        Some(0x3012),
        Some(0x30120201),
        Some(0x01),
        Some(0x66),
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_5_and9(
        6,
        844,
        0x18,
        PacketDir::Ingress,
        Some(54029),
        Some(389),
        Some(0x30),
        Some(0x300c),
        Some(0x300c0201),
        Some(0x01),
        Some(0x67),
        Some(0x13),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("ldap_modify_constraint_violation".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_modify"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| { stage.phase.as_deref() == Some("receive_modify_constraint_violation") })
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
fn ldap_modify_constraint_path_does_not_match_access_denied_result_code() {
    let binding = compile_file(&dsl_fixture_path("ldap_modify_constraint_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 845, 54029, "ldapmodify"));
    session.ingest(route_fact(2, 845, 7));
    session.ingest(tcp_state_fact_with_ports(3, 845, 1, 2, 54029, 389));
    session.ingest(tcp_state_fact_with_ports(4, 845, 2, 3, 54029, 389));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_5_and9(
        5,
        845,
        0x18,
        PacketDir::Egress,
        Some(54029),
        Some(389),
        Some(0x30),
        Some(0x3012),
        Some(0x30120201),
        Some(0x01),
        Some(0x66),
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_5_and9(
        6,
        845,
        0x18,
        PacketDir::Ingress,
        Some(54029),
        Some(389),
        Some(0x30),
        Some(0x300c),
        Some(0x300c0201),
        Some(0x01),
        Some(0x67),
        Some(0x32),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| { stage.phase.as_deref() != Some("receive_modify_constraint_violation") })
    );
}
