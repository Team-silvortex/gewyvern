use super::{protocol_dsl_path, protocol_entries, protocol_summary};

#[test]
fn stun_protocol_summary_exposes_new_entries_and_aliases() {
    let _lock = super::tests_env::lock();
    let summary = protocol_summary("stun").expect("stun summary should exist");
    assert_eq!(summary.default_entry, "binding");
    assert!(summary.entries.iter().any(|entry| entry.mode == "allocate"));
    assert!(
        summary
            .entries
            .iter()
            .any(|entry| entry.mode == "binding-error")
    );
    assert!(summary.entries.iter().any(|entry| entry.mode == "refresh"));

    let allocate = summary
        .entries
        .iter()
        .find(|entry| entry.mode == "allocate")
        .expect("stun allocate entry should exist");
    assert!(allocate.aliases.contains(&"relay".to_string()));
    assert!(allocate.aliases.contains(&"turn-allocate".to_string()));

    let binding_error = summary
        .entries
        .iter()
        .find(|entry| entry.mode == "binding-error")
        .expect("stun binding-error entry should exist");
    assert!(
        binding_error
            .aliases
            .contains(&"binding-denied".to_string())
    );
    assert!(binding_error.aliases.contains(&"binding-error".to_string()));
}

#[test]
fn stun_protocol_aliases_resolve_to_canonical_packages() {
    let _lock = super::tests_env::lock();
    assert_eq!(
        protocol_dsl_path("stun-allocate", None),
        Some(super::protocol_fixture_path("stun/allocate"))
    );
    assert_eq!(
        protocol_dsl_path("stun-refresh", None),
        Some(super::protocol_fixture_path("stun/refresh"))
    );
    assert_eq!(
        protocol_dsl_path("stun-binding-error", None),
        Some(super::protocol_fixture_path("stun/binding-error"))
    );

    let entries = protocol_entries("stun").expect("stun entries should resolve");
    assert_eq!(
        entries,
        vec![
            "allocate".to_string(),
            "binding".to_string(),
            "binding-error".to_string(),
            "refresh".to_string()
        ]
    );
}
