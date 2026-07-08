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
fn rip_registry_entries_and_aliases_resolve_to_packaged_paths() {
    assert_eq!(
        protocol_dsl_path("rip", Some("request")),
        Some(protocol_fixture_path("rip/request"))
    );
    assert_eq!(
        protocol_dsl_path("rip-request", None),
        Some(protocol_fixture_path("rip/request"))
    );
    assert_eq!(
        protocol_dsl_path("rip", Some("route-update")),
        Some(protocol_fixture_path("rip/response"))
    );
    assert_eq!(
        protocol_dsl_path("rip-response", None),
        Some(protocol_fixture_path("rip/response"))
    );
    assert_eq!(
        protocol_dsl_path("rip", Some("metric16")),
        Some(protocol_fixture_path("rip/unreachable"))
    );
    assert_eq!(
        protocol_dsl_path("rip_unreachable", None),
        Some(protocol_fixture_path("rip/unreachable"))
    );
}

#[test]
fn rip_surface_exposes_distance_vector_shelves_and_semantics() {
    assert_eq!(protocol_default_entry("rip"), Some("request".to_string()));
    let entries = protocol_entries("rip").expect("rip entries should resolve");
    assert_eq!(entries, ["request", "response", "unreachable"]);

    for (entry, shelf, category) in [
        ("request", "exchange", "distance-vector-route-request"),
        ("response", "exchange", "distance-vector-route-update"),
        ("unreachable", "failure", "distance-vector-route-withdrawal"),
    ] {
        let surface = protocol_surface("rip", entry).expect("rip surface should exist");
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
fn rip_stable_subset_dsl_files_compile() {
    for (file, template, operation) in [
        ("rip_request_path.gewy", "rip_request_path", "rip_request"),
        (
            "rip_response_path.gewy",
            "rip_response_path",
            "rip_response",
        ),
        (
            "rip_unreachable_path.gewy",
            "rip_unreachable_path",
            "rip_unreachable",
        ),
    ] {
        let binding = compile_file(&dsl_fixture_path(file)).expect("rip DSL should compile");
        assert_eq!(binding.template.id, template);
        assert_eq!(
            binding.template.program_model.as_ref().unwrap().operation,
            ProgramOperation::Custom(operation.into())
        );
    }
}
