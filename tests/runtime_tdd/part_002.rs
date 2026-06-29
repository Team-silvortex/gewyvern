use super::*;

#[test]
fn program_flow_operation_supports_custom_model_ids() {
    let mut template = udp_process_debug_template();
    template.id = "udp_dns_debug";
    template.program_model = Some(ProgramModel {
        id: "dns_lookup_v1",
        operation: ProgramOperation::Custom("dns_lookup".into()),
        rules: vec![
            ProgramRule {
                predicate: ProgramPredicate::ProcessBound,
                signal: Some(gewyvern::flow::ProgramStageKind::ProcessBound),
                narrative: ProgramNarrative::ProcessBound,
                dedupe: true,
                module: None,
                phase: None,
            },
            ProgramRule {
                predicate: ProgramPredicate::DatagramObserved {
                    l4_proto: 17,
                    dir: None,
                    local_port: None,
                    remote_port: None,
                    min_len: None,
                    first_byte_mask: None,
                    first_byte_value: None,
                    prefix2: None,
                    prefix4: None,
                    byte13_mask: None,
                    byte13_value: None,
                    byte_matches: vec![],
                    byte_sequences: vec![],
                },
                signal: Some(gewyvern::flow::ProgramStageKind::DatagramObserved),
                narrative: ProgramNarrative::Static("program emitted a DNS-style datagram"),
                dedupe: true,
                module: None,
                phase: None,
            },
            ProgramRule {
                predicate: ProgramPredicate::RouteResolved,
                signal: Some(gewyvern::flow::ProgramStageKind::RouteResolved),
                narrative: ProgramNarrative::Static("program resolved an upstream route"),
                dedupe: true,
                module: None,
                phase: None,
            },
        ],
    });

    let config = SessionConfig::for_template(template).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 103, 5353, "dig"));
    session.ingest(udp_packet_fact(2, 103, 72));
    session.ingest(route_fact(3, 103, 6));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(30));

    let export = session.export_bundle();

    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("dns_lookup".into())
    );
    assert!(
        export.program_flows[0]
            .narrative
            .iter()
            .any(|line| line == "program emitted a DNS-style datagram")
    );

    let replay = ExportBundle::from_json(&export.to_json())
        .unwrap()
        .replay()
        .unwrap();
    assert_eq!(export.program_flows, replay.program_flows);
    assert_eq!(export.program_findings, replay.program_findings);
}

#[test]
fn program_model_supports_all_and_any_predicates() {
    let mut template = udp_process_debug_template();
    template.id = "udp_compound_debug";
    template.program_model = Some(ProgramModel {
        id: "compound_rules_v1",
        operation: ProgramOperation::Custom("compound_udp_activity".into()),
        rules: vec![
            ProgramRule {
                predicate: ProgramPredicate::All(vec![
                    ProgramPredicate::ProcessBound,
                    ProgramPredicate::DatagramObserved {
                        l4_proto: 17,
                        dir: None,
                        local_port: None,
                        remote_port: None,
                        min_len: None,
                        first_byte_mask: None,
                        first_byte_value: None,
                        prefix2: None,
                        prefix4: None,
                        byte13_mask: None,
                        byte13_value: None,
                        byte_matches: vec![],
                        byte_sequences: vec![],
                    },
                ]),
                signal: Some(gewyvern::flow::ProgramStageKind::DatagramObserved),
                narrative: ProgramNarrative::Static("process-owned UDP activity observed"),
                dedupe: true,
                module: None,
                phase: None,
            },
            ProgramRule {
                predicate: ProgramPredicate::Any(vec![
                    ProgramPredicate::RouteResolved,
                    ProgramPredicate::SocketStateObserved {
                        local_port: None,
                        remote_port: None,
                        min_new_state: None,
                    },
                ]),
                signal: Some(gewyvern::flow::ProgramStageKind::RouteResolved),
                narrative: ProgramNarrative::Static(
                    "program observed either route or socket progress",
                ),
                dedupe: true,
                module: None,
                phase: None,
            },
        ],
    });

    let config = SessionConfig::for_template(template).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 104, 7000, "agent"));
    session.ingest(udp_packet_fact(2, 104, 90));
    session.ingest(route_fact(3, 104, 8));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(40));

    let export = session.export_bundle();
    let flow = &export.program_flows[0];

    assert_eq!(
        flow.operation,
        ProgramOperation::Custom("compound_udp_activity".into())
    );
    assert!(
        flow.narrative
            .iter()
            .any(|line| line == "process-owned UDP activity observed")
    );
    assert!(
        flow.narrative
            .iter()
            .any(|line| line == "program observed either route or socket progress")
    );
    assert_eq!(
        flow.stages
            .iter()
            .filter(|stage| stage.kind == gewyvern::flow::ProgramStageKind::DatagramObserved)
            .count(),
        1
    );

    let replay = ExportBundle::from_json(&export.to_json())
        .unwrap()
        .replay()
        .unwrap();
    assert_eq!(export.program_flows, replay.program_flows);
}

#[test]
fn udp_process_template_loads_sock_lineage_fragment() {
    let config = SessionConfig::for_template(udp_process_debug_template()).unwrap();
    let session = RuntimeSession::start(config).unwrap();
    let export = session.export_bundle();

    assert!(
        export
            .attach_report
            .fragments_loaded
            .contains(&"sock_lineage_fragment".to_string())
    );
}

#[test]
fn session_config_accepts_template_binding_compile_target() {
    let binding = udp_process_debug_template()
        .bind()
        .with_fragment_param(
            "udp_packet_meta_fragment",
            "min_len",
            FragmentParamValue::U64(64),
        )
        .with_fragment_param(
            "sock_lineage_fragment",
            "capture_comm",
            FragmentParamValue::Bool(true),
        );

    let config = SessionConfig::for_binding(binding).unwrap();
    let session = RuntimeSession::start(config).unwrap();
    let export = session.export_bundle();

    assert_eq!(export.template_id, "udp_process_debug");
    assert!(
        export
            .attach_report
            .fragments_loaded
            .contains(&"udp_packet_meta_fragment".to_string())
    );
}

#[test]
fn capture_comm_fragment_param_redacts_process_name_across_runtime_and_replay() {
    let binding = udp_process_debug_template().bind().with_fragment_param(
        "sock_lineage_fragment",
        "capture_comm",
        FragmentParamValue::Bool(false),
    );

    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 105, 4242, "curl"));
    session.ingest(udp_packet_fact(2, 105, 72));
    session.ingest(route_fact(3, 105, 9));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(40));

    let export = session.export_bundle();

    assert_eq!(export.flows[0].process.as_ref().unwrap().comm, "<redacted>");
    assert_eq!(
        export.fragment_params["sock_lineage_fragment"]["capture_comm"],
        FragmentParamValue::Bool(false)
    );
    assert!(matches!(
        &export.facts[0].kind,
        FactKind::SockLineage(lineage) if lineage.comm == [0; 16]
    ));
    assert!(
        export.program_flows[0]
            .narrative
            .iter()
            .any(|line| line == "process <redacted> (pid=4242) bound this network flow")
    );
    assert!(
        export.reasons[0]
            .l3
            .narrative
            .iter()
            .any(|line| line.text == "flow bound to process <redacted> (pid=4242)")
    );

    let replay = ExportBundle::from_json(&export.to_json())
        .unwrap()
        .replay()
        .unwrap();
    assert_eq!(export.fragment_params, replay.fragment_params);
    assert_eq!(export.flows, replay.flows);
    assert_eq!(export.program_flows, replay.program_flows);
    assert_eq!(export.reasons, replay.reasons);
}

#[test]
fn udp_packet_min_len_fragment_param_filters_small_packets_with_audit_trail() {
    let binding = udp_process_debug_template().bind().with_fragment_param(
        "udp_packet_meta_fragment",
        "min_len",
        FragmentParamValue::U64(80),
    );

    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 106, 4242, "curl"));
    session.ingest(udp_packet_fact(2, 106, 72));
    session.ingest(route_fact(3, 106, 10));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(40));

    let export = session.export_bundle();

    assert_eq!(export.facts.len(), 2);
    assert!(
        export
            .facts
            .iter()
            .all(|fact| fact.id != gewyvern::ledger::FactId(2))
    );
    assert_eq!(export.rejected_facts.len(), 1);
    assert_eq!(export.rejected_facts[0].id, gewyvern::ledger::FactId(2));
    assert_eq!(
        export.rejected_facts[0].reason,
        gewyvern::runtime::RejectedFactReason::FilteredByFragmentParam
    );
    assert_eq!(export.rejected_fact_summary.len(), 1);
    assert_eq!(
        export.rejected_fact_summary[0].reason,
        "filtered_by_fragment_param"
    );
    assert_eq!(export.rejected_fact_summary[0].count, 1);
    assert!(
        export.program_flows[0]
            .narrative
            .iter()
            .all(|line| line != "program emitted or received a UDP datagram")
    );

    let replay = ExportBundle::from_json(&export.to_json())
        .unwrap()
        .replay()
        .unwrap();
    assert_eq!(export.rejected_facts, replay.rejected_facts);
    assert_eq!(export.rejected_fact_summary, replay.rejected_fact_summary);
    assert_eq!(export.program_flows, replay.program_flows);
    assert_eq!(export.program_findings, replay.program_findings);
}

#[test]
fn attach_failures_are_lifted_into_program_findings_for_suspect_module_areas() {
    let config = SessionConfig::for_template(udp_process_debug_template()).unwrap();
    let loader = StaticFailureLoader {
        failures: vec![AttachFailure {
            fragment_id: "route_meta_fragment",
            hookpoint: HookPoint::KProbe("ip_route_output_flow"),
            error: "mock loader failure".into(),
        }],
    };

    let mut session = RuntimeSession::start_with_loader(config, &loader).unwrap();
    session.ingest(sock_lineage_fact(1, 107, 4242, "curl"));
    session.ingest(udp_packet_fact(2, 107, 96));
    session.ingest(route_fact(3, 107, 11));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(40));

    let export = session.export_bundle();

    assert_eq!(export.program_findings.len(), 1);
    assert_eq!(export.debug_summary.program_findings, 1);
    assert_eq!(export.module_findings.len(), 1);
    assert_eq!(export.debug_summary.module_findings, 1);
    assert_eq!(
        export.program_findings[0].module_label,
        "datagram_exchange::route_resolution::route_meta_fragment"
    );
    assert_eq!(export.program_findings[0].suspect_area, "route_resolution");
    assert_eq!(
        export.program_findings[0].cause,
        ProgramFindingCause::AttachFailure
    );
    assert_eq!(
        export.program_findings[0].supporting_fragments,
        vec!["route_meta_fragment".to_string()]
    );
    assert!(
        export.program_findings[0]
            .evidence_trace
            .iter()
            .any(|item| item == "missing_signal:route_resolved")
    );
    assert!(
        export.program_findings[0]
            .evidence_trace
            .iter()
            .any(|item| item == "observed_stage:process_bound@1")
    );
    assert!(
        export.program_findings[0]
            .evidence_trace
            .iter()
            .any(|item| item == "observed_stage:datagram_observed@2")
    );
    assert!(
        export.program_findings[0]
            .evidence_trace
            .iter()
            .any(|item| item == "failed_hookpoint:route_meta_fragment@kprobe:ip_route_output_flow")
    );
    assert!(
        export.program_findings[0]
            .summary
            .contains("process curl (pid=4242)")
    );
    assert_eq!(
        export.module_findings[0].module_label,
        "datagram_exchange::route_resolution::route_meta_fragment"
    );
    assert_eq!(export.module_findings[0].severity, ModuleSeverity::High);
    assert_eq!(
        export.module_findings[0].suspect_areas,
        vec!["route_resolution".to_string()]
    );
    assert_eq!(
        export.module_findings[0].program_flows,
        vec![export.program_findings[0].program_flow]
    );

    let replay = ExportBundle::from_json(&export.to_json())
        .unwrap()
        .replay()
        .unwrap();
    assert_eq!(export.program_findings, replay.program_findings);
    assert_eq!(export.module_findings, replay.module_findings);
}

#[test]
fn rejected_core_packet_evidence_points_to_datagram_io_as_suspect_area() {
    let binding = udp_process_debug_template().bind().with_fragment_param(
        "udp_packet_meta_fragment",
        "min_len",
        FragmentParamValue::U64(80),
    );

    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 108, 4242, "curl"));
    session.ingest(udp_packet_fact(2, 108, 72));
    session.ingest(route_fact(3, 108, 10));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(40));

    let export = session.export_bundle();

    assert_eq!(export.program_findings.len(), 1);
    assert_eq!(export.debug_summary.program_findings, 1);
    assert_eq!(export.module_findings.len(), 1);
    assert_eq!(export.debug_summary.module_findings, 1);
    assert_eq!(
        export.program_findings[0].module_label,
        "datagram_exchange::datagram_io::udp_packet_meta_fragment"
    );
    assert_eq!(export.program_findings[0].suspect_area, "datagram_io");
    assert_eq!(
        export.program_findings[0].cause,
        ProgramFindingCause::RejectedEvidence
    );
    assert_eq!(
        export.program_findings[0].supporting_fragments,
        vec!["udp_packet_meta_fragment".to_string()]
    );
    assert!(
        export.program_findings[0]
            .evidence_trace
            .iter()
            .any(|item| item == "missing_signal:datagram_observed")
    );
    assert!(
        export.program_findings[0]
            .evidence_trace
            .iter()
            .any(|item| item == "observed_stage:process_bound@1")
    );
    assert!(
        export.program_findings[0]
            .evidence_trace
            .iter()
            .any(|item| item == "observed_stage:route_resolved@3")
    );
    assert!(
        export.program_findings[0]
            .evidence_trace
            .iter()
            .any(|item| item
                == "rejected_fact:2:udp_packet_meta_fragment:filtered_by_fragment_param")
    );
    assert_eq!(
        export.module_findings[0].module_label,
        "datagram_exchange::datagram_io::udp_packet_meta_fragment"
    );
    assert_eq!(export.module_findings[0].severity, ModuleSeverity::Medium);
    assert_eq!(
        export.module_findings[0].suspect_areas,
        vec!["datagram_io".to_string()]
    );

    let replay = ExportBundle::from_json(&export.to_json())
        .unwrap()
        .replay()
        .unwrap();
    assert_eq!(export.program_findings, replay.program_findings);
    assert_eq!(export.module_findings, replay.module_findings);
}

#[test]
fn dsl_declared_module_names_override_auto_generated_module_labels() {
    let binding = gewyvern::dsl::compile_str(
        r#"
template(:udp_module_debug)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> fragment(:route_meta_fragment)
|> fragment(:sock_lineage_fragment)
|> operation(:datagram_exchange)
|> program_rule(predicate: :process_bound, stage: :process_bound, narrative: :process_bound, dedupe: true, module: :udp_request_path)
|> program_rule(predicate: "datagram_observed:udp", stage: :datagram_observed, narrative: :udp_datagram_observed, dedupe: true, module: :udp_request_path)
|> program_rule(predicate: :route_resolved, stage: :route_resolved, narrative: :route_changed, dedupe: true, module: :udp_request_path)
|> param(:udp_packet_meta_fragment.min_len, 80)
"#,
    )
    .unwrap();

    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 109, 4242, "curl"));
    session.ingest(udp_packet_fact(2, 109, 72));
    session.ingest(route_fact(3, 109, 10));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(40));

    let export = session.export_bundle();

    assert_eq!(export.program_findings.len(), 1);
    assert_eq!(export.module_findings.len(), 1);
    assert_eq!(export.program_findings[0].module_label, "udp_request_path");
    assert_eq!(export.module_findings[0].module_label, "udp_request_path");
    assert_eq!(export.module_findings[0].severity, ModuleSeverity::Medium);

    let replay = ExportBundle::from_json(&export.to_json())
        .unwrap()
        .replay()
        .unwrap();
    assert_eq!(export.program_findings, replay.program_findings);
    assert_eq!(export.module_findings, replay.module_findings);
}
