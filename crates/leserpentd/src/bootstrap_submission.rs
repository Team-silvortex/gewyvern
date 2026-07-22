use leserpent_adapters::HOST_BOOTSTRAP_EFFECT_KIND;
use leserpent_domain::bootstrap::{BootstrapPhase, DeploymentBootstrap};
use leserpent_protocol::bootstrap::{
    BOOTSTRAP_PROTOCOL_SCHEMA_VERSION, BootstrapProtocolError, BootstrapRequestEnvelope,
    BootstrapResponse, BootstrapResponseEnvelope, decode_bootstrap_request,
    encode_bootstrap_request,
};
use leserpent_runtime::ControlRuntime;
use ring::digest::{SHA256, digest};

const BOOTSTRAP_EFFECT_MAX_ATTEMPTS: u32 = 3;

pub(crate) fn decode_and_submit(
    runtime: &mut ControlRuntime,
    bytes: &[u8],
    enabled: bool,
) -> BootstrapResponseEnvelope {
    let request = match decode_bootstrap_request(bytes) {
        Ok(request) => request,
        Err(_) => {
            return error(None, "invalid_request", "bootstrap request is invalid");
        }
    };
    submit(runtime, request, enabled)
}

fn submit(
    runtime: &mut ControlRuntime,
    request: BootstrapRequestEnvelope,
    enabled: bool,
) -> BootstrapResponseEnvelope {
    let bootstrap_id = request.request.intent.bootstrap_id.clone();
    if !enabled {
        return error(
            Some(bootstrap_id),
            "bootstrap_unavailable",
            "native bootstrap origin is not configured",
        );
    }
    let bootstrap = match DeploymentBootstrap::plan(
        &request.request.principal,
        &request.request.capabilities,
        request.request.intent.clone(),
    ) {
        Ok(bootstrap) => bootstrap,
        Err(_) => {
            return error(
                Some(bootstrap_id),
                "invalid_request",
                "bootstrap authorization was rejected",
            );
        }
    };
    let planned = match bootstrap.checkpoint(1) {
        Ok(checkpoint) => checkpoint,
        Err(_) => {
            return error(
                Some(bootstrap_id),
                "invalid_request",
                "bootstrap plan could not be created",
            );
        }
    };
    match runtime.bootstrap_checkpoint(&bootstrap_id) {
        Ok(Some(existing)) => {
            if existing.state.target != planned.state.target
                || (existing.bootstrap_credential_handle.is_some()
                    && existing.bootstrap_credential_handle != planned.bootstrap_credential_handle)
            {
                return error(
                    Some(bootstrap_id),
                    "bootstrap_identity_conflict",
                    "bootstrap identity was already used by another request",
                );
            }
            if existing.state.phase != BootstrapPhase::Planned {
                return state(existing.state);
            }
        }
        Ok(None) => {}
        Err(_) => {
            return error(
                Some(bootstrap_id),
                "runtime_unavailable",
                "bootstrap persistence is unavailable",
            );
        }
    }
    let payload = match encode_bootstrap_request(&request) {
        Ok(payload) => payload,
        Err(_) => {
            return error(
                Some(bootstrap_id),
                "invalid_request",
                "bootstrap request could not be encoded",
            );
        }
    };
    let effect_id = effect_id(&bootstrap_id);
    match runtime.enqueue_bootstrap_effect(
        &effect_id,
        HOST_BOOTSTRAP_EFFECT_KIND,
        &payload,
        BOOTSTRAP_EFFECT_MAX_ATTEMPTS,
        &planned,
    ) {
        Ok(()) => state(planned.state),
        Err(_) => error(
            Some(bootstrap_id),
            "bootstrap_submission_failed",
            "bootstrap submission was not committed",
        ),
    }
}

fn effect_id(bootstrap_id: &leserpent_domain::bootstrap::BootstrapId) -> String {
    let hash = digest(&SHA256, bootstrap_id.as_str().as_bytes());
    format!("bootstrap:{}", hex(hash.as_ref()))
}

fn state(
    state: leserpent_domain::bootstrap::DeploymentBootstrapSnapshot,
) -> BootstrapResponseEnvelope {
    BootstrapResponseEnvelope {
        schema_version: BOOTSTRAP_PROTOCOL_SCHEMA_VERSION,
        response: BootstrapResponse::State(state),
    }
}

pub(crate) fn error(
    bootstrap_id: Option<leserpent_domain::bootstrap::BootstrapId>,
    code: &str,
    message: &str,
) -> BootstrapResponseEnvelope {
    BootstrapResponseEnvelope {
        schema_version: BOOTSTRAP_PROTOCOL_SCHEMA_VERSION,
        response: BootstrapResponse::Error(BootstrapProtocolError {
            bootstrap_id,
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
