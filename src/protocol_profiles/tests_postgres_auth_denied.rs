use super::summary::built_in_protocol_summary;
use super::{protocol_dsl_path, protocol_surface};

#[test]
fn postgres_auth_denied_surface_shares_connect_auth_shelf() {
    let postgres = protocol_surface("postgres", "auth-denied")
        .expect("postgres auth-denied surface should exist");
    let shelf = postgres
        .shelf
        .expect("postgres auth-denied should have a shelf");
    assert_eq!(shelf.key, "connect-auth");
    assert_eq!(shelf.label, "Connect And Auth");
    assert_eq!(
        shelf.page,
        "docs/book/reference-postgres-connect-surface.md"
    );
    assert!(shelf.entries.contains(&"connect".to_string()));
    assert!(shelf.entries.contains(&"auth".to_string()));
    assert!(shelf.entries.contains(&"auth-denied".to_string()));
}

#[test]
fn postgres_auth_denied_aliases_resolve_to_canonical_entries() {
    assert_eq!(
        protocol_dsl_path("postgres-auth-denied", None),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/postgres/auth-denied".to_string())
    );
    assert_eq!(
        protocol_dsl_path("postgres", Some("login-denied")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/postgres/auth-denied".to_string())
    );
    assert_eq!(
        protocol_dsl_path("postgres", Some("password-denied")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/postgres/auth-denied".to_string())
    );
}

#[test]
fn postgres_auth_denied_surface_exposes_summary_aliases_and_semantics() {
    let summary = built_in_protocol_summary("postgres").expect("postgres summary should exist");
    let auth_denied = summary
        .entries
        .iter()
        .find(|entry| entry.mode == "auth-denied")
        .expect("postgres auth-denied summary should exist");
    assert!(
        auth_denied
            .aliases
            .contains(&"postgres-auth-denied".to_string())
    );
    assert!(auth_denied.aliases.contains(&"login-denied".to_string()));
    assert!(auth_denied.aliases.contains(&"password-denied".to_string()));

    let semantics = protocol_surface("postgres", "auth-denied")
        .expect("postgres auth-denied surface should exist")
        .entry_semantics
        .expect("postgres auth-denied should expose semantics");
    assert_eq!(semantics.category, "failure-path");
    assert_eq!(
        semantics.operator_focus,
        "database authentication rejection after PostgreSQL password exchange"
    );
    assert_eq!(semantics.typical_signal.as_deref(), Some("ErrorResponse"));
    assert_eq!(
        semantics.primary_failure_mode.as_deref(),
        Some("server_denied")
    );
    assert_eq!(
        semantics.primary_failure_detail.as_deref(),
        Some("access_denied")
    );
    assert_eq!(
        semantics.primary_failure_basis.as_deref(),
        Some("direct_protocol_signal")
    );
}
