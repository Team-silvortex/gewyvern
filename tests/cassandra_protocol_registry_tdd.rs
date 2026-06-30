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
fn cassandra_registry_entries_resolve_to_packaged_paths() {
    assert_eq!(
        protocol_default_entry("cassandra"),
        Some("query".to_string())
    );
    assert_eq!(
        protocol_dsl_path("cassandra", None),
        Some(protocol_fixture_path("cassandra/query"))
    );
    assert_eq!(
        protocol_dsl_path("cql", None),
        Some(protocol_fixture_path("cassandra/query"))
    );
    assert_eq!(
        protocol_dsl_path("cassandra", Some("handshake")),
        Some(protocol_fixture_path("cassandra/startup"))
    );
    assert_eq!(
        protocol_dsl_path("cassandra", Some("rows")),
        Some(protocol_fixture_path("cassandra/result"))
    );
    assert_eq!(
        protocol_dsl_path("cassandra", Some("server-error")),
        Some(protocol_fixture_path("cassandra/error"))
    );
}

#[test]
fn cassandra_surface_exposes_database_cluster_shelves_and_semantics() {
    let entries = protocol_entries("cassandra")
        .expect("cassandra entries should resolve")
        .into_iter()
        .collect::<BTreeSet<_>>();
    assert_eq!(
        entries,
        ["startup", "query", "result", "error"]
            .into_iter()
            .map(String::from)
            .collect()
    );

    for (entry, shelf_key) in [
        ("startup", "session-query"),
        ("query", "session-query"),
        ("result", "session-query"),
        ("error", "error"),
    ] {
        let surface = protocol_surface("cassandra", entry).expect("cassandra surface should exist");
        assert_eq!(
            surface
                .cluster_hint
                .as_ref()
                .expect("cassandra cluster should exist")
                .key,
            "database-query-session"
        );
        assert_eq!(
            surface.shelf.expect("cassandra shelf should exist").key,
            shelf_key
        );
        assert!(
            surface.entry_semantics.is_some(),
            "cassandra {entry} should expose debugger semantics"
        );
    }
}

#[test]
fn cassandra_stable_subset_dsl_files_compile() {
    for file in [
        "cassandra_startup_path.gewy",
        "cassandra_query_path.gewy",
        "cassandra_result_path.gewy",
        "cassandra_error_path.gewy",
    ] {
        let binding = compile_file(&dsl_fixture_path(file)).expect("cassandra dsl should compile");
        assert!(
            binding.template.id.starts_with("cassandra_"),
            "cassandra template id should be protocol-specific"
        );
    }
}
