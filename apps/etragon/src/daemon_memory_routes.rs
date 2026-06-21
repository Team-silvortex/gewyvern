use super::*;

pub(super) fn memory_state_route_response(
    config: &PythonWorkerConfig,
    snapshot: Option<&DaemonSnapshot>,
) -> String {
    match python_worker_memory_state_json(config, snapshot) {
        Ok(memory_state) => daemon_http_response("HTTP/1.1 200 OK", &memory_state),
        Err(err) => daemon_http_response(
            "HTTP/1.1 502 Bad Gateway",
            &format!("{{\"error\":\"{}\"}}", escape_json_string(&err)),
        ),
    }
}

pub(super) fn memory_model_route_response(config: &PythonWorkerConfig) -> String {
    match python_worker_model_info_json(config) {
        Ok(model_info) => daemon_http_response("HTTP/1.1 200 OK", &model_info),
        Err(err) => daemon_http_response(
            "HTTP/1.1 502 Bad Gateway",
            &format!("{{\"error\":\"{}\"}}", escape_json_string(&err)),
        ),
    }
}

pub(super) fn memory_versions_route_response(config: &PythonWorkerConfig) -> String {
    match python_worker_memory_versions_json(config) {
        Ok(memory_versions) => daemon_http_response("HTTP/1.1 200 OK", &memory_versions),
        Err(err) => daemon_http_response(
            "HTTP/1.1 502 Bad Gateway",
            &format!("{{\"error\":\"{}\"}}", escape_json_string(&err)),
        ),
    }
}

pub(super) fn memory_snapshot_route_response(config: &PythonWorkerConfig) -> String {
    match python_worker_memory_snapshot_json(config) {
        Ok(memory_snapshot) => daemon_http_response("HTTP/1.1 200 OK", &memory_snapshot),
        Err(err) => daemon_http_response(
            "HTTP/1.1 502 Bad Gateway",
            &format!("{{\"error\":\"{}\"}}", escape_json_string(&err)),
        ),
    }
}

pub(super) fn save_memory_slot_route_response(
    config: &PythonWorkerConfig,
    request_text: &str,
) -> String {
    let body = match request_json_body(request_text) {
        Ok(body) => body,
        Err(err) => {
            return daemon_http_response(
                "HTTP/1.1 400 Bad Request",
                &format!("{{\"error\":\"{}\"}}", escape_json_string(&err)),
            );
        }
    };
    let slot = match request_json_field_from_body(body, "slot") {
        Ok(slot) => slot,
        Err(err) => {
            return daemon_http_response(
                "HTTP/1.1 400 Bad Request",
                &format!("{{\"error\":\"{}\"}}", escape_json_string(&err)),
            );
        }
    };
    let label = request_json_optional_field_from_body(body, "label");
    let note = request_json_optional_field_from_body(body, "note");
    let source = request_json_optional_field_from_body(body, "source");
    match save_python_worker_memory_slot(
        config,
        &slot,
        label.as_deref(),
        note.as_deref(),
        source.as_deref(),
    ) {
        Ok(saved) => daemon_http_response("HTTP/1.1 200 OK", &saved),
        Err(err) => daemon_http_response(
            "HTTP/1.1 502 Bad Gateway",
            &format!("{{\"error\":\"{}\"}}", escape_json_string(&err)),
        ),
    }
}

pub(super) fn clear_memory_route_response(
    config: &PythonWorkerConfig,
    latest: &Arc<Mutex<Option<DaemonSnapshot>>>,
    daemon_state_file: Option<&Path>,
    invalidation_epoch: &Arc<AtomicU64>,
) -> String {
    match clear_python_worker_memory(config, latest, daemon_state_file, invalidation_epoch) {
        Ok(memory_state) => daemon_http_response("HTTP/1.1 200 OK", &memory_state),
        Err(err) => daemon_http_response(
            "HTTP/1.1 502 Bad Gateway",
            &format!("{{\"error\":\"{}\"}}", escape_json_string(&err)),
        ),
    }
}

pub(super) fn load_memory_route_response(
    config: &PythonWorkerConfig,
    request_text: &str,
    latest: &Arc<Mutex<Option<DaemonSnapshot>>>,
    daemon_state_file: Option<&Path>,
    invalidation_epoch: &Arc<AtomicU64>,
) -> String {
    let body = match request_json_body(request_text) {
        Ok(body) => body,
        Err(err) => {
            return daemon_http_response(
                "HTTP/1.1 400 Bad Request",
                &format!("{{\"error\":\"{}\"}}", escape_json_string(&err)),
            );
        }
    };
    let slot = request_json_field_from_body(body, "slot").ok();
    let strategy =
        request_json_field_from_body(body, "strategy").unwrap_or_else(|_| "replace".to_string());
    let result = if let Some(slot) = slot {
        load_python_worker_memory_slot(
            config,
            &slot,
            &strategy,
            latest,
            daemon_state_file,
            invalidation_epoch,
        )
    } else {
        load_python_worker_memory(config, body, latest, daemon_state_file, invalidation_epoch)
    };
    match result {
        Ok(memory_state) => daemon_http_response("HTTP/1.1 200 OK", &memory_state),
        Err(err) => daemon_http_response(
            "HTTP/1.1 502 Bad Gateway",
            &format!("{{\"error\":\"{}\"}}", escape_json_string(&err)),
        ),
    }
}

pub(super) fn delete_memory_slot_route_response(
    config: &PythonWorkerConfig,
    request_text: &str,
) -> String {
    let slot = match request_json_field(request_text, "slot") {
        Ok(slot) => slot,
        Err(err) => {
            return daemon_http_response(
                "HTTP/1.1 400 Bad Request",
                &format!("{{\"error\":\"{}\"}}", escape_json_string(&err)),
            );
        }
    };
    match delete_python_worker_memory_slot(config, &slot) {
        Ok(deleted) => daemon_http_response("HTTP/1.1 200 OK", &deleted),
        Err(err) => daemon_http_response(
            "HTTP/1.1 502 Bad Gateway",
            &format!("{{\"error\":\"{}\"}}", escape_json_string(&err)),
        ),
    }
}

fn request_json_body(request_text: &str) -> Result<&str, String> {
    request_text
        .split_once("\r\n\r\n")
        .or_else(|| request_text.split_once("\n\n"))
        .map(|(_, body)| body.trim())
        .filter(|body| !body.is_empty())
        .ok_or_else(|| "missing_json_body".to_string())
}

fn request_json_field(request_text: &str, field: &str) -> Result<String, String> {
    let body = request_json_body(request_text)?;
    request_json_field_from_body(body, field)
}

fn request_json_field_from_body(body: &str, field: &str) -> Result<String, String> {
    let needle = format!("\"{}\":\"", field);
    let start = body
        .find(&needle)
        .ok_or_else(|| format!("missing_{}", field))?;
    let rest = &body[start + needle.len()..];
    let end = rest.find('"').ok_or_else(|| format!("invalid_{}", field))?;
    Ok(rest[..end].to_string())
}

fn request_json_optional_field_from_body(body: &str, field: &str) -> Option<String> {
    request_json_field_from_body(body, field).ok()
}
