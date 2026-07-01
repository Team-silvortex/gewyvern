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
fn etcd_profile_resolves_default_entry_and_aliases() {
    assert_eq!(protocol_default_entry("etcd"), Some("range".to_string()));
    assert_eq!(
        protocol_dsl_path("etcdctl", None),
        Some(protocol_fixture_path("etcd/range"))
    );
    assert_eq!(
        protocol_dsl_path("etcd-kv", None),
        Some(protocol_fixture_path("etcd/range"))
    );
}

#[test]
fn etcd_entries_resolve_to_packaged_paths() {
    for (entry, path) in [
        ("health", "etcd/health"),
        ("range", "etcd/range"),
        ("put", "etcd/put"),
        ("watch", "etcd/watch"),
        ("lease", "etcd/lease"),
    ] {
        assert_eq!(
            protocol_dsl_path("etcd", Some(entry)),
            Some(protocol_fixture_path(path))
        );
    }
}

#[test]
fn etcd_entry_aliases_resolve_to_control_plane_actions() {
    for (alias, path) in [
        ("etcd-health", "etcd/health"),
        ("etcd-range", "etcd/range"),
        ("etcd-put", "etcd/put"),
        ("etcd-watch", "etcd/watch"),
        ("etcd-lease", "etcd/lease"),
    ] {
        assert_eq!(
            protocol_dsl_path(alias, None),
            Some(protocol_fixture_path(path))
        );
    }
}

#[test]
fn etcd_surface_exposes_database_cluster_shelves_and_semantics() {
    let entries = protocol_entries("etcd")
        .expect("etcd entries should resolve")
        .into_iter()
        .collect::<BTreeSet<_>>();
    assert_eq!(
        entries,
        ["health", "range", "put", "watch", "lease"]
            .into_iter()
            .map(String::from)
            .collect()
    );

    for (entry, shelf_key) in [
        ("health", "cluster-health"),
        ("range", "kv"),
        ("put", "kv"),
        ("watch", "stream-lifecycle"),
        ("lease", "stream-lifecycle"),
    ] {
        let surface = protocol_surface("etcd", entry).expect("etcd surface should exist");
        assert_eq!(
            surface.cluster_hint.expect("etcd cluster should exist").key,
            "database-query-session"
        );
        assert_eq!(
            surface.shelf.expect("etcd shelf should exist").key,
            shelf_key
        );
        assert!(
            surface.entry_semantics.is_some(),
            "etcd {entry} should expose coordination datastore semantics"
        );
    }
}

#[test]
fn etcd_stable_subset_dsl_files_compile() {
    for (file, template_id) in [
        ("etcd_health_path.gewy", "etcd_health_path"),
        ("etcd_range_path.gewy", "etcd_range_path"),
        ("etcd_put_path.gewy", "etcd_put_path"),
        ("etcd_watch_path.gewy", "etcd_watch_path"),
        ("etcd_lease_path.gewy", "etcd_lease_path"),
    ] {
        let binding = compile_file(&dsl_fixture_path(file)).expect("etcd DSL should compile");
        assert_eq!(binding.template.id, template_id);
    }
}
