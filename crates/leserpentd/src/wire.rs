use leserpent_domain::{
    CAPABILITY_ORCHESTRA_WRITE, CAPABILITY_RUNTIME_DEPLOY, CAPABILITY_RUNTIME_READ,
    CAPABILITY_RUNTIME_REFRESH, Command, CommandPlan, IdempotencyKey, PlannedOperation, Query,
};
use leserpent_protocol::{
    DeploymentReceiptResponse, DeploymentReceiptStatus, EffectQueueHealth, HealthResponse,
    OrchestraPersistenceResponse, PROTOCOL_SCHEMA_VERSION, ProtocolError, ProtocolRequest,
    ProtocolResponse, RequestEnvelope, ResponseEnvelope,
};
use leserpent_runtime::{ControlRuntime, DeploymentEffectState, PlanResult, RuntimeError};

pub(crate) const MAX_AUTH_TOKEN_BYTES: usize = 256;

pub(crate) fn execute_request(
    runtime: &mut ControlRuntime,
    request: RequestEnvelope,
) -> ResponseEnvelope {
    let request = match request.request {
        ProtocolRequest::Health(_) => {
            return match runtime
                .heartbeat()
                .and_then(|()| runtime.effect_queue_stats())
            {
                Ok(queue) => response(ProtocolResponse::Health(HealthResponse {
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
                })),
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
                &request.envelope.event.event_type,
                &request.envelope.event.to_outcome,
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
            Command::RuntimeRefresh { .. } | Command::RuntimeCapabilitiesRefresh { .. } => {
                CAPABILITY_RUNTIME_REFRESH
            }
            Command::RuntimeDeploy { .. } => leserpent_domain::CAPABILITY_RUNTIME_DEPLOY,
            Command::DebuggerCancel { .. } => leserpent_domain::CAPABILITY_DEBUGGER_CONTROL,
        },
        ProtocolRequest::Health(_)
        | ProtocolRequest::DeploymentReceipt(_)
        | ProtocolRequest::OrchestraPersist(_) => unreachable!(),
    };
    let operation = match request {
        ProtocolRequest::Query(query) => PlannedOperation::Query(query),
        ProtocolRequest::Command(command) => PlannedOperation::Command(command),
        ProtocolRequest::Health(_)
        | ProtocolRequest::DeploymentReceipt(_)
        | ProtocolRequest::OrchestraPersist(_) => unreachable!(),
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
