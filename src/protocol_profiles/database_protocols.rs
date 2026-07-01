use super::{ProtocolEntryProfile, ProtocolProfile};

pub(super) const MSSQL_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "mssql",
    default_entry: "query",
    entries: &[
        ProtocolEntryProfile {
            mode: "prelogin",
            dsl_path: "dsl/mssql_prelogin_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "login",
            dsl_path: "dsl/mssql_login_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "query",
            dsl_path: "dsl/mssql_query_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "response",
            dsl_path: "dsl/mssql_response_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "colmetadata",
            dsl_path: "dsl/mssql_colmetadata_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "row",
            dsl_path: "dsl/mssql_row_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "done",
            dsl_path: "dsl/mssql_done_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "envchange",
            dsl_path: "dsl/mssql_envchange_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "error",
            dsl_path: "dsl/mssql_error_path.gewy",
        },
    ],
};

pub(super) const ELASTICSEARCH_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "elasticsearch",
    default_entry: "search",
    entries: &[
        ProtocolEntryProfile {
            mode: "health",
            dsl_path: "dsl/elasticsearch_health_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "search",
            dsl_path: "dsl/elasticsearch_search_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "index",
            dsl_path: "dsl/elasticsearch_index_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "bulk",
            dsl_path: "dsl/elasticsearch_bulk_path.gewy",
        },
    ],
};

pub(super) const ETCD_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "etcd",
    default_entry: "range",
    entries: &[
        ProtocolEntryProfile {
            mode: "health",
            dsl_path: "dsl/etcd_health_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "range",
            dsl_path: "dsl/etcd_range_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "put",
            dsl_path: "dsl/etcd_put_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "watch",
            dsl_path: "dsl/etcd_watch_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "lease",
            dsl_path: "dsl/etcd_lease_path.gewy",
        },
    ],
};
