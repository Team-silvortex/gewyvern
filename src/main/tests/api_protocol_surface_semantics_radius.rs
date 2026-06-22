use super::*;

#[test]
fn protocol_entry_surface_endpoint_exposes_radius_denied_semantics() {
    let snapshot = ApiSnapshot::default();
    let (status, _, body) =
        api_response_for_request("/v1/protocols/radius/entries/denied/surface.json", &snapshot);
    assert_eq!(status, 200);
    assert!(body.contains("\"protocol\":\"radius\""));
    assert!(body.contains("\"entry\":\"denied\""));
    assert!(body.contains("\"entry_semantics\":{"));
    assert!(body.contains("\"category\":\"failure-path\""));
    assert!(body.contains(
        "\"operator_focus\":\"identity access rejection during RADIUS Access-Reject evaluation\""
    ));
    assert!(body.contains("\"typical_signal\":\"Access-Reject\""));
    assert!(body.contains("\"primary_failure_mode\":\"server_denied\""));
    assert!(body.contains("\"primary_failure_detail\":\"access_denied\""));
    assert!(body.contains("\"primary_failure_basis\":\"direct_protocol_signal\""));
}

#[test]
fn protocol_entry_surface_endpoint_exposes_radius_challenge_semantics() {
    let snapshot = ApiSnapshot::default();
    let (status, _, body) =
        api_response_for_request("/v1/protocols/radius/entries/challenge/surface.json", &snapshot);
    assert_eq!(status, 200);
    assert!(body.contains("\"protocol\":\"radius\""));
    assert!(body.contains("\"entry\":\"challenge\""));
    assert!(body.contains("\"entry_semantics\":{"));
    assert!(body.contains("\"category\":\"continuation-path\""));
    assert!(body.contains(
        "\"operator_focus\":\"identity challenge continuation during RADIUS Access-Challenge evaluation\""
    ));
    assert!(body.contains("\"typical_signal\":\"Access-Challenge\""));
}
