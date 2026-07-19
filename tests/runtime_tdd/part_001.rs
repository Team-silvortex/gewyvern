use super::*;

#[test]
fn handshake_template_exports_attach_plan_and_replays() {
    let export = run_handshake_session(vec![
        tcp_state_fact(1, 10, 1, 2),
        packet_fact(2, 10, 0x02),
        route_fact(3, 10, 2),
    ]);

    let json = export.to_json();
    assert!(json.contains("\"fragment_inventory\""));
    assert!(json.contains("\"attach_plan\""));

    let replay = ExportBundle::from_json(&json).unwrap().replay().unwrap();
    assert_eq!(export.reasons, replay.reasons);
    assert_eq!(export.attach_report, replay.attach_report);
}

#[test]
fn syn_ack_missing_still_produces_deterministic_l1() {
    let export = run_handshake_session(vec![
        tcp_state_fact(1, 20, 1, 2),
        packet_fact(2, 20, 0x02),
        packet_fact(3, 20, 0x02),
    ]);

    assert_eq!(export.flows.len(), 1);
    assert_eq!(export.reasons.len(), 1);
    assert_eq!(export.reasons[0].flow.0, export.flows[0].id.0);

    let replay = ExportBundle::from_json(&export.to_json())
        .unwrap()
        .replay()
        .unwrap();
    assert_eq!(export.reasons, replay.reasons);
}

#[test]
fn route_fingerprint_change_rotates_into_new_flow() {
    let facts = vec![
        tcp_state_fact(1, 30, 1, 2),
        route_fact(2, 30, 2),
        packet_fact(3, 30, 0x02),
        route_fact(4, 30, 7),
        packet_fact(5, 30, 0x10),
    ];

    let flows = build_flow_snapshots(&facts);
    assert_eq!(flows.len(), 2);
    assert_eq!(flows[0].path.current_oif, Some(2));
    assert_eq!(flows[1].path.current_oif, Some(7));
}

#[test]
fn freeze_excludes_facts_beyond_lateness_cutoff() {
    let config = SessionConfig::for_template(handshake_debug_template()).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    let base = SystemTime::UNIX_EPOCH + Duration::from_secs(100);

    let mut state = tcp_state_fact(1, 40, 1, 2);
    state.ts = base + Duration::from_millis(900);
    session.ingest(state);

    let mut packet = packet_fact(2, 40, 0x02);
    packet.ts = base + Duration::from_millis(950);
    session.ingest(packet);

    session.freeze(base + Duration::from_secs(1));

    let mut late = route_fact(3, 40, 9);
    late.ts = base + Duration::from_millis(1_201);
    session.ingest(late);

    let export = session.export_bundle();
    assert_eq!(export.facts.len(), 2);
    assert!(
        export
            .facts
            .iter()
            .all(|fact| fact.fragment_id != "route_meta_fragment")
    );
    assert!(
        export
            .flows
            .iter()
            .all(|flow| flow.path.current_oif.is_none())
    );
    assert_eq!(export.rejected_facts.len(), 1);
    assert_eq!(export.rejected_facts[0].id.0, 3);
    assert_eq!(
        export.rejected_facts[0].reason,
        RejectedFactReason::AfterLatenessCutoff
    );
    assert_eq!(export.rejected_fact_summary.len(), 1);
    assert_eq!(
        export.rejected_fact_summary[0].reason,
        "after_lateness_cutoff"
    );
    assert_eq!(export.debug_summary.accepted_facts, 2);
    assert_eq!(export.debug_summary.rejected_facts, 1);
    assert!(export.debug_summary.degraded);

    let replay = ExportBundle::from_json(&export.to_json())
        .unwrap()
        .replay()
        .unwrap();
    assert_eq!(export.facts, replay.facts);
    assert_eq!(export.rejected_facts, replay.rejected_facts);
    assert_eq!(export.rejected_fact_summary, replay.rejected_fact_summary);
    assert_eq!(export.reasons, replay.reasons);
}

#[test]
fn freeze_materializes_only_the_active_window_plus_lateness() {
    let config = SessionConfig::for_template(handshake_debug_template()).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    let end = SystemTime::UNIX_EPOCH + Duration::from_secs(100);

    let mut too_old = tcp_state_fact(1, 50, 1, 2);
    too_old.ts = end - Duration::from_millis(5_001);
    session.ingest(too_old);

    let mut in_window = packet_fact(2, 50, 0x02);
    in_window.ts = end - Duration::from_millis(50);
    session.ingest(in_window);

    session.freeze(end);

    let mut late_but_allowed = route_fact(3, 50, 4);
    late_but_allowed.ts = end + Duration::from_millis(150);
    session.ingest(late_but_allowed);

    let mut too_late = route_fact(4, 50, 9);
    too_late.ts = end + Duration::from_millis(250);
    session.ingest(too_late);

    let export = session.export_bundle();
    let exported_ids = export
        .facts
        .iter()
        .map(|fact| fact.id.0)
        .collect::<Vec<_>>();

    assert_eq!(exported_ids, vec![2, 3]);
    assert_eq!(export.flows.len(), 1);
    assert_eq!(export.flows[0].evidence.tcp_state_facts.len(), 0);
    assert_eq!(export.flows[0].evidence.packet_facts.len(), 1);
    assert_eq!(export.flows[0].path.current_oif, Some(4));
    assert_eq!(
        export
            .rejected_facts
            .iter()
            .map(|fact| (fact.id.0, fact.reason.clone()))
            .collect::<Vec<_>>(),
        vec![
            (1, RejectedFactReason::BeforeWindowStart),
            (4, RejectedFactReason::AfterLatenessCutoff),
        ]
    );
    assert_eq!(export.debug_summary.accepted_facts, 2);
    assert_eq!(export.debug_summary.rejected_facts, 2);
    assert!(export.debug_summary.degraded);

    let replay = ExportBundle::from_json(&export.to_json())
        .unwrap()
        .replay()
        .unwrap();
    assert_eq!(export.facts, replay.facts);
    assert_eq!(export.rejected_facts, replay.rejected_facts);
    assert_eq!(export.rejected_fact_summary, replay.rejected_fact_summary);
    assert_eq!(export.reasons, replay.reasons);
}

#[test]
fn session_start_can_materialize_attach_failures_into_export() {
    let mut config = SessionConfig::for_template(handshake_debug_template()).unwrap();
    config.attach_failures = vec![AttachFailure {
        fragment_id: "route_meta_fragment".into(),
        hookpoint: HookPoint::KProbe("ip_route_output_flow".into()),
        error: "mock attach failure".into(),
    }];

    let session = RuntimeSession::start(config).unwrap();
    let export = session.export_bundle();

    assert_eq!(
        export.attach_report.hookpoints_failed,
        vec!["route_meta_fragment@kprobe:ip_route_output_flow".to_string()]
    );
    assert_eq!(export.attach_report.hookpoints_attached.len(), 2);
    assert_eq!(export.attach_report.fragments_loaded.len(), 2);
    assert!(
        !export
            .attach_report
            .fragments_loaded
            .contains(&"route_meta_fragment".to_string())
    );

    let replay = ExportBundle::from_json(&export.to_json())
        .unwrap()
        .replay()
        .unwrap();
    assert_eq!(replay.attach_report.hookpoints_failed, Vec::<String>::new());
}

#[test]
fn session_start_with_loader_materializes_structured_failures() {
    let config = SessionConfig::for_template(handshake_debug_template()).unwrap();
    let loader = StaticFailureLoader {
        failures: vec![AttachFailure {
            fragment_id: "route_meta_fragment".into(),
            hookpoint: HookPoint::KProbe("ip_route_output_flow".into()),
            error: "mock loader failure".into(),
        }],
    };

    let session = RuntimeSession::start_with_loader(config, &loader).unwrap();
    let export = session.export_bundle();

    assert_eq!(
        export.attach_report.hookpoints_failed,
        vec!["route_meta_fragment@kprobe:ip_route_output_flow".to_string()]
    );
    assert_eq!(export.attach_report.fragments_loaded.len(), 2);
    assert_eq!(export.attach_failure_summary.len(), 1);
    assert_eq!(export.attach_failure_summary[0].hookpoint_kind, "kprobe");
    assert_eq!(export.attach_failure_summary[0].count, 1);
}

#[test]
fn session_rejects_facts_from_fragments_that_failed_to_attach() {
    let config = SessionConfig::for_template(handshake_debug_template()).unwrap();
    let loader = StaticFailureLoader {
        failures: vec![AttachFailure {
            fragment_id: "route_meta_fragment".into(),
            hookpoint: HookPoint::KProbe("ip_route_output_flow".into()),
            error: "mock loader failure".into(),
        }],
    };

    let mut session = RuntimeSession::start_with_loader(config, &loader).unwrap();
    session.ingest(tcp_state_fact(1, 60, 1, 2));
    session.ingest(route_fact(2, 60, 7));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(20));

    let export = session.export_bundle();
    assert_eq!(export.facts.len(), 1);
    assert_eq!(export.facts[0].fragment_id, "tcp_state_fragment");
    assert_eq!(export.rejected_facts.len(), 1);
    assert_eq!(export.rejected_facts[0].fragment_id, "route_meta_fragment");
    assert_eq!(export.rejected_fact_summary.len(), 1);
    assert_eq!(
        export.rejected_fact_summary[0].fragment_id,
        "route_meta_fragment"
    );
    assert_eq!(
        export.rejected_fact_summary[0].reason,
        "fragment_not_loaded"
    );
    assert_eq!(export.rejected_fact_summary[0].count, 1);
    assert_eq!(export.debug_summary.fragments_loaded, 2);
    assert_eq!(export.debug_summary.hookpoints_failed, 1);
    assert_eq!(export.debug_summary.accepted_facts, 1);
    assert_eq!(export.debug_summary.rejected_facts, 1);
    assert_eq!(export.debug_summary.flows, 1);
    assert_eq!(export.debug_summary.program_findings, 1);
    assert_eq!(export.debug_summary.module_findings, 1);
    assert_eq!(export.debug_summary.reasons, 1);
    assert!(export.debug_summary.degraded);
    assert!(
        export
            .facts
            .iter()
            .all(|fact| fact.fragment_id != "route_meta_fragment")
    );

    let replay = ExportBundle::from_json(&export.to_json())
        .unwrap()
        .replay()
        .unwrap();
    assert_eq!(export.rejected_facts, replay.rejected_facts);
    assert_eq!(export.rejected_fact_summary, replay.rejected_fact_summary);
    assert_eq!(export.debug_summary, replay.debug_summary);
}

#[test]
fn rejected_fact_summary_groups_multiple_drops_by_fragment_and_reason() {
    let config = SessionConfig::for_template(handshake_debug_template()).unwrap();
    let loader = StaticFailureLoader {
        failures: vec![
            AttachFailure {
                fragment_id: "route_meta_fragment".into(),
                hookpoint: HookPoint::KProbe("ip_route_output_flow".into()),
                error: "mock loader failure".into(),
            },
            AttachFailure {
                fragment_id: "tcp_packet_meta_fragment".into(),
                hookpoint: HookPoint::TCIngress,
                error: "mock loader failure".into(),
            },
        ],
    };

    let mut session = RuntimeSession::start_with_loader(config, &loader).unwrap();
    session.ingest(route_fact(1, 70, 7));
    session.ingest(route_fact(2, 70, 9));
    session.ingest(packet_fact(3, 70, 0x12));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(20));

    let export = session.export_bundle();
    assert_eq!(export.rejected_facts.len(), 3);
    assert_eq!(export.rejected_fact_summary.len(), 2);
    assert_eq!(
        export.rejected_fact_summary[0].fragment_id,
        "route_meta_fragment"
    );
    assert_eq!(
        export.rejected_fact_summary[0].reason,
        "fragment_not_loaded"
    );
    assert_eq!(export.rejected_fact_summary[0].count, 2);
    assert_eq!(
        export.rejected_fact_summary[1].fragment_id,
        "tcp_packet_meta_fragment"
    );
    assert_eq!(
        export.rejected_fact_summary[1].reason,
        "fragment_not_loaded"
    );
    assert_eq!(export.rejected_fact_summary[1].count, 1);

    let replay = ExportBundle::from_json(&export.to_json())
        .unwrap()
        .replay()
        .unwrap();
    assert_eq!(export.rejected_fact_summary, replay.rejected_fact_summary);
}

#[test]
fn attach_failure_summary_groups_failures_by_hookpoint_kind() {
    let config = SessionConfig::for_template(handshake_debug_template()).unwrap();
    let loader = StaticFailureLoader {
        failures: vec![
            AttachFailure {
                fragment_id: "route_meta_fragment".into(),
                hookpoint: HookPoint::KProbe("ip_route_output_flow".into()),
                error: "mock loader failure".into(),
            },
            AttachFailure {
                fragment_id: "tcp_packet_meta_fragment".into(),
                hookpoint: HookPoint::TCIngress,
                error: "mock loader failure".into(),
            },
            AttachFailure {
                fragment_id: "linux_tracepoint_smoke_fragment".into(),
                hookpoint: HookPoint::TracePoint("syscalls/definitely_missing_smoke_event".into()),
                error: "mock loader failure".into(),
            },
        ],
    };

    let session = RuntimeSession::start_with_loader(config, &loader).unwrap();
    let export = session.export_bundle();

    assert_eq!(export.attach_report.hookpoints_failed.len(), 3);
    assert_eq!(export.attach_failure_summary.len(), 3);
    assert_eq!(export.attach_failure_summary[0].hookpoint_kind, "kprobe");
    assert_eq!(export.attach_failure_summary[0].count, 1);
    assert_eq!(
        export.attach_failure_summary[1].hookpoint_kind,
        "tc_ingress"
    );
    assert_eq!(export.attach_failure_summary[1].count, 1);
    assert_eq!(
        export.attach_failure_summary[2].hookpoint_kind,
        "tracepoint"
    );
    assert_eq!(export.attach_failure_summary[2].count, 1);

    let replay = ExportBundle::from_json(&export.to_json())
        .unwrap()
        .replay()
        .unwrap();
    assert_eq!(export.attach_failure_summary, replay.attach_failure_summary);
}

#[test]
fn debug_summary_stays_clean_when_session_has_no_loader_or_ingest_degradation() {
    let export = run_handshake_session(vec![
        tcp_state_fact(1, 80, 1, 2),
        packet_fact(2, 80, 0x02),
        route_fact(3, 80, 2),
    ]);

    assert_eq!(export.debug_summary.fragments_loaded, 3);
    assert_eq!(export.debug_summary.hookpoints_failed, 0);
    assert_eq!(export.debug_summary.accepted_facts, 3);
    assert_eq!(export.debug_summary.rejected_facts, 0);
    assert_eq!(export.debug_summary.flows, 1);
    assert_eq!(export.debug_summary.program_flows, 1);
    assert_eq!(export.debug_summary.program_findings, 0);
    assert_eq!(export.debug_summary.module_findings, 0);
    assert_eq!(export.debug_summary.reasons, 1);
    assert!(!export.debug_summary.degraded);

    let replay = ExportBundle::from_json(&export.to_json())
        .unwrap()
        .replay()
        .unwrap();
    assert_eq!(export.debug_summary, replay.debug_summary);
}

#[test]
fn udp_template_exports_deterministic_datagram_reason_chain() {
    let export = run_udp_session(vec![udp_packet_fact(1, 90, 72), route_fact(2, 90, 3)]);

    assert_eq!(export.template_id, "udp_debug");
    assert_eq!(export.facts.len(), 2);
    assert_eq!(export.flows.len(), 1);
    assert_eq!(export.reasons.len(), 1);
    assert_eq!(export.reasons[0].l1.tcp_state_timeline.len(), 0);
    assert_eq!(
        export.reasons[0].l1.path_segments,
        vec![gewyvern::ledger::FactId(2)]
    );
    assert_eq!(
        export.reasons[0].l3.narrative[0].text,
        "udp datagram observed"
    );
    assert_eq!(
        export.reasons[0].l3.narrative[1].text,
        "route fingerprint updated"
    );
    assert_eq!(export.debug_summary.fragments_loaded, 2);
    assert_eq!(export.debug_summary.program_findings, 0);
    assert_eq!(export.debug_summary.module_findings, 0);
    assert!(!export.debug_summary.degraded);

    let replay = ExportBundle::from_json(&export.to_json())
        .unwrap()
        .replay()
        .unwrap();
    assert_eq!(export.reasons, replay.reasons);
    assert_eq!(export.debug_summary, replay.debug_summary);
}

#[test]
fn udp_template_starts_without_tcp_state_fragment() {
    let config = SessionConfig::for_template(udp_debug_template()).unwrap();
    let session = RuntimeSession::start(config).unwrap();
    let export = session.export_bundle();

    assert_eq!(
        export.attach_report.fragments_loaded,
        vec![
            "udp_packet_meta_fragment".to_string(),
            "route_meta_fragment".to_string()
        ]
    );
}

#[test]
fn udp_process_template_binds_flow_to_process_identity() {
    let export = run_udp_process_session(vec![
        sock_lineage_fact(1, 91, 4242, "curl"),
        udp_packet_fact(2, 91, 88),
        route_fact(3, 91, 5),
    ]);

    assert_eq!(export.template_id, "udp_process_debug");
    assert_eq!(export.flows.len(), 1);
    assert_eq!(export.program_flows.len(), 1);
    assert_eq!(export.flows[0].process.as_ref().unwrap().pid, 4242);
    assert_eq!(export.flows[0].process.as_ref().unwrap().comm, "curl");
    assert_eq!(
        export.flows[0].evidence.lineage_facts,
        vec![gewyvern::ledger::FactId(1)]
    );
    assert_eq!(
        export.program_flows[0].operation,
        gewyvern::flow::ProgramOperation::DatagramExchange
    );
    assert_eq!(
        export.program_flows[0].transport_flows,
        vec![export.flows[0].id]
    );
    assert!(
        export.program_flows[0]
            .narrative
            .iter()
            .any(|line| line == "process curl (pid=4242) bound this network flow")
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
    assert!(
        export.reasons[0]
            .l3
            .narrative
            .iter()
            .any(|line| line.text == "flow bound to process curl (pid=4242)")
    );
    assert!(export.reasons[0].l1.key_events.iter().any(|event| matches!(
        event.kind,
        gewyvern::reason::KeyEventKind::ProcessIdentified
    )));

    let replay = ExportBundle::from_json(&export.to_json())
        .unwrap()
        .replay()
        .unwrap();
    assert_eq!(export.flows, replay.flows);
    assert_eq!(export.program_flows, replay.program_flows);
    assert_eq!(export.program_findings, replay.program_findings);
    assert_eq!(export.reasons, replay.reasons);
}

#[test]
fn program_flow_operation_comes_from_template_model() {
    let handshake_export = run_handshake_session(vec![
        tcp_state_fact(1, 101, 1, 2),
        packet_fact(2, 101, 0x02),
        route_fact(3, 101, 2),
    ]);
    let udp_export = run_udp_process_session(vec![
        sock_lineage_fact(1, 102, 9001, "dig"),
        udp_packet_fact(2, 102, 64),
        route_fact(3, 102, 5),
    ]);

    assert_eq!(
        handshake_export.program_flows[0].operation,
        gewyvern::flow::ProgramOperation::ConnectFlow
    );
    assert_eq!(
        udp_export.program_flows[0].operation,
        ProgramOperation::DatagramExchange
    );
}
