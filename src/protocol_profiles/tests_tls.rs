use super::summary::built_in_protocol_summary;
use super::{protocol_dsl_path, protocol_entries, protocol_surface};
use crate::dsl::compile_file;
use std::collections::BTreeSet;
use std::path::Path;

fn dsl_fixture_path(name: &str) -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("dsl")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

#[test]
fn tls_surfaces_split_client_and_server_shelves() {
    let client = protocol_surface("tls", "client").expect("tls client should exist");
    let client_shelf = client.shelf.expect("tls client shelf should exist");
    assert_eq!(client_shelf.key, "client");
    assert_eq!(
        client_shelf.page,
        "docs/book/reference-tls-client-surface.md"
    );

    let server = protocol_surface("tls", "server").expect("tls server should exist");
    let server_shelf = server.shelf.expect("tls server shelf should exist");
    assert_eq!(server_shelf.key, "server");
    assert_eq!(
        server_shelf.page,
        "docs/book/reference-tls-server-surface.md"
    );

    for entry in ["alert", "certificate"] {
        let surface = protocol_surface("tls", entry).expect("tls signal should exist");
        let shelf = surface.shelf.expect("tls signal shelf should exist");
        assert_eq!(shelf.key, "handshake-signal");
        assert_eq!(shelf.page, "docs/book/reference-tls-signal-surface.md");
        assert!(
            surface.entry_semantics.is_some(),
            "tls {entry} should expose debugger semantics"
        );
    }
}

#[test]
fn tls_aliases_resolve_to_canonical_entries() {
    assert_eq!(
        protocol_dsl_path("tls", Some("initiator")),
        Some(super::protocol_fixture_path("tls/client"))
    );
    assert_eq!(
        protocol_dsl_path("tls", Some("tls_client")),
        Some(super::protocol_fixture_path("tls/client"))
    );
    assert_eq!(
        protocol_dsl_path("tls", Some("acceptor")),
        Some(super::protocol_fixture_path("tls/server"))
    );
    assert_eq!(
        protocol_dsl_path("tls", Some("tls_server")),
        Some(super::protocol_fixture_path("tls/server"))
    );
    assert_eq!(
        protocol_dsl_path("tls", Some("alert-record")),
        Some(super::protocol_fixture_path("tls/alert"))
    );
    assert_eq!(
        protocol_dsl_path("tls", Some("x509-chain")),
        Some(super::protocol_fixture_path("tls/certificate"))
    );
}

#[test]
fn tls_summary_exposes_client_and_server_entries_and_aliases() {
    let summary = built_in_protocol_summary("tls").expect("tls summary should exist");
    let client = summary
        .entries
        .iter()
        .find(|entry| entry.mode == "client")
        .expect("tls client summary should exist");
    assert!(client.aliases.contains(&"initiator".to_string()));
    assert!(client.aliases.contains(&"tls-client".to_string()));
    assert!(client.aliases.contains(&"tls_client".to_string()));

    let server = summary
        .entries
        .iter()
        .find(|entry| entry.mode == "server")
        .expect("tls server summary should exist");
    assert!(server.aliases.contains(&"acceptor".to_string()));
    assert!(server.aliases.contains(&"tls-server".to_string()));
    assert!(server.aliases.contains(&"tls_server".to_string()));

    let entries = summary
        .entries
        .iter()
        .map(|entry| entry.mode.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        entries,
        ["alert", "certificate", "client", "server"]
            .into_iter()
            .collect()
    );

    let alert = summary
        .entries
        .iter()
        .find(|entry| entry.mode == "alert")
        .expect("tls alert summary should exist");
    assert!(alert.aliases.contains(&"alert-record".to_string()));

    let certificate = summary
        .entries
        .iter()
        .find(|entry| entry.mode == "certificate")
        .expect("tls certificate summary should exist");
    assert!(certificate.aliases.contains(&"x509".to_string()));
}

#[test]
fn tls_stable_subset_dsl_files_compile() {
    for file in [
        "tls_client_path.gewy",
        "tls_server_path.gewy",
        "tls_alert_path.gewy",
        "tls_certificate_path.gewy",
    ] {
        let binding = compile_file(&dsl_fixture_path(file)).expect("tls dsl should compile");
        assert!(
            binding.template.id.starts_with("tls_"),
            "tls template id should be protocol-specific"
        );
    }
}

#[test]
fn tls_registry_entries_are_complete() {
    let entries = protocol_entries("tls")
        .expect("tls entries should resolve")
        .into_iter()
        .collect::<BTreeSet<_>>();
    assert_eq!(
        entries,
        ["client", "server", "alert", "certificate"]
            .into_iter()
            .map(String::from)
            .collect()
    );
}
