use std::sync::Arc;

use leserpent_domain::bootstrap::BootstrapTransport;
use leserpent_domain::provisioning::{GewyvernServiceReceipt, RuntimeProvisioning};
use leserpent_protocol::provisioning::{
    PROVISIONING_PROTOCOL_SCHEMA_VERSION, ProvisioningRequestEnvelope, ProvisioningResponse,
    ProvisioningResponseEnvelope, decode_provisioning_request, encode_provisioning_response,
};
use leserpent_runtime::EffectExecution;

use crate::{EffectAdapter, SecretKey, SecretStore, SecretValue};

pub const GEWYVERN_PROVISIONING_EFFECT_KIND: &str = "gewyvern.runtime.provision";

pub struct GewyvernProvisioningJob<'a> {
    pub request: &'a ProvisioningRequestEnvelope,
    pub install_credential: &'a SecretValue,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GewyvernProvisioningTransportError {
    Authentication,
    HostKeyRejected,
    InstallerRejected,
    ServiceUnavailable,
    Transport,
}

pub trait GewyvernProvisioningTransport: Send {
    fn provision(
        &mut self,
        job: GewyvernProvisioningJob<'_>,
    ) -> Result<GewyvernServiceReceipt, GewyvernProvisioningTransportError>;
}

pub struct GewyvernProvisioningAdapter<T> {
    secrets: Arc<dyn SecretStore>,
    transport: T,
}

impl<T: GewyvernProvisioningTransport> GewyvernProvisioningAdapter<T> {
    pub fn new(secrets: Arc<dyn SecretStore>, transport: T) -> Self {
        Self { secrets, transport }
    }

    fn execute_request(&mut self, payload: &[u8]) -> Result<Vec<u8>, &'static str> {
        let request =
            decode_provisioning_request(payload).map_err(|_| "invalid provisioning payload")?;
        let mut provisioning = RuntimeProvisioning::plan(
            &request.request.principal,
            &request.request.capabilities,
            request.request.intent.clone(),
        )
        .map_err(|_| "invalid provisioning authorization")?;
        provisioning
            .begin()
            .map_err(|_| "invalid provisioning state")?;

        let intent = &request.request.intent;
        if intent.target.transport != BootstrapTransport::Ssh {
            return encode_failed(provisioning, "transport_not_supported");
        }
        let (provider, key) = intent.install_credential_handle.parts();
        if provider != "ssh" {
            return encode_failed(provisioning, "credential_provider_invalid");
        }
        let install_credential = match load_secret(self.secrets.as_ref(), key) {
            Ok(secret) => secret,
            Err(code) => return encode_failed(provisioning, code),
        };
        let receipt = match self.transport.provision(GewyvernProvisioningJob {
            request: &request,
            install_credential: &install_credential,
        }) {
            Ok(receipt) => receipt,
            Err(GewyvernProvisioningTransportError::Authentication) => {
                return encode_failed(provisioning, "authentication_failed");
            }
            Err(GewyvernProvisioningTransportError::HostKeyRejected) => {
                return encode_failed(provisioning, "host_key_rejected");
            }
            Err(GewyvernProvisioningTransportError::InstallerRejected) => {
                return encode_failed(provisioning, "installer_rejected");
            }
            Err(GewyvernProvisioningTransportError::ServiceUnavailable) => {
                return encode_failed(provisioning, "service_unavailable");
            }
            Err(GewyvernProvisioningTransportError::Transport) => {
                return encode_failed(provisioning, "transport_failure");
            }
        };
        let snapshot = match provisioning.accept_service(receipt) {
            Ok(snapshot) => snapshot,
            Err(_) => return encode_failed(provisioning, "service_identity_mismatch"),
        };
        encode_state(snapshot)
    }
}

impl<T: GewyvernProvisioningTransport> EffectAdapter for GewyvernProvisioningAdapter<T> {
    fn kind(&self) -> &str {
        GEWYVERN_PROVISIONING_EFFECT_KIND
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
    mut provisioning: RuntimeProvisioning,
    code: &'static str,
) -> Result<Vec<u8>, &'static str> {
    let snapshot = provisioning
        .record_fault(code)
        .map_err(|_| "invalid provisioning failure state")?;
    encode_state(snapshot)
}

fn encode_state(
    snapshot: leserpent_domain::provisioning::RuntimeProvisioningSnapshot,
) -> Result<Vec<u8>, &'static str> {
    encode_provisioning_response(&ProvisioningResponseEnvelope {
        schema_version: PROVISIONING_PROTOCOL_SCHEMA_VERSION,
        response: ProvisioningResponse::State(snapshot),
    })
    .map_err(|_| "provisioning response encoding failed")
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use leserpent_domain::RuntimeId;
    use leserpent_domain::bootstrap::{BootstrapTarget, CredentialHandle};
    use leserpent_domain::provisioning::{
        CAPABILITY_RUNTIME_PROVISION, PROVISIONING_DOMAIN_SCHEMA_VERSION, ProvisioningId,
        ProvisioningPhase, RuntimeProvisioningIntent,
    };
    use leserpent_domain::{CapabilitySet, Principal};
    use leserpent_protocol::provisioning::{
        ProvisioningRequest, decode_provisioning_response, encode_provisioning_request,
    };

    use super::*;
    use crate::{ConfiguredSecretStore, SecretValue};

    struct RecordingTransport {
        seen_secret: Arc<Mutex<Option<String>>>,
        fail: Option<GewyvernProvisioningTransportError>,
    }

    impl GewyvernProvisioningTransport for RecordingTransport {
        fn provision(
            &mut self,
            job: GewyvernProvisioningJob<'_>,
        ) -> Result<GewyvernServiceReceipt, GewyvernProvisioningTransportError> {
            *self.seen_secret.lock().unwrap() =
                Some(job.install_credential.expose_secret().to_string());
            if let Some(error) = self.fail {
                return Err(error);
            }
            Ok(GewyvernServiceReceipt {
                provisioning_id: job.request.request.intent.provisioning_id.clone(),
                runtime_id: job.request.request.intent.runtime_id.clone(),
                endpoint: "https://runtime.example:9443".into(),
                api_credential_handle: CredentialHandle::new("vault:gewyvern:runtime-api").unwrap(),
                trust_credential_handle: CredentialHandle::new("vault:gewyvern:runtime-ca")
                    .unwrap(),
            })
        }
    }

    fn request() -> ProvisioningRequestEnvelope {
        ProvisioningRequestEnvelope {
            schema_version: PROVISIONING_PROTOCOL_SCHEMA_VERSION,
            request: ProvisioningRequest {
                principal: Principal {
                    id: "operator-a".into(),
                },
                capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_PROVISION]),
                intent: RuntimeProvisioningIntent {
                    schema_version: PROVISIONING_DOMAIN_SCHEMA_VERSION,
                    provisioning_id: ProvisioningId::new("provision-1").unwrap(),
                    runtime_id: RuntimeId::new("runtime-a").unwrap(),
                    target: BootstrapTarget {
                        transport: BootstrapTransport::Ssh,
                        host: "host.example".into(),
                        port: 22,
                    },
                    install_credential_handle: CredentialHandle::new("vault:ssh:host-example")
                        .unwrap(),
                    requested_by: "operator-a".into(),
                    confirmed: true,
                },
            },
        }
    }

    fn execute(
        fail: Option<GewyvernProvisioningTransportError>,
    ) -> (
        leserpent_domain::provisioning::RuntimeProvisioningSnapshot,
        Option<String>,
        String,
    ) {
        let seen_secret = Arc::new(Mutex::new(None));
        let secrets = Arc::new(
            ConfiguredSecretStore::new([(
                SecretKey::new("host-example").unwrap(),
                SecretValue::new("install-password").unwrap(),
            )])
            .unwrap(),
        );
        let mut adapter = GewyvernProvisioningAdapter::new(
            secrets,
            RecordingTransport {
                seen_secret: Arc::clone(&seen_secret),
                fail,
            },
        );
        let payload = encode_provisioning_request(&request()).unwrap();
        let EffectExecution::Complete(outcome) = adapter.execute(&payload) else {
            panic!("valid provisioning must return a typed state");
        };
        let encoded = String::from_utf8(outcome.clone()).unwrap();
        let response = decode_provisioning_response(&outcome).unwrap();
        let ProvisioningResponse::State(state) = response.response else {
            panic!("adapter must return a state response");
        };
        let secret = seen_secret.lock().unwrap().clone();
        (state, secret, encoded)
    }

    #[test]
    fn adapter_resolves_secret_and_returns_service_ready_without_raw_authority() {
        let (state, secret, encoded) = execute(None);
        assert_eq!(state.phase, ProvisioningPhase::ServiceReady);
        assert!(!state.install_credential_present);
        assert_eq!(secret.as_deref(), Some("install-password"));
        assert!(!encoded.contains("install-password"));
    }

    #[test]
    fn transport_failure_returns_a_sanitized_terminal_state() {
        let (state, _, encoded) = execute(Some(GewyvernProvisioningTransportError::Transport));
        assert_eq!(state.phase, ProvisioningPhase::Failed);
        assert_eq!(state.fault_code.as_deref(), Some("transport_failure"));
        assert!(!encoded.contains("install-password"));
    }
}
