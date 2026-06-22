use super::*;

#[test]
fn protocol_entry_surface_endpoint_exposes_database_error_surface_semantics() {
    let snapshot = ApiSnapshot::default();

    let (mysql_status, _, mysql_body) =
        api_response_for_request("/v1/protocols/mysql/entries/error/surface.json", &snapshot);
    assert_eq!(mysql_status, 200);
    assert!(mysql_body.contains("\"protocol\":\"mysql\""));
    assert!(mysql_body.contains("\"entry\":\"error\""));
    assert!(mysql_body.contains("\"entry_semantics\":{"));
    assert!(mysql_body.contains("\"category\":\"failure-path\""));
    assert!(mysql_body.contains(
        "\"operator_focus\":\"database error response during MySQL query result handling\""
    ));
    assert!(mysql_body.contains("\"typical_signal\":\"ERR\""));
    assert!(mysql_body.contains("\"primary_failure_mode\":\"semantic_error\""));
    assert!(mysql_body.contains("\"primary_failure_detail\":\"protocol_error\""));
    assert!(mysql_body.contains("\"primary_failure_basis\":\"direct_protocol_signal\""));

    let (postgres_status, _, postgres_body) =
        api_response_for_request("/v1/protocols/postgres/entries/error/surface.json", &snapshot);
    assert_eq!(postgres_status, 200);
    assert!(postgres_body.contains("\"protocol\":\"postgres\""));
    assert!(postgres_body.contains("\"entry\":\"error\""));
    assert!(postgres_body.contains("\"entry_semantics\":{"));
    assert!(postgres_body.contains("\"category\":\"failure-path\""));
    assert!(postgres_body.contains(
        "\"operator_focus\":\"database error frame during PostgreSQL query result handling\""
    ));
    assert!(postgres_body.contains("\"typical_signal\":\"ErrorResponse\""));
    assert!(postgres_body.contains("\"primary_failure_mode\":\"semantic_error\""));
    assert!(postgres_body.contains("\"primary_failure_detail\":\"protocol_error\""));
    assert!(postgres_body.contains("\"primary_failure_basis\":\"direct_protocol_signal\""));
}
