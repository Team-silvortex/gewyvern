use super::{ProtocolEntryProfile, ProtocolProfile};

pub(super) const DNS_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "dns",
    default_entry: "udp",
    entries: &[
        ProtocolEntryProfile {
            mode: "udp",
            dsl_path: "dsl/dns_udp_process.gewy",
        },
        ProtocolEntryProfile {
            mode: "tcp",
            dsl_path: "dsl/dns_tcp_query_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "error",
            dsl_path: "dsl/dns_error_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "tcp-error",
            dsl_path: "dsl/dns_tcp_error_path.gewy",
        },
    ],
};

pub(super) const HTTPS_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "https",
    default_entry: "connect",
    entries: &[ProtocolEntryProfile {
        mode: "connect",
        dsl_path: "dsl/https_connect_process.gewy",
    }],
};

pub(super) const HTTP_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "http",
    default_entry: "request",
    entries: &[
        ProtocolEntryProfile {
            mode: "request",
            dsl_path: "dsl/http_request_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "response",
            dsl_path: "dsl/http_server_response_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "connect",
            dsl_path: "dsl/http_connect_tunnel_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "denied",
            dsl_path: "dsl/http_connect_denied_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "auth-required",
            dsl_path: "dsl/http_connect_auth_required_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "auth-tunnel",
            dsl_path: "dsl/http_connect_authenticated_tunnel_path.gewy",
        },
    ],
};

pub(super) const HTTP3_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "http3",
    default_entry: "request",
    entries: &[
        ProtocolEntryProfile {
            mode: "request",
            dsl_path: "dsl/http3_request_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "server",
            dsl_path: "dsl/http3_server_response_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "close",
            dsl_path: "dsl/http3_close_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "server-close",
            dsl_path: "dsl/http3_server_close_path.gewy",
        },
    ],
};

pub(super) const GRPC_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "grpc",
    default_entry: "call",
    entries: &[
        ProtocolEntryProfile {
            mode: "call",
            dsl_path: "dsl/grpc_call_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "status",
            dsl_path: "dsl/grpc_status_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "stream",
            dsl_path: "dsl/grpc_stream_path.gewy",
        },
    ],
};

pub(super) const WEBSOCKET_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "websocket",
    default_entry: "upgrade",
    entries: &[
        ProtocolEntryProfile {
            mode: "upgrade",
            dsl_path: "dsl/websocket_upgrade_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "frame",
            dsl_path: "dsl/websocket_frame_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "close",
            dsl_path: "dsl/websocket_close_path.gewy",
        },
    ],
};

pub(super) const GRAPHQL_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "graphql",
    default_entry: "query",
    entries: &[
        ProtocolEntryProfile {
            mode: "query",
            dsl_path: "dsl/graphql_query_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "mutation",
            dsl_path: "dsl/graphql_mutation_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "subscription",
            dsl_path: "dsl/graphql_subscription_path.gewy",
        },
    ],
};

pub(super) const S3_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "s3",
    default_entry: "get-object",
    entries: &[
        ProtocolEntryProfile {
            mode: "list-buckets",
            dsl_path: "dsl/s3_list_buckets_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "head-object",
            dsl_path: "dsl/s3_head_object_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "put-object",
            dsl_path: "dsl/s3_put_object_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "get-object",
            dsl_path: "dsl/s3_get_object_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "delete-object",
            dsl_path: "dsl/s3_delete_object_path.gewy",
        },
    ],
};

pub(super) const OTLP_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "otlp",
    default_entry: "traces",
    entries: &[
        ProtocolEntryProfile {
            mode: "traces",
            dsl_path: "dsl/otlp_traces_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "metrics",
            dsl_path: "dsl/otlp_metrics_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "logs",
            dsl_path: "dsl/otlp_logs_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "partial-success",
            dsl_path: "dsl/otlp_partial_success_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "export-error",
            dsl_path: "dsl/otlp_export_error_path.gewy",
        },
    ],
};

pub(super) const PROMETHEUS_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "prometheus",
    default_entry: "scrape",
    entries: &[
        ProtocolEntryProfile {
            mode: "scrape",
            dsl_path: "dsl/prometheus_scrape_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "remote-write",
            dsl_path: "dsl/prometheus_remote_write_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "query",
            dsl_path: "dsl/prometheus_query_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "alertmanager",
            dsl_path: "dsl/prometheus_alertmanager_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "rule-eval",
            dsl_path: "dsl/prometheus_rule_eval_path.gewy",
        },
    ],
};

pub(super) const LOKI_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "loki",
    default_entry: "push",
    entries: &[
        ProtocolEntryProfile {
            mode: "push",
            dsl_path: "dsl/loki_push_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "query",
            dsl_path: "dsl/loki_query_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "tail",
            dsl_path: "dsl/loki_tail_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "labels",
            dsl_path: "dsl/loki_labels_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "rules",
            dsl_path: "dsl/loki_rules_path.gewy",
        },
    ],
};

pub(super) const JAEGER_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "jaeger",
    default_entry: "collector",
    entries: &[
        ProtocolEntryProfile {
            mode: "collector",
            dsl_path: "dsl/jaeger_collector_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "agent-thrift",
            dsl_path: "dsl/jaeger_agent_thrift_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "query",
            dsl_path: "dsl/jaeger_query_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "sampling",
            dsl_path: "dsl/jaeger_sampling_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "dependencies",
            dsl_path: "dsl/jaeger_dependencies_path.gewy",
        },
    ],
};
