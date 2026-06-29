use super::*;

#[test]
fn mdns_query_path_materializes_query_and_response_datagrams() {
    let binding = compile_file(&dsl_fixture_path("mdns_query_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 819, 5353, "avahi-daemon"));
    session.ingest(route_fact(2, 819, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload_prefix4(
        3,
        819,
        64,
        PacketDir::Egress,
        Some(5353),
        Some(5353),
        Some(0x00),
        Some(0x0000),
        Some(0x00000000),
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload_prefix4(
        4,
        819,
        96,
        PacketDir::Ingress,
        Some(5353),
        Some(5353),
        Some(0x00),
        Some(0x0000),
        Some(0x00008400),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("mdns_query".into())
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
    assert!(phase_kinds.contains(&"emit_datagram".to_string()));
    assert!(phase_kinds.contains(&"receive_datagram".to_string()));
    assert_eq!(export.module_findings.len(), 0);
}

#[test]
fn mdns_query_path_does_not_match_wrong_response_flags() {
    let binding = compile_file(&dsl_fixture_path("mdns_query_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 820, 5353, "avahi-daemon"));
    session.ingest(route_fact(2, 820, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload_prefix4(
        3,
        820,
        64,
        PacketDir::Egress,
        Some(5353),
        Some(5353),
        Some(0x00),
        Some(0x0000),
        Some(0x00000000),
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload_prefix4(
        4,
        820,
        96,
        PacketDir::Ingress,
        Some(5353),
        Some(5353),
        Some(0x00),
        Some(0x0000),
        Some(0x00000400),
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
fn ssdp_discovery_path_materializes_search_and_response_datagrams() {
    let binding = compile_file(&dsl_fixture_path("ssdp_discovery_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 821, 1900, "ssdp-client"));
    session.ingest(route_fact(2, 821, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload_prefix4(
        3,
        821,
        96,
        PacketDir::Egress,
        Some(1900),
        Some(1900),
        Some(0x4d),
        Some(0x4d2d),
        Some(0x4d2d5345),
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload_prefix4(
        4,
        821,
        180,
        PacketDir::Ingress,
        Some(1900),
        Some(1900),
        Some(0x48),
        Some(0x4854),
        Some(0x48545450),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("ssdp_discovery".into())
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
            .any(|stage| stage.phase.as_deref() == Some("receive_response"))
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
fn ssdp_discovery_path_does_not_match_wrong_response_prefix() {
    let binding = compile_file(&dsl_fixture_path("ssdp_discovery_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 822, 1900, "ssdp-client"));
    session.ingest(route_fact(2, 822, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload_prefix4(
        3,
        822,
        96,
        PacketDir::Egress,
        Some(1900),
        Some(1900),
        Some(0x4d),
        Some(0x4d2d),
        Some(0x4d2d5345),
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload_prefix4(
        4,
        822,
        180,
        PacketDir::Ingress,
        Some(1900),
        Some(1900),
        Some(0x4e),
        Some(0x4e4f),
        Some(0x4e4f5459),
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
fn redis_ping_path_materializes_request_and_response_payload_phases() {
    let binding = compile_file(&dsl_fixture_path("redis_ping_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 823, 53001, "redis-cli"));
    session.ingest(route_fact(2, 823, 7));
    session.ingest(packet_fact_with_dir_and_payload(
        3,
        823,
        0x18,
        PacketDir::Egress,
        Some(53001),
        Some(6379),
        Some(0x2a),
        Some(0x2a31),
        Some(0x2a310d0a),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        823,
        0x18,
        PacketDir::Ingress,
        Some(53001),
        Some(6379),
        Some(0x2b),
        Some(0x2b50),
        Some(0x2b504f4e),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("redis_ping".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_ping"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_pong"))
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
fn redis_ping_path_does_not_match_wrong_response_prefix() {
    let binding = compile_file(&dsl_fixture_path("redis_ping_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 824, 53001, "redis-cli"));
    session.ingest(route_fact(2, 824, 7));
    session.ingest(packet_fact_with_dir_and_payload(
        3,
        824,
        0x18,
        PacketDir::Egress,
        Some(53001),
        Some(6379),
        Some(0x2a),
        Some(0x2a31),
        Some(0x2a310d0a),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        824,
        0x18,
        PacketDir::Ingress,
        Some(53001),
        Some(6379),
        Some(0x2d),
        Some(0x2d45),
        Some(0x2d455252),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_pong"))
    );
}

#[test]
fn mqtt_connect_path_materializes_connect_and_connack_payload_phases() {
    let binding = compile_file(&dsl_fixture_path("mqtt_connect_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 824, 53002, "mosquitto-pub"));
    session.ingest(route_fact(2, 824, 7));
    session.ingest(packet_fact_with_dir_and_payload(
        3,
        824,
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
        824,
        0x18,
        PacketDir::Ingress,
        Some(53002),
        Some(1883),
        Some(0x20),
        Some(0x2002),
        Some(0x20020000),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("mqtt_connect".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_connect"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_connack"))
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
fn radius_access_path_materializes_request_and_accept_datagrams() {
    let binding = compile_file(&dsl_fixture_path("radius_access_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 825, 53000, "wpa_supplicant"));
    session.ingest(route_fact(2, 825, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        825,
        96,
        PacketDir::Egress,
        Some(53000),
        Some(1812),
        Some(0x01),
        None,
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        4,
        825,
        96,
        PacketDir::Ingress,
        Some(53000),
        Some(1812),
        Some(0x02),
        None,
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("radius_access".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_access_request"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_access_accept"))
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
fn radius_access_path_does_not_match_wrong_response_code() {
    let binding = compile_file(&dsl_fixture_path("radius_access_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 826, 53000, "wpa_supplicant"));
    session.ingest(route_fact(2, 826, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        826,
        96,
        PacketDir::Egress,
        Some(53000),
        Some(1812),
        Some(0x01),
        None,
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        4,
        826,
        96,
        PacketDir::Ingress,
        Some(53000),
        Some(1812),
        Some(0x03),
        None,
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_access_accept"))
    );
}

#[test]
fn smtp_session_path_materializes_connect_banner_and_ehlo_phases() {
    let binding = compile_file(&dsl_fixture_path("smtp_session_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 827, 53010, "postfix-client"));
    session.ingest(route_fact(2, 827, 7));
    session.ingest(tcp_state_fact_with_ports(3, 827, 1, 2, 53010, 25));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        827,
        0x18,
        PacketDir::Ingress,
        Some(53010),
        Some(25),
        Some(0x32),
        Some(0x3232),
        Some(0x32323020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        827,
        0x18,
        PacketDir::Egress,
        Some(53010),
        Some(25),
        Some(0x45),
        Some(0x4548),
        Some(0x45484c4f),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(70));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("smtp_session".into())
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
            .any(|stage| stage.phase.as_deref() == Some("receive_banner"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_ehlo"))
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
