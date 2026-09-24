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
fn s3_profile_resolves_default_entry_and_aliases() {
    assert_eq!(protocol_default_entry("s3"), Some("get-object".to_string()));
    assert_eq!(
        protocol_dsl_path("aws-s3", None),
        Some(protocol_fixture_path("s3/get-object"))
    );
    assert_eq!(
        protocol_dsl_path("minio", None),
        Some(protocol_fixture_path("s3/get-object"))
    );
    assert_eq!(
        protocol_dsl_path("object-storage", None),
        Some(protocol_fixture_path("s3/get-object"))
    );
}

#[test]
fn s3_entries_resolve_to_stable_subset_dsl_paths() {
    for (entry, path) in [
        ("list-buckets", "s3/list-buckets"),
        ("head-object", "s3/head-object"),
        ("put-object", "s3/put-object"),
        ("get-object", "s3/get-object"),
        ("delete-object", "s3/delete-object"),
    ] {
        assert_eq!(
            protocol_dsl_path("s3", Some(entry)),
            Some(protocol_fixture_path(path))
        );
    }
}

#[test]
fn s3_entry_aliases_resolve_to_object_storage_actions() {
    for (alias, path) in [
        ("s3-list", "s3/list-buckets"),
        ("s3-head", "s3/head-object"),
        ("s3-put", "s3/put-object"),
        ("s3-get", "s3/get-object"),
        ("s3-delete", "s3/delete-object"),
    ] {
        assert_eq!(
            protocol_dsl_path(alias, None),
            Some(protocol_fixture_path(path))
        );
    }
}

#[test]
fn s3_surface_exposes_web_cluster_shelves_and_semantics() {
    let entries = protocol_entries("s3")
        .expect("s3 entries should resolve")
        .into_iter()
        .collect::<BTreeSet<_>>();
    assert_eq!(
        entries,
        [
            "list-buckets",
            "head-object",
            "put-object",
            "get-object",
            "delete-object"
        ]
        .into_iter()
        .map(String::from)
        .collect()
    );

    for (entry, shelf_key) in [
        ("list-buckets", "object-read"),
        ("head-object", "object-read"),
        ("get-object", "object-read"),
        ("put-object", "object-write"),
        ("delete-object", "object-write"),
    ] {
        let surface = protocol_surface("s3", entry).expect("s3 surface should exist");
        assert_eq!(
            surface.cluster_hint.expect("s3 cluster should exist").key,
            "web-proxy-request-response"
        );
        assert_eq!(surface.shelf.expect("s3 shelf should exist").key, shelf_key);
        assert!(
            surface.entry_semantics.is_some(),
            "s3 {entry} should expose object-storage semantics"
        );
    }
}

#[test]
fn s3_stable_subset_dsl_files_compile() {
    for (file, template_id) in [
        ("s3_list_buckets_path.gewy", "s3_list_buckets_path"),
        ("s3_head_object_path.gewy", "s3_head_object_path"),
        ("s3_put_object_path.gewy", "s3_put_object_path"),
        ("s3_get_object_path.gewy", "s3_get_object_path"),
        ("s3_delete_object_path.gewy", "s3_delete_object_path"),
    ] {
        let binding = compile_file(&dsl_fixture_path(file)).expect("s3 dsl should compile");
        assert_eq!(binding.template.id, template_id);
    }
}
