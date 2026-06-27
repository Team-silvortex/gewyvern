use super::common::{failure, summary};
use crate::protocol_profiles::ProtocolEntrySemanticsSummary;

pub(super) fn mysql_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    match entry {
        "auth-denied" => failure(
            "database authentication rejection during MySQL handshake response evaluation",
            Some("ERR"),
            Some("server_denied"),
            Some("access_denied"),
        ),
        "error" => failure(
            "database error response during MySQL query result handling",
            Some("ERR"),
            Some("semantic_error"),
            Some("protocol_error"),
        ),
        _ => None,
    }
}

pub(super) fn postgres_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    match entry {
        "auth-denied" => failure(
            "database authentication rejection after PostgreSQL password exchange",
            Some("ErrorResponse"),
            Some("server_denied"),
            Some("access_denied"),
        ),
        "error" => failure(
            "database error frame during PostgreSQL query result handling",
            Some("ErrorResponse"),
            Some("semantic_error"),
            Some("protocol_error"),
        ),
        _ => None,
    }
}

pub(super) fn redis_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    let (operator_focus, typical_signal, failure_mode, failure_detail) = match entry {
        "auth-required" => (
            "authentication gate before command execution",
            Some("-NOAUTH"),
            Some("server_denied"),
            Some("auth_required"),
        ),
        "auth-denied" => (
            "credential rejection after AUTH",
            Some("-WRONGPASS"),
            Some("server_denied"),
            Some("access_denied"),
        ),
        "error" => (
            "generic command or request semantic failure",
            Some("-ERR"),
            Some("semantic_error"),
            Some("protocol_error"),
        ),
        "wrongtype" => (
            "key-type mismatch against requested command",
            Some("-WRONGTYPE"),
            Some("semantic_error"),
            Some("protocol_constraint_violation"),
        ),
        "busygroup" => (
            "stream consumer-group creation conflict",
            Some("-BUSYGROUP"),
            Some("semantic_error"),
            Some("protocol_error"),
        ),
        "readonly" => (
            "replica write refusal or readonly placement mismatch",
            Some("-READONLY"),
            Some("server_denied"),
            Some("access_denied"),
        ),
        "noscript" => (
            "server script cache miss before EVALSHA-style reuse",
            Some("-NOSCRIPT"),
            Some("semantic_error"),
            Some("protocol_error"),
        ),
        "moved" => (
            "cluster slot redirect that requires target remap",
            Some("-MOVED"),
            Some("semantic_error"),
            Some("protocol_error"),
        ),
        "ask" => (
            "temporary cluster redirect that expects ASKING on retry",
            Some("-ASK"),
            Some("semantic_error"),
            Some("protocol_error"),
        ),
        "tryagain" => (
            "retry-needed transient cluster or script window",
            Some("-TRYAGAIN"),
            Some("semantic_error"),
            Some("protocol_error"),
        ),
        "loading" => (
            "server warmup window before command acceptance",
            Some("-LOADING"),
            Some("semantic_error"),
            Some("protocol_error"),
        ),
        "crossslot" => (
            "multi-key cluster slot mismatch",
            Some("-CROSSSLOT"),
            Some("semantic_error"),
            Some("protocol_error"),
        ),
        "clusterdown" => (
            "cluster topology unavailable for routing",
            Some("-CLUSTERDOWN"),
            Some("semantic_error"),
            Some("protocol_error"),
        ),
        "masterdown" => (
            "primary unavailable during failover window",
            Some("-MASTERDOWN"),
            Some("semantic_error"),
            Some("protocol_error"),
        ),
        "oom" => (
            "memory policy refusal for write amplification",
            Some("-OOM"),
            Some("semantic_error"),
            Some("protocol_error"),
        ),
        "busy" => (
            "Lua or long-running server-side script contention",
            Some("-BUSY"),
            Some("semantic_error"),
            Some("protocol_error"),
        ),
        "execabort" => (
            "transaction abort before EXEC completion",
            Some("-EXECABORT"),
            Some("semantic_error"),
            Some("protocol_error"),
        ),
        "misconf" => (
            "persistence or write-guard policy rejection",
            Some("-MISCONF"),
            Some("semantic_error"),
            Some("protocol_error"),
        ),
        _ => return None,
    };
    failure(operator_focus, typical_signal, failure_mode, failure_detail)
}

pub(super) fn kafka_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    let (category, operator_focus, typical_signal) = match entry {
        "metadata" => (
            "broker-metadata-path",
            "Kafka broker metadata lookup on TCP port 9092",
            Some("Metadata API key"),
        ),
        "produce" => (
            "stream-produce-path",
            "Kafka produce request/response against broker topic partitions",
            Some("Produce API key"),
        ),
        "fetch" => (
            "stream-fetch-path",
            "Kafka fetch request/response from broker topic partitions",
            Some("Fetch API key"),
        ),
        _ => return None,
    };
    summary(
        category,
        operator_focus,
        typical_signal,
        None,
        None,
        Some("protocol_entry_signal"),
    )
}

pub(super) fn nats_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    let (category, operator_focus, typical_signal) = match entry {
        "connect" => (
            "message-session-path",
            "NATS INFO/CONNECT session setup on TCP port 4222",
            Some("INFO / CONNECT"),
        ),
        "pub" => (
            "message-publish-path",
            "NATS PUB command sending a subject payload",
            Some("PUB"),
        ),
        "sub" => (
            "message-subscribe-path",
            "NATS SUB command and MSG delivery",
            Some("SUB / MSG"),
        ),
        _ => return None,
    };
    summary(
        category,
        operator_focus,
        typical_signal,
        None,
        None,
        Some("protocol_entry_signal"),
    )
}
