use super::*;

#[test]
fn protocol_entry_surface_endpoint_exposes_ldap_write_failure_semantics() {
    let snapshot = ApiSnapshot::default();

    let (denied_status, _, denied_body) =
        api_response_for_request("/v1/protocols/ldap/entries/denied/surface.json", &snapshot);
    assert_eq!(denied_status, 200);
    assert!(denied_body.contains("\"protocol\":\"ldap\""));
    assert!(denied_body.contains("\"entry\":\"denied\""));
    assert!(denied_body.contains("\"entry_semantics\":{"));
    assert!(denied_body.contains("\"category\":\"failure-path\""));
    assert!(denied_body.contains(
        "\"operator_focus\":\"directory write refusal during LDAP modify result evaluation\""
    ));
    assert!(denied_body.contains("\"typical_signal\":\"modifyResponse\""));
    assert!(denied_body.contains("\"primary_failure_mode\":\"server_denied\""));
    assert!(denied_body.contains("\"primary_failure_detail\":\"access_denied\""));
    assert!(denied_body.contains("\"primary_failure_basis\":\"direct_protocol_signal\""));

    let (constraint_status, _, constraint_body) =
        api_response_for_request("/v1/protocols/ldap/entries/constraint/surface.json", &snapshot);
    assert_eq!(constraint_status, 200);
    assert!(constraint_body.contains("\"protocol\":\"ldap\""));
    assert!(constraint_body.contains("\"entry\":\"constraint\""));
    assert!(constraint_body.contains("\"entry_semantics\":{"));
    assert!(constraint_body.contains("\"category\":\"failure-path\""));
    assert!(constraint_body.contains(
        "\"operator_focus\":\"directory constraint violation during LDAP modify result evaluation\""
    ));
    assert!(constraint_body.contains("\"typical_signal\":\"modifyResponse\""));
    assert!(constraint_body.contains("\"primary_failure_mode\":\"semantic_error\""));
    assert!(
        constraint_body.contains("\"primary_failure_detail\":\"protocol_constraint_violation\"")
    );
    assert!(constraint_body.contains("\"primary_failure_basis\":\"direct_protocol_signal\""));
}
