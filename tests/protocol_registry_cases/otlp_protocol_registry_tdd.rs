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
fn otlp_profile_resolves_default_entry_and_aliases() {
    assert_eq!(protocol_default_entry("otlp"), Some("traces".to_string()));
    assert_eq!(
        protocol_dsl_path("opentelemetry", None),
        Some(protocol_fixture_path("otlp/traces"))
    );
    assert_eq!(
        protocol_dsl_path("otel", None),
        Some(protocol_fixture_path("otlp/traces"))
    );
}

#[test]
fn otlp_entries_resolve_to_packaged_paths() {
    for (entry, path) in [
        ("traces", "otlp/traces"),
        ("metrics", "otlp/metrics"),
        ("logs", "otlp/logs"),
        ("partial-success", "otlp/partial-success"),
        ("export-error", "otlp/export-error"),
    ] {
        assert_eq!(
            protocol_dsl_path("otlp", Some(entry)),
            Some(protocol_fixture_path(path))
        );
    }
}

#[test]
fn otlp_entry_aliases_resolve_to_telemetry_paths() {
    for (alias, path) in [
        ("trace-export", "otlp/traces"),
        ("metric-export", "otlp/metrics"),
        ("log-export", "otlp/logs"),
        ("partial", "otlp/partial-success"),
        ("collector-error", "otlp/export-error"),
    ] {
        assert_eq!(
            protocol_dsl_path("otlp", Some(alias)),
            Some(protocol_fixture_path(path))
        );
    }
}

#[test]
fn otlp_surface_exposes_rpc_cluster_shelves_and_semantics() {
    let entries = protocol_entries("otlp")
        .expect("otlp entries should resolve")
        .into_iter()
        .collect::<BTreeSet<_>>();
    assert_eq!(
        entries,
        [
            "traces",
            "metrics",
            "logs",
            "partial-success",
            "export-error",
        ]
        .into_iter()
        .map(String::from)
        .collect()
    );

    for (entry, shelf_key) in [
        ("traces", "signal-export"),
        ("metrics", "signal-export"),
        ("logs", "signal-export"),
        ("partial-success", "collector-response"),
        ("export-error", "collector-response"),
    ] {
        let surface = protocol_surface("otlp", entry).expect("otlp surface should exist");
        assert_eq!(
            surface.cluster_hint.expect("otlp cluster should exist").key,
            "web-proxy-request-response"
        );
        assert_eq!(
            surface.shelf.expect("otlp shelf should exist").key,
            shelf_key
        );
        assert!(
            surface.entry_semantics.is_some(),
            "otlp {entry} should expose telemetry semantics"
        );
    }
}

#[test]
fn otlp_stable_subset_dsl_files_compile() {
    for (file, template_id) in [
        ("otlp_traces_path.gewy", "otlp_traces_path"),
        ("otlp_metrics_path.gewy", "otlp_metrics_path"),
        ("otlp_logs_path.gewy", "otlp_logs_path"),
        (
            "otlp_partial_success_path.gewy",
            "otlp_partial_success_path",
        ),
        ("otlp_export_error_path.gewy", "otlp_export_error_path"),
    ] {
        let binding = compile_file(&dsl_fixture_path(file)).expect("otlp DSL should compile");
        assert_eq!(binding.template.id, template_id);
    }
}
