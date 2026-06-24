use super::*;

#[test]
fn protocol_entry_surface_endpoint_exposes_quic_retry_semantics() {
    let snapshot = ApiSnapshot::default();
    let (status, _, body) =
        api_response_for_request("/v1/protocols/quic/entries/retry/surface.json", &snapshot);
    assert_eq!(status, 200);
    assert!(body.contains("\"protocol\":\"quic\""));
    assert!(body.contains("\"entry\":\"retry\""));
    assert!(body.contains("\"entry_semantics\":{"));
    assert!(body.contains("\"category\":\"continuation-path\""));
    assert!(body.contains(
        "\"operator_focus\":\"peer address-validation continuation during QUIC Retry evaluation\""
    ));
    assert!(body.contains("\"typical_signal\":\"Retry\""));
}

#[test]
fn protocol_entry_surface_endpoint_exposes_quic_close_semantics() {
    let snapshot = ApiSnapshot::default();
    let (status, _, body) =
        api_response_for_request("/v1/protocols/quic/entries/close/surface.json", &snapshot);
    assert_eq!(status, 200);
    assert!(body.contains("\"protocol\":\"quic\""));
    assert!(body.contains("\"entry\":\"close\""));
    assert!(body.contains("\"entry_semantics\":{"));
    assert!(body.contains("\"category\":\"failure-path\""));
    assert!(body.contains(
        "\"operator_focus\":\"peer transport termination during QUIC connection close evaluation\""
    ));
    assert!(body.contains("\"typical_signal\":\"CONNECTION_CLOSE\""));
    assert!(body.contains("\"primary_failure_mode\":\"peer_closed\""));
    assert!(body.contains("\"primary_failure_detail\":\"transport_terminated\""));
}

#[test]
fn protocol_entry_surface_endpoint_exposes_quic_local_close_semantics() {
    let snapshot = ApiSnapshot::default();
    let (status, _, body) = api_response_for_request(
        "/v1/protocols/quic/entries/local-close/surface.json",
        &snapshot,
    );
    assert_eq!(status, 200);
    assert!(body.contains("\"protocol\":\"quic\""));
    assert!(body.contains("\"entry\":\"local-close\""));
    assert!(body.contains("\"entry_semantics\":{"));
    assert!(body.contains("\"category\":\"failure-path\""));
    assert!(body.contains(
        "\"operator_focus\":\"local transport termination during QUIC connection close evaluation\""
    ));
    assert!(body.contains("\"typical_signal\":\"CONNECTION_CLOSE\""));
    assert!(body.contains("\"primary_failure_mode\":\"local_closed\""));
    assert!(body.contains("\"primary_failure_detail\":\"transport_terminated\""));
}
