use super::super::ShelfMatch;

pub(crate) fn grpc_shelf(entry: &str) -> Option<ShelfMatch> {
    const CALL: &[&str] = &["call"];
    const STATUS: &[&str] = &["status"];
    const STREAM: &[&str] = &["stream"];
    if CALL.contains(&entry) {
        Some((
            "call",
            "Unary Call",
            "docs/book/reference-grpc-call-surface.md",
            CALL,
        ))
    } else if STATUS.contains(&entry) {
        Some((
            "status",
            "Status Trailer",
            "docs/book/reference-grpc-status-surface.md",
            STATUS,
        ))
    } else if STREAM.contains(&entry) {
        Some((
            "stream",
            "Streaming RPC",
            "docs/book/reference-grpc-stream-surface.md",
            STREAM,
        ))
    } else {
        None
    }
}

pub(crate) fn websocket_shelf(entry: &str) -> Option<ShelfMatch> {
    const UPGRADE: &[&str] = &["upgrade"];
    const FRAME: &[&str] = &["frame"];
    const CLOSE: &[&str] = &["close"];
    if UPGRADE.contains(&entry) {
        Some((
            "upgrade",
            "HTTP Upgrade",
            "docs/book/reference-websocket-upgrade-surface.md",
            UPGRADE,
        ))
    } else if FRAME.contains(&entry) {
        Some((
            "frame",
            "Data Frame",
            "docs/book/reference-websocket-frame-surface.md",
            FRAME,
        ))
    } else if CLOSE.contains(&entry) {
        Some((
            "close",
            "Close Control",
            "docs/book/reference-websocket-close-surface.md",
            CLOSE,
        ))
    } else {
        None
    }
}

pub(crate) fn graphql_shelf(entry: &str) -> Option<ShelfMatch> {
    const QUERY: &[&str] = &["query"];
    const MUTATION: &[&str] = &["mutation"];
    const SUBSCRIPTION: &[&str] = &["subscription"];
    if QUERY.contains(&entry) {
        Some((
            "query",
            "Query",
            "docs/book/reference-graphql-query-surface.md",
            QUERY,
        ))
    } else if MUTATION.contains(&entry) {
        Some((
            "mutation",
            "Mutation",
            "docs/book/reference-graphql-mutation-surface.md",
            MUTATION,
        ))
    } else if SUBSCRIPTION.contains(&entry) {
        Some((
            "subscription",
            "Subscription",
            "docs/book/reference-graphql-subscription-surface.md",
            SUBSCRIPTION,
        ))
    } else {
        None
    }
}

pub(crate) fn s3_shelf(entry: &str) -> Option<ShelfMatch> {
    const READ: &[&str] = &["list-buckets", "head-object", "get-object"];
    const WRITE: &[&str] = &["put-object", "delete-object"];
    if READ.contains(&entry) {
        Some((
            "object-read",
            "Object Read And Metadata",
            "docs/book/reference-s3-object-read-surface.md",
            READ,
        ))
    } else if WRITE.contains(&entry) {
        Some((
            "object-write",
            "Object Write And Mutation",
            "docs/book/reference-s3-object-write-surface.md",
            WRITE,
        ))
    } else {
        None
    }
}

pub(crate) fn otlp_shelf(entry: &str) -> Option<ShelfMatch> {
    const SIGNAL_EXPORT: &[&str] = &["traces", "metrics", "logs"];
    const COLLECTOR_RESPONSE: &[&str] = &["partial-success", "export-error"];
    if SIGNAL_EXPORT.contains(&entry) {
        Some((
            "signal-export",
            "Signal Export",
            "docs/book/reference-otlp-signal-export-surface.md",
            SIGNAL_EXPORT,
        ))
    } else if COLLECTOR_RESPONSE.contains(&entry) {
        Some((
            "collector-response",
            "Collector Response",
            "docs/book/reference-otlp-collector-response-surface.md",
            COLLECTOR_RESPONSE,
        ))
    } else {
        None
    }
}

pub(crate) fn prometheus_shelf(entry: &str) -> Option<ShelfMatch> {
    const COLLECTION: &[&str] = &["scrape", "remote-write"];
    const QUERYING: &[&str] = &["query"];
    const ALERTING: &[&str] = &["alertmanager", "rule-eval"];
    if COLLECTION.contains(&entry) {
        Some((
            "metrics-collection",
            "Metrics Collection",
            "docs/book/reference-prometheus-metrics-collection-surface.md",
            COLLECTION,
        ))
    } else if QUERYING.contains(&entry) {
        Some((
            "query-api",
            "Query API",
            "docs/book/reference-prometheus-query-surface.md",
            QUERYING,
        ))
    } else if ALERTING.contains(&entry) {
        Some((
            "alerting",
            "Alerting",
            "docs/book/reference-prometheus-alerting-surface.md",
            ALERTING,
        ))
    } else {
        None
    }
}

pub(crate) fn loki_shelf(entry: &str) -> Option<ShelfMatch> {
    const INGEST: &[&str] = &["push"];
    const READ: &[&str] = &["query", "tail", "labels"];
    const RULER: &[&str] = &["rules"];
    if INGEST.contains(&entry) {
        Some((
            "log-ingest",
            "Log Ingest",
            "docs/book/reference-loki-log-ingest-surface.md",
            INGEST,
        ))
    } else if READ.contains(&entry) {
        Some((
            "log-read",
            "Log Query And Metadata",
            "docs/book/reference-loki-log-read-surface.md",
            READ,
        ))
    } else if RULER.contains(&entry) {
        Some((
            "ruler",
            "Ruler",
            "docs/book/reference-loki-ruler-surface.md",
            RULER,
        ))
    } else {
        None
    }
}

pub(crate) fn jaeger_shelf(entry: &str) -> Option<ShelfMatch> {
    const INGEST: &[&str] = &["collector", "agent-thrift"];
    const READ: &[&str] = &["query", "dependencies"];
    const CONTROL: &[&str] = &["sampling"];
    if INGEST.contains(&entry) {
        Some((
            "trace-ingest",
            "Trace Ingest",
            "docs/book/reference-jaeger-trace-ingest-surface.md",
            INGEST,
        ))
    } else if READ.contains(&entry) {
        Some((
            "trace-read",
            "Trace Query And Dependencies",
            "docs/book/reference-jaeger-trace-read-surface.md",
            READ,
        ))
    } else if CONTROL.contains(&entry) {
        Some((
            "sampling-control",
            "Sampling Control",
            "docs/book/reference-jaeger-sampling-surface.md",
            CONTROL,
        ))
    } else {
        None
    }
}
