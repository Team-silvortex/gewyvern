use super::{protocol_dsl_path, protocol_entries, protocol_summary};

#[test]
fn dhcp_protocol_summary_exposes_new_entries_and_aliases() {
    let summary = protocol_summary("dhcp").expect("dhcp summary should exist");
    assert_eq!(summary.default_entry, "client");
    assert!(summary.entries.iter().any(|entry| entry.mode == "discover"));
    assert!(summary.entries.iter().any(|entry| entry.mode == "request"));

    let request = summary
        .entries
        .iter()
        .find(|entry| entry.mode == "request")
        .expect("dhcp request entry should exist");
    assert!(request.aliases.contains(&"lease-request".to_string()));
    assert!(request.aliases.contains(&"renew".to_string()));
}

#[test]
fn dhcp_protocol_aliases_resolve_to_canonical_packages() {
    assert_eq!(
        protocol_dsl_path("dhcp-discover", None),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/dhcp/discover".to_string())
    );
    assert_eq!(
        protocol_dsl_path("dhcp-request", None),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/dhcp/request".to_string())
    );

    let entries = protocol_entries("dhcp").expect("dhcp entries should resolve");
    assert_eq!(
        entries,
        vec![
            "client".to_string(),
            "discover".to_string(),
            "request".to_string()
        ]
    );
}
