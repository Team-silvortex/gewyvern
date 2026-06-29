use super::*;

#[test]
fn built_in_ftp_active_retr_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("ftp_active_retr_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "ftp_active_retr_path");
    assert_eq!(
        binding.template.program_model.unwrap().operation,
        ProgramOperation::Custom("ftp_active_retr".into())
    );
}

#[test]
fn built_in_ftp_active_stor_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("ftp_active_stor_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "ftp_active_stor_path");
    assert_eq!(
        binding.template.program_model.unwrap().operation,
        ProgramOperation::Custom("ftp_active_stor".into())
    );
}

#[test]
fn built_in_ssh_session_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("ssh_session_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "ssh_session_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("ssh_session".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_ssh_auth_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("ssh_auth_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "ssh_auth_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("ssh_auth".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_ssh_auth_denied_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("ssh_auth_denied_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "ssh_auth_denied_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("ssh_auth_denied".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_ssh_channel_session_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("ssh_channel_session_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "ssh_channel_session_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("ssh_channel_session".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_socks5_session_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("socks5_session_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "socks5_session_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("socks5_session".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_socks5_auth_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("socks5_auth_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "socks5_auth_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("socks5_auth".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_socks5_auth_denied_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("socks5_auth_denied_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "socks5_auth_denied_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("socks5_auth_denied".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_socks5_auth_connect_denied_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("socks5_auth_connect_denied_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "socks5_auth_connect_denied_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("socks5_auth_connect_denied".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_socks5_denied_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("socks5_denied_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "socks5_denied_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("socks5_denied".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_http_connect_tunnel_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("http_connect_tunnel_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "http_connect_tunnel_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("http_connect_tunnel".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_http_connect_denied_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("http_connect_denied_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "http_connect_denied_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("http_connect_denied".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_http_connect_auth_required_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("http_connect_auth_required_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "http_connect_auth_required_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("http_connect_auth_required".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_http_connect_authenticated_tunnel_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path(
        "http_connect_authenticated_tunnel_path.gewy",
    ))
    .unwrap();

    assert_eq!(
        binding.template.id,
        "http_connect_authenticated_tunnel_path"
    );
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("http_connect_authenticated_tunnel".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_sip_register_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("sip_register_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "sip_register_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("sip_register".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_ldap_bind_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("ldap_bind_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "ldap_bind_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("ldap_bind".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_ldap_search_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("ldap_search_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "ldap_search_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("ldap_search".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_ldap_modify_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("ldap_modify_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "ldap_modify_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("ldap_modify".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_ldap_bind_denied_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("ldap_bind_denied_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "ldap_bind_denied_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("ldap_bind_denied".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_ldap_modify_denied_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("ldap_modify_denied_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "ldap_modify_denied_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("ldap_modify_denied".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_ldap_modify_constraint_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("ldap_modify_constraint_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "ldap_modify_constraint_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("ldap_modify_constraint_violation".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_ldap_directory_session_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("ldap_directory_session.gewy")).unwrap();

    assert_eq!(binding.template.id, "ldap_directory_session");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("ldap_directory_session".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_ldap_directory_write_session_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("ldap_directory_write_session.gewy")).unwrap();

    assert_eq!(binding.template.id, "ldap_directory_write_session");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("ldap_directory_write_session".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_ldap_directory_sync_session_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("ldap_directory_sync_session.gewy")).unwrap();

    assert_eq!(binding.template.id, "ldap_directory_sync_session");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("ldap_directory_sync_session".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_snmp_get_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("snmp_get_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "snmp_get_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("snmp_get".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_dns_tcp_query_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("dns_tcp_query_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "dns_tcp_query_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("dns_tcp_query".into())
    );
}

#[test]
fn udp_process_dsl_binding_drives_runtime_session() {
    let binding = compile_file(&dsl_fixture_path("udp_process_debug.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 201, 4242, "curl"));
    session.ingest(udp_packet_fact(2, 201, 88));
    session.ingest(route_fact(3, 201, 5));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(40));

    let export = session.export_bundle();
    assert_eq!(export.template_id, "udp_process_debug");
    assert_eq!(export.program_flows.len(), 1);
    assert_eq!(export.flows[0].process.as_ref().unwrap().comm, "curl");
}

#[test]
fn dsl_supports_custom_predicates_and_fragment_params() {
    let binding = compile_str(
        r#"
template(:udp_dns_debug)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> fragment(:route_meta_fragment)
|> fragment(:sock_lineage_fragment)
|> program_model(:dns_lookup_v1)
|> operation(:dns_lookup)
|> program_rule(predicate: "all(process_bound,datagram_observed:udp)", stage: :datagram_observed, narrative: "static:process-owned dns datagram", dedupe: true)
|> program_rule(predicate: "any(route_resolved,socket_state_observed)", stage: :route_resolved, narrative: "static:upstream path or socket progress observed", dedupe: true)
|> param(:sock_lineage_fragment.capture_comm, false)
|> param(:udp_packet_meta_fragment.min_len, 80)
"#,
    )
    .unwrap();

    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 202, 5353, "dig"));
    session.ingest(udp_packet_fact(2, 202, 72));
    session.ingest(route_fact(3, 202, 7));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(40));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("dns_lookup".into())
    );
    assert_eq!(export.flows[0].process.as_ref().unwrap().comm, "<redacted>");
    assert_eq!(export.rejected_facts.len(), 1);
    assert_eq!(
        export.rejected_fact_summary[0].reason,
        "filtered_by_fragment_param"
    );
}

#[test]
fn dns_dsl_uses_egress_direction_to_model_lookup_requests() {
    let binding = compile_file(&dsl_fixture_path("dns_udp_process.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 303, 5353, "dig"));
    session.ingest(route_fact(2, 303, 7));
    session.ingest(udp_packet_fact_with_dir(3, 303, 96, PacketDir::Egress));
    session.ingest(udp_packet_fact_with_dir(4, 303, 96, PacketDir::Ingress));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(50));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("dns_lookup".into())
    );
    assert!(
        export.program_flows[0]
            .narrative
            .iter()
            .any(|line| line == "program emitted a UDP datagram")
    );
    assert!(
        export.program_flows[0]
            .narrative
            .iter()
            .any(|line| line == "program received a UDP datagram")
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_reply"))
    );
    assert_eq!(export.module_findings.len(), 0);
    assert_eq!(
        export.reasons[0]
            .l1
            .key_events
            .iter()
            .filter(|event| event.kind == KeyEventKind::UdpDatagramSeen)
            .count(),
        2
    );
}
