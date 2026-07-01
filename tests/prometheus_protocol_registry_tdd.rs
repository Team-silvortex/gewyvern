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
fn prometheus_profile_resolves_default_entry_and_aliases() {
    assert_eq!(
        protocol_default_entry("prometheus"),
        Some("scrape".to_string())
    );
    assert_eq!(
        protocol_dsl_path("prom", None),
        Some(protocol_fixture_path("prometheus/scrape"))
    );
    assert_eq!(
        protocol_dsl_path("metrics-scrape", None),
        Some(protocol_fixture_path("prometheus/scrape"))
    );
}

#[test]
fn prometheus_entries_resolve_to_packaged_paths() {
    for (entry, path) in [
        ("scrape", "prometheus/scrape"),
        ("remote-write", "prometheus/remote-write"),
        ("query", "prometheus/query"),
        ("alertmanager", "prometheus/alertmanager"),
        ("rule-eval", "prometheus/rule-eval"),
    ] {
        assert_eq!(
            protocol_dsl_path("prometheus", Some(entry)),
            Some(protocol_fixture_path(path))
        );
    }
}

#[test]
fn prometheus_entry_aliases_resolve_to_metrics_paths() {
    for (alias, path) in [
        ("metrics-endpoint", "prometheus/scrape"),
        ("remote_write", "prometheus/remote-write"),
        ("query-range", "prometheus/query"),
        ("alerts", "prometheus/alertmanager"),
        ("rule_eval", "prometheus/rule-eval"),
    ] {
        assert_eq!(
            protocol_dsl_path("prometheus", Some(alias)),
            Some(protocol_fixture_path(path))
        );
    }
}

#[test]
fn prometheus_surface_exposes_rpc_cluster_shelves_and_semantics() {
    let entries = protocol_entries("prometheus")
        .expect("prometheus entries should resolve")
        .into_iter()
        .collect::<BTreeSet<_>>();
    assert_eq!(
        entries,
        [
            "scrape",
            "remote-write",
            "query",
            "alertmanager",
            "rule-eval",
        ]
        .into_iter()
        .map(String::from)
        .collect()
    );

    for (entry, shelf_key) in [
        ("scrape", "metrics-collection"),
        ("remote-write", "metrics-collection"),
        ("query", "query-api"),
        ("alertmanager", "alerting"),
        ("rule-eval", "alerting"),
    ] {
        let surface =
            protocol_surface("prometheus", entry).expect("prometheus surface should exist");
        assert_eq!(
            surface
                .cluster_hint
                .expect("prometheus cluster should exist")
                .key,
            "web-proxy-request-response"
        );
        assert_eq!(
            surface.shelf.expect("prometheus shelf should exist").key,
            shelf_key
        );
        assert!(
            surface.entry_semantics.is_some(),
            "prometheus {entry} should expose metrics semantics"
        );
    }
}

#[test]
fn prometheus_stable_subset_dsl_files_compile() {
    for (file, template_id) in [
        ("prometheus_scrape_path.gewy", "prometheus_scrape_path"),
        (
            "prometheus_remote_write_path.gewy",
            "prometheus_remote_write_path",
        ),
        ("prometheus_query_path.gewy", "prometheus_query_path"),
        (
            "prometheus_alertmanager_path.gewy",
            "prometheus_alertmanager_path",
        ),
        (
            "prometheus_rule_eval_path.gewy",
            "prometheus_rule_eval_path",
        ),
    ] {
        let binding = compile_file(&dsl_fixture_path(file)).expect("prometheus DSL should compile");
        assert_eq!(binding.template.id, template_id);
    }
}
