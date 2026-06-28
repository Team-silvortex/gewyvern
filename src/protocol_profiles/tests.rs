use super::{
    protocol_dsl_path, protocol_entries, protocol_summary, protocol_surface,
    resolve_built_in_dsl_path,
};
use std::fs;
#[cfg(target_family = "unix")]
use std::os::unix::fs as unix_fs;
use std::path::PathBuf;

use super::tests_env::EnvGuard;

#[test]
fn http_entry_aliases_resolve_to_canonical_registry_targets() {
    let _lock = super::tests_env::lock();
    assert_eq!(
        protocol_dsl_path("http", Some("client")),
        Some(super::protocol_fixture_path("http/request"))
    );
    assert_eq!(
        protocol_dsl_path("http", Some("server")),
        Some(super::protocol_fixture_path("http/response"))
    );
}

#[test]
fn list_entries_prefers_canonical_http_entries() {
    let _lock = super::tests_env::lock();
    let entries = protocol_entries("http").expect("http entries should resolve");
    assert!(entries.contains(&"request".to_string()));
    assert!(entries.contains(&"response".to_string()));
    assert!(!entries.contains(&"client".to_string()));
    assert!(!entries.contains(&"server".to_string()));
}

#[test]
fn built_in_protocol_summary_surfaces_entry_aliases() {
    let _lock = super::tests_env::lock();
    let summary = protocol_summary("mysql").expect("mysql summary should exist");
    let session = summary
        .entries
        .into_iter()
        .find(|entry| entry.mode == "session")
        .expect("mysql session entry should exist");
    assert!(session.default);
    assert!(session.aliases.contains(&"mysql-session".to_string()));
    assert!(session.aliases.contains(&"mysql_session".to_string()));
}

#[test]
fn protocol_surface_exposes_protocol_shelves_for_grouped_families() {
    let _lock = super::tests_env::lock();
    let hy2 = protocol_surface("hy2", "tcp").expect("hy2 tcp surface should exist");
    let hy2_shelf = hy2.shelf.expect("hy2 tcp should have a shelf");
    assert_eq!(hy2_shelf.key, "relay");
    assert_eq!(hy2_shelf.label, "Relay");

    let quic = protocol_surface("quic", "bidi").expect("quic bidi surface should exist");
    let quic_shelf = quic.shelf.expect("quic bidi should have a shelf");
    assert_eq!(quic_shelf.key, "bidi");
    assert_eq!(quic_shelf.label, "Bidirectional Stream");

    let dns = protocol_surface("dns", "udp").expect("dns udp surface should exist");
    let dns_shelf = dns.shelf.expect("dns udp should have a shelf");
    assert_eq!(dns_shelf.key, "udp");
    assert_eq!(dns_shelf.label, "UDP Lookup");

    let https = protocol_surface("https", "connect").expect("https connect surface should exist");
    let https_shelf = https.shelf.expect("https connect should have a shelf");
    assert_eq!(https_shelf.key, "connect");
    assert_eq!(https_shelf.label, "Connect");

    let http = protocol_surface("http", "auth-required").expect("http auth surface should exist");
    let http_shelf = http.shelf.expect("http auth-required should have a shelf");
    assert_eq!(http_shelf.key, "connect-auth");
    assert_eq!(http_shelf.label, "Connect Auth");

    let postgres =
        protocol_surface("postgres", "auth").expect("postgres auth surface should exist");
    let postgres_shelf = postgres.shelf.expect("postgres auth should have a shelf");
    assert_eq!(postgres_shelf.key, "connect-auth");
    assert_eq!(postgres_shelf.label, "Connect And Auth");
    assert_eq!(
        postgres_shelf.page,
        "docs/book/reference-postgres-connect-surface.md"
    );
    assert!(postgres_shelf.entries.contains(&"connect".to_string()));
    assert!(postgres_shelf.entries.contains(&"auth".to_string()));

    let mysql = protocol_surface("mysql", "session").expect("mysql session surface should exist");
    let mysql_shelf = mysql.shelf.expect("mysql session should have a shelf");
    assert_eq!(mysql_shelf.key, "query-session");
    assert_eq!(mysql_shelf.label, "Query And Session");
    assert!(mysql_shelf.entries.contains(&"query".to_string()));
    assert!(mysql_shelf.entries.contains(&"session".to_string()));

    let mqtt =
        protocol_surface("mqtt", "disconnect").expect("mqtt disconnect surface should exist");
    let mqtt_shelf = mqtt.shelf.expect("mqtt disconnect should have a shelf");
    assert_eq!(mqtt_shelf.key, "qos2-teardown");
    assert_eq!(mqtt_shelf.label, "QoS2 And Teardown");
    assert_eq!(mqtt_shelf.page, "docs/book/reference-mqtt-qos2-surface.md");
    assert!(mqtt_shelf.entries.contains(&"pubrel".to_string()));
    assert!(mqtt_shelf.entries.contains(&"disconnect".to_string()));

    let memcached =
        protocol_surface("memcached", "set").expect("memcached set surface should exist");
    let memcached_shelf = memcached.shelf.expect("memcached set should have a shelf");
    assert_eq!(memcached_shelf.key, "set");
    assert_eq!(memcached_shelf.label, "Set");

    let radius = protocol_surface("radius", "access").expect("radius access surface should exist");
    let radius_shelf = radius.shelf.expect("radius access should have a shelf");
    assert_eq!(radius_shelf.key, "access");
    assert_eq!(radius_shelf.label, "Access");

    let redis = protocol_surface("redis", "zadd").expect("redis zadd surface should exist");
    let redis_shelf = redis.shelf.expect("redis zadd should have a shelf");
    assert_eq!(redis_shelf.key, "sorted-set");
    assert_eq!(redis_shelf.label, "Sorted Set");
    assert_eq!(
        redis_shelf.page,
        "docs/book/reference-redis-sorted-set-surface.md"
    );
    assert!(redis_shelf.entries.contains(&"zadd".to_string()));
    assert!(redis_shelf.entries.contains(&"zrange".to_string()));

    let amqp = protocol_surface("amqp", "publish").expect("amqp publish surface should exist");
    let amqp_shelf = amqp.shelf.expect("amqp publish should have a shelf");
    assert_eq!(amqp_shelf.key, "session-publish");
    assert_eq!(amqp_shelf.label, "Session And Publish");
    assert_eq!(
        amqp_shelf.page,
        "docs/book/reference-amqp-session-surface.md"
    );
    assert!(amqp_shelf.entries.contains(&"session".to_string()));
    assert!(amqp_shelf.entries.contains(&"publish".to_string()));

    let smtp = protocol_surface("smtp", "rcpt-denied").expect("smtp rcpt surface should exist");
    let smtp_shelf = smtp.shelf.expect("smtp rcpt-denied should have a shelf");
    assert_eq!(smtp_shelf.key, "envelope");
    assert_eq!(smtp_shelf.label, "Envelope");

    let kerberos = protocol_surface("kerberos", "as-error").expect("kerberos as surface exists");
    let kerberos_shelf = kerberos
        .shelf
        .expect("kerberos as-error should have a shelf");
    assert_eq!(kerberos_shelf.key, "as");
    assert_eq!(kerberos_shelf.label, "AS Exchange");

    let ftp = protocol_surface("ftp", "active-retr").expect("ftp active retr should exist");
    let ftp_shelf = ftp.shelf.expect("ftp active-retr should have a shelf");
    assert_eq!(ftp_shelf.key, "active");
    assert_eq!(ftp_shelf.label, "Active Data");

    let rtsp = protocol_surface("rtsp", "describe").expect("rtsp describe should exist");
    let rtsp_shelf = rtsp.shelf.expect("rtsp describe should have a shelf");
    assert_eq!(rtsp_shelf.key, "describe");
    assert_eq!(rtsp_shelf.label, "Describe");

    let ssh = protocol_surface("ssh", "channel").expect("ssh channel should exist");
    let ssh_shelf = ssh.shelf.expect("ssh channel should have a shelf");
    assert_eq!(ssh_shelf.key, "channel");
    assert_eq!(ssh_shelf.label, "Channel");

    let imap = protocol_surface("imap", "auth-denied").expect("imap auth surface should exist");
    let imap_shelf = imap.shelf.expect("imap auth-denied should have a shelf");
    assert_eq!(imap_shelf.key, "auth");
    assert_eq!(imap_shelf.label, "Auth");

    let pop3 = protocol_surface("pop3", "list").expect("pop3 list surface should exist");
    let pop3_shelf = pop3.shelf.expect("pop3 list should have a shelf");
    assert_eq!(pop3_shelf.key, "list");
    assert_eq!(pop3_shelf.label, "Mailbox List");

    let socks5 =
        protocol_surface("socks5", "auth-connect-denied").expect("socks5 denied should exist");
    let socks5_shelf = socks5.shelf.expect("socks5 denied should have a shelf");
    assert_eq!(socks5_shelf.key, "denied");
    assert_eq!(socks5_shelf.label, "Denied");

    let sip = protocol_surface("sip", "register").expect("sip register surface should exist");
    let sip_shelf = sip.shelf.expect("sip register should have a shelf");
    assert_eq!(sip_shelf.key, "register");
    assert_eq!(sip_shelf.label, "Register");

    let ldap = protocol_surface("ldap", "sync").expect("ldap sync surface should exist");
    let ldap_shelf = ldap.shelf.expect("ldap sync should have a shelf");
    assert_eq!(ldap_shelf.key, "write-sync");
    assert_eq!(ldap_shelf.label, "Write And Sync");
}

#[test]
fn protocol_surface_exposes_single_entry_shelves() {
    let _lock = super::tests_env::lock();
    for (protocol, entry, key) in [
        ("tls", "client", "client"),
        ("stun", "binding", "binding"),
        ("coap", "get", "get"),
        ("ntp", "client", "client"),
        ("dhcp", "client", "client"),
        ("wireguard", "handshake", "handshake"),
        ("mdns", "query", "query"),
        ("ssdp", "discovery", "discovery"),
        ("gtpu", "echo", "echo"),
        ("snmp", "get", "read"),
    ] {
        let surface = protocol_surface(protocol, entry).expect("single-entry surface should exist");
        assert_eq!(
            surface.shelf.expect("single-entry shelf should exist").key,
            key
        );
    }
}

#[test]
fn mysql_query_entry_resolves_to_dedicated_query_package() {
    let _lock = super::tests_env::lock();
    assert_eq!(
        protocol_dsl_path("mysql", Some("query")),
        Some(super::protocol_fixture_path("mysql/query"))
    );
}

#[test]
fn rtsp_package_aliases_resolve_to_canonical_entries() {
    let _lock = super::tests_env::lock();
    assert_eq!(
        protocol_dsl_path("rtsp-options", None),
        Some(super::protocol_fixture_path("rtsp/options"))
    );
    assert_eq!(
        protocol_dsl_path("rtsp-describe", None),
        Some(super::protocol_fixture_path("rtsp/describe"))
    );
    assert_eq!(
        protocol_dsl_path("rtsp-setup", None),
        Some(super::protocol_fixture_path("rtsp/setup"))
    );
}

#[test]
fn http_connect_family_aliases_resolve_to_canonical_entries() {
    let _lock = super::tests_env::lock();
    assert_eq!(
        protocol_dsl_path("http-connect-auth-required", None),
        Some(super::protocol_fixture_path("http/auth-required"))
    );
    assert_eq!(
        protocol_dsl_path("http-connect-auth-tunnel", None),
        Some(super::protocol_fixture_path("http/auth-tunnel"))
    );
    assert_eq!(
        protocol_dsl_path("http-connect-denied", None),
        Some(super::protocol_fixture_path("http/denied"))
    );
}

#[test]
fn memcached_entry_aliases_resolve_to_canonical_entries() {
    let _lock = super::tests_env::lock();
    assert_eq!(
        protocol_dsl_path("memcached", Some("read")),
        Some(super::protocol_fixture_path("memcached/get"))
    );
    assert_eq!(
        protocol_dsl_path("memcached", Some("write")),
        Some(super::protocol_fixture_path("memcached/set"))
    );
}

#[test]
fn mail_retrieval_entry_aliases_resolve_to_canonical_entries() {
    let _lock = super::tests_env::lock();
    assert_eq!(
        protocol_dsl_path("imap", Some("login")),
        Some(super::protocol_fixture_path("imap/auth"))
    );
    assert_eq!(
        protocol_dsl_path("imap", Some("login-denied")),
        Some(super::protocol_fixture_path("imap/auth-denied"))
    );
    assert_eq!(
        protocol_dsl_path("imap", Some("mailbox")),
        Some(super::protocol_fixture_path("imap/select"))
    );
    assert_eq!(
        protocol_dsl_path("pop3", Some("login")),
        Some(super::protocol_fixture_path("pop3/auth"))
    );
    assert_eq!(
        protocol_dsl_path("pop3", Some("login-denied")),
        Some(super::protocol_fixture_path("pop3/auth-denied"))
    );
    assert_eq!(
        protocol_dsl_path("pop3", Some("mailbox")),
        Some(super::protocol_fixture_path("pop3/list"))
    );
}

#[test]
fn smtp_entry_aliases_resolve_to_canonical_entries() {
    let _lock = super::tests_env::lock();
    assert_eq!(
        protocol_dsl_path("smtp", Some("login")),
        Some(super::protocol_fixture_path("smtp/auth"))
    );
    assert_eq!(
        protocol_dsl_path("smtp", Some("login-denied")),
        Some(super::protocol_fixture_path("smtp/auth-denied"))
    );
    assert_eq!(
        protocol_dsl_path("smtp", Some("sender")),
        Some(super::protocol_fixture_path("smtp/mail"))
    );
    assert_eq!(
        protocol_dsl_path("smtp", Some("recipient")),
        Some(super::protocol_fixture_path("smtp/rcpt"))
    );
    assert_eq!(
        protocol_dsl_path("smtp", Some("message")),
        Some(super::protocol_fixture_path("smtp/data"))
    );
    assert_eq!(
        protocol_dsl_path("smtp", Some("recipient-denied")),
        Some(super::protocol_fixture_path("smtp/rcpt-denied"))
    );
    assert_eq!(
        protocol_dsl_path("smtp", Some("message-denied")),
        Some(super::protocol_fixture_path("smtp/data-denied"))
    );
}

#[test]
fn ftp_entry_aliases_resolve_to_canonical_entries() {
    let _lock = super::tests_env::lock();
    assert_eq!(
        protocol_dsl_path("ftp", Some("login")),
        Some(super::protocol_fixture_path("ftp/session"))
    );
    assert_eq!(
        protocol_dsl_path("ftp", Some("control")),
        Some(super::protocol_fixture_path("ftp/session"))
    );
    assert_eq!(
        protocol_dsl_path("ftp", Some("directory")),
        Some(super::protocol_fixture_path("ftp/list"))
    );
    assert_eq!(
        protocol_dsl_path("ftp", Some("download")),
        Some(super::protocol_fixture_path("ftp/retr"))
    );
    assert_eq!(
        protocol_dsl_path("ftp", Some("upload")),
        Some(super::protocol_fixture_path("ftp/stor"))
    );
    assert_eq!(
        protocol_dsl_path("ftp", Some("active-directory")),
        Some(super::protocol_fixture_path("ftp/active-list"))
    );
    assert_eq!(
        protocol_dsl_path("ftp", Some("active-download")),
        Some(super::protocol_fixture_path("ftp/active-retr"))
    );
    assert_eq!(
        protocol_dsl_path("ftp", Some("active-upload")),
        Some(super::protocol_fixture_path("ftp/active-stor"))
    );
    assert_eq!(
        protocol_dsl_path("ftp", Some("login-denied")),
        Some(super::protocol_fixture_path("ftp/denied"))
    );
}

#[test]
fn auth_family_entry_aliases_resolve_to_canonical_entries() {
    let _lock = super::tests_env::lock();
    assert_eq!(
        protocol_dsl_path("kerberos", Some("login")),
        Some(super::protocol_fixture_path("kerberos/as"))
    );
    assert_eq!(
        protocol_dsl_path("kerberos", Some("initial-auth")),
        Some(super::protocol_fixture_path("kerberos/as"))
    );
    assert_eq!(
        protocol_dsl_path("kerberos", Some("ticket")),
        Some(super::protocol_fixture_path("kerberos/tgs"))
    );
    assert_eq!(
        protocol_dsl_path("kerberos", Some("service-ticket")),
        Some(super::protocol_fixture_path("kerberos/tgs"))
    );
    assert_eq!(
        protocol_dsl_path("kerberos", Some("login-denied")),
        Some(super::protocol_fixture_path("kerberos/as-error"))
    );
    assert_eq!(
        protocol_dsl_path("kerberos", Some("initial-auth-error")),
        Some(super::protocol_fixture_path("kerberos/as-error"))
    );
    assert_eq!(
        protocol_dsl_path("radius", Some("login")),
        Some(super::protocol_fixture_path("radius/access"))
    );
    assert_eq!(
        protocol_dsl_path("radius", Some("auth")),
        Some(super::protocol_fixture_path("radius/access"))
    );
}

#[test]
fn access_and_messaging_entry_aliases_resolve_to_canonical_entries() {
    let _lock = super::tests_env::lock();
    assert_eq!(
        protocol_dsl_path("ssh", Some("connect")),
        Some(super::protocol_fixture_path("ssh/session"))
    );
    assert_eq!(
        protocol_dsl_path("ssh", Some("login")),
        Some(super::protocol_fixture_path("ssh/auth"))
    );
    assert_eq!(
        protocol_dsl_path("ssh", Some("shell")),
        Some(super::protocol_fixture_path("ssh/channel"))
    );
    assert_eq!(
        protocol_dsl_path("socks5", Some("proxy")),
        Some(super::protocol_fixture_path("socks5/session"))
    );
    assert_eq!(
        protocol_dsl_path("socks5", Some("userpass")),
        Some(super::protocol_fixture_path("socks5/auth"))
    );
    assert_eq!(
        protocol_dsl_path("socks5", Some("connect-denied")),
        Some(super::protocol_fixture_path("socks5/denied"))
    );
    assert_eq!(
        protocol_dsl_path("socks5", Some("login-connect-denied")),
        Some(super::protocol_fixture_path("socks5/auth-connect-denied"))
    );
    assert_eq!(
        protocol_dsl_path("ldap", Some("login")),
        Some(super::protocol_fixture_path("ldap/bind"))
    );
    assert_eq!(
        protocol_dsl_path("ldap", Some("directory")),
        Some(super::protocol_fixture_path("ldap/search"))
    );
    assert_eq!(
        protocol_dsl_path("ldap", Some("directory-session")),
        Some(super::protocol_fixture_path("ldap/session"))
    );
    assert_eq!(
        protocol_dsl_path("ldap", Some("replication")),
        Some(super::protocol_fixture_path("ldap/sync"))
    );
    assert_eq!(
        protocol_dsl_path("snmp", Some("query")),
        Some(super::protocol_fixture_path("snmp/get"))
    );
    assert_eq!(
        protocol_dsl_path("sip", Some("login")),
        Some(super::protocol_fixture_path("sip/register"))
    );
    assert_eq!(
        protocol_dsl_path("rtsp", Some("probe")),
        Some(super::protocol_fixture_path("rtsp/options"))
    );
    assert_eq!(
        protocol_dsl_path("rtsp", Some("metadata")),
        Some(super::protocol_fixture_path("rtsp/describe"))
    );
    assert_eq!(
        protocol_dsl_path("rtsp", Some("stream")),
        Some(super::protocol_fixture_path("rtsp/setup"))
    );
    assert_eq!(
        protocol_dsl_path("amqp", Some("login")),
        Some(super::protocol_fixture_path("amqp/start"))
    );
    assert_eq!(
        protocol_dsl_path("amqp-auth-denied", None),
        Some(super::protocol_fixture_path("amqp/auth-denied"))
    );
    assert_eq!(
        protocol_dsl_path("amqp", Some("login-denied")),
        Some(super::protocol_fixture_path("amqp/auth-denied"))
    );
    assert_eq!(
        protocol_dsl_path("amqp", Some("connect")),
        Some(super::protocol_fixture_path("amqp/session"))
    );
    assert_eq!(
        protocol_dsl_path("amqp", Some("send")),
        Some(super::protocol_fixture_path("amqp/publish"))
    );
    assert_eq!(
        protocol_dsl_path("mqtt", Some("session")),
        Some(super::protocol_fixture_path("mqtt/connect"))
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("health")),
        Some(super::protocol_fixture_path("redis/ping"))
    );
}

#[test]
fn built_in_dsl_path_falls_back_to_packaged_share_root() {
    let _lock = super::tests_env::lock();
    let root = std::env::temp_dir().join(format!(
        "gewyvern-packaged-dsl-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let dsl_dir = root.join("dsl");
    fs::create_dir_all(&dsl_dir).unwrap();
    let file = dsl_dir.join("http_request_path.gewy");
    fs::write(&file, "template(:http_request_path)\n").unwrap();
    let _guard = EnvGuard::set("GEWY_SHARE_ROOT", root.to_string_lossy().into_owned());
    let resolved = resolve_built_in_dsl_path("/definitely/missing/dsl/http_request_path.gewy");
    fs::remove_dir_all(&root).unwrap();

    assert_eq!(PathBuf::from(resolved), file);
}

#[test]
fn packaged_registry_root_is_used_when_explicitly_set() {
    let _lock = super::tests_env::lock();
    let root = std::env::temp_dir().join(format!(
        "gewyvern-packaged-protocol-registry-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let package_dir = root.join("http").join("request");
    fs::create_dir_all(&package_dir).unwrap();
    fs::write(
        package_dir.join("gewy.pkg"),
        "name=http_request\nversion=0.18.2\nentry=main.gewy\nregister.protocol=http\nregister.entry=request\nregister.default=true\n",
    )
    .unwrap();
    fs::write(package_dir.join("main.gewy"), "template(:http_request)\n").unwrap();
    let _guard = EnvGuard::set(
        "GEWY_PROTOCOL_REGISTRY_ROOT",
        root.to_string_lossy().into_owned(),
    );

    let resolved = protocol_dsl_path("http", Some("request"));
    let expected = fs::canonicalize(&package_dir)
        .unwrap()
        .to_string_lossy()
        .into_owned();
    fs::remove_dir_all(&root).unwrap();

    assert_eq!(resolved, Some(expected));
}

#[cfg(target_family = "unix")]
#[test]
fn registry_scan_ignores_symlinked_directories() {
    let root = std::env::temp_dir().join(format!(
        "gewyvern-protocol-registry-symlink-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let package_dir = root.join("mysql").join("session");
    fs::create_dir_all(&package_dir).unwrap();
    fs::write(
        package_dir.join("gewy.pkg"),
        "name=mysql_session\nversion=0.18.2\nentry=main.gewy\nregister.protocol=mysql\nregister.entry=session\nregister.default=true\n",
    )
    .unwrap();
    fs::write(package_dir.join("main.gewy"), "template(:mysql_session)\n").unwrap();
    unix_fs::symlink(root.join("mysql"), root.join("mysql-link")).unwrap();

    let targets = super::default_protocol_scan_set_from_dir(root.to_str().unwrap()).unwrap();
    fs::remove_dir_all(&root).unwrap();

    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].protocol, "mysql");
    assert_eq!(targets[0].entry, "session");
}
