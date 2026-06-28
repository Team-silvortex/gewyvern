use super::summary::built_in_protocol_summary;
use super::{protocol_dsl_path, protocol_surface};

#[test]
fn quic_surfaces_split_initial_retry_crypto_close_and_stream_shelves() {
    let initial = protocol_surface("quic", "initial").expect("quic initial should exist");
    let initial_shelf = initial.shelf.expect("quic initial shelf should exist");
    assert_eq!(initial_shelf.key, "initial");
    assert_eq!(
        initial_shelf.page,
        "docs/book/reference-quic-initial-surface.md"
    );

    let retry = protocol_surface("quic", "retry").expect("quic retry should exist");
    let retry_shelf = retry.shelf.expect("quic retry shelf should exist");
    assert_eq!(retry_shelf.key, "retry");
    assert_eq!(
        retry_shelf.page,
        "docs/book/reference-quic-retry-surface.md"
    );

    let close = protocol_surface("quic", "close").expect("quic close should exist");
    let close_shelf = close.shelf.expect("quic close shelf should exist");
    assert_eq!(close_shelf.key, "close");
    assert_eq!(
        close_shelf.page,
        "docs/book/reference-quic-close-surface.md"
    );

    let local_close =
        protocol_surface("quic", "local-close").expect("quic local-close should exist");
    let local_close_shelf = local_close
        .shelf
        .expect("quic local-close shelf should exist");
    assert_eq!(local_close_shelf.key, "local-close");
    assert_eq!(
        local_close_shelf.page,
        "docs/book/reference-quic-local-close-surface.md"
    );
}

#[test]
fn quic_retry_aliases_resolve_to_canonical_entry() {
    assert_eq!(
        protocol_dsl_path("quic", Some("address-validation")),
        Some(super::protocol_fixture_path("quic/retry"))
    );
    assert_eq!(
        protocol_dsl_path("quic", Some("quic-retry")),
        Some(super::protocol_fixture_path("quic/retry"))
    );
    assert_eq!(
        protocol_dsl_path("quic", Some("connection-close")),
        Some(super::protocol_fixture_path("quic/close"))
    );
    assert_eq!(
        protocol_dsl_path("quic", Some("active-close")),
        Some(super::protocol_fixture_path("quic/local-close"))
    );
}

#[test]
fn quic_summary_and_semantics_expose_retry_and_close_entries() {
    let summary = built_in_protocol_summary("quic").expect("quic summary should exist");
    let retry = summary
        .entries
        .iter()
        .find(|entry| entry.mode == "retry")
        .expect("quic retry summary should exist");
    assert!(retry.aliases.contains(&"address-validation".to_string()));
    assert!(retry.aliases.contains(&"quic-retry".to_string()));

    let semantics = protocol_surface("quic", "retry")
        .expect("quic retry should exist")
        .entry_semantics
        .expect("quic retry should expose semantics");
    assert_eq!(semantics.category, "continuation-path");
    assert_eq!(
        semantics.operator_focus,
        "peer address-validation continuation during QUIC Retry evaluation"
    );
    assert_eq!(semantics.typical_signal.as_deref(), Some("Retry"));

    let close = summary
        .entries
        .iter()
        .find(|entry| entry.mode == "close")
        .expect("quic close summary should exist");
    assert!(close.aliases.contains(&"terminate".to_string()));
    assert!(close.aliases.contains(&"quic-close".to_string()));

    let close_semantics = protocol_surface("quic", "close")
        .expect("quic close should exist")
        .entry_semantics
        .expect("quic close should expose semantics");
    assert_eq!(close_semantics.category, "failure-path");
    assert_eq!(
        close_semantics.operator_focus,
        "peer transport termination during QUIC connection close evaluation"
    );
    assert_eq!(
        close_semantics.typical_signal.as_deref(),
        Some("CONNECTION_CLOSE")
    );
    assert_eq!(
        close_semantics.primary_failure_mode.as_deref(),
        Some("peer_closed")
    );

    let local_close = summary
        .entries
        .iter()
        .find(|entry| entry.mode == "local-close")
        .expect("quic local-close summary should exist");
    assert!(local_close.aliases.contains(&"active-close".to_string()));
    assert!(
        local_close
            .aliases
            .contains(&"quic-local-close".to_string())
    );

    let local_close_semantics = protocol_surface("quic", "local-close")
        .expect("quic local-close should exist")
        .entry_semantics
        .expect("quic local-close should expose semantics");
    assert_eq!(local_close_semantics.category, "failure-path");
    assert_eq!(
        local_close_semantics.operator_focus,
        "local transport termination during QUIC connection close evaluation"
    );
    assert_eq!(
        local_close_semantics.typical_signal.as_deref(),
        Some("CONNECTION_CLOSE")
    );
    assert_eq!(
        local_close_semantics.primary_failure_mode.as_deref(),
        Some("local_closed")
    );
}
