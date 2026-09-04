use std::collections::{BTreeMap, BTreeSet};

use leserpent_domain::{
    CAPABILITY_ORCHESTRA_WRITE, CapabilitySet, CommandId, DomainError, Principal, RuntimeId,
    RuntimeProjection,
};
use leserpent_protocol::compatibility_v1::{
    LegacyOrchestraEvent, LegacyOrchestraRun, LegacyOrchestraStep,
};
use leserpent_protocol::{
    OrchestraCancelCommandRequest, OrchestraPlan, OrchestraPlanCatalogRequest,
    OrchestraPlanCatalogResponse, OrchestraRetryCommandRequest, OrchestraRunCommandRequest,
};
use leserpent_runtime::{ControlRuntime, RuntimeError};
use ring::digest;
use ring::rand::{SecureRandom, SystemRandom};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::orchestra::{self, OrchestraAuthorityError};
use crate::web_console::MAX_ORCHESTRA_COMMAND_BYTES;
use crate::web_console_error::{ConsoleWriteError, ConsoleWriteStatus};

const WEB_ORCHESTRA_PRINCIPAL: &str = "rust-web-console";
const MAX_WEB_ORCHESTRA_RUNS: usize = 4_096;
const MAX_WEB_ORCHESTRA_EVENTS: usize = 256;
const MAX_WEB_ORCHESTRA_FLEET_ITEMS: usize = 256;
const HISTORY_PAGE_SIZE: u16 = 64;
const MAX_WEB_COMMAND_ID_BYTES: usize = 96;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExecuteRequest {
    confirmed: bool,
    expected_revision: Option<String>,
    approved_by: Option<String>,
    approval_note: Option<String>,
    request_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RetryRequest {
    confirmed: bool,
    approved_by: Option<String>,
    approval_note: Option<String>,
    request_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SessionHandoffRequest {
    pipeline_kind: String,
    requested_by: String,
    #[serde(default)]
    request_id: Option<String>,
}

pub(crate) fn plan_value(
    runtime: &ControlRuntime,
    runtime_id: &RuntimeId,
) -> Result<Value, ConsoleWriteError> {
    let projection = runtime
        .runtime_projection(runtime_id)
        .ok_or_else(runtime_not_found)?;
    let catalog = orchestra::plan_catalog(
        runtime,
        &OrchestraPlanCatalogRequest {
            principal: web_principal(),
            capabilities: web_capabilities(),
            runtime_id: runtime_id.clone(),
        },
    )
    .map_err(map_authority_error)?;
    Ok(plan_catalog_value(projection, &catalog))
}

pub(crate) fn runtime_runs_value(
    runtime: &mut ControlRuntime,
    runtime_id: &RuntimeId,
) -> Result<Value, ConsoleWriteError> {
    if runtime.runtime_projection(runtime_id).is_none() {
        return Err(runtime_not_found());
    }
    let runs = load_runs(runtime, Some(runtime_id.as_str()))?;
    Ok(json!({
        "runtimeId": runtime_id.as_str(),
        "runs": runs,
    }))
}

pub(crate) fn run_events_value(
    runtime: &mut ControlRuntime,
    runtime_id: &RuntimeId,
    run_id: &str,
) -> Result<Value, ConsoleWriteError> {
    if runtime.runtime_projection(runtime_id).is_none() {
        return Err(runtime_not_found());
    }
    if !load_runs(runtime, Some(runtime_id.as_str()))?
        .iter()
        .any(|run| run.run_id == run_id)
    {
        return Err(ConsoleWriteError::new(
            ConsoleWriteStatus::NotFound,
            "orchestra_run_not_found",
            "Orchestra run was not found",
        ));
    }
    let mut offset = 0_u32;
    let mut events = Vec::new();
    loop {
        let page = runtime
            .load_orchestra_history(
                Some(runtime_id.as_str()),
                Some(run_id),
                offset,
                HISTORY_PAGE_SIZE,
            )
            .map_err(map_runtime_error)?;
        for (event_id, envelope) in page.events {
            let mut event: LegacyOrchestraEvent =
                serde_json::from_slice(&envelope).map_err(|_| invalid_persisted_state())?;
            event.event_id = event_id;
            events.push(event);
            if events.len() > MAX_WEB_ORCHESTRA_EVENTS {
                return Err(invalid_persisted_state());
            }
        }
        let Some(next_offset) = page.next_offset else {
            break;
        };
        if next_offset <= offset {
            return Err(invalid_persisted_state());
        }
        offset = next_offset;
    }
    if events.is_empty() {
        return Err(invalid_persisted_state());
    }
    Ok(json!({
        "runtimeId": runtime_id.as_str(),
        "runId": run_id,
        "events": events,
    }))
}

pub(crate) fn fleet_runs_value(runtime: &mut ControlRuntime) -> Result<Value, ConsoleWriteError> {
    let runs = load_runs(runtime, None)?;
    let (_, runtimes) = runtime.runtime_event_state();
    let projections = runtimes
        .iter()
        .map(|projection| (projection.id.as_str(), projection))
        .collect::<BTreeMap<_, _>>();
    let mut runtime_ids = BTreeSet::new();
    let mut active_count = 0_usize;
    let mut failed_count = 0_usize;
    let mut degraded_count = 0_usize;
    let mut retryable_count = 0_usize;
    let mut items = Vec::new();
    for run in &runs {
        let projection = projections
            .get(run.runtime_id.as_str())
            .ok_or_else(invalid_persisted_state)?;
        runtime_ids.insert(run.runtime_id.as_str());
        active_count += usize::from(active_outcome(&run.outcome));
        failed_count += usize::from(run.outcome == "failed");
        degraded_count += usize::from(run.outcome == "degraded");
        retryable_count +=
            usize::from(terminal_outcome(&run.outcome) && run.plan_id != "session_preparation");
        if items.len() < MAX_WEB_ORCHESTRA_FLEET_ITEMS {
            items.push(json!({
                "runtimeId": projection.id.as_str(),
                "runtimeName": projection.name,
                "tags": tags_value(projection),
                "run": run,
            }));
        }
    }
    Ok(json!({
        "runtimeCount": runtime_ids.len(),
        "runCount": runs.len(),
        "activeCount": active_count,
        "failedCount": failed_count,
        "degradedCount": degraded_count,
        "retryableCount": retryable_count,
        "runs": items,
    }))
}

pub(crate) fn execute_plan(
    runtime: &mut ControlRuntime,
    runtime_id: &RuntimeId,
    plan_id: &str,
    body: &[u8],
) -> Result<Value, ConsoleWriteError> {
    let request: ExecuteRequest = decode_request(body)?;
    let command_id = command_id(request.request_id.as_deref())?;
    let expected_plan_revision = required_text(request.expected_revision.as_deref(), 128)?;
    let receipt = orchestra::run_command(
        runtime,
        OrchestraRunCommandRequest {
            principal: web_principal(),
            capabilities: web_capabilities(),
            command_id,
            runtime_id: runtime_id.clone(),
            plan_id: plan_id.to_string(),
            expected_plan_revision: expected_plan_revision.to_string(),
            confirmed: request.confirmed,
            approved_by: request.approved_by,
            approval_note: request.approval_note,
        },
    )
    .map_err(map_authority_error)?;
    Ok(json!({
        "run": receipt.run,
        "replayed": receipt.replayed,
    }))
}

pub(crate) fn cancel_run(
    runtime: &mut ControlRuntime,
    runtime_id: &RuntimeId,
    run_id: &str,
) -> Result<Value, ConsoleWriteError> {
    let receipt = orchestra::cancel_command(
        runtime,
        OrchestraCancelCommandRequest {
            principal: web_principal(),
            capabilities: web_capabilities(),
            command_id: deterministic_cancel_command_id(runtime_id, run_id)?,
            runtime_id: runtime_id.clone(),
            run_id: run_id.to_string(),
            confirmed: true,
        },
    )
    .map_err(map_authority_error)?;
    Ok(json!({
        "run": receipt.run,
        "replayed": receipt.replayed,
    }))
}

pub(crate) fn retry_run(
    runtime: &mut ControlRuntime,
    runtime_id: &RuntimeId,
    run_id: &str,
    body: &[u8],
) -> Result<Value, ConsoleWriteError> {
    let request: RetryRequest = decode_request(body)?;
    let command_id = command_id(request.request_id.as_deref())?;
    let previous = load_runs(runtime, Some(runtime_id.as_str()))?
        .into_iter()
        .find(|run| run.run_id == run_id)
        .ok_or_else(|| {
            ConsoleWriteError::new(
                ConsoleWriteStatus::NotFound,
                "orchestra_run_not_found",
                "Orchestra run was not found",
            )
        })?;
    let catalog = orchestra::plan_catalog(
        runtime,
        &OrchestraPlanCatalogRequest {
            principal: web_principal(),
            capabilities: web_capabilities(),
            runtime_id: runtime_id.clone(),
        },
    )
    .map_err(map_authority_error)?;
    let expected_plan_revision = catalog
        .plans
        .iter()
        .find(|plan| plan.plan_id == previous.plan_id)
        .map(|plan| plan.revision.clone())
        .ok_or_else(|| {
            ConsoleWriteError::new(
                ConsoleWriteStatus::Conflict,
                "orchestra_run_not_retryable",
                "Orchestra run no longer has an executable plan",
            )
        })?;
    let receipt = orchestra::retry_command(
        runtime,
        OrchestraRetryCommandRequest {
            principal: web_principal(),
            capabilities: web_capabilities(),
            command_id,
            runtime_id: runtime_id.clone(),
            run_id: run_id.to_string(),
            expected_plan_revision,
            confirmed: request.confirmed,
            approved_by: request.approved_by,
            approval_note: request.approval_note,
        },
    )
    .map_err(map_authority_error)?;
    Ok(json!({
        "run": receipt.run,
        "replayed": receipt.replayed,
    }))
}

pub(crate) fn create_session_handoff(
    runtime: &mut ControlRuntime,
    runtime_id: &RuntimeId,
    body: &[u8],
) -> Result<Value, ConsoleWriteError> {
    let request: SessionHandoffRequest = decode_request(body)?;
    let pipeline_kind = required_text(Some(&request.pipeline_kind), 128)?;
    let requested_by = required_text(Some(&request.requested_by), 80)?;
    let request_id = match request.request_id.as_deref() {
        Some(value) => command_id(Some(value))?.as_str().to_string(),
        None => random_session_request_id()?,
    };
    let projection = runtime
        .runtime_projection(runtime_id)
        .cloned()
        .ok_or_else(runtime_not_found)?;
    let catalog = orchestra::plan_catalog(
        runtime,
        &OrchestraPlanCatalogRequest {
            principal: web_principal(),
            capabilities: web_capabilities(),
            runtime_id: runtime_id.clone(),
        },
    )
    .map_err(map_authority_error)?;
    let session_plan = catalog
        .plans
        .iter()
        .find(|plan| plan.plan_id == "session_preparation")
        .ok_or_else(invalid_persisted_state)?;
    let session_id = deterministic_identity("session", &request_id, runtime_id);
    let run_id = deterministic_identity("orun-session", &request_id, runtime_id);
    let existing_run = load_runs(runtime, Some(runtime_id.as_str()))?
        .into_iter()
        .find(|run| run.request_id.as_deref() == Some(request_id.as_str()));
    if existing_run.as_ref().is_some_and(|run| {
        run.run_id != run_id
            || run.runtime_id != runtime_id.as_str()
            || run.plan_id != "session_preparation"
            || run.outcome != "ok"
    }) {
        return Err(map_authority_error(
            OrchestraAuthorityError::RequestConflict,
        ));
    }
    if existing_run.is_some()
        && runtime
            .control_session(&session_id)
            .map_err(map_runtime_error)?
            .is_none()
    {
        return Err(invalid_persisted_state());
    }
    let created = runtime
        .create_control_session(
            &request_id,
            &session_id,
            runtime_id,
            pipeline_kind,
            requested_by,
            Vec::new(),
        )
        .map_err(map_runtime_error)?;
    if let Some(existing_run) = existing_run {
        if !created.replayed {
            return Err(invalid_persisted_state());
        }
        return Ok(json!({
            "run": existing_run,
            "session": crate::web_console::control_session_value(&created.session),
            "currentPlan": plan_catalog_value(&projection, &catalog),
            "replayed": true,
        }));
    }
    let recorded_at = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|_| invalid_persisted_state())?;
    let run = LegacyOrchestraRun {
        run_id: run_id.clone(),
        runtime_id: runtime_id.as_str().to_string(),
        plan_id: "session_preparation".into(),
        outcome: "ok".into(),
        executed_at: recorded_at.clone(),
        steps: vec![LegacyOrchestraStep {
            step: "create_session".into(),
            outcome: "ok".into(),
            summary: format!(
                "session {} created for pipeline {}",
                created.session.session_id, created.session.pipeline_kind
            ),
        }],
        completed_at: Some(recorded_at.clone()),
        attempt: 1,
        retried_from_run_id: None,
        approved_by: Some(requested_by.to_string()),
        approval_note: Some("guided session handoff".into()),
        plan_revision: Some(session_plan.revision.clone()),
        request_id: Some(request_id.clone()),
    };
    let event = LegacyOrchestraEvent {
        event_id: 0,
        run_id: run_id.clone(),
        runtime_id: runtime_id.as_str().to_string(),
        event_type: "session_handoff".into(),
        from_outcome: None,
        to_outcome: "ok".into(),
        summary: format!("session {} created", created.session.session_id),
        recorded_at: recorded_at.clone(),
    };
    let run_bytes = serde_json::to_vec(&run).map_err(|_| invalid_persisted_state())?;
    let event_bytes = serde_json::to_vec(&event).map_err(|_| invalid_persisted_state())?;
    runtime
        .persist_orchestra_run_event(
            &run_id,
            runtime_id.as_str(),
            Some(&request_id),
            "session_handoff",
            None,
            "ok",
            "ok",
            &recorded_at,
            &run_bytes,
            &event_bytes,
        )
        .map_err(map_runtime_error)?;
    let current_plan = plan_catalog_value(&projection, &catalog);
    Ok(json!({
        "run": run,
        "session": crate::web_console::control_session_value(&created.session),
        "currentPlan": current_plan,
        "replayed": created.replayed,
    }))
}

fn random_session_request_id() -> Result<String, ConsoleWriteError> {
    let mut bytes = [0_u8; 16];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| invalid_persisted_state())?;
    let mut encoded = String::with_capacity(32);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut encoded, "{byte:02x}");
    }
    Ok(format!("web-session:{encoded}"))
}

fn deterministic_identity(prefix: &str, request_id: &str, runtime_id: &RuntimeId) -> String {
    let input = format!("{prefix}\0{request_id}\0{}", runtime_id.as_str());
    let digest = digest::digest(&digest::SHA256, input.as_bytes());
    let mut encoded = String::with_capacity(32);
    for byte in digest.as_ref().iter().take(16) {
        use std::fmt::Write as _;
        let _ = write!(&mut encoded, "{byte:02x}");
    }
    format!("{prefix}-{encoded}")
}

fn load_runs(
    runtime: &mut ControlRuntime,
    runtime_id: Option<&str>,
) -> Result<Vec<LegacyOrchestraRun>, ConsoleWriteError> {
    let mut offset = 0_u32;
    let mut runs = Vec::new();
    loop {
        let page = runtime
            .load_orchestra_history(runtime_id, None, offset, HISTORY_PAGE_SIZE)
            .map_err(map_runtime_error)?;
        for envelope in page.runs {
            runs.push(serde_json::from_slice(&envelope).map_err(|_| invalid_persisted_state())?);
            if runs.len() > MAX_WEB_ORCHESTRA_RUNS {
                return Err(invalid_persisted_state());
            }
        }
        let Some(next_offset) = page.next_offset else {
            return Ok(runs);
        };
        if next_offset <= offset {
            return Err(invalid_persisted_state());
        }
        offset = next_offset;
    }
}

fn plan_catalog_value(
    projection: &RuntimeProjection,
    catalog: &OrchestraPlanCatalogResponse,
) -> Value {
    json!({
        "runtimeId": projection.id.as_str(),
        "name": catalog.runtime_name,
        "endpoint": projection.endpoint,
        "tags": tags_value(projection),
        "statusSource": catalog.status_source,
        "attentionSeverity": catalog.attention_severity,
        "needsAttention": catalog.needs_attention,
        "attentionReasons": catalog.attention_reasons,
        "plans": catalog
            .plans
            .iter()
            .map(|plan| plan_entry_value(projection.id.as_str(), plan))
            .collect::<Vec<_>>(),
    })
}

fn plan_entry_value(runtime_id: &str, plan: &OrchestraPlan) -> Value {
    json!({
        "planId": plan.plan_id,
        "intent": plan.intent,
        "title": plan.title,
        "summary": plan.summary,
        "riskLevel": plan.risk_level,
        "executionReadiness": plan.execution_readiness,
        "executionMode": plan.execution_mode,
        "reasons": plan.reasons,
        "requiredCapabilities": plan.required_capabilities,
        "steps": plan.steps.iter().map(|step| json!({
            "key": step.key,
            "title": step.title,
            "detail": step.detail,
            "kind": step.kind,
        })).collect::<Vec<_>>(),
        "suggestedSurfaces": suggested_surfaces(runtime_id, &plan.plan_id),
        "approvalMode": plan.approval_mode,
        "revision": plan.revision,
    })
}

fn suggested_surfaces(runtime_id: &str, plan_id: &str) -> Vec<Value> {
    let runtime_id = encode_query_value(runtime_id);
    let runtime_detail = format!("/?tab=runtimes&runtimeId={runtime_id}");
    let child_panel = format!("{runtime_detail}&runtimeMainTab=panel");
    let detail_panel = format!("{runtime_detail}&runtimeMainTab=detail");
    match plan_id {
        "runtime_triage" => vec![
            json!({ "label": "Runtime detail", "path": runtime_detail }),
            json!({ "label": "Runtime attention", "path": detail_panel }),
        ],
        "analysis_recovery" => vec![
            json!({ "label": "Runtimes workspace", "path": runtime_detail }),
            json!({ "label": "Child panel", "path": child_panel }),
        ],
        "sidecar_coordination" => vec![
            json!({ "label": "Runtime detail", "path": detail_panel }),
            json!({ "label": "Child panel", "path": child_panel }),
        ],
        "session_preparation" => vec![
            json!({ "label": "Sessions", "path": "/?tab=sessions" }),
            json!({ "label": "Runtime detail", "path": runtime_detail }),
        ],
        _ => Vec::new(),
    }
}

fn tags_value(runtime: &RuntimeProjection) -> Value {
    json!({
        "environment": runtime.tags.environment,
        "cluster": runtime.tags.cluster,
        "role": runtime.tags.role,
    })
}

fn decode_request<T: DeserializeOwned>(body: &[u8]) -> Result<T, ConsoleWriteError> {
    if body.is_empty() || body.len() > MAX_ORCHESTRA_COMMAND_BYTES {
        return Err(invalid_request());
    }
    serde_json::from_slice(body).map_err(|_| invalid_request())
}

fn command_id(value: Option<&str>) -> Result<CommandId, ConsoleWriteError> {
    let value = required_text(value, MAX_WEB_COMMAND_ID_BYTES)?;
    if value.len() < 8 {
        return Err(invalid_request());
    }
    CommandId::new(value.to_string()).map_err(|_| invalid_request())
}

fn required_text(value: Option<&str>, maximum: usize) -> Result<&str, ConsoleWriteError> {
    let Some(value) = value else {
        return Err(invalid_request());
    };
    if value.is_empty()
        || value.len() > maximum
        || value != value.trim()
        || value.chars().any(char::is_control)
    {
        return Err(invalid_request());
    }
    Ok(value)
}

fn deterministic_cancel_command_id(
    runtime_id: &RuntimeId,
    run_id: &str,
) -> Result<CommandId, ConsoleWriteError> {
    let input = format!("{}\0{run_id}", runtime_id.as_str());
    let digest = digest::digest(&digest::SHA256, input.as_bytes());
    let mut encoded = String::with_capacity(32);
    for byte in digest.as_ref().iter().take(16) {
        use std::fmt::Write as _;
        let _ = write!(&mut encoded, "{byte:02x}");
    }
    CommandId::new(format!("web-cancel-{encoded}")).map_err(|_| invalid_request())
}

fn web_principal() -> Principal {
    Principal {
        id: WEB_ORCHESTRA_PRINCIPAL.into(),
    }
}

fn web_capabilities() -> CapabilitySet {
    CapabilitySet::new([CAPABILITY_ORCHESTRA_WRITE])
}

fn encode_query_value(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(char::from(byte));
        } else {
            use std::fmt::Write as _;
            let _ = write!(&mut encoded, "%{byte:02X}");
        }
    }
    encoded
}

fn active_outcome(outcome: &str) -> bool {
    matches!(outcome, "queued" | "running")
}

fn terminal_outcome(outcome: &str) -> bool {
    matches!(
        outcome,
        "succeeded" | "degraded" | "failed" | "cancelled" | "ok"
    )
}

fn map_authority_error(error: OrchestraAuthorityError) -> ConsoleWriteError {
    let status = match error {
        OrchestraAuthorityError::RuntimeNotFound
        | OrchestraAuthorityError::PlanNotFound
        | OrchestraAuthorityError::RunNotFound => ConsoleWriteStatus::NotFound,
        OrchestraAuthorityError::InvalidApproval | OrchestraAuthorityError::InvalidCommand => {
            ConsoleWriteStatus::BadRequest
        }
        OrchestraAuthorityError::PlanNotExecutable
        | OrchestraAuthorityError::PlanRevisionChanged
        | OrchestraAuthorityError::ConfirmationRequired
        | OrchestraAuthorityError::RequestConflict
        | OrchestraAuthorityError::RuntimeBusy
        | OrchestraAuthorityError::RunNotTerminal
        | OrchestraAuthorityError::RunAlreadyTerminal
        | OrchestraAuthorityError::RunNotCancelable => ConsoleWriteStatus::Conflict,
        OrchestraAuthorityError::Unauthorized
        | OrchestraAuthorityError::PersistenceFailed
        | OrchestraAuthorityError::InvalidPersistedState => ConsoleWriteStatus::ServiceUnavailable,
    };
    ConsoleWriteError::new(status, error.code(), error.message())
}

fn map_runtime_error(error: RuntimeError) -> ConsoleWriteError {
    match error {
        RuntimeError::Domain(DomainError::IdempotencyConflict { .. }) => {
            map_authority_error(OrchestraAuthorityError::RequestConflict)
        }
        RuntimeError::Storage(_) => ConsoleWriteError::new(
            ConsoleWriteStatus::ServiceUnavailable,
            "orchestra_persistence_unavailable",
            "Orchestra persistence authority is unavailable",
        ),
        _ => invalid_persisted_state(),
    }
}

fn runtime_not_found() -> ConsoleWriteError {
    ConsoleWriteError::new(
        ConsoleWriteStatus::NotFound,
        "runtime_not_found",
        "runtime was not found",
    )
}

fn invalid_request() -> ConsoleWriteError {
    ConsoleWriteError::new(
        ConsoleWriteStatus::BadRequest,
        "invalid_orchestra_request",
        "Orchestra request failed strict validation",
    )
}

fn invalid_persisted_state() -> ConsoleWriteError {
    ConsoleWriteError::new(
        ConsoleWriteStatus::ServiceUnavailable,
        "orchestra_state_invalid",
        "persisted Orchestra state failed validation",
    )
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    fn temp_database(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "leserpent-web-orchestra-{label}-{}-{nonce}.sqlite",
            std::process::id()
        ))
    }

    #[test]
    fn compatibility_projection_and_commands_share_rust_authority() {
        let path = temp_database("lifecycle");
        let runtime_id = RuntimeId::new("runtime-web-orchestra").unwrap();
        let mut runtime = ControlRuntime::open(&path).unwrap();
        runtime
            .register_runtime(
                runtime_id.clone(),
                "Web Orchestra runtime",
                "https://runtime.invalid",
            )
            .unwrap();

        let plan = plan_value(&runtime, &runtime_id).unwrap();
        assert_eq!(plan["runtimeId"], runtime_id.as_str());
        assert_eq!(plan["name"], "Web Orchestra runtime");
        let triage = plan["plans"]
            .as_array()
            .unwrap()
            .iter()
            .find(|plan| plan["planId"] == "runtime_triage")
            .unwrap();
        assert_eq!(triage["executionMode"], "automatic");
        assert!(!triage["suggestedSurfaces"].as_array().unwrap().is_empty());
        let revision = triage["revision"].as_str().unwrap();
        let body = serde_json::to_vec(&json!({
            "confirmed": true,
            "expectedRevision": revision,
            "approvedBy": "automatic",
            "approvalNote": null,
            "requestId": "request-run-0001",
        }))
        .unwrap();
        let started = execute_plan(&mut runtime, &runtime_id, "runtime_triage", &body).unwrap();
        assert_eq!(started["replayed"], false);
        assert_eq!(started["run"]["outcome"], "queued");
        let run_id = started["run"]["runId"].as_str().unwrap().to_string();
        let replay = execute_plan(&mut runtime, &runtime_id, "runtime_triage", &body).unwrap();
        assert_eq!(replay["replayed"], true);
        assert_eq!(replay["run"]["runId"], run_id);

        let history = runtime_runs_value(&mut runtime, &runtime_id).unwrap();
        assert_eq!(history["runs"].as_array().unwrap().len(), 1);
        let events = run_events_value(&mut runtime, &runtime_id, &run_id).unwrap();
        assert_eq!(events["events"][0]["eventType"], "run_queued");
        let fleet = fleet_runs_value(&mut runtime).unwrap();
        assert_eq!(fleet["runCount"], 1);
        assert_eq!(fleet["activeCount"], 1);

        let cancelled = cancel_run(&mut runtime, &runtime_id, &run_id).unwrap();
        assert_eq!(cancelled["run"]["outcome"], "cancelled");
        let retry = retry_run(
            &mut runtime,
            &runtime_id,
            &run_id,
            br#"{"confirmed":true,"approvedBy":"automatic","approvalNote":null,"requestId":"request-retry-0001"}"#,
        )
        .unwrap();
        assert_eq!(retry["run"]["retriedFromRunId"], run_id);
        assert_eq!(retry["run"]["attempt"], 2);
        let fleet = fleet_runs_value(&mut runtime).unwrap();
        assert_eq!(fleet["runCount"], 2);
        assert_eq!(fleet["activeCount"], 1);
        assert_eq!(fleet["retryableCount"], 1);

        drop(runtime);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn compatibility_requests_are_strict_and_session_handoff_is_durable() {
        let path = temp_database("strict");
        let runtime_id = RuntimeId::new("runtime-web-orchestra-strict").unwrap();
        let mut runtime = ControlRuntime::open(&path).unwrap();
        runtime
            .register_runtime(
                runtime_id.clone(),
                "Strict runtime",
                "https://runtime.invalid",
            )
            .unwrap();
        let error = execute_plan(
            &mut runtime,
            &runtime_id,
            "runtime_triage",
            br#"{"confirmed":true,"expectedRevision":"x","requestId":"request-run-0002","unknown":true}"#,
        )
        .unwrap_err();
        assert_eq!(error.status, ConsoleWriteStatus::BadRequest);
        assert_eq!(error.code, "invalid_orchestra_request");

        let plan = plan_value(&runtime, &runtime_id).unwrap();
        let revision = plan["plans"]
            .as_array()
            .unwrap()
            .iter()
            .find(|plan| plan["planId"] == "runtime_triage")
            .unwrap()["revision"]
            .as_str()
            .unwrap();
        let conflicting_request = serde_json::to_vec(&json!({
            "confirmed": true,
            "expectedRevision": revision,
            "approvedBy": "automatic",
            "approvalNote": null,
            "requestId": "request-session-conflict-0001",
        }))
        .unwrap();
        execute_plan(
            &mut runtime,
            &runtime_id,
            "runtime_triage",
            &conflicting_request,
        )
        .unwrap();
        let conflict = create_session_handoff(
            &mut runtime,
            &runtime_id,
            br#"{"pipelineKind":"diagnostic","requestedBy":"operator","requestId":"request-session-conflict-0001"}"#,
        )
        .unwrap_err();
        assert_eq!(conflict.status, ConsoleWriteStatus::Conflict);
        assert_eq!(conflict.code, "orchestra_request_conflict");
        assert!(runtime.list_control_sessions().unwrap().is_empty());

        let session_body = br#"{"pipelineKind":"diagnostic","requestedBy":"operator","requestId":"request-session-0001"}"#;
        let created = create_session_handoff(&mut runtime, &runtime_id, session_body).unwrap();
        assert_eq!(created["session"]["status"], "running");
        assert_eq!(created["run"]["planId"], "session_preparation");
        assert_eq!(created["replayed"], false);
        let replay = create_session_handoff(&mut runtime, &runtime_id, session_body).unwrap();
        assert_eq!(
            replay["session"]["sessionId"],
            created["session"]["sessionId"]
        );
        assert_eq!(replay["run"]["runId"], created["run"]["runId"]);
        assert_eq!(replay["replayed"], true);

        let missing = RuntimeId::new("runtime-missing").unwrap();
        assert_eq!(
            plan_value(&runtime, &missing).unwrap_err().status,
            ConsoleWriteStatus::NotFound
        );
        let missing_run = run_events_value(&mut runtime, &runtime_id, "orun-missing").unwrap_err();
        assert_eq!(missing_run.status, ConsoleWriteStatus::NotFound);
        assert_eq!(missing_run.code, "orchestra_run_not_found");
        drop(runtime);
        fs::remove_file(path).unwrap();
    }
}
