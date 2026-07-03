use super::ProtocolAlias;

pub(crate) const PROTOCOL_ENTRY_ALIASES_MANIFEST: &[ProtocolAlias] = &[
    ProtocolAlias {
        alias: "dot",
        protocol: "dns",
        entry: Some("tcp"),
    },
    ProtocolAlias {
        alias: "dns-over-tls",
        protocol: "dns",
        entry: Some("tcp"),
    },
    ProtocolAlias {
        alias: "dns_over_tls",
        protocol: "dns",
        entry: Some("tcp"),
    },
    ProtocolAlias {
        alias: "dns-error",
        protocol: "dns",
        entry: Some("error"),
    },
    ProtocolAlias {
        alias: "dns_error",
        protocol: "dns",
        entry: Some("error"),
    },
    ProtocolAlias {
        alias: "nxdomain",
        protocol: "dns",
        entry: Some("error"),
    },
    ProtocolAlias {
        alias: "servfail",
        protocol: "dns",
        entry: Some("error"),
    },
    ProtocolAlias {
        alias: "refused",
        protocol: "dns",
        entry: Some("error"),
    },
    ProtocolAlias {
        alias: "formerr",
        protocol: "dns",
        entry: Some("error"),
    },
    ProtocolAlias {
        alias: "resolution-failed",
        protocol: "dns",
        entry: Some("error"),
    },
    ProtocolAlias {
        alias: "resolution_failed",
        protocol: "dns",
        entry: Some("error"),
    },
    ProtocolAlias {
        alias: "dns-tcp-error",
        protocol: "dns",
        entry: Some("tcp-error"),
    },
    ProtocolAlias {
        alias: "dns_tcp_error",
        protocol: "dns",
        entry: Some("tcp-error"),
    },
    ProtocolAlias {
        alias: "tcp-nxdomain",
        protocol: "dns",
        entry: Some("tcp-error"),
    },
    ProtocolAlias {
        alias: "tcp-servfail",
        protocol: "dns",
        entry: Some("tcp-error"),
    },
    ProtocolAlias {
        alias: "tcp-refused",
        protocol: "dns",
        entry: Some("tcp-error"),
    },
    ProtocolAlias {
        alias: "tcp-formerr",
        protocol: "dns",
        entry: Some("tcp-error"),
    },
    ProtocolAlias {
        alias: "doh",
        protocol: "http",
        entry: Some("request"),
    },
    ProtocolAlias {
        alias: "dns-over-https",
        protocol: "http",
        entry: Some("request"),
    },
    ProtocolAlias {
        alias: "dns_over_https",
        protocol: "http",
        entry: Some("request"),
    },
    ProtocolAlias {
        alias: "http-connect-auth-required",
        protocol: "http",
        entry: Some("auth-required"),
    },
    ProtocolAlias {
        alias: "http_connect_auth_required",
        protocol: "http",
        entry: Some("auth-required"),
    },
    ProtocolAlias {
        alias: "http-connect-auth-tunnel",
        protocol: "http",
        entry: Some("auth-tunnel"),
    },
    ProtocolAlias {
        alias: "http_connect_auth_tunnel",
        protocol: "http",
        entry: Some("auth-tunnel"),
    },
    ProtocolAlias {
        alias: "http-connect-denied",
        protocol: "http",
        entry: Some("denied"),
    },
    ProtocolAlias {
        alias: "http_connect_denied",
        protocol: "http",
        entry: Some("denied"),
    },
    ProtocolAlias {
        alias: "http-connect",
        protocol: "http",
        entry: Some("connect"),
    },
    ProtocolAlias {
        alias: "http_connect",
        protocol: "http",
        entry: Some("connect"),
    },
    ProtocolAlias {
        alias: "server",
        protocol: "http",
        entry: Some("response"),
    },
    ProtocolAlias {
        alias: "client",
        protocol: "http",
        entry: Some("request"),
    },
    ProtocolAlias {
        alias: "amqp-auth-denied",
        protocol: "amqp",
        entry: Some("auth-denied"),
    },
    ProtocolAlias {
        alias: "amqp_auth_denied",
        protocol: "amqp",
        entry: Some("auth-denied"),
    },
    ProtocolAlias {
        alias: "login-denied",
        protocol: "amqp",
        entry: Some("auth-denied"),
    },
    ProtocolAlias {
        alias: "negotiate-denied",
        protocol: "amqp",
        entry: Some("auth-denied"),
    },
    ProtocolAlias {
        alias: "imap-auth",
        protocol: "imap",
        entry: Some("auth"),
    },
    ProtocolAlias {
        alias: "imap_auth",
        protocol: "imap",
        entry: Some("auth"),
    },
    ProtocolAlias {
        alias: "login",
        protocol: "imap",
        entry: Some("auth"),
    },
    ProtocolAlias {
        alias: "imap-auth-denied",
        protocol: "imap",
        entry: Some("auth-denied"),
    },
    ProtocolAlias {
        alias: "imap_auth_denied",
        protocol: "imap",
        entry: Some("auth-denied"),
    },
    ProtocolAlias {
        alias: "login-denied",
        protocol: "imap",
        entry: Some("auth-denied"),
    },
    ProtocolAlias {
        alias: "imap-select",
        protocol: "imap",
        entry: Some("select"),
    },
    ProtocolAlias {
        alias: "imap_select",
        protocol: "imap",
        entry: Some("select"),
    },
    ProtocolAlias {
        alias: "mailbox",
        protocol: "imap",
        entry: Some("select"),
    },
    ProtocolAlias {
        alias: "ldap-bind-denied",
        protocol: "ldap",
        entry: Some("bind-denied"),
    },
    ProtocolAlias {
        alias: "ldap_bind_denied",
        protocol: "ldap",
        entry: Some("bind-denied"),
    },
    ProtocolAlias {
        alias: "stun-refresh",
        protocol: "stun",
        entry: Some("refresh"),
    },
    ProtocolAlias {
        alias: "stun_refresh",
        protocol: "stun",
        entry: Some("refresh"),
    },
    ProtocolAlias {
        alias: "keepalive",
        protocol: "stun",
        entry: Some("refresh"),
    },
    ProtocolAlias {
        alias: "turn-refresh",
        protocol: "stun",
        entry: Some("refresh"),
    },
    ProtocolAlias {
        alias: "stun-allocate",
        protocol: "stun",
        entry: Some("allocate"),
    },
    ProtocolAlias {
        alias: "stun_allocate",
        protocol: "stun",
        entry: Some("allocate"),
    },
    ProtocolAlias {
        alias: "relay",
        protocol: "stun",
        entry: Some("allocate"),
    },
    ProtocolAlias {
        alias: "turn-allocate",
        protocol: "stun",
        entry: Some("allocate"),
    },
    ProtocolAlias {
        alias: "ntp-sync",
        protocol: "ntp",
        entry: Some("sync"),
    },
    ProtocolAlias {
        alias: "ntp_sync",
        protocol: "ntp",
        entry: Some("sync"),
    },
    ProtocolAlias {
        alias: "clock-sync",
        protocol: "ntp",
        entry: Some("sync"),
    },
    ProtocolAlias {
        alias: "time-sync",
        protocol: "ntp",
        entry: Some("sync"),
    },
    ProtocolAlias {
        alias: "ntp-query",
        protocol: "ntp",
        entry: Some("query"),
    },
    ProtocolAlias {
        alias: "ntp_query",
        protocol: "ntp",
        entry: Some("query"),
    },
    ProtocolAlias {
        alias: "probe",
        protocol: "ntp",
        entry: Some("query"),
    },
    ProtocolAlias {
        alias: "check",
        protocol: "ntp",
        entry: Some("query"),
    },
    ProtocolAlias {
        alias: "memcached-read",
        protocol: "memcached",
        entry: Some("get"),
    },
    ProtocolAlias {
        alias: "memcached_read",
        protocol: "memcached",
        entry: Some("get"),
    },
    ProtocolAlias {
        alias: "read",
        protocol: "memcached",
        entry: Some("get"),
    },
    ProtocolAlias {
        alias: "cache-miss",
        protocol: "memcached",
        entry: Some("miss"),
    },
    ProtocolAlias {
        alias: "cache_miss",
        protocol: "memcached",
        entry: Some("miss"),
    },
    ProtocolAlias {
        alias: "not-found",
        protocol: "memcached",
        entry: Some("miss"),
    },
    ProtocolAlias {
        alias: "not_found",
        protocol: "memcached",
        entry: Some("miss"),
    },
    ProtocolAlias {
        alias: "memcached-write",
        protocol: "memcached",
        entry: Some("set"),
    },
    ProtocolAlias {
        alias: "memcached_write",
        protocol: "memcached",
        entry: Some("set"),
    },
    ProtocolAlias {
        alias: "write",
        protocol: "memcached",
        entry: Some("set"),
    },
    ProtocolAlias {
        alias: "not_stored",
        protocol: "memcached",
        entry: Some("not-stored"),
    },
    ProtocolAlias {
        alias: "store-miss",
        protocol: "memcached",
        entry: Some("not-stored"),
    },
    ProtocolAlias {
        alias: "store_miss",
        protocol: "memcached",
        entry: Some("not-stored"),
    },
    ProtocolAlias {
        alias: "write-miss",
        protocol: "memcached",
        entry: Some("not-stored"),
    },
    ProtocolAlias {
        alias: "write_miss",
        protocol: "memcached",
        entry: Some("not-stored"),
    },
    ProtocolAlias {
        alias: "postgres-session",
        protocol: "postgres",
        entry: Some("session"),
    },
    ProtocolAlias {
        alias: "postgres_session",
        protocol: "postgres",
        entry: Some("session"),
    },
    ProtocolAlias {
        alias: "query-session",
        protocol: "postgres",
        entry: Some("session"),
    },
    ProtocolAlias {
        alias: "auth-query",
        protocol: "postgres",
        entry: Some("session"),
    },
];
