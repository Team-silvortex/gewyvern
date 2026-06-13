use super::{protocol_dsl_path, protocol_entries, protocol_summary, resolve_built_in_dsl_path};
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
