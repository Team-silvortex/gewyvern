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
fn consul_profile_resolves_default_entry_and_aliases() {
    assert_eq!(
        protocol_default_entry("consul"),
        Some("service".to_string())
    );
    assert_eq!(
        protocol_dsl_path("consul-agent", None),
        Some(protocol_fixture_path("consul/service"))
    );
    assert_eq!(
        protocol_dsl_path("service-discovery", None),
        Some(protocol_fixture_path("consul/service"))
    );
}

#[test]
fn consul_entries_resolve_to_packaged_paths() {
    for (entry, path) in [
        ("health", "consul/health"),
        ("catalog", "consul/catalog"),
        ("service", "consul/service"),
        ("kv", "consul/kv"),
        ("session", "consul/session"),
    ] {
        assert_eq!(
            protocol_dsl_path("consul", Some(entry)),
            Some(protocol_fixture_path(path))
        );
    }
}

#[test]
fn consul_entry_aliases_resolve_to_service_discovery_actions() {
    for (alias, path) in [
        ("consul-health", "consul/health"),
        ("consul-catalog", "consul/catalog"),
        ("consul-service", "consul/service"),
        ("consul-kv", "consul/kv"),
        ("consul-session", "consul/session"),
    ] {
        assert_eq!(
            protocol_dsl_path(alias, None),
            Some(protocol_fixture_path(path))
        );
    }
}

#[test]
fn consul_surface_exposes_database_cluster_shelves_and_semantics() {
    let entries = protocol_entries("consul")
        .expect("consul entries should resolve")
        .into_iter()
        .collect::<BTreeSet<_>>();
    assert_eq!(
        entries,
        ["health", "catalog", "service", "kv", "session"]
            .into_iter()
            .map(String::from)
            .collect()
    );

    for (entry, shelf_key) in [
        ("health", "discovery-health"),
        ("catalog", "discovery-health"),
        ("service", "discovery-health"),
        ("kv", "state-session"),
        ("session", "state-session"),
    ] {
        let surface = protocol_surface("consul", entry).expect("consul surface should exist");
        assert_eq!(
            surface
                .cluster_hint
                .expect("consul cluster should exist")
                .key,
            "database-query-session"
        );
        assert_eq!(
            surface.shelf.expect("consul shelf should exist").key,
            shelf_key
        );
        assert!(
            surface.entry_semantics.is_some(),
            "consul {entry} should expose service discovery semantics"
        );
    }
}

#[test]
fn consul_stable_subset_dsl_files_compile() {
    for (file, template_id) in [
        ("consul_health_path.gewy", "consul_health_path"),
        ("consul_catalog_path.gewy", "consul_catalog_path"),
        ("consul_service_path.gewy", "consul_service_path"),
        ("consul_kv_path.gewy", "consul_kv_path"),
        ("consul_session_path.gewy", "consul_session_path"),
    ] {
        let binding = compile_file(&dsl_fixture_path(file)).expect("Consul DSL should compile");
        assert_eq!(binding.template.id, template_id);
    }
}
