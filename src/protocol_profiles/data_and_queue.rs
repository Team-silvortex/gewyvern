use super::{ProtocolEntryProfile, ProtocolProfile};

pub(super) const POSTGRES_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "postgres",
    default_entry: "query",
    entries: &[
        ProtocolEntryProfile {
            mode: "connect",
            dsl_path: "dsl/postgres_connect_process.gewy",
        },
        ProtocolEntryProfile {
            mode: "auth",
            dsl_path: "dsl/postgres_auth_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "auth-denied",
            dsl_path: "dsl/postgres_auth_denied_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "query",
            dsl_path: "dsl/postgres_simple_query_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "error",
            dsl_path: "dsl/postgres_query_error_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "session",
            dsl_path: "dsl/postgres_query_session.gewy",
        },
    ],
};

pub(super) const MYSQL_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "mysql",
    default_entry: "session",
    entries: &[
        ProtocolEntryProfile {
            mode: "connect",
            dsl_path: "dsl/mysql_connect_process.gewy",
        },
        ProtocolEntryProfile {
            mode: "auth",
            dsl_path: "dsl/mysql_auth_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "auth-denied",
            dsl_path: "dsl/mysql_auth_denied_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "query",
            dsl_path: "dsl/mysql_simple_query_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "session",
            dsl_path: "dsl/mysql_query_session.gewy",
        },
        ProtocolEntryProfile {
            mode: "error",
            dsl_path: "dsl/mysql_query_error_path.gewy",
        },
    ],
};

pub(super) const MONGODB_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "mongodb",
    default_entry: "command",
    entries: &[
        ProtocolEntryProfile {
            mode: "command",
            dsl_path: "dsl/mongodb_command_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "reply",
            dsl_path: "dsl/mongodb_reply_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "legacy-query",
            dsl_path: "dsl/mongodb_legacy_query_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "query-failure",
            dsl_path: "dsl/mongodb_query_failure_path.gewy",
        },
    ],
};

pub(super) const CASSANDRA_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "cassandra",
    default_entry: "query",
    entries: &[
        ProtocolEntryProfile {
            mode: "startup",
            dsl_path: "dsl/cassandra_startup_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "authenticate",
            dsl_path: "dsl/cassandra_authenticate_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "query",
            dsl_path: "dsl/cassandra_query_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "result",
            dsl_path: "dsl/cassandra_result_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "error",
            dsl_path: "dsl/cassandra_error_path.gewy",
        },
    ],
};

pub(super) const MEMCACHED_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "memcached",
    default_entry: "get",
    entries: &[
        ProtocolEntryProfile {
            mode: "get",
            dsl_path: "dsl/memcached_get_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "miss",
            dsl_path: "dsl/memcached_miss_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "set",
            dsl_path: "dsl/memcached_set_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "not-stored",
            dsl_path: "dsl/memcached_not_stored_path.gewy",
        },
    ],
};

pub(super) const AMQP_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "amqp",
    default_entry: "session",
    entries: &[
        ProtocolEntryProfile {
            mode: "start",
            dsl_path: "dsl/amqp_connection_start_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "auth-denied",
            dsl_path: "dsl/amqp_auth_denied_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "publish",
            dsl_path: "dsl/amqp_basic_publish_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "consume",
            dsl_path: "dsl/amqp_basic_consume_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "session",
            dsl_path: "dsl/amqp_publish_session.gewy",
        },
    ],
};

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

pub(super) const MQTT_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "mqtt",
    default_entry: "connect",
    entries: &[
        ProtocolEntryProfile {
            mode: "connect",
            dsl_path: "dsl/mqtt_connect_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "connack",
            dsl_path: "dsl/mqtt_connack_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "publish",
            dsl_path: "dsl/mqtt_publish_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "subscribe",
            dsl_path: "dsl/mqtt_subscribe_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "disconnect",
            dsl_path: "dsl/mqtt_disconnect_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "pubrec",
            dsl_path: "dsl/mqtt_pubrec_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "pubrel",
            dsl_path: "dsl/mqtt_pubrel_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "pubcomp",
            dsl_path: "dsl/mqtt_pubcomp_path.gewy",
        },
    ],
};

pub(super) const RADIUS_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "radius",
    default_entry: "access",
    entries: &[
        ProtocolEntryProfile {
            mode: "access",
            dsl_path: "dsl/radius_access_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "challenge",
            dsl_path: "dsl/radius_challenge_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "denied",
            dsl_path: "dsl/radius_denied_path.gewy",
        },
    ],
};

pub(super) const GTPU_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "gtpu",
    default_entry: "echo",
    entries: &[ProtocolEntryProfile {
        mode: "echo",
        dsl_path: "dsl/gtpu_echo_path.gewy",
    }],
};
