use leserpent_adapters::DAEMON_RETIREMENT_EFFECT_KIND;
use leserpent_domain::bootstrap_retirement::{
    DaemonRetirement, DaemonRetirementPhase, DaemonRetirementSnapshot,
};
use leserpent_domain::retirement::RetirementId;
use leserpent_protocol::bootstrap_retirement_control::{
    DAEMON_RETIREMENT_PROTOCOL_SCHEMA_VERSION, DaemonRetirementEffectEnvelope,
    DaemonRetirementProtocolError, DaemonRetirementRequestEnvelope, DaemonRetirementResponse,
    DaemonRetirementResponseEnvelope, decode_daemon_retirement_request,
    encode_daemon_retirement_effect,
};
use leserpent_runtime::ControlRuntime;
use ring::digest::{SHA256, digest};

const DAEMON_RETIREMENT_EFFECT_MAX_ATTEMPTS: u32 = 3;

pub(crate) fn decode_and_submit(
    runtime: &mut ControlRuntime,
    bytes: &[u8],
    enabled: bool,
) -> DaemonRetirementResponseEnvelope {
    let request = match decode_daemon_retirement_request(bytes) {
        Ok(request) => request,
        Err(_) => {
            return error(
                None,
                "invalid_request",
                "daemon retirement request is invalid",
            );
        }
    };
    submit(runtime, request, enabled)
}

fn submit(
    runtime: &mut ControlRuntime,
    request: DaemonRetirementRequestEnvelope,
    enabled: bool,
) -> DaemonRetirementResponseEnvelope {
    let retirement_id = request.request.intent.retirement_id.clone();
    if !enabled {
        return error(
            Some(retirement_id),
            "daemon_retirement_unavailable",
            "native daemon retirement origin is not configured",
        );
    }
    let deployment = match runtime.bootstrap_checkpoint(&request.request.intent.bootstrap_id) {
        Ok(Some(checkpoint)) => checkpoint,
        Ok(None) => {
            return error(
                Some(retirement_id),
                "deployment_authority_not_found",
                "bound deployment authority was not found",
            );
        }
        Err(_) => {
            return error(
                Some(retirement_id),
                "runtime_unavailable",
                "daemon retirement persistence is unavailable",
            );
        }
    };
    let retirement = match DaemonRetirement::plan(
        &request.request.principal,
        &request.request.capabilities,
        request.request.intent,
        &deployment,
    ) {
        Ok(retirement) => retirement,
        Err(_) => {
            return error(
                Some(retirement_id),
                "invalid_request",
                "daemon retirement authorization or deployment authority was rejected",
            );
        }
    };
    let planned = match retirement.checkpoint(1) {
        Ok(checkpoint) => checkpoint,
        Err(_) => {
            return error(
                Some(retirement_id),
                "invalid_request",
                "daemon retirement plan could not be created",
            );
        }
    };
    match runtime.daemon_retirement_checkpoint(&retirement_id) {
        Ok(Some(existing)) => {
            if existing.state.bootstrap_id != planned.state.bootstrap_id
                || existing.state.daemon_id != planned.state.daemon_id
                || existing.state.target != planned.state.target
                || existing.state.generation != planned.state.generation
                || existing.state.install_profile != planned.state.install_profile
                || (existing.retirement_credential_handle.is_some()
                    && existing.retirement_credential_handle
                        != planned.retirement_credential_handle)
            {
                return error(
                    Some(retirement_id),
                    "daemon_retirement_identity_conflict",
                    "daemon retirement identity was already used by another request",
                );
            }
            if existing.state.phase != DaemonRetirementPhase::Planned {
                return state(existing.state);
            }
        }
        Ok(None) => {}
        Err(_) => {
            return error(
                Some(retirement_id),
                "runtime_unavailable",
                "daemon retirement persistence is unavailable",
            );
        }
    }
    let payload = match encode_daemon_retirement_effect(&DaemonRetirementEffectEnvelope {
        schema_version: DAEMON_RETIREMENT_PROTOCOL_SCHEMA_VERSION,
        checkpoint: planned.clone(),
    }) {
        Ok(payload) => payload,
        Err(_) => {
            return error(
                Some(retirement_id),
                "invalid_request",
                "daemon retirement effect could not be encoded",
            );
        }
    };
    let effect_id = effect_id(&retirement_id);
    match runtime.enqueue_daemon_retirement_effect(
        &effect_id,
        DAEMON_RETIREMENT_EFFECT_KIND,
        &payload,
        DAEMON_RETIREMENT_EFFECT_MAX_ATTEMPTS,
        &planned,
    ) {
        Ok(()) => state(planned.state),
        Err(_) => error(
            Some(retirement_id),
            "daemon_retirement_submission_failed",
            "daemon retirement submission was not committed",
        ),
    }
}

fn effect_id(retirement_id: &RetirementId) -> String {
    let hash = digest(&SHA256, retirement_id.as_str().as_bytes());
    format!("daemon-retirement:{}", hex(hash.as_ref()))
}

fn state(state: DaemonRetirementSnapshot) -> DaemonRetirementResponseEnvelope {
    DaemonRetirementResponseEnvelope {
        schema_version: DAEMON_RETIREMENT_PROTOCOL_SCHEMA_VERSION,
        response: DaemonRetirementResponse::State(state),
    }
}

pub(crate) fn error(
    retirement_id: Option<RetirementId>,
    code: &str,
    message: &str,
) -> DaemonRetirementResponseEnvelope {
    DaemonRetirementResponseEnvelope {
        schema_version: DAEMON_RETIREMENT_PROTOCOL_SCHEMA_VERSION,
        response: DaemonRetirementResponse::Error(DaemonRetirementProtocolError {
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

#[cfg(test)]
pub(crate) fn seed_bound_deployment(runtime: &mut ControlRuntime, bootstrap_id: &str) {
    use std::time::Duration;

    use leserpent_domain::bootstrap::{
        BOOTSTRAP_DOMAIN_SCHEMA_VERSION, BOOTSTRAP_SESSION_PROTOCOL_VERSION, BootstrapId,
        BootstrapIntent, BootstrapTarget, BootstrapTransport, CAPABILITY_HOST_BOOTSTRAP,
        CredentialHandle, DaemonBootstrapReceipt, DaemonId, DaemonSessionProof,
        DeploymentBootstrap,
    };
    use leserpent_domain::{CapabilitySet, Principal};

    let bootstrap_id = BootstrapId::new(bootstrap_id).unwrap();
    let principal = Principal {
        id: "operator-a".into(),
    };
    let mut deployment = DeploymentBootstrap::plan(
        &principal,
        &CapabilitySet::new([CAPABILITY_HOST_BOOTSTRAP]),
        BootstrapIntent {
            schema_version: BOOTSTRAP_DOMAIN_SCHEMA_VERSION,
            bootstrap_id: bootstrap_id.clone(),
            target: BootstrapTarget {
                transport: BootstrapTransport::Ssh,
                host: "host.example".into(),
                port: 22,
            },
            credential_handle: leserpent_domain::bootstrap::CredentialHandle::new(
                "vault:ssh:host-example",
            )
            .unwrap(),
            requested_by: principal.id.clone(),
            confirmed: true,
        },
    )
    .unwrap();
    runtime
        .enqueue_bootstrap_effect(
            &format!("test-bootstrap-{}", bootstrap_id.as_str()),
            leserpent_adapters::HOST_BOOTSTRAP_EFFECT_KIND,
            b"test-bootstrap-request",
            1,
            &deployment.checkpoint(1).unwrap(),
        )
        .unwrap();
    let lease = runtime
        .claim_effect("test-bootstrap-worker", Duration::from_secs(30))
        .unwrap()
        .unwrap();
    deployment.begin().unwrap();
    deployment
        .accept_deployed(DaemonBootstrapReceipt {
            bootstrap_id: bootstrap_id.clone(),
            daemon_id: DaemonId::new("daemon-host-example").unwrap(),
            endpoint: "https://host.example:9443/".into(),
            generation: "a".repeat(64),
            install_profile: "system".into(),
            session_credential_handle: CredentialHandle::new("vault:leserpentd:host-example")
                .unwrap(),
            trust_credential_handle: CredentialHandle::new("vault:leserpent-ca:host-example")
                .unwrap(),
        })
        .unwrap();
    runtime
        .complete_bootstrap_effect(
            &lease,
            b"test-bootstrap-ready",
            &deployment.checkpoint(2).unwrap(),
        )
        .unwrap();
    runtime
        .bind_bootstrap_session(
            &bootstrap_id,
            DaemonSessionProof {
                bootstrap_id: bootstrap_id.clone(),
                daemon_id: DaemonId::new("daemon-host-example").unwrap(),
                session_credential_handle: CredentialHandle::new("vault:leserpentd:host-example")
                    .unwrap(),
                trust_credential_handle: CredentialHandle::new("vault:leserpent-ca:host-example")
                    .unwrap(),
                authority_owned: true,
                protocol_schema_version: BOOTSTRAP_SESSION_PROTOCOL_VERSION,
            },
        )
        .unwrap();
}
