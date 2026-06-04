#[derive(Clone, Copy)]
pub(super) struct ProtocolAlias {
    pub(super) alias: &'static str,
    pub(super) protocol: &'static str,
    pub(super) entry: Option<&'static str>,
}
pub(super) const PROTOCOL_ALIASES: &[ProtocolAlias] = &[
    ProtocolAlias {
        alias: "dns-tcp",
        protocol: "dns",
        entry: Some("tcp"),
    },
    ProtocolAlias {
        alias: "dns_tcp",
        protocol: "dns",
        entry: Some("tcp"),
    },
    ProtocolAlias {
        alias: "http-request",
        protocol: "http",
        entry: Some("request"),
    },
    ProtocolAlias {
        alias: "http_request",
        protocol: "http",
        entry: Some("request"),
    },
    ProtocolAlias {
        alias: "http-client",
        protocol: "http",
        entry: Some("request"),
    },
    ProtocolAlias {
        alias: "http_client",
        protocol: "http",
        entry: Some("request"),
    },
    ProtocolAlias {
        alias: "http-server",
        protocol: "http",
        entry: Some("server"),
    },
    ProtocolAlias {
        alias: "http_server",
        protocol: "http",
        entry: Some("server"),
    },
    ProtocolAlias {
        alias: "postgres-connect",
        protocol: "postgres",
        entry: Some("connect"),
    },
    ProtocolAlias {
        alias: "postgres_connect",
        protocol: "postgres",
        entry: Some("connect"),
    },
    ProtocolAlias {
        alias: "postgres-auth",
        protocol: "postgres",
        entry: Some("auth"),
    },
    ProtocolAlias {
        alias: "postgres_auth",
        protocol: "postgres",
        entry: Some("auth"),
    },
    ProtocolAlias {
        alias: "postgres-query",
        protocol: "postgres",
        entry: Some("query"),
    },
    ProtocolAlias {
        alias: "postgres_query",
        protocol: "postgres",
        entry: Some("query"),
    },
    ProtocolAlias {
        alias: "postgres-error",
        protocol: "postgres",
        entry: Some("error"),
    },
    ProtocolAlias {
        alias: "postgres_error",
        protocol: "postgres",
        entry: Some("error"),
    },
    ProtocolAlias {
        alias: "mysql-connect",
        protocol: "mysql",
        entry: Some("connect"),
    },
    ProtocolAlias {
        alias: "mysql_connect",
        protocol: "mysql",
        entry: Some("connect"),
    },
    ProtocolAlias {
        alias: "mysql-query",
        protocol: "mysql",
        entry: Some("query"),
    },
    ProtocolAlias {
        alias: "mysql_query",
        protocol: "mysql",
        entry: Some("query"),
    },
    ProtocolAlias {
        alias: "mysql-session",
        protocol: "mysql",
        entry: Some("session"),
    },
    ProtocolAlias {
        alias: "mysql_session",
        protocol: "mysql",
        entry: Some("session"),
    },
    ProtocolAlias {
        alias: "mysql-error",
        protocol: "mysql",
        entry: Some("error"),
    },
    ProtocolAlias {
        alias: "mysql_error",
        protocol: "mysql",
        entry: Some("error"),
    },
    ProtocolAlias {
        alias: "memcached-get",
        protocol: "memcached",
        entry: Some("get"),
    },
    ProtocolAlias {
        alias: "memcached_get",
        protocol: "memcached",
        entry: Some("get"),
    },
    ProtocolAlias {
        alias: "memcached-set",
        protocol: "memcached",
        entry: Some("set"),
    },
    ProtocolAlias {
        alias: "memcached_set",
        protocol: "memcached",
        entry: Some("set"),
    },
    ProtocolAlias {
        alias: "amqp-start",
        protocol: "amqp",
        entry: Some("start"),
    },
    ProtocolAlias {
        alias: "amqp_start",
        protocol: "amqp",
        entry: Some("start"),
    },
    ProtocolAlias {
        alias: "amqp-publish",
        protocol: "amqp",
        entry: Some("publish"),
    },
    ProtocolAlias {
        alias: "amqp_publish",
        protocol: "amqp",
        entry: Some("publish"),
    },
    ProtocolAlias {
        alias: "amqp-session",
        protocol: "amqp",
        entry: Some("session"),
    },
    ProtocolAlias {
        alias: "amqp_session",
        protocol: "amqp",
        entry: Some("session"),
    },
    ProtocolAlias {
        alias: "redis-ping",
        protocol: "redis",
        entry: Some("ping"),
    },
    ProtocolAlias {
        alias: "redis_ping",
        protocol: "redis",
        entry: Some("ping"),
    },
    ProtocolAlias {
        alias: "gtp-u",
        protocol: "gtpu",
        entry: Some("echo"),
    },
    ProtocolAlias {
        alias: "gtp_u",
        protocol: "gtpu",
        entry: Some("echo"),
    },
    ProtocolAlias {
        alias: "ldap-bind",
        protocol: "ldap",
        entry: Some("bind"),
    },
    ProtocolAlias {
        alias: "ldap_bind",
        protocol: "ldap",
        entry: Some("bind"),
    },
    ProtocolAlias {
        alias: "ldap-search",
        protocol: "ldap",
        entry: Some("search"),
    },
    ProtocolAlias {
        alias: "ldap_search",
        protocol: "ldap",
        entry: Some("search"),
    },
    ProtocolAlias {
        alias: "ldap-modify",
        protocol: "ldap",
        entry: Some("modify"),
    },
    ProtocolAlias {
        alias: "ldap_modify",
        protocol: "ldap",
        entry: Some("modify"),
    },
    ProtocolAlias {
        alias: "ldap-denied",
        protocol: "ldap",
        entry: Some("denied"),
    },
    ProtocolAlias {
        alias: "ldap_denied",
        protocol: "ldap",
        entry: Some("denied"),
    },
    ProtocolAlias {
        alias: "ldap-constraint",
        protocol: "ldap",
        entry: Some("constraint"),
    },
    ProtocolAlias {
        alias: "ldap_constraint",
        protocol: "ldap",
        entry: Some("constraint"),
    },
    ProtocolAlias {
        alias: "ldap-session",
        protocol: "ldap",
        entry: Some("session"),
    },
    ProtocolAlias {
        alias: "ldap_session",
        protocol: "ldap",
        entry: Some("session"),
    },
    ProtocolAlias {
        alias: "ldap-write",
        protocol: "ldap",
        entry: Some("write"),
    },
    ProtocolAlias {
        alias: "ldap_write",
        protocol: "ldap",
        entry: Some("write"),
    },
    ProtocolAlias {
        alias: "ldap-sync",
        protocol: "ldap",
        entry: Some("sync"),
    },
    ProtocolAlias {
        alias: "ldap_sync",
        protocol: "ldap",
        entry: Some("sync"),
    },
];

pub(super) fn split_protocol_alias(protocol: &str) -> (&str, Option<&str>) {
    PROTOCOL_ALIASES
        .iter()
        .find(|alias| alias.alias == protocol)
        .map(|alias| (alias.protocol, alias.entry))
        .unwrap_or((protocol, None))
}
