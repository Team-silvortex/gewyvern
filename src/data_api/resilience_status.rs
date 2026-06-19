use crate::external_analysis::current_external_resilience_status;
use crate::socket_resilience::current_socket_resilience_status;

use super::json::json_string;

pub(super) fn api_runtime_resilience_json() -> String {
    let external = current_external_resilience_status();
    let socket = current_socket_resilience_status();
    let degraded = external.circuit_open || socket.consecutive_failures > 0;
    let status = runtime_resilience_status_label(external.circuit_open, socket.consecutive_failures);
    let severity = runtime_resilience_severity(external.circuit_open, socket.consecutive_failures);
    let summary = runtime_resilience_summary(external.circuit_open, socket.consecutive_failures);
    let mut json = String::with_capacity(1024);
    json.push_str("{\"degraded\":");
    json.push_str(if degraded { "true" } else { "false" });
    json.push_str(",\"status\":");
    json.push_str(&json_string(status));
    json.push_str(",\"severity\":");
    json.push_str(&json_string(severity));
    json.push_str(",\"summary\":");
    json.push_str(&json_string(summary));
    json.push_str(",\"recommended_actions\":");
    append_recommended_actions_json(&mut json, external.circuit_open, socket.consecutive_failures);
    json.push_str(",\"external_analysis\":{\"consecutive_failures\":");
    json.push_str(&external.consecutive_failures.to_string());
    json.push_str(",\"total_failures\":");
    json.push_str(&external.total_failures.to_string());
    json.push_str(",\"circuit_open\":");
    json.push_str(if external.circuit_open { "true" } else { "false" });
    json.push_str(",\"cooldown_remaining_ms\":");
    json.push_str(&external.cooldown_remaining_ms.to_string());
    json.push_str(",\"circuit_threshold\":");
    json.push_str(&external.circuit_threshold.to_string());
    json.push_str(",\"circuit_cooldown_seconds\":");
    json.push_str(&external.circuit_cooldown_seconds.to_string());
    json.push_str(",\"status\":");
    json.push_str(&json_string(if external.circuit_open {
        "circuit_open"
    } else if external.consecutive_failures > 0 {
        "degraded"
    } else {
        "healthy"
    }));
    json.push_str(",\"summary\":");
    json.push_str(&json_string(&external_summary(
        external.circuit_open,
        external.consecutive_failures,
        external.cooldown_remaining_ms,
    )));
    json.push_str("},\"socket_service\":{\"consecutive_failures\":");
    json.push_str(&socket.consecutive_failures.to_string());
    json.push_str(",\"total_failures\":");
    json.push_str(&socket.total_failures.to_string());
    json.push_str(",\"current_backoff_ms\":");
    json.push_str(&socket.current_backoff_ms.to_string());
    json.push_str(",\"backoff_base_ms\":");
    json.push_str(&socket.backoff_base_ms.to_string());
    json.push_str(",\"backoff_cap_ms\":");
    json.push_str(&socket.backoff_cap_ms.to_string());
    json.push_str(",\"status\":");
    json.push_str(&json_string(if socket.consecutive_failures > 0 {
        "backing_off"
    } else {
        "healthy"
    }));
    json.push_str(",\"summary\":");
    json.push_str(&json_string(&socket_summary(
        socket.consecutive_failures,
        socket.current_backoff_ms,
    )));
    json.push_str("},\"surface\":\"runtime_resilience\"}");
    json
}

pub(super) fn append_runtime_resilience_flag_json(target: &mut String) {
    let degraded = current_external_resilience_status().circuit_open
        || current_socket_resilience_status().consecutive_failures > 0;
    target.push_str("\"resilience_degraded\":");
    target.push_str(if degraded { "true" } else { "false" });
}

fn runtime_resilience_status_label(external_circuit_open: bool, socket_failures: usize) -> &'static str {
    if external_circuit_open {
        "circuit_open"
    } else if socket_failures > 0 {
        "degraded"
    } else {
        "healthy"
    }
}

fn runtime_resilience_severity(external_circuit_open: bool, socket_failures: usize) -> &'static str {
    if external_circuit_open {
        "warning"
    } else if socket_failures > 0 {
        "warning"
    } else {
        "ok"
    }
}

fn runtime_resilience_summary(external_circuit_open: bool, socket_failures: usize) -> &'static str {
    if external_circuit_open {
        "external analysis circuit is open; runtime is serving with bounded fallback"
    } else if socket_failures > 0 {
        "socket service is backing off after repeated failures"
    } else {
        "runtime resilience posture is healthy"
    }
}

fn external_summary(
    external_circuit_open: bool,
    external_failures: usize,
    cooldown_remaining_ms: u128,
) -> String {
    if external_circuit_open {
        format!(
            "external analysis circuit is open for another {cooldown_remaining_ms}ms after repeated failures"
        )
    } else if external_failures > 0 {
        format!(
            "external analysis has {external_failures} consecutive failures but the circuit is not open"
        )
    } else {
        "external analysis path is healthy".into()
    }
}

fn socket_summary(socket_failures: usize, backoff_ms: u128) -> String {
    if socket_failures > 0 {
        format!(
            "socket service is backing off after {socket_failures} consecutive failures with {backoff_ms}ms current delay"
        )
    } else {
        "socket service is healthy".into()
    }
}

fn append_recommended_actions_json(
    target: &mut String,
    external_circuit_open: bool,
    socket_failures: usize,
) {
    let mut first = true;
    target.push('[');
    if external_circuit_open {
        append_action_json(
            target,
            &mut first,
            "check the external analysis engine and wait for the cooldown window before expecting recovery",
        );
    }
    if socket_failures > 0 {
        append_action_json(
            target,
            &mut first,
            "inspect recent socket clients for malformed or incomplete sessions",
        );
        append_action_json(
            target,
            &mut first,
            "watch for socket_service_recovered before clearing the runtime from attention lists",
        );
    }
    if first {
        append_action_json(
            target,
            &mut first,
            "no operator action required",
        );
    }
    target.push(']');
}

fn append_action_json(target: &mut String, first: &mut bool, value: &str) {
    if !*first {
        target.push(',');
    }
    *first = false;
    target.push_str(&json_string(value));
}
