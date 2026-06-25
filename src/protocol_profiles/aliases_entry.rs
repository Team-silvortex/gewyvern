use super::ProtocolAlias;

pub(crate) const PROTOCOL_ENTRY_ALIASES: &[ProtocolAlias] = &[
    ProtocolAlias {
        alias: "login",
        protocol: "ftp",
        entry: Some("session"),
    },
    ProtocolAlias {
        alias: "control",
        protocol: "ftp",
        entry: Some("session"),
    },
    ProtocolAlias {
        alias: "directory",
        protocol: "ftp",
        entry: Some("list"),
    },
    ProtocolAlias {
        alias: "download",
        protocol: "ftp",
        entry: Some("retr"),
    },
    ProtocolAlias {
        alias: "upload",
        protocol: "ftp",
        entry: Some("stor"),
    },
    ProtocolAlias {
        alias: "active-directory",
        protocol: "ftp",
        entry: Some("active-list"),
    },
    ProtocolAlias {
        alias: "active-download",
        protocol: "ftp",
        entry: Some("active-retr"),
    },
    ProtocolAlias {
        alias: "active-upload",
        protocol: "ftp",
        entry: Some("active-stor"),
    },
    ProtocolAlias {
        alias: "login-denied",
        protocol: "ftp",
        entry: Some("denied"),
    },
    ProtocolAlias {
        alias: "login",
        protocol: "kerberos",
        entry: Some("as"),
    },
    ProtocolAlias {
        alias: "initial-auth",
        protocol: "kerberos",
        entry: Some("as"),
    },
    ProtocolAlias {
        alias: "login-denied",
        protocol: "kerberos",
        entry: Some("as-error"),
    },
    ProtocolAlias {
        alias: "initial-auth-error",
        protocol: "kerberos",
        entry: Some("as-error"),
    },
    ProtocolAlias {
        alias: "ticket",
        protocol: "kerberos",
        entry: Some("tgs"),
    },
    ProtocolAlias {
        alias: "service-ticket",
        protocol: "kerberos",
        entry: Some("tgs"),
    },
    ProtocolAlias {
        alias: "login",
        protocol: "ldap",
        entry: Some("bind"),
    },
    ProtocolAlias {
        alias: "auth",
        protocol: "ldap",
        entry: Some("bind"),
    },
    ProtocolAlias {
        alias: "login-denied",
        protocol: "ldap",
        entry: Some("bind-denied"),
    },
    ProtocolAlias {
        alias: "auth-denied",
        protocol: "ldap",
        entry: Some("bind-denied"),
    },
    ProtocolAlias {
        alias: "directory",
        protocol: "ldap",
        entry: Some("search"),
    },
    ProtocolAlias {
        alias: "query",
        protocol: "ldap",
        entry: Some("search"),
    },
    ProtocolAlias {
        alias: "directory-session",
        protocol: "ldap",
        entry: Some("session"),
    },
    ProtocolAlias {
        alias: "replication",
        protocol: "ldap",
        entry: Some("sync"),
    },
    ProtocolAlias {
        alias: "connect",
        protocol: "amqp",
        entry: Some("session"),
    },
    ProtocolAlias {
        alias: "login",
        protocol: "amqp",
        entry: Some("start"),
    },
    ProtocolAlias {
        alias: "negotiate",
        protocol: "amqp",
        entry: Some("start"),
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
        alias: "send",
        protocol: "amqp",
        entry: Some("publish"),
    },
    ProtocolAlias {
        alias: "receive",
        protocol: "amqp",
        entry: Some("consume"),
    },
    ProtocolAlias {
        alias: "deliver",
        protocol: "amqp",
        entry: Some("consume"),
    },
    ProtocolAlias {
        alias: "address-validation",
        protocol: "quic",
        entry: Some("retry"),
    },
    ProtocolAlias {
        alias: "token-challenge",
        protocol: "quic",
        entry: Some("retry"),
    },
    ProtocolAlias {
        alias: "quic-retry",
        protocol: "quic",
        entry: Some("retry"),
    },
    ProtocolAlias {
        alias: "quic_retry",
        protocol: "quic",
        entry: Some("retry"),
    },
    ProtocolAlias {
        alias: "terminate",
        protocol: "quic",
        entry: Some("close"),
    },
    ProtocolAlias {
        alias: "connection-close",
        protocol: "quic",
        entry: Some("close"),
    },
    ProtocolAlias {
        alias: "connection_close",
        protocol: "quic",
        entry: Some("close"),
    },
    ProtocolAlias {
        alias: "quic-close",
        protocol: "quic",
        entry: Some("close"),
    },
    ProtocolAlias {
        alias: "quic_close",
        protocol: "quic",
        entry: Some("close"),
    },
    ProtocolAlias {
        alias: "local-close",
        protocol: "quic",
        entry: Some("local-close"),
    },
    ProtocolAlias {
        alias: "local_close",
        protocol: "quic",
        entry: Some("local-close"),
    },
    ProtocolAlias {
        alias: "quic-local-close",
        protocol: "quic",
        entry: Some("local-close"),
    },
    ProtocolAlias {
        alias: "quic_local_close",
        protocol: "quic",
        entry: Some("local-close"),
    },
    ProtocolAlias {
        alias: "active-close",
        protocol: "quic",
        entry: Some("local-close"),
    },
    ProtocolAlias {
        alias: "active_close",
        protocol: "quic",
        entry: Some("local-close"),
    },
    ProtocolAlias {
        alias: "terminate",
        protocol: "http3",
        entry: Some("close"),
    },
    ProtocolAlias {
        alias: "connection-close",
        protocol: "http3",
        entry: Some("close"),
    },
    ProtocolAlias {
        alias: "connection_close",
        protocol: "http3",
        entry: Some("close"),
    },
    ProtocolAlias {
        alias: "h3-close",
        protocol: "http3",
        entry: Some("close"),
    },
    ProtocolAlias {
        alias: "h3_close",
        protocol: "http3",
        entry: Some("close"),
    },
    ProtocolAlias {
        alias: "http3-close",
        protocol: "http3",
        entry: Some("close"),
    },
    ProtocolAlias {
        alias: "http3_close",
        protocol: "http3",
        entry: Some("close"),
    },
    ProtocolAlias {
        alias: "server-close",
        protocol: "http3",
        entry: Some("server-close"),
    },
    ProtocolAlias {
        alias: "server_close",
        protocol: "http3",
        entry: Some("server-close"),
    },
    ProtocolAlias {
        alias: "h3-server-close",
        protocol: "http3",
        entry: Some("server-close"),
    },
    ProtocolAlias {
        alias: "h3_server_close",
        protocol: "http3",
        entry: Some("server-close"),
    },
    ProtocolAlias {
        alias: "http3-server-close",
        protocol: "http3",
        entry: Some("server-close"),
    },
    ProtocolAlias {
        alias: "http3_server_close",
        protocol: "http3",
        entry: Some("server-close"),
    },
    ProtocolAlias {
        alias: "response-close",
        protocol: "http3",
        entry: Some("server-close"),
    },
    ProtocolAlias {
        alias: "response_close",
        protocol: "http3",
        entry: Some("server-close"),
    },
    ProtocolAlias {
        alias: "terminate",
        protocol: "hy2",
        entry: Some("close"),
    },
    ProtocolAlias {
        alias: "session-close",
        protocol: "hy2",
        entry: Some("close"),
    },
    ProtocolAlias {
        alias: "session_close",
        protocol: "hy2",
        entry: Some("close"),
    },
    ProtocolAlias {
        alias: "hy2-close",
        protocol: "hy2",
        entry: Some("close"),
    },
    ProtocolAlias {
        alias: "hy2_close",
        protocol: "hy2",
        entry: Some("close"),
    },
    ProtocolAlias {
        alias: "hysteria2-close",
        protocol: "hy2",
        entry: Some("close"),
    },
    ProtocolAlias {
        alias: "hysteria2_close",
        protocol: "hy2",
        entry: Some("close"),
    },
    ProtocolAlias {
        alias: "tcp-close",
        protocol: "hy2",
        entry: Some("tcp-close"),
    },
    ProtocolAlias {
        alias: "tcp_close",
        protocol: "hy2",
        entry: Some("tcp-close"),
    },
    ProtocolAlias {
        alias: "hy2-tcp-close",
        protocol: "hy2",
        entry: Some("tcp-close"),
    },
    ProtocolAlias {
        alias: "hy2_tcp_close",
        protocol: "hy2",
        entry: Some("tcp-close"),
    },
    ProtocolAlias {
        alias: "hysteria2-tcp-close",
        protocol: "hy2",
        entry: Some("tcp-close"),
    },
    ProtocolAlias {
        alias: "hysteria2_tcp_close",
        protocol: "hy2",
        entry: Some("tcp-close"),
    },
    ProtocolAlias {
        alias: "stream-close",
        protocol: "hy2",
        entry: Some("tcp-close"),
    },
    ProtocolAlias {
        alias: "stream_close",
        protocol: "hy2",
        entry: Some("tcp-close"),
    },
    ProtocolAlias {
        alias: "udp-close",
        protocol: "hy2",
        entry: Some("udp-close"),
    },
    ProtocolAlias {
        alias: "udp_close",
        protocol: "hy2",
        entry: Some("udp-close"),
    },
    ProtocolAlias {
        alias: "hy2-udp-close",
        protocol: "hy2",
        entry: Some("udp-close"),
    },
    ProtocolAlias {
        alias: "hy2_udp_close",
        protocol: "hy2",
        entry: Some("udp-close"),
    },
    ProtocolAlias {
        alias: "hysteria2-udp-close",
        protocol: "hy2",
        entry: Some("udp-close"),
    },
    ProtocolAlias {
        alias: "hysteria2_udp_close",
        protocol: "hy2",
        entry: Some("udp-close"),
    },
    ProtocolAlias {
        alias: "datagram-close",
        protocol: "hy2",
        entry: Some("udp-close"),
    },
    ProtocolAlias {
        alias: "datagram_close",
        protocol: "hy2",
        entry: Some("udp-close"),
    },
    ProtocolAlias {
        alias: "initiator",
        protocol: "tls",
        entry: Some("client"),
    },
    ProtocolAlias {
        alias: "tls-client",
        protocol: "tls",
        entry: Some("client"),
    },
    ProtocolAlias {
        alias: "tls_client",
        protocol: "tls",
        entry: Some("client"),
    },
    ProtocolAlias {
        alias: "acceptor",
        protocol: "tls",
        entry: Some("server"),
    },
    ProtocolAlias {
        alias: "tls-server",
        protocol: "tls",
        entry: Some("server"),
    },
    ProtocolAlias {
        alias: "tls_server",
        protocol: "tls",
        entry: Some("server"),
    },
    ProtocolAlias {
        alias: "health",
        protocol: "redis",
        entry: Some("ping"),
    },
    ProtocolAlias {
        alias: "connect",
        protocol: "redis",
        entry: Some("session"),
    },
    ProtocolAlias {
        alias: "roundtrip",
        protocol: "redis",
        entry: Some("session"),
    },
    ProtocolAlias {
        alias: "read",
        protocol: "redis",
        entry: Some("get"),
    },
    ProtocolAlias {
        alias: "kv-read",
        protocol: "redis",
        entry: Some("get"),
    },
    ProtocolAlias {
        alias: "write",
        protocol: "redis",
        entry: Some("set"),
    },
    ProtocolAlias {
        alias: "kv-write",
        protocol: "redis",
        entry: Some("set"),
    },
    ProtocolAlias {
        alias: "stream-group-read",
        protocol: "redis",
        entry: Some("xreadgroup"),
    },
    ProtocolAlias {
        alias: "stream-consumer-read",
        protocol: "redis",
        entry: Some("xreadgroup"),
    },
    ProtocolAlias {
        alias: "stream-read",
        protocol: "redis",
        entry: Some("xread"),
    },
    ProtocolAlias {
        alias: "stream-consume",
        protocol: "redis",
        entry: Some("xread"),
    },
    ProtocolAlias {
        alias: "pubsub-send",
        protocol: "redis",
        entry: Some("publish"),
    },
    ProtocolAlias {
        alias: "channel-write",
        protocol: "redis",
        entry: Some("publish"),
    },
    ProtocolAlias {
        alias: "pubsub-listen",
        protocol: "redis",
        entry: Some("subscribe"),
    },
    ProtocolAlias {
        alias: "channel-read",
        protocol: "redis",
        entry: Some("subscribe"),
    },
    ProtocolAlias {
        alias: "sorted-add",
        protocol: "redis",
        entry: Some("zadd"),
    },
    ProtocolAlias {
        alias: "score-add",
        protocol: "redis",
        entry: Some("zadd"),
    },
    ProtocolAlias {
        alias: "list-blocking-move",
        protocol: "redis",
        entry: Some("blmove"),
    },
    ProtocolAlias {
        alias: "list-blocking-directional-move",
        protocol: "redis",
        entry: Some("blmove"),
    },
    ProtocolAlias {
        alias: "blocking-left-right-move",
        protocol: "redis",
        entry: Some("blmove"),
    },
    ProtocolAlias {
        alias: "blocking-right-left-move",
        protocol: "redis",
        entry: Some("blmove"),
    },
    ProtocolAlias {
        alias: "sorted-blocking-pop-max",
        protocol: "redis",
        entry: Some("bzpopmax"),
    },
    ProtocolAlias {
        alias: "score-blocking-pop-highest",
        protocol: "redis",
        entry: Some("bzpopmax"),
    },
];
