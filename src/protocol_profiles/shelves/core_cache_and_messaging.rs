use super::super::ShelfMatch;

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
    const FAILURE: &[&str] = &[
        "auth-required",
        "auth-denied",
        "error",
        "wrongtype",
        "busygroup",
        "readonly",
        "noscript",
        "moved",
        "ask",
        "tryagain",
        "loading",
        "crossslot",
        "clusterdown",
        "masterdown",
        "oom",
        "busy",
        "execabort",
        "misconf",
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
    } else if FAILURE.contains(&entry) {
        Some((
            "failure",
            "Failure Semantics",
            "docs/book/reference-redis-failure-semantics.md",
            FAILURE,
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
