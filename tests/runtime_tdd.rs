mod support;

use gewyvern::export::ExportBundle;
use gewyvern::fragment::{AttachFailure, HookPoint};
use gewyvern::runtime::{build_flow_snapshots, RuntimeSession, SessionConfig};
use gewyvern::template::handshake_debug_template;
use support::{packet_fact, route_fact, run_handshake_session, tcp_state_fact};
use std::time::{Duration, SystemTime};

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

    let replay = ExportBundle::from_json(&export.to_json()).unwrap().replay().unwrap();
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
    assert!(export
        .facts
        .iter()
        .all(|fact| fact.fragment_id != "route_meta_fragment"));
    assert!(export.flows.iter().all(|flow| flow.path.current_oif.is_none()));

    let replay = ExportBundle::from_json(&export.to_json()).unwrap().replay().unwrap();
    assert_eq!(export.facts, replay.facts);
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
    let exported_ids = export.facts.iter().map(|fact| fact.id.0).collect::<Vec<_>>();

    assert_eq!(exported_ids, vec![2, 3]);
    assert_eq!(export.flows.len(), 1);
    assert_eq!(export.flows[0].evidence.tcp_state_facts.len(), 0);
    assert_eq!(export.flows[0].evidence.packet_facts.len(), 1);
    assert_eq!(export.flows[0].path.current_oif, Some(4));

    let replay = ExportBundle::from_json(&export.to_json()).unwrap().replay().unwrap();
    assert_eq!(export.facts, replay.facts);
    assert_eq!(export.reasons, replay.reasons);
}

#[test]
fn session_start_can_materialize_attach_failures_into_export() {
    let mut config = SessionConfig::for_template(handshake_debug_template()).unwrap();
    config.attach_failures = vec![AttachFailure {
        fragment_id: "route_meta_fragment",
        hookpoint: HookPoint::KProbe("ip_route_output_flow"),
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
    assert!(!export
        .attach_report
        .fragments_loaded
        .contains(&"route_meta_fragment".to_string()));

    let replay = ExportBundle::from_json(&export.to_json()).unwrap().replay().unwrap();
    assert_eq!(replay.attach_report.hookpoints_failed, Vec::<String>::new());
}
