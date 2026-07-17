mod support;

use gewyvern::export::ExportBundle;
use support::{packet_fact, route_fact, run_handshake_session, tcp_state_fact};

#[test]
fn export_bundle_keeps_replay_critical_top_level_fields() {
    let export = run_handshake_session(vec![
        tcp_state_fact(1, 10, 1, 2),
        packet_fact(2, 10, 0x02),
        route_fact(3, 10, 2),
    ]);

    let json = export.to_json();
    assert!(json.contains("\"template_id\""));
    assert!(json.contains("\"fragment_inventory\""));
    assert!(json.contains("\"attach_plan\""));
    assert!(json.contains("\"attach_report\""));
    assert!(json.contains("\"binding_diagnostics\""));
    assert!(json.contains("\"window_profile\""));
    assert!(json.contains("\"reason_profile_id\""));
    assert!(json.contains("\"reason_profile\""));
    assert!(json.contains("\"facts\""));
    assert!(json.contains("\"flows\""));
    assert!(json.contains("\"program_flows\""));
    assert!(json.contains("\"reasons\""));
}

#[test]
fn export_bundle_roundtrip_preserves_replay_spine() {
    let export = run_handshake_session(vec![
        tcp_state_fact(1, 20, 1, 2),
        packet_fact(2, 20, 0x02),
        route_fact(3, 20, 7),
    ]);

    let replay = ExportBundle::from_json(&export.to_json())
        .unwrap()
        .replay()
        .unwrap();

    assert_eq!(export.template_id, replay.template_id);
    assert_eq!(export.fragment_inventory, replay.fragment_inventory);
    assert_eq!(export.window_profile, replay.window_profile);
    assert_eq!(export.reason_profile_id, replay.reason_profile_id);
    assert_eq!(export.reason_profile, replay.reason_profile);
    assert_eq!(export.facts, replay.facts);
    assert_eq!(export.flows, replay.flows);
    assert_eq!(export.reasons, replay.reasons);
}

#[test]
fn export_attach_descriptors_accept_runtime_owned_text() {
    let export = run_handshake_session(vec![
        tcp_state_fact(1, 30, 1, 2),
        packet_fact(2, 30, 0x02),
        route_fact(3, 30, 7),
    ]);
    let json = export
        .to_json()
        .replace("tcp_state_fragment", "runtime_custom_state_fragment")
        .replace("sock/inet_sock_set_state", "runtime/custom_tracepoint")
        .replace("sample_payload_offsets", "runtime_sample_offsets")
        .replace("\"events\"", "\"runtime_events\"");

    let decoded = ExportBundle::from_json(&json).expect("dynamic attach text must be owned");
    let state = decoded
        .attach_plan
        .fragments
        .iter()
        .find(|fragment| fragment.id == "runtime_custom_state_fragment")
        .expect("dynamic fragment id must survive decoding");

    assert_eq!(
        state.hookpoints[0].label(),
        "tracepoint:runtime/custom_tracepoint"
    );
    assert_eq!(state.maps[0].name, "runtime_events");
    assert!(decoded.attach_plan.fragments.iter().any(|fragment| {
        fragment
            .params
            .iter()
            .any(|param| param.key == "runtime_sample_offsets")
    }));
}
