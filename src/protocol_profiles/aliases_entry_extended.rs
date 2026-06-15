use super::ProtocolAlias;

pub(crate) const PROTOCOL_ENTRY_ALIASES_EXTENDED: &[ProtocolAlias] = &[
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
    ProtocolAlias {
        alias: "stream-ack",
        protocol: "redis",
        entry: Some("xack"),
    },
    ProtocolAlias {
        alias: "stream-acknowledge",
        protocol: "redis",
        entry: Some("xack"),
    },
    ProtocolAlias {
        alias: "stream-append",
        protocol: "redis",
        entry: Some("xadd"),
    },
    ProtocolAlias {
        alias: "stream-write",
        protocol: "redis",
        entry: Some("xadd"),
    },
    ProtocolAlias {
        alias: "stream-auto-claim",
        protocol: "redis",
        entry: Some("xautoclaim"),
    },
    ProtocolAlias {
        alias: "stream-idle-reassign",
        protocol: "redis",
        entry: Some("xautoclaim"),
    },
    ProtocolAlias {
        alias: "stream-claim",
        protocol: "redis",
        entry: Some("xclaim"),
    },
    ProtocolAlias {
        alias: "stream-reassign",
        protocol: "redis",
        entry: Some("xclaim"),
    },
    ProtocolAlias {
        alias: "stream-delete",
        protocol: "redis",
        entry: Some("xdel"),
    },
    ProtocolAlias {
        alias: "stream-prune-entry",
        protocol: "redis",
        entry: Some("xdel"),
    },
    ProtocolAlias {
        alias: "stream-group",
        protocol: "redis",
        entry: Some("xgroup"),
    },
    ProtocolAlias {
        alias: "stream-consumer-group",
        protocol: "redis",
        entry: Some("xgroup"),
    },
    ProtocolAlias {
        alias: "stream-group-manage",
        protocol: "redis",
        entry: Some("xgroup"),
    },
    ProtocolAlias {
        alias: "stream-group-create",
        protocol: "redis",
        entry: Some("xgroup"),
    },
    ProtocolAlias {
        alias: "stream-group-destroy",
        protocol: "redis",
        entry: Some("xgroup"),
    },
    ProtocolAlias {
        alias: "stream-group-create-consumer",
        protocol: "redis",
        entry: Some("xgroup"),
    },
    ProtocolAlias {
        alias: "stream-group-drop-consumer",
        protocol: "redis",
        entry: Some("xgroup"),
    },
    ProtocolAlias {
        alias: "stream-group-setid",
        protocol: "redis",
        entry: Some("xgroup"),
    },
    ProtocolAlias {
        alias: "stream-group-help",
        protocol: "redis",
        entry: Some("xgroup"),
    },
    ProtocolAlias {
        alias: "stream-group-list-consumers",
        protocol: "redis",
        entry: Some("xgroup"),
    },
    ProtocolAlias {
        alias: "stream-group-list-groups",
        protocol: "redis",
        entry: Some("xgroup"),
    },
    ProtocolAlias {
        alias: "stream-info",
        protocol: "redis",
        entry: Some("xinfo"),
    },
    ProtocolAlias {
        alias: "stream-inspect",
        protocol: "redis",
        entry: Some("xinfo"),
    },
    ProtocolAlias {
        alias: "stream-info-stream",
        protocol: "redis",
        entry: Some("xinfo"),
    },
    ProtocolAlias {
        alias: "stream-info-groups",
        protocol: "redis",
        entry: Some("xinfo"),
    },
    ProtocolAlias {
        alias: "stream-info-consumers",
        protocol: "redis",
        entry: Some("xinfo"),
    },
    ProtocolAlias {
        alias: "stream-length",
        protocol: "redis",
        entry: Some("xlen"),
    },
    ProtocolAlias {
        alias: "stream-count",
        protocol: "redis",
        entry: Some("xlen"),
    },
    ProtocolAlias {
        alias: "stream-pending",
        protocol: "redis",
        entry: Some("xpending"),
    },
    ProtocolAlias {
        alias: "stream-delivery-backlog",
        protocol: "redis",
        entry: Some("xpending"),
    },
    ProtocolAlias {
        alias: "stream-range",
        protocol: "redis",
        entry: Some("xrange"),
    },
    ProtocolAlias {
        alias: "stream-history",
        protocol: "redis",
        entry: Some("xrange"),
    },
    ProtocolAlias {
        alias: "stream-range-reverse",
        protocol: "redis",
        entry: Some("xrevrange"),
    },
    ProtocolAlias {
        alias: "stream-history-reverse",
        protocol: "redis",
        entry: Some("xrevrange"),
    },
    ProtocolAlias {
        alias: "stream-trim",
        protocol: "redis",
        entry: Some("xtrim"),
    },
    ProtocolAlias {
        alias: "stream-prune",
        protocol: "redis",
        entry: Some("xtrim"),
    },
    ProtocolAlias {
        alias: "sorted-count",
        protocol: "redis",
        entry: Some("zcard"),
    },
    ProtocolAlias {
        alias: "score-count",
        protocol: "redis",
        entry: Some("zcard"),
    },
    ProtocolAlias {
        alias: "sorted-range-count",
        protocol: "redis",
        entry: Some("zcount"),
    },
    ProtocolAlias {
        alias: "score-window-count",
        protocol: "redis",
        entry: Some("zcount"),
    },
    ProtocolAlias {
        alias: "sorted-score-increment",
        protocol: "redis",
        entry: Some("zincrby"),
    },
    ProtocolAlias {
        alias: "score-bump",
        protocol: "redis",
        entry: Some("zincrby"),
    },
    ProtocolAlias {
        alias: "sorted-multi-pop",
        protocol: "redis",
        entry: Some("zmpop"),
    },
    ProtocolAlias {
        alias: "score-pop-many",
        protocol: "redis",
        entry: Some("zmpop"),
    },
    ProtocolAlias {
        alias: "sorted-pop-max",
        protocol: "redis",
        entry: Some("zpopmax"),
    },
    ProtocolAlias {
        alias: "score-pop-highest",
        protocol: "redis",
        entry: Some("zpopmax"),
    },
    ProtocolAlias {
        alias: "sorted-pop-min",
        protocol: "redis",
        entry: Some("zpopmin"),
    },
    ProtocolAlias {
        alias: "score-pop-lowest",
        protocol: "redis",
        entry: Some("zpopmin"),
    },
    ProtocolAlias {
        alias: "sorted-read",
        protocol: "redis",
        entry: Some("zrange"),
    },
    ProtocolAlias {
        alias: "score-read",
        protocol: "redis",
        entry: Some("zrange"),
    },
    ProtocolAlias {
        alias: "sorted-range-score",
        protocol: "redis",
        entry: Some("zrangebyscore"),
    },
    ProtocolAlias {
        alias: "score-window-read",
        protocol: "redis",
        entry: Some("zrangebyscore"),
    },
    ProtocolAlias {
        alias: "sorted-member-rank",
        protocol: "redis",
        entry: Some("zrank"),
    },
    ProtocolAlias {
        alias: "score-rank-member",
        protocol: "redis",
        entry: Some("zrank"),
    },
    ProtocolAlias {
        alias: "sorted-remove",
        protocol: "redis",
        entry: Some("zrem"),
    },
    ProtocolAlias {
        alias: "score-remove",
        protocol: "redis",
        entry: Some("zrem"),
    },
    ProtocolAlias {
        alias: "sorted-revrange-score",
        protocol: "redis",
        entry: Some("zrevrangebyscore"),
    },
    ProtocolAlias {
        alias: "score-window-read-reverse",
        protocol: "redis",
        entry: Some("zrevrangebyscore"),
    },
    ProtocolAlias {
        alias: "sorted-member-revrank",
        protocol: "redis",
        entry: Some("zrevrank"),
    },
    ProtocolAlias {
        alias: "score-revrank-member",
        protocol: "redis",
        entry: Some("zrevrank"),
    },
    ProtocolAlias {
        alias: "sorted-member-score",
        protocol: "redis",
        entry: Some("zscore"),
    },
    ProtocolAlias {
        alias: "score-read-member",
        protocol: "redis",
        entry: Some("zscore"),
    },
];
