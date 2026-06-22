use super::*;

#[test]
fn protocol_entry_surface_endpoint_exposes_postgres_auth_denied_semantics() {
    let snapshot = ApiSnapshot::default();
    let (status, _, body) = api_response_for_request(
        "/v1/protocols/postgres/entries/auth-denied/surface.json",
        &snapshot,
    );
    assert_eq!(status, 200);
    assert!(body.contains("\"protocol\":\"postgres\""));
    assert!(body.contains("\"entry\":\"auth-denied\""));
    assert!(body.contains("\"entry_semantics\":{"));
    assert!(body.contains("\"category\":\"failure-path\""));
    assert!(body.contains(
        "\"operator_focus\":\"database authentication rejection after PostgreSQL password exchange\""
    ));
    assert!(body.contains("\"typical_signal\":\"ErrorResponse\""));
    assert!(body.contains("\"primary_failure_mode\":\"server_denied\""));
    assert!(body.contains("\"primary_failure_detail\":\"access_denied\""));
    assert!(body.contains("\"primary_failure_basis\":\"direct_protocol_signal\""));
}
