use super::{protocol_dsl_path, protocol_entries, protocol_summary};

#[test]
fn snmp_protocol_summary_exposes_new_entries_and_aliases() {
    let summary = protocol_summary("snmp").expect("snmp summary should exist");
    assert_eq!(summary.default_entry, "get");
    assert!(summary.entries.iter().any(|entry| entry.mode == "get-next"));
    assert!(summary.entries.iter().any(|entry| entry.mode == "set"));

    let get_next = summary
        .entries
        .iter()
        .find(|entry| entry.mode == "get-next")
        .expect("snmp get-next entry should exist");
    assert!(get_next.aliases.contains(&"walk".to_string()));
    assert!(get_next.aliases.contains(&"next".to_string()));
}

#[test]
fn snmp_protocol_aliases_resolve_to_canonical_packages() {
    assert_eq!(
        protocol_dsl_path("snmp-get-next", None),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/snmp/get-next".to_string())
    );
    assert_eq!(
        protocol_dsl_path("snmp-set", None),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/snmp/set".to_string())
    );

    let entries = protocol_entries("snmp").expect("snmp entries should resolve");
    assert_eq!(
        entries,
        vec!["get".to_string(), "get-next".to_string(), "set".to_string()]
    );
}
