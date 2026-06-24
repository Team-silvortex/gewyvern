use gewyvern::dsl::compile_file;
use gewyvern::flow::ProgramOperation;
use gewyvern::protocol_profiles::{
    protocol_default_entry, protocol_dsl_path, protocol_entries, protocol_surface,
};

#[test]
fn mdns_registry_entries_resolve_to_discovery_cluster_packages() {
    assert_eq!(
        protocol_dsl_path("mdns", Some("query")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/mdns/query".to_string())
    );
    assert_eq!(
        protocol_dsl_path("mdns", Some("response")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/mdns/response".to_string())
    );
    assert_eq!(
        protocol_dsl_path("mdns", Some("announcement")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/mdns/response".to_string())
    );
    assert_eq!(
        protocol_dsl_path("mdns", Some("probe")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/mdns/probe".to_string())
    );
    assert_eq!(
        protocol_dsl_path("mdns", Some("conflict-check")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/mdns/probe".to_string())
    );
}

#[test]
fn ssdp_registry_entries_resolve_to_discovery_cluster_packages() {
    assert_eq!(
        protocol_dsl_path("ssdp", Some("discovery")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/ssdp/discovery".to_string())
    );
    assert_eq!(
        protocol_dsl_path("ssdp", Some("notify")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/ssdp/notify".to_string())
    );
    assert_eq!(
        protocol_dsl_path("ssdp", Some("advertise")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/ssdp/notify".to_string())
    );
}

#[test]
fn discovery_defaults_stay_stable_while_entries_grow() {
    assert_eq!(protocol_default_entry("mdns"), Some("query".to_string()));
    assert_eq!(
        protocol_default_entry("ssdp"),
        Some("discovery".to_string())
    );

    let mdns = protocol_entries("mdns").expect("mdns entries should resolve");
    assert!(mdns.contains(&"query".to_string()));
    assert!(mdns.contains(&"response".to_string()));
    assert!(mdns.contains(&"probe".to_string()));

    let ssdp = protocol_entries("ssdp").expect("ssdp entries should resolve");
    assert!(ssdp.contains(&"discovery".to_string()));
    assert!(ssdp.contains(&"notify".to_string()));
}

#[test]
fn discovery_surfaces_expose_cluster_shelves_and_semantics() {
    for (protocol, entry, key, category) in [
        ("mdns", "query", "query", "discovery-path"),
        ("mdns", "response", "response", "discovery-path"),
        ("mdns", "probe", "probe", "discovery-path"),
        ("ssdp", "discovery", "discovery", "discovery-path"),
        ("ssdp", "notify", "notify", "discovery-path"),
    ] {
        let surface = protocol_surface(protocol, entry).expect("surface should resolve");
        let shelf = surface.shelf.expect("surface should expose shelf");
        assert_eq!(shelf.key, key);
        assert_eq!(
            surface
                .entry_semantics
                .expect("surface should expose semantics")
                .category,
            category
        );
        assert_eq!(
            surface.cluster_hint.expect("cluster hint should exist").key,
            "network-control-discovery"
        );
    }
}

#[test]
fn discovery_dsl_files_compile_into_expected_operations() {
    for (path, template, operation) in [
        (
            "/Users/Shared/chroot/dev/gewyvern/dsl/mdns_response_path.gewy",
            "mdns_response_path",
            "mdns_response",
        ),
        (
            "/Users/Shared/chroot/dev/gewyvern/dsl/mdns_probe_path.gewy",
            "mdns_probe_path",
            "mdns_probe",
        ),
        (
            "/Users/Shared/chroot/dev/gewyvern/dsl/ssdp_notify_path.gewy",
            "ssdp_notify_path",
            "ssdp_notify",
        ),
    ] {
        let binding = compile_file(path).expect("discovery dsl should compile");
        assert_eq!(binding.template.id, template);
        assert_eq!(
            binding.template.program_model.as_ref().unwrap().operation,
            ProgramOperation::Custom(operation.into())
        );
    }
}
