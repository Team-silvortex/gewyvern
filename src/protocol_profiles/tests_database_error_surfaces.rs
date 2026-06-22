use super::protocol_surface;

#[test]
fn mysql_and_postgres_error_surfaces_expose_machine_readable_entry_semantics() {
    let mysql = protocol_surface("mysql", "error").expect("mysql error should exist");
    let mysql_semantics = mysql
        .entry_semantics
        .expect("mysql error should expose semantics");
    assert_eq!(mysql_semantics.category, "failure-path");
    assert_eq!(
        mysql_semantics.operator_focus,
        "database error response during MySQL query result handling"
    );
    assert_eq!(mysql_semantics.typical_signal.as_deref(), Some("ERR"));
    assert_eq!(
        mysql_semantics.primary_failure_mode.as_deref(),
        Some("semantic_error")
    );
    assert_eq!(
        mysql_semantics.primary_failure_detail.as_deref(),
        Some("protocol_error")
    );
    assert_eq!(
        mysql_semantics.primary_failure_basis.as_deref(),
        Some("direct_protocol_signal")
    );

    let postgres = protocol_surface("postgres", "error").expect("postgres error should exist");
    let postgres_semantics = postgres
        .entry_semantics
        .expect("postgres error should expose semantics");
    assert_eq!(postgres_semantics.category, "failure-path");
    assert_eq!(
        postgres_semantics.operator_focus,
        "database error frame during PostgreSQL query result handling"
    );
    assert_eq!(
        postgres_semantics.typical_signal.as_deref(),
        Some("ErrorResponse")
    );
    assert_eq!(
        postgres_semantics.primary_failure_mode.as_deref(),
        Some("semantic_error")
    );
    assert_eq!(
        postgres_semantics.primary_failure_detail.as_deref(),
        Some("protocol_error")
    );
    assert_eq!(
        postgres_semantics.primary_failure_basis.as_deref(),
        Some("direct_protocol_signal")
    );
}
