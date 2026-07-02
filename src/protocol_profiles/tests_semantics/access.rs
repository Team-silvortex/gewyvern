use super::super::protocol_surface;

#[test]
fn socks5_denial_surfaces_expose_machine_readable_entry_semantics() {
    let denied = protocol_surface("socks5", "denied").expect("socks5 denied surface should exist");
    let denied_semantics = denied
        .entry_semantics
        .expect("socks5 denied should expose semantics");
    assert_eq!(denied_semantics.category, "failure-path");
    assert_eq!(
        denied_semantics.operator_focus,
        "upstream connect refusal after no-auth method selection"
    );
    assert!(denied_semantics.typical_signal.is_none());
    assert_eq!(
        denied_semantics.primary_failure_mode.as_deref(),
        Some("server_denied")
    );
    assert_eq!(
        denied_semantics.primary_failure_detail.as_deref(),
        Some("access_denied")
    );
    assert_eq!(
        denied_semantics.primary_failure_basis.as_deref(),
        Some("direct_protocol_signal")
    );

    let auth_denied =
        protocol_surface("socks5", "auth-denied").expect("socks5 auth-denied should exist");
    let auth_denied_semantics = auth_denied
        .entry_semantics
        .expect("socks5 auth-denied should expose semantics");
    assert_eq!(
        auth_denied_semantics.operator_focus,
        "username/password rejection during proxy auth exchange"
    );

    let auth_connect_denied = protocol_surface("socks5", "auth-connect-denied")
        .expect("socks5 auth-connect-denied should exist");
    let auth_connect_denied_semantics = auth_connect_denied
        .entry_semantics
        .expect("socks5 auth-connect-denied should expose semantics");
    assert_eq!(
        auth_connect_denied_semantics.operator_focus,
        "upstream connect refusal after authenticated proxy setup"
    );

    let auth = protocol_surface("socks5", "auth").expect("socks5 auth surface should exist");
    let auth_semantics = auth
        .entry_semantics
        .expect("socks5 auth should expose positive connect semantics");
    assert_eq!(auth_semantics.category, "socks5-auth-connect-path");
}

#[test]
fn http_and_ldap_denial_surfaces_expose_machine_readable_entry_semantics() {
    let http_denied = protocol_surface("http", "denied").expect("http denied surface should exist");
    let http_semantics = http_denied
        .entry_semantics
        .expect("http denied should expose semantics");
    assert_eq!(http_semantics.category, "failure-path");
    assert_eq!(
        http_semantics.operator_focus,
        "proxy tunnel refusal after CONNECT policy evaluation"
    );
    assert_eq!(http_semantics.typical_signal.as_deref(), Some("403"));
    assert_eq!(
        http_semantics.primary_failure_mode.as_deref(),
        Some("server_denied")
    );
    assert_eq!(
        http_semantics.primary_failure_detail.as_deref(),
        Some("access_denied")
    );
    assert_eq!(
        http_semantics.primary_failure_basis.as_deref(),
        Some("direct_protocol_signal")
    );

    let ldap_denied =
        protocol_surface("ldap", "bind-denied").expect("ldap bind-denied surface should exist");
    let ldap_semantics = ldap_denied
        .entry_semantics
        .expect("ldap bind-denied should expose semantics");
    assert_eq!(ldap_semantics.category, "failure-path");
    assert_eq!(
        ldap_semantics.operator_focus,
        "directory credential or bind-policy rejection during auth establishment"
    );
    assert!(ldap_semantics.typical_signal.is_none());
    assert_eq!(
        ldap_semantics.primary_failure_mode.as_deref(),
        Some("server_denied")
    );
    assert_eq!(
        ldap_semantics.primary_failure_detail.as_deref(),
        Some("access_denied")
    );
    assert_eq!(
        ldap_semantics.primary_failure_basis.as_deref(),
        Some("direct_protocol_signal")
    );

    let http_connect = protocol_surface("http", "connect").expect("http connect should exist");
    let http_connect_semantics = http_connect
        .entry_semantics
        .expect("http connect should expose tunnel semantics");
    assert_eq!(http_connect_semantics.category, "http-connect-tunnel-path");

    let ldap_bind = protocol_surface("ldap", "bind").expect("ldap bind should exist");
    let ldap_bind_semantics = ldap_bind
        .entry_semantics
        .expect("ldap bind should expose positive bind semantics");
    assert_eq!(ldap_bind_semantics.category, "directory-bind-path");
}
