use super::ShelfMatch;

const GENERIC_SURFACE_PAGE: &str = "docs/book/reference-protocol-surface.md";

pub(crate) fn dns_shelf(entry: &str) -> Option<ShelfMatch> {
    const UDP: &[&str] = &["udp"];
    const TCP: &[&str] = &["tcp"];
    if UDP.contains(&entry) {
        Some((
            "udp",
            "UDP Lookup",
            "docs/book/reference-dns-udp-surface.md",
            UDP,
        ))
    } else if TCP.contains(&entry) {
        Some((
            "tcp",
            "TCP Query",
            "docs/book/reference-dns-tcp-surface.md",
            TCP,
        ))
    } else {
        None
    }
}

pub(crate) fn https_shelf(entry: &str) -> Option<ShelfMatch> {
    const CONNECT: &[&str] = &["connect"];
    if CONNECT.contains(&entry) {
        Some(("connect", "Connect", GENERIC_SURFACE_PAGE, CONNECT))
    } else {
        None
    }
}

pub(crate) fn http_shelf(entry: &str) -> Option<ShelfMatch> {
    const MESSAGE: &[&str] = &["request", "response"];
    const CONNECT: &[&str] = &["connect", "denied"];
    const CONNECT_AUTH: &[&str] = &["auth-required", "auth-tunnel"];
    if MESSAGE.contains(&entry) {
        Some((
            "message",
            "Message",
            "docs/book/reference-http-message-surface.md",
            MESSAGE,
        ))
    } else if CONNECT.contains(&entry) {
        Some((
            "connect",
            "Connect Tunnel",
            "docs/book/reference-http-connect-surface.md",
            CONNECT,
        ))
    } else if CONNECT_AUTH.contains(&entry) {
        Some((
            "connect-auth",
            "Connect Auth",
            "docs/book/reference-http-connect-auth-surface.md",
            CONNECT_AUTH,
        ))
    } else {
        None
    }
}

pub(crate) fn hy2_shelf(entry: &str) -> Option<ShelfMatch> {
    const AUTH: &[&str] = &["auth"];
    const RELAY: &[&str] = &["udp", "tcp"];
    if AUTH.contains(&entry) {
        Some(("auth", "Auth", GENERIC_SURFACE_PAGE, AUTH))
    } else if RELAY.contains(&entry) {
        Some(("relay", "Relay", GENERIC_SURFACE_PAGE, RELAY))
    } else {
        None
    }
}

pub(crate) fn tls_shelf(entry: &str) -> Option<ShelfMatch> {
    const CLIENT: &[&str] = &["client"];
    if CLIENT.contains(&entry) {
        Some(("client", "Client", GENERIC_SURFACE_PAGE, CLIENT))
    } else {
        None
    }
}

pub(crate) fn stun_shelf(entry: &str) -> Option<ShelfMatch> {
    const BINDING: &[&str] = &["binding"];
    if BINDING.contains(&entry) {
        Some(("binding", "Binding", GENERIC_SURFACE_PAGE, BINDING))
    } else {
        None
    }
}

pub(crate) fn coap_shelf(entry: &str) -> Option<ShelfMatch> {
    const GET: &[&str] = &["get"];
    if GET.contains(&entry) {
        Some(("get", "Get", GENERIC_SURFACE_PAGE, GET))
    } else {
        None
    }
}

pub(crate) fn ntp_shelf(entry: &str) -> Option<ShelfMatch> {
    const CLIENT: &[&str] = &["client"];
    if CLIENT.contains(&entry) {
        Some(("client", "Client", GENERIC_SURFACE_PAGE, CLIENT))
    } else {
        None
    }
}

pub(crate) fn dhcp_shelf(entry: &str) -> Option<ShelfMatch> {
    const CLIENT: &[&str] = &["client"];
    if CLIENT.contains(&entry) {
        Some(("client", "Client", GENERIC_SURFACE_PAGE, CLIENT))
    } else {
        None
    }
}

pub(crate) fn wireguard_shelf(entry: &str) -> Option<ShelfMatch> {
    const HANDSHAKE: &[&str] = &["handshake"];
    if HANDSHAKE.contains(&entry) {
        Some(("handshake", "Handshake", GENERIC_SURFACE_PAGE, HANDSHAKE))
    } else {
        None
    }
}

pub(crate) fn mdns_shelf(entry: &str) -> Option<ShelfMatch> {
    const QUERY: &[&str] = &["query"];
    if QUERY.contains(&entry) {
        Some(("query", "Query", GENERIC_SURFACE_PAGE, QUERY))
    } else {
        None
    }
}

pub(crate) fn ssdp_shelf(entry: &str) -> Option<ShelfMatch> {
    const DISCOVERY: &[&str] = &["discovery"];
    if DISCOVERY.contains(&entry) {
        Some(("discovery", "Discovery", GENERIC_SURFACE_PAGE, DISCOVERY))
    } else {
        None
    }
}

pub(crate) fn quic_shelf(entry: &str) -> Option<ShelfMatch> {
    const INITIAL: &[&str] = &["initial"];
    const CRYPTO: &[&str] = &["crypto"];
    const STREAM: &[&str] = &["stream"];
    const BIDI: &[&str] = &["bidi"];
    if INITIAL.contains(&entry) {
        Some((
            "initial",
            "Initial",
            "docs/book/reference-quic-initial-surface.md",
            INITIAL,
        ))
    } else if CRYPTO.contains(&entry) {
        Some((
            "crypto",
            "Crypto Handshake",
            "docs/book/reference-quic-crypto-surface.md",
            CRYPTO,
        ))
    } else if STREAM.contains(&entry) {
        Some((
            "stream",
            "Outbound Stream",
            "docs/book/reference-quic-stream-surface.md",
            STREAM,
        ))
    } else if BIDI.contains(&entry) {
        Some((
            "bidi",
            "Bidirectional Stream",
            "docs/book/reference-quic-bidi-surface.md",
            BIDI,
        ))
    } else {
        None
    }
}

pub(crate) fn radius_shelf(entry: &str) -> Option<ShelfMatch> {
    const ACCESS: &[&str] = &["access"];
    if ACCESS.contains(&entry) {
        Some(("access", "Access", GENERIC_SURFACE_PAGE, ACCESS))
    } else {
        None
    }
}

pub(crate) fn gtpu_shelf(entry: &str) -> Option<ShelfMatch> {
    const ECHO: &[&str] = &["echo"];
    if ECHO.contains(&entry) {
        Some(("echo", "Echo", GENERIC_SURFACE_PAGE, ECHO))
    } else {
        None
    }
}

pub(crate) fn mysql_shelf(entry: &str) -> Option<ShelfMatch> {
    const CONNECT: &[&str] = &["connect"];
    const QUERY_SESSION: &[&str] = &["query", "session"];
    const ERROR: &[&str] = &["error"];
    if CONNECT.contains(&entry) {
        Some((
            "connect",
            "Connect",
            "docs/book/reference-mysql-connect-surface.md",
            CONNECT,
        ))
    } else if QUERY_SESSION.contains(&entry) {
        Some((
            "query-session",
            "Query And Session",
            "docs/book/reference-mysql-query-surface.md",
            QUERY_SESSION,
        ))
    } else if ERROR.contains(&entry) {
        Some((
            "error",
            "Query Error",
            "docs/book/reference-mysql-error-surface.md",
            ERROR,
        ))
    } else {
        None
    }
}

pub(crate) fn postgres_shelf(entry: &str) -> Option<ShelfMatch> {
    const CONNECT_AUTH: &[&str] = &["connect", "auth"];
    const QUERY_SESSION: &[&str] = &["query", "session"];
    const ERROR: &[&str] = &["error"];
    if CONNECT_AUTH.contains(&entry) {
        Some((
            "connect-auth",
            "Connect And Auth",
            "docs/book/reference-postgres-connect-surface.md",
            CONNECT_AUTH,
        ))
    } else if QUERY_SESSION.contains(&entry) {
        Some((
            "query-session",
            "Query And Session",
            "docs/book/reference-postgres-query-surface.md",
            QUERY_SESSION,
        ))
    } else if ERROR.contains(&entry) {
        Some((
            "error",
            "Query Error",
            "docs/book/reference-postgres-error-surface.md",
            ERROR,
        ))
    } else {
        None
    }
}

pub(crate) fn mqtt_shelf(entry: &str) -> Option<ShelfMatch> {
    const SESSION: &[&str] = &["session"];
    const PUBSUB: &[&str] = &["publish", "subscribe"];
    const QOS2: &[&str] = &["pubrec", "pubrel", "pubcomp", "disconnect"];
    const CONNECT_ONLY: &[&str] = &["connect"];
    if SESSION.contains(&entry) || CONNECT_ONLY.contains(&entry) {
        Some((
            "session",
            "Session",
            "docs/book/reference-mqtt-session-surface.md",
            if SESSION.contains(&entry) {
                SESSION
            } else {
                CONNECT_ONLY
            },
        ))
    } else if PUBSUB.contains(&entry) {
        Some((
            "pubsub",
            "Publish And Subscribe",
            "docs/book/reference-mqtt-pubsub-surface.md",
            PUBSUB,
        ))
    } else if QOS2.contains(&entry) {
        Some((
            "qos2-teardown",
            "QoS2 And Teardown",
            "docs/book/reference-mqtt-qos2-surface.md",
            QOS2,
        ))
    } else {
        None
    }
}

pub(crate) fn memcached_shelf(entry: &str) -> Option<ShelfMatch> {
    const GET: &[&str] = &["get"];
    const SET: &[&str] = &["set"];
    if GET.contains(&entry) {
        Some((
            "get",
            "Get",
            "docs/book/reference-memcached-get-surface.md",
            GET,
        ))
    } else if SET.contains(&entry) {
        Some((
            "set",
            "Set",
            "docs/book/reference-memcached-set-surface.md",
            SET,
        ))
    } else {
        None
    }
}

pub(crate) fn ftp_shelf(entry: &str) -> Option<ShelfMatch> {
    const SESSION: &[&str] = &["session", "denied"];
    const PASSIVE: &[&str] = &["list", "retr", "stor"];
    const ACTIVE: &[&str] = &["active-list", "active-retr", "active-stor"];
    if SESSION.contains(&entry) {
        Some((
            "session",
            "Session",
            "docs/book/reference-ftp-session-surface.md",
            SESSION,
        ))
    } else if PASSIVE.contains(&entry) {
        Some((
            "passive",
            "Passive Data",
            "docs/book/reference-ftp-passive-surface.md",
            PASSIVE,
        ))
    } else if ACTIVE.contains(&entry) {
        Some((
            "active",
            "Active Data",
            "docs/book/reference-ftp-active-surface.md",
            ACTIVE,
        ))
    } else {
        None
    }
}

pub(crate) fn redis_shelf(entry: &str) -> Option<ShelfMatch> {
    const KV: &[&str] = &[
        "session", "ping", "get", "set", "incr", "decr", "mget", "mset", "exists", "del", "expire",
        "ttl", "pttl",
    ];
    const PUBSUB: &[&str] = &["publish", "subscribe"];
    const SET: &[&str] = &["sadd", "smembers"];
    const HASH: &[&str] = &["hget", "hset", "hmget", "hmset"];
    const LIST: &[&str] = &[
        "lpush",
        "rpush",
        "lpop",
        "rpop",
        "blpop",
        "brpop",
        "rpoplpush",
        "brpoplpush",
        "lmove",
        "blmove",
        "lmpop",
        "blmpop",
    ];
    const SORTED_SET: &[&str] = &[
        "zadd",
        "zcard",
        "zcount",
        "zincrby",
        "zrank",
        "zrem",
        "zrevrangebyscore",
        "zrevrank",
        "zscore",
        "zrange",
        "zrangebyscore",
        "zpopmin",
        "zpopmax",
        "bzpopmin",
        "bzpopmax",
        "zmpop",
        "bzmpop",
    ];
    const STREAM: &[&str] = &[
        "xadd",
        "xread",
        "xrange",
        "xrevrange",
        "xtrim",
        "xlen",
        "xack",
        "xpending",
        "xgroup",
        "xreadgroup",
        "xclaim",
        "xautoclaim",
        "xdel",
        "xinfo",
    ];
    if KV.contains(&entry) {
        Some((
            "kv-session",
            "Key-Value And Session",
            "docs/book/reference-redis-kv-surface.md",
            KV,
        ))
    } else if PUBSUB.contains(&entry) {
        Some((
            "pubsub",
            "Publish And Subscribe",
            "docs/book/reference-redis-surface.md",
            PUBSUB,
        ))
    } else if SET.contains(&entry) {
        Some(("set", "Set", "docs/book/reference-redis-surface.md", SET))
    } else if HASH.contains(&entry) {
        Some((
            "hash",
            "Hash",
            "docs/book/reference-redis-hash-surface.md",
            HASH,
        ))
    } else if LIST.contains(&entry) {
        Some((
            "list",
            "List",
            "docs/book/reference-redis-list-surface.md",
            LIST,
        ))
    } else if SORTED_SET.contains(&entry) {
        Some((
            "sorted-set",
            "Sorted Set",
            "docs/book/reference-redis-sorted-set-surface.md",
            SORTED_SET,
        ))
    } else if STREAM.contains(&entry) {
        Some((
            "stream",
            "Stream",
            "docs/book/reference-redis-stream-surface.md",
            STREAM,
        ))
    } else {
        None
    }
}

pub(crate) fn http3_shelf(entry: &str) -> Option<ShelfMatch> {
    const REQUEST: &[&str] = &["request"];
    const RESPONSE: &[&str] = &["response"];
    const SERVER: &[&str] = &["server"];
    if REQUEST.contains(&entry) {
        Some((
            "request",
            "Request",
            "docs/book/reference-http3-request-surface.md",
            REQUEST,
        ))
    } else if RESPONSE.contains(&entry) || SERVER.contains(&entry) {
        Some((
            "server",
            "Server",
            "docs/book/reference-http3-server-surface.md",
            if RESPONSE.contains(&entry) {
                RESPONSE
            } else {
                SERVER
            },
        ))
    } else {
        None
    }
}

pub(crate) fn snmp_shelf(entry: &str) -> Option<ShelfMatch> {
    const GET: &[&str] = &["get"];
    if GET.contains(&entry) {
        Some(("get", "Get", GENERIC_SURFACE_PAGE, GET))
    } else {
        None
    }
}
