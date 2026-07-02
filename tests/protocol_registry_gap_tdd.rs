use gewyvern::protocol_profiles::{
    protocol_default_entry, protocol_dsl_path, protocol_entries, protocol_surface,
};

fn protocol_fixture_path(relative: &str) -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("protocols")
        .join(relative)
        .to_string_lossy()
        .into_owned()
}
#[test]
fn http3_hy2_and_transport_registry_entries_resolve_to_packaged_paths() {
    assert_eq!(
        protocol_dsl_path("http3", Some("server")),
        Some(protocol_fixture_path("http3/server").to_string())
    );
    assert_eq!(
        protocol_dsl_path("http3", Some("h3-server")),
        Some(protocol_fixture_path("http3/server").to_string())
    );
    assert_eq!(
        protocol_dsl_path("http3", Some("h3-close")),
        Some(protocol_fixture_path("http3/close").to_string())
    );
    assert_eq!(
        protocol_dsl_path("http3", Some("h3-server-close")),
        Some(protocol_fixture_path("http3/server-close").to_string())
    );
    assert_eq!(
        protocol_dsl_path("hy2", Some("hy2-stream")),
        Some(protocol_fixture_path("hy2/tcp").to_string())
    );
    assert_eq!(
        protocol_dsl_path("hy2", Some("hy2-relay")),
        Some(protocol_fixture_path("hy2/udp").to_string())
    );
    assert_eq!(
        protocol_dsl_path("hy2", Some("session-close")),
        Some(protocol_fixture_path("hy2/close").to_string())
    );
    assert_eq!(
        protocol_dsl_path("hy2", Some("hy2-tcp-close")),
        Some(protocol_fixture_path("hy2/tcp-close").to_string())
    );
    assert_eq!(
        protocol_dsl_path("hy2", Some("hy2-udp-close")),
        Some(protocol_fixture_path("hy2/udp-close").to_string())
    );
    assert_eq!(
        protocol_dsl_path("tls", Some("client")),
        Some(protocol_fixture_path("tls/client").to_string())
    );
    assert_eq!(
        protocol_dsl_path("tls", Some("tls-server")),
        Some(protocol_fixture_path("tls/server").to_string())
    );
    assert_eq!(
        protocol_dsl_path("wireguard", Some("handshake")),
        Some(protocol_fixture_path("wireguard/handshake").to_string())
    );
}

#[test]
fn socks5_smtp_kerberos_and_control_plane_aliases_resolve_to_packaged_paths() {
    assert_eq!(
        protocol_dsl_path("socks5", Some("proxy")),
        Some(protocol_fixture_path("socks5/session").to_string())
    );
    assert_eq!(
        protocol_dsl_path("socks5", Some("userpass-connect-denied")),
        Some(protocol_fixture_path("socks5/auth-connect-denied").to_string())
    );
    assert_eq!(
        protocol_dsl_path("smtp", Some("message-denied")),
        Some(protocol_fixture_path("smtp/data-denied").to_string())
    );
    assert_eq!(
        protocol_dsl_path("kerberos", Some("service-ticket")),
        Some(protocol_fixture_path("kerberos/tgs").to_string())
    );
    assert_eq!(
        protocol_dsl_path("radius", Some("auth")),
        Some(protocol_fixture_path("radius/access").to_string())
    );
    assert_eq!(
        protocol_dsl_path("gtp-u", None),
        Some(protocol_fixture_path("gtpu/echo").to_string())
    );
}

#[test]
fn gap_protocol_default_entries_and_surface_shelves_stay_stable() {
    assert_eq!(protocol_default_entry("http3"), Some("request".to_string()));
    assert_eq!(protocol_default_entry("hy2"), Some("auth".to_string()));
    assert_eq!(
        protocol_default_entry("socks5"),
        Some("session".to_string())
    );
    assert_eq!(protocol_default_entry("smtp"), Some("session".to_string()));
    assert_eq!(protocol_default_entry("kerberos"), Some("as".to_string()));
    assert_eq!(protocol_default_entry("radius"), Some("access".to_string()));
    assert_eq!(protocol_default_entry("gtpu"), Some("echo".to_string()));

    let http3 = protocol_surface("http3", "server").expect("http3 server surface should exist");
    assert_eq!(http3.shelf.expect("http3 shelf should exist").key, "server");
    let http3_close = protocol_surface("http3", "close").expect("http3 close surface should exist");
    assert_eq!(
        http3_close
            .shelf
            .expect("http3 close shelf should exist")
            .key,
        "close"
    );
    let http3_server_close =
        protocol_surface("http3", "server-close").expect("http3 server-close surface should exist");
    assert_eq!(
        http3_server_close
            .shelf
            .expect("http3 server-close shelf should exist")
            .key,
        "server-close"
    );

    let hy2 = protocol_surface("hy2", "tcp").expect("hy2 tcp surface should exist");
    assert_eq!(hy2.shelf.expect("hy2 shelf should exist").key, "relay");
    let hy2_close = protocol_surface("hy2", "close").expect("hy2 close surface should exist");
    assert_eq!(
        hy2_close.shelf.expect("hy2 close shelf should exist").key,
        "close"
    );
    let hy2_tcp_close =
        protocol_surface("hy2", "tcp-close").expect("hy2 tcp-close surface should exist");
    assert_eq!(
        hy2_tcp_close
            .shelf
            .expect("hy2 tcp-close shelf should exist")
            .key,
        "tcp-close"
    );
    let hy2_udp_close =
        protocol_surface("hy2", "udp-close").expect("hy2 udp-close surface should exist");
    assert_eq!(
        hy2_udp_close
            .shelf
            .expect("hy2 udp-close shelf should exist")
            .key,
        "udp-close"
    );

    let socks5 =
        protocol_surface("socks5", "auth-connect-denied").expect("socks5 shelf should exist");
    assert_eq!(
        socks5.shelf.expect("socks5 shelf should exist").key,
        "denied"
    );

    let smtp = protocol_surface("smtp", "data-denied").expect("smtp shelf should exist");
    assert_eq!(smtp.shelf.expect("smtp shelf should exist").key, "data");

    let kerberos = protocol_surface("kerberos", "as-error").expect("kerberos shelf should exist");
    assert_eq!(
        kerberos.shelf.expect("kerberos shelf should exist").key,
        "as"
    );

    let tls = protocol_surface("tls", "server").expect("tls server surface should exist");
    assert_eq!(tls.shelf.expect("tls shelf should exist").key, "server");

    let quic_local_close =
        protocol_surface("quic", "local-close").expect("quic local-close surface should exist");
    assert_eq!(
        quic_local_close
            .shelf
            .expect("quic local-close shelf should exist")
            .key,
        "local-close"
    );

    let gtpu = protocol_surface("gtpu", "echo").expect("gtpu echo surface should exist");
    let gtpu_shelf = gtpu.shelf.expect("gtpu shelf should exist");
    assert_eq!(gtpu_shelf.key, "liveness");
    assert_eq!(
        gtpu_shelf.page,
        "docs/book/reference-gtpu-liveness-surface.md"
    );
    assert_eq!(
        gtpu.entry_semantics
            .expect("gtpu echo semantics should exist")
            .category,
        "tunnel-liveness-path"
    );
}

#[test]
fn gap_protocol_entries_remain_visible_in_family_summaries() {
    let http3 = protocol_entries("http3").expect("http3 entries should resolve");
    assert!(http3.contains(&"request".to_string()));
    assert!(http3.contains(&"server".to_string()));
    assert!(http3.contains(&"close".to_string()));
    assert!(http3.contains(&"server-close".to_string()));

    let hy2 = protocol_entries("hy2").expect("hy2 entries should resolve");
    assert!(hy2.contains(&"auth".to_string()));
    assert!(hy2.contains(&"tcp".to_string()));
    assert!(hy2.contains(&"udp".to_string()));
    assert!(hy2.contains(&"close".to_string()));
    assert!(hy2.contains(&"tcp-close".to_string()));
    assert!(hy2.contains(&"udp-close".to_string()));

    let socks5 = protocol_entries("socks5").expect("socks5 entries should resolve");
    assert!(socks5.contains(&"session".to_string()));
    assert!(socks5.contains(&"auth".to_string()));
    assert!(socks5.contains(&"auth-denied".to_string()));
    assert!(socks5.contains(&"auth-connect-denied".to_string()));
    assert!(socks5.contains(&"denied".to_string()));

    let smtp = protocol_entries("smtp").expect("smtp entries should resolve");
    assert!(smtp.contains(&"session".to_string()));
    assert!(smtp.contains(&"auth".to_string()));
    assert!(smtp.contains(&"mail".to_string()));
    assert!(smtp.contains(&"rcpt".to_string()));
    assert!(smtp.contains(&"rcpt-denied".to_string()));
    assert!(smtp.contains(&"data".to_string()));
    assert!(smtp.contains(&"data-denied".to_string()));

    let tls = protocol_entries("tls").expect("tls entries should resolve");
    assert!(tls.contains(&"client".to_string()));
    assert!(tls.contains(&"server".to_string()));

    let quic = protocol_entries("quic").expect("quic entries should resolve");
    assert!(quic.contains(&"initial".to_string()));
    assert!(quic.contains(&"retry".to_string()));
    assert!(quic.contains(&"crypto".to_string()));
    assert!(quic.contains(&"close".to_string()));
    assert!(quic.contains(&"local-close".to_string()));
}
