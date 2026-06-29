use super::*;

#[test]
fn built_in_tls_client_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("tls_client_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "tls_client_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("tls_client".into())
    );
}

#[test]
fn built_in_quic_client_initial_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("quic_client_initial_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "quic_client_initial_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("quic_client_initial".into())
    );
}

#[test]
fn built_in_quic_crypto_handshake_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("quic_crypto_handshake_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "quic_crypto_handshake_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("quic_crypto_handshake".into())
    );
}

#[test]
fn built_in_quic_stream_session_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("quic_stream_session_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "quic_stream_session_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("quic_stream_session".into())
    );
}

#[test]
fn built_in_quic_bidi_stream_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("quic_bidi_stream_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "quic_bidi_stream_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("quic_bidi_stream".into())
    );
}

#[test]
fn built_in_stun_binding_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("stun_binding_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "stun_binding_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("stun_binding".into())
    );
}

#[test]
fn built_in_coap_get_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("coap_get_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "coap_get_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("coap_get".into())
    );
}

#[test]
fn built_in_ntp_client_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("ntp_client_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "ntp_client_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("ntp_client".into())
    );
}

#[test]
fn built_in_dhcp_client_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("dhcp_client_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "dhcp_client_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("dhcp_client".into())
    );
}

#[test]
fn built_in_wireguard_handshake_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("wireguard_handshake_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "wireguard_handshake_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("wireguard_handshake".into())
    );
}

#[test]
fn built_in_mdns_query_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("mdns_query_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "mdns_query_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("mdns_query".into())
    );
}

#[test]
fn built_in_ssdp_discovery_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("ssdp_discovery_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "ssdp_discovery_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("ssdp_discovery".into())
    );
}

#[test]
fn built_in_redis_ping_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("redis_ping_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "redis_ping_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("redis_ping".into())
    );
}

#[test]
fn built_in_mqtt_connect_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("mqtt_connect_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "mqtt_connect_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("mqtt_connect".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_radius_access_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("radius_access_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "radius_access_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("radius_access".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_smtp_session_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("smtp_session_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "smtp_session_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("smtp_session".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_smtp_auth_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("smtp_auth_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "smtp_auth_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("smtp_auth".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_imap_auth_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("imap_auth_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "imap_auth_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("imap_auth".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_imap_auth_denied_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("imap_auth_denied_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "imap_auth_denied_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("imap_auth_denied".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_imap_select_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("imap_select_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "imap_select_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("imap_select".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_pop3_auth_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("pop3_auth_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "pop3_auth_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("pop3_auth".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_pop3_auth_denied_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("pop3_auth_denied_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "pop3_auth_denied_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("pop3_auth_denied".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_pop3_list_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("pop3_list_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "pop3_list_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("pop3_list".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_kerberos_as_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("kerberos_as_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "kerberos_as_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("kerberos_as".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_kerberos_as_error_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("kerberos_as_error_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "kerberos_as_error_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("kerberos_as_error".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_kerberos_tgs_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("kerberos_tgs_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "kerberos_tgs_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("kerberos_tgs".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_rtsp_options_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("rtsp_options_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "rtsp_options_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("rtsp_options".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_rtsp_describe_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("rtsp_describe_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "rtsp_describe_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("rtsp_describe".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_rtsp_setup_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("rtsp_setup_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "rtsp_setup_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("rtsp_setup".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_smtp_mail_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("smtp_mail_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "smtp_mail_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("smtp_mail".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_smtp_rcpt_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("smtp_rcpt_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "smtp_rcpt_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("smtp_rcpt".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_smtp_data_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("smtp_data_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "smtp_data_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("smtp_data".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_smtp_data_denied_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("smtp_data_denied_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "smtp_data_denied_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("smtp_data_denied".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_smtp_rcpt_denied_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("smtp_rcpt_denied_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "smtp_rcpt_denied_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("smtp_rcpt_denied".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_ftp_session_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("ftp_session_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "ftp_session_path");
    assert_eq!(
        binding.template.program_model.unwrap().operation,
        ProgramOperation::Custom("ftp_session".into())
    );
}

#[test]
fn built_in_ftp_denied_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("ftp_denied_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "ftp_denied_path");
    assert_eq!(
        binding.template.program_model.unwrap().operation,
        ProgramOperation::Custom("ftp_denied".into())
    );
}

#[test]
fn built_in_ftp_passive_list_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("ftp_passive_list_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "ftp_passive_list_path");
    assert_eq!(
        binding.template.program_model.unwrap().operation,
        ProgramOperation::Custom("ftp_passive_list".into())
    );
}

#[test]
fn built_in_ftp_retr_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("ftp_retr_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "ftp_retr_path");
    assert_eq!(
        binding.template.program_model.unwrap().operation,
        ProgramOperation::Custom("ftp_retr".into())
    );
}

#[test]
fn built_in_ftp_stor_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("ftp_stor_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "ftp_stor_path");
    assert_eq!(
        binding.template.program_model.unwrap().operation,
        ProgramOperation::Custom("ftp_stor".into())
    );
}

#[test]
fn built_in_ftp_active_list_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("ftp_active_list_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "ftp_active_list_path");
    assert_eq!(
        binding.template.program_model.unwrap().operation,
        ProgramOperation::Custom("ftp_active_list".into())
    );
}
