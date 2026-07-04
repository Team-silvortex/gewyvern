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
fn syslog_registry_entries_and_aliases_resolve_to_packaged_paths() {
    assert_eq!(
        protocol_dsl_path("syslog", Some("udp")),
        Some(protocol_fixture_path("syslog/udp"))
    );
    assert_eq!(
        protocol_dsl_path("syslog-udp", None),
        Some(protocol_fixture_path("syslog/udp"))
    );
    assert_eq!(
        protocol_dsl_path("syslog", Some("octet-counted")),
        Some(protocol_fixture_path("syslog/tcp"))
    );
    assert_eq!(
        protocol_dsl_path("syslog-tcp", None),
        Some(protocol_fixture_path("syslog/tcp"))
    );
    assert_eq!(
        protocol_dsl_path("syslog", Some("rfc5425")),
        Some(protocol_fixture_path("syslog/tls"))
    );
    assert_eq!(
        protocol_dsl_path("syslog-secure", None),
        Some(protocol_fixture_path("syslog/tls"))
    );
}

#[test]
fn syslog_surface_exposes_log_shelves_and_semantics() {
    assert_eq!(protocol_default_entry("syslog"), Some("udp".to_string()));
    let entries = protocol_entries("syslog")
        .expect("syslog entries should resolve")
        .into_iter()
        .collect::<BTreeSet<_>>();
    assert_eq!(
        entries,
        ["udp", "tcp", "tls"]
            .into_iter()
            .map(String::from)
            .collect()
    );

    for (entry, shelf, semantics) in [
        ("udp", "log-ingest", "syslog-udp-message-path"),
        ("tcp", "log-ingest", "syslog-tcp-message-path"),
        ("tls", "secure-transport", "syslog-tls-transport-path"),
    ] {
        let surface = protocol_surface("syslog", entry).expect("syslog surface should exist");
        assert_eq!(
            surface
                .cluster_hint
                .expect("syslog cluster should exist")
                .key,
            "web-proxy-request-response"
        );
        assert_eq!(surface.shelf.expect("syslog shelf should exist").key, shelf);
        assert_eq!(
            surface
                .entry_semantics
                .expect("syslog semantics should exist")
                .category,
            semantics
        );
    }
}

#[test]
fn syslog_stable_subset_dsl_files_compile() {
    for (file, template_id, operation) in [
        (
            "syslog_udp_message_path.gewy",
            "syslog_udp_message_path",
            "syslog_udp_message",
        ),
        (
            "syslog_tcp_message_path.gewy",
            "syslog_tcp_message_path",
            "syslog_tcp_message",
        ),
        (
            "syslog_tls_transport_path.gewy",
            "syslog_tls_transport_path",
            "syslog_tls_transport",
        ),
    ] {
        let binding = compile_file(&dsl_fixture_path(file)).expect("syslog DSL should compile");
        assert_eq!(binding.template.id, template_id);
        assert_eq!(
            binding.template.program_model.as_ref().unwrap().operation,
            ProgramOperation::Custom(operation.into())
        );
    }
}
