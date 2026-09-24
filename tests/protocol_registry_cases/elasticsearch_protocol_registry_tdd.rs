use gewyvern::dsl::compile_file;
use gewyvern::protocol_profiles::{
    protocol_default_entry, protocol_dsl_path, protocol_entries, protocol_surface,
};
use std::collections::BTreeSet;

fn dsl_fixture_path(name: &str) -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("dsl")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

fn protocol_fixture_path(relative: &str) -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("protocols")
        .join(relative)
        .to_string_lossy()
        .into_owned()
}

#[test]
fn elasticsearch_profile_resolves_default_entry_and_aliases() {
    assert_eq!(
        protocol_default_entry("elasticsearch"),
        Some("search".to_string())
    );
    assert_eq!(
        protocol_dsl_path("opensearch", None),
        Some(protocol_fixture_path("elasticsearch/search"))
    );
    assert_eq!(
        protocol_dsl_path("elastic", None),
        Some(protocol_fixture_path("elasticsearch/search"))
    );
    assert_eq!(
        protocol_dsl_path("es", None),
        Some(protocol_fixture_path("elasticsearch/search"))
    );
}

#[test]
fn elasticsearch_entries_resolve_to_packaged_paths() {
    for (entry, path) in [
        ("health", "elasticsearch/health"),
        ("search", "elasticsearch/search"),
        ("index", "elasticsearch/index"),
        ("bulk", "elasticsearch/bulk"),
    ] {
        assert_eq!(
            protocol_dsl_path("elasticsearch", Some(entry)),
            Some(protocol_fixture_path(path))
        );
    }
}

#[test]
fn elasticsearch_entry_aliases_resolve_to_search_datastore_actions() {
    for (alias, path) in [
        ("es-health", "elasticsearch/health"),
        ("es-search", "elasticsearch/search"),
        ("es-index", "elasticsearch/index"),
        ("es-bulk", "elasticsearch/bulk"),
    ] {
        assert_eq!(
            protocol_dsl_path(alias, None),
            Some(protocol_fixture_path(path))
        );
    }
}

#[test]
fn elasticsearch_surface_exposes_database_cluster_shelves_and_semantics() {
    let entries = protocol_entries("elasticsearch")
        .expect("elasticsearch entries should resolve")
        .into_iter()
        .collect::<BTreeSet<_>>();
    assert_eq!(
        entries,
        ["health", "search", "index", "bulk"]
            .into_iter()
            .map(String::from)
            .collect()
    );

    for (entry, shelf_key) in [
        ("health", "cluster-health"),
        ("search", "query"),
        ("index", "mutation"),
        ("bulk", "mutation"),
    ] {
        let surface =
            protocol_surface("elasticsearch", entry).expect("elasticsearch surface should exist");
        assert_eq!(
            surface
                .cluster_hint
                .expect("elasticsearch cluster should exist")
                .key,
            "database-query-session"
        );
        assert_eq!(
            surface.shelf.expect("elasticsearch shelf should exist").key,
            shelf_key
        );
        assert!(
            surface.entry_semantics.is_some(),
            "elasticsearch {entry} should expose search datastore semantics"
        );
    }
}

#[test]
fn elasticsearch_stable_subset_dsl_files_compile() {
    for (file, template_id) in [
        (
            "elasticsearch_health_path.gewy",
            "elasticsearch_health_path",
        ),
        (
            "elasticsearch_search_path.gewy",
            "elasticsearch_search_path",
        ),
        ("elasticsearch_index_path.gewy", "elasticsearch_index_path"),
        ("elasticsearch_bulk_path.gewy", "elasticsearch_bulk_path"),
    ] {
        let binding =
            compile_file(&dsl_fixture_path(file)).expect("elasticsearch dsl should compile");
        assert_eq!(binding.template.id, template_id);
    }
}
