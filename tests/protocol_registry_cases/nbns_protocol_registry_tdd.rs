use gewyvern::dsl::compile_file;
use gewyvern::flow::ProgramOperation;
use gewyvern::protocol_profiles::{
    protocol_default_entry, protocol_dsl_path, protocol_entries, protocol_surface,
};

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
fn nbns_registry_entries_and_aliases_resolve_to_packaged_paths() {
    assert_eq!(
        protocol_dsl_path("nbns", Some("query")),
        Some(protocol_fixture_path("nbns/query"))
    );
    assert_eq!(
        protocol_dsl_path("nbns-query", None),
        Some(protocol_fixture_path("nbns/query"))
    );
    assert_eq!(
        protocol_dsl_path("nbns", Some("name-answer")),
        Some(protocol_fixture_path("nbns/response"))
    );
    assert_eq!(
        protocol_dsl_path("nbns-response", None),
        Some(protocol_fixture_path("nbns/response"))
    );
    assert_eq!(
        protocol_dsl_path("nbns", Some("name-not-found")),
        Some(protocol_fixture_path("nbns/negative"))
    );
    assert_eq!(
        protocol_dsl_path("nbns_negative", None),
        Some(protocol_fixture_path("nbns/negative"))
    );
}

#[test]
fn nbns_surface_exposes_legacy_local_name_shelves_and_semantics() {
    assert_eq!(protocol_default_entry("nbns"), Some("query".to_string()));
    let entries = protocol_entries("nbns").expect("nbns entries should resolve");
    assert_eq!(entries, ["negative", "query", "response"]);

    for (entry, shelf, category) in [
        ("query", "query", "legacy-local-name-query-path"),
        ("response", "response", "legacy-local-name-response-path"),
        ("negative", "negative", "legacy-local-name-negative-path"),
    ] {
        let surface = protocol_surface("nbns", entry).expect("nbns surface should exist");
        assert_eq!(
            surface
                .cluster_hint
                .as_ref()
                .map(|cluster| cluster.key.as_str()),
            Some("network-control-discovery")
        );
        assert_eq!(
            surface.shelf.as_ref().map(|item| item.key.as_str()),
            Some(shelf)
        );
        assert_eq!(
            surface
                .entry_semantics
                .as_ref()
                .map(|item| item.category.as_str()),
            Some(category)
        );
    }
}

#[test]
fn nbns_stable_subset_dsl_files_compile() {
    for (file, template, operation) in [
        ("nbns_query_path.gewy", "nbns_query_path", "nbns_query"),
        (
            "nbns_response_path.gewy",
            "nbns_response_path",
            "nbns_response",
        ),
        (
            "nbns_negative_path.gewy",
            "nbns_negative_path",
            "nbns_negative",
        ),
    ] {
        let binding = compile_file(&dsl_fixture_path(file)).expect("nbns DSL should compile");
        assert_eq!(binding.template.id, template);
        assert_eq!(
            binding.template.program_model.as_ref().unwrap().operation,
            ProgramOperation::Custom(operation.into())
        );
    }
}
