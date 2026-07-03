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
