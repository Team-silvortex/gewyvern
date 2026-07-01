use super::common::summary;
use crate::protocol_profiles::ProtocolEntrySemanticsSummary;

pub(super) fn otlp_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    match entry {
        "traces" => summary(
            "otlp-traces-export-path",
            "OpenTelemetry trace export carrying spans to a collector over OTLP/gRPC or OTLP/HTTP",
            Some("/v1/traces or TraceService/Export"),
            None,
            None,
            Some("telemetry_protocol_entry"),
        ),
        "metrics" => summary(
            "otlp-metrics-export-path",
            "OpenTelemetry metrics export where temporality, aggregation, and resource identity matter",
            Some("/v1/metrics or MetricsService/Export"),
            None,
            None,
            Some("telemetry_protocol_entry"),
        ),
        "logs" => summary(
            "otlp-logs-export-path",
            "OpenTelemetry logs export where event volume and resource attribution often explain drops",
            Some("/v1/logs or LogsService/Export"),
            None,
            None,
            Some("telemetry_protocol_entry"),
        ),
        "partial-success" => summary(
            "otlp-partial-success-path",
            "collector accepted the export request but reported dropped spans, metrics, or log records",
            Some("Export response partial_success"),
            Some("collector_partial_accept"),
            Some("telemetry_dropped_by_collector"),
            Some("telemetry_protocol_response"),
        ),
        "export-error" => summary(
            "otlp-export-error-path",
            "collector or gateway rejected the telemetry export with transport, quota, or status failure",
            Some("grpc-status non-zero or HTTP error"),
            Some("collector_rejected"),
            Some("telemetry_export_failed"),
            Some("telemetry_protocol_response"),
        ),
        _ => None,
    }
}

pub(super) fn prometheus_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    match entry {
        "scrape" => summary(
            "prometheus-scrape-path",
            "Prometheus scrape request against an exporter or service metrics endpoint",
            Some("GET /metrics"),
            None,
            None,
            Some("metrics_protocol_entry"),
        ),
        "remote-write" => summary(
            "prometheus-remote-write-path",
            "Prometheus remote-write batch sent to a compatible metrics receiver",
            Some("POST /api/v1/write"),
            None,
            None,
            Some("metrics_protocol_entry"),
        ),
        "query" => summary(
            "prometheus-query-path",
            "Prometheus HTTP API query or range query where PromQL shape and response status matter",
            Some("GET/POST /api/v1/query"),
            None,
            None,
            Some("metrics_query_protocol_entry"),
        ),
        "alertmanager" => summary(
            "prometheus-alertmanager-path",
            "Alertmanager notification or alert API request tied to alert delivery posture",
            Some("POST /api/v2/alerts"),
            None,
            None,
            Some("metrics_alert_protocol_entry"),
        ),
        "rule-eval" => summary(
            "prometheus-rule-eval-path",
            "Prometheus rule or alert state API request used to debug evaluation posture",
            Some("GET /api/v1/rules or /api/v1/alerts"),
            None,
            None,
            Some("metrics_alert_protocol_entry"),
        ),
        _ => None,
    }
}

pub(super) fn loki_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    match entry {
        "push" => summary(
            "loki-push-path",
            "Loki log batch push request sent by Promtail, an agent, or an OTLP/log bridge",
            Some("POST /loki/api/v1/push"),
            None,
            None,
            Some("log_protocol_entry"),
        ),
        "query" => summary(
            "loki-query-path",
            "Loki LogQL query or range query where selector shape and response status matter",
            Some("GET/POST /loki/api/v1/query"),
            None,
            None,
            Some("log_query_protocol_entry"),
        ),
        "tail" => summary(
            "loki-tail-path",
            "Loki live tail stream used to follow matching log lines in near real time",
            Some("GET /loki/api/v1/tail"),
            None,
            None,
            Some("log_stream_protocol_entry"),
        ),
        "labels" => summary(
            "loki-labels-path",
            "Loki label, label-values, or series metadata request used to debug stream selection",
            Some("GET /loki/api/v1/labels or /series"),
            None,
            None,
            Some("log_metadata_protocol_entry"),
        ),
        "rules" => summary(
            "loki-rules-path",
            "Loki ruler API request used to inspect or mutate log-derived alerting rules",
            Some("GET/POST /loki/api/v1/rules"),
            None,
            None,
            Some("log_alert_protocol_entry"),
        ),
        _ => None,
    }
}

pub(super) fn jaeger_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    match entry {
        "collector" => summary(
            "jaeger-collector-path",
            "Jaeger collector ingest request carrying spans over HTTP, gRPC, or compatible bridge transport",
            Some("POST /api/traces or collector gRPC"),
            None,
            None,
            Some("trace_protocol_entry"),
        ),
        "agent-thrift" => summary(
            "jaeger-agent-thrift-path",
            "Jaeger agent compact-thrift UDP span packet from an instrumented process",
            Some("UDP compact thrift agent packet"),
            None,
            None,
            Some("trace_agent_protocol_entry"),
        ),
        "query" => summary(
            "jaeger-query-path",
            "Jaeger query API request used to search or fetch trace details",
            Some("GET /api/traces"),
            None,
            None,
            Some("trace_query_protocol_entry"),
        ),
        "sampling" => summary(
            "jaeger-sampling-path",
            "Jaeger sampling strategy request used by clients or agents before span emission",
            Some("GET /sampling"),
            None,
            None,
            Some("trace_control_protocol_entry"),
        ),
        "dependencies" => summary(
            "jaeger-dependencies-path",
            "Jaeger dependencies API request used to inspect service graph relationships",
            Some("GET /api/dependencies"),
            None,
            None,
            Some("trace_query_protocol_entry"),
        ),
        _ => None,
    }
}
