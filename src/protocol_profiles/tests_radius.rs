use super::summary::built_in_protocol_summary;
use super::{protocol_dsl_path, protocol_surface};

#[test]
fn radius_surfaces_split_access_challenge_and_denied_shelves() {
    let access = protocol_surface("radius", "access").expect("radius access surface should exist");
    let access_shelf = access.shelf.expect("radius access should have a shelf");
    assert_eq!(access_shelf.key, "access");
    assert_eq!(access_shelf.label, "Access");
    assert_eq!(
        access_shelf.page,
        "docs/book/reference-radius-access-surface.md"
    );

    let challenge =
        protocol_surface("radius", "challenge").expect("radius challenge surface should exist");
    let challenge_shelf = challenge.shelf.expect("radius challenge should have a shelf");
    assert_eq!(challenge_shelf.key, "challenge");
    assert_eq!(challenge_shelf.label, "Challenge");
    assert_eq!(
        challenge_shelf.page,
        "docs/book/reference-radius-challenge-surface.md"
    );

    let denied = protocol_surface("radius", "denied").expect("radius denied surface should exist");
    let denied_shelf = denied.shelf.expect("radius denied should have a shelf");
    assert_eq!(denied_shelf.key, "denied");
    assert_eq!(denied_shelf.label, "Denied");
    assert_eq!(
        denied_shelf.page,
        "docs/book/reference-radius-denied-surface.md"
    );
}

#[test]
fn radius_aliases_resolve_to_canonical_entries() {
    assert_eq!(
        protocol_dsl_path("radius", Some("radius-access")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/radius/access".to_string())
    );
    assert_eq!(
        protocol_dsl_path("radius", Some("otp")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/radius/challenge".to_string())
    );
    assert_eq!(
        protocol_dsl_path("radius", Some("reject")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/radius/denied".to_string())
    );
}

#[test]
fn radius_summary_and_semantics_expose_new_entries() {
    let summary = built_in_protocol_summary("radius").expect("radius summary should exist");
    let challenge = summary
        .entries
        .iter()
        .find(|entry| entry.mode == "challenge")
        .expect("radius challenge summary should exist");
    assert!(challenge.aliases.contains(&"radius-challenge".to_string()));
    assert!(challenge.aliases.contains(&"otp".to_string()));

    let denied = summary
        .entries
        .iter()
        .find(|entry| entry.mode == "denied")
        .expect("radius denied summary should exist");
    assert!(denied.aliases.contains(&"radius-denied".to_string()));
    assert!(denied.aliases.contains(&"access-denied".to_string()));
    assert!(denied.aliases.contains(&"reject".to_string()));

    let challenge_semantics = protocol_surface("radius", "challenge")
        .expect("radius challenge surface should exist")
        .entry_semantics
        .expect("radius challenge should expose semantics");
    assert_eq!(challenge_semantics.category, "continuation-path");
    assert_eq!(
        challenge_semantics.operator_focus,
        "identity challenge continuation during RADIUS Access-Challenge evaluation"
    );
    assert_eq!(
        challenge_semantics.typical_signal.as_deref(),
        Some("Access-Challenge")
    );

    let denied_semantics = protocol_surface("radius", "denied")
        .expect("radius denied surface should exist")
        .entry_semantics
        .expect("radius denied should expose semantics");
    assert_eq!(denied_semantics.category, "failure-path");
    assert_eq!(
        denied_semantics.operator_focus,
        "identity access rejection during RADIUS Access-Reject evaluation"
    );
    assert_eq!(
        denied_semantics.typical_signal.as_deref(),
        Some("Access-Reject")
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
}
