use std::collections::{BTreeMap, HashSet};

use leserpent_adapters::{GewyvernTarget, MAX_SECRET_BYTES, SecretValue};
use leserpent_domain::{
    CAPABILITY_RUNTIME_REFRESH, COMMAND_PLAN_SCHEMA_VERSION, CapabilitySet, Command,
    CommandEnvelope, CommandId, CommandOrigin, CommandPlan, Confirmation,
    DOMAIN_SNAPSHOT_SCHEMA_VERSION, DomainError, DomainSnapshot, IdempotencyKey, PlannedOperation,
    Principal, RefreshStatus, Revision, RuntimeCapabilitySnapshot, RuntimeId, RuntimeListFilter,
    RuntimeProjection, RuntimeSidecarMemorySlotSnapshot, RuntimeSidecarMemorySnapshot,
    RuntimeSidecarStatusSnapshot, RuntimeStatusSnapshot, RuntimeTags,
    canonical_runtime_endpoint_identity, normalize_runtime_http_endpoint,
    validate_registration_intent,
};
use leserpent_protocol::{AuthorityWriterFence, MAX_PROTOCOL_MESSAGE_BYTES};
use leserpent_runtime::{
    AuthorityWriterFenceError, ControlRuntime, ControlSession, OrchestraImportRecord, PlanResult,
    RuntimeError, RuntimeUnregisterTarget, SessionCapabilityRequirement, SessionImportRecord,
};
use ring::digest;
use ring::rand::{SecureRandom, SystemRandom};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use time::{Duration as TimeDuration, OffsetDateTime, format_description::well_known::Rfc3339};

use crate::runtime_target_registration::{
    RuntimeTargetDescriptor, RuntimeTargetRegistrationAction, RuntimeTargetRegistrationAuthority,
    RuntimeTargetRegistrationError, RuntimeTargetRegistrationErrorKind,
    RuntimeTargetRegistrationIntent, loopback_address,
};
use crate::web_console::{
    CleanupKind, ConsoleApiRoute, MAX_ATOMIC_CLEANUP_TARGETS, MAX_CLEANUP_REQUEST_BYTES,
    MAX_ORCHESTRA_COMMAND_BYTES, MAX_REGISTRATION_PLAN_BYTES, MAX_REGISTRATION_REQUEST_BYTES,
    PERSISTENCE_EXPORT_SCHEMA_VERSION, build_cleanup_plan_with_sessions, control_session_value,
    runtime_value, sha256_hex,
};
pub(crate) use crate::web_console_error::{ConsoleWriteError, ConsoleWriteStatus};
use crate::web_console_orchestra;

const REGISTRATION_TRANSACTION_REASON: &str = "rust_web_registration_transaction_unavailable";
const REGISTRATION_LOOPBACK_REASON: &str = "loopback_gewyvern_target_required";
const REGISTRATION_CA_REASON: &str = "explicit_gewyvern_ca_required";
const REGISTRATION_CA_MISMATCH_REASON: &str = "gewyvern_ca_not_applicable";
const REGISTRATION_HTTPS_ORIGIN_REASON: &str = "gewyvern_https_origin_required";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RefreshKind {
    All,
    Capabilities,
    Status,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RegistrationPlanRequest {
    name: String,
    endpoint: String,
    #[serde(default)]
    sidecar_endpoint: Option<String>,
    #[serde(default)]
    tls_ca_sha256: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RegistrationTagsRequest {
    #[serde(default)]
    environment: Option<String>,
    #[serde(default)]
    cluster: Option<String>,
    #[serde(default)]
    role: Option<String>,
}

impl From<RegistrationTagsRequest> for RuntimeTags {
    fn from(value: RegistrationTagsRequest) -> Self {
        Self {
            environment: value.environment,
            cluster: value.cluster,
            role: value.role,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RegistrationRequest {
    name: String,
    endpoint: String,
    pairing_token: String,
    #[serde(default)]
    capabilities: Vec<Value>,
    #[serde(default)]
    tags: RegistrationTagsRequest,
    #[serde(default)]
    fetch_capabilities: bool,
    #[serde(default)]
    capability_endpoint: Option<String>,
    #[serde(default)]
    status_endpoint: Option<String>,
    #[serde(default)]
    sidecar_endpoint: Option<String>,
    #[serde(default)]
    sidecar_status_endpoint: Option<String>,
    #[serde(default)]
    sidecar_admin_token: Option<String>,
    #[serde(default)]
    tls_ca_pem: Option<String>,
    #[serde(default)]
    tls_ca_sha256: Option<String>,
    registration_plan_token: String,
}

#[derive(Clone, Debug)]
struct RegistrationCoordinates {
    name: String,
    endpoint: String,
    sidecar_endpoint: Option<String>,
    tls_ca_sha256: Option<String>,
}

#[derive(Clone, Debug)]
struct RegistrationDecision {
    action: Option<RuntimeTargetRegistrationAction>,
    reason: Option<&'static str>,
    existing: Option<RuntimeProjection>,
    planned_runtime_id: RuntimeId,
    expected_revision: Option<Revision>,
    plan_token: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CleanupRequest {
    plan_token: String,
    #[serde(default)]
    challenge: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SessionCreateRequest {
    runtime_id: String,
    pipeline_kind: String,
    requested_by: String,
    #[serde(default)]
    requirements: Vec<SessionCapabilityRequirement>,
    #[serde(default)]
    request_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SessionStopRequest {
    requested_by: String,
    reason: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PersistenceImportRequest {
    schema_version: u32,
    saved_at: String,
    runtimes: Vec<PersistenceRuntime>,
    sessions: Vec<PersistenceSession>,
    #[serde(default)]
    orchestra_runs: Option<Vec<PersistenceOrchestraRun>>,
    #[serde(default)]
    pending_runtime_deletions: Option<Vec<Value>>,
    #[serde(default)]
    runtime_deletion_retry_audit: Option<Vec<Value>>,
    #[serde(default)]
    runtime_deletion_reconciliation_audit: Option<Vec<Value>>,
    #[serde(default)]
    orchestra_delete_checkpoint_monitor: Option<Value>,
    #[serde(default)]
    orchestra_delete_checkpoint_alert_outbox: Option<Vec<Value>>,
    #[serde(default)]
    pending_runtime_registrations: Option<Vec<Value>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PersistenceSession {
    session_id: String,
    runtime_id: String,
    pipeline_kind: String,
    requested_by: String,
    status: String,
    created_at: String,
    updated_at: String,
    requirements: Vec<SessionCapabilityRequirement>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PersistenceRuntime {
    runtime_id: String,
    name: String,
    endpoint: String,
    sidecar_endpoint: Option<String>,
    registered_at: String,
    updated_at: String,
    capabilities: Vec<PersistenceCapability>,
    capability_source: String,
    capability_fetched_at: Option<String>,
    capability_fetch_error: Option<String>,
    tags: PersistenceTags,
    status: PersistenceRuntimeStatus,
    sidecar_status: Option<PersistenceSidecarStatus>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PersistenceCapability {
    key: String,
    support: String,
    description: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PersistenceTags {
    environment: Option<String>,
    cluster: Option<String>,
    role: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PersistenceRuntimeStatus {
    status_source: String,
    status_fetched_at: Option<String>,
    status_fetch_error: Option<String>,
    has_latest_snapshot: bool,
    snapshot_kind: Option<String>,
    target_count: Option<u64>,
    has_summary_json: bool,
    has_analysis_json: bool,
    has_training_example_json: bool,
    has_training_dataset_manifest: bool,
    has_export_json: bool,
    has_report_json: bool,
    has_report_html: bool,
    has_external_sidecar_context: bool,
    has_external_evidence_chain_enrichment: bool,
    has_external_diagnostic_opinion: bool,
    #[serde(default)]
    resilience_degraded: bool,
    #[serde(default)]
    resilience_status: Option<String>,
    #[serde(default)]
    resilience_summary: Option<String>,
    #[serde(default)]
    socket_service_status: Option<String>,
    #[serde(default)]
    socket_consecutive_idle_timeouts: Option<u64>,
    #[serde(default)]
    socket_total_idle_timeouts: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PersistenceSidecarStatus {
    status_source: String,
    status_fetched_at: Option<String>,
    status_fetch_error: Option<String>,
    healthy: bool,
    daemon_status: String,
    target_count: Option<u64>,
    learning_active: bool,
    learned_routes: u64,
    has_evidence_chain_enrichment: bool,
    has_diagnostic_opinion: bool,
    #[serde(default)]
    last_error: Option<String>,
    #[serde(default)]
    memory: Option<PersistenceSidecarMemory>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PersistenceSidecarMemory {
    versions_supported: bool,
    slot_count: u64,
    history_count: u64,
    latest_slot: Option<String>,
    latest_label: Option<String>,
    latest_source: Option<String>,
    slots: Vec<PersistenceSidecarMemorySlot>,
    #[serde(default)]
    fetch_error: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PersistenceSidecarMemorySlot {
    slot: String,
    label: Option<String>,
    note: Option<String>,
    source: String,
    saved_at: Option<String>,
    pattern_count: u64,
    label_count: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PersistenceOrchestraRun {
    run_id: String,
    runtime_id: String,
    plan_id: String,
    outcome: String,
    executed_at: String,
    steps: Vec<PersistenceOrchestraStep>,
    #[serde(default)]
    completed_at: Option<String>,
    #[serde(default = "default_orchestra_attempt")]
    attempt: u32,
    #[serde(default)]
    retried_from_run_id: Option<String>,
    #[serde(default)]
    approved_by: Option<String>,
    #[serde(default)]
    approval_note: Option<String>,
    #[serde(default)]
    plan_revision: Option<String>,
    #[serde(default)]
    request_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PersistenceOrchestraStep {
    step: String,
    outcome: String,
    summary: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PersistenceOrchestraEvent<'a> {
    event_id: u64,
    run_id: &'a str,
    runtime_id: &'a str,
    event_type: &'static str,
    from_outcome: Option<&'a str>,
    to_outcome: &'a str,
    summary: &'static str,
    recorded_at: &'a str,
}

const fn default_orchestra_attempt() -> u32 {
    1
}

#[cfg(test)]
pub(crate) fn execute(
    route: &ConsoleApiRoute,
    body: &[u8],
    runtime: &mut ControlRuntime,
    writer_fence: Option<&AuthorityWriterFence>,
) -> Result<Vec<u8>, ConsoleWriteError> {
    execute_with_registration(route, body, runtime, writer_fence, None)
}

pub(crate) fn execute_with_registration(
    route: &ConsoleApiRoute,
    body: &[u8],
    runtime: &mut ControlRuntime,
    writer_fence: Option<&AuthorityWriterFence>,
    registration: Option<&RuntimeTargetRegistrationAuthority>,
) -> Result<Vec<u8>, ConsoleWriteError> {
    let value = match route {
        ConsoleApiRoute::RegistrationPlan => {
            registration_plan(body, runtime, registration.is_some())?
        }
        ConsoleApiRoute::Registration => register_runtime(
            body,
            runtime,
            writer_fence,
            registration.ok_or_else(registration_unavailable)?,
        )?,
        ConsoleApiRoute::FleetRefreshAll(filter) => {
            refresh_fleet(runtime, writer_fence, filter, RefreshKind::All)?
        }
        ConsoleApiRoute::FleetRefreshCapabilities(filter) => {
            refresh_fleet(runtime, writer_fence, filter, RefreshKind::Capabilities)?
        }
        ConsoleApiRoute::FleetRefreshStatus(filter) => {
            refresh_fleet(runtime, writer_fence, filter, RefreshKind::Status)?
        }
        ConsoleApiRoute::RuntimeCleanup(kind, filter) => {
            cleanup_runtimes(runtime, writer_fence, registration, *kind, filter, body)?
        }
        ConsoleApiRoute::RuntimeDelete(runtime_id) => {
            delete_runtime(runtime, writer_fence, registration, runtime_id)?
        }
        ConsoleApiRoute::PersistenceImport => {
            import_persistence(body, runtime, writer_fence, registration)?
        }
        ConsoleApiRoute::PersistenceSave => save_persistence(runtime, writer_fence)?,
        ConsoleApiRoute::SessionCreate => create_control_session(body, runtime, writer_fence)?,
        ConsoleApiRoute::SessionStop(session_id) => {
            stop_control_session(session_id, body, runtime, writer_fence)?
        }
        ConsoleApiRoute::OrchestraExecute {
            runtime_id,
            plan_id,
        } => {
            require_writer(runtime, writer_fence)?;
            web_console_orchestra::execute_plan(runtime, runtime_id, plan_id, body)?
        }
        ConsoleApiRoute::OrchestraCancel { runtime_id, run_id } => {
            require_writer(runtime, writer_fence)?;
            web_console_orchestra::cancel_run(runtime, runtime_id, run_id)?
        }
        ConsoleApiRoute::OrchestraRetry { runtime_id, run_id } => {
            require_writer(runtime, writer_fence)?;
            web_console_orchestra::retry_run(runtime, runtime_id, run_id, body)?
        }
        ConsoleApiRoute::OrchestraSession(runtime_id) => {
            require_writer(runtime, writer_fence)?;
            web_console_orchestra::create_session_handoff(runtime, runtime_id, body)?
        }
        _ => {
            return Err(ConsoleWriteError {
                status: ConsoleWriteStatus::InternalServerError,
                code: "web_route_confused",
                reason: "Rust Web request reached an incompatible route executor",
            });
        }
    };
    let body = serde_json::to_vec(&value).map_err(|_| ConsoleWriteError {
        status: ConsoleWriteStatus::InternalServerError,
        code: "web_response_failed",
        reason: "Rust Web response serialization failed",
    })?;
    if body.len() > MAX_PROTOCOL_MESSAGE_BYTES {
        return Err(ConsoleWriteError {
            status: ConsoleWriteStatus::InternalServerError,
            code: "web_response_too_large",
            reason: "Rust Web response exceeded the protocol limit",
        });
    }
    Ok(body)
}

fn save_persistence(
    runtime: &mut ControlRuntime,
    writer_fence: Option<&AuthorityWriterFence>,
) -> Result<Value, ConsoleWriteError> {
    require_writer(runtime, writer_fence)?;
    let through_sequence = runtime.create_snapshot().map_err(map_runtime_error)?;
    let saved_at = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|_| ConsoleWriteError {
            status: ConsoleWriteStatus::InternalServerError,
            code: "persistence_save_response_failed",
            reason: "Rust Web persistence timestamp formatting failed",
        })?;
    Ok(json!({
        "ok": true,
        "savedAt": saved_at,
        "throughSequence": through_sequence,
    }))
}

fn create_control_session(
    body: &[u8],
    runtime: &mut ControlRuntime,
    writer_fence: Option<&AuthorityWriterFence>,
) -> Result<Value, ConsoleWriteError> {
    require_writer(runtime, writer_fence)?;
    if body.is_empty() || body.len() > MAX_ORCHESTRA_COMMAND_BYTES {
        return Err(invalid_session_request());
    }
    let mut request: SessionCreateRequest =
        serde_json::from_slice(body).map_err(|_| invalid_session_request())?;
    let runtime_id =
        RuntimeId::new(request.runtime_id.clone()).map_err(|_| invalid_session_request())?;
    if !valid_import_text(&request.pipeline_kind, 128, false)
        || !valid_import_text(&request.requested_by, 256, false)
        || request.requirements.len() > leserpent_runtime::MAX_SESSION_REQUIREMENTS
    {
        return Err(invalid_session_request());
    }
    let mut requirement_keys = HashSet::with_capacity(request.requirements.len());
    request.requirements.sort_by(|left, right| {
        left.key
            .to_ascii_lowercase()
            .cmp(&right.key.to_ascii_lowercase())
    });
    for requirement in &request.requirements {
        if !valid_import_text(&requirement.key, 128, false)
            || !requirement_keys.insert(requirement.key.to_ascii_lowercase())
            || !matches!(
                requirement.minimum_support.as_str(),
                "fully_supported" | "risky" | "not_supported"
            )
        {
            return Err(invalid_session_request());
        }
    }
    let projection = runtime
        .runtime_projection(&runtime_id)
        .cloned()
        .ok_or_else(|| {
            map_domain_error(DomainError::RuntimeNotFound {
                runtime_id: runtime_id.as_str().to_string(),
            })
        })?;
    if request
        .requirements
        .iter()
        .any(|requirement| !runtime_supports_requirement(&projection, &requirement.key))
    {
        return Err(ConsoleWriteError::new(
            ConsoleWriteStatus::BadRequest,
            "capability_requirements_not_satisfied",
            "runtime does not satisfy the requested session capabilities",
        ));
    }
    let request_id = match request.request_id.as_deref() {
        Some(value) => validate_session_request_id(value)?.to_string(),
        None => new_session_request_id()?,
    };
    let session_id = deterministic_session_id(&request_id, &runtime_id);
    let created = runtime
        .create_control_session(
            &request_id,
            &session_id,
            &runtime_id,
            &request.pipeline_kind,
            &request.requested_by,
            request.requirements,
        )
        .map_err(map_runtime_error)?;
    Ok(control_session_value(&created.session))
}

fn stop_control_session(
    session_id: &str,
    body: &[u8],
    runtime: &mut ControlRuntime,
    writer_fence: Option<&AuthorityWriterFence>,
) -> Result<Value, ConsoleWriteError> {
    require_writer(runtime, writer_fence)?;
    if body.is_empty() || body.len() > MAX_ORCHESTRA_COMMAND_BYTES {
        return Err(invalid_session_request());
    }
    let request: SessionStopRequest =
        serde_json::from_slice(body).map_err(|_| invalid_session_request())?;
    if !valid_import_text(&request.requested_by, 256, false)
        || !valid_import_text(&request.reason, 1_024, true)
    {
        return Err(invalid_session_request());
    }
    let stopped = runtime
        .stop_control_session(session_id)
        .map_err(map_runtime_error)?
        .ok_or_else(|| {
            ConsoleWriteError::new(
                ConsoleWriteStatus::NotFound,
                "session_not_found",
                "control session was not found",
            )
        })?;
    Ok(control_session_value(&stopped))
}

fn invalid_session_request() -> ConsoleWriteError {
    ConsoleWriteError::new(
        ConsoleWriteStatus::BadRequest,
        "invalid_session_request",
        "control session request failed validation",
    )
}

fn validate_session_request_id(value: &str) -> Result<&str, ConsoleWriteError> {
    if value.len() < 8
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':' | b'.'))
    {
        return Err(invalid_session_request());
    }
    Ok(value)
}

fn new_session_request_id() -> Result<String, ConsoleWriteError> {
    let mut bytes = [0_u8; 16];
    SystemRandom::new().fill(&mut bytes).map_err(|_| {
        ConsoleWriteError::new(
            ConsoleWriteStatus::InternalServerError,
            "session_identity_unavailable",
            "control session identity generation failed",
        )
    })?;
    Ok(format!("web-session:{}", hex_prefix(&bytes)))
}

fn deterministic_session_id(request_id: &str, runtime_id: &RuntimeId) -> String {
    let input = format!(
        "leserpent-session-v1\0{request_id}\0{}",
        runtime_id.as_str()
    );
    let digest = digest::digest(&digest::SHA256, input.as_bytes());
    format!("session-{}", hex_prefix(&digest.as_ref()[..16]))
}

fn hex_prefix(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        encoded.push(char::from(HEX[(byte >> 4) as usize]));
        encoded.push(char::from(HEX[(byte & 0x0f) as usize]));
    }
    encoded
}

fn runtime_supports_requirement(runtime: &RuntimeProjection, key: &str) -> bool {
    match key {
        "latest_snapshot" => runtime.capabilities.latest_snapshot,
        "authenticated_deployment" => runtime.capabilities.authenticated_deployment,
        "serve_required" => runtime.capabilities.serve_required,
        "external_sidecar_context" => runtime.capabilities.external_sidecar_context,
        key => runtime
            .capabilities
            .extensions
            .get(key)
            .copied()
            .unwrap_or(false),
    }
}

fn import_persistence(
    body: &[u8],
    runtime: &mut ControlRuntime,
    writer_fence: Option<&AuthorityWriterFence>,
    registration: Option<&RuntimeTargetRegistrationAuthority>,
) -> Result<Value, ConsoleWriteError> {
    require_writer(runtime, writer_fence)?;
    if body.is_empty() || body.len() > MAX_PROTOCOL_MESSAGE_BYTES {
        return Err(invalid_persistence_import());
    }
    let request: PersistenceImportRequest =
        serde_json::from_slice(body).map_err(|_| invalid_persistence_import())?;
    let imported_from_saved_at = validate_import_timestamp(&request.saved_at)?;
    if request.schema_version != PERSISTENCE_EXPORT_SCHEMA_VERSION
        || request.sessions.len() > leserpent_runtime::MAX_CONTROL_SESSIONS
        || request
            .orchestra_runs
            .as_ref()
            .is_some_and(|runs| runs.len() > 4_096)
        || import_has_items(&request.pending_runtime_deletions)
        || import_has_items(&request.runtime_deletion_retry_audit)
        || import_has_items(&request.runtime_deletion_reconciliation_audit)
        || request.orchestra_delete_checkpoint_monitor.is_some()
        || import_has_items(&request.orchestra_delete_checkpoint_alert_outbox)
        || import_has_items(&request.pending_runtime_registrations)
    {
        return Err(invalid_persistence_import());
    }
    let current_revision = runtime.runtime_event_state().0;
    let (snapshot, orchestra_runs, sessions) = build_import_snapshot(&request, current_revision)?;
    let protected_binding_runtime_ids = match registration {
        Some(registration) => registration
            .validate_import_bindings(runtime, &snapshot.runtimes)
            .map_err(map_registration_error)?,
        None => {
            let bindings = runtime
                .runtime_target_bindings()
                .map_err(map_runtime_error)?;
            if !bindings.is_empty() {
                return Err(ConsoleWriteError {
                    status: ConsoleWriteStatus::ServiceUnavailable,
                    code: "persistence_import_binding_authority_unavailable",
                    reason: "credential-bound runtime targets require registration authority during import",
                });
            }
            Vec::new()
        }
    };
    let imported_runtime_count = snapshot.runtimes.len();
    let imported_session_count = sessions.len();
    let imported = runtime
        .import_control_plane_state(
            snapshot,
            &orchestra_runs,
            &sessions,
            &protected_binding_runtime_ids,
        )
        .map_err(map_persistence_import_runtime_error)?;
    let saved_at = OffsetDateTime::from_unix_timestamp_nanos(
        i128::from(imported.saved_at_unix_ms) * 1_000_000,
    )
    .map_err(|_| ConsoleWriteError {
        status: ConsoleWriteStatus::InternalServerError,
        code: "persistence_import_response_failed",
        reason: "Rust Web persistence import timestamp formatting failed",
    })?
    .format(&Rfc3339)
    .map_err(|_| ConsoleWriteError {
        status: ConsoleWriteStatus::InternalServerError,
        code: "persistence_import_response_failed",
        reason: "Rust Web persistence import timestamp formatting failed",
    })?;
    Ok(json!({
        "ok": true,
        "importedRuntimeCount": imported_runtime_count,
        "importedSessionCount": imported_session_count,
        "savedAt": saved_at,
        "importedFromSavedAt": imported_from_saved_at
            .format(&Rfc3339)
            .map_err(|_| invalid_persistence_import())?,
        "throughSequence": imported.through_sequence,
    }))
}

fn build_import_snapshot(
    request: &PersistenceImportRequest,
    current_revision: Revision,
) -> Result<
    (
        DomainSnapshot,
        Vec<OrchestraImportRecord>,
        Vec<SessionImportRecord>,
    ),
    ConsoleWriteError,
> {
    if request.runtimes.len() > 4_096 {
        return Err(invalid_persistence_import());
    }
    let mut runtime_ids = HashSet::with_capacity(request.runtimes.len());
    let mut folded_runtime_ids = HashSet::with_capacity(request.runtimes.len());
    let mut endpoint_ids = HashSet::with_capacity(request.runtimes.len());
    let mut next_revision = current_revision.0;
    let mut runtimes = Vec::with_capacity(request.runtimes.len());
    for imported in &request.runtimes {
        let runtime_id = RuntimeId::new(imported.runtime_id.clone())
            .map_err(|_| invalid_persistence_import())?;
        let tags = RuntimeTags {
            environment: imported.tags.environment.clone(),
            cluster: imported.tags.cluster.clone(),
            role: imported.tags.role.clone(),
        };
        validate_registration_intent(
            &imported.name,
            &imported.endpoint,
            imported.sidecar_endpoint.as_deref(),
            &tags,
        )
        .map_err(|_| invalid_persistence_import())?;
        if !runtime_ids.insert(imported.runtime_id.clone())
            || !folded_runtime_ids.insert(imported.runtime_id.to_ascii_lowercase())
            || !endpoint_ids.insert(canonical_runtime_endpoint_identity(&imported.endpoint))
        {
            return Err(invalid_persistence_import());
        }
        let registered_at = import_timestamp_unix_ms(&imported.registered_at)?;
        let updated_at = import_timestamp_unix_ms(&imported.updated_at)?;
        if registered_at > updated_at {
            return Err(invalid_persistence_import());
        }
        validate_capability_posture(imported)?;
        let capabilities =
            import_capabilities(&imported.capabilities, &imported.capability_source)?;
        let status = import_runtime_status(&imported.status)?;
        let sidecar_status = imported
            .sidecar_status
            .as_ref()
            .map(import_sidecar_status)
            .transpose()?;
        next_revision = next_revision
            .checked_add(1)
            .ok_or_else(invalid_persistence_import)?;
        let observed = status.status_source != "unobserved" || !capabilities.is_unobserved();
        runtimes.push(RuntimeProjection {
            id: runtime_id,
            name: imported.name.clone(),
            endpoint: imported.endpoint.clone(),
            sidecar_endpoint: imported.sidecar_endpoint.clone(),
            registered_at_unix_ms: Some(registered_at),
            updated_at_unix_ms: Some(updated_at),
            revision: Revision(next_revision),
            refresh_count: 0,
            refresh_status: if observed {
                RefreshStatus::Ready
            } else {
                RefreshStatus::NeverRequested
            },
            tags,
            status,
            sidecar_status,
            capabilities,
            capabilities_observed_for_revision: None,
        });
    }
    if runtimes.is_empty() {
        next_revision = next_revision
            .checked_add(1)
            .ok_or_else(invalid_persistence_import)?;
    }
    runtimes.sort_by(|left, right| left.id.cmp(&right.id));
    let orchestra_runs = import_orchestra_runs(
        request.orchestra_runs.as_deref().unwrap_or_default(),
        &runtime_ids,
    )?;
    let sessions = import_sessions(&request.sessions, &runtime_ids)?;
    let snapshot = DomainSnapshot {
        schema_version: DOMAIN_SNAPSHOT_SCHEMA_VERSION,
        revision: Revision(next_revision),
        runtimes,
        applied_commands: Vec::new(),
    };
    snapshot
        .validate()
        .map_err(|_| invalid_persistence_import())?;
    Ok((snapshot, orchestra_runs, sessions))
}

fn import_sessions(
    imported: &[PersistenceSession],
    runtime_ids: &HashSet<String>,
) -> Result<Vec<SessionImportRecord>, ConsoleWriteError> {
    let mut session_ids = HashSet::with_capacity(imported.len());
    let mut sessions = Vec::with_capacity(imported.len());
    for imported in imported {
        validate_import_identity(&imported.session_id)?;
        validate_import_identity(&imported.runtime_id)?;
        if !runtime_ids.contains(&imported.runtime_id)
            || !session_ids.insert(imported.session_id.to_ascii_lowercase())
            || !valid_import_text(&imported.pipeline_kind, 128, false)
            || !valid_import_text(&imported.requested_by, 256, false)
            || !matches!(imported.status.as_str(), "running" | "stopped")
            || imported.requirements.len() > leserpent_runtime::MAX_SESSION_REQUIREMENTS
        {
            return Err(invalid_persistence_import());
        }
        let mut requirement_keys = HashSet::with_capacity(imported.requirements.len());
        for requirement in &imported.requirements {
            if !valid_import_text(&requirement.key, 128, false)
                || !requirement_keys.insert(requirement.key.to_ascii_lowercase())
                || !matches!(
                    requirement.minimum_support.as_str(),
                    "fully_supported" | "risky" | "not_supported"
                )
            {
                return Err(invalid_persistence_import());
            }
        }
        let created_at_unix_ms = import_timestamp_unix_ms(&imported.created_at)?;
        let updated_at_unix_ms = import_timestamp_unix_ms(&imported.updated_at)?;
        if created_at_unix_ms > updated_at_unix_ms {
            return Err(invalid_persistence_import());
        }
        let runtime_id = RuntimeId::new(imported.runtime_id.clone())
            .map_err(|_| invalid_persistence_import())?;
        sessions.push(SessionImportRecord {
            request_id: format!(
                "import-session-{}",
                &sha256_hex(imported.session_id.as_bytes())[..32]
            ),
            session: ControlSession {
                session_id: imported.session_id.clone(),
                runtime_id,
                pipeline_kind: imported.pipeline_kind.clone(),
                requested_by: imported.requested_by.clone(),
                status: imported.status.clone(),
                created_at_unix_ms,
                updated_at_unix_ms,
                requirements: imported.requirements.clone(),
            },
        });
    }
    sessions.sort_by(|left, right| left.session.session_id.cmp(&right.session.session_id));
    Ok(sessions)
}

fn import_has_items(items: &Option<Vec<Value>>) -> bool {
    items.as_ref().is_some_and(|items| !items.is_empty())
}

fn validate_import_timestamp(value: &str) -> Result<OffsetDateTime, ConsoleWriteError> {
    if !valid_import_text(value, 64, false) {
        return Err(invalid_persistence_import());
    }
    let timestamp =
        OffsetDateTime::parse(value, &Rfc3339).map_err(|_| invalid_persistence_import())?;
    if timestamp.unix_timestamp_nanos() < 0
        || timestamp > OffsetDateTime::now_utc() + TimeDuration::minutes(5)
    {
        return Err(invalid_persistence_import());
    }
    Ok(timestamp)
}

fn import_timestamp_unix_ms(value: &str) -> Result<u64, ConsoleWriteError> {
    let timestamp = validate_import_timestamp(value)?;
    u64::try_from(timestamp.unix_timestamp_nanos() / 1_000_000)
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(invalid_persistence_import)
}

fn validate_capability_posture(runtime: &PersistenceRuntime) -> Result<(), ConsoleWriteError> {
    if let Some(timestamp) = runtime.capability_fetched_at.as_deref() {
        validate_import_timestamp(timestamp)?;
    }
    if runtime
        .capability_fetch_error
        .as_deref()
        .is_some_and(|error| error != "capability_fetch_failed")
    {
        return Err(invalid_persistence_import());
    }
    let valid = match runtime.capability_source.as_str() {
        "manual" => runtime.capability_fetched_at.is_none(),
        "gewyvern-api" => {
            (runtime.capability_fetched_at.is_some() && runtime.capability_fetch_error.is_none())
                || (runtime.capability_fetched_at.is_none()
                    && runtime.capability_fetch_error.as_deref() == Some("capability_fetch_failed"))
        }
        "fetch_failed" => {
            runtime.capability_fetched_at.is_none()
                && runtime.capability_fetch_error.as_deref() == Some("capability_fetch_failed")
        }
        _ => false,
    };
    valid.then_some(()).ok_or_else(invalid_persistence_import)
}

fn import_capabilities(
    imported: &[PersistenceCapability],
    source: &str,
) -> Result<RuntimeCapabilitySnapshot, ConsoleWriteError> {
    if imported.len() > 256 {
        return Err(invalid_persistence_import());
    }
    let mut keys = HashSet::with_capacity(imported.len());
    let mut projected_keys = HashSet::with_capacity(imported.len());
    let mut capabilities = RuntimeCapabilitySnapshot::default();
    for capability in imported {
        if !valid_import_text(&capability.key, 128, false)
            || !valid_import_text(&capability.description, 1_024, false)
            || !matches!(
                capability.support.as_str(),
                "fully_supported" | "risky" | "not_supported"
            )
            || !keys.insert(capability.key.to_ascii_lowercase())
        {
            return Err(invalid_persistence_import());
        }
        let supported = capability.support == "fully_supported";
        match capability.key.as_str() {
            "latest_snapshot" | "api.latest_snapshot" => {
                if !projected_keys.insert("latest_snapshot".to_string()) {
                    return Err(invalid_persistence_import());
                }
                capabilities.latest_snapshot = supported;
            }
            "authenticated_deployment" | "control.authenticated_deployment" => {
                if !projected_keys.insert("authenticated_deployment".to_string()) {
                    return Err(invalid_persistence_import());
                }
                capabilities.authenticated_deployment = supported;
            }
            "serve_required" | "runtime.serve_required" => {
                if !projected_keys.insert("serve_required".to_string()) {
                    return Err(invalid_persistence_import());
                }
                capabilities.serve_required = supported;
            }
            "external_sidecar_context" | "api.external_sidecar_context" => {
                if !projected_keys.insert("external_sidecar_context".to_string()) {
                    return Err(invalid_persistence_import());
                }
                capabilities.external_sidecar_context = supported;
            }
            "api.target_routing" => {
                if !projected_keys.insert("target_routing".to_string()) {
                    return Err(invalid_persistence_import());
                }
            }
            key => {
                let key = normalize_import_capability_key(key)?;
                if matches!(
                    key.as_str(),
                    "latest_snapshot"
                        | "authenticated_deployment"
                        | "serve_required"
                        | "external_sidecar_context"
                        | "target_routing"
                ) || !projected_keys.insert(key.clone())
                    || capabilities.extensions.insert(key, supported).is_some()
                {
                    return Err(invalid_persistence_import());
                }
            }
        }
    }
    if source == "gewyvern-api" {
        capabilities.source = "gewyvern-api".into();
        capabilities.service = "gewyvern-api".into();
        capabilities.version = "1".into();
        capabilities.target_path_segment_encoding = "percent-encoding".into();
        capabilities.target_direct_path_chars = "unreserved".into();
        capabilities.endpoints.push("/v1/capabilities".into());
        if capabilities.authenticated_deployment {
            capabilities.endpoints.push("/v1/deployments".into());
        }
    }
    Ok(capabilities)
}

fn normalize_import_capability_key(key: &str) -> Result<String, ConsoleWriteError> {
    let normalized = key
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' => char::from(byte.to_ascii_lowercase()),
            b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-' => char::from(byte),
            _ => '_',
        })
        .collect::<String>();
    if normalized.is_empty() || normalized.len() > 64 {
        return Err(invalid_persistence_import());
    }
    Ok(normalized)
}

fn import_runtime_status(
    imported: &PersistenceRuntimeStatus,
) -> Result<RuntimeStatusSnapshot, ConsoleWriteError> {
    if let Some(timestamp) = imported.status_fetched_at.as_deref() {
        validate_import_timestamp(timestamp)?;
    }
    let bounded = valid_import_optional_text(imported.status_fetch_error.as_deref(), 128)
        && valid_import_optional_text(imported.snapshot_kind.as_deref(), 128)
        && valid_import_optional_text(imported.resilience_status.as_deref(), 128)
        && valid_import_optional_text(imported.resilience_summary.as_deref(), 1_024)
        && valid_import_optional_text(imported.socket_service_status.as_deref(), 128)
        && imported
            .target_count
            .is_none_or(|value| value <= 10_000_000)
        && imported
            .socket_consecutive_idle_timeouts
            .is_none_or(|value| value <= 10_000_000)
        && imported
            .socket_total_idle_timeouts
            .is_none_or(|value| value <= 100_000_000);
    let posture = match imported.status_source.as_str() {
        "unobserved" => {
            imported.status_fetched_at.is_none()
                && imported.status_fetch_error.is_none()
                && !imported.has_latest_snapshot
        }
        "gewyvern-api" => {
            imported.status_fetched_at.is_some() && imported.status_fetch_error.is_none()
        }
        "fetch_failed" => {
            imported.status_fetched_at.is_none()
                && imported.status_fetch_error.as_deref() == Some("runtime_status_fetch_failed")
                && !imported.has_latest_snapshot
        }
        _ => false,
    };
    if !bounded || !posture {
        return Err(invalid_persistence_import());
    }
    Ok(RuntimeStatusSnapshot {
        status_source: imported.status_source.clone(),
        status_fetched_at: imported.status_fetched_at.clone(),
        status_fetch_error: imported.status_fetch_error.clone(),
        has_latest_snapshot: imported.has_latest_snapshot,
        snapshot_kind: imported.snapshot_kind.clone(),
        target_count: imported.target_count,
        has_summary_json: imported.has_summary_json,
        has_analysis_json: imported.has_analysis_json,
        has_training_example_json: imported.has_training_example_json,
        has_training_dataset_manifest: imported.has_training_dataset_manifest,
        has_export_json: imported.has_export_json,
        has_report_json: imported.has_report_json,
        has_report_html: imported.has_report_html,
        has_external_sidecar_context: imported.has_external_sidecar_context,
        has_external_evidence_chain_enrichment: imported.has_external_evidence_chain_enrichment,
        has_external_diagnostic_opinion: imported.has_external_diagnostic_opinion,
        resilience_degraded: imported.resilience_degraded,
        resilience_status: imported.resilience_status.clone(),
        resilience_summary: imported.resilience_summary.clone(),
        socket_service_status: imported.socket_service_status.clone(),
        socket_consecutive_idle_timeouts: imported.socket_consecutive_idle_timeouts,
        socket_total_idle_timeouts: imported.socket_total_idle_timeouts,
    })
}

fn import_sidecar_status(
    imported: &PersistenceSidecarStatus,
) -> Result<RuntimeSidecarStatusSnapshot, ConsoleWriteError> {
    if let Some(timestamp) = imported.status_fetched_at.as_deref() {
        validate_import_timestamp(timestamp)?;
    }
    let posture = match imported.status_source.as_str() {
        "etragon-api" => {
            imported.status_fetched_at.is_some()
                && imported.status_fetch_error.is_none()
                && imported
                    .last_error
                    .as_deref()
                    .is_none_or(|error| error == "sidecar_reported_error")
        }
        "fetch_failed" => {
            imported.status_fetched_at.is_none()
                && imported.status_fetch_error.as_deref() == Some("sidecar_fetch_failed")
                && imported
                    .last_error
                    .as_deref()
                    .is_none_or(|error| error == "sidecar_fetch_failed")
                && !imported.healthy
        }
        _ => false,
    };
    if !posture
        || !valid_import_text(&imported.daemon_status, 128, false)
        || !valid_import_optional_text(imported.status_fetch_error.as_deref(), 128)
        || !valid_import_optional_text(imported.last_error.as_deref(), 128)
        || imported
            .target_count
            .is_some_and(|value| value > 10_000_000)
        || imported.learned_routes > 10_000_000
    {
        return Err(invalid_persistence_import());
    }
    let memory = imported
        .memory
        .as_ref()
        .map(import_sidecar_memory)
        .transpose()?;
    Ok(RuntimeSidecarStatusSnapshot {
        status_source: imported.status_source.clone(),
        status_fetched_at: imported.status_fetched_at.clone(),
        status_fetch_error: imported.status_fetch_error.clone(),
        healthy: imported.healthy,
        daemon_status: imported.daemon_status.clone(),
        target_count: imported.target_count,
        learning_active: imported.learning_active,
        learned_routes: imported.learned_routes,
        has_evidence_chain_enrichment: imported.has_evidence_chain_enrichment,
        has_diagnostic_opinion: imported.has_diagnostic_opinion,
        last_error: imported.last_error.clone(),
        memory,
    })
}

fn import_sidecar_memory(
    imported: &PersistenceSidecarMemory,
) -> Result<RuntimeSidecarMemorySnapshot, ConsoleWriteError> {
    if imported.slot_count > 10_000
        || imported.history_count > 1_000_000
        || imported.slots.len() > 128
        || !valid_import_optional_text(imported.latest_slot.as_deref(), 128)
        || !valid_import_optional_text(imported.latest_label.as_deref(), 256)
        || !valid_import_optional_text(imported.latest_source.as_deref(), 128)
        || imported
            .fetch_error
            .as_deref()
            .is_some_and(|error| error != "sidecar_memory_fetch_failed")
    {
        return Err(invalid_persistence_import());
    }
    let mut slot_ids = HashSet::with_capacity(imported.slots.len());
    let mut slots = Vec::with_capacity(imported.slots.len());
    for slot in &imported.slots {
        if !slot_ids.insert(slot.slot.to_ascii_lowercase())
            || !valid_import_text(&slot.slot, 128, false)
            || !valid_import_optional_text(slot.label.as_deref(), 256)
            || !valid_import_optional_text(slot.note.as_deref(), 1_024)
            || !valid_import_text(&slot.source, 128, false)
            || slot.pattern_count > 10_000_000
            || slot.label_count > 10_000_000
        {
            return Err(invalid_persistence_import());
        }
        if let Some(timestamp) = slot.saved_at.as_deref() {
            validate_import_timestamp(timestamp)?;
        }
        slots.push(RuntimeSidecarMemorySlotSnapshot {
            slot: slot.slot.clone(),
            label: slot.label.clone(),
            note: slot.note.clone(),
            source: slot.source.clone(),
            saved_at: slot.saved_at.clone(),
            pattern_count: slot.pattern_count,
            label_count: slot.label_count,
        });
    }
    Ok(RuntimeSidecarMemorySnapshot {
        versions_supported: imported.versions_supported,
        slot_count: imported.slot_count,
        history_count: imported.history_count,
        latest_slot: imported.latest_slot.clone(),
        latest_label: imported.latest_label.clone(),
        latest_source: imported.latest_source.clone(),
        slots,
        fetch_error: imported.fetch_error.clone(),
    })
}

fn import_orchestra_runs(
    imported: &[PersistenceOrchestraRun],
    runtime_ids: &HashSet<String>,
) -> Result<Vec<OrchestraImportRecord>, ConsoleWriteError> {
    let mut run_ids = HashSet::with_capacity(imported.len());
    let mut runs_by_id = BTreeMap::new();
    let mut request_ids = HashSet::new();
    for run in imported {
        validate_import_identity(&run.run_id)?;
        validate_import_identity(&run.runtime_id)?;
        validate_import_identity(&run.plan_id)?;
        if !runtime_ids.contains(&run.runtime_id)
            || !run_ids.insert(run.run_id.to_ascii_lowercase())
            || !matches!(
                run.outcome.as_str(),
                "succeeded" | "degraded" | "failed" | "cancelled" | "ok"
            )
            || run.attempt == 0
            || run.attempt > 1_000_000
            || run.steps.len() > 256
            || !valid_import_optional_text(run.approved_by.as_deref(), 256)
            || !valid_import_optional_text(run.approval_note.as_deref(), 1_024)
        {
            return Err(invalid_persistence_import());
        }
        if let Some(identity) = run.retried_from_run_id.as_deref() {
            validate_import_identity(identity)?;
            if identity.eq_ignore_ascii_case(&run.run_id) || run.attempt < 2 {
                return Err(invalid_persistence_import());
            }
        } else if run.attempt != 1 {
            return Err(invalid_persistence_import());
        }
        if let Some(identity) = run.plan_revision.as_deref() {
            validate_import_identity(identity)?;
        }
        if let Some(identity) = run.request_id.as_deref() {
            validate_import_identity(identity)?;
            if !request_ids.insert((run.runtime_id.clone(), identity.to_string())) {
                return Err(invalid_persistence_import());
            }
        }
        let executed_at = validate_import_timestamp(&run.executed_at)?;
        let completed_at = run
            .completed_at
            .as_deref()
            .map(validate_import_timestamp)
            .transpose()?;
        if completed_at.is_some_and(|completed_at| completed_at < executed_at)
            || run.steps.iter().any(|step| {
                !valid_import_text(&step.step, 128, false)
                    || !valid_import_text(&step.outcome, 128, false)
                    || !valid_import_text(&step.summary, 1_024, true)
            })
        {
            return Err(invalid_persistence_import());
        }
        runs_by_id.insert(run.run_id.to_ascii_lowercase(), (run, executed_at));
    }
    for run in imported {
        let Some(parent_id) = run.retried_from_run_id.as_ref() else {
            continue;
        };
        let Some((parent, parent_executed_at)) = runs_by_id.get(&parent_id.to_ascii_lowercase())
        else {
            continue;
        };
        let (_, executed_at) = runs_by_id
            .get(&run.run_id.to_ascii_lowercase())
            .ok_or_else(invalid_persistence_import)?;
        if parent.runtime_id != run.runtime_id
            || !parent.plan_id.eq_ignore_ascii_case(&run.plan_id)
            || run.attempt != parent.attempt.saturating_add(1)
            || executed_at < parent_executed_at
        {
            return Err(invalid_persistence_import());
        }
    }

    let mut records = Vec::with_capacity(imported.len());
    for run in imported {
        let recorded_at = run.completed_at.as_deref().unwrap_or(&run.executed_at);
        let event = PersistenceOrchestraEvent {
            event_id: 0,
            run_id: &run.run_id,
            runtime_id: &run.runtime_id,
            event_type: "legacy_import",
            from_outcome: None,
            to_outcome: &run.outcome,
            summary: "Imported from Leserpent portable persistence",
            recorded_at,
        };
        records.push((
            validate_import_timestamp(&run.executed_at)?.unix_timestamp_nanos(),
            OrchestraImportRecord {
                run_id: run.run_id.clone(),
                runtime_id: run.runtime_id.clone(),
                request_id: run.request_id.clone(),
                event_type: "legacy_import".into(),
                outcome: run.outcome.clone(),
                recorded_at: recorded_at.to_string(),
                run: serde_json::to_vec(run).map_err(|_| invalid_persistence_import())?,
                event: serde_json::to_vec(&event).map_err(|_| invalid_persistence_import())?,
            },
        ));
    }
    records.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| left.1.run_id.cmp(&right.1.run_id))
    });
    Ok(records.into_iter().map(|(_, record)| record).collect())
}

fn validate_import_identity(value: &str) -> Result<(), ConsoleWriteError> {
    RuntimeId::new(value.to_string())
        .map(|_| ())
        .map_err(|_| invalid_persistence_import())
}

fn valid_import_optional_text(value: Option<&str>, maximum: usize) -> bool {
    value.is_none_or(|value| valid_import_text(value, maximum, false))
}

fn valid_import_text(value: &str, maximum: usize, allow_empty: bool) -> bool {
    value.len() <= maximum
        && (allow_empty || !value.is_empty())
        && value == value.trim()
        && !value.chars().any(char::is_control)
}

fn invalid_persistence_import() -> ConsoleWriteError {
    ConsoleWriteError {
        status: ConsoleWriteStatus::BadRequest,
        code: "invalid_persistence_import",
        reason: "control-plane import document failed portable schema validation",
    }
}

fn map_persistence_import_runtime_error(error: RuntimeError) -> ConsoleWriteError {
    match error {
        RuntimeError::InvalidSnapshot(_) | RuntimeError::Domain(_) => invalid_persistence_import(),
        RuntimeError::Storage(message)
            if message.contains("control-plane import is blocked")
                || message.contains("control-plane import requires")
                || message.contains("control-plane import would alter") =>
        {
            ConsoleWriteError {
                status: ConsoleWriteStatus::Conflict,
                code: "persistence_import_not_quiescent",
                reason: "control-plane import requires a quiescent authority state",
            }
        }
        error => map_runtime_error(error),
    }
}

fn registration_plan(
    body: &[u8],
    runtime: &ControlRuntime,
    registration_available: bool,
) -> Result<Value, ConsoleWriteError> {
    if body.is_empty() || body.len() > MAX_REGISTRATION_PLAN_BYTES {
        return Err(invalid_registration_plan());
    }
    let request: RegistrationPlanRequest =
        serde_json::from_slice(body).map_err(|_| invalid_registration_plan())?;
    let coordinates = normalize_registration_coordinates(
        request.name,
        request.endpoint,
        request.sidecar_endpoint,
        request.tls_ca_sha256,
        &RuntimeTags::default(),
    )?;
    let decision = registration_decision(runtime, &coordinates, registration_available)?;
    let action = decision
        .action
        .map(registration_action_label)
        .unwrap_or("reject");
    let reason_message = match decision.reason {
        Some("endpoint_conflict") => "runtime endpoint is already registered to another runtime",
        Some(REGISTRATION_LOOPBACK_REASON) => {
            "remote Gewyvern targets require an explicit HTTPS origin"
        }
        Some(REGISTRATION_CA_REASON) => {
            "HTTPS Gewyvern registration requires an explicitly reviewed CA certificate"
        }
        Some(REGISTRATION_CA_MISMATCH_REASON) => {
            "CA trust is only accepted for an HTTPS Gewyvern origin"
        }
        Some(REGISTRATION_HTTPS_ORIGIN_REASON) => {
            "Gewyvern HTTPS registration requires a root origin without path, query, or credentials"
        }
        Some(REGISTRATION_TRANSACTION_REASON) => {
            "native Rust registration requires daemon-owned durable credential authority"
        }
        Some(_) => "runtime registration is unavailable",
        None => "runtime registration plan is ready",
    };
    Ok(json!({
        "allowed": decision.action.is_some(),
        "action": action,
        "reason": decision.reason,
        "reasonMessage": reason_message,
        "existingRuntimeId": decision.existing.as_ref().map(|runtime| runtime.id.as_str()),
        "existingRuntimeName": decision.existing.as_ref().map(|runtime| runtime.name.as_str()),
        "existingRuntimeEndpoint": decision.existing.as_ref().map(|runtime| runtime.endpoint.as_str()),
        "plannedRuntimeId": decision.planned_runtime_id.as_str(),
        "expectedRevision": decision.expected_revision.map(|revision| revision.0),
        "authorityBound": true,
        "trustMode": if coordinates.tls_ca_sha256.is_some() { "pinned_https" } else { "loopback_http" },
        "tlsCaSha256": coordinates.tls_ca_sha256,
        "planToken": decision.plan_token,
    }))
}

fn registration_decision(
    runtime: &ControlRuntime,
    coordinates: &RegistrationCoordinates,
    registration_available: bool,
) -> Result<RegistrationDecision, ConsoleWriteError> {
    let (_, runtimes) = runtime.runtime_event_state();
    let same_name = runtimes
        .iter()
        .find(|runtime| runtime.name.eq_ignore_ascii_case(&coordinates.name));
    let same_endpoint = runtimes.iter().find(|runtime| {
        normalize_http_endpoint(&runtime.endpoint)
            .is_some_and(|candidate| candidate == coordinates.endpoint)
    });
    let endpoint_conflict = same_endpoint.is_some_and(|endpoint_owner| {
        same_name.is_none_or(|name_owner| endpoint_owner.id != name_owner.id)
    });
    let existing = if endpoint_conflict {
        same_endpoint
    } else {
        same_name
    }
    .cloned();
    let planned_runtime_id = match &existing {
        Some(runtime) => runtime.id.clone(),
        None => RuntimeId::new(proposed_runtime_id(
            &coordinates.name,
            &coordinates.endpoint,
        ))
        .map_err(|error| map_domain_error(error.into()))?,
    };
    let expected_revision = existing.as_ref().map(|runtime| runtime.revision);
    let transport_reason = registration_transport_reason(coordinates);
    let (action, reason) = if endpoint_conflict {
        (None, Some("endpoint_conflict"))
    } else if !registration_available {
        (None, Some(REGISTRATION_TRANSACTION_REASON))
    } else if let Some(reason) = transport_reason {
        (None, Some(reason))
    } else if existing.is_some() {
        (Some(RuntimeTargetRegistrationAction::Update), None)
    } else {
        (Some(RuntimeTargetRegistrationAction::Create), None)
    };
    let plan_token = registration_plan_token(
        &coordinates.name,
        &coordinates.endpoint,
        coordinates.sidecar_endpoint.as_deref(),
        coordinates.tls_ca_sha256.as_deref(),
        &planned_runtime_id,
        expected_revision,
        action.map(registration_action_label).unwrap_or("reject"),
    );
    Ok(RegistrationDecision {
        action,
        reason,
        existing,
        planned_runtime_id,
        expected_revision,
        plan_token,
    })
}

fn register_runtime(
    body: &[u8],
    runtime: &mut ControlRuntime,
    writer_fence: Option<&AuthorityWriterFence>,
    registration: &RuntimeTargetRegistrationAuthority,
) -> Result<Value, ConsoleWriteError> {
    require_writer(runtime, writer_fence)?;
    if body.is_empty() || body.len() > MAX_REGISTRATION_REQUEST_BYTES {
        return Err(invalid_registration_request());
    }
    let request: RegistrationRequest =
        serde_json::from_slice(body).map_err(|_| invalid_registration_request())?;
    if !request.capabilities.is_empty()
        || nonempty(&request.capability_endpoint)
        || nonempty(&request.status_endpoint)
        || nonempty(&request.sidecar_status_endpoint)
        || nonempty(&request.sidecar_admin_token)
    {
        return Err(invalid_registration_request());
    }
    let tags = RuntimeTags::from(request.tags);
    let coordinates = normalize_registration_coordinates(
        request.name,
        request.endpoint,
        request.sidecar_endpoint,
        request.tls_ca_sha256,
        &tags,
    )?;
    if request.registration_plan_token.len() != 64
        || !request
            .registration_plan_token
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        || request.pairing_token.len() > MAX_SECRET_BYTES
    {
        return Err(invalid_registration_request());
    }
    let secret =
        SecretValue::new(request.pairing_token).map_err(|_| invalid_registration_request())?;
    let operation_id = format!("web-register-{}", request.registration_plan_token);
    let persisted = registration
        .persisted_intent(runtime, &operation_id)
        .map_err(map_registration_error)?;
    let intent = match persisted {
        Some(intent) => {
            if intent.plan_token != request.registration_plan_token
                || intent.name != coordinates.name
                || intent.endpoint != coordinates.endpoint
                || intent.sidecar_endpoint != coordinates.sidecar_endpoint
                || intent.tags != tags
                || intent.tls_ca_sha256 != coordinates.tls_ca_sha256
                || intent.tls_ca_pem != request.tls_ca_pem
            {
                return Err(registration_conflict());
            }
            intent
        }
        None => {
            let decision = registration_decision(runtime, &coordinates, true)?;
            if decision.plan_token != request.registration_plan_token {
                return Err(registration_conflict());
            }
            let action = decision.action.ok_or_else(registration_conflict)?;
            RuntimeTargetRegistrationIntent::new_with_trust(
                action,
                RuntimeTargetDescriptor {
                    runtime_id: decision.planned_runtime_id,
                    name: coordinates.name,
                    endpoint: coordinates.endpoint,
                    sidecar_endpoint: coordinates.sidecar_endpoint,
                    tags,
                },
                decision.expected_revision,
                request.registration_plan_token,
                request.tls_ca_pem,
                coordinates.tls_ca_sha256,
            )
            .map_err(map_registration_error)?
        }
    };
    let outcome = registration
        .execute(runtime, &intent, &secret)
        .map_err(map_registration_error)?;
    let refresh = if request.fetch_capabilities {
        match execute_registration_capability_refresh(
            runtime,
            &intent,
            outcome.registration_revision,
        ) {
            Ok(_) => json!({ "requested": true, "scheduled": true, "error": Value::Null }),
            Err(_) => json!({
                "requested": true,
                "scheduled": false,
                "error": "capability_refresh_not_scheduled"
            }),
        }
    } else {
        json!({ "requested": false, "scheduled": false, "error": Value::Null })
    };
    let projection = runtime
        .runtime_projection(&intent.runtime_id)
        .cloned()
        .unwrap_or(outcome.projection);
    let mut value = runtime_value(&projection);
    value["registrationReplayed"] = Value::Bool(outcome.replayed);
    value["capabilityRefresh"] = refresh;
    Ok(value)
}

fn normalize_registration_coordinates(
    name: String,
    endpoint: String,
    sidecar_endpoint: Option<String>,
    tls_ca_sha256: Option<String>,
    tags: &RuntimeTags,
) -> Result<RegistrationCoordinates, ConsoleWriteError> {
    let name = name.trim().to_string();
    let endpoint = normalize_http_endpoint(&endpoint).ok_or_else(invalid_registration_plan)?;
    let sidecar_endpoint = sidecar_endpoint
        .filter(|endpoint| !endpoint.trim().is_empty())
        .map(|endpoint| normalize_http_endpoint(&endpoint).ok_or_else(invalid_registration_plan))
        .transpose()?;
    let tls_ca_sha256 = tls_ca_sha256
        .filter(|fingerprint| !fingerprint.trim().is_empty())
        .map(|fingerprint| {
            let fingerprint = fingerprint.trim().to_ascii_lowercase();
            valid_sha256(&fingerprint)
                .then_some(fingerprint)
                .ok_or_else(invalid_registration_plan)
        })
        .transpose()?;
    validate_registration_intent(&name, &endpoint, sidecar_endpoint.as_deref(), tags)
        .map_err(|_| invalid_registration_plan())?;
    Ok(RegistrationCoordinates {
        name,
        endpoint,
        sidecar_endpoint,
        tls_ca_sha256,
    })
}

fn registration_transport_reason(coordinates: &RegistrationCoordinates) -> Option<&'static str> {
    if loopback_address(&coordinates.endpoint).is_ok() {
        return coordinates
            .tls_ca_sha256
            .is_some()
            .then_some(REGISTRATION_CA_MISMATCH_REASON);
    }
    if !coordinates.endpoint.starts_with("https://") {
        return Some(REGISTRATION_LOOPBACK_REASON);
    }
    let Some(origin) = coordinates.endpoint.strip_suffix('/') else {
        return Some(REGISTRATION_HTTPS_ORIGIN_REASON);
    };
    if GewyvernTarget::validate_https_origin(origin).is_err() {
        return Some(REGISTRATION_HTTPS_ORIGIN_REASON);
    }
    coordinates
        .tls_ca_sha256
        .is_none()
        .then_some(REGISTRATION_CA_REASON)
}

fn invalid_registration_plan() -> ConsoleWriteError {
    ConsoleWriteError {
        status: ConsoleWriteStatus::BadRequest,
        code: "invalid_runtime_registration_plan",
        reason: "registration plan requires bounded name and HTTP(S) endpoint fields",
    }
}

fn invalid_registration_request() -> ConsoleWriteError {
    ConsoleWriteError {
        status: ConsoleWriteStatus::BadRequest,
        code: "invalid_runtime_registration",
        reason: "runtime registration request is invalid or contains unsupported credential fields",
    }
}

fn registration_unavailable() -> ConsoleWriteError {
    ConsoleWriteError {
        status: ConsoleWriteStatus::ServiceUnavailable,
        code: "runtime_registration_unavailable",
        reason: "durable runtime target registration authority is unavailable",
    }
}

fn registration_conflict() -> ConsoleWriteError {
    ConsoleWriteError {
        status: ConsoleWriteStatus::Conflict,
        code: "runtime_registration_plan_changed",
        reason: "runtime registration authority changed after the plan was reviewed",
    }
}

fn map_registration_error(error: RuntimeTargetRegistrationError) -> ConsoleWriteError {
    match error.kind {
        RuntimeTargetRegistrationErrorKind::Invalid => invalid_registration_request(),
        RuntimeTargetRegistrationErrorKind::Conflict => registration_conflict(),
        RuntimeTargetRegistrationErrorKind::Unavailable => registration_unavailable(),
        RuntimeTargetRegistrationErrorKind::Internal => ConsoleWriteError {
            status: ConsoleWriteStatus::InternalServerError,
            code: "runtime_registration_failed",
            reason: "runtime registration authority failed closed",
        },
    }
}

fn nonempty(value: &Option<String>) -> bool {
    value.as_ref().is_some_and(|value| !value.trim().is_empty())
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn normalize_http_endpoint(value: &str) -> Option<String> {
    normalize_runtime_http_endpoint(value)
}

fn proposed_runtime_id(name: &str, endpoint: &str) -> String {
    let input = format!("{}\0{}", name.to_ascii_lowercase(), endpoint);
    sha256_hex(input.as_bytes())[..32].to_string()
}

fn registration_plan_token(
    name: &str,
    endpoint: &str,
    sidecar_endpoint: Option<&str>,
    tls_ca_sha256: Option<&str>,
    planned_runtime_id: &RuntimeId,
    expected_revision: Option<Revision>,
    action: &str,
) -> String {
    let input = format!(
        "runtime-registration-plan-v3\n{}\n{}\n{}\n{}\ndaemon\n{}\n{}\n{}",
        name.to_ascii_lowercase(),
        endpoint,
        sidecar_endpoint.unwrap_or_default(),
        tls_ca_sha256.unwrap_or_default(),
        action,
        planned_runtime_id.as_str().to_ascii_lowercase(),
        expected_revision.map_or_else(String::new, |revision| revision.0.to_string()),
    );
    sha256_hex(input.as_bytes())
}

fn registration_action_label(action: RuntimeTargetRegistrationAction) -> &'static str {
    match action {
        RuntimeTargetRegistrationAction::Create => "create",
        RuntimeTargetRegistrationAction::Update => "update",
    }
}

fn refresh_fleet(
    runtime: &mut ControlRuntime,
    writer_fence: Option<&AuthorityWriterFence>,
    filter: &RuntimeListFilter,
    kind: RefreshKind,
) -> Result<Value, ConsoleWriteError> {
    require_writer(runtime, writer_fence)?;
    let (_, runtimes) = runtime.runtime_event_state();
    let targets = runtimes
        .into_iter()
        .filter(|runtime| runtime_matches(runtime, filter))
        .collect::<Vec<_>>();
    let mut refreshed = Vec::with_capacity(targets.len());
    for mut target in targets {
        let result = match kind {
            RefreshKind::Status => execute_refresh(
                runtime,
                &target,
                "status",
                Command::RuntimeRefresh {
                    runtime_id: target.id.clone(),
                },
            ),
            RefreshKind::Capabilities => execute_refresh(
                runtime,
                &target,
                "capabilities",
                Command::RuntimeCapabilitiesRefresh {
                    runtime_id: target.id.clone(),
                },
            ),
            RefreshKind::All => {
                target = execute_refresh(
                    runtime,
                    &target,
                    "status",
                    Command::RuntimeRefresh {
                        runtime_id: target.id.clone(),
                    },
                )?;
                execute_refresh(
                    runtime,
                    &target,
                    "capabilities",
                    Command::RuntimeCapabilitiesRefresh {
                        runtime_id: target.id.clone(),
                    },
                )
            }
        }?;
        refreshed.push(json!({
            "runtimeId": result.id.as_str(),
            "name": result.name,
            "endpoint": result.endpoint,
            "outcome": "scheduled",
            "revision": result.revision.0,
        }));
    }
    Ok(json!({
        "filter": filter_value(filter),
        "refresh": {
            "refreshedCount": refreshed.len(),
            "runtimes": refreshed,
        }
    }))
}

fn execute_refresh(
    runtime: &mut ControlRuntime,
    target: &RuntimeProjection,
    kind: &str,
    command: Command,
) -> Result<RuntimeProjection, ConsoleWriteError> {
    let identity = stable_identifier("web-refresh", target, kind);
    let envelope = CommandEnvelope {
        schema_version: leserpent_domain::DOMAIN_SCHEMA_VERSION,
        command_id: CommandId::new(identity.clone()).map_err(map_domain_error)?,
        idempotency_key: IdempotencyKey::new(identity).map_err(map_domain_error)?,
        expected_revision: Some(target.revision),
        principal: Principal {
            id: "rust-web-console".into(),
        },
        capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_REFRESH]),
        origin: CommandOrigin::CompatibilityAdapter,
        confirmation: Confirmation::Confirmed,
        dry_run: false,
        command,
    };
    match runtime.execute_plan(CommandPlan {
        schema_version: COMMAND_PLAN_SCHEMA_VERSION,
        required_capability: CAPABILITY_RUNTIME_REFRESH.into(),
        operation: PlannedOperation::Command(envelope),
    }) {
        Ok(PlanResult::Command(result)) => Ok(result.runtime),
        Ok(PlanResult::Query(_)) => Err(ConsoleWriteError {
            status: ConsoleWriteStatus::InternalServerError,
            code: "runtime_refresh_confused",
            reason: "runtime refresh returned a query result",
        }),
        Err(error) => Err(map_runtime_error(error)),
    }
}

fn execute_registration_capability_refresh(
    runtime: &mut ControlRuntime,
    intent: &RuntimeTargetRegistrationIntent,
    expected_revision: Revision,
) -> Result<RuntimeProjection, ConsoleWriteError> {
    let identity = format!("web-registration-refresh-{}", intent.plan_token);
    let envelope = CommandEnvelope {
        schema_version: leserpent_domain::DOMAIN_SCHEMA_VERSION,
        command_id: CommandId::new(identity.clone()).map_err(map_domain_error)?,
        idempotency_key: IdempotencyKey::new(identity).map_err(map_domain_error)?,
        expected_revision: Some(expected_revision),
        principal: Principal {
            id: "rust-web-console".into(),
        },
        capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_REFRESH]),
        origin: CommandOrigin::CompatibilityAdapter,
        confirmation: Confirmation::Confirmed,
        dry_run: false,
        command: Command::RuntimeCapabilitiesRefresh {
            runtime_id: intent.runtime_id.clone(),
        },
    };
    match runtime.execute_plan(CommandPlan {
        schema_version: COMMAND_PLAN_SCHEMA_VERSION,
        required_capability: CAPABILITY_RUNTIME_REFRESH.into(),
        operation: PlannedOperation::Command(envelope),
    }) {
        Ok(PlanResult::Command(result)) => Ok(result.runtime),
        Ok(PlanResult::Query(_)) => Err(ConsoleWriteError {
            status: ConsoleWriteStatus::InternalServerError,
            code: "runtime_refresh_confused",
            reason: "runtime refresh returned a query result",
        }),
        Err(error) => Err(map_runtime_error(error)),
    }
}

fn cleanup_runtimes(
    runtime: &mut ControlRuntime,
    writer_fence: Option<&AuthorityWriterFence>,
    registration: Option<&RuntimeTargetRegistrationAuthority>,
    kind: CleanupKind,
    filter: &RuntimeListFilter,
    body: &[u8],
) -> Result<Value, ConsoleWriteError> {
    require_writer(runtime, writer_fence)?;
    if body.is_empty() || body.len() > MAX_CLEANUP_REQUEST_BYTES {
        return Err(invalid_cleanup_request());
    }
    let request: CleanupRequest =
        serde_json::from_slice(body).map_err(|_| invalid_cleanup_request())?;
    if request.plan_token.len() != 64
        || !request
            .plan_token
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(invalid_cleanup_request());
    }
    let command_id =
        CommandId::new(format!("web-cleanup-{}", request.plan_token)).map_err(map_domain_error)?;

    let lookup = runtime
        .runtime_unregistration_receipt(command_id.clone())
        .map_err(map_runtime_error)?;
    if let Some(receipt) = lookup.receipt {
        validate_cleanup_challenge(kind, request.challenge.as_deref(), receipt.removed.len())?;
        retire_runtime_targets(
            runtime,
            registration,
            receipt.removed.iter().map(|target| &target.runtime_id),
        )?;
        let removed_runtime_ids = receipt
            .removed
            .iter()
            .map(|target| target.runtime_id.as_str())
            .collect::<Vec<_>>();
        return Ok(json!({
            "deleted": true,
            "filter": filter_value(filter),
            "removedRuntimeCount": receipt.removed.len(),
            "removedSessionCount": receipt.deleted_session_count,
            "removedRuntimeNames": [],
            "removedRuntimeIds": removed_runtime_ids,
            "operationGeneration": receipt.operation_generation,
            "removedAtUnixMs": receipt.removed_at_unix_ms,
            "replayed": true,
            "deletedOrchestraRuntimeCount": receipt.deleted_orchestra_runtime_count,
            "deletedOrchestraRunCount": receipt.deleted_orchestra_run_count,
            "deletedOrchestraEventCount": receipt.deleted_orchestra_event_count,
        }));
    }

    let (_, runtimes) = runtime.runtime_event_state();
    let sessions = runtime.list_control_sessions().map_err(map_runtime_error)?;
    let plan = build_cleanup_plan_with_sessions(filter, &runtimes, &sessions);
    let action = plan.action(kind);
    if request.plan_token != action.plan_token {
        return Err(cleanup_plan_changed(
            "runtime cleanup plan changed; review the current targets before retrying",
        ));
    }
    validate_cleanup_challenge(kind, request.challenge.as_deref(), action.targets.len())?;
    if action.targets.len() > MAX_ATOMIC_CLEANUP_TARGETS {
        return Err(ConsoleWriteError {
            status: ConsoleWriteStatus::Conflict,
            code: "runtime_cleanup_atomic_limit",
            reason: "runtime cleanup exceeds the 128-target atomic transaction limit",
        });
    }
    if action.targets.is_empty() {
        return Ok(json!({
            "deleted": true,
            "filter": filter_value(filter),
            "removedRuntimeCount": 0,
            "removedSessionCount": 0,
            "removedRuntimeNames": [],
            "removedRuntimeIds": [],
            "operationGeneration": Value::Null,
            "removedAtUnixMs": Value::Null,
            "replayed": false,
            "deletedOrchestraRuntimeCount": 0,
            "deletedOrchestraRunCount": 0,
            "deletedOrchestraEventCount": 0,
        }));
    }

    let removed_runtime_names = action
        .targets
        .iter()
        .map(|target| target.name.as_str())
        .collect::<Vec<_>>();
    let targets = action
        .targets
        .iter()
        .map(|target| RuntimeUnregisterTarget {
            runtime_id: target.runtime_id.clone(),
            expected_revision: target.revision,
        })
        .collect::<Vec<_>>();
    let result = runtime
        .unregister_runtimes(command_id, targets)
        .map_err(map_runtime_error)?;
    retire_runtime_targets(
        runtime,
        registration,
        result.removed.iter().map(|target| &target.runtime_id),
    )?;
    let removed_runtime_ids = result
        .removed
        .iter()
        .map(|target| target.runtime_id.as_str())
        .collect::<Vec<_>>();
    Ok(json!({
        "deleted": true,
        "filter": filter_value(filter),
        "removedRuntimeCount": result.removed.len(),
        "removedSessionCount": result.deleted_session_count,
        "removedRuntimeNames": removed_runtime_names,
        "removedRuntimeIds": removed_runtime_ids,
        "operationGeneration": result.operation_generation,
        "removedAtUnixMs": result.removed_at_unix_ms,
        "replayed": result.replayed,
        "deletedOrchestraRuntimeCount": result.deleted_orchestra_runtime_count,
        "deletedOrchestraRunCount": result.deleted_orchestra_run_count,
        "deletedOrchestraEventCount": result.deleted_orchestra_event_count,
    }))
}

fn invalid_cleanup_request() -> ConsoleWriteError {
    ConsoleWriteError {
        status: ConsoleWriteStatus::BadRequest,
        code: "invalid_runtime_cleanup_request",
        reason: "runtime cleanup requires one lowercase SHA-256 plan token",
    }
}

fn cleanup_plan_changed(reason: &'static str) -> ConsoleWriteError {
    ConsoleWriteError {
        status: ConsoleWriteStatus::Conflict,
        code: "runtime_cleanup_plan_changed",
        reason,
    }
}

fn validate_cleanup_challenge(
    kind: CleanupKind,
    supplied: Option<&str>,
    runtime_count: usize,
) -> Result<(), ConsoleWriteError> {
    if kind != CleanupKind::Slice {
        return Ok(());
    }
    let expected = format!("CLEAR {runtime_count}");
    if supplied.map(str::trim) != Some(expected.as_str()) {
        return Err(cleanup_plan_changed(
            "runtime cleanup challenge does not match the current plan",
        ));
    }
    Ok(())
}

fn delete_runtime(
    runtime: &mut ControlRuntime,
    writer_fence: Option<&AuthorityWriterFence>,
    registration: Option<&RuntimeTargetRegistrationAuthority>,
    runtime_id: &RuntimeId,
) -> Result<Value, ConsoleWriteError> {
    require_writer(runtime, writer_fence)?;
    let projection = runtime
        .runtime_projection(runtime_id)
        .cloned()
        .ok_or(ConsoleWriteError {
            status: ConsoleWriteStatus::NotFound,
            code: "runtime_not_found",
            reason: "runtime was not found",
        })?;
    let command_id = CommandId::new(stable_identifier("web-delete", &projection, "runtime"))
        .map_err(map_domain_error)?;
    let result = runtime
        .unregister_runtimes(
            command_id,
            vec![RuntimeUnregisterTarget {
                runtime_id: runtime_id.clone(),
                expected_revision: projection.revision,
            }],
        )
        .map_err(map_runtime_error)?;
    retire_runtime_targets(runtime, registration, std::iter::once(runtime_id))?;
    Ok(json!({
        "deleted": true,
        "runtimeId": projection.id.as_str(),
        "name": projection.name,
        "endpoint": projection.endpoint,
        "removedSessionCount": result.deleted_session_count,
        "operationGeneration": result.operation_generation,
        "removedAtUnixMs": result.removed_at_unix_ms,
        "replayed": result.replayed,
        "deletedOrchestraRuntimeCount": result.deleted_orchestra_runtime_count,
        "deletedOrchestraRunCount": result.deleted_orchestra_run_count,
        "deletedOrchestraEventCount": result.deleted_orchestra_event_count,
    }))
}

fn retire_runtime_targets<'a>(
    runtime: &mut ControlRuntime,
    registration: Option<&RuntimeTargetRegistrationAuthority>,
    runtime_ids: impl IntoIterator<Item = &'a RuntimeId>,
) -> Result<(), ConsoleWriteError> {
    let Some(registration) = registration else {
        return Ok(());
    };
    for runtime_id in runtime_ids {
        registration
            .retire(runtime, runtime_id)
            .map_err(map_registration_error)?;
    }
    Ok(())
}

fn require_writer(
    runtime: &mut ControlRuntime,
    writer_fence: Option<&AuthorityWriterFence>,
) -> Result<(), ConsoleWriteError> {
    let Some(writer_fence) = writer_fence else {
        return Err(ConsoleWriteError {
            status: ConsoleWriteStatus::ServiceUnavailable,
            code: "web_console_writer_disabled",
            reason: "Rust Web mutations require explicit daemon-owned writer mode",
        });
    };
    runtime
        .require_authority_writer(Some(writer_fence.generation), Some(&writer_fence.writer_id))
        .map_err(|error| match error {
            RuntimeError::AuthorityWriterFence(AuthorityWriterFenceError::Required) => {
                ConsoleWriteError {
                    status: ConsoleWriteStatus::ServiceUnavailable,
                    code: "web_console_writer_unavailable",
                    reason: "Rust Web writer ownership is unavailable",
                }
            }
            RuntimeError::AuthorityWriterFence(AuthorityWriterFenceError::Rejected) => {
                ConsoleWriteError {
                    status: ConsoleWriteStatus::Conflict,
                    code: "web_console_writer_standby",
                    reason: "another control-plane writer owns the daemon authority",
                }
            }
            error => map_runtime_error(error),
        })
}

fn stable_identifier(prefix: &str, runtime: &RuntimeProjection, operation: &str) -> String {
    let input = format!(
        "{prefix}\0{}\0{}\0{}",
        runtime.id.as_str(),
        runtime.revision.0,
        operation
    );
    format!("{prefix}-{}", &sha256_hex(input.as_bytes())[..32])
}

fn runtime_matches(runtime: &RuntimeProjection, filter: &RuntimeListFilter) -> bool {
    tag_matches(
        runtime.tags.environment.as_deref(),
        filter.environment.as_deref(),
    ) && tag_matches(runtime.tags.cluster.as_deref(), filter.cluster.as_deref())
        && tag_matches(runtime.tags.role.as_deref(), filter.role.as_deref())
}

fn tag_matches(actual: Option<&str>, expected: Option<&str>) -> bool {
    expected
        .is_none_or(|expected| actual.is_some_and(|actual| actual.eq_ignore_ascii_case(expected)))
}

fn filter_value(filter: &RuntimeListFilter) -> Value {
    json!({
        "environment": filter.environment,
        "cluster": filter.cluster,
        "role": filter.role,
    })
}

fn map_domain_error(error: DomainError) -> ConsoleWriteError {
    map_runtime_error(RuntimeError::Domain(error))
}

fn map_runtime_error(error: RuntimeError) -> ConsoleWriteError {
    match error {
        RuntimeError::Domain(DomainError::RuntimeNotFound { .. }) => ConsoleWriteError {
            status: ConsoleWriteStatus::NotFound,
            code: "runtime_not_found",
            reason: "runtime was not found",
        },
        RuntimeError::Domain(
            DomainError::RevisionConflict { .. } | DomainError::IdempotencyConflict { .. },
        ) => ConsoleWriteError {
            status: ConsoleWriteStatus::Conflict,
            code: "runtime_revision_conflict",
            reason: "runtime authority changed before the mutation committed",
        },
        RuntimeError::Domain(
            DomainError::InvalidIdentifier { .. }
            | DomainError::InvalidQuery { .. }
            | DomainError::ConfirmationRequired,
        )
        | RuntimeError::InvalidPlan(_) => ConsoleWriteError {
            status: ConsoleWriteStatus::BadRequest,
            code: "invalid_web_mutation",
            reason: "Rust Web mutation request was rejected",
        },
        RuntimeError::Storage(_) | RuntimeError::OrchestraDeleteReplayHorizonSaturated => {
            ConsoleWriteError {
                status: ConsoleWriteStatus::ServiceUnavailable,
                code: "web_mutation_persistence_unavailable",
                reason: "durable Rust Web mutation authority is unavailable",
            }
        }
        RuntimeError::AuthorityWriterFence(_) => ConsoleWriteError {
            status: ConsoleWriteStatus::Conflict,
            code: "web_console_writer_standby",
            reason: "another control-plane writer owns the daemon authority",
        },
        _ => ConsoleWriteError {
            status: ConsoleWriteStatus::InternalServerError,
            code: "web_mutation_failed",
            reason: "Rust Web mutation authority failed",
        },
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};
    use std::time::{SystemTime, UNIX_EPOCH};

    use leserpent_adapters::{
        GewyvernTarget, GewyvernTargetCatalog, MutableSecretStore, SecretKey, SecretStore,
        SecretStoreError,
    };
    use leserpent_domain::RuntimeLogLevel;

    use super::*;

    #[derive(Default)]
    struct TestSecretStore {
        values: Mutex<BTreeMap<String, String>>,
    }

    impl TestSecretStore {
        fn keys(&self) -> Vec<String> {
            self.values.lock().unwrap().keys().cloned().collect()
        }
    }

    impl SecretStore for TestSecretStore {
        fn load(&self, key: &SecretKey) -> Result<Option<SecretValue>, SecretStoreError> {
            self.values
                .lock()
                .map_err(|_| SecretStoreError::Unavailable)?
                .get(key.as_str())
                .map(|value| SecretValue::new(value.clone()))
                .transpose()
        }
    }

    impl MutableSecretStore for TestSecretStore {
        fn store_atomic(
            &self,
            key: &SecretKey,
            value: &SecretValue,
        ) -> Result<(), SecretStoreError> {
            self.values
                .lock()
                .map_err(|_| SecretStoreError::Unavailable)?
                .insert(key.as_str().to_string(), value.expose_secret().to_string());
            Ok(())
        }

        fn remove(&self, key: &SecretKey) -> Result<bool, SecretStoreError> {
            Ok(self
                .values
                .lock()
                .map_err(|_| SecretStoreError::Unavailable)?
                .remove(key.as_str())
                .is_some())
        }
    }

    fn temp_database(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "leserpent-web-console-{label}-{}-{nonce}.sqlite",
            std::process::id()
        ))
    }

    #[test]
    fn registration_plan_is_strict_secret_free_and_honestly_transaction_blocked() {
        let body = br#"{
            "name":"Runtime A",
            "endpoint":"HTTPS://Example.INVALID",
            "sidecarEndpoint":null
        }"#;
        let response = execute(
            &ConsoleApiRoute::RegistrationPlan,
            body,
            &mut ControlRuntime::default(),
            None,
        )
        .unwrap();
        let value: Value = serde_json::from_slice(&response).unwrap();
        assert_eq!(value["allowed"], false);
        assert_eq!(value["reason"], REGISTRATION_TRANSACTION_REASON);
        assert_eq!(value["authorityBound"], true);
        assert_eq!(value["plannedRuntimeId"].as_str().unwrap().len(), 32);
        let encoded = String::from_utf8(response).unwrap();
        assert!(!encoded.contains("pairingToken"));
        assert!(!encoded.contains("adminToken"));
        let invalid = execute(
            &ConsoleApiRoute::RegistrationPlan,
            br#"{"name":"A","endpoint":"https://a.invalid","unexpected":true}"#,
            &mut ControlRuntime::default(),
            None,
        )
        .unwrap_err();
        assert_eq!(invalid.status, ConsoleWriteStatus::BadRequest);

        let fragment = execute(
            &ConsoleApiRoute::RegistrationPlan,
            br#"{"name":"A","endpoint":"https://a.invalid/#client-fragment"}"#,
            &mut ControlRuntime::default(),
            None,
        )
        .unwrap_err();
        assert_eq!(fragment.status, ConsoleWriteStatus::BadRequest);
    }

    #[test]
    fn https_registration_plan_binds_reviewed_ca_and_rejects_pem_drift_before_mutation() {
        let path = temp_database("https-registration");
        let mut runtime = ControlRuntime::open(&path).unwrap();
        let writer_id = "adadadadadadadadadadadadadadadad";
        let fence = AuthorityWriterFence {
            generation: runtime
                .claim_authority_writer(writer_id)
                .unwrap()
                .generation,
            writer_id: writer_id.into(),
        };
        let targets = GewyvernTargetCatalog::default();
        let secrets = Arc::new(TestSecretStore::default());
        let mutable_secrets: Arc<dyn MutableSecretStore> = secrets.clone();
        let authority = RuntimeTargetRegistrationAuthority::new(targets.clone(), mutable_secrets);
        let certificate = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
        let ca_pem = certificate.cert.pem();
        let ca_sha256 = sha256_hex(ca_pem.as_bytes());

        let missing_trust: Value = serde_json::from_slice(
            &execute_with_registration(
                &ConsoleApiRoute::RegistrationPlan,
                br#"{"name":"HTTPS Runtime","endpoint":"https://localhost:19443"}"#,
                &mut runtime,
                Some(&fence),
                Some(&authority),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(missing_trust["allowed"], false);
        assert_eq!(missing_trust["reason"], REGISTRATION_CA_REASON);

        let plan_body = serde_json::to_vec(&json!({
            "name": "HTTPS Runtime",
            "endpoint": "https://localhost:19443",
            "sidecarEndpoint": null,
            "tlsCaSha256": ca_sha256,
        }))
        .unwrap();
        let plan: Value = serde_json::from_slice(
            &execute_with_registration(
                &ConsoleApiRoute::RegistrationPlan,
                &plan_body,
                &mut runtime,
                Some(&fence),
                Some(&authority),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(plan["allowed"], true);
        assert_eq!(plan["trustMode"], "pinned_https");
        assert_eq!(plan["tlsCaSha256"], ca_sha256);

        let drifted = serde_json::to_vec(&json!({
            "name": "HTTPS Runtime",
            "endpoint": "https://localhost:19443",
            "pairingToken": "https-pairing-secret",
            "capabilities": [],
            "tags": {},
            "fetchCapabilities": false,
            "sidecarEndpoint": null,
            "sidecarAdminToken": null,
            "tlsCaPem": ca_pem.replace('A', "B"),
            "tlsCaSha256": ca_sha256,
            "registrationPlanToken": plan["planToken"],
        }))
        .unwrap();
        assert_eq!(
            execute_with_registration(
                &ConsoleApiRoute::Registration,
                &drifted,
                &mut runtime,
                Some(&fence),
                Some(&authority),
            )
            .unwrap_err()
            .status,
            ConsoleWriteStatus::BadRequest
        );
        assert!(runtime.runtime_event_state().1.is_empty());
        assert!(runtime.runtime_target_bindings().unwrap().is_empty());
        assert!(secrets.keys().is_empty());

        let request = serde_json::to_vec(&json!({
            "name": "HTTPS Runtime",
            "endpoint": "https://localhost:19443",
            "pairingToken": "https-pairing-secret",
            "capabilities": [],
            "tags": {},
            "fetchCapabilities": false,
            "sidecarEndpoint": null,
            "sidecarAdminToken": null,
            "tlsCaPem": ca_pem,
            "tlsCaSha256": ca_sha256,
            "registrationPlanToken": plan["planToken"],
        }))
        .unwrap();
        let registered: Value = serde_json::from_slice(
            &execute_with_registration(
                &ConsoleApiRoute::Registration,
                &request,
                &mut runtime,
                Some(&fence),
                Some(&authority),
            )
            .unwrap(),
        )
        .unwrap();
        let runtime_id = registered["runtimeId"].as_str().unwrap().to_string();
        assert_eq!(
            targets.endpoint_origins().unwrap(),
            vec![(runtime_id.clone(), "https://localhost:19443".into())]
        );
        drop(runtime);

        let recovered_targets = GewyvernTargetCatalog::default();
        let recovered_authority = RuntimeTargetRegistrationAuthority::new(
            recovered_targets.clone(),
            secrets as Arc<dyn MutableSecretStore>,
        );
        let mut recovered = ControlRuntime::open(&path).unwrap();
        recovered_authority.recover(&mut recovered).unwrap();
        assert_eq!(
            recovered_targets.endpoint_origins().unwrap(),
            vec![(runtime_id, "https://localhost:19443".into())]
        );
        drop(recovered);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn persistence_save_is_writer_fenced_durable_and_restart_recoverable() {
        let path = temp_database("persistence-save");
        let runtime_id = RuntimeId::new("runtime-save").unwrap();
        let mut runtime = ControlRuntime::open(&path).unwrap();
        runtime
            .register_runtime(
                runtime_id.clone(),
                "Saved runtime",
                "https://runtime.invalid",
            )
            .unwrap();

        let disabled =
            execute(&ConsoleApiRoute::PersistenceSave, &[], &mut runtime, None).unwrap_err();
        assert_eq!(disabled.code, "web_console_writer_disabled");

        let writer_id = "cdefcdefcdefcdefcdefcdefcdefcdef";
        let fence = AuthorityWriterFence {
            generation: runtime
                .claim_authority_writer(writer_id)
                .unwrap()
                .generation,
            writer_id: writer_id.into(),
        };
        let response = execute(
            &ConsoleApiRoute::PersistenceSave,
            &[],
            &mut runtime,
            Some(&fence),
        )
        .unwrap();
        let response: Value = serde_json::from_slice(&response).unwrap();
        assert_eq!(response["ok"], true);
        assert_eq!(response["throughSequence"], 1);
        assert!(
            response["savedAt"]
                .as_str()
                .is_some_and(|value| value.ends_with('Z'))
        );

        drop(runtime);
        let recovered = ControlRuntime::open(&path).unwrap();
        assert_eq!(
            recovered.runtime_projection(&runtime_id).unwrap().name,
            "Saved runtime"
        );
        drop(recovered);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn orchestra_compatibility_mutations_are_writer_fenced_and_replay_safe() {
        let path = temp_database("orchestra-compatibility");
        let runtime_id = RuntimeId::new("runtime-orchestra-web").unwrap();
        let mut runtime = ControlRuntime::open(&path).unwrap();
        runtime
            .register_runtime(
                runtime_id.clone(),
                "Orchestra Web runtime",
                "https://runtime.invalid",
            )
            .unwrap();
        let plan: Value = serde_json::from_slice(
            &crate::web_console::render_api(
                &ConsoleApiRoute::OrchestraPlan(runtime_id.clone()),
                &mut runtime,
                false,
            )
            .unwrap(),
        )
        .unwrap();
        let revision = plan["plans"]
            .as_array()
            .unwrap()
            .iter()
            .find(|plan| plan["planId"] == "runtime_triage")
            .unwrap()["revision"]
            .as_str()
            .unwrap()
            .to_string();
        let route = ConsoleApiRoute::OrchestraExecute {
            runtime_id: runtime_id.clone(),
            plan_id: "runtime_triage".into(),
        };
        let body = serde_json::to_vec(&json!({
            "confirmed": true,
            "expectedRevision": revision,
            "approvedBy": "automatic",
            "approvalNote": null,
            "requestId": "request-web-run-0001",
        }))
        .unwrap();
        assert_eq!(
            execute(&route, &body, &mut runtime, None).unwrap_err().code,
            "web_console_writer_disabled"
        );

        let writer_id = "bcbcbcbcbcbcbcbcbcbcbcbcbcbcbcbc";
        let fence = AuthorityWriterFence {
            generation: runtime
                .claim_authority_writer(writer_id)
                .unwrap()
                .generation,
            writer_id: writer_id.into(),
        };
        let started: Value =
            serde_json::from_slice(&execute(&route, &body, &mut runtime, Some(&fence)).unwrap())
                .unwrap();
        let run_id = started["run"]["runId"].as_str().unwrap().to_string();
        assert_eq!(started["replayed"], false);
        let replay: Value =
            serde_json::from_slice(&execute(&route, &body, &mut runtime, Some(&fence)).unwrap())
                .unwrap();
        assert_eq!(replay["replayed"], true);

        let cancelled: Value = serde_json::from_slice(
            &execute(
                &ConsoleApiRoute::OrchestraCancel {
                    runtime_id: runtime_id.clone(),
                    run_id: run_id.clone(),
                },
                &[],
                &mut runtime,
                Some(&fence),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(cancelled["run"]["outcome"], "cancelled");
        let retried: Value = serde_json::from_slice(
            &execute(
                &ConsoleApiRoute::OrchestraRetry {
                    runtime_id: runtime_id.clone(),
                    run_id,
                },
                br#"{"confirmed":true,"approvedBy":"automatic","approvalNote":null,"requestId":"request-web-retry-0001"}"#,
                &mut runtime,
                Some(&fence),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(retried["run"]["attempt"], 2);

        let session: Value = serde_json::from_slice(&execute(
            &ConsoleApiRoute::OrchestraSession(runtime_id),
            br#"{"pipelineKind":"diagnostic","requestedBy":"operator","requestId":"request-web-session-0001"}"#,
            &mut runtime,
            Some(&fence),
        )
        .unwrap()).unwrap();
        assert_eq!(session["session"]["status"], "running");
        assert_eq!(session["run"]["planId"], "session_preparation");
        drop(runtime);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn persistence_import_round_trips_atomically_clears_logs_and_recovers() {
        let path = temp_database("persistence-import");
        let runtime_id = RuntimeId::new("runtime-import").unwrap();
        let mut runtime = ControlRuntime::open(&path).unwrap();
        runtime
            .register_runtime(
                runtime_id.clone(),
                "Imported runtime",
                "https://runtime.invalid",
            )
            .unwrap();
        runtime
            .append_runtime_log(&runtime_id, RuntimeLogLevel::Info, "pre-import log")
            .unwrap();
        let run = br#"{"runId":"orun-import","runtimeId":"runtime-import","planId":"plan-import","outcome":"succeeded","executedAt":"2026-08-28T00:00:00Z","steps":[],"completedAt":"2026-08-28T00:00:01Z","attempt":1,"retriedFromRunId":null,"approvedBy":null,"approvalNote":null,"planRevision":null,"requestId":"request-import"}"#;
        let event = br#"{"eventId":0,"runId":"orun-import","runtimeId":"runtime-import","eventType":"run_succeeded","fromOutcome":null,"toOutcome":"succeeded","summary":"succeeded","recordedAt":"2026-08-28T00:00:01Z"}"#;
        runtime
            .persist_orchestra_run_event_start(
                "orun-import",
                "runtime-import",
                "request-import",
                "run_succeeded",
                "succeeded",
                "2026-08-28T00:00:01Z",
                run,
                event,
            )
            .unwrap();
        runtime
            .create_control_session(
                "request-import-session",
                "session-import",
                &runtime_id,
                "diagnostic",
                "operator",
                Vec::new(),
            )
            .unwrap();
        let previous_revision = runtime.runtime_event_state().0;
        let export =
            crate::web_console::render_api(&ConsoleApiRoute::PersistenceExport, &mut runtime, true)
                .unwrap();
        let exported: Value = serde_json::from_slice(&export).unwrap();
        assert_eq!(exported["runtimes"][0]["capabilitySource"], "manual");
        assert_eq!(exported["sessions"][0]["sessionId"], "session-import");
        assert_eq!(exported["sessions"][0]["status"], "running");
        assert!(
            exported["runtimes"][0]
                .get("hasRuntimeAdminToken")
                .is_none()
        );

        let writer_id = "dededededededededededededededede";
        let fence = AuthorityWriterFence {
            generation: runtime
                .claim_authority_writer(writer_id)
                .unwrap()
                .generation,
            writer_id: writer_id.into(),
        };
        let mut unsafe_import = exported.clone();
        unsafe_import["runtimes"][0]["endpoint"] = json!("javascript:alert(1)");
        let revision_before_rejection = runtime.runtime_event_state().0;
        let error = execute(
            &ConsoleApiRoute::PersistenceImport,
            &serde_json::to_vec(&unsafe_import).unwrap(),
            &mut runtime,
            Some(&fence),
        )
        .unwrap_err();
        assert_eq!(error.code, "invalid_persistence_import");
        assert_eq!(runtime.runtime_event_state().0, revision_before_rejection);
        assert_eq!(
            runtime.runtime_projection(&runtime_id).unwrap().endpoint,
            "https://runtime.invalid"
        );

        let response = execute(
            &ConsoleApiRoute::PersistenceImport,
            &export,
            &mut runtime,
            Some(&fence),
        )
        .unwrap();
        let response: Value = serde_json::from_slice(&response).unwrap();
        assert_eq!(response["ok"], true);
        assert_eq!(response["importedRuntimeCount"], 1);
        assert_eq!(response["importedSessionCount"], 1);
        assert!(runtime.runtime_event_state().0 > previous_revision);
        let log_reader = rusqlite::Connection::open(&path).unwrap();
        assert_eq!(
            log_reader
                .query_row("SELECT COUNT(*) FROM runtime_logs", [], |row| {
                    row.get::<_, i64>(0)
                })
                .unwrap(),
            0
        );
        drop(log_reader);
        assert_eq!(
            runtime
                .load_orchestra_history(Some("runtime-import"), None, 0, 16)
                .unwrap()
                .runs
                .len(),
            1
        );

        drop(runtime);
        let mut recovered = ControlRuntime::open(&path).unwrap();
        assert_eq!(
            recovered.runtime_projection(&runtime_id).unwrap().name,
            "Imported runtime"
        );
        assert_eq!(
            recovered
                .load_orchestra_history(Some("runtime-import"), None, 0, 16)
                .unwrap()
                .runs
                .len(),
            1
        );
        assert_eq!(
            recovered
                .control_session("session-import")
                .unwrap()
                .unwrap()
                .status,
            "running"
        );
        drop(recovered);
        let connection = rusqlite::Connection::open(&path).unwrap();
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM runtime_snapshots", [], |row| {
                    row.get::<_, i64>(0)
                })
                .unwrap(),
            2
        );
        drop(connection);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn persistence_import_rejects_non_quiescent_state_without_mutation() {
        let path = temp_database("persistence-import-quiescence");
        let runtime_id = RuntimeId::new("runtime-import-stable").unwrap();
        let mut runtime = ControlRuntime::open(&path).unwrap();
        runtime
            .register_runtime(
                runtime_id.clone(),
                "Stable runtime",
                "https://stable.invalid",
            )
            .unwrap();
        let export =
            crate::web_console::render_api(&ConsoleApiRoute::PersistenceExport, &mut runtime, true)
                .unwrap();
        let mut replacement: Value = serde_json::from_slice(&export).unwrap();
        replacement["runtimes"][0]["name"] = json!("Replacement runtime");
        let replacement = serde_json::to_vec(&replacement).unwrap();
        runtime
            .enqueue_effect("import-blocker", "test.effect", b"{}", 1)
            .unwrap();
        let writer_id = "efefefefefefefefefefefefefefefef";
        let fence = AuthorityWriterFence {
            generation: runtime
                .claim_authority_writer(writer_id)
                .unwrap()
                .generation,
            writer_id: writer_id.into(),
        };
        let error = execute(
            &ConsoleApiRoute::PersistenceImport,
            &replacement,
            &mut runtime,
            Some(&fence),
        )
        .unwrap_err();
        assert_eq!(error.status, ConsoleWriteStatus::Conflict);
        assert_eq!(error.code, "persistence_import_not_quiescent");
        assert_eq!(
            runtime.runtime_projection(&runtime_id).unwrap().name,
            "Stable runtime"
        );
        assert!(
            runtime
                .latest_snapshot_created_at_unix_ms()
                .unwrap()
                .is_none()
        );
        drop(runtime);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn persistence_import_preserves_static_target_catalog_identity() {
        let path = temp_database("persistence-import-static-target");
        let runtime_id = RuntimeId::new("runtime-import-static").unwrap();
        let mut runtime = ControlRuntime::open(&path).unwrap();
        runtime
            .register_runtime(
                runtime_id.clone(),
                "Static runtime",
                "http://127.0.0.1:19422",
            )
            .unwrap();
        let writer_id = "acacacacacacacacacacacacacacacac";
        let fence = AuthorityWriterFence {
            generation: runtime
                .claim_authority_writer(writer_id)
                .unwrap()
                .generation,
            writer_id: writer_id.into(),
        };
        let targets = GewyvernTargetCatalog::new([(
            runtime_id.as_str().to_string(),
            GewyvernTarget::loopback("127.0.0.1:19422".parse().unwrap(), None).unwrap(),
        )])
        .unwrap();
        let secrets: Arc<dyn MutableSecretStore> = Arc::new(TestSecretStore::default());
        let authority = RuntimeTargetRegistrationAuthority::new(targets, secrets);
        let export =
            crate::web_console::render_api(&ConsoleApiRoute::PersistenceExport, &mut runtime, true)
                .unwrap();
        execute_with_registration(
            &ConsoleApiRoute::PersistenceImport,
            &export,
            &mut runtime,
            Some(&fence),
            Some(&authority),
        )
        .unwrap();

        let mut conflicting: Value = serde_json::from_slice(&export).unwrap();
        conflicting["runtimes"][0]["endpoint"] = json!("http://127.0.0.1:19423/");
        let error = execute_with_registration(
            &ConsoleApiRoute::PersistenceImport,
            &serde_json::to_vec(&conflicting).unwrap(),
            &mut runtime,
            Some(&fence),
            Some(&authority),
        )
        .unwrap_err();
        assert_eq!(error.status, ConsoleWriteStatus::Conflict);
        assert_eq!(
            runtime.runtime_projection(&runtime_id).unwrap().endpoint,
            "http://127.0.0.1:19422"
        );
        drop(runtime);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn writer_registration_plans_commits_rotates_replays_and_retires_without_secret_leakage() {
        let path = temp_database("registration");
        let mut runtime = ControlRuntime::open(&path).unwrap();
        let writer_id = "abababababababababababababababab";
        let generation = runtime
            .claim_authority_writer(writer_id)
            .unwrap()
            .generation;
        let fence = AuthorityWriterFence {
            generation,
            writer_id: writer_id.into(),
        };
        let targets = GewyvernTargetCatalog::default();
        let secrets = Arc::new(TestSecretStore::default());
        let mutable_secrets: Arc<dyn MutableSecretStore> = secrets.clone();
        let authority = RuntimeTargetRegistrationAuthority::new(targets.clone(), mutable_secrets);

        let plan = execute_with_registration(
            &ConsoleApiRoute::RegistrationPlan,
            br#"{"name":"Runtime A","endpoint":"http://127.0.0.1:9411","sidecarEndpoint":null}"#,
            &mut runtime,
            Some(&fence),
            Some(&authority),
        )
        .unwrap();
        let plan: Value = serde_json::from_slice(&plan).unwrap();
        assert_eq!(plan["allowed"], true);
        assert_eq!(plan["action"], "create");
        assert!(plan["reason"].is_null());
        let plan_token = plan["planToken"].as_str().unwrap().to_string();
        let registration_body = serde_json::to_vec(&json!({
            "name": "Runtime A",
            "endpoint": "http://127.0.0.1:9411",
            "pairingToken": "first-pairing-secret",
            "capabilities": [],
            "tags": { "environment": "test", "cluster": null, "role": null },
            "fetchCapabilities": false,
            "sidecarEndpoint": null,
            "sidecarAdminToken": null,
            "registrationPlanToken": plan_token,
        }))
        .unwrap();
        let registered = execute_with_registration(
            &ConsoleApiRoute::Registration,
            &registration_body,
            &mut runtime,
            Some(&fence),
            Some(&authority),
        )
        .unwrap();
        let registered_value: Value = serde_json::from_slice(&registered).unwrap();
        assert_eq!(registered_value["registrationReplayed"], false);
        let runtime_id = RuntimeId::new(registered_value["runtimeId"].as_str().unwrap()).unwrap();
        assert!(targets.contains(runtime_id.as_str()).unwrap());
        assert_eq!(secrets.keys().len(), 1);
        let registered_text = String::from_utf8(registered).unwrap();
        assert!(!registered_text.contains("first-pairing-secret"));
        let binding = runtime.runtime_target_bindings().unwrap().pop().unwrap();
        assert!(
            !binding
                .payload
                .windows("first-pairing-secret".len())
                .any(|window| window == b"first-pairing-secret")
        );

        let replay = execute_with_registration(
            &ConsoleApiRoute::Registration,
            &registration_body,
            &mut runtime,
            Some(&fence),
            Some(&authority),
        )
        .unwrap();
        let replay: Value = serde_json::from_slice(&replay).unwrap();
        assert_eq!(replay["registrationReplayed"], true);
        let wrong_secret_body = serde_json::to_vec(&json!({
            "name": "Runtime A",
            "endpoint": "http://127.0.0.1:9411",
            "pairingToken": "wrong-pairing-secret",
            "capabilities": [],
            "tags": { "environment": "test", "cluster": null, "role": null },
            "fetchCapabilities": false,
            "sidecarEndpoint": null,
            "sidecarAdminToken": null,
            "registrationPlanToken": plan_token,
        }))
        .unwrap();
        assert_eq!(
            execute_with_registration(
                &ConsoleApiRoute::Registration,
                &wrong_secret_body,
                &mut runtime,
                Some(&fence),
                Some(&authority),
            )
            .unwrap_err()
            .status,
            ConsoleWriteStatus::Conflict
        );

        let update_plan = execute_with_registration(
            &ConsoleApiRoute::RegistrationPlan,
            br#"{"name":"Runtime A","endpoint":"http://127.0.0.1:9412","sidecarEndpoint":null}"#,
            &mut runtime,
            Some(&fence),
            Some(&authority),
        )
        .unwrap();
        let update_plan: Value = serde_json::from_slice(&update_plan).unwrap();
        assert_eq!(update_plan["action"], "update");
        let update_body = serde_json::to_vec(&json!({
            "name": "Runtime A",
            "endpoint": "http://127.0.0.1:9412",
            "pairingToken": "rotated-pairing-secret",
            "capabilities": [],
            "tags": { "environment": "test", "cluster": null, "role": null },
            "fetchCapabilities": false,
            "sidecarEndpoint": null,
            "sidecarAdminToken": null,
            "registrationPlanToken": update_plan["planToken"],
        }))
        .unwrap();
        execute_with_registration(
            &ConsoleApiRoute::Registration,
            &update_body,
            &mut runtime,
            Some(&fence),
            Some(&authority),
        )
        .unwrap();
        assert_eq!(secrets.keys().len(), 1);
        assert_eq!(runtime.runtime_target_bindings().unwrap().len(), 1);
        assert_eq!(
            runtime.runtime_projection(&runtime_id).unwrap().endpoint,
            "http://127.0.0.1:9412/"
        );

        let export =
            crate::web_console::render_api(&ConsoleApiRoute::PersistenceExport, &mut runtime, true)
                .unwrap();
        execute_with_registration(
            &ConsoleApiRoute::PersistenceImport,
            &export,
            &mut runtime,
            Some(&fence),
            Some(&authority),
        )
        .unwrap();
        assert!(targets.contains(runtime_id.as_str()).unwrap());
        assert_eq!(secrets.keys().len(), 1);
        let mut conflicting_import: Value = serde_json::from_slice(&export).unwrap();
        conflicting_import["runtimes"][0]["endpoint"] = json!("http://127.0.0.1:9555/");
        let conflicting_import = serde_json::to_vec(&conflicting_import).unwrap();
        let error = execute_with_registration(
            &ConsoleApiRoute::PersistenceImport,
            &conflicting_import,
            &mut runtime,
            Some(&fence),
            Some(&authority),
        )
        .unwrap_err();
        assert_eq!(error.status, ConsoleWriteStatus::Conflict);
        assert_eq!(
            runtime.runtime_projection(&runtime_id).unwrap().endpoint,
            "http://127.0.0.1:9412/"
        );

        execute_with_registration(
            &ConsoleApiRoute::RuntimeDelete(runtime_id.clone()),
            &[],
            &mut runtime,
            Some(&fence),
            Some(&authority),
        )
        .unwrap();
        assert!(!targets.contains(runtime_id.as_str()).unwrap());
        assert!(secrets.keys().is_empty());
        assert!(runtime.runtime_target_bindings().unwrap().is_empty());
        drop(runtime);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn writer_fenced_refresh_and_delete_use_durable_runtime_authority() {
        let path = temp_database("mutation");
        let mut runtime = ControlRuntime::open(&path).unwrap();
        let runtime_id = RuntimeId::new("runtime-web-write").unwrap();
        runtime
            .register_runtime(
                runtime_id.clone(),
                "Web write runtime",
                "https://runtime.invalid",
            )
            .unwrap();
        let writer_id = "0123456789abcdef0123456789abcdef";
        let generation = runtime
            .claim_authority_writer(writer_id)
            .unwrap()
            .generation;
        let fence = AuthorityWriterFence {
            generation,
            writer_id: writer_id.into(),
        };

        let disabled = execute(
            &ConsoleApiRoute::FleetRefreshStatus(RuntimeListFilter::default()),
            &[],
            &mut runtime,
            None,
        )
        .unwrap_err();
        assert_eq!(disabled.code, "web_console_writer_disabled");

        let refreshed = execute(
            &ConsoleApiRoute::FleetRefreshStatus(RuntimeListFilter::default()),
            &[],
            &mut runtime,
            Some(&fence),
        )
        .unwrap();
        let refreshed: Value = serde_json::from_slice(&refreshed).unwrap();
        assert_eq!(refreshed["refresh"]["refreshedCount"], 1);
        assert_eq!(
            runtime
                .runtime_projection(&runtime_id)
                .unwrap()
                .refresh_count,
            1
        );

        let deleted = execute(
            &ConsoleApiRoute::RuntimeDelete(runtime_id.clone()),
            &[],
            &mut runtime,
            Some(&fence),
        )
        .unwrap();
        let deleted: Value = serde_json::from_slice(&deleted).unwrap();
        assert_eq!(deleted["deleted"], true);
        assert!(runtime.runtime_projection(&runtime_id).is_none());

        drop(runtime);
        let recovered = ControlRuntime::open(&path).unwrap();
        assert!(recovered.runtime_projection(&runtime_id).is_none());
        drop(recovered);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn cleanup_is_plan_fenced_challenged_atomic_and_restart_replayable() {
        let path = temp_database("cleanup");
        let mut runtime = ControlRuntime::open(&path).unwrap();
        for suffix in ["a", "b"] {
            runtime
                .register_runtime(
                    RuntimeId::new(format!("runtime-cleanup-{suffix}")).unwrap(),
                    format!("Cleanup {suffix}"),
                    format!("https://cleanup-{suffix}.invalid"),
                )
                .unwrap();
        }
        let cleanup_runtime = RuntimeId::new("runtime-cleanup-a").unwrap();
        runtime
            .create_control_session(
                "request-cleanup-session-0001",
                "session-cleanup-a",
                &cleanup_runtime,
                "diagnostic",
                "operator",
                Vec::new(),
            )
            .unwrap();
        let writer_a = "11111111111111111111111111111111";
        let generation = runtime.claim_authority_writer(writer_a).unwrap().generation;
        let fence = AuthorityWriterFence {
            generation,
            writer_id: writer_a.into(),
        };
        let filter = RuntimeListFilter::default();
        let invalid = execute(
            &ConsoleApiRoute::RuntimeCleanup(CleanupKind::Unobserved, filter.clone()),
            br#"{"planToken":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","unexpected":true}"#,
            &mut runtime,
            Some(&fence),
        )
        .unwrap_err();
        assert_eq!(invalid.status, ConsoleWriteStatus::BadRequest);
        let (_, initial_runtimes) = runtime.runtime_event_state();
        let initial_sessions = runtime.list_control_sessions().unwrap();
        let initial_plan =
            build_cleanup_plan_with_sessions(&filter, &initial_runtimes, &initial_sessions);
        let stale_token = initial_plan
            .action(CleanupKind::Unobserved)
            .plan_token
            .clone();

        runtime
            .register_runtime(
                RuntimeId::new("runtime-cleanup-c").unwrap(),
                "Cleanup c",
                "https://cleanup-c.invalid",
            )
            .unwrap();
        let stale_body = serde_json::to_vec(&json!({ "planToken": stale_token })).unwrap();
        let stale = execute(
            &ConsoleApiRoute::RuntimeCleanup(CleanupKind::Unobserved, filter.clone()),
            &stale_body,
            &mut runtime,
            Some(&fence),
        )
        .unwrap_err();
        assert_eq!(stale.code, "runtime_cleanup_plan_changed");
        assert_eq!(runtime.runtime_event_state().1.len(), 3);

        let (_, current_runtimes) = runtime.runtime_event_state();
        let current_sessions = runtime.list_control_sessions().unwrap();
        let current_plan =
            build_cleanup_plan_with_sessions(&filter, &current_runtimes, &current_sessions);
        let slice = current_plan.action(CleanupKind::Slice);
        assert_eq!(slice.affected_session_ids, ["session-cleanup-a"]);
        let missing_challenge = serde_json::to_vec(&json!({
            "planToken": slice.plan_token,
        }))
        .unwrap();
        let rejected = execute(
            &ConsoleApiRoute::RuntimeCleanup(CleanupKind::Slice, filter.clone()),
            &missing_challenge,
            &mut runtime,
            Some(&fence),
        )
        .unwrap_err();
        assert_eq!(rejected.code, "runtime_cleanup_plan_changed");
        assert_eq!(runtime.runtime_event_state().1.len(), 3);

        let cleanup = current_plan.action(CleanupKind::Unobserved);
        let cleanup_body = serde_json::to_vec(&json!({
            "planToken": cleanup.plan_token,
        }))
        .unwrap();
        let response = execute(
            &ConsoleApiRoute::RuntimeCleanup(CleanupKind::Unobserved, filter.clone()),
            &cleanup_body,
            &mut runtime,
            Some(&fence),
        )
        .unwrap();
        let response: Value = serde_json::from_slice(&response).unwrap();
        assert_eq!(response["removedRuntimeCount"], 3);
        assert_eq!(response["removedSessionCount"], 1);
        assert_eq!(response["replayed"], false);
        assert!(runtime.runtime_event_state().1.is_empty());
        assert!(runtime.list_control_sessions().unwrap().is_empty());

        drop(runtime);
        let mut recovered = ControlRuntime::open(&path).unwrap();
        let writer_b = "22222222222222222222222222222222";
        let generation = recovered
            .claim_authority_writer(writer_b)
            .unwrap()
            .generation;
        let recovered_fence = AuthorityWriterFence {
            generation,
            writer_id: writer_b.into(),
        };
        let replay = execute(
            &ConsoleApiRoute::RuntimeCleanup(CleanupKind::Unobserved, filter),
            &cleanup_body,
            &mut recovered,
            Some(&recovered_fence),
        )
        .unwrap();
        let replay: Value = serde_json::from_slice(&replay).unwrap();
        assert_eq!(replay["removedRuntimeCount"], 3);
        assert_eq!(replay["removedSessionCount"], 1);
        assert_eq!(replay["replayed"], true);
        assert!(recovered.runtime_event_state().1.is_empty());
        assert!(recovered.list_control_sessions().unwrap().is_empty());
        drop(recovered);
        fs::remove_file(path).unwrap();
    }
}
