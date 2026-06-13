use super::{
    protocol_dsl_path, protocol_entries, protocol_summary, protocol_surface,
    resolve_built_in_dsl_path,
};
use std::fs;
#[cfg(target_family = "unix")]
use std::os::unix::fs as unix_fs;
use std::path::PathBuf;

struct EnvGuard {
    key: &'static str,
    previous: Option<String>,
}

impl EnvGuard {
    fn set(key: &'static str, value: String) -> Self {
        let previous = std::env::var(key).ok();
        unsafe {
            std::env::set_var(key, value);
        }
        Self { key, previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        match &self.previous {
            Some(value) => unsafe {
                std::env::set_var(self.key, value);
            },
            None => unsafe {
                std::env::remove_var(self.key);
            },
        }
    }
}

#[test]
fn http_entry_aliases_resolve_to_canonical_registry_targets() {
    assert_eq!(
        protocol_dsl_path("http", Some("client")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/http/request".to_string())
    );
    assert_eq!(
        protocol_dsl_path("http", Some("server")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/http/response".to_string())
    );
}

#[test]
fn list_entries_prefers_canonical_http_entries() {
    let entries = protocol_entries("http").expect("http entries should resolve");
    assert!(entries.contains(&"request".to_string()));
    assert!(entries.contains(&"response".to_string()));
    assert!(!entries.contains(&"client".to_string()));
    assert!(!entries.contains(&"server".to_string()));
}

#[test]
fn built_in_protocol_summary_surfaces_entry_aliases() {
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
        ("snmp", "get", "get"),
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
    assert_eq!(
        protocol_dsl_path("mysql", Some("query")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/mysql/query".to_string())
    );
}

#[test]
fn rtsp_package_aliases_resolve_to_canonical_entries() {
    assert_eq!(
        protocol_dsl_path("rtsp-options", None),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/rtsp/options".to_string())
    );
    assert_eq!(
        protocol_dsl_path("rtsp-describe", None),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/rtsp/describe".to_string())
    );
    assert_eq!(
        protocol_dsl_path("rtsp-setup", None),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/rtsp/setup".to_string())
    );
}

#[test]
fn http_connect_family_aliases_resolve_to_canonical_entries() {
    assert_eq!(
        protocol_dsl_path("http-connect-auth-required", None),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/http/auth-required".to_string())
    );
    assert_eq!(
        protocol_dsl_path("http-connect-auth-tunnel", None),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/http/auth-tunnel".to_string())
    );
    assert_eq!(
        protocol_dsl_path("http-connect-denied", None),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/http/denied".to_string())
    );
}

#[test]
fn memcached_entry_aliases_resolve_to_canonical_entries() {
    assert_eq!(
        protocol_dsl_path("memcached", Some("read")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/memcached/get".to_string())
    );
    assert_eq!(
        protocol_dsl_path("memcached", Some("write")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/memcached/set".to_string())
    );
}

#[test]
fn mail_retrieval_entry_aliases_resolve_to_canonical_entries() {
    assert_eq!(
        protocol_dsl_path("imap", Some("login")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/imap/auth".to_string())
    );
    assert_eq!(
        protocol_dsl_path("imap", Some("login-denied")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/imap/auth-denied".to_string())
    );
    assert_eq!(
        protocol_dsl_path("imap", Some("mailbox")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/imap/select".to_string())
    );
    assert_eq!(
        protocol_dsl_path("pop3", Some("login")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/pop3/auth".to_string())
    );
    assert_eq!(
        protocol_dsl_path("pop3", Some("login-denied")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/pop3/auth-denied".to_string())
    );
    assert_eq!(
        protocol_dsl_path("pop3", Some("mailbox")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/pop3/list".to_string())
    );
}

#[test]
fn smtp_entry_aliases_resolve_to_canonical_entries() {
    assert_eq!(
        protocol_dsl_path("smtp", Some("login")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/smtp/auth".to_string())
    );
    assert_eq!(
        protocol_dsl_path("smtp", Some("sender")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/smtp/mail".to_string())
    );
    assert_eq!(
        protocol_dsl_path("smtp", Some("recipient")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/smtp/rcpt".to_string())
    );
    assert_eq!(
        protocol_dsl_path("smtp", Some("message")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/smtp/data".to_string())
    );
    assert_eq!(
        protocol_dsl_path("smtp", Some("recipient-denied")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/smtp/rcpt-denied".to_string())
    );
    assert_eq!(
        protocol_dsl_path("smtp", Some("message-denied")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/smtp/data-denied".to_string())
    );
}

#[test]
fn ftp_entry_aliases_resolve_to_canonical_entries() {
    assert_eq!(
        protocol_dsl_path("ftp", Some("login")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/ftp/session".to_string())
    );
    assert_eq!(
        protocol_dsl_path("ftp", Some("control")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/ftp/session".to_string())
    );
    assert_eq!(
        protocol_dsl_path("ftp", Some("directory")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/ftp/list".to_string())
    );
    assert_eq!(
        protocol_dsl_path("ftp", Some("download")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/ftp/retr".to_string())
    );
    assert_eq!(
        protocol_dsl_path("ftp", Some("upload")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/ftp/stor".to_string())
    );
    assert_eq!(
        protocol_dsl_path("ftp", Some("active-directory")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/ftp/active-list".to_string())
    );
    assert_eq!(
        protocol_dsl_path("ftp", Some("active-download")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/ftp/active-retr".to_string())
    );
    assert_eq!(
        protocol_dsl_path("ftp", Some("active-upload")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/ftp/active-stor".to_string())
    );
    assert_eq!(
        protocol_dsl_path("ftp", Some("login-denied")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/ftp/denied".to_string())
    );
}

#[test]
fn auth_family_entry_aliases_resolve_to_canonical_entries() {
    assert_eq!(
        protocol_dsl_path("kerberos", Some("login")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/kerberos/as".to_string())
    );
    assert_eq!(
        protocol_dsl_path("kerberos", Some("initial-auth")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/kerberos/as".to_string())
    );
    assert_eq!(
        protocol_dsl_path("kerberos", Some("ticket")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/kerberos/tgs".to_string())
    );
    assert_eq!(
        protocol_dsl_path("kerberos", Some("service-ticket")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/kerberos/tgs".to_string())
    );
    assert_eq!(
        protocol_dsl_path("kerberos", Some("login-denied")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/kerberos/as-error".to_string())
    );
    assert_eq!(
        protocol_dsl_path("kerberos", Some("initial-auth-error")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/kerberos/as-error".to_string())
    );
    assert_eq!(
        protocol_dsl_path("radius", Some("login")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/radius/access".to_string())
    );
    assert_eq!(
        protocol_dsl_path("radius", Some("auth")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/radius/access".to_string())
    );
}

#[test]
fn access_and_messaging_entry_aliases_resolve_to_canonical_entries() {
    assert_eq!(
        protocol_dsl_path("ssh", Some("connect")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/ssh/session".to_string())
    );
    assert_eq!(
        protocol_dsl_path("ssh", Some("login")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/ssh/auth".to_string())
    );
    assert_eq!(
        protocol_dsl_path("ssh", Some("shell")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/ssh/channel".to_string())
    );
    assert_eq!(
        protocol_dsl_path("socks5", Some("proxy")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/socks5/session".to_string())
    );
    assert_eq!(
        protocol_dsl_path("socks5", Some("userpass")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/socks5/auth".to_string())
    );
    assert_eq!(
        protocol_dsl_path("socks5", Some("connect-denied")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/socks5/denied".to_string())
    );
    assert_eq!(
        protocol_dsl_path("socks5", Some("login-connect-denied")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/socks5/auth-connect-denied".to_string())
    );
    assert_eq!(
        protocol_dsl_path("ldap", Some("login")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/ldap/bind".to_string())
    );
    assert_eq!(
        protocol_dsl_path("ldap", Some("directory")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/ldap/search".to_string())
    );
    assert_eq!(
        protocol_dsl_path("ldap", Some("directory-session")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/ldap/session".to_string())
    );
    assert_eq!(
        protocol_dsl_path("ldap", Some("replication")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/ldap/sync".to_string())
    );
    assert_eq!(
        protocol_dsl_path("snmp", Some("query")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/snmp/get".to_string())
    );
    assert_eq!(
        protocol_dsl_path("sip", Some("login")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/sip/register".to_string())
    );
    assert_eq!(
        protocol_dsl_path("rtsp", Some("probe")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/rtsp/options".to_string())
    );
    assert_eq!(
        protocol_dsl_path("rtsp", Some("metadata")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/rtsp/describe".to_string())
    );
    assert_eq!(
        protocol_dsl_path("rtsp", Some("stream")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/rtsp/setup".to_string())
    );
    assert_eq!(
        protocol_dsl_path("amqp", Some("login")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/amqp/start".to_string())
    );
    assert_eq!(
        protocol_dsl_path("amqp", Some("connect")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/amqp/session".to_string())
    );
    assert_eq!(
        protocol_dsl_path("amqp", Some("send")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/amqp/publish".to_string())
    );
    assert_eq!(
        protocol_dsl_path("mqtt", Some("session")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/mqtt/connect".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("health")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/ping".to_string())
    );
}

#[test]
fn built_in_dsl_path_falls_back_to_packaged_share_root() {
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
        "name=http_request\nversion=0.10.0\nentry=main.gewy\nregister.protocol=http\nregister.entry=request\nregister.default=true\n",
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
        "name=mysql_session\nversion=0.10.0\nentry=main.gewy\nregister.protocol=mysql\nregister.entry=session\nregister.default=true\n",
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
