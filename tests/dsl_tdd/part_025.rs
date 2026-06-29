use super::*;

#[test]
fn http_connect_authenticated_tunnel_path_materializes_auth_and_established_phases() {
    let binding = compile_file(&dsl_fixture_path(
        "http_connect_authenticated_tunnel_path.gewy",
    ))
    .unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8293, 53186, "proxy-client"));
    session.ingest(route_fact(2, 8293, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8293, 1, 2, 53186, 8080));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        8293,
        0x18,
        PacketDir::Egress,
        Some(53186),
        Some(8080),
        Some(0x43),
        Some(0x434f),
        Some(0x434f4e4e),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        8293,
        0x18,
        PacketDir::Ingress,
        Some(53186),
        Some(8080),
        Some(0x34),
        Some(0x3430),
        Some(0x34303720),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        6,
        8293,
        0x18,
        PacketDir::Egress,
        Some(53186),
        Some(8080),
        Some(0x43),
        Some(0x434f),
        Some(0x434f4e4e),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        7,
        8293,
        0x18,
        PacketDir::Ingress,
        Some(53186),
        Some(8080),
        Some(0x32),
        Some(0x3230),
        Some(0x32303020),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("http_connect_authenticated_tunnel".into())
    );
    for phase in [
        "send_connect_request",
        "receive_auth_required",
        "receive_connect_established",
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
fn http_connect_authenticated_tunnel_path_does_not_treat_407_as_established_without_auth_followup()
{
    let binding = compile_file(&dsl_fixture_path(
        "http_connect_authenticated_tunnel_path.gewy",
    ))
    .unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8294, 53186, "proxy-client"));
    session.ingest(route_fact(2, 8294, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8294, 1, 2, 53186, 8080));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        8294,
        0x18,
        PacketDir::Egress,
        Some(53186),
        Some(8080),
        Some(0x43),
        Some(0x434f),
        Some(0x434f4e4e),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        8294,
        0x18,
        PacketDir::Ingress,
        Some(53186),
        Some(8080),
        Some(0x34),
        Some(0x3430),
        Some(0x34303720),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_connect_established"))
    );
}

#[test]
fn sip_register_path_materializes_register_and_ok_datagrams() {
    let binding = compile_file(&dsl_fixture_path("sip_register_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 829, 54010, "sip-client"));
    session.ingest(route_fact(2, 829, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload_prefix4(
        3,
        829,
        180,
        PacketDir::Egress,
        Some(54010),
        Some(5060),
        Some(0x52),
        Some(0x5245),
        Some(0x52454749),
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload_prefix4(
        4,
        829,
        220,
        PacketDir::Ingress,
        Some(54010),
        Some(5060),
        Some(0x53),
        Some(0x5349),
        Some(0x5349502f),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("sip_register".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_register"))
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
    assert!(phase_kinds.contains(&"emit_datagram".to_string()));
    assert!(phase_kinds.contains(&"receive_datagram".to_string()));
    assert_eq!(export.module_findings.len(), 0);
}

#[test]
fn sip_register_path_does_not_match_wrong_response_prefix() {
    let binding = compile_file(&dsl_fixture_path("sip_register_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 830, 54010, "sip-client"));
    session.ingest(route_fact(2, 830, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload_prefix4(
        3,
        830,
        180,
        PacketDir::Egress,
        Some(54010),
        Some(5060),
        Some(0x52),
        Some(0x5245),
        Some(0x52454749),
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload_prefix4(
        4,
        830,
        220,
        PacketDir::Ingress,
        Some(54010),
        Some(5060),
        Some(0x52),
        Some(0x5245),
        Some(0x52455350),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_ok"))
    );
}

#[test]
fn ldap_bind_path_materializes_connect_bind_and_response_phases() {
    let binding = compile_file(&dsl_fixture_path("ldap_bind_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 831, 54020, "ldap-client"));
    session.ingest(route_fact(2, 831, 7));
    session.ingest(tcp_state_fact_with_ports(3, 831, 1, 2, 54020, 389));
    session.ingest(tcp_state_fact_with_ports(4, 831, 2, 3, 54020, 389));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        5,
        831,
        0x18,
        PacketDir::Egress,
        Some(54020),
        Some(389),
        Some(0x30),
        Some(0x300c),
        Some(0x300c0201),
        Some(0x01),
        Some(0x60),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        6,
        831,
        0x18,
        PacketDir::Ingress,
        Some(54020),
        Some(389),
        Some(0x30),
        Some(0x300c),
        Some(0x300c0201),
        Some(0x01),
        Some(0x61),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("ldap_bind".into())
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
            .any(|stage| stage.phase.as_deref() == Some("send_bind"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_bind_response"))
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
fn ldap_bind_path_does_not_match_wrong_response_op_tag() {
    let binding = compile_file(&dsl_fixture_path("ldap_bind_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 832, 54020, "ldap-client"));
    session.ingest(route_fact(2, 832, 7));
    session.ingest(tcp_state_fact_with_ports(3, 832, 1, 2, 54020, 389));
    session.ingest(tcp_state_fact_with_ports(4, 832, 2, 3, 54020, 389));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        5,
        832,
        0x18,
        PacketDir::Egress,
        Some(54020),
        Some(389),
        Some(0x30),
        Some(0x300c),
        Some(0x300c0201),
        Some(0x01),
        Some(0x60),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        6,
        832,
        0x18,
        PacketDir::Ingress,
        Some(54020),
        Some(389),
        Some(0x30),
        Some(0x300c),
        Some(0x300c0201),
        Some(0x01),
        Some(0x64),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_bind_response"))
    );
}

#[test]
fn ldap_search_path_materializes_connect_search_and_result_phases() {
    let binding = compile_file(&dsl_fixture_path("ldap_search_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 833, 54021, "ldapsearch"));
    session.ingest(route_fact(2, 833, 7));
    session.ingest(tcp_state_fact_with_ports(3, 833, 1, 2, 54021, 389));
    session.ingest(tcp_state_fact_with_ports(4, 833, 2, 3, 54021, 389));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        5,
        833,
        0x18,
        PacketDir::Egress,
        Some(54021),
        Some(389),
        Some(0x30),
        Some(0x3010),
        Some(0x30100201),
        Some(0x01),
        Some(0x63),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        6,
        833,
        0x18,
        PacketDir::Ingress,
        Some(54021),
        Some(389),
        Some(0x30),
        Some(0x300c),
        Some(0x300c0201),
        Some(0x01),
        Some(0x65),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("ldap_search".into())
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
            .any(|stage| stage.phase.as_deref() == Some("send_search"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_search_result"))
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
fn ldap_search_path_does_not_match_wrong_response_op_tag() {
    let binding = compile_file(&dsl_fixture_path("ldap_search_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 834, 54021, "ldapsearch"));
    session.ingest(route_fact(2, 834, 7));
    session.ingest(tcp_state_fact_with_ports(3, 834, 1, 2, 54021, 389));
    session.ingest(tcp_state_fact_with_ports(4, 834, 2, 3, 54021, 389));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        5,
        834,
        0x18,
        PacketDir::Egress,
        Some(54021),
        Some(389),
        Some(0x30),
        Some(0x3010),
        Some(0x30100201),
        Some(0x01),
        Some(0x63),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        6,
        834,
        0x18,
        PacketDir::Ingress,
        Some(54021),
        Some(389),
        Some(0x30),
        Some(0x300c),
        Some(0x300c0201),
        Some(0x01),
        Some(0x64),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_search_result"))
    );
}
