use leserpent_domain::{
    CAPABILITY_RUNTIME_READ, CAPABILITY_RUNTIME_REFRESH, Command, CommandPlan, PlannedOperation,
    Query,
};
use leserpent_protocol::{
    EffectQueueHealth, HealthResponse, PROTOCOL_SCHEMA_VERSION, ProtocolError, ProtocolRequest,
    ProtocolResponse, RequestEnvelope, ResponseEnvelope,
};
use leserpent_runtime::{ControlRuntime, PlanResult, RuntimeError};

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
        ProtocolRequest::Health(_) => unreachable!(),
    };
    let operation = match request {
        ProtocolRequest::Query(query) => PlannedOperation::Query(query),
        ProtocolRequest::Command(command) => PlannedOperation::Command(command),
        ProtocolRequest::Health(_) => unreachable!(),
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
