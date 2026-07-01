use super::ProtocolAlias;

pub(crate) const PROTOCOL_ALIASES: &[ProtocolAlias] = &[
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
        entry: Some("response"),
    },
    ProtocolAlias {
        alias: "http_server",
        protocol: "http",
        entry: Some("response"),
    },
    ProtocolAlias {
        alias: "h3-request",
        protocol: "http3",
        entry: Some("request"),
    },
    ProtocolAlias {
        alias: "h3_request",
        protocol: "http3",
        entry: Some("request"),
    },
    ProtocolAlias {
        alias: "http3-server-response",
        protocol: "http3",
        entry: Some("server"),
    },
    ProtocolAlias {
        alias: "http3_server_response",
        protocol: "http3",
        entry: Some("server"),
    },
    ProtocolAlias {
        alias: "h3-server",
        protocol: "http3",
        entry: Some("server"),
    },
    ProtocolAlias {
        alias: "h3_server",
        protocol: "http3",
        entry: Some("server"),
    },
    ProtocolAlias {
        alias: "aws-s3",
        protocol: "s3",
        entry: None,
    },
    ProtocolAlias {
        alias: "minio",
        protocol: "s3",
        entry: None,
    },
    ProtocolAlias {
        alias: "object-storage",
        protocol: "s3",
        entry: None,
    },
    ProtocolAlias {
        alias: "s3-list",
        protocol: "s3",
        entry: Some("list-buckets"),
    },
    ProtocolAlias {
        alias: "list-buckets",
        protocol: "s3",
        entry: Some("list-buckets"),
    },
    ProtocolAlias {
        alias: "s3-head",
        protocol: "s3",
        entry: Some("head-object"),
    },
    ProtocolAlias {
        alias: "head-object",
        protocol: "s3",
        entry: Some("head-object"),
    },
    ProtocolAlias {
        alias: "s3-put",
        protocol: "s3",
        entry: Some("put-object"),
    },
    ProtocolAlias {
        alias: "put-object",
        protocol: "s3",
        entry: Some("put-object"),
    },
    ProtocolAlias {
        alias: "s3-get",
        protocol: "s3",
        entry: Some("get-object"),
    },
    ProtocolAlias {
        alias: "get-object",
        protocol: "s3",
        entry: Some("get-object"),
    },
    ProtocolAlias {
        alias: "s3-delete",
        protocol: "s3",
        entry: Some("delete-object"),
    },
    ProtocolAlias {
        alias: "delete-object",
        protocol: "s3",
        entry: Some("delete-object"),
    },
    ProtocolAlias {
        alias: "hy2-auth",
        protocol: "hy2",
        entry: Some("auth"),
    },
    ProtocolAlias {
        alias: "hysteria2-auth",
        protocol: "hy2",
        entry: Some("auth"),
    },
    ProtocolAlias {
        alias: "hysteria2",
        protocol: "hy2",
        entry: Some("auth"),
    },
    ProtocolAlias {
        alias: "hy2-tcp",
        protocol: "hy2",
        entry: Some("tcp"),
    },
    ProtocolAlias {
        alias: "hy2-stream",
        protocol: "hy2",
        entry: Some("tcp"),
    },
    ProtocolAlias {
        alias: "hysteria2-tcp",
        protocol: "hy2",
        entry: Some("tcp"),
    },
    ProtocolAlias {
        alias: "hy2-udp",
        protocol: "hy2",
        entry: Some("udp"),
    },
    ProtocolAlias {
        alias: "hy2-relay",
        protocol: "hy2",
        entry: Some("udp"),
    },
    ProtocolAlias {
        alias: "hysteria2-udp",
        protocol: "hy2",
        entry: Some("udp"),
    },
    ProtocolAlias {
        alias: "radius-challenge",
        protocol: "radius",
        entry: Some("challenge"),
    },
    ProtocolAlias {
        alias: "radius_challenge",
        protocol: "radius",
        entry: Some("challenge"),
    },
    ProtocolAlias {
        alias: "radius-denied",
        protocol: "radius",
        entry: Some("denied"),
    },
    ProtocolAlias {
        alias: "radius_denied",
        protocol: "radius",
        entry: Some("denied"),
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
        alias: "postgres-auth-denied",
        protocol: "postgres",
        entry: Some("auth-denied"),
    },
    ProtocolAlias {
        alias: "postgres_auth_denied",
        protocol: "postgres",
        entry: Some("auth-denied"),
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
        alias: "mysql-auth",
        protocol: "mysql",
        entry: Some("auth"),
    },
    ProtocolAlias {
        alias: "mysql_auth",
        protocol: "mysql",
        entry: Some("auth"),
    },
    ProtocolAlias {
        alias: "mysql-auth-denied",
        protocol: "mysql",
        entry: Some("auth-denied"),
    },
    ProtocolAlias {
        alias: "mysql_auth_denied",
        protocol: "mysql",
        entry: Some("auth-denied"),
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
        alias: "memcached-miss",
        protocol: "memcached",
        entry: Some("miss"),
    },
    ProtocolAlias {
        alias: "memcached_miss",
        protocol: "memcached",
        entry: Some("miss"),
    },
    ProtocolAlias {
        alias: "memcached-not-stored",
        protocol: "memcached",
        entry: Some("not-stored"),
    },
    ProtocolAlias {
        alias: "memcached_not_stored",
        protocol: "memcached",
        entry: Some("not-stored"),
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
        alias: "amqp-consume",
        protocol: "amqp",
        entry: Some("consume"),
    },
    ProtocolAlias {
        alias: "amqp_consume",
        protocol: "amqp",
        entry: Some("consume"),
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
        alias: "redis-session",
        protocol: "redis",
        entry: Some("session"),
    },
    ProtocolAlias {
        alias: "redis_session",
        protocol: "redis",
        entry: Some("session"),
    },
    ProtocolAlias {
        alias: "redis-get",
        protocol: "redis",
        entry: Some("get"),
    },
    ProtocolAlias {
        alias: "redis_get",
        protocol: "redis",
        entry: Some("get"),
    },
    ProtocolAlias {
        alias: "redis-set",
        protocol: "redis",
        entry: Some("set"),
    },
    ProtocolAlias {
        alias: "redis_set",
        protocol: "redis",
        entry: Some("set"),
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
    ProtocolAlias {
        alias: "socks",
        protocol: "socks5",
        entry: Some("session"),
    },
    ProtocolAlias {
        alias: "socks5-session",
        protocol: "socks5",
        entry: Some("session"),
    },
    ProtocolAlias {
        alias: "socks5_session",
        protocol: "socks5",
        entry: Some("session"),
    },
];
