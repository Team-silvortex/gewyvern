use std::sync::Arc;

use leserpent_domain::bootstrap::BootstrapTransport;
use leserpent_domain::retirement::{
    GewyvernRetirementReceipt, RuntimeRetirement, RuntimeRetirementSnapshot,
};
use leserpent_protocol::retirement::{
    RETIREMENT_PROTOCOL_SCHEMA_VERSION, RetirementRequestEnvelope, RetirementResponse,
    RetirementResponseEnvelope, decode_retirement_request, encode_retirement_response,
};
use leserpent_runtime::EffectExecution;

use crate::{EffectAdapter, SecretKey, SecretStore, SecretValue};

pub const GEWYVERN_RETIREMENT_EFFECT_KIND: &str = "gewyvern.runtime.retire";

pub struct GewyvernRetirementJob<'a> {
    pub request: &'a RetirementRequestEnvelope,
    pub retirement_credential: &'a SecretValue,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GewyvernRetirementTransportError {
    Authentication,
    HostKeyRejected,
    ServiceRejected,
    InvalidResponse,
    Transport,
}

pub trait GewyvernRetirementTransport: Send {
    fn retire(
        &mut self,
        job: GewyvernRetirementJob<'_>,
    ) -> Result<GewyvernRetirementReceipt, GewyvernRetirementTransportError>;
}

pub struct GewyvernRetirementAdapter<T> {
    secrets: Arc<dyn SecretStore>,
    transport: T,
}

impl<T: GewyvernRetirementTransport> GewyvernRetirementAdapter<T> {
    pub fn new(secrets: Arc<dyn SecretStore>, transport: T) -> Self {
        Self { secrets, transport }
    }

    fn execute_request(&mut self, payload: &[u8]) -> Result<Vec<u8>, &'static str> {
        let request =
            decode_retirement_request(payload).map_err(|_| "invalid retirement payload")?;
        let mut retirement = RuntimeRetirement::plan(
            &request.request.principal,
            &request.request.capabilities,
            request.request.intent.clone(),
        )
        .map_err(|_| "invalid retirement authorization")?;
        retirement.begin().map_err(|_| "invalid retirement state")?;

        let intent = &request.request.intent;
        if intent.target.transport != BootstrapTransport::Ssh {
            return encode_failed(retirement, "transport_not_supported");
        }
        let (provider, key) = intent.retirement_credential_handle.parts();
        if provider != "ssh" {
            return encode_failed(retirement, "credential_provider_invalid");
        }
        let retirement_credential = match load_secret(self.secrets.as_ref(), key) {
            Ok(secret) => secret,
            Err(code) => return encode_failed(retirement, code),
        };
        let receipt = match self.transport.retire(GewyvernRetirementJob {
            request: &request,
            retirement_credential: &retirement_credential,
        }) {
            Ok(receipt) => receipt,
            Err(GewyvernRetirementTransportError::Authentication) => {
                return encode_failed(retirement, "authentication_failed");
            }
            Err(GewyvernRetirementTransportError::HostKeyRejected) => {
                return encode_failed(retirement, "host_key_rejected");
            }
            Err(GewyvernRetirementTransportError::ServiceRejected) => {
                return encode_failed(retirement, "service_retirement_rejected");
            }
            Err(GewyvernRetirementTransportError::InvalidResponse) => {
                return encode_failed(retirement, "retirement_response_invalid");
            }
            Err(GewyvernRetirementTransportError::Transport) => {
                return encode_failed(retirement, "transport_failure");
            }
        };
        let snapshot = match retirement.accept_service_retirement(receipt) {
            Ok(snapshot) => snapshot,
            Err(_) => return encode_failed(retirement, "service_identity_mismatch"),
        };
        encode_state(snapshot)
    }
}

impl<T: GewyvernRetirementTransport> EffectAdapter for GewyvernRetirementAdapter<T> {
    fn kind(&self) -> &str {
        GEWYVERN_RETIREMENT_EFFECT_KIND
    }

    fn execute(&mut self, payload: &[u8]) -> EffectExecution {
        match self.execute_request(payload) {
            Ok(response) => EffectExecution::Complete(response),
            Err(error) => EffectExecution::Reject {
                error: error.into(),
            },
        }
    }
}

fn load_secret(store: &dyn SecretStore, key: &str) -> Result<SecretValue, &'static str> {
    let key = SecretKey::new(key).map_err(|_| "credential_handle_invalid")?;
    store
        .load(&key)
        .map_err(|_| "credential_store_unavailable")?
        .ok_or("credential_not_found")
}

fn encode_failed(
    mut retirement: RuntimeRetirement,
    code: &'static str,
) -> Result<Vec<u8>, &'static str> {
    let snapshot = retirement
        .record_fault(code)
        .map_err(|_| "invalid retirement failure state")?;
    encode_state(snapshot)
}

fn encode_state(snapshot: RuntimeRetirementSnapshot) -> Result<Vec<u8>, &'static str> {
    encode_retirement_response(&RetirementResponseEnvelope {
        schema_version: RETIREMENT_PROTOCOL_SCHEMA_VERSION,
        response: RetirementResponse::State(snapshot),
    })
    .map_err(|_| "retirement response encoding failed")
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use leserpent_domain::RuntimeId;
    use leserpent_domain::bootstrap::{BootstrapTarget, BootstrapTransport, CredentialHandle};
    use leserpent_domain::provisioning::ProvisioningId;
    use leserpent_domain::retirement::{
        CAPABILITY_RUNTIME_RETIRE, RETIREMENT_DOMAIN_SCHEMA_VERSION, RetirementId, RetirementPhase,
        RuntimeRetirementIntent,
    };
    use leserpent_domain::{CapabilitySet, Principal};
    use leserpent_protocol::retirement::{
        RetirementRequest, decode_retirement_response, encode_retirement_request,
    };

    use super::*;
    use crate::{ConfiguredSecretStore, SecretValue};

    struct RecordingTransport {
        seen_secret: Arc<Mutex<Option<String>>>,
        fail: Option<GewyvernRetirementTransportError>,
    }

    impl GewyvernRetirementTransport for RecordingTransport {
        fn retire(
            &mut self,
            job: GewyvernRetirementJob<'_>,
        ) -> Result<GewyvernRetirementReceipt, GewyvernRetirementTransportError> {
            *self.seen_secret.lock().unwrap() =
                Some(job.retirement_credential.expose_secret().to_string());
            if let Some(error) = self.fail {
                return Err(error);
            }
            Ok(GewyvernRetirementReceipt {
                retirement_id: job.request.request.intent.retirement_id.clone(),
                provisioning_id: job.request.request.intent.provisioning_id.clone(),
                runtime_id: job.request.request.intent.runtime_id.clone(),
                service_retired: true,
            })
        }
    }

    fn request() -> RetirementRequestEnvelope {
        RetirementRequestEnvelope {
            schema_version: RETIREMENT_PROTOCOL_SCHEMA_VERSION,
            request: RetirementRequest {
                principal: Principal {
                    id: "operator-a".into(),
                },
                capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_RETIRE]),
                intent: RuntimeRetirementIntent {
                    schema_version: RETIREMENT_DOMAIN_SCHEMA_VERSION,
                    retirement_id: RetirementId::new("retire-a").unwrap(),
                    provisioning_id: ProvisioningId::new("provision-a").unwrap(),
                    runtime_id: RuntimeId::new("runtime-a").unwrap(),
                    target: BootstrapTarget {
                        transport: BootstrapTransport::Ssh,
                        host: "runtime.example".into(),
                        port: 22,
                    },
                    retirement_credential_handle: CredentialHandle::new("vault:ssh:runtime-a")
                        .unwrap(),
                    requested_by: "operator-a".into(),
                    confirmed: true,
                },
            },
        }
    }

    #[test]
    fn adapter_resolves_secret_only_at_transport_boundary() {
        let seen_secret = Arc::new(Mutex::new(None));
        let store = Arc::new(
            ConfiguredSecretStore::new([(
                SecretKey::new("runtime-a").unwrap(),
                SecretValue::new("retirement-password").unwrap(),
            )])
            .unwrap(),
        );
        let mut adapter = GewyvernRetirementAdapter::new(
            store,
            RecordingTransport {
                seen_secret: seen_secret.clone(),
                fail: None,
            },
        );
        let payload = encode_retirement_request(&request()).unwrap();
        let EffectExecution::Complete(response) = adapter.execute(&payload) else {
            panic!("retirement adapter must complete");
        };
        assert_eq!(
            seen_secret.lock().unwrap().as_deref(),
            Some("retirement-password")
        );
        assert!(
            !String::from_utf8(response.clone())
                .unwrap()
                .contains("retirement-password")
        );
        let decoded = decode_retirement_response(&response).unwrap();
        let RetirementResponse::State(state) = decoded.response else {
            panic!("retirement adapter must return state");
        };
        assert_eq!(state.phase, RetirementPhase::ServiceRetired);
        assert!(!state.retirement_credential_present);
    }

    #[test]
    fn transport_failure_returns_terminal_state_and_preserves_registration() {
        let store = Arc::new(
            ConfiguredSecretStore::new([(
                SecretKey::new("runtime-a").unwrap(),
                SecretValue::new("retirement-password").unwrap(),
            )])
            .unwrap(),
        );
        let mut adapter = GewyvernRetirementAdapter::new(
            store,
            RecordingTransport {
                seen_secret: Arc::new(Mutex::new(None)),
                fail: Some(GewyvernRetirementTransportError::Transport),
            },
        );
        let EffectExecution::Complete(response) =
            adapter.execute(&encode_retirement_request(&request()).unwrap())
        else {
            panic!("operational failure must be a terminal domain response");
        };
        let decoded = decode_retirement_response(&response).unwrap();
        let RetirementResponse::State(state) = decoded.response else {
            panic!("retirement adapter must return state");
        };
        assert_eq!(state.phase, RetirementPhase::Failed);
        assert!(state.runtime_registered);
        assert!(!state.retirement_credential_present);
    }
}
