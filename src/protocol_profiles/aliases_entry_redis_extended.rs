use super::ProtocolAlias;

pub(crate) const PROTOCOL_ENTRY_ALIASES_REDIS_EXTENDED: &[ProtocolAlias] = &[
    ProtocolAlias {
        alias: "list-blocking-pop-left",
        protocol: "redis",
        entry: Some("blpop"),
    },
    ProtocolAlias {
        alias: "left-blocking-pop",
        protocol: "redis",
        entry: Some("blpop"),
    },
    ProtocolAlias {
        alias: "list-blocking-pop-right",
        protocol: "redis",
        entry: Some("brpop"),
    },
    ProtocolAlias {
        alias: "right-blocking-pop",
        protocol: "redis",
        entry: Some("brpop"),
    },
    ProtocolAlias {
        alias: "list-blocking-move-right-to-left",
        protocol: "redis",
        entry: Some("brpoplpush"),
    },
    ProtocolAlias {
        alias: "right-blocking-pop-left-push",
        protocol: "redis",
        entry: Some("brpoplpush"),
    },
    ProtocolAlias {
        alias: "sorted-blocking-multi-pop",
        protocol: "redis",
        entry: Some("bzmpop"),
    },
    ProtocolAlias {
        alias: "score-blocking-pop-many",
        protocol: "redis",
        entry: Some("bzmpop"),
    },
    ProtocolAlias {
        alias: "sorted-blocking-pop-min",
        protocol: "redis",
        entry: Some("bzpopmin"),
    },
    ProtocolAlias {
        alias: "score-blocking-pop-lowest",
        protocol: "redis",
        entry: Some("bzpopmin"),
    },
    ProtocolAlias {
        alias: "decrement",
        protocol: "redis",
        entry: Some("decr"),
    },
    ProtocolAlias {
        alias: "count-down",
        protocol: "redis",
        entry: Some("decr"),
    },
    ProtocolAlias {
        alias: "delete",
        protocol: "redis",
        entry: Some("del"),
    },
    ProtocolAlias {
        alias: "remove",
        protocol: "redis",
        entry: Some("del"),
    },
    ProtocolAlias {
        alias: "present",
        protocol: "redis",
        entry: Some("exists"),
    },
    ProtocolAlias {
        alias: "key-check",
        protocol: "redis",
        entry: Some("exists"),
    },
    ProtocolAlias {
        alias: "set-ttl",
        protocol: "redis",
        entry: Some("expire"),
    },
    ProtocolAlias {
        alias: "expiry",
        protocol: "redis",
        entry: Some("expire"),
    },
    ProtocolAlias {
        alias: "hash-read",
        protocol: "redis",
        entry: Some("hget"),
    },
    ProtocolAlias {
        alias: "field-read",
        protocol: "redis",
        entry: Some("hget"),
    },
    ProtocolAlias {
        alias: "hash-multi-read",
        protocol: "redis",
        entry: Some("hmget"),
    },
    ProtocolAlias {
        alias: "fields-read",
        protocol: "redis",
        entry: Some("hmget"),
    },
    ProtocolAlias {
        alias: "hash-multi-write",
        protocol: "redis",
        entry: Some("hmset"),
    },
    ProtocolAlias {
        alias: "fields-write",
        protocol: "redis",
        entry: Some("hmset"),
    },
    ProtocolAlias {
        alias: "hash-write",
        protocol: "redis",
        entry: Some("hset"),
    },
    ProtocolAlias {
        alias: "field-write",
        protocol: "redis",
        entry: Some("hset"),
    },
    ProtocolAlias {
        alias: "increment",
        protocol: "redis",
        entry: Some("incr"),
    },
    ProtocolAlias {
        alias: "count-up",
        protocol: "redis",
        entry: Some("incr"),
    },
    ProtocolAlias {
        alias: "list-move",
        protocol: "redis",
        entry: Some("lmove"),
    },
    ProtocolAlias {
        alias: "list-directional-move",
        protocol: "redis",
        entry: Some("lmove"),
    },
    ProtocolAlias {
        alias: "left-right-move",
        protocol: "redis",
        entry: Some("lmove"),
    },
    ProtocolAlias {
        alias: "right-left-move",
        protocol: "redis",
        entry: Some("lmove"),
    },
    ProtocolAlias {
        alias: "list-multi-pop",
        protocol: "redis",
        entry: Some("lmpop"),
    },
    ProtocolAlias {
        alias: "list-pop-many",
        protocol: "redis",
        entry: Some("lmpop"),
    },
    ProtocolAlias {
        alias: "list-pop-left",
        protocol: "redis",
        entry: Some("lpop"),
    },
    ProtocolAlias {
        alias: "left-pop",
        protocol: "redis",
        entry: Some("lpop"),
    },
    ProtocolAlias {
        alias: "list-prepend",
        protocol: "redis",
        entry: Some("lpush"),
    },
    ProtocolAlias {
        alias: "left-push",
        protocol: "redis",
        entry: Some("lpush"),
    },
    ProtocolAlias {
        alias: "multi-read",
        protocol: "redis",
        entry: Some("mget"),
    },
    ProtocolAlias {
        alias: "bulk-read",
        protocol: "redis",
        entry: Some("mget"),
    },
    ProtocolAlias {
        alias: "multi-write",
        protocol: "redis",
        entry: Some("mset"),
    },
    ProtocolAlias {
        alias: "bulk-write",
        protocol: "redis",
        entry: Some("mset"),
    },
    ProtocolAlias {
        alias: "precise-ttl",
        protocol: "redis",
        entry: Some("pttl"),
    },
    ProtocolAlias {
        alias: "ms-ttl",
        protocol: "redis",
        entry: Some("pttl"),
    },
    ProtocolAlias {
        alias: "list-pop-right",
        protocol: "redis",
        entry: Some("rpop"),
    },
    ProtocolAlias {
        alias: "right-pop",
        protocol: "redis",
        entry: Some("rpop"),
    },
    ProtocolAlias {
        alias: "list-move-right-to-left",
        protocol: "redis",
        entry: Some("rpoplpush"),
    },
    ProtocolAlias {
        alias: "right-pop-left-push",
        protocol: "redis",
        entry: Some("rpoplpush"),
    },
    ProtocolAlias {
        alias: "list-append",
        protocol: "redis",
        entry: Some("rpush"),
    },
    ProtocolAlias {
        alias: "right-push",
        protocol: "redis",
        entry: Some("rpush"),
    },
    ProtocolAlias {
        alias: "set-add",
        protocol: "redis",
        entry: Some("sadd"),
    },
    ProtocolAlias {
        alias: "member-add",
        protocol: "redis",
        entry: Some("sadd"),
    },
    ProtocolAlias {
        alias: "set-read",
        protocol: "redis",
        entry: Some("smembers"),
    },
    ProtocolAlias {
        alias: "members-read",
        protocol: "redis",
        entry: Some("smembers"),
    },
    ProtocolAlias {
        alias: "time-to-live",
        protocol: "redis",
        entry: Some("ttl"),
    },
    ProtocolAlias {
        alias: "key-ttl",
        protocol: "redis",
        entry: Some("ttl"),
    },
];
