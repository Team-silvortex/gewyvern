use super::*;

pub(super) fn memory_state_route_response(
    config: &LearningBackendConfig,
    snapshot: Option<&DaemonSnapshot>,
) -> String {
    daemon_gateway_json_response(learning_backend_memory_state_json(config, snapshot))
}

pub(super) fn memory_model_route_response(config: &LearningBackendConfig) -> String {
    daemon_gateway_json_response(learning_backend_model_info_json(config))
}

pub(super) fn memory_versions_route_response(config: &LearningBackendConfig) -> String {
    daemon_gateway_json_response(learning_backend_memory_versions_json(config))
}

pub(super) fn memory_snapshot_route_response(config: &LearningBackendConfig) -> String {
    daemon_gateway_json_response(learning_backend_memory_snapshot_json(config))
}

pub(super) fn save_memory_slot_route_response(
    config: &LearningBackendConfig,
    request_text: &str,
) -> String {
    with_request_slot(request_text, |body, slot| {
        let label = body.optional_field("label");
        let note = body.optional_field("note");
        let source = body.optional_field("source");
        save_learning_backend_memory_slot(
            config,
            slot,
            label.as_deref(),
            note.as_deref(),
            source.as_deref(),
        )
    })
}

pub(super) fn clear_memory_route_response(
    config: &LearningBackendConfig,
    latest: &Arc<Mutex<Option<DaemonSnapshot>>>,
    daemon_state_file: Option<&Path>,
    invalidation_epoch: &Arc<AtomicU64>,
) -> String {
    daemon_gateway_json_response(clear_learning_backend_memory(
        config,
        latest,
        daemon_state_file,
        invalidation_epoch,
    ))
}

pub(super) fn load_memory_route_response(
    config: &LearningBackendConfig,
    request_text: &str,
    latest: &Arc<Mutex<Option<DaemonSnapshot>>>,
    daemon_state_file: Option<&Path>,
    invalidation_epoch: &Arc<AtomicU64>,
) -> String {
    let body = match RequestJsonBody::from_request(request_text) {
        Ok(body) => body,
        Err(response) => return response,
    };
    let slot = body.optional_field("slot");
    let strategy = body
        .optional_field("strategy")
        .unwrap_or_else(|| "replace".to_string());
    let result = if let Some(slot) = slot {
        load_learning_backend_memory_slot(
            config,
            &slot,
            &strategy,
            latest,
            daemon_state_file,
            invalidation_epoch,
        )
    } else {
        load_learning_backend_memory(
            config,
            body.body,
            latest,
            daemon_state_file,
            invalidation_epoch,
        )
    };
    daemon_gateway_json_response(result)
}

pub(super) fn delete_memory_slot_route_response(
    config: &LearningBackendConfig,
    request_text: &str,
) -> String {
    with_request_slot(request_text, |_, slot| {
        delete_learning_backend_memory_slot(config, slot)
    })
}

struct RequestJsonBody<'a> {
    body: &'a str,
}

impl RequestJsonBody<'_> {
    fn from_request(request_text: &str) -> Result<RequestJsonBody<'_>, String> {
        request_json(request_text).map_err(|err| daemon_bad_request_response(&err))
    }

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

    fn required_field_response(&self, field: &str) -> Result<String, String> {
        self.required_field(field)
            .map_err(|err| daemon_bad_request_response(&err))
    }
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

fn with_request_slot<F>(request_text: &str, f: F) -> String
where
    F: FnOnce(&RequestJsonBody<'_>, &str) -> Result<String, String>,
{
    let body = match RequestJsonBody::from_request(request_text) {
        Ok(body) => body,
        Err(response) => return response,
    };
    let slot = match body.required_field_response("slot") {
        Ok(slot) => slot,
        Err(response) => return response,
    };
    daemon_gateway_json_response(f(&body, &slot))
}
