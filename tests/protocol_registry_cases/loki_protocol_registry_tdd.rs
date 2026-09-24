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
fn loki_profile_resolves_default_entry_and_aliases() {
    assert_eq!(protocol_default_entry("loki"), Some("push".to_string()));
    assert_eq!(
        protocol_dsl_path("log-push", None),
        Some(protocol_fixture_path("loki/push"))
    );
    assert_eq!(
        protocol_dsl_path("loki_push", None),
        Some(protocol_fixture_path("loki/push"))
    );
}

#[test]
fn loki_entries_resolve_to_packaged_paths() {
    for (entry, path) in [
        ("push", "loki/push"),
        ("query", "loki/query"),
        ("tail", "loki/tail"),
        ("labels", "loki/labels"),
        ("rules", "loki/rules"),
    ] {
        assert_eq!(
            protocol_dsl_path("loki", Some(entry)),
            Some(protocol_fixture_path(path))
        );
    }
}

#[test]
fn loki_entry_aliases_resolve_to_log_paths() {
    for (alias, path) in [
        ("push-logs", "loki/push"),
        ("query-range", "loki/query"),
        ("live-tail", "loki/tail"),
        ("label-values", "loki/labels"),
        ("rule-groups", "loki/rules"),
    ] {
        assert_eq!(
            protocol_dsl_path("loki", Some(alias)),
            Some(protocol_fixture_path(path))
        );
    }
}

#[test]
fn loki_surface_exposes_rpc_cluster_shelves_and_semantics() {
    let entries = protocol_entries("loki")
        .expect("loki entries should resolve")
        .into_iter()
        .collect::<BTreeSet<_>>();
    assert_eq!(
        entries,
        ["push", "query", "tail", "labels", "rules"]
            .into_iter()
            .map(String::from)
            .collect()
    );

    for (entry, shelf_key) in [
        ("push", "log-ingest"),
        ("query", "log-read"),
        ("tail", "log-read"),
        ("labels", "log-read"),
        ("rules", "ruler"),
    ] {
        let surface = protocol_surface("loki", entry).expect("loki surface should exist");
        assert_eq!(
            surface.cluster_hint.expect("loki cluster should exist").key,
            "web-proxy-request-response"
        );
        assert_eq!(
            surface.shelf.expect("loki shelf should exist").key,
            shelf_key
        );
        assert!(
            surface.entry_semantics.is_some(),
            "loki {entry} should expose log semantics"
        );
    }
}

#[test]
fn loki_stable_subset_dsl_files_compile() {
    for (file, template_id) in [
        ("loki_push_path.gewy", "loki_push_path"),
        ("loki_query_path.gewy", "loki_query_path"),
        ("loki_tail_path.gewy", "loki_tail_path"),
        ("loki_labels_path.gewy", "loki_labels_path"),
        ("loki_rules_path.gewy", "loki_rules_path"),
    ] {
        let binding = compile_file(&dsl_fixture_path(file)).expect("loki DSL should compile");
        assert_eq!(binding.template.id, template_id);
    }
}
