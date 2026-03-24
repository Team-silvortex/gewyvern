mod support;

use gewyvern::export::ExportBundle;
use gewyvern::runtime::build_flow_snapshots;
use support::{packet_fact, route_fact, run_handshake_session, tcp_state_fact};

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
