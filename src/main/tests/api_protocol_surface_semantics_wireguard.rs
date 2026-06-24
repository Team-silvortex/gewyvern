use super::*;

#[test]
fn protocol_entry_surface_endpoint_exposes_wireguard_cookie_semantics() {
    let snapshot = ApiSnapshot::default();
    let (status, _, body) = api_response_for_request(
        "/v1/protocols/wireguard/entries/cookie/surface.json",
        &snapshot,
    );
    assert_eq!(status, 200);
    assert!(body.contains("\"protocol\":\"wireguard\""));
    assert!(body.contains("\"entry\":\"cookie\""));
    assert!(body.contains("\"entry_semantics\":{"));
    assert!(body.contains("\"category\":\"continuation-path\""));
    assert!(body.contains(
        "\"operator_focus\":\"peer anti-abuse continuation during WireGuard cookie reply evaluation\""
    ));
    assert!(body.contains("\"typical_signal\":\"Cookie Reply\""));
}
