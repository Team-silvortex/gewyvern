mod support;

use gewyvern::dsl::compile_file;
use gewyvern::flow::ProgramOperation;
use gewyvern::protocol_profiles::{
    protocol_default_entry, protocol_dsl_path, protocol_entries, protocol_surface,
};

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
fn dhcpv6_registry_entries_and_aliases_resolve_to_packaged_paths() {
    assert_eq!(
        protocol_dsl_path("dhcpv6", Some("solicit")),
        Some(protocol_fixture_path("dhcpv6/solicit"))
    );
    assert_eq!(
        protocol_dsl_path("dhcp6-solicit", None),
        Some(protocol_fixture_path("dhcpv6/solicit"))
    );
    assert_eq!(
        protocol_dsl_path("dhcpv6", Some("reply")),
        Some(protocol_fixture_path("dhcpv6/request"))
    );
    assert_eq!(
        protocol_dsl_path("dhcpv6-request", None),
        Some(protocol_fixture_path("dhcpv6/request"))
    );
    assert_eq!(
        protocol_dsl_path("dhcpv6", Some("lease-release")),
        Some(protocol_fixture_path("dhcpv6/release"))
    );
    assert_eq!(
        protocol_dsl_path("dhcp6_release", None),
        Some(protocol_fixture_path("dhcpv6/release"))
    );
}

#[test]
fn dhcpv6_surface_exposes_ipv6_lease_shelves_and_semantics() {
    assert_eq!(
        protocol_default_entry("dhcpv6"),
        Some("solicit".to_string())
    );
    let entries = protocol_entries("dhcpv6").expect("dhcpv6 entries should resolve");
    assert_eq!(entries, ["release", "request", "solicit"]);

    for (entry, shelf, category) in [
        ("solicit", "lease", "ipv6-lease-discovery-path"),
        ("request", "lease", "ipv6-lease-request-path"),
        ("release", "lifecycle", "ipv6-lease-release-path"),
    ] {
        let surface = protocol_surface("dhcpv6", entry).expect("dhcpv6 surface should exist");
        assert_eq!(
            surface
                .cluster_hint
                .as_ref()
                .map(|cluster| cluster.key.as_str()),
            Some("network-control-discovery")
        );
        assert_eq!(
            surface.shelf.as_ref().map(|item| item.key.as_str()),
            Some(shelf)
        );
        assert_eq!(
            surface
                .entry_semantics
                .as_ref()
                .map(|item| item.category.as_str()),
            Some(category)
        );
    }
}

#[test]
fn dhcpv6_stable_subset_dsl_files_compile() {
    for (file, template, operation) in [
        (
            "dhcpv6_solicit_path.gewy",
            "dhcpv6_solicit_path",
            "dhcpv6_solicit",
        ),
        (
            "dhcpv6_request_path.gewy",
            "dhcpv6_request_path",
            "dhcpv6_request",
        ),
        (
            "dhcpv6_release_path.gewy",
            "dhcpv6_release_path",
            "dhcpv6_release",
        ),
    ] {
        let binding = compile_file(&dsl_fixture_path(file)).expect("dhcpv6 DSL should compile");
        assert_eq!(binding.template.id, template);
        assert_eq!(
            binding.template.program_model.as_ref().unwrap().operation,
            ProgramOperation::Custom(operation.into())
        );
    }
}
