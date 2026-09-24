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
fn zookeeper_profile_resolves_default_entry_and_aliases() {
    assert_eq!(
        protocol_default_entry("zookeeper"),
        Some("read".to_string())
    );
    assert_eq!(
        protocol_dsl_path("zk", None),
        Some(protocol_fixture_path("zookeeper/read"))
    );
    assert_eq!(
        protocol_dsl_path("zookeeper-client", None),
        Some(protocol_fixture_path("zookeeper/read"))
    );
}

#[test]
fn zookeeper_entries_resolve_to_packaged_paths() {
    for (entry, path) in [
        ("connect", "zookeeper/connect"),
        ("read", "zookeeper/read"),
        ("write", "zookeeper/write"),
        ("watch", "zookeeper/watch"),
        ("auth-denied", "zookeeper/auth-denied"),
    ] {
        assert_eq!(
            protocol_dsl_path("zookeeper", Some(entry)),
            Some(protocol_fixture_path(path))
        );
    }
}

#[test]
fn zookeeper_entry_aliases_resolve_to_coordination_actions() {
    for (alias, path) in [
        ("zk-connect", "zookeeper/connect"),
        ("zk-read", "zookeeper/read"),
        ("zk-write", "zookeeper/write"),
        ("zk-watch", "zookeeper/watch"),
        ("zk-auth-denied", "zookeeper/auth-denied"),
    ] {
        assert_eq!(
            protocol_dsl_path(alias, None),
            Some(protocol_fixture_path(path))
        );
    }
}

#[test]
fn zookeeper_surface_exposes_database_cluster_shelves_and_semantics() {
    let entries = protocol_entries("zookeeper")
        .expect("zookeeper entries should resolve")
        .into_iter()
        .collect::<BTreeSet<_>>();
    assert_eq!(
        entries,
        ["connect", "read", "write", "watch", "auth-denied"]
            .into_iter()
            .map(String::from)
            .collect()
    );

    for (entry, shelf_key) in [
        ("connect", "session-auth"),
        ("auth-denied", "session-auth"),
        ("read", "znode-data"),
        ("write", "znode-data"),
        ("watch", "watch"),
    ] {
        let surface = protocol_surface("zookeeper", entry).expect("surface should exist");
        assert_eq!(
            surface
                .cluster_hint
                .expect("zookeeper cluster should exist")
                .key,
            "database-query-session"
        );
        assert_eq!(
            surface.shelf.expect("zookeeper shelf should exist").key,
            shelf_key
        );
        assert!(
            surface.entry_semantics.is_some(),
            "zookeeper {entry} should expose coordination semantics"
        );
    }
}

#[test]
fn zookeeper_stable_subset_dsl_files_compile() {
    for (file, template_id) in [
        ("zookeeper_connect_path.gewy", "zookeeper_connect_path"),
        ("zookeeper_read_path.gewy", "zookeeper_read_path"),
        ("zookeeper_write_path.gewy", "zookeeper_write_path"),
        ("zookeeper_watch_path.gewy", "zookeeper_watch_path"),
        (
            "zookeeper_auth_denied_path.gewy",
            "zookeeper_auth_denied_path",
        ),
    ] {
        let binding = compile_file(&dsl_fixture_path(file)).expect("ZooKeeper DSL should compile");
        assert_eq!(binding.template.id, template_id);
    }
}
