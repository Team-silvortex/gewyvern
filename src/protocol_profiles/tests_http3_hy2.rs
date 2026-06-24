use super::summary::built_in_protocol_summary;
use super::{protocol_dsl_path, protocol_surface};

#[test]
fn http3_and_hy2_close_surfaces_expose_failure_shelves() {
    let http3_close = protocol_surface("http3", "close").expect("http3 close should exist");
    let http3_shelf = http3_close.shelf.expect("http3 close shelf should exist");
    assert_eq!(http3_shelf.key, "close");
    assert_eq!(
        http3_shelf.page,
        "docs/book/reference-http3-close-surface.md"
    );

    let http3_server_close =
        protocol_surface("http3", "server-close").expect("http3 server-close should exist");
    let http3_server_close_shelf = http3_server_close
        .shelf
        .expect("http3 server-close shelf should exist");
    assert_eq!(http3_server_close_shelf.key, "server-close");
    assert_eq!(
        http3_server_close_shelf.page,
        "docs/book/reference-http3-server-close-surface.md"
    );

    let hy2_close = protocol_surface("hy2", "close").expect("hy2 close should exist");
    let hy2_shelf = hy2_close.shelf.expect("hy2 close shelf should exist");
    assert_eq!(hy2_shelf.key, "close");
    assert_eq!(hy2_shelf.page, "docs/book/reference-hy2-close-surface.md");

    let hy2_tcp_close = protocol_surface("hy2", "tcp-close").expect("hy2 tcp-close should exist");
    let hy2_tcp_close_shelf = hy2_tcp_close
        .shelf
        .expect("hy2 tcp-close shelf should exist");
    assert_eq!(hy2_tcp_close_shelf.key, "tcp-close");
    assert_eq!(
        hy2_tcp_close_shelf.page,
        "docs/book/reference-hy2-tcp-close-surface.md"
    );

    let hy2_udp_close = protocol_surface("hy2", "udp-close").expect("hy2 udp-close should exist");
    let hy2_udp_close_shelf = hy2_udp_close
        .shelf
        .expect("hy2 udp-close shelf should exist");
    assert_eq!(hy2_udp_close_shelf.key, "udp-close");
    assert_eq!(
        hy2_udp_close_shelf.page,
        "docs/book/reference-hy2-udp-close-surface.md"
    );
}

#[test]
fn http3_and_hy2_close_aliases_resolve_to_canonical_entries() {
    assert_eq!(
        protocol_dsl_path("http3", Some("h3-close")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/http3/close".to_string())
    );
    assert_eq!(
        protocol_dsl_path("http3", Some("h3-server-close")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/http3/server-close".to_string())
    );
    assert_eq!(
        protocol_dsl_path("hy2", Some("session-close")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/hy2/close".to_string())
    );
    assert_eq!(
        protocol_dsl_path("hy2", Some("hy2-tcp-close")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/hy2/tcp-close".to_string())
    );
    assert_eq!(
        protocol_dsl_path("hy2", Some("hy2-udp-close")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/hy2/udp-close".to_string())
    );
}

#[test]
fn http3_and_hy2_close_summary_and_semantics_stay_exposed() {
    let http3_summary = built_in_protocol_summary("http3").expect("http3 summary should exist");
    let http3_close = http3_summary
        .entries
        .iter()
        .find(|entry| entry.mode == "close")
        .expect("http3 close summary should exist");
    assert!(http3_close.aliases.contains(&"terminate".to_string()));
    assert!(http3_close.aliases.contains(&"h3-close".to_string()));

    let http3_semantics = protocol_surface("http3", "close")
        .expect("http3 close should exist")
        .entry_semantics
        .expect("http3 close should expose semantics");
    assert_eq!(http3_semantics.category, "failure-path");
    assert_eq!(
        http3_semantics.typical_signal.as_deref(),
        Some("CONNECTION_CLOSE")
    );
    assert_eq!(
        http3_semantics.primary_failure_mode.as_deref(),
        Some("peer_closed")
    );

    let http3_server_close = http3_summary
        .entries
        .iter()
        .find(|entry| entry.mode == "server-close")
        .expect("http3 server-close summary should exist");
    assert!(
        http3_server_close
            .aliases
            .contains(&"h3-server-close".to_string())
    );
    assert!(
        http3_server_close
            .aliases
            .contains(&"response-close".to_string())
    );

    let http3_server_close_semantics = protocol_surface("http3", "server-close")
        .expect("http3 server-close should exist")
        .entry_semantics
        .expect("http3 server-close should expose semantics");
    assert_eq!(http3_server_close_semantics.category, "failure-path");
    assert_eq!(
        http3_server_close_semantics.typical_signal.as_deref(),
        Some("CONNECTION_CLOSE")
    );
    assert_eq!(
        http3_server_close_semantics.primary_failure_mode.as_deref(),
        Some("local_closed")
    );

    let hy2_summary = built_in_protocol_summary("hy2").expect("hy2 summary should exist");
    let hy2_close = hy2_summary
        .entries
        .iter()
        .find(|entry| entry.mode == "close")
        .expect("hy2 close summary should exist");
    assert!(hy2_close.aliases.contains(&"session-close".to_string()));
    assert!(hy2_close.aliases.contains(&"hy2-close".to_string()));

    let hy2_semantics = protocol_surface("hy2", "close")
        .expect("hy2 close should exist")
        .entry_semantics
        .expect("hy2 close should expose semantics");
    assert_eq!(hy2_semantics.category, "failure-path");
    assert_eq!(
        hy2_semantics.typical_signal.as_deref(),
        Some("CONNECTION_CLOSE")
    );
    assert_eq!(
        hy2_semantics.primary_failure_detail.as_deref(),
        Some("secure_session_terminated")
    );

    let hy2_tcp_close = hy2_summary
        .entries
        .iter()
        .find(|entry| entry.mode == "tcp-close")
        .expect("hy2 tcp-close summary should exist");
    assert!(hy2_tcp_close.aliases.contains(&"hy2-tcp-close".to_string()));
    assert!(hy2_tcp_close.aliases.contains(&"stream-close".to_string()));

    let hy2_tcp_close_semantics = protocol_surface("hy2", "tcp-close")
        .expect("hy2 tcp-close should exist")
        .entry_semantics
        .expect("hy2 tcp-close should expose semantics");
    assert_eq!(hy2_tcp_close_semantics.category, "failure-path");
    assert_eq!(
        hy2_tcp_close_semantics.typical_signal.as_deref(),
        Some("CONNECTION_CLOSE")
    );
    assert_eq!(
        hy2_tcp_close_semantics.primary_failure_detail.as_deref(),
        Some("tcp_relay_terminated")
    );

    let hy2_udp_close = hy2_summary
        .entries
        .iter()
        .find(|entry| entry.mode == "udp-close")
        .expect("hy2 udp-close summary should exist");
    assert!(hy2_udp_close.aliases.contains(&"hy2-udp-close".to_string()));
    assert!(
        hy2_udp_close
            .aliases
            .contains(&"datagram-close".to_string())
    );

    let hy2_udp_close_semantics = protocol_surface("hy2", "udp-close")
        .expect("hy2 udp-close should exist")
        .entry_semantics
        .expect("hy2 udp-close should expose semantics");
    assert_eq!(hy2_udp_close_semantics.category, "failure-path");
    assert_eq!(
        hy2_udp_close_semantics.typical_signal.as_deref(),
        Some("CONNECTION_CLOSE")
    );
    assert_eq!(
        hy2_udp_close_semantics.primary_failure_detail.as_deref(),
        Some("udp_relay_terminated")
    );
}
