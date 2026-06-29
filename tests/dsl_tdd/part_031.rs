use super::*;

#[test]
fn amqp_publish_session_can_span_startup_and_publish_in_one_module() {
    let binding = compile_file(&dsl_fixture_path("amqp_publish_session.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 524, 7799, "amqp-publisher"));
    session.ingest(route_fact(2, 524, 6));
    session.ingest(tcp_state_fact_with_ports(3, 524, 1, 2, 43143, 5672));
    session.ingest(tcp_state_fact_with_ports(4, 524, 2, 3, 43143, 5672));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        524,
        0,
        PacketDir::Egress,
        Some(43143),
        Some(5672),
        None,
        None,
        Some(0x414d5150),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_byte10(
        6,
        524,
        0,
        PacketDir::Ingress,
        Some(43143),
        Some(5672),
        Some(0x01),
        Some(0x0a),
        None,
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_byte10(
        7,
        524,
        0,
        PacketDir::Egress,
        Some(43143),
        Some(5672),
        Some(0x01),
        Some(0x0b),
        None,
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_byte10(
        8,
        524,
        0,
        PacketDir::Egress,
        Some(43143),
        Some(5672),
        Some(0x01),
        Some(0x28),
        None,
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_byte10(
        9,
        524,
        0,
        PacketDir::Ingress,
        Some(43143),
        Some(5672),
        Some(0x01),
        Some(0x50),
        None,
        None,
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(100));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("amqp_publish_session".into())
    );
    let phases = export.program_flows[0]
        .stages
        .iter()
        .filter_map(|stage| stage.phase.clone())
        .collect::<Vec<_>>();
    assert!(phases.contains(&"connect".to_string()));
    assert!(phases.contains(&"establish".to_string()));
    assert!(phases.contains(&"send_protocol_header".to_string()));
    assert!(phases.contains(&"receive_start".to_string()));
    assert!(phases.contains(&"send_start_ok".to_string()));
    assert!(phases.contains(&"send_publish".to_string()));
    assert!(phases.contains(&"receive_ack".to_string()));
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
fn amqp_publish_session_missing_publish_produces_start_ok_to_publish_transition() {
    let binding = compile_file(&dsl_fixture_path("amqp_publish_session.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 525, 7800, "amqp-publisher"));
    session.ingest(route_fact(2, 525, 6));
    session.ingest(tcp_state_fact_with_ports(3, 525, 1, 2, 43144, 5672));
    session.ingest(tcp_state_fact_with_ports(4, 525, 2, 3, 43144, 5672));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        525,
        0,
        PacketDir::Egress,
        Some(43144),
        Some(5672),
        None,
        None,
        Some(0x414d5150),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_byte10(
        6,
        525,
        0,
        PacketDir::Ingress,
        Some(43144),
        Some(5672),
        Some(0x01),
        Some(0x0a),
        None,
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_byte10(
        7,
        525,
        0,
        PacketDir::Egress,
        Some(43144),
        Some(5672),
        Some(0x01),
        Some(0x0b),
        None,
        None,
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(90));

    let export = session.export_bundle();
    assert!(export.program_findings.iter().any(|finding| {
        finding.module_label == "amqp_publish_session"
            && finding.phase.as_deref() == Some("send_publish")
            && finding.phase_transition.as_deref() == Some("send_start_ok->send_publish")
    }));
    assert!(export.module_findings.iter().any(|finding| {
        finding
            .phase_transitions
            .contains(&"send_start_ok->send_publish".to_string())
    }));
}

#[test]
fn declarative_module_phases_are_preserved_in_export_and_replay() {
    let binding = compile_file(&dsl_fixture_path("postgres_connect_process.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 503, 7778, "psql"));
    session.ingest(tcp_state_fact_with_ports(2, 503, 1, 2, 43123, 5432));
    session.ingest(route_fact(3, 503, 6));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(40));

    let export = session.export_bundle();
    let phases = export.program_flows[0]
        .stages
        .iter()
        .filter_map(|stage| stage.phase.clone())
        .collect::<Vec<_>>();
    assert!(phases.contains(&"bind".to_string()));
    assert!(phases.contains(&"connect".to_string()));
    assert!(phases.contains(&"resolve".to_string()));

    let replay = gewyvern::export::ExportBundle::from_json(&export.to_json())
        .unwrap()
        .replay()
        .unwrap();
    assert_eq!(export.program_flows, replay.program_flows);
}

#[test]
fn missing_connect_phase_produces_bind_to_connect_transition_finding() {
    let binding = compile_file(&dsl_fixture_path("postgres_connect_process.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 504, 7779, "psql"));
    session.ingest(route_fact(2, 504, 6));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(40));

    let export = session.export_bundle();
    assert_eq!(export.program_findings.len(), 1);
    assert_eq!(export.program_findings[0].phase.as_deref(), Some("connect"));
    assert_eq!(
        export.program_findings[0].phase_transition.as_deref(),
        Some("bind->connect")
    );
    assert_eq!(
        export.program_findings[0].phase_transition_kind.as_deref(),
        Some("bind_process->initiate_connection")
    );
    assert!(export.program_findings[0].summary.contains("bind->connect"));
    assert_eq!(
        export.module_findings[0].phase_transitions,
        vec!["bind->connect".to_string()]
    );

    let replay = gewyvern::export::ExportBundle::from_json(&export.to_json())
        .unwrap()
        .replay()
        .unwrap();
    assert_eq!(export.program_findings, replay.program_findings);
    assert_eq!(export.module_findings, replay.module_findings);
}

#[test]
fn missing_establish_phase_produces_connect_to_establish_transition_finding() {
    let binding = compile_file(&dsl_fixture_path("postgres_connect_process.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 505, 7780, "psql"));
    session.ingest(tcp_state_fact_with_ports(2, 505, 1, 2, 43123, 5432));
    session.ingest(route_fact(3, 505, 6));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(40));

    let export = session.export_bundle();
    assert!(export.program_findings.iter().any(|finding| {
        finding.phase.as_deref() == Some("establish")
            && finding.phase_transition.as_deref() == Some("connect->establish")
    }));
    assert!(export.module_findings.iter().any(|finding| {
        finding
            .phase_transitions
            .contains(&"connect->establish".to_string())
    }));
}

#[test]
fn handshake_dsl_compiles_and_preserves_tcp_shape() {
    let binding = compile_file(&dsl_fixture_path("handshake_debug.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(tcp_state_fact(1, 203, 1, 2));
    session.ingest(route_fact(2, 203, 2));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(30));

    let export = session.export_bundle();
    assert_eq!(export.template_id, "handshake_debug");
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::ConnectFlow
    );
}

#[test]
fn dsl_supports_inline_window_and_infers_program_model_id() {
    let binding = compile_str(
        r#"
template(:udp_inline_debug)
|> window(duration_ms: 9000, lateness_ms: 450)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> fragment(:route_meta_fragment)
|> operation(:datagram_exchange)
|> program_rule(predicate: "datagram_observed:udp", stage: :datagram_observed, narrative: "static:inline udp activity observed", dedupe: true)
"#,
    )
    .unwrap();

    assert_eq!(
        binding.template.window_profile.as_ref().unwrap().id,
        "inline"
    );
    assert_eq!(
        binding
            .template
            .window_profile
            .as_ref()
            .unwrap()
            .duration_ms,
        9_000
    );
    assert_eq!(
        binding
            .template
            .window_profile
            .as_ref()
            .unwrap()
            .lateness_ms,
        450
    );
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().id,
        "udp_inline_debug_dsl_model"
    );
}

#[test]
fn dsl_accepts_pipeline_template_blocks() {
    let binding = compile_str(
        r#"
template(:structured_udp_debug)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> fragment(:route_meta_fragment)
|> fragment(:sock_lineage_fragment)
|> program_model(:structured_udp_debug_model)
|> operation(:datagram_exchange)
|> program_rule(predicate: :process_bound, stage: :process_bound, narrative: :process_bound, dedupe: true, module: :structured_udp_debug, phase: :bind)
|> program_rule(predicate: "datagram_observed:udp:local_to_remote", stage: :datagram_observed, narrative: :udp_datagram_sent, dedupe: true, module: :structured_udp_debug, phase: :send_request)
"#,
    )
    .unwrap();

    assert_eq!(binding.template.id, "structured_udp_debug");
    assert_eq!(
        binding.template.fragment_set,
        vec![
            "udp_packet_meta_fragment",
            "route_meta_fragment",
            "sock_lineage_fragment"
        ]
    );
    let model = binding.template.program_model.as_ref().unwrap();
    assert_eq!(model.id, "structured_udp_debug_model");
    assert_eq!(model.operation, ProgramOperation::DatagramExchange);
    assert_eq!(model.rules.len(), 2);
    assert_eq!(
        model.rules[1].module.as_deref(),
        Some("structured_udp_debug")
    );
    assert_eq!(model.rules[1].phase.as_deref(), Some("send_request"));
}

#[test]
fn dsl_accepts_pipeline_reason_model_blocks() {
    let binding = compile_str(
        r#"
template(:structured_reason_udp)
|> window(:default_5s)
|> fragment(:udp_packet_meta_fragment)
|> fragment(:route_meta_fragment)
|> fragment(:sock_lineage_fragment)
|> program_model(:structured_reason_udp_model)
|> operation(:datagram_exchange)
|> program_rule(predicate: :process_bound, stage: :process_bound, narrative: :process_bound, dedupe: true, module: :structured_reason_udp, phase: :bind)
|> reason_model(:structured_reason_udp_reason)
|> reason_rule(predicate: :process_bound, key_event: :process_identified, narrative: :process_bound, dedupe: true, module: :structured_reason_udp, phase: :bind)
"#,
    )
    .unwrap();

    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(reason) if reason.id == "structured_reason_udp_reason"
    ));
}

#[test]
fn dsl_accepts_pipeline_template_calls() {
    let binding = compile_str(
        r#"
template(:pipeline_udp_debug)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> fragment(:route_meta_fragment)
|> fragment(:sock_lineage_fragment)
|> operation(:datagram_exchange)
|> program_model(:pipeline_udp_debug_model)
|> program_rule(predicate: :process_bound, stage: :process_bound, narrative: :process_bound, dedupe: true, module: :pipeline_udp_debug, phase: :bind)
|> program_rule(predicate: "datagram_observed:udp:local_to_remote", stage: :datagram_observed, narrative: :udp_datagram_sent, dedupe: true, module: :pipeline_udp_debug, phase: :send_request)
|> param(:sock_lineage_fragment.capture_comm, true)
"#,
    )
    .unwrap();

    assert_eq!(binding.template.id, "pipeline_udp_debug");
    assert_eq!(
        binding.template.fragment_set,
        vec![
            "udp_packet_meta_fragment",
            "route_meta_fragment",
            "sock_lineage_fragment"
        ]
    );
    let model = binding.template.program_model.as_ref().unwrap();
    assert_eq!(model.id, "pipeline_udp_debug_model");
    assert_eq!(model.operation, ProgramOperation::DatagramExchange);
    assert_eq!(model.rules.len(), 2);
    assert_eq!(
        binding.fragment_params["sock_lineage_fragment"]["capture_comm"],
        FragmentParamValue::Bool(true)
    );
}

#[test]
fn dsl_accepts_pipeline_reason_rule_calls() {
    let binding = compile_str(
        r#"
template(:pipeline_reason_udp)
|> window(duration_ms: 5000, lateness_ms: 200)
|> fragment(:udp_packet_meta_fragment)
|> fragment(:route_meta_fragment)
|> fragment(:sock_lineage_fragment)
|> operation(:datagram_exchange)
|> program_model(:pipeline_reason_udp_model)
|> program_rule(predicate: :process_bound, stage: :process_bound, narrative: :process_bound, dedupe: true, module: :pipeline_reason_udp, phase: :bind)
|> reason_model(:pipeline_reason_udp_reason)
|> reason_rule(predicate: :process_bound, key_event: :process_identified, narrative: :process_bound, dedupe: true, module: :pipeline_reason_udp, phase: :bind)
"#,
    )
    .unwrap();

    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(reason) if reason.id == "pipeline_reason_udp_reason"
    ));
}

#[test]
fn dsl_accepts_pipeline_function_units_without_global_state() {
    let binding = compile_str(
        r#"
fn udp_core() {
  |> fragment(:udp_packet_meta_fragment)
  |> fragment(:route_meta_fragment)
  |> fragment(:sock_lineage_fragment)
  |> operation(:datagram_exchange)
  |> program_model(:pipeline_function_udp_model)
  |> program_rule(predicate: :process_bound, stage: :process_bound, narrative: :process_bound, dedupe: true, module: :pipeline_function_udp, phase: :bind)
}

template(:pipeline_function_udp)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> use(:udp_core)
|> param(:sock_lineage_fragment.capture_comm, true)
"#,
    )
    .unwrap();

    assert_eq!(binding.template.id, "pipeline_function_udp");
    assert_eq!(
        binding.template.fragment_set,
        vec![
            "udp_packet_meta_fragment",
            "route_meta_fragment",
            "sock_lineage_fragment"
        ]
    );
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().id,
        "pipeline_function_udp_model"
    );
    assert_eq!(
        binding.fragment_params["sock_lineage_fragment"]["capture_comm"],
        FragmentParamValue::Bool(true)
    );
}

#[test]
fn dsl_accepts_parameterized_pipeline_function_units() {
    let binding = compile_str(
        r#"
fn udp_core(model_name, op_name) {
  |> fragment(:udp_packet_meta_fragment)
  |> fragment(:route_meta_fragment)
  |> fragment(:sock_lineage_fragment)
  |> operation(${op_name})
  |> program_model(${model_name})
  |> program_rule(predicate: :process_bound, stage: :process_bound, narrative: :process_bound, dedupe: true, module: ${model_name}, phase: :bind)
}

template(:pipeline_parameter_fn_udp)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> use(:udp_core, :pipeline_parameter_fn_udp_model, :datagram_exchange)
"#,
    )
    .unwrap();

    assert_eq!(binding.template.id, "pipeline_parameter_fn_udp");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().id,
        "pipeline_parameter_fn_udp_model"
    );
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::DatagramExchange
    );
}
