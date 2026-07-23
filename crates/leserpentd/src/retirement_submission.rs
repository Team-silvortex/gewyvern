use leserpent_adapters::GEWYVERN_RETIREMENT_EFFECT_KIND;
use leserpent_domain::retirement::{RetirementId, RetirementPhase, RuntimeRetirement};
use leserpent_protocol::retirement::{
    RETIREMENT_PROTOCOL_SCHEMA_VERSION, RetirementProtocolError, RetirementRequestEnvelope,
    RetirementResponse, RetirementResponseEnvelope, decode_retirement_request,
    encode_retirement_request,
};
use leserpent_runtime::ControlRuntime;
use ring::digest::{SHA256, digest};

const RETIREMENT_EFFECT_MAX_ATTEMPTS: u32 = 3;

pub(crate) fn decode_and_submit(
    runtime: &mut ControlRuntime,
    bytes: &[u8],
    enabled: bool,
) -> RetirementResponseEnvelope {
    let request = match decode_retirement_request(bytes) {
        Ok(request) => request,
        Err(_) => return error(None, "invalid_request", "retirement request is invalid"),
    };
    submit(runtime, request, enabled)
}

fn submit(
    runtime: &mut ControlRuntime,
    request: RetirementRequestEnvelope,
    enabled: bool,
) -> RetirementResponseEnvelope {
    let retirement_id = request.request.intent.retirement_id.clone();
    if !enabled {
        return error(
            Some(retirement_id),
            "retirement_unavailable",
            "Gewyvern retirement adapter is not configured",
        );
    }
    let retirement = match RuntimeRetirement::plan(
        &request.request.principal,
        &request.request.capabilities,
        request.request.intent.clone(),
    ) {
        Ok(retirement) => retirement,
        Err(_) => {
            return error(
                Some(retirement_id),
                "invalid_request",
                "retirement authorization was rejected",
            );
        }
    };
    let planned = match retirement.checkpoint(1) {
        Ok(checkpoint) => checkpoint,
        Err(_) => {
            return error(
                Some(retirement_id),
                "invalid_request",
                "retirement plan could not be created",
            );
        }
    };
    match runtime.retirement_checkpoint(&retirement_id) {
        Ok(Some(existing)) => {
            if existing.state.provisioning_id != planned.state.provisioning_id
                || existing.state.runtime_id != planned.state.runtime_id
                || existing.state.target != planned.state.target
                || (existing.retirement_credential_handle.is_some()
                    && existing.retirement_credential_handle
                        != planned.retirement_credential_handle)
            {
                return error(
                    Some(retirement_id),
                    "retirement_identity_conflict",
                    "retirement identity was already used by another request",
                );
            }
            if existing.state.phase != RetirementPhase::Planned {
                return state(existing.state);
            }
        }
        Ok(None) => {}
        Err(_) => {
            return error(
                Some(retirement_id),
                "runtime_unavailable",
                "retirement persistence is unavailable",
            );
        }
    }
    if runtime
        .runtime_projection(&request.request.intent.runtime_id)
        .is_none()
    {
        return error(
            Some(retirement_id),
            "runtime_not_registered",
            "runtime is not registered with this daemon",
        );
    }
    let payload = match encode_retirement_request(&request) {
        Ok(payload) => payload,
        Err(_) => {
            return error(
                Some(retirement_id),
                "invalid_request",
                "retirement request could not be encoded",
            );
        }
    };
    let effect_id = effect_id(&retirement_id);
    match runtime.enqueue_retirement_effect(
        &effect_id,
        GEWYVERN_RETIREMENT_EFFECT_KIND,
        &payload,
        RETIREMENT_EFFECT_MAX_ATTEMPTS,
        &planned,
    ) {
        Ok(()) => state(planned.state),
        Err(_) => error(
            Some(retirement_id),
            "retirement_submission_failed",
            "retirement submission was not committed",
        ),
    }
}

fn effect_id(retirement_id: &RetirementId) -> String {
    let hash = digest(&SHA256, retirement_id.as_str().as_bytes());
    format!("retirement:{}", hex(hash.as_ref()))
}

fn state(
    state: leserpent_domain::retirement::RuntimeRetirementSnapshot,
) -> RetirementResponseEnvelope {
    RetirementResponseEnvelope {
        schema_version: RETIREMENT_PROTOCOL_SCHEMA_VERSION,
        response: RetirementResponse::State(state),
    }
}

pub(crate) fn error(
    retirement_id: Option<RetirementId>,
    code: &str,
    message: &str,
) -> RetirementResponseEnvelope {
    RetirementResponseEnvelope {
        schema_version: RETIREMENT_PROTOCOL_SCHEMA_VERSION,
        response: RetirementResponse::Error(RetirementProtocolError {
            retirement_id,
            code: code.into(),
            message: message.into(),
        }),
    }
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}
