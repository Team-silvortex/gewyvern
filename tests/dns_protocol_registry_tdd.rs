use gewyvern::dsl::compile_file;
use gewyvern::flow::ProgramOperation;
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
fn dns_registry_entries_resolve_to_transport_and_error_packages() {
    assert_eq!(protocol_default_entry("dns"), Some("udp".to_string()));
    assert_eq!(
        protocol_dsl_path("dns", None),
        Some(protocol_fixture_path("dns/udp"))
    );
    assert_eq!(
        protocol_dsl_path("dns", Some("dns-tcp")),
        Some(protocol_fixture_path("dns/tcp"))
    );
    assert_eq!(
        protocol_dsl_path("dns", Some("nxdomain")),
        Some(protocol_fixture_path("dns/error"))
    );
    assert_eq!(
        protocol_dsl_path("dns", Some("resolution-failed")),
        Some(protocol_fixture_path("dns/error"))
    );
    assert_eq!(
        protocol_dsl_path("dns", Some("tcp-nxdomain")),
        Some(protocol_fixture_path("dns/tcp-error"))
    );
}

#[test]
fn dns_surface_exposes_error_shelf_and_semantics() {
    let entries = protocol_entries("dns")
        .expect("dns entries should resolve")
        .into_iter()
        .collect::<BTreeSet<_>>();
    assert_eq!(
        entries,
        ["udp", "tcp", "error", "tcp-error"]
            .into_iter()
            .map(String::from)
            .collect()
    );

    for (entry, shelf_key, category) in [
        ("udp", "udp", "name-resolution-path"),
        ("tcp", "tcp", "name-resolution-path"),
        ("error", "error", "name-resolution-error"),
        ("tcp-error", "error", "name-resolution-error"),
    ] {
        let surface = protocol_surface("dns", entry).expect("dns surface should exist");
        assert_eq!(
            surface.shelf.expect("dns shelf should exist").key,
            shelf_key
        );
        assert_eq!(
            surface
                .entry_semantics
                .expect("dns semantics should exist")
                .category,
            category
        );
    }
}

#[test]
fn dns_stable_subset_dsl_files_compile() {
    for (file, template, operation) in [
        ("dns_udp_process.gewy", "dns_udp_process", "dns_lookup"),
        (
            "dns_tcp_query_path.gewy",
            "dns_tcp_query_path",
            "dns_tcp_query",
        ),
        ("dns_error_path.gewy", "dns_error_path", "dns_error"),
        (
            "dns_tcp_error_path.gewy",
            "dns_tcp_error_path",
            "dns_tcp_error",
        ),
    ] {
        let binding = compile_file(&dsl_fixture_path(file)).expect("dns dsl should compile");
        assert_eq!(binding.template.id, template);
        assert_eq!(
            binding.template.program_model.as_ref().unwrap().operation,
            ProgramOperation::Custom(operation.into())
        );
    }
}
