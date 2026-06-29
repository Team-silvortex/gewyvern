use super::*;

#[test]
fn built_in_udp_process_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("udp_process_debug.gewy")).unwrap();

    assert_eq!(binding.template.id, "udp_process_debug");
    assert_eq!(
        binding.template.fragment_set,
        vec![
            "udp_packet_meta_fragment",
            "route_meta_fragment",
            "sock_lineage_fragment"
        ]
    );
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::DatagramExchange
    );
    assert_eq!(
        binding
            .template
            .window_profile
            .as_ref()
            .unwrap()
            .duration_ms,
        5_000
    );
    assert_eq!(
        binding
            .template
            .window_profile
            .as_ref()
            .unwrap()
            .lateness_ms,
        200
    );
    assert_eq!(
        binding.fragment_params["sock_lineage_fragment"]["capture_comm"],
        FragmentParamValue::Bool(true)
    );
}

#[test]
fn built_in_structured_udp_process_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("structured_udp_process_debug.gewy")).unwrap();

    assert_eq!(binding.template.id, "structured_udp_process_debug");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::DatagramExchange
    );
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().rules.len(),
        3
    );
}

#[test]
fn built_in_pipeline_udp_process_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("pipeline_udp_process_debug.gewy")).unwrap();

    assert_eq!(binding.template.id, "pipeline_udp_process_debug");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::DatagramExchange
    );
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().rules.len(),
        3
    );
}

#[test]
fn pipeline_dsl_supports_inline_and_block_comments() {
    let binding = compile_str(
        r#"
        # header comment
        template(:commented_demo) # inline comment
        |> window(:default_5s)
        |> reason(:udp_datagram_l1)
        /* this reusable fragment bundle is intentionally small */
        |> fragment(:udp_packet_meta_fragment)
        |> fragment(:route_meta_fragment)
        |> fragment(:sock_lineage_fragment)
        |> operation(:datagram_exchange)
        |> program_model(:commented_demo_model)
        |> program_rule(predicate: :process_bound, stage: :process_bound, narrative: :process_bound, dedupe: true)
        "#,
    )
    .unwrap();

    assert_eq!(binding.template.id, "commented_demo");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::DatagramExchange
    );
}

#[test]
fn pipeline_frontend_surfaces_module_and_function_docs() {
    let source = r#"
//! Demo UDP module
//! Focused on readable authoring
/// Reusable UDP rules
fn udp_rules() {
  |> operation(:datagram_exchange)
  |> program_model(:frontend_doc_model)
  |> program_rule(predicate: :process_bound, stage: :process_bound, narrative: :process_bound, dedupe: true, module: :frontend_doc, phase: :bind)
}

/// Entry template for the documented demo
template(:frontend_doc)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> fragment(:route_meta_fragment)
|> fragment(:sock_lineage_fragment)
|> use(:udp_rules)
"#;
    let frontend = parse_str_unvalidated(source).unwrap();

    assert_eq!(frontend.template.id, "frontend_doc");
    let summary = gewyvern::dsl::summarize_frontend_str(source).unwrap();
    assert_eq!(
        summary.module_doc.as_deref(),
        Some("Demo UDP module\nFocused on readable authoring")
    );
    assert_eq!(
        summary.template_doc.as_deref(),
        Some("Entry template for the documented demo")
    );
    assert_eq!(
        summary
            .function_nodes
            .iter()
            .find(|node| node.name == "udp_rules")
            .and_then(|node| node.doc.as_deref()),
        Some("Reusable UDP rules")
    );
}

#[test]
fn built_in_dns_udp_process_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("dns_udp_process.gewy")).unwrap();

    assert_eq!(binding.template.id, "dns_udp_process");
    assert_eq!(
        binding.template.fragment_set,
        vec![
            "udp_packet_meta_fragment",
            "route_meta_fragment",
            "sock_lineage_fragment"
        ]
    );
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("dns_lookup".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_https_connect_process_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("https_connect_process.gewy")).unwrap();

    assert_eq!(binding.template.id, "https_connect_process");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("https_connect".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_postgres_connect_process_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("postgres_connect_process.gewy")).unwrap();

    assert_eq!(binding.template.id, "postgres_connect_process");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("postgres_connect".into())
    );
}

#[test]
fn built_in_postgres_simple_query_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("postgres_simple_query_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "postgres_simple_query_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("postgres_simple_query".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_postgres_auth_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("postgres_auth_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "postgres_auth_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("postgres_auth".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_postgres_query_error_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("postgres_query_error_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "postgres_query_error_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("postgres_query_error".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_mysql_connect_process_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("mysql_connect_process.gewy")).unwrap();

    assert_eq!(binding.template.id, "mysql_connect_process");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("mysql_connect".into())
    );
}

#[test]
fn built_in_mysql_simple_query_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("mysql_simple_query_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "mysql_simple_query_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("mysql_simple_query".into())
    );
}

#[test]
fn built_in_mysql_query_session_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("mysql_query_session.gewy")).unwrap();

    assert_eq!(binding.template.id, "mysql_query_session");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("mysql_query_session".into())
    );
}

#[test]
fn built_in_mysql_query_error_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("mysql_query_error_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "mysql_query_error_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("mysql_query_error".into())
    );
}

#[test]
fn built_in_memcached_get_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("memcached_get_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "memcached_get_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("memcached_get".into())
    );
}

#[test]
fn built_in_memcached_set_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("memcached_set_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "memcached_set_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("memcached_set".into())
    );
}

#[test]
fn built_in_amqp_connection_start_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("amqp_connection_start_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "amqp_connection_start_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("amqp_connection_start".into())
    );
}

#[test]
fn built_in_amqp_basic_publish_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("amqp_basic_publish_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "amqp_basic_publish_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("amqp_basic_publish".into())
    );
}

#[test]
fn built_in_amqp_publish_session_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("amqp_publish_session.gewy")).unwrap();

    assert_eq!(binding.template.id, "amqp_publish_session");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("amqp_publish_session".into())
    );
}

#[test]
fn built_in_gtpu_echo_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("gtpu_echo_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "gtpu_echo_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("gtpu_echo".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_http_request_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("http_request_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "http_request_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("http_request".into())
    );
}

#[test]
fn built_in_http_server_response_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("http_server_response_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "http_server_response_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("http_server_response".into())
    );
}

#[test]
fn built_in_http3_request_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("http3_request_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "http3_request_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("http3_request".into())
    );
}

#[test]
fn built_in_http3_server_response_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("http3_server_response_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "http3_server_response_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("http3_server_response".into())
    );
}

#[test]
fn built_in_hy2_auth_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("hy2_auth_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "hy2_auth_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("hy2_auth".into())
    );
}

#[test]
fn built_in_hy2_udp_relay_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("hy2_udp_relay_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "hy2_udp_relay_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("hy2_udp_relay".into())
    );
}

#[test]
fn built_in_hy2_tcp_relay_path_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("hy2_tcp_relay_path.gewy")).unwrap();

    assert_eq!(binding.template.id, "hy2_tcp_relay_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("hy2_tcp_relay".into())
    );
}

#[test]
fn dsl_accepts_local_remote_port_predicates() {
    let local_binding = compile_str(
        r#"
template(:http_server_compat)
|> window(:default_5s)
|> reason(:handshake_l1)
|> fragment(:tcp_state_fragment)
|> program_model(:http_server_compat_model)
|> operation(:http_server_response)
|> program_rule(predicate: "socket_state_observed:local:http", stage: :socket_state_transition, narrative: "static:local http socket observed", dedupe: true)
"#,
    )
    .unwrap();
    let remote_binding = compile_str(
        r#"
template(:http_client_remote)
|> window(:default_5s)
|> reason(:handshake_l1)
|> fragment(:tcp_state_fragment)
|> program_model(:http_client_remote_model)
|> operation(:http_request)
|> program_rule(predicate: "socket_state_observed:remote:https", stage: :socket_state_transition, narrative: "static:remote https socket observed", dedupe: true)
"#,
    )
    .unwrap();

    let local_rule = &local_binding.template.program_model.as_ref().unwrap().rules[0];
    let remote_rule = &remote_binding
        .template
        .program_model
        .as_ref()
        .unwrap()
        .rules[0];

    assert_eq!(
        local_rule.predicate,
        gewyvern::ir::FlowPredicate::SocketStateObserved {
            local_port: Some(80),
            remote_port: None,
            min_new_state: None,
        }
    );
    assert_eq!(
        remote_rule.predicate,
        gewyvern::ir::FlowPredicate::SocketStateObserved {
            local_port: None,
            remote_port: Some(443),
            min_new_state: None,
        }
    );
}
