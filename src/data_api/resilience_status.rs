use crate::external_analysis::current_external_resilience_status;
use crate::socket_resilience::current_socket_resilience_status;

use super::json::json_string;

struct RuntimeResilienceView<'a> {
    degraded: bool,
    status: &'a str,
    severity: &'a str,
    summary: &'a str,
    recommended_actions: Vec<&'a str>,
    external_status: &'a str,
    external_summary: String,
    socket_status: &'a str,
    socket_summary: String,
}

pub(super) fn api_runtime_resilience_json() -> String {
    let external = current_external_resilience_status();
    let socket = current_socket_resilience_status();
    let view = build_runtime_resilience_view(external, socket);
    let mut json = String::with_capacity(1024);
    json.push_str("{\"degraded\":");
    json.push_str(if view.degraded { "true" } else { "false" });
    json.push_str(",\"status\":");
    json.push_str(&json_string(view.status));
    json.push_str(",\"severity\":");
    json.push_str(&json_string(view.severity));
    json.push_str(",\"summary\":");
    json.push_str(&json_string(view.summary));
    json.push_str(",\"recommended_actions\":");
    append_string_list_json(&mut json, &view.recommended_actions);
    json.push_str(",\"external_analysis\":{\"consecutive_failures\":");
    json.push_str(&external.consecutive_failures.to_string());
    json.push_str(",\"total_failures\":");
    json.push_str(&external.total_failures.to_string());
    json.push_str(",\"circuit_open\":");
    json.push_str(if external.circuit_open {
        "true"
    } else {
        "false"
    });
    json.push_str(",\"cooldown_remaining_ms\":");
    json.push_str(&external.cooldown_remaining_ms.to_string());
    json.push_str(",\"circuit_threshold\":");
    json.push_str(&external.circuit_threshold.to_string());
    json.push_str(",\"circuit_cooldown_seconds\":");
    json.push_str(&external.circuit_cooldown_seconds.to_string());
    json.push_str(",\"status\":");
    json.push_str(&json_string(view.external_status));
    json.push_str(",\"summary\":");
    json.push_str(&json_string(&view.external_summary));
    json.push_str("},\"socket_service\":{\"consecutive_failures\":");
    json.push_str(&socket.consecutive_failures.to_string());
    json.push_str(",\"total_failures\":");
    json.push_str(&socket.total_failures.to_string());
    json.push_str(",\"consecutive_idle_timeouts\":");
    json.push_str(&socket.consecutive_idle_timeouts.to_string());
    json.push_str(",\"total_idle_timeouts\":");
    json.push_str(&socket.total_idle_timeouts.to_string());
    json.push_str(",\"current_backoff_ms\":");
    json.push_str(&socket.current_backoff_ms.to_string());
    json.push_str(",\"backoff_base_ms\":");
    json.push_str(&socket.backoff_base_ms.to_string());
    json.push_str(",\"backoff_cap_ms\":");
    json.push_str(&socket.backoff_cap_ms.to_string());
    json.push_str(",\"status\":");
    json.push_str(&json_string(view.socket_status));
    json.push_str(",\"summary\":");
    json.push_str(&json_string(&view.socket_summary));
    json.push_str("},\"surface\":\"runtime_resilience\"}");
    json
}

pub(super) fn append_runtime_resilience_flag_json(target: &mut String) {
    let degraded = build_runtime_resilience_view(
        current_external_resilience_status(),
        current_socket_resilience_status(),
    )
    .degraded;
    target.push_str("\"resilience_degraded\":");
    target.push_str(if degraded { "true" } else { "false" });
}

fn runtime_resilience_status_label(
    external_circuit_open: bool,
    socket_failures: usize,
    socket_idle_timeouts: usize,
) -> &'static str {
    if external_circuit_open {
        "circuit_open"
    } else if socket_failures > 0 {
        "degraded"
    } else if socket_idle_timeouts > 0 {
        "idle_ready"
    } else {
        "healthy"
    }
}

fn runtime_resilience_severity(
    external_circuit_open: bool,
    socket_failures: usize,
    _socket_idle_timeouts: usize,
) -> &'static str {
    if external_circuit_open || socket_failures > 0 {
        "warning"
    } else {
        "ok"
    }
}

fn runtime_resilience_summary(
    external_circuit_open: bool,
    socket_failures: usize,
    socket_idle_timeouts: usize,
) -> &'static str {
    if external_circuit_open {
        "external analysis circuit is open; runtime is serving with bounded fallback"
    } else if socket_failures > 0 {
        "socket service is backing off after repeated failures"
    } else if socket_idle_timeouts > 0 {
        "socket service is idle and healthy while waiting for the next client"
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

fn socket_summary(
    socket_failures: usize,
    socket_idle_timeouts: usize,
    total_idle_timeouts: usize,
    backoff_ms: u128,
) -> String {
    if socket_failures > 0 {
        format!(
            "socket service is backing off after {socket_failures} consecutive failures with {backoff_ms}ms current delay"
        )
    } else if socket_idle_timeouts > 0 {
        format!(
            "socket service is idle after {socket_idle_timeouts} consecutive timeout polls ({total_idle_timeouts} total idle polls observed)"
        )
    } else {
        "socket service is healthy".into()
    }
}

fn build_runtime_resilience_view(
    external: crate::external_analysis::ExternalResilienceStatus,
    socket: crate::socket_resilience::SocketResilienceStatus,
) -> RuntimeResilienceView<'static> {
    let degraded = external.circuit_open || socket.consecutive_failures > 0;
    RuntimeResilienceView {
        degraded,
        status: runtime_resilience_status_label(
            external.circuit_open,
            socket.consecutive_failures,
            socket.consecutive_idle_timeouts,
        ),
        severity: runtime_resilience_severity(
            external.circuit_open,
            socket.consecutive_failures,
            socket.consecutive_idle_timeouts,
        ),
        summary: runtime_resilience_summary(
            external.circuit_open,
            socket.consecutive_failures,
            socket.consecutive_idle_timeouts,
        ),
        recommended_actions: recommended_actions(
            external.circuit_open,
            socket.consecutive_failures,
            socket.consecutive_idle_timeouts,
        ),
        external_status: external_status_label(
            external.circuit_open,
            external.consecutive_failures,
        ),
        external_summary: external_summary(
            external.circuit_open,
            external.consecutive_failures,
            external.cooldown_remaining_ms,
        ),
        socket_status: socket_status_label(
            socket.consecutive_failures,
            socket.consecutive_idle_timeouts,
        ),
        socket_summary: socket_summary(
            socket.consecutive_failures,
            socket.consecutive_idle_timeouts,
            socket.total_idle_timeouts,
            socket.current_backoff_ms,
        ),
    }
}

fn external_status_label(external_circuit_open: bool, external_failures: usize) -> &'static str {
    if external_circuit_open {
        "circuit_open"
    } else if external_failures > 0 {
        "degraded"
    } else {
        "healthy"
    }
}

fn socket_status_label(socket_failures: usize, socket_idle_timeouts: usize) -> &'static str {
    if socket_failures > 0 {
        "backing_off"
    } else if socket_idle_timeouts > 0 {
        "idle"
    } else {
        "healthy"
    }
}

fn recommended_actions(
    external_circuit_open: bool,
    socket_failures: usize,
    socket_idle_timeouts: usize,
) -> Vec<&'static str> {
    let mut actions = Vec::with_capacity(3);
    if external_circuit_open {
        actions.push(
            "check the external analysis engine and wait for the cooldown window before expecting recovery",
        );
    }
    if socket_failures > 0 {
        actions.push("inspect recent socket clients for malformed or incomplete sessions");
        actions.push(
            "watch for socket_service_recovered before clearing the runtime from attention lists",
        );
    }
    if socket_failures == 0 && socket_idle_timeouts > 0 && !external_circuit_open {
        actions.push("no operator action required while the runtime is idle and awaiting a client");
    }
    if actions.is_empty() {
        actions.push("no operator action required");
    }
    actions
}

fn append_string_list_json(target: &mut String, values: &[&str]) {
    target.push('[');
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            target.push(',');
        }
        target.push_str(&json_string(value));
    }
    target.push(']');
}
