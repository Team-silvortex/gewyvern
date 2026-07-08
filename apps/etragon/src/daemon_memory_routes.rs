use super::*;

pub(super) fn memory_state_route_response(
    config: &PythonWorkerConfig,
    snapshot: Option<&DaemonSnapshot>,
) -> String {
    gateway_json_response(python_worker_memory_state_json(config, snapshot))
}

pub(super) fn memory_model_route_response(config: &PythonWorkerConfig) -> String {
    gateway_json_response(python_worker_model_info_json(config))
}

pub(super) fn memory_versions_route_response(config: &PythonWorkerConfig) -> String {
    gateway_json_response(python_worker_memory_versions_json(config))
}

pub(super) fn memory_snapshot_route_response(config: &PythonWorkerConfig) -> String {
    gateway_json_response(python_worker_memory_snapshot_json(config))
}

pub(super) fn save_memory_slot_route_response(
    config: &PythonWorkerConfig,
    request_text: &str,
) -> String {
    let body = match request_json(request_text) {
        Ok(body) => body,
        Err(err) => return bad_request_response(&err),
    };
    let slot = match body.required_field("slot") {
        Ok(slot) => slot,
        Err(err) => return bad_request_response(&err),
    };
    let label = body.optional_field("label");
    let note = body.optional_field("note");
    let source = body.optional_field("source");
    gateway_json_response(save_python_worker_memory_slot(
        config,
        &slot,
        label.as_deref(),
        note.as_deref(),
        source.as_deref(),
    ))
}

pub(super) fn clear_memory_route_response(
    config: &PythonWorkerConfig,
    latest: &Arc<Mutex<Option<DaemonSnapshot>>>,
    daemon_state_file: Option<&Path>,
    invalidation_epoch: &Arc<AtomicU64>,
) -> String {
    gateway_json_response(clear_python_worker_memory(
        config,
        latest,
        daemon_state_file,
        invalidation_epoch,
    ))
}

pub(super) fn load_memory_route_response(
    config: &PythonWorkerConfig,
    request_text: &str,
    latest: &Arc<Mutex<Option<DaemonSnapshot>>>,
    daemon_state_file: Option<&Path>,
    invalidation_epoch: &Arc<AtomicU64>,
) -> String {
    let body = match request_json(request_text) {
        Ok(body) => body,
        Err(err) => return bad_request_response(&err),
    };
    let slot = body.optional_field("slot");
    let strategy = body
        .optional_field("strategy")
        .unwrap_or_else(|| "replace".to_string());
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
        load_python_worker_memory(
            config,
            body.body,
            latest,
            daemon_state_file,
            invalidation_epoch,
        )
    };
    gateway_json_response(result)
}

pub(super) fn delete_memory_slot_route_response(
    config: &PythonWorkerConfig,
    request_text: &str,
) -> String {
    let body = match request_json(request_text) {
        Ok(body) => body,
        Err(err) => return bad_request_response(&err),
    };
    let slot = match body.required_field("slot") {
        Ok(slot) => slot,
        Err(err) => return bad_request_response(&err),
    };
    gateway_json_response(delete_python_worker_memory_slot(config, &slot))
}

struct RequestJsonBody<'a> {
    body: &'a str,
}

fn request_json(request_text: &str) -> Result<RequestJsonBody<'_>, String> {
    request_text
        .split_once("\r\n\r\n")
        .or_else(|| request_text.split_once("\n\n"))
        .map(|(_, body)| body.trim())
        .filter(|body| !body.is_empty())
        .map(|body| RequestJsonBody { body })
        .ok_or_else(|| "missing_json_body".to_string())
}

impl RequestJsonBody<'_> {
    fn required_field(&self, field: &str) -> Result<String, String> {
        let needle = format!("\"{}\":\"", field);
        let start = self
            .body
            .find(&needle)
            .ok_or_else(|| format!("missing_{}", field))?;
        let rest = &self.body[start + needle.len()..];
        let end = rest.find('"').ok_or_else(|| format!("invalid_{}", field))?;
        Ok(rest[..end].to_string())
    }

    fn optional_field(&self, field: &str) -> Option<String> {
        self.required_field(field).ok()
    }
}

fn gateway_json_response(result: Result<String, String>) -> String {
    match result {
        Ok(body) => daemon_http_response("HTTP/1.1 200 OK", &body),
        Err(err) => daemon_http_response(
            "HTTP/1.1 502 Bad Gateway",
            &format!("{{\"error\":\"{}\"}}", escape_json_string(&err)),
        ),
    }
}

fn bad_request_response(err: &str) -> String {
    daemon_http_response(
        "HTTP/1.1 400 Bad Request",
        &format!("{{\"error\":\"{}\"}}", escape_json_string(err)),
    )
}
