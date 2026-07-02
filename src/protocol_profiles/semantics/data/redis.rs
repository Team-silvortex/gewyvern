use super::super::common::{failure, summary};
use crate::protocol_profiles::ProtocolEntrySemanticsSummary;

pub(super) fn redis_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    match entry {
        "xadd" => {
            return summary(
                "redis-stream-append-path",
                "Redis XADD appending an event into a stream and returning the entry id",
                Some("XADD command + bulk string entry id"),
                None,
                None,
                Some("protocol_entry_signal"),
            );
        }
        "xread" | "xreadgroup" => {
            return summary(
                "redis-stream-read-path",
                "Redis stream read operation consuming entries from one or more streams",
                Some("XREAD/XREADGROUP command + array reply"),
                None,
                None,
                Some("protocol_entry_signal"),
            );
        }
        "xack" => {
            return summary(
                "redis-stream-ack-path",
                "Redis XACK acknowledging processed stream messages for a consumer group",
                Some("XACK command + integer reply"),
                None,
                None,
                Some("protocol_entry_signal"),
            );
        }
        "zadd" => {
            return summary(
                "redis-sorted-set-write-path",
                "Redis ZADD updating sorted-set scores and returning changed member count",
                Some("ZADD command + integer reply"),
                None,
                None,
                Some("protocol_entry_signal"),
            );
        }
        "zrange" | "zrangebyscore" | "zrevrangebyscore" => {
            return summary(
                "redis-sorted-set-read-path",
                "Redis sorted-set range read returning ordered members by rank or score",
                Some("ZRANGE-style command + array reply"),
                None,
                None,
                Some("protocol_entry_signal"),
            );
        }
        "zpopmin" | "zpopmax" | "bzpopmin" | "bzpopmax" | "zmpop" | "bzmpop" => {
            return summary(
                "redis-sorted-set-pop-path",
                "Redis sorted-set pop removing min/max scored members from one or more keys",
                Some("ZPOP/BZPOP/ZMPOP command + member reply"),
                None,
                None,
                Some("protocol_entry_signal"),
            );
        }
        _ => {}
    }

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
