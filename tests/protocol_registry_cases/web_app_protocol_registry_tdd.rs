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

fn entry_set(protocol: &str) -> BTreeSet<String> {
    protocol_entries(protocol)
        .expect("protocol entries should resolve")
        .into_iter()
        .collect()
}

#[test]
fn websocket_registry_entries_resolve_to_packaged_paths() {
    assert_eq!(
        protocol_default_entry("websocket"),
        Some("upgrade".to_string())
    );
    assert_eq!(
        protocol_dsl_path("websocket", None),
        Some(protocol_fixture_path("websocket/upgrade"))
    );
    assert_eq!(
        protocol_dsl_path("ws", None),
        Some(protocol_fixture_path("websocket/upgrade"))
    );
    assert_eq!(
        protocol_dsl_path("websocket", Some("message")),
        Some(protocol_fixture_path("websocket/frame"))
    );
    assert_eq!(
        protocol_dsl_path("websocket", Some("teardown")),
        Some(protocol_fixture_path("websocket/close"))
    );
}

#[test]
fn graphql_registry_entries_resolve_to_packaged_paths() {
    assert_eq!(protocol_default_entry("graphql"), Some("query".to_string()));
    assert_eq!(
        protocol_dsl_path("graphql", None),
        Some(protocol_fixture_path("graphql/query"))
    );
    assert_eq!(
        protocol_dsl_path("gql", None),
        Some(protocol_fixture_path("graphql/query"))
    );
    assert_eq!(
        protocol_dsl_path("graphql", Some("write")),
        Some(protocol_fixture_path("graphql/mutation"))
    );
    assert_eq!(
        protocol_dsl_path("graphql", Some("subscribe")),
        Some(protocol_fixture_path("graphql/subscription"))
    );
}

#[test]
fn web_app_surfaces_expose_shelves_clusters_and_semantics() {
    assert_eq!(
        entry_set("websocket"),
        ["upgrade", "frame", "close"]
            .into_iter()
            .map(String::from)
            .collect()
    );
    assert_eq!(
        entry_set("graphql"),
        ["query", "mutation", "subscription"]
            .into_iter()
            .map(String::from)
            .collect()
    );

    for (protocol, entries) in [
        ("websocket", ["upgrade", "frame", "close"].as_slice()),
        ("graphql", ["query", "mutation", "subscription"].as_slice()),
    ] {
        for entry in entries {
            let surface = protocol_surface(protocol, entry).expect("surface should exist");
            assert_eq!(
                surface
                    .cluster_hint
                    .as_ref()
                    .expect("cluster should exist")
                    .key,
                "web-proxy-request-response"
            );
            assert!(
                surface.shelf.is_some(),
                "{protocol}/{entry} should expose shelf"
            );
            assert!(
                surface.entry_semantics.is_some(),
                "{protocol}/{entry} should expose debugger semantics"
            );
        }
    }
}

#[test]
fn web_app_stable_subset_dsl_files_compile() {
    for file in [
        "websocket_upgrade_path.gewy",
        "websocket_frame_path.gewy",
        "websocket_close_path.gewy",
        "graphql_query_path.gewy",
        "graphql_mutation_path.gewy",
        "graphql_subscription_path.gewy",
    ] {
        let binding = compile_file(&dsl_fixture_path(file)).expect("web app dsl should compile");
        assert!(
            binding.template.id.starts_with("websocket_")
                || binding.template.id.starts_with("graphql_"),
            "template id should be protocol-specific"
        );
    }
}
