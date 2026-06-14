use super::{ProtocolEntryProfile, ProtocolProfile};

pub(super) const POSTGRES_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "postgres",
    default_entry: "query",
    entries: &[
        ProtocolEntryProfile {
            mode: "connect",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/postgres_connect_process.gewy",
        },
        ProtocolEntryProfile {
            mode: "auth",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/postgres_auth_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "query",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/postgres_simple_query_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "error",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/postgres_query_error_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "session",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/postgres_query_session.gewy",
        },
    ],
};

pub(super) const MYSQL_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "mysql",
    default_entry: "session",
    entries: &[
        ProtocolEntryProfile {
            mode: "connect",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/mysql_connect_process.gewy",
        },
        ProtocolEntryProfile {
            mode: "query",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/mysql_simple_query_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "session",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/mysql_query_session.gewy",
        },
        ProtocolEntryProfile {
            mode: "error",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/mysql_query_error_path.gewy",
        },
    ],
};

pub(super) const MEMCACHED_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "memcached",
    default_entry: "get",
    entries: &[
        ProtocolEntryProfile {
            mode: "get",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/memcached_get_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "set",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/memcached_set_path.gewy",
        },
    ],
};

pub(super) const AMQP_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "amqp",
    default_entry: "session",
    entries: &[
        ProtocolEntryProfile {
            mode: "start",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/amqp_connection_start_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "publish",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/amqp_basic_publish_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "consume",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/amqp_basic_consume_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "session",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/amqp_publish_session.gewy",
        },
    ],
};

pub(super) const REDIS_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "redis",
    default_entry: "ping",
    entries: &[ProtocolEntryProfile {
        mode: "ping",
        dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/redis_ping_path.gewy",
    }],
};

pub(super) const MQTT_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "mqtt",
    default_entry: "connect",
    entries: &[ProtocolEntryProfile {
        mode: "connect",
        dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/mqtt_connect_path.gewy",
    }],
};

pub(super) const RADIUS_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "radius",
    default_entry: "access",
    entries: &[ProtocolEntryProfile {
        mode: "access",
        dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/radius_access_path.gewy",
    }],
};

pub(super) const GTPU_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "gtpu",
    default_entry: "echo",
    entries: &[ProtocolEntryProfile {
        mode: "echo",
        dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/gtpu_echo_path.gewy",
    }],
};
