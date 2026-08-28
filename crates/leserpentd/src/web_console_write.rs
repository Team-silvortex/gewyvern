use leserpent_adapters::{MAX_SECRET_BYTES, SecretValue};
use leserpent_domain::{
    CAPABILITY_RUNTIME_REFRESH, COMMAND_PLAN_SCHEMA_VERSION, CapabilitySet, Command,
    CommandEnvelope, CommandId, CommandOrigin, CommandPlan, Confirmation, DomainError,
    IdempotencyKey, PlannedOperation, Principal, Revision, RuntimeId, RuntimeListFilter,
    RuntimeProjection, RuntimeTags, validate_registration_intent,
};
use leserpent_protocol::{AuthorityWriterFence, MAX_PROTOCOL_MESSAGE_BYTES};
use leserpent_runtime::{
    AuthorityWriterFenceError, ControlRuntime, PlanResult, RuntimeError, RuntimeUnregisterTarget,
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::runtime_target_registration::{
    RuntimeTargetDescriptor, RuntimeTargetRegistrationAction, RuntimeTargetRegistrationAuthority,
    RuntimeTargetRegistrationError, RuntimeTargetRegistrationErrorKind,
    RuntimeTargetRegistrationIntent, loopback_address,
};
use crate::web_console::{
    CleanupKind, ConsoleApiRoute, MAX_ATOMIC_CLEANUP_TARGETS, MAX_CLEANUP_REQUEST_BYTES,
    MAX_REGISTRATION_PLAN_BYTES, MAX_REGISTRATION_REQUEST_BYTES, build_cleanup_plan, runtime_value,
    sha256_hex,
};

const REGISTRATION_TRANSACTION_REASON: &str = "rust_web_registration_transaction_unavailable";
const REGISTRATION_LOOPBACK_REASON: &str = "loopback_gewyvern_target_required";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ConsoleWriteStatus {
    BadRequest,
    NotFound,
    Conflict,
    ServiceUnavailable,
    InternalServerError,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ConsoleWriteError {
    pub(crate) status: ConsoleWriteStatus,
    pub(crate) code: &'static str,
    pub(crate) reason: &'static str,
}

impl ConsoleWriteError {
    pub(crate) fn body(&self) -> Vec<u8> {
        serde_json::to_vec(&json!({
            "error": self.code,
            "reason": self.reason,
        }))
        .expect("fixed Rust Web error response must serialize")
    }
}

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
    registration_plan_token: String,
}

#[derive(Clone, Debug)]
struct RegistrationCoordinates {
    name: String,
    endpoint: String,
    sidecar_endpoint: Option<String>,
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
            "native Rust registration currently accepts an explicit loopback HTTP Gewyvern origin"
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
        .map_err(map_domain_error)?,
    };
    let expected_revision = existing.as_ref().map(|runtime| runtime.revision);
    let (action, reason) = if endpoint_conflict {
        (None, Some("endpoint_conflict"))
    } else if !registration_available {
        (None, Some(REGISTRATION_TRANSACTION_REASON))
    } else if loopback_address(&coordinates.endpoint).is_err() {
        (None, Some(REGISTRATION_LOOPBACK_REASON))
    } else if existing.is_some() {
        (Some(RuntimeTargetRegistrationAction::Update), None)
    } else {
        (Some(RuntimeTargetRegistrationAction::Create), None)
    };
    let plan_token = registration_plan_token(
        &coordinates.name,
        &coordinates.endpoint,
        coordinates.sidecar_endpoint.as_deref(),
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
            RuntimeTargetRegistrationIntent::new(
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
    tags: &RuntimeTags,
) -> Result<RegistrationCoordinates, ConsoleWriteError> {
    let name = name.trim().to_string();
    let endpoint = normalize_http_endpoint(&endpoint).ok_or_else(invalid_registration_plan)?;
    let sidecar_endpoint = sidecar_endpoint
        .filter(|endpoint| !endpoint.trim().is_empty())
        .map(|endpoint| normalize_http_endpoint(&endpoint).ok_or_else(invalid_registration_plan))
        .transpose()?;
    validate_registration_intent(&name, &endpoint, sidecar_endpoint.as_deref(), tags)
        .map_err(|_| invalid_registration_plan())?;
    Ok(RegistrationCoordinates {
        name,
        endpoint,
        sidecar_endpoint,
    })
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

fn normalize_http_endpoint(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > 2048
        || value
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
    {
        return None;
    }
    let (scheme, remainder) = value.split_once("://")?;
    if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
        return None;
    }
    let authority_end = remainder.find(['/', '?', '#']).unwrap_or(remainder.len());
    let authority = &remainder[..authority_end];
    if authority.is_empty() || authority.contains('@') {
        return None;
    }
    let suffix = &remainder[authority_end..];
    if suffix.contains('#') {
        return None;
    }
    let suffix = if suffix.is_empty() { "/" } else { suffix };
    Some(format!(
        "{}://{}{}",
        scheme.to_ascii_lowercase(),
        authority.to_ascii_lowercase(),
        suffix
    ))
}

fn proposed_runtime_id(name: &str, endpoint: &str) -> String {
    let input = format!("{}\0{}", name.to_ascii_lowercase(), endpoint);
    sha256_hex(input.as_bytes())[..32].to_string()
}

fn registration_plan_token(
    name: &str,
    endpoint: &str,
    sidecar_endpoint: Option<&str>,
    planned_runtime_id: &RuntimeId,
    expected_revision: Option<Revision>,
    action: &str,
) -> String {
    let input = format!(
        "runtime-registration-plan-v2\n{}\n{}\n{}\ndaemon\n{}\n{}\n{}",
        name.to_ascii_lowercase(),
        endpoint,
        sidecar_endpoint.unwrap_or_default(),
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
        CommandId::new(format!("web-cleanup-{}", &request.plan_token)).map_err(map_domain_error)?;

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
            "removedSessionCount": 0,
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
    let plan = build_cleanup_plan(filter, &runtimes);
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
        "removedSessionCount": 0,
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
        "removedSessionCount": 0,
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
        GewyvernTargetCatalog, MutableSecretStore, SecretKey, SecretStore, SecretStoreError,
    };

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
        let initial_plan = build_cleanup_plan(&filter, &initial_runtimes);
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
        let current_plan = build_cleanup_plan(&filter, &current_runtimes);
        let slice = current_plan.action(CleanupKind::Slice);
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
        assert_eq!(response["replayed"], false);
        assert!(runtime.runtime_event_state().1.is_empty());

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
        assert_eq!(replay["replayed"], true);
        assert!(recovered.runtime_event_state().1.is_empty());
        drop(recovered);
        fs::remove_file(path).unwrap();
    }
}
