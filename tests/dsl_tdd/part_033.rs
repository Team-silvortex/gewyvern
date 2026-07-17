use super::*;

#[test]
fn dsl_package_entry_can_include_pipeline_module_from_named_source_dependency() {
    let root =
        std::env::temp_dir().join(format!("gewy-package-{}-source-deps", std::process::id()));
    let app_dir = root.join("app");
    let registry_dir = root.join("registry");
    let dep_dir = registry_dir.join("udp_stdlib");
    fs::create_dir_all(&app_dir).unwrap();
    fs::create_dir_all(&dep_dir).unwrap();

    fs::write(
        app_dir.join("gewy.pkg"),
        format!(
            "name=app_with_source_dep\nversion=0.1.0\nentry=main.gewy\nsource.local={}\ndep.std=source:local/udp_stdlib\n",
            registry_dir.to_string_lossy()
        ),
    )
    .unwrap();
    fs::write(
        app_dir.join("main.gewy"),
        r#"
template(:app_with_source_dep)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> include("std:udp_module.gewy")
"#,
    )
    .unwrap();
    fs::write(
        dep_dir.join("udp_module.gewy"),
        r#"
|> fragment(:udp_packet_meta_fragment)
|> fragment(:route_meta_fragment)
|> fragment(:sock_lineage_fragment)
|> operation(:datagram_exchange)
|> program_model(:app_with_source_dep_model)
|> program_rule(predicate: :process_bound, stage: :process_bound, narrative: :process_bound, dedupe: true, module: :app_with_source_dep, phase: :bind)
"#,
    )
    .unwrap();

    let binding = compile_file(app_dir.to_str().unwrap()).unwrap();
    assert_eq!(binding.template.id, "app_with_source_dep");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().id,
        "app_with_source_dep_model"
    );
}

#[test]
fn dsl_package_entry_can_use_function_defined_in_included_module() {
    let package_dir =
        std::env::temp_dir().join(format!("gewy-package-{}-functions", std::process::id()));
    fs::create_dir_all(&package_dir).unwrap();
    fs::write(
        package_dir.join("gewy.pkg"),
        "name=package_fn_udp\nversion=0.1.0\nentry=main.gewy\n",
    )
    .unwrap();
    fs::write(
        package_dir.join("main.gewy"),
        r#"
template(:package_fn_udp)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> include("./module.gewy")
|> use(:udp_core)
"#,
    )
    .unwrap();
    fs::write(
        package_dir.join("module.gewy"),
        r#"
fn udp_core() {
  |> fragment(:udp_packet_meta_fragment)
  |> fragment(:route_meta_fragment)
  |> fragment(:sock_lineage_fragment)
  |> operation(:datagram_exchange)
  |> program_model(:package_fn_udp_model)
  |> program_rule(predicate: :process_bound, stage: :process_bound, narrative: :process_bound, dedupe: true, module: :package_fn_udp, phase: :bind)
}
"#,
    )
    .unwrap();

    let binding = compile_file(package_dir.to_str().unwrap()).unwrap();
    assert_eq!(binding.template.id, "package_fn_udp");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().id,
        "package_fn_udp_model"
    );
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::DatagramExchange
    );
}

#[test]
fn dsl_package_entry_rejects_include_that_escapes_package_root() {
    let root = std::env::temp_dir().join(format!("gewy-package-{}-escape", std::process::id()));
    let package_dir = root.join("app");
    fs::create_dir_all(&package_dir).unwrap();
    fs::write(
        package_dir.join("gewy.pkg"),
        "name=escape_guard\nversion=0.1.0\nentry=main.gewy\n",
    )
    .unwrap();
    fs::write(
        package_dir.join("main.gewy"),
        r#"
template(:escape_guard)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> include("../outside.gewy")
"#,
    )
    .unwrap();
    fs::write(
        root.join("outside.gewy"),
        "|> fragment(:udp_packet_meta_fragment)\n",
    )
    .unwrap();

    let err = compile_file(package_dir.to_str().unwrap()).unwrap_err();
    assert!(
        format!("{err:?}").contains("escapes package root"),
        "unexpected error: {err:?}"
    );
}

#[test]
fn dsl_package_dependency_include_rejects_escape_from_dependency_root() {
    let root = std::env::temp_dir().join(format!("gewy-package-{}-dep-escape", std::process::id()));
    let app_dir = root.join("app");
    let dep_dir = root.join("dep");
    fs::create_dir_all(&app_dir).unwrap();
    fs::create_dir_all(&dep_dir).unwrap();
    fs::write(
        app_dir.join("gewy.pkg"),
        format!(
            "name=dep_escape_guard\nversion=0.1.0\nentry=main.gewy\ndep.std={}\n",
            dep_dir.to_string_lossy()
        ),
    )
    .unwrap();
    fs::write(
        app_dir.join("main.gewy"),
        r#"
template(:dep_escape_guard)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> include("std:../outside.gewy")
"#,
    )
    .unwrap();
    fs::write(
        root.join("outside.gewy"),
        "|> fragment(:udp_packet_meta_fragment)\n",
    )
    .unwrap();

    let err = compile_file(app_dir.to_str().unwrap()).unwrap_err();
    assert!(
        format!("{err:?}").contains("escapes package root"),
        "unexpected error: {err:?}"
    );
}

#[test]
fn dsl_rejects_pipeline_include_cycles() {
    let package_dir =
        std::env::temp_dir().join(format!("gewy-package-{}-include-cycle", std::process::id()));
    fs::create_dir_all(&package_dir).unwrap();
    fs::write(
        package_dir.join("gewy.pkg"),
        "name=include_cycle\nversion=0.1.0\nentry=main.gewy\n",
    )
    .unwrap();
    fs::write(
        package_dir.join("main.gewy"),
        r#"
template(:include_cycle)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> include("./module.gewy")
"#,
    )
    .unwrap();
    fs::write(
        package_dir.join("module.gewy"),
        r#"
|> include("./main.gewy")
"#,
    )
    .unwrap();

    let err = compile_file(package_dir.to_str().unwrap()).unwrap_err();
    assert!(
        format!("{err:?}").contains("include cycle detected"),
        "unexpected error: {err:?}"
    );
}

#[test]
fn dsl_rejects_pipeline_use_cycles() {
    let err = compile_str(
        r#"
fn alpha() {
  |> use(:beta)
}

fn beta() {
  |> use(:alpha)
}

template(:use_cycle)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> use(:alpha)
"#,
    )
    .unwrap_err();
    assert!(
        format!("{err:?}").contains("use cycle detected"),
        "unexpected error: {err:?}"
    );
}

#[test]
fn dsl_rejects_duplicate_pipeline_function_declarations() {
    let err = compile_str(
        r#"
fn udp_core() =
  |> fragment(:udp_packet_meta_fragment)

fn udp_core() =
  |> fragment(:route_meta_fragment)

template(:duplicate_function)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> use(:udp_core)
"#,
    )
    .unwrap_err();
    assert!(
        format!("{err:?}").contains("duplicate pipeline function 'udp_core'"),
        "unexpected error: {err:?}"
    );
}

#[test]
fn dsl_rejects_duplicate_pipeline_function_parameters() {
    let err = compile_str(
        r#"
fn udp_core(model: atom, model: atom) =
  |> program_model($model)

template(:duplicate_parameter)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> use(:udp_core, :udp_model, :other_model)
"#,
    )
    .unwrap_err();
    assert!(
        format!("{err:?}").contains("duplicate pipeline parameter 'model'"),
        "unexpected error: {err:?}"
    );
}

#[test]
fn dsl_can_fall_back_to_default_program_model_from_reason_profile() {
    let binding = compile_str(
        r#"
template(:udp_minimal)
|> window(duration_ms: 5000, lateness_ms: 200)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> fragment(:route_meta_fragment)
"#,
    )
    .unwrap();

    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::DatagramExchange
    );
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().id,
        "datagram_exchange_v1"
    );
}

#[test]
fn dsl_supports_declarative_reason_rules_and_replay_preserves_them() {
    let binding = compile_str(
        r#"
template(:udp_reason_inline)
|> window(duration_ms: 5000, lateness_ms: 200)
|> fragment(:udp_packet_meta_fragment)
|> fragment(:route_meta_fragment)
|> fragment(:sock_lineage_fragment)
|> operation(:datagram_exchange)
|> program_rule(predicate: :process_bound, stage: :process_bound, narrative: :process_bound, dedupe: true)
|> reason_rule(predicate: :process_bound, key_event: :process_identified, narrative: :process_bound, dedupe: true)
|> reason_rule(predicate: "datagram_observed:udp", key_event: :udp_datagram_seen, narrative: :udp_datagram_observed, dedupe: true)
|> reason_rule(predicate: :route_resolved, key_event: :route_changed, narrative: :route_changed, dedupe: true)
"#,
    )
    .unwrap();

    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
    assert_eq!(
        binding.template.reason_profile.as_ref().unwrap().id(),
        "udp_reason_inline_reason_model"
    );

    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 204, 4444, "dig"));
    session.ingest(udp_packet_fact(2, 204, 96));
    session.ingest(route_fact(3, 204, 8));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(40));

    let export = session.export_bundle();
    assert_eq!(export.reason_profile.id(), "udp_reason_inline_reason_model");
    assert_eq!(export.reasons[0].l1.key_events.len(), 3);
    assert_eq!(
        export.reasons[0].l1.key_events[0].kind,
        KeyEventKind::ProcessIdentified
    );
    assert_eq!(
        export.reasons[0].l1.key_events[1].kind,
        KeyEventKind::UdpDatagramSeen
    );
    assert_eq!(
        export.reasons[0].l1.key_events[2].kind,
        KeyEventKind::RouteChanged
    );

    let replay = gewyvern::export::ExportBundle::from_json(&export.to_json())
        .unwrap()
        .replay()
        .unwrap();
    assert_eq!(export.reason_profile, replay.reason_profile);
    assert_eq!(export.reasons, replay.reasons);
}

#[test]
fn dsl_program_rules_can_use_shared_narrative_templates() {
    let binding = compile_str(
        r#"
template(:udp_shared_ir)
|> window(duration_ms: 5000, lateness_ms: 200)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> fragment(:route_meta_fragment)
|> fragment(:sock_lineage_fragment)
|> operation(:datagram_exchange)
|> program_rule(predicate: :process_bound, stage: :process_bound, narrative: :process_bound, dedupe: true)
|> program_rule(predicate: "datagram_observed:udp", stage: :datagram_observed, narrative: :udp_datagram_observed, dedupe: true)
|> program_rule(predicate: :route_resolved, stage: :route_resolved, narrative: :route_changed, dedupe: true)
"#,
    )
    .unwrap();

    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 205, 5353, "dig"));
    session.ingest(udp_packet_fact(2, 205, 88));
    session.ingest(route_fact(3, 205, 9));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(40));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .narrative
            .iter()
            .any(|line| line == "process dig (pid=5353) bound this network flow")
    );
    assert!(
        export.program_flows[0]
            .narrative
            .iter()
            .any(|line| line == "program emitted or received a UDP datagram")
    );
    assert!(
        export.program_flows[0]
            .narrative
            .iter()
            .any(|line| line == "program resolved a route for this network flow")
    );
}

#[test]
fn dsl_reason_rules_can_use_shared_signal_ids() {
    let binding = compile_str(
        r#"
template(:udp_shared_signal_reason)
|> window(duration_ms: 5000, lateness_ms: 200)
|> fragment(:udp_packet_meta_fragment)
|> fragment(:route_meta_fragment)
|> fragment(:sock_lineage_fragment)
|> operation(:datagram_exchange)
|> program_rule(predicate: :process_bound, stage: :process_bound, narrative: :process_bound, dedupe: true)
|> reason_rule(predicate: :process_bound, key_event: :process_bound, narrative: :process_bound, dedupe: true)
|> reason_rule(predicate: "datagram_observed:udp", key_event: :datagram_observed, narrative: :udp_datagram_observed, dedupe: true)
|> reason_rule(predicate: :route_resolved, key_event: :route_resolved, narrative: :route_changed, dedupe: true)
"#,
    )
    .unwrap();

    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 206, 5354, "dig"));
    session.ingest(udp_packet_fact(2, 206, 88));
    session.ingest(route_fact(3, 206, 9));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(40));

    let export = session.export_bundle();
    assert_eq!(
        export.reasons[0].l1.key_events[0].kind,
        KeyEventKind::ProcessIdentified
    );
    assert_eq!(
        export.reasons[0].l1.key_events[1].kind,
        KeyEventKind::UdpDatagramSeen
    );
    assert_eq!(
        export.reasons[0].l1.key_events[2].kind,
        KeyEventKind::RouteChanged
    );
}

#[test]
fn dsl_rejects_program_rules_when_fragment_set_cannot_supply_evidence() {
    let err = compile_str(
        r#"
template(:route_only_invalid)
|> window(duration_ms: 5000, lateness_ms: 200)
|> reason(:udp_datagram_l1)
|> fragment(:route_meta_fragment)
|> operation(:datagram_exchange)
|> program_rule(predicate: :process_bound, stage: :process_bound, narrative: :process_bound, dedupe: true)
"#,
    )
    .unwrap_err();

    assert_eq!(
        err,
        DslError::Registry(RegistryError::MissingRuleEvidence {
            model: "program_model".into(),
            rule_index: 0,
            missing: vec![gewyvern::ledger::FactKindTag::SockLineage],
        })
    );
}

#[test]
fn binding_diagnostics_report_rule_support_and_supporting_fragments() {
    let binding = compile_file(&dsl_fixture_path("udp_process_debug.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let session = RuntimeSession::start(config).unwrap();
    let export = session.export_bundle();

    let diagnostics = export.binding_diagnostics.program_model.as_ref().unwrap();
    assert_eq!(diagnostics.model, "udp_process_debug_dsl_model");
    assert_eq!(diagnostics.rules.len(), 3);
    assert!(diagnostics.rules.iter().all(|rule| rule.supported));
    assert_eq!(diagnostics.rules[0].tier, RuleTier::OptionalEnhancement);
    assert_eq!(diagnostics.rules[1].tier, RuleTier::CoreRequirement);
    assert_eq!(diagnostics.rules[2].tier, RuleTier::CoreRequirement);
    assert_eq!(
        diagnostics.rules[0].required_facts,
        vec![gewyvern::ledger::FactKindTag::SockLineage]
    );
    assert_eq!(
        diagnostics.rules[0].supporting_fragments,
        vec!["sock_lineage_fragment".to_string()]
    );
    assert_eq!(
        diagnostics.rules[1].supporting_fragments,
        vec!["udp_packet_meta_fragment".to_string()]
    );
    assert_eq!(
        diagnostics.rules[2].supporting_fragments,
        vec!["route_meta_fragment".to_string()]
    );

    let replay = gewyvern::export::ExportBundle::from_json(&export.to_json())
        .unwrap()
        .replay()
        .unwrap();
    assert_eq!(export.binding_diagnostics, replay.binding_diagnostics);
}

#[test]
fn binding_diagnostics_reports_unsupported_payload_offsets() {
    let binding = parse_str_unvalidated(
        r#"
template(:unsupported_payload_offset)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> program_model(:unsupported_payload_offset_model)
|> operation(:snmp_get)
|> program_rule(predicate: "datagram_observed:udp:remote:snmp:byte_at:8:0xff:0xa0", stage: :datagram_observed, narrative: :udp_datagram_sent, dedupe: true)
"#,
    )
    .unwrap();

    let diagnostics = collect_binding_diagnostics(&binding).unwrap();
    let rule = &diagnostics.program_model.as_ref().unwrap().rules[0];
    assert!(!rule.supported);
    assert_eq!(rule.tier, RuleTier::Unsupported);
    assert_eq!(
        rule.missing_facts,
        Vec::<gewyvern::ledger::FactKindTag>::new()
    );
    assert_eq!(rule.unsupported_payload_offsets, vec![8]);
}

#[test]
fn binding_diagnostics_reports_expanded_sequence_offsets() {
    let binding = parse_str_unvalidated(
        r#"
template(:unsupported_payload_sequence)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> program_model(:unsupported_payload_sequence_model)
|> operation(:snmp_get)
|> program_rule(predicate: "datagram_observed:udp:remote:snmp:bytes_at:8:0x30,0x82,0x01,0x00", stage: :datagram_observed, narrative: :udp_datagram_sent, dedupe: true)
"#,
    )
    .unwrap();

    let diagnostics = collect_binding_diagnostics(&binding).unwrap();
    let rule = &diagnostics.program_model.as_ref().unwrap().rules[0];
    assert!(!rule.supported);
    assert_eq!(rule.unsupported_payload_offsets, vec![8, 11]);
}
