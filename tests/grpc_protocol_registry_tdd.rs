use gewyvern::dsl::compile_file;
use gewyvern::protocol_profiles::{
    protocol_default_entry, protocol_dsl_path, protocol_entries, protocol_surface,
};

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
fn grpc_registry_entries_resolve_to_packaged_paths() {
    assert_eq!(protocol_default_entry("grpc"), Some("call".to_string()));
    assert_eq!(
        protocol_dsl_path("grpc", None),
        Some(protocol_fixture_path("grpc/call"))
    );
    assert_eq!(
        protocol_dsl_path("grpc", Some("status")),
        Some(protocol_fixture_path("grpc/status"))
    );
    assert_eq!(
        protocol_dsl_path("grpc-stream", None),
        Some(protocol_fixture_path("grpc/stream"))
    );
    assert_eq!(
        protocol_dsl_path("grpc", Some("bidi")),
        Some(protocol_fixture_path("grpc/stream"))
    );
}

#[test]
fn grpc_surface_exposes_rpc_shelves_and_semantics() {
    let entries = protocol_entries("grpc").expect("grpc entries should resolve");
    assert_eq!(entries, vec!["call", "status", "stream"]);

    for (entry, shelf_key) in [("call", "call"), ("status", "status"), ("stream", "stream")] {
        let surface = protocol_surface("grpc", entry).expect("grpc surface should exist");
        assert_eq!(
            surface
                .cluster_hint
                .as_ref()
                .expect("grpc cluster should exist")
                .key,
            "web-proxy-request-response"
        );
        assert_eq!(
            surface.shelf.expect("grpc shelf should exist").key,
            shelf_key
        );
        assert!(
            surface.entry_semantics.is_some(),
            "grpc {entry} should expose debugger semantics"
        );
    }
}

#[test]
fn grpc_stable_subset_dsl_files_compile() {
    for file in [
        "grpc_call_path.gewy",
        "grpc_status_path.gewy",
        "grpc_stream_path.gewy",
    ] {
        let binding = compile_file(&dsl_fixture_path(file)).expect("grpc dsl should compile");
        assert!(
            binding.template.id.starts_with("grpc_"),
            "grpc template id should be protocol-specific"
        );
    }
}
