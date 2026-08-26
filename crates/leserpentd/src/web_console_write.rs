use leserpent_domain::{
    CAPABILITY_RUNTIME_REFRESH, COMMAND_PLAN_SCHEMA_VERSION, CapabilitySet, Command,
    CommandEnvelope, CommandId, CommandOrigin, CommandPlan, Confirmation, DomainError,
    IdempotencyKey, PlannedOperation, Principal, RuntimeId, RuntimeListFilter, RuntimeProjection,
    RuntimeTags, validate_registration_intent,
};
use leserpent_protocol::{AuthorityWriterFence, MAX_PROTOCOL_MESSAGE_BYTES};
use leserpent_runtime::{
    AuthorityWriterFenceError, ControlRuntime, PlanResult, RuntimeError, RuntimeUnregisterTarget,
};
use ring::digest;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::web_console::{ConsoleApiRoute, MAX_REGISTRATION_PLAN_BYTES};

const REGISTRATION_SECRET_STORE_REASON: &str = "rust_web_registration_secret_store_unavailable";

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

pub(crate) fn execute(
    route: &ConsoleApiRoute,
    body: &[u8],
    runtime: &mut ControlRuntime,
    writer_fence: Option<&AuthorityWriterFence>,
) -> Result<Vec<u8>, ConsoleWriteError> {
    let value = match route {
        ConsoleApiRoute::RegistrationPlan => registration_plan(body, runtime)?,
        ConsoleApiRoute::FleetRefreshAll(filter) => {
            refresh_fleet(runtime, writer_fence, filter, RefreshKind::All)?
        }
        ConsoleApiRoute::FleetRefreshCapabilities(filter) => {
            refresh_fleet(runtime, writer_fence, filter, RefreshKind::Capabilities)?
        }
        ConsoleApiRoute::FleetRefreshStatus(filter) => {
            refresh_fleet(runtime, writer_fence, filter, RefreshKind::Status)?
        }
        ConsoleApiRoute::RuntimeDelete(runtime_id) => {
            delete_runtime(runtime, writer_fence, runtime_id)?
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

fn registration_plan(body: &[u8], runtime: &ControlRuntime) -> Result<Value, ConsoleWriteError> {
    if body.is_empty() || body.len() > MAX_REGISTRATION_PLAN_BYTES {
        return Err(invalid_registration_plan());
    }
    let request: RegistrationPlanRequest =
        serde_json::from_slice(body).map_err(|_| invalid_registration_plan())?;
    let name = request.name.trim().to_string();
    let endpoint =
        normalize_http_endpoint(&request.endpoint).ok_or_else(invalid_registration_plan)?;
    let sidecar_endpoint = match request.sidecar_endpoint.as_deref() {
        Some(endpoint) => {
            Some(normalize_http_endpoint(endpoint).ok_or_else(invalid_registration_plan)?)
        }
        None => None,
    };
    validate_registration_intent(
        &name,
        &endpoint,
        sidecar_endpoint.as_deref(),
        &RuntimeTags::default(),
    )
    .map_err(|_| invalid_registration_plan())?;

    let (_, runtimes) = runtime.runtime_event_state();
    let same_name = runtimes
        .iter()
        .find(|runtime| runtime.name.eq_ignore_ascii_case(&name));
    let same_endpoint = runtimes.iter().find(|runtime| {
        normalize_http_endpoint(&runtime.endpoint).is_some_and(|candidate| candidate == endpoint)
    });
    let endpoint_conflict = same_endpoint.is_some_and(|endpoint_owner| {
        same_name.is_none_or(|name_owner| endpoint_owner.id != name_owner.id)
    });
    let existing = if endpoint_conflict {
        same_endpoint
    } else {
        same_name
    };
    let planned_runtime_id = existing
        .map(|runtime| runtime.id.as_str().to_string())
        .unwrap_or_else(|| proposed_runtime_id(&name, &endpoint));
    let expected_revision = existing.map(|runtime| runtime.revision.0);
    let reason = if endpoint_conflict {
        "endpoint_conflict"
    } else {
        REGISTRATION_SECRET_STORE_REASON
    };
    let plan_token = registration_plan_token(
        &name,
        &endpoint,
        sidecar_endpoint.as_deref(),
        &planned_runtime_id,
        expected_revision,
        reason,
    );
    Ok(json!({
        "allowed": false,
        "action": "reject",
        "reason": reason,
        "reasonMessage": if endpoint_conflict {
            "runtime endpoint is already registered to another runtime"
        } else {
            "native Rust registration is waiting for an atomic platform secret-store write contract"
        },
        "existingRuntimeId": existing.map(|runtime| runtime.id.as_str()),
        "existingRuntimeName": existing.map(|runtime| runtime.name.as_str()),
        "existingRuntimeEndpoint": existing.map(|runtime| runtime.endpoint.as_str()),
        "plannedRuntimeId": planned_runtime_id,
        "expectedRevision": expected_revision,
        "authorityBound": true,
        "planToken": plan_token,
    }))
}

fn invalid_registration_plan() -> ConsoleWriteError {
    ConsoleWriteError {
        status: ConsoleWriteStatus::BadRequest,
        code: "invalid_runtime_registration_plan",
        reason: "registration plan requires bounded name and HTTP(S) endpoint fields",
    }
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
    hex_digest(input.as_bytes())[..32].to_string()
}

fn registration_plan_token(
    name: &str,
    endpoint: &str,
    sidecar_endpoint: Option<&str>,
    planned_runtime_id: &str,
    expected_revision: Option<u64>,
    reason: &str,
) -> String {
    let input = format!(
        "runtime-registration-plan-rust-v1\n{}\n{}\n{}\ndaemon\nreject\n{}\n{}\n{}",
        name.to_ascii_lowercase(),
        endpoint,
        sidecar_endpoint.unwrap_or_default(),
        planned_runtime_id.to_ascii_lowercase(),
        expected_revision.map_or_else(String::new, |revision| revision.to_string()),
        reason,
    );
    hex_digest(input.as_bytes())
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

fn delete_runtime(
    runtime: &mut ControlRuntime,
    writer_fence: Option<&AuthorityWriterFence>,
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
    format!("{prefix}-{}", &hex_digest(input.as_bytes())[..32])
}

fn hex_digest(bytes: &[u8]) -> String {
    let digest = digest::digest(&digest::SHA256, bytes);
    let mut encoded = String::with_capacity(digest.as_ref().len() * 2);
    for byte in digest.as_ref() {
        use std::fmt::Write as _;
        write!(&mut encoded, "{byte:02x}").expect("writing to a String cannot fail");
    }
    encoded
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
            "leserpent-web-console-{label}-{}-{nonce}.sqlite",
            std::process::id()
        ))
    }

    #[test]
    fn registration_plan_is_strict_secret_free_and_honestly_blocked() {
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
        assert_eq!(value["reason"], REGISTRATION_SECRET_STORE_REASON);
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
}
