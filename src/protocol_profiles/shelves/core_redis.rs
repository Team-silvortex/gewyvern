use super::super::ShelfMatch;

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
