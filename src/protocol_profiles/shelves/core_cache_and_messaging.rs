use super::super::ShelfMatch;

pub(crate) fn mysql_shelf(entry: &str) -> Option<ShelfMatch> {
    const CONNECT_AUTH: &[&str] = &["connect", "auth", "auth-denied"];
    const QUERY_SESSION: &[&str] = &["query", "session"];
    const ERROR: &[&str] = &["error"];
    if CONNECT_AUTH.contains(&entry) {
        Some((
            "connect-auth",
            "Connect And Auth",
            "docs/book/reference-mysql-connect-surface.md",
            CONNECT_AUTH,
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
    const CONNECT_AUTH: &[&str] = &["connect", "auth", "auth-denied"];
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

pub(crate) fn mongodb_shelf(entry: &str) -> Option<ShelfMatch> {
    const COMMAND: &[&str] = &["command"];
    const REPLY: &[&str] = &["reply"];
    const LEGACY: &[&str] = &["legacy-query", "query-failure"];
    if COMMAND.contains(&entry) {
        Some((
            "command",
            "Command",
            "docs/book/reference-mongodb-command-surface.md",
            COMMAND,
        ))
    } else if REPLY.contains(&entry) {
        Some((
            "reply",
            "Reply",
            "docs/book/reference-mongodb-reply-surface.md",
            REPLY,
        ))
    } else if LEGACY.contains(&entry) {
        Some((
            "legacy-query",
            "Legacy Query",
            "docs/book/reference-mongodb-legacy-query-surface.md",
            LEGACY,
        ))
    } else {
        None
    }
}

pub(crate) fn cassandra_shelf(entry: &str) -> Option<ShelfMatch> {
    const SESSION_AUTH: &[&str] = &["startup", "authenticate"];
    const QUERY_RESULT: &[&str] = &["query", "result"];
    const ERROR: &[&str] = &["error"];
    if SESSION_AUTH.contains(&entry) {
        Some((
            "session-auth",
            "Session And Auth",
            "docs/book/reference-cassandra-session-surface.md",
            SESSION_AUTH,
        ))
    } else if QUERY_RESULT.contains(&entry) {
        Some((
            "query-result",
            "Query And Result",
            "docs/book/reference-cassandra-query-surface.md",
            QUERY_RESULT,
        ))
    } else if ERROR.contains(&entry) {
        Some((
            "error",
            "Error",
            "docs/book/reference-cassandra-error-surface.md",
            ERROR,
        ))
    } else {
        None
    }
}

pub(crate) fn mssql_shelf(entry: &str) -> Option<ShelfMatch> {
    const SESSION_AUTH: &[&str] = &["prelogin", "login"];
    const QUERY_RESPONSE: &[&str] = &["query", "response"];
    const TOKENS: &[&str] = &["colmetadata", "row", "done", "envchange"];
    const ERROR: &[&str] = &["error"];
    if SESSION_AUTH.contains(&entry) {
        Some((
            "session-auth",
            "Session And Auth",
            "docs/book/reference-mssql-session-surface.md",
            SESSION_AUTH,
        ))
    } else if QUERY_RESPONSE.contains(&entry) {
        Some((
            "query-response",
            "Query And Response",
            "docs/book/reference-mssql-query-surface.md",
            QUERY_RESPONSE,
        ))
    } else if TOKENS.contains(&entry) {
        Some((
            "token",
            "TDS Tokens",
            "docs/book/reference-mssql-token-surface.md",
            TOKENS,
        ))
    } else if ERROR.contains(&entry) {
        Some((
            "error",
            "Error",
            "docs/book/reference-mssql-error-surface.md",
            ERROR,
        ))
    } else {
        None
    }
}

pub(crate) fn elasticsearch_shelf(entry: &str) -> Option<ShelfMatch> {
    const CLUSTER: &[&str] = &["health"];
    const QUERY: &[&str] = &["search"];
    const MUTATION: &[&str] = &["index", "bulk"];
    if CLUSTER.contains(&entry) {
        Some((
            "cluster-health",
            "Cluster Health",
            "docs/book/reference-elasticsearch-health-surface.md",
            CLUSTER,
        ))
    } else if QUERY.contains(&entry) {
        Some((
            "query",
            "Search Query",
            "docs/book/reference-elasticsearch-search-surface.md",
            QUERY,
        ))
    } else if MUTATION.contains(&entry) {
        Some((
            "mutation",
            "Index And Bulk Mutation",
            "docs/book/reference-elasticsearch-mutation-surface.md",
            MUTATION,
        ))
    } else {
        None
    }
}

pub(crate) fn etcd_shelf(entry: &str) -> Option<ShelfMatch> {
    const HEALTH: &[&str] = &["health"];
    const KV: &[&str] = &["range", "put"];
    const STREAM_LIFECYCLE: &[&str] = &["watch", "lease"];
    if HEALTH.contains(&entry) {
        Some((
            "cluster-health",
            "Cluster Health",
            "docs/book/reference-etcd-health-surface.md",
            HEALTH,
        ))
    } else if KV.contains(&entry) {
        Some((
            "kv",
            "KV Read And Write",
            "docs/book/reference-etcd-kv-surface.md",
            KV,
        ))
    } else if STREAM_LIFECYCLE.contains(&entry) {
        Some((
            "stream-lifecycle",
            "Watch And Lease Lifecycle",
            "docs/book/reference-etcd-stream-lifecycle-surface.md",
            STREAM_LIFECYCLE,
        ))
    } else {
        None
    }
}

pub(crate) fn zookeeper_shelf(entry: &str) -> Option<ShelfMatch> {
    const SESSION: &[&str] = &["connect", "auth-denied"];
    const DATA: &[&str] = &["read", "write"];
    const WATCH: &[&str] = &["watch"];
    if SESSION.contains(&entry) {
        Some((
            "session-auth",
            "Session And Auth",
            "docs/book/reference-zookeeper-session-surface.md",
            SESSION,
        ))
    } else if DATA.contains(&entry) {
        Some((
            "znode-data",
            "Znode Read And Write",
            "docs/book/reference-zookeeper-znode-surface.md",
            DATA,
        ))
    } else if WATCH.contains(&entry) {
        Some((
            "watch",
            "Watch Delivery",
            "docs/book/reference-zookeeper-watch-surface.md",
            WATCH,
        ))
    } else {
        None
    }
}

pub(crate) fn consul_shelf(entry: &str) -> Option<ShelfMatch> {
    const DISCOVERY: &[&str] = &["health", "catalog", "service"];
    const STATE: &[&str] = &["kv", "session"];
    if DISCOVERY.contains(&entry) {
        Some((
            "discovery-health",
            "Discovery And Health",
            "docs/book/reference-consul-discovery-surface.md",
            DISCOVERY,
        ))
    } else if STATE.contains(&entry) {
        Some((
            "state-session",
            "KV And Session State",
            "docs/book/reference-consul-state-surface.md",
            STATE,
        ))
    } else {
        None
    }
}

pub(crate) fn mqtt_shelf(entry: &str) -> Option<ShelfMatch> {
    const SESSION: &[&str] = &["connect", "connack"];
    const PUBLISH: &[&str] = &["publish"];
    const SUBSCRIBE: &[&str] = &["subscribe"];
    const QOS2: &[&str] = &["pubrec", "pubrel", "pubcomp", "disconnect"];
    if SESSION.contains(&entry) {
        Some((
            "session",
            "Session",
            "docs/book/reference-mqtt-session-surface.md",
            SESSION,
        ))
    } else if PUBLISH.contains(&entry) {
        Some((
            "publish",
            "Publish",
            "docs/book/reference-mqtt-publish-surface.md",
            PUBLISH,
        ))
    } else if SUBSCRIBE.contains(&entry) {
        Some((
            "subscribe",
            "Subscribe",
            "docs/book/reference-mqtt-subscribe-surface.md",
            SUBSCRIBE,
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
    const GET: &[&str] = &["get", "miss"];
    const SET: &[&str] = &["set", "not-stored"];
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
    const PUBLISH: &[&str] = &["publish"];
    const SUBSCRIBE: &[&str] = &["subscribe"];
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
    } else if PUBLISH.contains(&entry) {
        Some((
            "publish",
            "Publish",
            "docs/book/reference-redis-publish-surface.md",
            PUBLISH,
        ))
    } else if SUBSCRIBE.contains(&entry) {
        Some((
            "subscribe",
            "Subscribe",
            "docs/book/reference-redis-subscribe-surface.md",
            SUBSCRIBE,
        ))
    } else if SET.contains(&entry) {
        Some((
            "set",
            "Set",
            "docs/book/reference-redis-set-surface.md",
            SET,
        ))
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
    const CLOSE: &[&str] = &["close"];
    const SERVER_CLOSE: &[&str] = &["server-close"];
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
    } else if CLOSE.contains(&entry) {
        Some((
            "close",
            "Connection Close",
            "docs/book/reference-http3-close-surface.md",
            CLOSE,
        ))
    } else if SERVER_CLOSE.contains(&entry) {
        Some((
            "server-close",
            "Server Close",
            "docs/book/reference-http3-server-close-surface.md",
            SERVER_CLOSE,
        ))
    } else {
        None
    }
}
