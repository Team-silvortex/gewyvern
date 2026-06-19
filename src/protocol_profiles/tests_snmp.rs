use super::{protocol_dsl_path, protocol_entries, protocol_summary};

#[test]
fn snmp_protocol_summary_exposes_new_entries_and_aliases() {
    let summary = protocol_summary("snmp").expect("snmp summary should exist");
    assert_eq!(summary.default_entry, "get");
    assert!(summary.entries.iter().any(|entry| entry.mode == "bulk"));
    assert!(summary.entries.iter().any(|entry| entry.mode == "get-next"));
    assert!(summary.entries.iter().any(|entry| entry.mode == "set"));
    assert!(summary.entries.iter().any(|entry| entry.mode == "trap"));
    assert!(summary.entries.iter().any(|entry| entry.mode == "inform"));
    assert!(summary.entries.iter().any(|entry| entry.mode == "engine-sync"));
    assert!(summary.entries.iter().any(|entry| entry.mode == "trap-recv"));
    assert!(summary.entries.iter().any(|entry| entry.mode == "v3-auth"));
    assert!(summary.entries.iter().any(|entry| entry.mode == "v3-priv"));

    let bulk = summary
        .entries
        .iter()
        .find(|entry| entry.mode == "bulk")
        .expect("snmp bulk entry should exist");
    assert!(bulk.aliases.contains(&"bulk-walk".to_string()));
    assert!(bulk.aliases.contains(&"table-read".to_string()));

    let get_next = summary
        .entries
        .iter()
        .find(|entry| entry.mode == "get-next")
        .expect("snmp get-next entry should exist");
    assert!(get_next.aliases.contains(&"walk".to_string()));
    assert!(get_next.aliases.contains(&"next".to_string()));

    let trap = summary
        .entries
        .iter()
        .find(|entry| entry.mode == "trap")
        .expect("snmp trap entry should exist");
    assert!(trap.aliases.contains(&"notify".to_string()));
    assert!(trap.aliases.contains(&"alert".to_string()));

    let inform = summary
        .entries
        .iter()
        .find(|entry| entry.mode == "inform")
        .expect("snmp inform entry should exist");
    assert!(inform.aliases.contains(&"ack-notify".to_string()));
    assert!(inform.aliases.contains(&"confirm-notify".to_string()));

    let engine_sync = summary
        .entries
        .iter()
        .find(|entry| entry.mode == "engine-sync")
        .expect("snmp engine-sync entry should exist");
    assert!(engine_sync.aliases.contains(&"engine-discovery".to_string()));
    assert!(engine_sync.aliases.contains(&"report-sync".to_string()));

    let trap_recv = summary
        .entries
        .iter()
        .find(|entry| entry.mode == "trap-recv")
        .expect("snmp trap-recv entry should exist");
    assert!(trap_recv.aliases.contains(&"listen-trap".to_string()));
    assert!(trap_recv.aliases.contains(&"trap-listener".to_string()));

    let v3_auth = summary
        .entries
        .iter()
        .find(|entry| entry.mode == "v3-auth")
        .expect("snmp v3-auth entry should exist");
    assert!(v3_auth.aliases.contains(&"auth-user".to_string()));
    assert!(v3_auth.aliases.contains(&"auth-session".to_string()));

    let v3_priv = summary
        .entries
        .iter()
        .find(|entry| entry.mode == "v3-priv")
        .expect("snmp v3-priv entry should exist");
    assert!(v3_priv.aliases.contains(&"private-session".to_string()));
    assert!(v3_priv.aliases.contains(&"encrypted-session".to_string()));
}

#[test]
fn snmp_protocol_aliases_resolve_to_canonical_packages() {
    assert_eq!(
        protocol_dsl_path("snmp-bulk", None),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/snmp/bulk".to_string())
    );
    assert_eq!(
        protocol_dsl_path("snmp-get-next", None),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/snmp/get-next".to_string())
    );
    assert_eq!(
        protocol_dsl_path("snmp-set", None),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/snmp/set".to_string())
    );
    assert_eq!(
        protocol_dsl_path("snmp-trap", None),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/snmp/trap".to_string())
    );
    assert_eq!(
        protocol_dsl_path("snmp-engine-sync", None),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/snmp/engine-sync".to_string())
    );
    assert_eq!(
        protocol_dsl_path("snmp-trap-recv", None),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/snmp/trap-recv".to_string())
    );
    assert_eq!(
        protocol_dsl_path("snmp-v3-auth", None),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/snmp/v3-auth".to_string())
    );
    assert_eq!(
        protocol_dsl_path("snmp-v3-priv", None),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/snmp/v3-priv".to_string())
    );

    let entries = protocol_entries("snmp").expect("snmp entries should resolve");
    assert_eq!(
        entries,
        vec![
            "bulk".to_string(),
            "engine-sync".to_string(),
            "get".to_string(),
            "get-next".to_string(),
            "inform".to_string(),
            "set".to_string(),
            "trap".to_string(),
            "trap-recv".to_string(),
            "v3-auth".to_string(),
            "v3-priv".to_string()
        ]
    );
}
