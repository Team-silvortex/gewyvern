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
fn jaeger_profile_resolves_default_entry_and_aliases() {
    assert_eq!(
        protocol_default_entry("jaeger"),
        Some("collector".to_string())
    );
    assert_eq!(
        protocol_dsl_path("trace-collector", None),
        Some(protocol_fixture_path("jaeger/collector"))
    );
    assert_eq!(
        protocol_dsl_path("jaeger_collector", None),
        Some(protocol_fixture_path("jaeger/collector"))
    );
}

#[test]
fn jaeger_entries_resolve_to_packaged_paths() {
    for (entry, path) in [
        ("collector", "jaeger/collector"),
        ("agent-thrift", "jaeger/agent-thrift"),
        ("query", "jaeger/query"),
        ("sampling", "jaeger/sampling"),
        ("dependencies", "jaeger/dependencies"),
    ] {
        assert_eq!(
            protocol_dsl_path("jaeger", Some(entry)),
            Some(protocol_fixture_path(path))
        );
    }
}

#[test]
fn jaeger_entry_aliases_resolve_to_trace_paths() {
    for (alias, path) in [
        ("span-ingest", "jaeger/collector"),
        ("compact-thrift", "jaeger/agent-thrift"),
        ("trace-search", "jaeger/query"),
        ("sampling-strategies", "jaeger/sampling"),
        ("service-graph", "jaeger/dependencies"),
    ] {
        assert_eq!(
            protocol_dsl_path("jaeger", Some(alias)),
            Some(protocol_fixture_path(path))
        );
    }
}

#[test]
fn jaeger_surface_exposes_rpc_cluster_shelves_and_semantics() {
    let entries = protocol_entries("jaeger")
        .expect("jaeger entries should resolve")
        .into_iter()
        .collect::<BTreeSet<_>>();
    assert_eq!(
        entries,
        [
            "collector",
            "agent-thrift",
            "query",
            "sampling",
            "dependencies",
        ]
        .into_iter()
        .map(String::from)
        .collect()
    );

    for (entry, shelf_key) in [
        ("collector", "trace-ingest"),
        ("agent-thrift", "trace-ingest"),
        ("query", "trace-read"),
        ("dependencies", "trace-read"),
        ("sampling", "sampling-control"),
    ] {
        let surface = protocol_surface("jaeger", entry).expect("jaeger surface should exist");
        assert_eq!(
            surface
                .cluster_hint
                .expect("jaeger cluster should exist")
                .key,
            "web-proxy-request-response"
        );
        assert_eq!(
            surface.shelf.expect("jaeger shelf should exist").key,
            shelf_key
        );
        assert!(
            surface.entry_semantics.is_some(),
            "jaeger {entry} should expose trace semantics"
        );
    }
}

#[test]
fn jaeger_stable_subset_dsl_files_compile() {
    for (file, template_id) in [
        ("jaeger_collector_path.gewy", "jaeger_collector_path"),
        ("jaeger_agent_thrift_path.gewy", "jaeger_agent_thrift_path"),
        ("jaeger_query_path.gewy", "jaeger_query_path"),
        ("jaeger_sampling_path.gewy", "jaeger_sampling_path"),
        ("jaeger_dependencies_path.gewy", "jaeger_dependencies_path"),
    ] {
        let binding = compile_file(&dsl_fixture_path(file)).expect("jaeger DSL should compile");
        assert_eq!(binding.template.id, template_id);
    }
}
