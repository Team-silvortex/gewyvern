use super::{protocol_dsl_path, protocol_entries, protocol_summary};

#[test]
fn ntp_protocol_summary_exposes_new_entries_and_aliases() {
    let _lock = super::tests_env::lock();
    let summary = protocol_summary("ntp").expect("ntp summary should exist");
    assert_eq!(summary.default_entry, "client");
    assert!(summary.entries.iter().any(|entry| entry.mode == "query"));
    assert!(summary.entries.iter().any(|entry| entry.mode == "sync"));

    let sync = summary
        .entries
        .iter()
        .find(|entry| entry.mode == "sync")
        .expect("ntp sync entry should exist");
    assert!(sync.aliases.contains(&"clock-sync".to_string()));
    assert!(sync.aliases.contains(&"time-sync".to_string()));
}

#[test]
fn ntp_protocol_aliases_resolve_to_canonical_packages() {
    let _lock = super::tests_env::lock();
    assert_eq!(
        protocol_dsl_path("ntp-query", None),
        Some(super::protocol_fixture_path("ntp/query"))
    );
    assert_eq!(
        protocol_dsl_path("ntp-sync", None),
        Some(super::protocol_fixture_path("ntp/sync"))
    );

    let entries = protocol_entries("ntp").expect("ntp entries should resolve");
    assert_eq!(
        entries,
        vec![
            "client".to_string(),
            "query".to_string(),
            "sync".to_string()
        ]
    );
}
