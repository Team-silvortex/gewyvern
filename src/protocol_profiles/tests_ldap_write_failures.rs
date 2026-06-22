use super::protocol_surface;

#[test]
fn ldap_write_failure_surfaces_expose_machine_readable_entry_semantics() {
    let denied = protocol_surface("ldap", "denied").expect("ldap denied should exist");
    let denied_semantics = denied
        .entry_semantics
        .expect("ldap denied should expose semantics");
    assert_eq!(denied_semantics.category, "failure-path");
    assert_eq!(
        denied_semantics.operator_focus,
        "directory write refusal during LDAP modify result evaluation"
    );
    assert_eq!(
        denied_semantics.typical_signal.as_deref(),
        Some("modifyResponse")
    );
    assert_eq!(
        denied_semantics.primary_failure_mode.as_deref(),
        Some("server_denied")
    );
    assert_eq!(
        denied_semantics.primary_failure_detail.as_deref(),
        Some("access_denied")
    );
    assert_eq!(
        denied_semantics.primary_failure_basis.as_deref(),
        Some("direct_protocol_signal")
    );

    let constraint = protocol_surface("ldap", "constraint").expect("ldap constraint should exist");
    let constraint_semantics = constraint
        .entry_semantics
        .expect("ldap constraint should expose semantics");
    assert_eq!(constraint_semantics.category, "failure-path");
    assert_eq!(
        constraint_semantics.operator_focus,
        "directory constraint violation during LDAP modify result evaluation"
    );
    assert_eq!(
        constraint_semantics.typical_signal.as_deref(),
        Some("modifyResponse")
    );
    assert_eq!(
        constraint_semantics.primary_failure_mode.as_deref(),
        Some("semantic_error")
    );
    assert_eq!(
        constraint_semantics.primary_failure_detail.as_deref(),
        Some("protocol_constraint_violation")
    );
    assert_eq!(
        constraint_semantics.primary_failure_basis.as_deref(),
        Some("direct_protocol_signal")
    );

    let modify = protocol_surface("ldap", "modify").expect("ldap modify should exist");
    assert!(modify.entry_semantics.is_none());
}
