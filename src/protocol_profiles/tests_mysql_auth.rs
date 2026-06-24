use super::summary::built_in_protocol_summary;
use super::{protocol_dsl_path, protocol_surface};

#[test]
fn mysql_auth_surfaces_share_connect_auth_shelf() {
    let auth = protocol_surface("mysql", "auth").expect("mysql auth surface should exist");
    let shelf = auth.shelf.expect("mysql auth should have a shelf");
    assert_eq!(shelf.key, "connect-auth");
    assert_eq!(shelf.label, "Connect And Auth");
    assert_eq!(shelf.page, "docs/book/reference-mysql-connect-surface.md");
    assert!(shelf.entries.contains(&"connect".to_string()));
    assert!(shelf.entries.contains(&"auth".to_string()));
    assert!(shelf.entries.contains(&"auth-denied".to_string()));
}

#[test]
fn mysql_auth_denied_aliases_resolve_to_canonical_entry() {
    assert_eq!(
        protocol_dsl_path("mysql-auth-denied", None),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/mysql/auth-denied".to_string())
    );
    assert_eq!(
        protocol_dsl_path("mysql", Some("login-denied")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/mysql/auth-denied".to_string())
    );
    assert_eq!(
        protocol_dsl_path("mysql", Some("handshake-denied")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/mysql/auth-denied".to_string())
    );
}

#[test]
fn mysql_auth_denied_surface_exposes_summary_aliases_and_semantics() {
    let summary = built_in_protocol_summary("mysql").expect("mysql summary should exist");
    let auth_denied = summary
        .entries
        .iter()
        .find(|entry| entry.mode == "auth-denied")
        .expect("mysql auth-denied summary should exist");
    assert!(
        auth_denied
            .aliases
            .contains(&"mysql-auth-denied".to_string())
    );
    assert!(auth_denied.aliases.contains(&"login-denied".to_string()));
    assert!(
        auth_denied
            .aliases
            .contains(&"handshake-denied".to_string())
    );

    let semantics = protocol_surface("mysql", "auth-denied")
        .expect("mysql auth-denied surface should exist")
        .entry_semantics
        .expect("mysql auth-denied should expose semantics");
    assert_eq!(semantics.category, "failure-path");
    assert_eq!(
        semantics.operator_focus,
        "database authentication rejection during MySQL handshake response evaluation"
    );
    assert_eq!(semantics.typical_signal.as_deref(), Some("ERR"));
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

    let auth = protocol_surface("mysql", "auth").expect("mysql auth should exist");
    assert!(auth.entry_semantics.is_none());
}
