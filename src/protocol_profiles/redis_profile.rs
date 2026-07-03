use super::{ProtocolEntryProfile, ProtocolProfile};

pub(super) const REDIS_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "redis",
    default_entry: "ping",
    entries: &[
        ProtocolEntryProfile {
            mode: "ping",
            dsl_path: "dsl/redis_ping_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "session",
            dsl_path: "dsl/redis_session_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "get",
            dsl_path: "dsl/redis_get_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "set",
            dsl_path: "dsl/redis_set_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "auth-required",
            dsl_path: "protocols/redis/auth-required/main.gewy",
        },
        ProtocolEntryProfile {
            mode: "auth-denied",
            dsl_path: "protocols/redis/auth-denied/main.gewy",
        },
        ProtocolEntryProfile {
            mode: "error",
            dsl_path: "protocols/redis/error/main.gewy",
        },
        ProtocolEntryProfile {
            mode: "wrongtype",
            dsl_path: "protocols/redis/wrongtype/main.gewy",
        },
        ProtocolEntryProfile {
            mode: "busygroup",
            dsl_path: "protocols/redis/busygroup/main.gewy",
        },
        ProtocolEntryProfile {
            mode: "readonly",
            dsl_path: "protocols/redis/readonly/main.gewy",
        },
        ProtocolEntryProfile {
            mode: "noscript",
            dsl_path: "protocols/redis/noscript/main.gewy",
        },
        ProtocolEntryProfile {
            mode: "moved",
            dsl_path: "protocols/redis/moved/main.gewy",
        },
        ProtocolEntryProfile {
            mode: "ask",
            dsl_path: "protocols/redis/ask/main.gewy",
        },
        ProtocolEntryProfile {
            mode: "tryagain",
            dsl_path: "protocols/redis/tryagain/main.gewy",
        },
        ProtocolEntryProfile {
            mode: "loading",
            dsl_path: "protocols/redis/loading/main.gewy",
        },
        ProtocolEntryProfile {
            mode: "crossslot",
            dsl_path: "protocols/redis/crossslot/main.gewy",
        },
        ProtocolEntryProfile {
            mode: "clusterdown",
            dsl_path: "protocols/redis/clusterdown/main.gewy",
        },
        ProtocolEntryProfile {
            mode: "masterdown",
            dsl_path: "protocols/redis/masterdown/main.gewy",
        },
        ProtocolEntryProfile {
            mode: "oom",
            dsl_path: "protocols/redis/oom/main.gewy",
        },
        ProtocolEntryProfile {
            mode: "busy",
            dsl_path: "protocols/redis/busy/main.gewy",
        },
        ProtocolEntryProfile {
            mode: "execabort",
            dsl_path: "protocols/redis/execabort/main.gewy",
        },
        ProtocolEntryProfile {
            mode: "misconf",
            dsl_path: "protocols/redis/misconf/main.gewy",
        },
        ProtocolEntryProfile {
            mode: "del",
            dsl_path: "dsl/redis_del_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "incr",
            dsl_path: "dsl/redis_incr_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "decr",
            dsl_path: "dsl/redis_decr_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "mget",
            dsl_path: "dsl/redis_mget_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "mset",
            dsl_path: "dsl/redis_mset_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "exists",
            dsl_path: "dsl/redis_exists_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "expire",
            dsl_path: "dsl/redis_expire_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "ttl",
            dsl_path: "dsl/redis_ttl_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "pttl",
            dsl_path: "dsl/redis_pttl_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "hget",
            dsl_path: "dsl/redis_hget_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "hset",
            dsl_path: "dsl/redis_hset_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "hmget",
            dsl_path: "dsl/redis_hmget_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "hmset",
            dsl_path: "dsl/redis_hmset_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "lpush",
            dsl_path: "dsl/redis_lpush_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "rpush",
            dsl_path: "dsl/redis_rpush_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "lpop",
            dsl_path: "dsl/redis_lpop_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "rpop",
            dsl_path: "dsl/redis_rpop_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "lmove",
            dsl_path: "dsl/redis_lmove_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "blmove",
            dsl_path: "dsl/redis_blmove_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "lmpop",
            dsl_path: "dsl/redis_lmpop_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "blmpop",
            dsl_path: "dsl/redis_blmpop_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "blpop",
            dsl_path: "dsl/redis_blpop_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "brpop",
            dsl_path: "dsl/redis_brpop_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "rpoplpush",
            dsl_path: "dsl/redis_rpoplpush_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "brpoplpush",
            dsl_path: "dsl/redis_brpoplpush_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "sadd",
            dsl_path: "dsl/redis_sadd_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "smembers",
            dsl_path: "dsl/redis_smembers_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "publish",
            dsl_path: "dsl/redis_publish_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "subscribe",
            dsl_path: "dsl/redis_subscribe_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "zadd",
            dsl_path: "dsl/redis_zadd_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "zrange",
            dsl_path: "dsl/redis_zrange_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "zrangebyscore",
            dsl_path: "dsl/redis_zrangebyscore_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "zrevrangebyscore",
            dsl_path: "dsl/redis_zrevrangebyscore_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "zrank",
            dsl_path: "dsl/redis_zrank_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "zrevrank",
            dsl_path: "dsl/redis_zrevrank_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "zscore",
            dsl_path: "dsl/redis_zscore_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "zrem",
            dsl_path: "dsl/redis_zrem_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "zcard",
            dsl_path: "dsl/redis_zcard_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "zcount",
            dsl_path: "dsl/redis_zcount_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "zincrby",
            dsl_path: "dsl/redis_zincrby_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "zpopmax",
            dsl_path: "dsl/redis_zpopmax_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "zpopmin",
            dsl_path: "dsl/redis_zpopmin_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "zmpop",
            dsl_path: "dsl/redis_zmpop_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "bzpopmax",
            dsl_path: "dsl/redis_bzpopmax_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "bzpopmin",
            dsl_path: "dsl/redis_bzpopmin_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "bzmpop",
            dsl_path: "dsl/redis_bzmpop_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "xadd",
            dsl_path: "dsl/redis_xadd_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "xread",
            dsl_path: "dsl/redis_xread_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "xrange",
            dsl_path: "dsl/redis_xrange_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "xrevrange",
            dsl_path: "dsl/redis_xrevrange_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "xdel",
            dsl_path: "dsl/redis_xdel_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "xtrim",
            dsl_path: "dsl/redis_xtrim_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "xlen",
            dsl_path: "dsl/redis_xlen_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "xack",
            dsl_path: "dsl/redis_xack_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "xpending",
            dsl_path: "dsl/redis_xpending_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "xgroup",
            dsl_path: "dsl/redis_xgroup_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "xinfo",
            dsl_path: "dsl/redis_xinfo_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "xreadgroup",
            dsl_path: "dsl/redis_xreadgroup_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "xclaim",
            dsl_path: "dsl/redis_xclaim_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "xautoclaim",
            dsl_path: "dsl/redis_xautoclaim_path.gewy",
        },
    ],
};
