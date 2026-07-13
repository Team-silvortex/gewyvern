use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Map, Value};

use super::json::json_string;

const MAX_DEPLOYMENTS: usize = 256;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ApiDeployment {
    deployment_id: String,
    request_id: String,
    pipeline_kind: String,
    requested_by: String,
    status: String,
    accepted_unix_ms: u128,
    target: Option<String>,
}

#[derive(Debug, Default)]
pub(super) struct ApiDeploymentStore {
    deployments: Vec<ApiDeployment>,
    next_id: u64,
}

pub(super) fn accept_deployment(body: &str, store: &mut ApiDeploymentStore) -> (u16, String) {
    let request = match parse_deployment_request(body) {
        Ok(request) => request,
        Err(reason) => return (400, error_json("invalid_deployment", &reason)),
    };

    if let Some(existing) = store
        .deployments
        .iter()
        .find(|deployment| deployment.request_id == request.request_id)
    {
        if existing.pipeline_kind != request.pipeline_kind
            || existing.requested_by != request.requested_by
            || existing.target != request.target
        {
            return (
                409,
                error_json(
                    "deployment_request_conflict",
                    "request_id was already used for a different deployment",
                ),
            );
        }
        return (200, deployment_json(existing, true));
    }

    store.next_id = store.next_id.saturating_add(1);
    let deployment = ApiDeployment {
        deployment_id: format!("gdep_{:016x}", store.next_id),
        request_id: request.request_id,
        pipeline_kind: request.pipeline_kind,
        requested_by: request.requested_by,
        status: "accepted".to_string(),
        accepted_unix_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or_default(),
        target: request.target,
    };
    store.deployments.push(deployment.clone());
    if store.deployments.len() > MAX_DEPLOYMENTS {
        store.deployments.remove(0);
    }
    (202, deployment_json(&deployment, false))
}

pub(super) fn deployment_list_json(store: &ApiDeploymentStore) -> String {
    let deployments = store
        .deployments
        .iter()
        .rev()
        .map(|deployment| deployment_json(deployment, false))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"count\":{},\"deployments\":[{}]}}",
        store.deployments.len(),
        deployments
    )
}

struct DeploymentRequest {
    request_id: String,
    pipeline_kind: String,
    requested_by: String,
    target: Option<String>,
}

fn parse_deployment_request(body: &str) -> Result<DeploymentRequest, String> {
    let value: Value =
        serde_json::from_str(body).map_err(|_| "request body must be a JSON object".to_string())?;
    let object = value
        .as_object()
        .ok_or_else(|| "request body must be a JSON object".to_string())?;
    reject_unknown_fields(object)?;
    if object.get("confirmed").and_then(Value::as_bool) != Some(true) {
        return Err("confirmed=true is required".to_string());
    }

    Ok(DeploymentRequest {
        request_id: required_token(object, "request_id", 128)?,
        pipeline_kind: required_token(object, "pipeline_kind", 128)?,
        requested_by: required_text(object, "requested_by", 128)?,
        target: optional_text(object, "target", 256)?,
    })
}

fn reject_unknown_fields(object: &Map<String, Value>) -> Result<(), String> {
    const ALLOWED: [&str; 5] = [
        "request_id",
        "pipeline_kind",
        "requested_by",
        "confirmed",
        "target",
    ];
    if let Some(field) = object
        .keys()
        .find(|field| !ALLOWED.contains(&field.as_str()))
    {
        return Err(format!("unknown field: {field}"));
    }
    Ok(())
}

fn required_token(
    object: &Map<String, Value>,
    field: &str,
    max_len: usize,
) -> Result<String, String> {
    let value = required_text(object, field, max_len)?;
    if !value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'/' | b'_' | b'-'))
    {
        return Err(format!(
            "{field} may contain only ASCII letters, digits, dots, slashes, dashes, or underscores"
        ));
    }
    Ok(value)
}

fn required_text(
    object: &Map<String, Value>,
    field: &str,
    max_len: usize,
) -> Result<String, String> {
    let value = object
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("{field} is required"))?;
    if value.len() > max_len || value.chars().any(char::is_control) {
        return Err(format!("{field} must not exceed {max_len} safe characters"));
    }
    Ok(value.to_string())
}

fn optional_text(
    object: &Map<String, Value>,
    field: &str,
    max_len: usize,
) -> Result<Option<String>, String> {
    match object.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if value.trim().is_empty() => Ok(None),
        Some(Value::String(value))
            if value.trim().len() <= max_len && !value.chars().any(char::is_control) =>
        {
            Ok(Some(value.trim().to_string()))
        }
        _ => Err(format!(
            "{field} must be a string no longer than {max_len} characters"
        )),
    }
}

fn deployment_json(deployment: &ApiDeployment, replayed: bool) -> String {
    format!(
        "{{\"deployment_id\":{},\"request_id\":{},\"pipeline_kind\":{},\"requested_by\":{},\"status\":{},\"accepted_unix_ms\":{},\"target\":{},\"replayed\":{}}}",
        json_string(&deployment.deployment_id),
        json_string(&deployment.request_id),
        json_string(&deployment.pipeline_kind),
        json_string(&deployment.requested_by),
        json_string(&deployment.status),
        deployment.accepted_unix_ms,
        deployment
            .target
            .as_deref()
            .map(json_string)
            .unwrap_or_else(|| "null".to_string()),
        replayed,
    )
}

fn error_json(error: &str, reason: &str) -> String {
    format!(
        "{{\"error\":{},\"reason\":{}}}",
        json_string(error),
        json_string(reason)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deployment_is_idempotent_and_rejects_conflicting_reuse() {
        let mut store = ApiDeploymentStore::default();
        let body = r#"{"request_id":"req-1","pipeline_kind":"http/request","requested_by":"operator","confirmed":true,"target":"pid:42"}"#;
        let first = accept_deployment(body, &mut store);
        let replay = accept_deployment(body, &mut store);
        let conflict = accept_deployment(
            r#"{"request_id":"req-1","pipeline_kind":"dns/udp","requested_by":"operator","confirmed":true}"#,
            &mut store,
        );
        assert_eq!(first.0, 202);
        assert_eq!(replay.0, 200);
        assert!(replay.1.contains("\"replayed\":true"));
        assert_eq!(conflict.0, 409);
        assert_eq!(store.deployments.len(), 1);
    }

    #[test]
    fn deployment_requires_confirmation_and_rejects_unknown_fields() {
        let mut store = ApiDeploymentStore::default();
        let unconfirmed = accept_deployment(
            r#"{"request_id":"req-1","pipeline_kind":"http/request","requested_by":"operator","confirmed":false}"#,
            &mut store,
        );
        let unknown = accept_deployment(
            r#"{"request_id":"req-1","pipeline_kind":"http/request","requested_by":"operator","confirmed":true,"command":"sh"}"#,
            &mut store,
        );
        assert_eq!(unconfirmed.0, 400);
        assert_eq!(unknown.0, 400);
        assert!(store.deployments.is_empty());
    }
}
