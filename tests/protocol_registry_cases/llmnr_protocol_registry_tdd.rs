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
fn llmnr_registry_entries_and_aliases_resolve_to_packaged_paths() {
    assert_eq!(
        protocol_dsl_path("llmnr", Some("query")),
        Some(protocol_fixture_path("llmnr/query"))
    );
    assert_eq!(
        protocol_dsl_path("llmnr-query", None),
        Some(protocol_fixture_path("llmnr/query"))
    );
    assert_eq!(
        protocol_dsl_path("llmnr", Some("answer")),
        Some(protocol_fixture_path("llmnr/response"))
    );
    assert_eq!(
        protocol_dsl_path("llmnr-response", None),
        Some(protocol_fixture_path("llmnr/response"))
    );
    assert_eq!(
        protocol_dsl_path("llmnr", Some("resolution-failed")),
        Some(protocol_fixture_path("llmnr/error"))
    );
    assert_eq!(
        protocol_dsl_path("llmnr_error", None),
        Some(protocol_fixture_path("llmnr/error"))
    );
}

#[test]
fn llmnr_surface_exposes_local_name_shelves_and_semantics() {
    assert_eq!(protocol_default_entry("llmnr"), Some("query".to_string()));
    let entries = protocol_entries("llmnr").expect("llmnr entries should resolve");
    assert_eq!(entries, ["error", "query", "response"]);

    for (entry, shelf, category) in [
        ("query", "query", "local-name-query-path"),
        ("response", "response", "local-name-response-path"),
        ("error", "error", "local-name-error-path"),
    ] {
        let surface = protocol_surface("llmnr", entry).expect("llmnr surface should exist");
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
fn llmnr_stable_subset_dsl_files_compile() {
    for (file, template, operation) in [
        ("llmnr_query_path.gewy", "llmnr_query_path", "llmnr_query"),
        (
            "llmnr_response_path.gewy",
            "llmnr_response_path",
            "llmnr_response",
        ),
        ("llmnr_error_path.gewy", "llmnr_error_path", "llmnr_error"),
    ] {
        let binding = compile_file(&dsl_fixture_path(file)).expect("llmnr DSL should compile");
        assert_eq!(binding.template.id, template);
        assert_eq!(
            binding.template.program_model.as_ref().unwrap().operation,
            ProgramOperation::Custom(operation.into())
        );
    }
}
