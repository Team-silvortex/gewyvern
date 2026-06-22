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
    ProtocolAlias {
        alias: "session",
        protocol: "mqtt",
        entry: Some("connect"),
    },
    ProtocolAlias {
        alias: "login",
        protocol: "mqtt",
        entry: Some("connect"),
    },
    ProtocolAlias {
        alias: "send",
        protocol: "mqtt",
        entry: Some("publish"),
    },
    ProtocolAlias {
        alias: "message",
        protocol: "mqtt",
        entry: Some("publish"),
    },
    ProtocolAlias {
        alias: "read",
        protocol: "mqtt",
        entry: Some("subscribe"),
    },
    ProtocolAlias {
        alias: "listen",
        protocol: "mqtt",
        entry: Some("subscribe"),
    },
    ProtocolAlias {
        alias: "close",
        protocol: "mqtt",
        entry: Some("disconnect"),
    },
    ProtocolAlias {
        alias: "teardown",
        protocol: "mqtt",
        entry: Some("disconnect"),
    },
    ProtocolAlias {
        alias: "qos2-receipt",
        protocol: "mqtt",
        entry: Some("pubrec"),
    },
    ProtocolAlias {
        alias: "stage-2",
        protocol: "mqtt",
        entry: Some("pubrec"),
    },
    ProtocolAlias {
        alias: "qos2-release",
        protocol: "mqtt",
        entry: Some("pubrel"),
    },
    ProtocolAlias {
        alias: "resume",
        protocol: "mqtt",
        entry: Some("pubrel"),
    },
    ProtocolAlias {
        alias: "qos2-complete",
        protocol: "mqtt",
        entry: Some("pubcomp"),
    },
    ProtocolAlias {
        alias: "complete",
        protocol: "mqtt",
        entry: Some("pubcomp"),
    },
    ProtocolAlias {
        alias: "login",
        protocol: "radius",
        entry: Some("access"),
    },
    ProtocolAlias {
        alias: "auth",
        protocol: "radius",
        entry: Some("access"),
    },
    ProtocolAlias {
        alias: "radius-access",
        protocol: "radius",
        entry: Some("access"),
    },
    ProtocolAlias {
        alias: "radius_access",
        protocol: "radius",
        entry: Some("access"),
    },
    ProtocolAlias {
        alias: "otp",
        protocol: "radius",
        entry: Some("challenge"),
    },
    ProtocolAlias {
        alias: "mfa",
        protocol: "radius",
        entry: Some("challenge"),
    },
    ProtocolAlias {
        alias: "access-denied",
        protocol: "radius",
        entry: Some("denied"),
    },
    ProtocolAlias {
        alias: "login-denied",
        protocol: "radius",
        entry: Some("denied"),
    },
    ProtocolAlias {
        alias: "reject",
        protocol: "radius",
        entry: Some("denied"),
    },
    ProtocolAlias {
        alias: "cookie-reply",
        protocol: "wireguard",
        entry: Some("cookie"),
    },
    ProtocolAlias {
        alias: "wireguard-cookie",
        protocol: "wireguard",
        entry: Some("cookie"),
    },
    ProtocolAlias {
        alias: "wireguard_cookie",
        protocol: "wireguard",
        entry: Some("cookie"),
    },
    ProtocolAlias {
        alias: "data",
        protocol: "wireguard",
        entry: Some("transport"),
    },
    ProtocolAlias {
        alias: "session",
        protocol: "wireguard",
        entry: Some("transport"),
    },
    ProtocolAlias {
        alias: "wireguard-data",
        protocol: "wireguard",
        entry: Some("transport"),
    },
    ProtocolAlias {
        alias: "wireguard_data",
        protocol: "wireguard",
        entry: Some("transport"),
    },
    ProtocolAlias {
        alias: "login",
        protocol: "smtp",
        entry: Some("auth"),
    },
    ProtocolAlias {
        alias: "login-denied",
        protocol: "smtp",
        entry: Some("auth-denied"),
    },
    ProtocolAlias {
        alias: "sender",
        protocol: "smtp",
        entry: Some("mail"),
    },
    ProtocolAlias {
        alias: "recipient",
        protocol: "smtp",
        entry: Some("rcpt"),
    },
    ProtocolAlias {
        alias: "recipient-denied",
        protocol: "smtp",
        entry: Some("rcpt-denied"),
    },
    ProtocolAlias {
        alias: "message",
        protocol: "smtp",
        entry: Some("data"),
    },
    ProtocolAlias {
        alias: "message-denied",
        protocol: "smtp",
        entry: Some("data-denied"),
    },
    ProtocolAlias {
        alias: "probe",
        protocol: "rtsp",
        entry: Some("options"),
    },
    ProtocolAlias {
        alias: "metadata",
        protocol: "rtsp",
        entry: Some("describe"),
    },
    ProtocolAlias {
        alias: "stream",
        protocol: "rtsp",
        entry: Some("setup"),
    },
    ProtocolAlias {
        alias: "start",
        protocol: "rtsp",
        entry: Some("play"),
    },
    ProtocolAlias {
        alias: "query",
        protocol: "snmp",
        entry: Some("get"),
    },
    ProtocolAlias {
        alias: "read",
        protocol: "snmp",
        entry: Some("get"),
    },
    ProtocolAlias {
        alias: "walk",
        protocol: "snmp",
        entry: Some("get-next"),
    },
    ProtocolAlias {
        alias: "next",
        protocol: "snmp",
        entry: Some("get-next"),
    },
    ProtocolAlias {
        alias: "write",
        protocol: "snmp",
        entry: Some("set"),
    },
    ProtocolAlias {
        alias: "update",
        protocol: "snmp",
        entry: Some("set"),
    },
    ProtocolAlias {
        alias: "notify",
        protocol: "snmp",
        entry: Some("trap"),
    },
    ProtocolAlias {
        alias: "alert",
        protocol: "snmp",
        entry: Some("trap"),
    },
    ProtocolAlias {
        alias: "ack-notify",
        protocol: "snmp",
        entry: Some("inform"),
    },
    ProtocolAlias {
        alias: "confirm-notify",
        protocol: "snmp",
        entry: Some("inform"),
    },
    ProtocolAlias {
        alias: "bulk-walk",
        protocol: "snmp",
        entry: Some("bulk"),
    },
    ProtocolAlias {
        alias: "table-read",
        protocol: "snmp",
        entry: Some("bulk"),
    },
    ProtocolAlias {
        alias: "auth-user",
        protocol: "snmp",
        entry: Some("v3-auth"),
    },
    ProtocolAlias {
        alias: "auth-session",
        protocol: "snmp",
        entry: Some("v3-auth"),
    },
    ProtocolAlias {
        alias: "private-session",
        protocol: "snmp",
        entry: Some("v3-priv"),
    },
    ProtocolAlias {
        alias: "encrypted-session",
        protocol: "snmp",
        entry: Some("v3-priv"),
    },
    ProtocolAlias {
        alias: "engine-discovery",
        protocol: "snmp",
        entry: Some("engine-sync"),
    },
    ProtocolAlias {
        alias: "report-sync",
        protocol: "snmp",
        entry: Some("engine-sync"),
    },
    ProtocolAlias {
        alias: "listen-trap",
        protocol: "snmp",
        entry: Some("trap-recv"),
    },
    ProtocolAlias {
        alias: "trap-listener",
        protocol: "snmp",
        entry: Some("trap-recv"),
    },
    ProtocolAlias {
        alias: "engine-report",
        protocol: "snmp",
        entry: Some("report"),
    },
    ProtocolAlias {
        alias: "report-pdu",
        protocol: "snmp",
        entry: Some("report"),
    },
    ProtocolAlias {
        alias: "auth-failed",
        protocol: "snmp",
        entry: Some("unauthorized"),
    },
    ProtocolAlias {
        alias: "access-denied",
        protocol: "snmp",
        entry: Some("unauthorized"),
    },
    ProtocolAlias {
        alias: "offer-denied",
        protocol: "dhcp",
        entry: Some("nak"),
    },
    ProtocolAlias {
        alias: "dhcp-nak",
        protocol: "dhcp",
        entry: Some("nak"),
    },
    ProtocolAlias {
        alias: "dhcp_nak",
        protocol: "dhcp",
        entry: Some("nak"),
    },
    ProtocolAlias {
        alias: "lease-denied",
        protocol: "dhcp",
        entry: Some("nak"),
    },
    ProtocolAlias {
        alias: "binding-denied",
        protocol: "stun",
        entry: Some("binding-error"),
    },
    ProtocolAlias {
        alias: "stun-binding-error",
        protocol: "stun",
        entry: Some("binding-error"),
    },
    ProtocolAlias {
        alias: "stun_binding_error",
        protocol: "stun",
        entry: Some("binding-error"),
    },
    ProtocolAlias {
        alias: "binding-error",
        protocol: "stun",
        entry: Some("binding-error"),
    },
    ProtocolAlias {
        alias: "connect",
        protocol: "socks5",
        entry: Some("session"),
    },
    ProtocolAlias {
        alias: "proxy",
        protocol: "socks5",
        entry: Some("session"),
    },
    ProtocolAlias {
        alias: "login",
        protocol: "socks5",
        entry: Some("auth"),
    },
    ProtocolAlias {
        alias: "userpass",
        protocol: "socks5",
        entry: Some("auth"),
    },
    ProtocolAlias {
        alias: "login-denied",
        protocol: "socks5",
        entry: Some("auth-denied"),
    },
    ProtocolAlias {
        alias: "userpass-denied",
        protocol: "socks5",
        entry: Some("auth-denied"),
    },
    ProtocolAlias {
        alias: "connect-denied",
        protocol: "socks5",
        entry: Some("denied"),
    },
    ProtocolAlias {
        alias: "login-connect-denied",
        protocol: "socks5",
        entry: Some("auth-connect-denied"),
    },
    ProtocolAlias {
        alias: "userpass-connect-denied",
        protocol: "socks5",
        entry: Some("auth-connect-denied"),
    },
];
