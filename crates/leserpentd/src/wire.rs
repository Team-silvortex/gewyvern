use leserpent_domain::bootstrap::{
    CAPABILITY_HOST_BOOTSTRAP, DaemonSessionProof, DeploymentBootstrapCheckpoint,
};
use leserpent_domain::{
    CAPABILITY_ORCHESTRA_WRITE, CAPABILITY_RUNTIME_DEPLOY, CAPABILITY_RUNTIME_READ,
    CAPABILITY_RUNTIME_REFRESH, CAPABILITY_RUNTIME_UNREGISTER, Command, CommandPlan,
    IdempotencyKey, PlannedOperation, Query,
};
use leserpent_protocol::{
    AuthorityWriterClaimResponse, AuthorityWriterFence, CAPABILITY_AUTHORITY_WRITER,
    DeploymentReceiptResponse, DeploymentReceiptStatus, EffectQueueHealth, HealthResponse,
    OrchestraDeleteReceiptResponse, OrchestraDeleteReplayAdmissionPressure,
    OrchestraDeleteReplayAdmissionState, OrchestraDeleteReplayHorizonResponse,
    OrchestraDeleteReplayOperatorAction, OrchestraDeleteResponse, OrchestraHistoryResponse,
    OrchestraPersistenceResponse, PROTOCOL_SCHEMA_VERSION, ProtocolError, ProtocolRequest,
    ProtocolResponse, RequestEnvelope, ResponseEnvelope, RuntimeUnregisterResponse,
    RuntimeUnregisterTarget, RuntimeUnregistrationReceipt,
    RuntimeUnregistrationReceiptLookupResponse, RuntimeUnregistrationReplayHorizonHealth,
};
use leserpent_runtime::{
    ControlRuntime, DeploymentEffectState, PlanResult, RuntimeError,
    RuntimeUnregisterTarget as RuntimeTarget,
};

pub(crate) const MAX_AUTH_TOKEN_BYTES: usize = 256;

pub trait BootstrapSessionVerifier: Send + Sync {
    fn prove_session(
        &self,
        checkpoint: &DeploymentBootstrapCheckpoint,
    ) -> Result<DaemonSessionProof, String>;
}

pub(crate) fn execute_request(
    runtime: &mut ControlRuntime,
    request: RequestEnvelope,
    bootstrap_verifier: Option<&dyn BootstrapSessionVerifier>,
    writer_fence: Option<&AuthorityWriterFence>,
    authority_writer_claim_enabled: bool,
) -> ResponseEnvelope {
    let request = request.request;
    if let ProtocolRequest::AuthorityWriterClaim(claim) = &request {
        if !authority_writer_claim_enabled {
            return error_response(
                "authority_writer_claim_unavailable",
                "authority writer claims require the private local transport",
            );
        }
        if claim.principal.id.trim().is_empty()
            || !claim.capabilities.contains(CAPABILITY_AUTHORITY_WRITER)
        {
            return error_response(
                "authority_writer_claim_rejected",
                "authority writer claim requires explicit authority",
            );
        }
        return match runtime.claim_authority_writer(&claim.writer_id) {
            Ok(claim) => response(ProtocolResponse::AuthorityWriterClaimed(
                AuthorityWriterClaimResponse {
                    generation: claim.generation,
                    writer_id: claim.writer_id,
                    replayed: claim.replayed,
                },
            )),
            Err(_) => error_response(
                "authority_writer_claim_failed",
                "authority writer claim failed",
            ),
        };
    }
    if requires_authority_writer_fence(&request) {
        let fenced = runtime.require_authority_writer(
            writer_fence.map(|fence| fence.generation),
            writer_fence.map(|fence| fence.writer_id.as_str()),
        );
        if let Err(error) = fenced {
            return authority_writer_fence_error(error);
        }
    }
    let request = match request {
        ProtocolRequest::Health(_) => {
            return match runtime.heartbeat().and_then(|()| {
                let queue = runtime.effect_queue_stats()?;
                let unregistration_horizon = runtime.runtime_unregistration_replay_horizon()?;
                let orchestra_horizon = runtime.orchestra_delete_replay_horizon()?;
                Ok((queue, unregistration_horizon, orchestra_horizon))
            }) {
                Ok((queue, replay_horizon, orchestra_horizon)) => {
                    response(ProtocolResponse::Health(HealthResponse {
                        status: "ready".into(),
                        authority_owned: true,
                        protocol_schema_version: PROTOCOL_SCHEMA_VERSION,
                        effect_queue: Some(EffectQueueHealth {
                            ready: queue.ready,
                            leased: queue.leased,
                            completed: queue.completed,
                            failed: queue.failed,
                            active: queue.active(),
                            terminal: queue.terminal(),
                            capacity: queue.capacity,
                            saturated: queue.saturated(),
                        }),
                        runtime_unregistration_replay_horizon: Some(
                            RuntimeUnregistrationReplayHorizonHealth {
                                capacity: replay_horizon.capacity,
                                retained: replay_horizon.retained,
                                oldest_generation: replay_horizon.oldest_generation,
                                newest_generation: replay_horizon.newest_generation,
                                next_generation: replay_horizon.next_generation,
                                evicted_through_generation: replay_horizon
                                    .evicted_through_generation,
                            },
                        ),
                        orchestra_delete_replay_horizon: Some(
                            orchestra_delete_replay_horizon_response(orchestra_horizon),
                        ),
                    }))
                }
                Err(_) => error_response("runtime_unavailable", "runtime authority is unavailable"),
            };
        }
        ProtocolRequest::DeploymentReceipt(receipt) => {
            if receipt.principal.id.trim().is_empty()
                || !receipt.capabilities.contains(CAPABILITY_RUNTIME_DEPLOY)
                || IdempotencyKey::new(receipt.request_id.clone()).is_err()
            {
                return error_response("unauthorized", "deployment receipt access was rejected");
            }
            return match runtime
                .deployment_effect_receipt(receipt.command_id.as_str(), &receipt.request_id)
            {
                Ok(Some(result)) => response(ProtocolResponse::DeploymentReceipt(
                    DeploymentReceiptResponse {
                        command_id: receipt.command_id,
                        request_id: receipt.request_id,
                        status: match result.state {
                            DeploymentEffectState::Pending => DeploymentReceiptStatus::Pending,
                            DeploymentEffectState::Completed => DeploymentReceiptStatus::Completed,
                            DeploymentEffectState::Failed => DeploymentReceiptStatus::Failed,
                        },
                        attempt: result.attempt,
                        outcome: result.outcome,
                        error: result.error,
                    },
                )),
                Ok(None) => error_response(
                    "deployment_receipt_not_found",
                    "deployment receipt was not found",
                ),
                Err(RuntimeError::InvalidEffectOutcome(_)) => error_response(
                    "invalid_request",
                    "deployment receipt identity was rejected",
                ),
                Err(_) => error_response("runtime_failed", "deployment receipt lookup failed"),
            };
        }
        ProtocolRequest::OrchestraPersist(request) => {
            if request.principal.id.trim().is_empty()
                || !request.capabilities.contains(CAPABILITY_ORCHESTRA_WRITE)
                || leserpent_protocol::compatibility_v1::validate_orchestra_persistence(
                    &request.envelope,
                )
                .is_err()
            {
                return error_response("invalid_request", "Orchestra persistence was rejected");
            }
            let run = match serde_json::to_vec(&request.envelope.run) {
                Ok(run) => run,
                Err(_) => {
                    return error_response("invalid_request", "Orchestra run could not be encoded");
                }
            };
            let event = match serde_json::to_vec(&request.envelope.event) {
                Ok(event) => event,
                Err(_) => {
                    return error_response(
                        "invalid_request",
                        "Orchestra event could not be encoded",
                    );
                }
            };
            return match runtime.persist_orchestra_run_event(
                &request.envelope.run.run_id,
                &request.envelope.run.runtime_id,
                request.envelope.run.request_id.as_deref(),
                &request.envelope.event.event_type,
                request.envelope.event.from_outcome.as_deref(),
                &request.envelope.event.to_outcome,
                &request.envelope.run.outcome,
                &request.envelope.event.recorded_at,
                &run,
                &event,
            ) {
                Ok(receipt) => {
                    let run = serde_json::from_slice(&receipt.run);
                    let event = serde_json::from_slice(&receipt.event);
                    match (run, event) {
                        (Ok(run), Ok(event)) => response(ProtocolResponse::OrchestraPersisted(
                            OrchestraPersistenceResponse {
                                envelope:
                                    leserpent_protocol::compatibility_v1::LegacyOrchestraPersistenceEnvelope {
                                        run,
                                        event,
                                    },
                                event_count: receipt.event_count,
                            },
                        )),
                        _ => error_response(
                            "runtime_failed",
                            "Orchestra persistence read-back was invalid",
                        ),
                    }
                }
                Err(RuntimeError::Storage(_)) => error_response(
                    "orchestra_persistence_failed",
                    "Orchestra persistence transaction failed",
                ),
                Err(_) => {
                    error_response("runtime_failed", "Orchestra persistence authority failed")
                }
            };
        }
        ProtocolRequest::OrchestraHistory(request) => {
            if request.principal.id.trim().is_empty() {
                return error_response("invalid_principal", "principal must not be blank");
            }
            if !request.capabilities.contains(CAPABILITY_ORCHESTRA_WRITE) {
                return error_response("capability_denied", "missing capability 'orchestra.write'");
            }
            let history = match runtime.load_orchestra_history(
                request.runtime_id.as_deref(),
                request.run_id.as_deref(),
                request.offset,
                request.limit,
            ) {
                Ok(history) => history,
                Err(_) => {
                    return error_response(
                        "orchestra_history_failed",
                        "Orchestra history query failed",
                    );
                }
            };
            let runs = history
                .runs
                .iter()
                .map(|bytes| serde_json::from_slice(bytes))
                .collect::<Result<Vec<_>, _>>();
            let events = history
                .events
                .iter()
                .map(|(event_id, bytes)| {
                    let mut event: leserpent_protocol::compatibility_v1::LegacyOrchestraEvent =
                        serde_json::from_slice(bytes)?;
                    event.event_id = *event_id;
                    Ok::<_, serde_json::Error>(event)
                })
                .collect::<Result<Vec<_>, _>>();
            return match (runs, events) {
                (Ok(runs), Ok(events)) => response(ProtocolResponse::OrchestraHistory(
                    OrchestraHistoryResponse {
                        runs,
                        events,
                        next_offset: history.next_offset,
                    },
                )),
                _ => error_response("runtime_failed", "Orchestra history read-back was invalid"),
            };
        }
        ProtocolRequest::OrchestraDelete(request) => {
            if request.principal.id.trim().is_empty() {
                return error_response("invalid_principal", "principal must not be blank");
            }
            if !request.capabilities.contains(CAPABILITY_ORCHESTRA_WRITE) {
                return error_response("capability_denied", "missing capability 'orchestra.write'");
            }
            return match runtime.delete_orchestra_runtimes(&request.runtime_ids) {
                Ok(deleted) => response(ProtocolResponse::OrchestraDeleted(
                    OrchestraDeleteResponse {
                        deleted_runtime_count: deleted.deleted_runtime_count,
                        deleted_run_count: deleted.deleted_run_count,
                        deleted_event_count: deleted.deleted_event_count,
                    },
                )),
                Err(_) => {
                    error_response("orchestra_delete_failed", "Orchestra history delete failed")
                }
            };
        }
        ProtocolRequest::OrchestraDeleteCommand(request) => {
            if request.principal.id.trim().is_empty() {
                return error_response("invalid_principal", "principal must not be blank");
            }
            if !request.capabilities.contains(CAPABILITY_ORCHESTRA_WRITE) {
                return error_response("capability_denied", "missing capability 'orchestra.write'");
            }
            return match runtime
                .delete_orchestra_runtimes_idempotent(request.command_id, &request.runtime_ids)
            {
                Ok(receipt) => response(ProtocolResponse::OrchestraDeleteReceipt(
                    OrchestraDeleteReceiptResponse {
                        command_id: receipt.command_id,
                        operation_generation: receipt.operation_generation,
                        runtime_ids: receipt.runtime_ids,
                        deleted_runtime_count: receipt.deleted_runtime_count,
                        deleted_run_count: receipt.deleted_run_count,
                        deleted_event_count: receipt.deleted_event_count,
                        committed_at_unix_ms: receipt.committed_at_unix_ms,
                        replayed: receipt.replayed,
                    },
                )),
                Err(error) => orchestra_delete_command_error(error),
            };
        }
        ProtocolRequest::OrchestraDeleteReplayHorizon(request) => {
            if request.principal.id.trim().is_empty() {
                return error_response("invalid_principal", "principal must not be blank");
            }
            if !request.capabilities.contains(CAPABILITY_ORCHESTRA_WRITE) {
                return error_response("capability_denied", "missing capability 'orchestra.write'");
            }
            return match runtime.orchestra_delete_replay_horizon() {
                Ok(horizon) => response(ProtocolResponse::OrchestraDeleteReplayHorizon(
                    orchestra_delete_replay_horizon_response(horizon),
                )),
                Err(_) => error_response(
                    "orchestra_delete_replay_horizon_failed",
                    "Orchestra delete replay horizon is unavailable",
                ),
            };
        }
        ProtocolRequest::OrchestraDeleteReplayCheckpoint(request) => {
            if request.principal.id.trim().is_empty() {
                return error_response("invalid_principal", "principal must not be blank");
            }
            if !request.capabilities.contains(CAPABILITY_ORCHESTRA_WRITE) {
                return error_response("capability_denied", "missing capability 'orchestra.write'");
            }
            return match runtime.checkpoint_orchestra_delete_replay_horizon(
                request.minimum_retained_generation,
                request.observed_through_generation,
            ) {
                Ok(horizon) => response(ProtocolResponse::OrchestraDeleteReplayHorizon(
                    orchestra_delete_replay_horizon_response(horizon),
                )),
                Err(_) => error_response(
                    "orchestra_delete_replay_checkpoint_failed",
                    "Orchestra delete replay checkpoint was rejected",
                ),
            };
        }
        ProtocolRequest::RuntimeUnregister(request) => {
            if request.principal.id.trim().is_empty()
                || !request.capabilities.contains(CAPABILITY_RUNTIME_UNREGISTER)
                || !request.confirmed
            {
                return error_response(
                    "runtime_unregister_rejected",
                    "runtime unregistration requires explicit authority and confirmation",
                );
            }
            let targets = request
                .targets
                .iter()
                .map(|target| RuntimeTarget {
                    runtime_id: target.runtime_id.clone(),
                    expected_revision: target.expected_revision,
                })
                .collect();
            return match runtime.unregister_runtimes(request.command_id.clone(), targets) {
                Ok(result) => response(ProtocolResponse::RuntimeUnregistered(
                    RuntimeUnregisterResponse {
                        command_id: result.command_id,
                        operation_generation: Some(result.operation_generation),
                        removed: request.targets,
                        deleted_orchestra_runtime_count: result.deleted_orchestra_runtime_count,
                        deleted_orchestra_run_count: result.deleted_orchestra_run_count,
                        deleted_orchestra_event_count: result.deleted_orchestra_event_count,
                        removed_at_unix_ms: result.removed_at_unix_ms,
                        replayed: result.replayed,
                    },
                )),
                Err(RuntimeError::Domain(error)) => {
                    leserpent_protocol::domain_error_response(&error)
                }
                Err(_) => {
                    error_response("runtime_unregister_failed", "runtime unregistration failed")
                }
            };
        }
        ProtocolRequest::RuntimeUnregistrationReceipt(request) => {
            if request.principal.id.trim().is_empty()
                || !request.capabilities.contains(CAPABILITY_RUNTIME_READ)
            {
                return error_response(
                    "unauthorized",
                    "runtime unregistration receipt access was rejected",
                );
            }
            return match runtime.runtime_unregistration_receipt(request.command_id.clone()) {
                Ok(lookup) => response(ProtocolResponse::RuntimeUnregistrationReceipt(
                    RuntimeUnregistrationReceiptLookupResponse {
                        command_id: lookup.command_id,
                        receipt: lookup.receipt.map(|receipt| RuntimeUnregistrationReceipt {
                            operation_generation: receipt.operation_generation,
                            removed: receipt
                                .removed
                                .into_iter()
                                .map(|target| RuntimeUnregisterTarget {
                                    runtime_id: target.runtime_id,
                                    expected_revision: target.expected_revision,
                                })
                                .collect(),
                            deleted_orchestra_runtime_count: receipt
                                .deleted_orchestra_runtime_count,
                            deleted_orchestra_run_count: receipt.deleted_orchestra_run_count,
                            deleted_orchestra_event_count: receipt.deleted_orchestra_event_count,
                            removed_at_unix_ms: receipt.removed_at_unix_ms,
                        }),
                        replay_horizon: RuntimeUnregistrationReplayHorizonHealth {
                            capacity: lookup.replay_horizon.capacity,
                            retained: lookup.replay_horizon.retained,
                            oldest_generation: lookup.replay_horizon.oldest_generation,
                            newest_generation: lookup.replay_horizon.newest_generation,
                            next_generation: lookup.replay_horizon.next_generation,
                            evicted_through_generation: lookup
                                .replay_horizon
                                .evicted_through_generation,
                        },
                    },
                )),
                Err(_) => error_response(
                    "runtime_unregistration_receipt_failed",
                    "runtime unregistration receipt lookup failed",
                ),
            };
        }
        ProtocolRequest::BootstrapHandoff(request) => {
            if request.principal.id.trim().is_empty()
                || !request.capabilities.contains(CAPABILITY_HOST_BOOTSTRAP)
            {
                return error_response("unauthorized", "bootstrap handoff access was rejected");
            }
            return match runtime.bootstrap_checkpoint(&request.bootstrap_id) {
                Ok(Some(checkpoint)) => {
                    response(ProtocolResponse::BootstrapHandoff(checkpoint.state))
                }
                Ok(None) => error_response(
                    "bootstrap_handoff_not_found",
                    "bootstrap handoff was not found",
                ),
                Err(_) => error_response("runtime_failed", "bootstrap handoff lookup failed"),
            };
        }
        ProtocolRequest::BootstrapSessionBind(request) => {
            if request.principal.id.trim().is_empty()
                || !request.capabilities.contains(CAPABILITY_HOST_BOOTSTRAP)
                || !request.confirmed
            {
                return error_response("unauthorized", "bootstrap session binding was rejected");
            }
            let checkpoint = match runtime.bootstrap_checkpoint(&request.bootstrap_id) {
                Ok(Some(checkpoint)) => checkpoint,
                Ok(None) => {
                    return error_response(
                        "bootstrap_handoff_not_found",
                        "bootstrap handoff was not found",
                    );
                }
                Err(_) => {
                    return error_response("runtime_failed", "bootstrap handoff lookup failed");
                }
            };
            let Some(verifier) = bootstrap_verifier else {
                return error_response(
                    "bootstrap_verifier_unavailable",
                    "server-side bootstrap session verification is unavailable",
                );
            };
            let proof = match verifier.prove_session(&checkpoint) {
                Ok(proof) => proof,
                Err(_) => {
                    return error_response(
                        "bootstrap_session_unverified",
                        "target daemon session authority could not be verified",
                    );
                }
            };
            return match runtime.bind_bootstrap_session(&request.bootstrap_id, proof) {
                Ok(state) => response(ProtocolResponse::BootstrapHandoff(state)),
                Err(RuntimeError::Bootstrap(_)) => error_response(
                    "bootstrap_session_rejected",
                    "target daemon session identity was rejected",
                ),
                Err(_) => error_response("runtime_failed", "bootstrap session binding failed"),
            };
        }
        request => request,
    };
    let required_capability = match &request {
        ProtocolRequest::Query(query) => match query.query {
            Query::RuntimeList { .. }
            | Query::RuntimeInspect { .. }
            | Query::RuntimeHistory { .. }
            | Query::RuntimeLogs { .. } => CAPABILITY_RUNTIME_READ,
        },
        ProtocolRequest::Command(command) => match command.command {
            Command::RuntimeRegister { .. }
            | Command::RuntimeRegistrationUpdate { .. }
            | Command::RuntimeDiscoveryIntake { .. } => {
                leserpent_domain::CAPABILITY_RUNTIME_REGISTER
            }
            Command::RuntimeRefresh { .. } | Command::RuntimeCapabilitiesRefresh { .. } => {
                CAPABILITY_RUNTIME_REFRESH
            }
            Command::RuntimeDeploy { .. } => leserpent_domain::CAPABILITY_RUNTIME_DEPLOY,
            Command::DebuggerCancel { .. } => leserpent_domain::CAPABILITY_DEBUGGER_CONTROL,
        },
        ProtocolRequest::Health(_)
        | ProtocolRequest::DeploymentReceipt(_)
        | ProtocolRequest::OrchestraPersist(_)
        | ProtocolRequest::OrchestraHistory(_)
        | ProtocolRequest::OrchestraDelete(_)
        | ProtocolRequest::OrchestraDeleteCommand(_)
        | ProtocolRequest::OrchestraDeleteReplayHorizon(_)
        | ProtocolRequest::OrchestraDeleteReplayCheckpoint(_)
        | ProtocolRequest::RuntimeUnregister(_)
        | ProtocolRequest::RuntimeUnregistrationReceipt(_)
        | ProtocolRequest::AuthorityWriterClaim(_)
        | ProtocolRequest::BootstrapHandoff(_)
        | ProtocolRequest::BootstrapSessionBind(_) => unreachable!(),
    };
    let operation = match request {
        ProtocolRequest::Query(query) => PlannedOperation::Query(query),
        ProtocolRequest::Command(command) => PlannedOperation::Command(command),
        ProtocolRequest::Health(_)
        | ProtocolRequest::DeploymentReceipt(_)
        | ProtocolRequest::OrchestraPersist(_)
        | ProtocolRequest::OrchestraHistory(_)
        | ProtocolRequest::OrchestraDelete(_)
        | ProtocolRequest::OrchestraDeleteCommand(_)
        | ProtocolRequest::OrchestraDeleteReplayHorizon(_)
        | ProtocolRequest::OrchestraDeleteReplayCheckpoint(_)
        | ProtocolRequest::RuntimeUnregister(_)
        | ProtocolRequest::RuntimeUnregistrationReceipt(_)
        | ProtocolRequest::AuthorityWriterClaim(_)
        | ProtocolRequest::BootstrapHandoff(_)
        | ProtocolRequest::BootstrapSessionBind(_) => unreachable!(),
    };
    match runtime.execute_plan(CommandPlan {
        schema_version: leserpent_domain::COMMAND_PLAN_SCHEMA_VERSION,
        required_capability: required_capability.to_string(),
        operation,
    }) {
        Ok(PlanResult::Query(result)) => response(ProtocolResponse::Query(result)),
        Ok(PlanResult::Command(result)) => response(ProtocolResponse::Command(Box::new(result))),
        Err(RuntimeError::Domain(error)) => leserpent_protocol::domain_error_response(&error),
        Err(RuntimeError::InvalidPlan(_)) => {
            error_response("invalid_request", "protocol command plan is invalid")
        }
        Err(_) => error_response("runtime_failed", "runtime request failed"),
    }
}

fn requires_authority_writer_fence(request: &ProtocolRequest) -> bool {
    matches!(request, ProtocolRequest::RuntimeUnregister(_))
        || matches!(
            request,
            ProtocolRequest::Command(command)
                if matches!(
                    command.command,
                    Command::RuntimeRegister { .. }
                        | Command::RuntimeRegistrationUpdate { .. }
                        | Command::RuntimeDiscoveryIntake { .. }
                )
        )
}

fn authority_writer_fence_error(error: RuntimeError) -> ResponseEnvelope {
    match error {
        RuntimeError::AuthorityWriterFence(
            leserpent_runtime::AuthorityWriterFenceError::Required,
        ) => error_response(
            "authority_writer_fence_required",
            "authority mutation requires the active writer fence",
        ),
        RuntimeError::AuthorityWriterFence(
            leserpent_runtime::AuthorityWriterFenceError::Rejected,
        ) => error_response(
            "authority_writer_fence_rejected",
            "authority mutation was submitted by a stale writer",
        ),
        _ => error_response(
            "authority_writer_fence_failed",
            "authority writer fence validation failed",
        ),
    }
}

fn orchestra_delete_command_error(error: RuntimeError) -> ResponseEnvelope {
    match error {
        RuntimeError::Domain(error) => leserpent_protocol::domain_error_response(&error),
        RuntimeError::OrchestraDeleteReplayHorizonSaturated => error_response(
            "orchestra_delete_replay_horizon_saturated",
            "cleanup receipt admission is blocked; persist reconciliation audit and advance its checkpoint",
        ),
        _ => error_response(
            "orchestra_delete_command_failed",
            "idempotent Orchestra history delete failed",
        ),
    }
}

fn orchestra_delete_replay_horizon_response(
    horizon: leserpent_runtime::OrchestraDeleteReplayHorizon,
) -> OrchestraDeleteReplayHorizonResponse {
    let admission_blocked = horizon.admission_blocked();
    let admission_pressure = match horizon.admission_pressure() {
        leserpent_runtime::OrchestraDeleteReplayAdmissionPressure::Healthy => {
            OrchestraDeleteReplayAdmissionPressure::Healthy
        }
        leserpent_runtime::OrchestraDeleteReplayAdmissionPressure::Warning => {
            OrchestraDeleteReplayAdmissionPressure::Warning
        }
        leserpent_runtime::OrchestraDeleteReplayAdmissionPressure::Critical => {
            OrchestraDeleteReplayAdmissionPressure::Critical
        }
        leserpent_runtime::OrchestraDeleteReplayAdmissionPressure::Blocked => {
            OrchestraDeleteReplayAdmissionPressure::Blocked
        }
    };
    OrchestraDeleteReplayHorizonResponse {
        capacity: horizon.capacity,
        retained: horizon.retained,
        available_capacity: horizon.available_capacity(),
        warning_available_capacity:
            leserpent_runtime::ORCHESTRA_DELETE_REPLAY_WARNING_AVAILABLE_CAPACITY,
        critical_available_capacity:
            leserpent_runtime::ORCHESTRA_DELETE_REPLAY_CRITICAL_AVAILABLE_CAPACITY,
        warning_recovery_available_capacity:
            leserpent_runtime::ORCHESTRA_DELETE_REPLAY_WARNING_RECOVERY_AVAILABLE_CAPACITY,
        critical_recovery_available_capacity:
            leserpent_runtime::ORCHESTRA_DELETE_REPLAY_CRITICAL_RECOVERY_AVAILABLE_CAPACITY,
        checkpoint_lag_generations: horizon.checkpoint_lag_generations(),
        saturated: horizon.saturated(),
        admission_state: if admission_blocked {
            OrchestraDeleteReplayAdmissionState::BlockedByReconciliationAudit
        } else {
            OrchestraDeleteReplayAdmissionState::Ready
        },
        admission_pressure,
        operator_action: horizon
            .operator_action_required()
            .then_some(OrchestraDeleteReplayOperatorAction::PersistAuditAndAdvanceCheckpoint),
        oldest_generation: horizon.oldest_generation,
        newest_generation: horizon.newest_generation,
        next_generation: horizon.next_generation,
        evicted_through_generation: horizon.evicted_through_generation,
        protected_from_generation: horizon.protected_from_generation,
        checkpointed_through_generation: horizon.checkpointed_through_generation,
    }
}

pub(crate) fn error_response(code: &str, message: &str) -> ResponseEnvelope {
    response(ProtocolResponse::Error(ProtocolError {
        code: code.to_string(),
        message: message.to_string(),
    }))
}

pub(crate) fn validate_auth_token(token: &str) -> Result<(), String> {
    if token.len() < 32
        || token.len() > MAX_AUTH_TOKEN_BYTES
        || token.bytes().any(|byte| byte <= 0x20)
    {
        return Err("authentication token must contain 32 to 256 non-whitespace bytes".into());
    }
    Ok(())
}

pub(crate) fn constant_time_equals(left: &[u8], right: &[u8]) -> bool {
    let mut difference = left.len() ^ right.len();
    let length = left.len().max(right.len());
    for index in 0..length {
        difference |= usize::from(
            left.get(index).copied().unwrap_or(0) ^ right.get(index).copied().unwrap_or(0),
        );
    }
    difference == 0
}

fn response(response: ProtocolResponse) -> ResponseEnvelope {
    ResponseEnvelope {
        schema_version: PROTOCOL_SCHEMA_VERSION,
        response,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use leserpent_domain::bootstrap::{
        BOOTSTRAP_SESSION_PROTOCOL_VERSION, BootstrapId, BootstrapPhase, BootstrapTarget,
        BootstrapTransport, CredentialHandle, DaemonId, DeploymentBootstrapSnapshot,
    };
    use leserpent_domain::{CapabilitySet, Principal};
    use leserpent_protocol::{
        AuthorityWriterClaimRequest, BootstrapHandoffRequest, BootstrapSessionBindRequest,
        CAPABILITY_AUTHORITY_WRITER, ProtocolRequest,
    };

    use super::*;

    struct FixedVerifier {
        proof: DaemonSessionProof,
    }

    impl BootstrapSessionVerifier for FixedVerifier {
        fn prove_session(
            &self,
            _checkpoint: &DeploymentBootstrapCheckpoint,
        ) -> Result<DaemonSessionProof, String> {
            Ok(self.proof.clone())
        }
    }

    fn temp_database() -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "leserpent-wire-bootstrap-{}-{unique}.sqlite",
            std::process::id()
        ))
    }

    #[test]
    fn authority_writer_claim_is_unavailable_outside_private_ipc() {
        let path = temp_database();
        let mut runtime = ControlRuntime::open(&path).unwrap();
        let claim = RequestEnvelope {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            request: ProtocolRequest::AuthorityWriterClaim(AuthorityWriterClaimRequest {
                principal: Principal {
                    id: "operator".into(),
                },
                capabilities: CapabilitySet::new([CAPABILITY_AUTHORITY_WRITER]),
                writer_id: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into(),
            }),
        };

        let rejected = execute_request(&mut runtime, claim, None, None, false);
        assert!(matches!(
            rejected.response,
            ProtocolResponse::Error(ref error)
                if error.code == "authority_writer_claim_unavailable"
        ));
        runtime.require_authority_writer(None, None).unwrap();

        drop(runtime);
        fs::remove_file(path).unwrap();
    }

    fn seed_bootstrapped(runtime: &mut ControlRuntime) -> BootstrapId {
        let bootstrap_id = BootstrapId::new("bootstrap-wire-1").unwrap();
        runtime
            .enqueue_effect(
                "bootstrap-wire-effect",
                "leserpent.host.bootstrap",
                b"request",
                3,
            )
            .unwrap();
        let lease = runtime
            .claim_effect("wire-worker", Duration::from_secs(30))
            .unwrap()
            .unwrap();
        let checkpoint = DeploymentBootstrapCheckpoint::new(
            1,
            DeploymentBootstrapSnapshot {
                bootstrap_id: bootstrap_id.clone(),
                phase: BootstrapPhase::Bootstrapped,
                target: BootstrapTarget {
                    transport: BootstrapTransport::Ssh,
                    host: "host.example".into(),
                    port: 22,
                },
                bootstrap_credential_present: true,
                daemon_id: Some(DaemonId::new("daemon-host-example").unwrap()),
                endpoint: Some("https://host.example:9443/".into()),
                session_credential_handle: Some(
                    CredentialHandle::new("vault:leserpentd:host-example").unwrap(),
                ),
                trust_credential_handle: Some(
                    CredentialHandle::new("vault:leserpent-ca:host-example").unwrap(),
                ),
                fault_code: None,
                mutation_authorized: false,
            },
            Some(CredentialHandle::new("vault:ssh:host-example").unwrap()),
        )
        .unwrap();
        runtime
            .complete_bootstrap_effect(&lease, b"outcome", &checkpoint)
            .unwrap();
        bootstrap_id
    }

    fn query(bootstrap_id: &BootstrapId) -> RequestEnvelope {
        RequestEnvelope {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            request: ProtocolRequest::BootstrapHandoff(BootstrapHandoffRequest {
                principal: Principal {
                    id: "operator-a".into(),
                },
                capabilities: CapabilitySet::new([CAPABILITY_HOST_BOOTSTRAP]),
                bootstrap_id: bootstrap_id.clone(),
            }),
        }
    }

    fn bind(bootstrap_id: &BootstrapId) -> RequestEnvelope {
        RequestEnvelope {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            request: ProtocolRequest::BootstrapSessionBind(BootstrapSessionBindRequest {
                principal: Principal {
                    id: "operator-a".into(),
                },
                capabilities: CapabilitySet::new([CAPABILITY_HOST_BOOTSTRAP]),
                bootstrap_id: bootstrap_id.clone(),
                confirmed: true,
            }),
        }
    }

    fn proof(bootstrap_id: &BootstrapId, daemon_id: &str) -> DaemonSessionProof {
        DaemonSessionProof {
            bootstrap_id: bootstrap_id.clone(),
            daemon_id: DaemonId::new(daemon_id).unwrap(),
            session_credential_handle: CredentialHandle::new("vault:leserpentd:host-example")
                .unwrap(),
            trust_credential_handle: CredentialHandle::new("vault:leserpent-ca:host-example")
                .unwrap(),
            authority_owned: true,
            protocol_schema_version: BOOTSTRAP_SESSION_PROTOCOL_VERSION,
        }
    }

    #[test]
    fn cleanup_horizon_saturation_is_an_actionable_wire_error() {
        let response =
            orchestra_delete_command_error(RuntimeError::OrchestraDeleteReplayHorizonSaturated);
        assert!(matches!(
            response.response,
            ProtocolResponse::Error(ref error)
                if error.code == "orchestra_delete_replay_horizon_saturated"
                    && error.message.contains("advance its checkpoint")
        ));
    }

    #[test]
    fn cleanup_horizon_warning_is_actionable_before_admission_blocks() {
        let response = orchestra_delete_replay_horizon_response(
            leserpent_runtime::OrchestraDeleteReplayHorizon {
                capacity: 4_096,
                retained: 3_584,
                oldest_generation: Some(1),
                newest_generation: Some(3_584),
                next_generation: 3_585,
                evicted_through_generation: 0,
                protected_from_generation: Some(1),
                checkpointed_through_generation: None,
            },
        );
        assert_eq!(response.available_capacity, 512);
        assert!(!response.saturated);
        assert_eq!(
            response.admission_state,
            OrchestraDeleteReplayAdmissionState::Ready
        );
        assert_eq!(
            response.admission_pressure,
            OrchestraDeleteReplayAdmissionPressure::Warning
        );
        assert_eq!(
            response.operator_action,
            Some(OrchestraDeleteReplayOperatorAction::PersistAuditAndAdvanceCheckpoint)
        );
    }

    #[test]
    fn wire_handoff_requires_server_side_proof_before_session_binding() {
        let path = temp_database();
        let mut runtime = ControlRuntime::open(&path).unwrap();
        let bootstrap_id = seed_bootstrapped(&mut runtime);

        let queried = execute_request(&mut runtime, query(&bootstrap_id), None, None, false);
        assert!(matches!(
            queried.response,
            ProtocolResponse::BootstrapHandoff(ref state)
                if state.phase == BootstrapPhase::Bootstrapped
                    && !state.mutation_authorized
        ));
        let unavailable = execute_request(&mut runtime, bind(&bootstrap_id), None, None, false);
        assert!(matches!(
            unavailable.response,
            ProtocolResponse::Error(ref error)
                if error.code == "bootstrap_verifier_unavailable"
        ));

        let wrong = FixedVerifier {
            proof: proof(&bootstrap_id, "daemon-wrong"),
        };
        let rejected =
            execute_request(&mut runtime, bind(&bootstrap_id), Some(&wrong), None, false);
        assert!(matches!(
            rejected.response,
            ProtocolResponse::Error(ref error)
                if error.code == "bootstrap_session_rejected"
        ));
        assert_eq!(
            runtime
                .bootstrap_checkpoint(&bootstrap_id)
                .unwrap()
                .unwrap()
                .revision,
            1
        );

        let verifier = FixedVerifier {
            proof: proof(&bootstrap_id, "daemon-host-example"),
        };
        let bound = execute_request(
            &mut runtime,
            bind(&bootstrap_id),
            Some(&verifier),
            None,
            false,
        );
        assert!(matches!(
            &bound.response,
            ProtocolResponse::BootstrapHandoff(state)
                if state.phase == BootstrapPhase::SessionBound && state.mutation_authorized
        ));
        let replay = execute_request(
            &mut runtime,
            bind(&bootstrap_id),
            Some(&verifier),
            None,
            false,
        );
        assert_eq!(replay, bound);
        assert_eq!(
            runtime
                .bootstrap_checkpoint(&bootstrap_id)
                .unwrap()
                .unwrap()
                .revision,
            2
        );
        drop(runtime);
        fs::remove_file(path).unwrap();
    }
}
