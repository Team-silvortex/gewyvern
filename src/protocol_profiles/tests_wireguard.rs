use super::summary::built_in_protocol_summary;
use super::{protocol_dsl_path, protocol_surface};

#[test]
fn wireguard_surfaces_split_handshake_cookie_and_transport_shelves() {
    let handshake =
        protocol_surface("wireguard", "handshake").expect("wireguard handshake should exist");
    let handshake_shelf = handshake
        .shelf
        .expect("wireguard handshake shelf should exist");
    assert_eq!(handshake_shelf.key, "handshake");
    assert_eq!(
        handshake_shelf.page,
        "docs/book/reference-wireguard-handshake-surface.md"
    );

    let cookie = protocol_surface("wireguard", "cookie").expect("wireguard cookie should exist");
    let cookie_shelf = cookie.shelf.expect("wireguard cookie shelf should exist");
    assert_eq!(cookie_shelf.key, "cookie");
    assert_eq!(
        cookie_shelf.page,
        "docs/book/reference-wireguard-cookie-surface.md"
    );

    let transport =
        protocol_surface("wireguard", "transport").expect("wireguard transport should exist");
    let transport_shelf = transport
        .shelf
        .expect("wireguard transport shelf should exist");
    assert_eq!(transport_shelf.key, "transport");
    assert_eq!(
        transport_shelf.page,
        "docs/book/reference-wireguard-transport-surface.md"
    );
}

#[test]
fn wireguard_aliases_resolve_to_canonical_entries() {
    assert_eq!(
        protocol_dsl_path("wireguard", Some("cookie-reply")),
        Some(super::protocol_fixture_path("wireguard/cookie"))
    );
    assert_eq!(
        protocol_dsl_path("wireguard", Some("data")),
        Some(super::protocol_fixture_path("wireguard/transport"))
    );
}

#[test]
fn wireguard_summary_and_semantics_expose_new_entries() {
    let summary = built_in_protocol_summary("wireguard").expect("wireguard summary should exist");
    let cookie = summary
        .entries
        .iter()
        .find(|entry| entry.mode == "cookie")
        .expect("wireguard cookie summary should exist");
    assert!(cookie.aliases.contains(&"cookie-reply".to_string()));
    assert!(cookie.aliases.contains(&"wireguard-cookie".to_string()));

    let transport = summary
        .entries
        .iter()
        .find(|entry| entry.mode == "transport")
        .expect("wireguard transport summary should exist");
    assert!(transport.aliases.contains(&"data".to_string()));
    assert!(transport.aliases.contains(&"wireguard-data".to_string()));

    let semantics = protocol_surface("wireguard", "cookie")
        .expect("wireguard cookie should exist")
        .entry_semantics
        .expect("wireguard cookie should expose semantics");
    assert_eq!(semantics.category, "continuation-path");
    assert_eq!(
        semantics.operator_focus,
        "peer anti-abuse continuation during WireGuard cookie reply evaluation"
    );
    assert_eq!(semantics.typical_signal.as_deref(), Some("Cookie Reply"));
}
