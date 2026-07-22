use leserpent_adapters::GEWYVERN_PROVISIONING_EFFECT_KIND;
use leserpent_domain::provisioning::{ProvisioningPhase, RuntimeProvisioning};
use leserpent_protocol::provisioning::{
    PROVISIONING_PROTOCOL_SCHEMA_VERSION, ProvisioningProtocolError, ProvisioningRequestEnvelope,
    ProvisioningResponse, ProvisioningResponseEnvelope, decode_provisioning_request,
    encode_provisioning_request,
};
use leserpent_runtime::ControlRuntime;
use ring::digest::{SHA256, digest};

const PROVISIONING_EFFECT_MAX_ATTEMPTS: u32 = 3;

pub(crate) fn decode_and_submit(
    runtime: &mut ControlRuntime,
    bytes: &[u8],
    enabled: bool,
) -> ProvisioningResponseEnvelope {
    let request = match decode_provisioning_request(bytes) {
        Ok(request) => request,
        Err(_) => return error(None, "invalid_request", "provisioning request is invalid"),
    };
    submit(runtime, request, enabled)
}

fn submit(
    runtime: &mut ControlRuntime,
    request: ProvisioningRequestEnvelope,
    enabled: bool,
) -> ProvisioningResponseEnvelope {
    let provisioning_id = request.request.intent.provisioning_id.clone();
    if !enabled {
        return error(
            Some(provisioning_id),
            "provisioning_unavailable",
            "Gewyvern provisioning adapter is not configured",
        );
    }
    let provisioning = match RuntimeProvisioning::plan(
        &request.request.principal,
        &request.request.capabilities,
        request.request.intent.clone(),
    ) {
        Ok(provisioning) => provisioning,
        Err(_) => {
            return error(
                Some(provisioning_id),
                "invalid_request",
                "provisioning authorization was rejected",
            );
        }
    };
    let planned = match provisioning.checkpoint(1) {
        Ok(checkpoint) => checkpoint,
        Err(_) => {
            return error(
                Some(provisioning_id),
                "invalid_request",
                "provisioning plan could not be created",
            );
        }
    };
    match runtime.provisioning_checkpoint(&provisioning_id) {
        Ok(Some(existing)) => {
            if existing.state.runtime_id != planned.state.runtime_id
                || existing.state.target != planned.state.target
                || (existing.install_credential_handle.is_some()
                    && existing.install_credential_handle != planned.install_credential_handle)
            {
                return error(
                    Some(provisioning_id),
                    "provisioning_identity_conflict",
                    "provisioning identity was already used by another request",
                );
            }
            if existing.state.phase != ProvisioningPhase::Planned {
                return state(existing.state);
            }
        }
        Ok(None) => {}
        Err(_) => {
            return error(
                Some(provisioning_id),
                "runtime_unavailable",
                "provisioning persistence is unavailable",
            );
        }
    }
    let payload = match encode_provisioning_request(&request) {
        Ok(payload) => payload,
        Err(_) => {
            return error(
                Some(provisioning_id),
                "invalid_request",
                "provisioning request could not be encoded",
            );
        }
    };
    let effect_id = effect_id(&provisioning_id);
    match runtime.enqueue_provisioning_effect(
        &effect_id,
        GEWYVERN_PROVISIONING_EFFECT_KIND,
        &payload,
        PROVISIONING_EFFECT_MAX_ATTEMPTS,
        &planned,
    ) {
        Ok(()) => state(planned.state),
        Err(_) => error(
            Some(provisioning_id),
            "provisioning_submission_failed",
            "provisioning submission was not committed",
        ),
    }
}

fn effect_id(provisioning_id: &leserpent_domain::provisioning::ProvisioningId) -> String {
    let hash = digest(&SHA256, provisioning_id.as_str().as_bytes());
    format!("provisioning:{}", hex(hash.as_ref()))
}

fn state(
    state: leserpent_domain::provisioning::RuntimeProvisioningSnapshot,
) -> ProvisioningResponseEnvelope {
    ProvisioningResponseEnvelope {
        schema_version: PROVISIONING_PROTOCOL_SCHEMA_VERSION,
        response: ProvisioningResponse::State(state),
    }
}

pub(crate) fn error(
    provisioning_id: Option<leserpent_domain::provisioning::ProvisioningId>,
    code: &str,
    message: &str,
) -> ProvisioningResponseEnvelope {
    ProvisioningResponseEnvelope {
        schema_version: PROVISIONING_PROTOCOL_SCHEMA_VERSION,
        response: ProvisioningResponse::Error(ProvisioningProtocolError {
            provisioning_id,
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
