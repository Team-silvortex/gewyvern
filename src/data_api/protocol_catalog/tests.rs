use super::*;

#[test]
fn protocol_catalog_lists_mysql_summary_and_entry_path_template() {
    let body = api_protocol_catalog_json();
    assert!(body.contains("\"surface\":\"protocol_catalog\""));
    assert!(body.contains("\"protocol\":\"mysql\""));
    assert!(body.contains("\"default_entry\":\"session\""));
    assert!(body.contains("\"cluster_hint\":{"));
    assert!(body.contains(
        "\"entry_surface_path_template\":\"/v1/protocols/mysql/entries/<entry>/surface.json\""
    ));
}

#[test]
fn protocol_surface_by_name_includes_redis_shelf_context() {
    let body = api_protocol_surface_by_name_json("redis", "zadd")
        .expect("redis zadd surface should exist");
    assert!(body.contains("\"protocol\":\"redis\""));
    assert!(body.contains("\"entry\":\"zadd\""));
    assert!(body.contains("\"selected_is_default\":false"));
    assert!(body.contains("\"cluster_hint\":{"));
    assert!(body.contains("\"key\":\"cache-queue-stream\""));
    assert!(body.contains("\"key\":\"sorted-set\""));
    assert!(body.contains("\"entry_semantics\":null"));
}

#[test]
fn protocol_surface_by_name_includes_redis_failure_entry_semantics() {
    let body = api_protocol_surface_by_name_json("redis", "clusterdown")
        .expect("redis clusterdown surface should exist");
    assert!(body.contains("\"entry\":\"clusterdown\""));
    assert!(body.contains("\"entry_semantics\":{"));
    assert!(body.contains("\"category\":\"failure-path\""));
    assert!(body.contains("\"typical_signal\":\"-CLUSTERDOWN\""));
    assert!(body.contains("\"primary_failure_mode\":\"semantic_error\""));
    assert!(body.contains("\"primary_failure_detail\":\"protocol_error\""));
    assert!(body.contains("\"primary_failure_basis\":\"direct_protocol_signal\""));
}

#[test]
fn protocol_surface_by_name_includes_socks5_denied_entry_semantics() {
    let body = api_protocol_surface_by_name_json("socks5", "auth-connect-denied")
        .expect("socks5 auth-connect-denied surface should exist");
    assert!(body.contains("\"entry\":\"auth-connect-denied\""));
    assert!(body.contains("\"entry_semantics\":{"));
    assert!(body.contains("\"category\":\"failure-path\""));
    assert!(body.contains(
        "\"operator_focus\":\"upstream connect refusal after authenticated proxy setup\""
    ));
    assert!(body.contains("\"typical_signal\":null"));
    assert!(body.contains("\"primary_failure_mode\":\"server_denied\""));
    assert!(body.contains("\"primary_failure_detail\":\"access_denied\""));
    assert!(body.contains("\"primary_failure_basis\":\"direct_protocol_signal\""));
}

#[test]
fn protocol_surface_by_name_includes_http_connect_denied_entry_semantics() {
    let body = api_protocol_surface_by_name_json("http", "denied")
        .expect("http denied surface should exist");
    assert!(body.contains("\"entry\":\"denied\""));
    assert!(body.contains("\"entry_semantics\":{"));
    assert!(body.contains("\"category\":\"failure-path\""));
    assert!(
        body.contains(
            "\"operator_focus\":\"proxy tunnel refusal after CONNECT policy evaluation\""
        )
    );
    assert!(body.contains("\"typical_signal\":\"403\""));
    assert!(body.contains("\"primary_failure_mode\":\"server_denied\""));
    assert!(body.contains("\"primary_failure_detail\":\"access_denied\""));
    assert!(body.contains("\"primary_failure_basis\":\"direct_protocol_signal\""));
}

#[test]
fn protocol_clusters_catalog_groups_cache_queue_families() {
    let body = api_protocol_clusters_json();
    assert!(body.contains("\"surface\":\"protocol_cluster_catalog\""));
    assert!(body.contains("\"key\":\"cache-queue-stream\""));
    assert!(body.contains("\"protocol\":\"redis\""));
    assert!(body.contains("\"protocol\":\"mqtt\""));
}

#[test]
fn protocol_cluster_view_returns_identity_access_cluster() {
    let body = api_protocol_cluster_json("identity-directory-access")
        .expect("identity cluster should exist");
    assert!(body.contains("\"key\":\"identity-directory-access\""));
    assert!(body.contains("\"protocol\":\"ldap\""));
    assert!(body.contains("\"protocol\":\"ssh\""));
}

#[test]
fn scan_target_name_resolves_protocol_surface() {
    let surface =
        api_protocol_surface_for_target("scan:http:request").expect("scan target should resolve");
    assert_eq!(surface.protocol, "http");
    assert_eq!(surface.entry, "request");
}

#[test]
fn protocol_surface_by_alias_resolves_dot_and_doh_targets() {
    let dot =
        api_protocol_surface_for_target("scan:dot:tcp").expect("dot target alias should resolve");
    assert_eq!(dot.protocol, "dns");
    assert_eq!(dot.entry, "tcp");
    assert_eq!(dot.selected_overlay.as_deref(), Some("dot"));
    assert!(dot.overlays.iter().any(|overlay| overlay.key == "dot"));

    let doh = api_protocol_surface_for_target("scan:doh:request")
        .expect("doh target alias should resolve");
    assert_eq!(doh.protocol, "http");
    assert_eq!(doh.entry, "request");
    assert_eq!(doh.selected_overlay.as_deref(), Some("doh"));
    assert!(doh.overlays.iter().any(|overlay| overlay.key == "doh"));
}

#[test]
fn protocol_surface_json_emits_overlay_metadata_for_dot_and_doh() {
    let dot = api_protocol_surface_by_name_json("dot", "tcp")
        .expect("dot overlay should render as dns/tcp surface");
    assert!(dot.contains("\"protocol\":\"dns\""));
    assert!(dot.contains("\"entry\":\"tcp\""));
    assert!(dot.contains("\"selected_overlay\":\"dot\""));
    assert!(dot.contains("\"key\":\"dot\""));
    assert!(dot.contains("\"kind\":\"encrypted_resolver_overlay\""));

    let doh = api_protocol_surface_by_name_json("doh", "request")
        .expect("doh overlay should render as http/request surface");
    assert!(doh.contains("\"protocol\":\"http\""));
    assert!(doh.contains("\"entry\":\"request\""));
    assert!(doh.contains("\"selected_overlay\":\"doh\""));
    assert!(doh.contains("\"key\":\"doh\""));
    assert!(doh.contains("\"kind\":\"resolver_payload_overlay\""));
}

#[test]
fn protocol_surface_json_emits_http_connect_overlay_metadata() {
    let connect = api_protocol_surface_by_name_json("http-connect", "connect")
        .expect("http connect alias should render as http/connect surface");
    assert!(connect.contains("\"protocol\":\"http\""));
    assert!(connect.contains("\"entry\":\"connect\""));
    assert!(connect.contains("\"selected_overlay\":\"http-connect\""));
    assert!(connect.contains("\"key\":\"http-connect\""));
    assert!(connect.contains("\"kind\":\"proxy_tunnel_overlay\""));

    let denied = api_protocol_surface_by_name_json("http-connect-denied", "denied")
        .expect("http connect denied alias should render as denied tunnel surface");
    assert!(denied.contains("\"protocol\":\"http\""));
    assert!(denied.contains("\"entry\":\"denied\""));
    assert!(denied.contains("\"selected_overlay\":\"http-connect\""));
    assert!(denied.contains("\"key\":\"http-connect\""));
}

#[test]
fn protocol_surface_json_emits_starttls_overlay_metadata_on_mail_auth_surfaces() {
    let smtp =
        api_protocol_surface_by_name_json("smtp", "auth").expect("smtp auth surface should exist");
    assert!(smtp.contains("\"protocol\":\"smtp\""));
    assert!(smtp.contains("\"entry\":\"auth\""));
    assert!(smtp.contains("\"selected_overlay\":null"));
    assert!(smtp.contains("\"key\":\"starttls\""));
    assert!(smtp.contains("\"kind\":\"tls_upgrade_overlay\""));

    let smtp_denied = api_protocol_surface_by_name_json("smtp", "auth-denied")
        .expect("smtp denied auth surface should exist");
    assert!(smtp_denied.contains("\"protocol\":\"smtp\""));
    assert!(smtp_denied.contains("\"entry\":\"auth-denied\""));
    assert!(smtp_denied.contains("\"key\":\"starttls\""));

    let imap = api_protocol_surface_by_name_json("imap", "auth-denied")
        .expect("imap denied auth surface should exist");
    assert!(imap.contains("\"protocol\":\"imap\""));
    assert!(imap.contains("\"entry\":\"auth-denied\""));
    assert!(imap.contains("\"key\":\"starttls\""));

    let pop3 =
        api_protocol_surface_by_name_json("pop3", "auth").expect("pop3 auth surface should exist");
    assert!(pop3.contains("\"protocol\":\"pop3\""));
    assert!(pop3.contains("\"entry\":\"auth\""));
    assert!(pop3.contains("\"key\":\"starttls\""));
}

#[test]
fn protocol_surface_json_emits_https_and_tls_companion_overlay_metadata() {
    let https = api_protocol_surface_by_name_json("https", "connect")
        .expect("https connect surface should exist");
    assert!(https.contains("\"protocol\":\"https\""));
    assert!(https.contains("\"entry\":\"connect\""));
    assert!(https.contains("\"key\":\"https\""));
    assert!(https.contains("\"kind\":\"tls_application_overlay\""));
    assert!(https.contains("\"companion_protocol\":\"tls\""));
    assert!(https.contains("\"companion_entry\":\"client\""));
    assert!(https.contains(
        "\"reading_companions\":[{\"protocol\":\"tls\",\"entry\":\"client\",\"via_overlay\":\"https\""
    ));

    let tls = api_protocol_surface_by_name_json("tls", "client")
        .expect("tls client surface should exist");
    assert!(tls.contains("\"protocol\":\"tls\""));
    assert!(tls.contains("\"entry\":\"client\""));
    assert!(tls.contains("\"key\":\"https\""));
    assert!(tls.contains("\"companion_protocol\":\"https\""));
    assert!(tls.contains("\"companion_entry\":\"connect\""));
    assert!(tls.contains("\"key\":\"dot\""));
    assert!(tls.contains("\"reading_companions\":["));
}

#[test]
fn protocol_surface_json_emits_http3_and_quic_companion_overlay_metadata() {
    let http3 = api_protocol_surface_by_name_json("http3", "request")
        .expect("http3 request surface should exist");
    assert!(http3.contains("\"protocol\":\"http3\""));
    assert!(http3.contains("\"entry\":\"request\""));
    assert!(http3.contains("\"key\":\"http3\""));
    assert!(http3.contains("\"kind\":\"quic_application_overlay\""));
    assert!(http3.contains("\"companion_protocol\":\"quic\""));
    assert!(http3.contains("\"companion_entry\":\"initial\""));
    assert!(http3.contains(
        "\"reading_companions\":[{\"protocol\":\"quic\",\"entry\":\"initial\",\"via_overlay\":\"http3\""
    ));

    let quic = api_protocol_surface_by_name_json("quic", "crypto")
        .expect("quic crypto surface should exist");
    assert!(quic.contains("\"protocol\":\"quic\""));
    assert!(quic.contains("\"entry\":\"crypto\""));
    assert!(quic.contains("\"key\":\"http3\""));
    assert!(quic.contains("\"companion_protocol\":\"http3\""));
    assert!(quic.contains("\"companion_entry\":\"request\""));
    assert!(quic.contains(
        "\"reading_companions\":[{\"protocol\":\"http3\",\"entry\":\"request\",\"via_overlay\":\"http3\""
    ));
}
