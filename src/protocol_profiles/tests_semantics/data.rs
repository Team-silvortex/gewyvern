use super::super::protocol_surface;

#[test]
fn redis_failure_surfaces_expose_machine_readable_entry_semantics() {
    let moved = protocol_surface("redis", "moved").expect("redis moved surface should exist");
    let moved_semantics = moved
        .entry_semantics
        .expect("redis moved should expose semantics");
    assert_eq!(moved_semantics.category, "failure-path");
    assert_eq!(
        moved_semantics.operator_focus,
        "cluster slot redirect that requires target remap"
    );
    assert_eq!(moved_semantics.typical_signal.as_deref(), Some("-MOVED"));
    assert_eq!(
        moved_semantics.primary_failure_mode.as_deref(),
        Some("semantic_error")
    );
    assert_eq!(
        moved_semantics.primary_failure_detail.as_deref(),
        Some("protocol_error")
    );
    assert_eq!(
        moved_semantics.primary_failure_basis.as_deref(),
        Some("direct_protocol_signal")
    );

    let readonly =
        protocol_surface("redis", "readonly").expect("redis readonly surface should exist");
    let readonly_semantics = readonly
        .entry_semantics
        .expect("redis readonly should expose semantics");
    assert_eq!(
        readonly_semantics.operator_focus,
        "replica write refusal or readonly placement mismatch"
    );
    assert_eq!(
        readonly_semantics.primary_failure_mode.as_deref(),
        Some("server_denied")
    );
    assert_eq!(
        readonly_semantics.primary_failure_detail.as_deref(),
        Some("access_denied")
    );

    let zadd = protocol_surface("redis", "zadd").expect("redis zadd surface should exist");
    let zadd_semantics = zadd
        .entry_semantics
        .expect("redis zadd should expose sorted-set write semantics");
    assert_eq!(zadd_semantics.category, "redis-sorted-set-write-path");
    assert_eq!(
        zadd_semantics.operator_focus,
        "Redis ZADD updating sorted-set scores and returning changed member count"
    );

    let xadd = protocol_surface("redis", "xadd").expect("redis xadd surface should exist");
    let xadd_semantics = xadd
        .entry_semantics
        .expect("redis xadd should expose stream append semantics");
    assert_eq!(xadd_semantics.category, "redis-stream-append-path");
}

#[test]
fn database_positive_surfaces_expose_machine_readable_entry_semantics() {
    let mysql_query = protocol_surface("mysql", "query").expect("mysql query should exist");
    let mysql_query_semantics = mysql_query
        .entry_semantics
        .expect("mysql query should expose positive query semantics");
    assert_eq!(mysql_query_semantics.category, "mysql-query-path");
    assert_eq!(
        mysql_query_semantics.typical_signal.as_deref(),
        Some("COM_QUERY 0x03 + OK/result")
    );

    let postgres_query =
        protocol_surface("postgres", "query").expect("postgres query should exist");
    let postgres_query_semantics = postgres_query
        .entry_semantics
        .expect("postgres query should expose positive query semantics");
    assert_eq!(postgres_query_semantics.category, "postgres-query-path");
    assert_eq!(
        postgres_query_semantics.typical_signal.as_deref(),
        Some("Query message 'Q' + ReadyForQuery 'Z'")
    );

    let postgres_auth = protocol_surface("postgres", "auth").expect("postgres auth should exist");
    let postgres_auth_semantics = postgres_auth
        .entry_semantics
        .expect("postgres auth should expose accepted auth semantics");
    assert_eq!(postgres_auth_semantics.category, "postgres-auth-path");
}
