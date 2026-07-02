use gewyvern::dsl::compile_file;
use gewyvern::protocol_profiles::{
    protocol_default_entry, protocol_dsl_path, protocol_entries, protocol_surface,
};
use std::collections::BTreeSet;

fn protocol_fixture_path(relative: &str) -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("protocols")
        .join(relative)
        .to_string_lossy()
        .into_owned()
}

fn dsl_fixture_path(name: &str) -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("dsl")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

#[test]
fn mongodb_registry_entries_resolve_to_packaged_paths() {
    assert_eq!(
        protocol_default_entry("mongodb"),
        Some("command".to_string())
    );
    assert_eq!(
        protocol_dsl_path("mongodb", None),
        Some(protocol_fixture_path("mongodb/command"))
    );
    assert_eq!(
        protocol_dsl_path("mongo", None),
        Some(protocol_fixture_path("mongodb/command"))
    );
    assert_eq!(
        protocol_dsl_path("mongodb", Some("response")),
        Some(protocol_fixture_path("mongodb/reply"))
    );
    assert_eq!(
        protocol_dsl_path("mongodb", Some("op-query")),
        Some(protocol_fixture_path("mongodb/legacy-query"))
    );
    assert_eq!(
        protocol_dsl_path("mongodb", Some("legacy-failure")),
        Some(protocol_fixture_path("mongodb/query-failure"))
    );
    assert_eq!(
        protocol_dsl_path("mongo-query-failure", None),
        Some(protocol_fixture_path("mongodb/query-failure"))
    );
}

#[test]
fn mongodb_surface_exposes_database_cluster_shelves_and_semantics() {
    let entries = protocol_entries("mongodb")
        .expect("mongodb entries should resolve")
        .into_iter()
        .collect::<BTreeSet<_>>();
    assert_eq!(
        entries,
        ["command", "reply", "legacy-query", "query-failure"]
            .into_iter()
            .map(String::from)
            .collect()
    );

    for (entry, shelf_key) in [
        ("command", "command"),
        ("reply", "reply"),
        ("legacy-query", "legacy-query"),
        ("query-failure", "legacy-query"),
    ] {
        let surface = protocol_surface("mongodb", entry).expect("mongodb surface should exist");
        assert_eq!(
            surface
                .cluster_hint
                .as_ref()
                .expect("mongodb cluster should exist")
                .key,
            "database-query-session"
        );
        assert_eq!(
            surface.shelf.expect("mongodb shelf should exist").key,
            shelf_key
        );
        assert!(
            surface.entry_semantics.is_some(),
            "mongodb {entry} should expose debugger semantics"
        );
    }

    let failure = protocol_surface("mongodb", "query-failure")
        .expect("mongodb query-failure surface should exist")
        .entry_semantics
        .expect("mongodb query-failure should expose semantics");
    assert_eq!(failure.category, "failure-path");
    assert_eq!(
        failure.primary_failure_mode.as_deref(),
        Some("semantic_error")
    );
}

#[test]
fn mongodb_stable_subset_dsl_files_compile() {
    for file in [
        "mongodb_command_path.gewy",
        "mongodb_reply_path.gewy",
        "mongodb_legacy_query_path.gewy",
        "mongodb_query_failure_path.gewy",
    ] {
        let binding = compile_file(&dsl_fixture_path(file)).expect("mongodb dsl should compile");
        assert!(
            binding.template.id.starts_with("mongodb_"),
            "mongodb template id should be protocol-specific"
        );
    }
}
