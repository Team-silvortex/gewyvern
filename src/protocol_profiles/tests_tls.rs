use super::summary::built_in_protocol_summary;
use super::{protocol_dsl_path, protocol_surface};

#[test]
fn tls_surfaces_split_client_and_server_shelves() {
    let client = protocol_surface("tls", "client").expect("tls client should exist");
    let client_shelf = client.shelf.expect("tls client shelf should exist");
    assert_eq!(client_shelf.key, "client");
    assert_eq!(client_shelf.page, "docs/book/reference-tls-client-surface.md");

    let server = protocol_surface("tls", "server").expect("tls server should exist");
    let server_shelf = server.shelf.expect("tls server shelf should exist");
    assert_eq!(server_shelf.key, "server");
    assert_eq!(server_shelf.page, "docs/book/reference-tls-server-surface.md");
}

#[test]
fn tls_aliases_resolve_to_canonical_entries() {
    assert_eq!(
        protocol_dsl_path("tls", Some("initiator")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/tls/client".to_string())
    );
    assert_eq!(
        protocol_dsl_path("tls", Some("tls_client")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/tls/client".to_string())
    );
    assert_eq!(
        protocol_dsl_path("tls", Some("acceptor")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/tls/server".to_string())
    );
    assert_eq!(
        protocol_dsl_path("tls", Some("tls_server")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/tls/server".to_string())
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
}
