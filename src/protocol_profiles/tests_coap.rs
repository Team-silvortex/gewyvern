use super::{protocol_dsl_path, protocol_entries, protocol_summary};

#[test]
fn coap_protocol_summary_exposes_new_entries_and_aliases() {
    let _lock = super::tests_env::lock();
    let summary = protocol_summary("coap").expect("coap summary should exist");
    assert_eq!(summary.default_entry, "get");
    assert!(summary.entries.iter().any(|entry| entry.mode == "post"));
    assert!(summary.entries.iter().any(|entry| entry.mode == "put"));
    assert!(summary.entries.iter().any(|entry| entry.mode == "delete"));

    let post = summary
        .entries
        .iter()
        .find(|entry| entry.mode == "post")
        .expect("coap post entry should exist");
    assert!(post.aliases.contains(&"create".to_string()));
    assert!(post.aliases.contains(&"write".to_string()));
}

#[test]
fn coap_protocol_aliases_resolve_to_canonical_packages() {
    let _lock = super::tests_env::lock();
    assert_eq!(
        protocol_dsl_path("coap-post", None),
        Some(super::protocol_fixture_path("coap/post"))
    );
    assert_eq!(
        protocol_dsl_path("coap-put", None),
        Some(super::protocol_fixture_path("coap/put"))
    );
    assert_eq!(
        protocol_dsl_path("coap-delete", None),
        Some(super::protocol_fixture_path("coap/delete"))
    );

    let entries = protocol_entries("coap").expect("coap entries should resolve");
    assert_eq!(
        entries,
        vec![
            "delete".to_string(),
            "get".to_string(),
            "post".to_string(),
            "put".to_string()
        ]
    );
}
