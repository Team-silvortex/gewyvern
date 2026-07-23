#[cfg(feature = "native-ssh")]
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;
#[cfg(feature = "native-ssh")]
use std::time::Duration;

use leserpent_domain::RuntimeId;
use leserpent_domain::bootstrap::{BootstrapTarget, BootstrapTransport, CredentialHandle};
use leserpent_domain::provisioning::{GewyvernServiceReceipt, RuntimeProvisioning};
#[cfg(feature = "native-ssh")]
use leserpent_protocol::gewyvern_installer::{
    GewyvernInstallerRequest, GewyvernInstallerResponse, GewyvernInstallerServiceState,
    MAX_GEWYVERN_INSTALLER_BYTES, decode_gewyvern_installer_response,
    encode_gewyvern_installer_request, validate_gewyvern_installer_readiness,
    validate_gewyvern_installer_response_binding,
};
use leserpent_protocol::provisioning::{
    PROVISIONING_PROTOCOL_SCHEMA_VERSION, ProvisioningRequestEnvelope, ProvisioningResponse,
    ProvisioningResponseEnvelope, decode_provisioning_request, encode_provisioning_response,
};
use leserpent_runtime::EffectExecution;
use ring::digest::{SHA256, digest};

#[cfg(feature = "native-ssh")]
use crate::bootstrap::target_key;
use crate::bootstrap::{valid_sha256_fingerprint, valid_staging_prefix, validate_https_origin};
#[cfg(feature = "native-ssh")]
use crate::native_ssh::{NativeSshClient, NativeSshError, NativeSshJob};
#[cfg(feature = "native-ssh")]
use crate::{BootstrapTrustRecord, BootstrapTrustStore};
use crate::{EffectAdapter, SecretKey, SecretStore, SecretValue, validate_id};

pub const GEWYVERN_PROVISIONING_EFFECT_KIND: &str = "gewyvern.runtime.provision";
pub const MAX_GEWYVERN_ARTIFACT_BYTES: usize = 256 * 1024 * 1024;

#[derive(Clone)]
pub struct GewyvernArtifact {
    bytes: Arc<[u8]>,
    sha256_hex: String,
    staging_prefix: String,
}

impl GewyvernArtifact {
    pub fn new(
        bytes: impl Into<Arc<[u8]>>,
        staging_prefix: impl Into<String>,
    ) -> Result<Self, String> {
        let bytes = bytes.into();
        if bytes.is_empty() || bytes.len() > MAX_GEWYVERN_ARTIFACT_BYTES {
            return Err("Gewyvern artifact size is invalid".into());
        }
        let staging_prefix = staging_prefix.into();
        if !valid_staging_prefix(&staging_prefix) {
            return Err("Gewyvern artifact staging prefix is invalid".into());
        }
        Ok(Self {
            sha256_hex: hex(digest(&SHA256, &bytes).as_ref()),
            bytes,
            staging_prefix,
        })
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn sha256_hex(&self) -> &str {
        &self.sha256_hex
    }

    pub fn staging_prefix(&self) -> &str {
        &self.staging_prefix
    }

    #[cfg(feature = "native-ssh")]
    pub(crate) fn staging_path_for(&self, operation_id: &str) -> String {
        format!("{}-{operation_id}.stage", self.staging_prefix)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SshGewyvernHostPolicy {
    pub(crate) target: BootstrapTarget,
    pub(crate) runtime_id: RuntimeId,
    pub(crate) username: String,
    pub(crate) host_key_sha256: String,
    endpoint: String,
    api_credential_handle: CredentialHandle,
    trust_credential_handle: CredentialHandle,
    pub(crate) install_profile: String,
}

impl SshGewyvernHostPolicy {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        target: BootstrapTarget,
        runtime_id: RuntimeId,
        username: impl Into<String>,
        host_key_sha256: impl Into<String>,
        endpoint: impl Into<String>,
        api_credential_handle: CredentialHandle,
        trust_credential_handle: CredentialHandle,
        install_profile: impl Into<String>,
    ) -> Result<Self, String> {
        target.validate().map_err(|error| error.to_string())?;
        if target.transport != BootstrapTransport::Ssh {
            return Err("Gewyvern host policy requires an SSH target".into());
        }
        let username = username.into();
        validate_id("SSH username", &username)?;
        let host_key_sha256 = host_key_sha256.into();
        if !valid_sha256_fingerprint(&host_key_sha256) {
            return Err("SSH host key fingerprint must be pinned as SHA256".into());
        }
        let endpoint = endpoint.into();
        validate_https_origin(&endpoint)?;
        if api_credential_handle.parts().0 != "gewyvern" {
            return Err("API credential handle must use the gewyvern vault provider".into());
        }
        if trust_credential_handle.parts().0 != "gewyvern-ca" {
            return Err("trust credential handle must use the gewyvern-ca vault provider".into());
        }
        let install_profile = install_profile.into();
        if !matches!(install_profile.as_str(), "system" | "user") {
            return Err("Gewyvern install profile must be system or user".into());
        }
        Ok(Self {
            target,
            runtime_id,
            username,
            host_key_sha256,
            endpoint,
            api_credential_handle,
            trust_credential_handle,
            install_profile,
        })
    }

    #[cfg(feature = "native-ssh")]
    fn key(&self) -> String {
        gewyvern_target_key(&self.target, &self.runtime_id)
    }
}

pub struct GewyvernProvisioningJob<'a> {
    pub request: &'a ProvisioningRequestEnvelope,
    pub install_credential: &'a SecretValue,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GewyvernProvisioningTransportError {
    Authentication,
    CredentialUnavailable,
    HostKeyRejected,
    InstallerRejected,
    InvalidResponse,
    ServiceUnavailable,
    Transport,
    TrustPersistence,
    UploadRejected,
}

impl fmt::Display for GewyvernProvisioningTransportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Authentication => "SSH authentication failed",
            Self::CredentialUnavailable => "Gewyvern API credential is unavailable",
            Self::HostKeyRejected => "SSH host key was rejected",
            Self::InstallerRejected => "remote Gewyvern installer rejected the request",
            Self::InvalidResponse => "remote Gewyvern installer returned an invalid response",
            Self::ServiceUnavailable => "Gewyvern service is not ready",
            Self::Transport => "SSH transport failed",
            Self::TrustPersistence => "Gewyvern trust persistence failed",
            Self::UploadRejected => "SSH artifact upload failed",
        })
    }
}

impl std::error::Error for GewyvernProvisioningTransportError {}

#[cfg(feature = "native-ssh")]
pub struct NativeSshGewyvernProvisioningTransport {
    policies: BTreeMap<String, SshGewyvernHostPolicy>,
    secrets: Arc<dyn SecretStore>,
    trust: Arc<dyn BootstrapTrustStore>,
    artifact: GewyvernArtifact,
    client: NativeSshClient,
}

#[cfg(feature = "native-ssh")]
impl NativeSshGewyvernProvisioningTransport {
    pub fn new(
        policies: impl IntoIterator<Item = SshGewyvernHostPolicy>,
        secrets: Arc<dyn SecretStore>,
        trust: Arc<dyn BootstrapTrustStore>,
        artifact: GewyvernArtifact,
    ) -> Result<Self, String> {
        let mut normalized = BTreeMap::new();
        for policy in policies {
            if normalized.insert(policy.key(), policy).is_some() {
                return Err("duplicate SSH Gewyvern host policy".into());
            }
        }
        if normalized.is_empty() {
            return Err("at least one SSH Gewyvern host policy is required".into());
        }
        Ok(Self {
            policies: normalized,
            secrets,
            trust,
            artifact,
            client: NativeSshClient::default(),
        })
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Result<Self, String> {
        self.client = NativeSshClient::with_timeout(timeout)?;
        Ok(self)
    }
}

pub trait GewyvernProvisioningTransport: Send {
    fn provision(
        &mut self,
        job: GewyvernProvisioningJob<'_>,
    ) -> Result<GewyvernServiceReceipt, GewyvernProvisioningTransportError>;
}

#[cfg(feature = "native-ssh")]
impl GewyvernProvisioningTransport for NativeSshGewyvernProvisioningTransport {
    fn provision(
        &mut self,
        job: GewyvernProvisioningJob<'_>,
    ) -> Result<GewyvernServiceReceipt, GewyvernProvisioningTransportError> {
        let intent = &job.request.request.intent;
        let policy = self
            .policies
            .get(&gewyvern_target_key(&intent.target, &intent.runtime_id))
            .ok_or(GewyvernProvisioningTransportError::InstallerRejected)?;
        if policy.target != intent.target || policy.runtime_id != intent.runtime_id {
            return Err(GewyvernProvisioningTransportError::InstallerRejected);
        }
        let (_, api_key) = policy.api_credential_handle.parts();
        let api_token = load_secret(self.secrets.as_ref(), api_key)
            .map_err(|_| GewyvernProvisioningTransportError::CredentialUnavailable)?;
        let request = GewyvernInstallerRequest::new(
            intent.provisioning_id.clone(),
            intent.runtime_id.clone(),
            &policy.endpoint,
            &policy.install_profile,
            self.artifact.sha256_hex(),
            policy.api_credential_handle.clone(),
            policy.trust_credential_handle.clone(),
            api_token.expose_secret(),
        )
        .map_err(|_| GewyvernProvisioningTransportError::InstallerRejected)?;
        let payload = encode_gewyvern_installer_request(&request)
            .map_err(|_| GewyvernProvisioningTransportError::InstallerRejected)?;
        let staging_path = self
            .artifact
            .staging_path_for(intent.provisioning_id.as_str());
        let command = format!("{staging_path} gewyvern-activate-v1");
        let stdout = self
            .client
            .execute(NativeSshJob {
                host: &intent.target.host,
                port: intent.target.port,
                username: &policy.username,
                host_key_sha256: &policy.host_key_sha256,
                password: job.install_credential.expose_secret(),
                staging_path: &staging_path,
                artifact: self.artifact.bytes(),
                artifact_sha256: self.artifact.sha256_hex(),
                command: &command,
                stdin: &payload,
                max_stdout_bytes: MAX_GEWYVERN_INSTALLER_BYTES,
            })
            .map_err(map_native_ssh_error)?;
        let response = decode_gewyvern_installer_response(&stdout)
            .map_err(|_| GewyvernProvisioningTransportError::InvalidResponse)?;
        accept_installer_response(&request, response, self.trust.as_ref())
    }
}

#[cfg(feature = "native-ssh")]
fn map_native_ssh_error(error: NativeSshError) -> GewyvernProvisioningTransportError {
    match error {
        NativeSshError::Authentication => GewyvernProvisioningTransportError::Authentication,
        NativeSshError::HostKeyRejected => GewyvernProvisioningTransportError::HostKeyRejected,
        NativeSshError::Transport => GewyvernProvisioningTransportError::Transport,
        NativeSshError::UploadRejected => GewyvernProvisioningTransportError::UploadRejected,
        NativeSshError::CommandRejected => GewyvernProvisioningTransportError::InstallerRejected,
        NativeSshError::InvalidResponse => GewyvernProvisioningTransportError::InvalidResponse,
    }
}

#[cfg(feature = "native-ssh")]
fn accept_installer_response(
    request: &GewyvernInstallerRequest,
    response: GewyvernInstallerResponse,
    trust: &dyn BootstrapTrustStore,
) -> Result<GewyvernServiceReceipt, GewyvernProvisioningTransportError> {
    validate_gewyvern_installer_response_binding(request, &response)
        .map_err(|_| GewyvernProvisioningTransportError::InvalidResponse)?;
    if response.service_state == GewyvernInstallerServiceState::Installed {
        return Err(GewyvernProvisioningTransportError::ServiceUnavailable);
    }
    validate_gewyvern_installer_readiness(request, &response)
        .map_err(|_| GewyvernProvisioningTransportError::InvalidResponse)?;
    trust
        .persist(
            &response.trust_credential_handle,
            &BootstrapTrustRecord {
                endpoint: response.endpoint.clone(),
                ca_pem: response.tls_ca_pem.clone(),
                ca_sha256: response.tls_ca_sha256.clone(),
            },
        )
        .map_err(|_| GewyvernProvisioningTransportError::TrustPersistence)?;
    Ok(GewyvernServiceReceipt {
        provisioning_id: response.provisioning_id,
        runtime_id: response.runtime_id,
        endpoint: response.endpoint,
        api_credential_handle: response.api_credential_handle,
        trust_credential_handle: response.trust_credential_handle,
    })
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
            Err(GewyvernProvisioningTransportError::CredentialUnavailable) => {
                return encode_failed(provisioning, "runtime_credential_unavailable");
            }
            Err(GewyvernProvisioningTransportError::HostKeyRejected) => {
                return encode_failed(provisioning, "host_key_rejected");
            }
            Err(GewyvernProvisioningTransportError::InstallerRejected) => {
                return encode_failed(provisioning, "installer_rejected");
            }
            Err(GewyvernProvisioningTransportError::InvalidResponse) => {
                return encode_failed(provisioning, "installer_response_invalid");
            }
            Err(GewyvernProvisioningTransportError::ServiceUnavailable) => {
                return encode_failed(provisioning, "service_unavailable");
            }
            Err(GewyvernProvisioningTransportError::Transport) => {
                return encode_failed(provisioning, "transport_failure");
            }
            Err(GewyvernProvisioningTransportError::TrustPersistence) => {
                return encode_failed(provisioning, "trust_persistence_failed");
            }
            Err(GewyvernProvisioningTransportError::UploadRejected) => {
                return encode_failed(provisioning, "artifact_upload_rejected");
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

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

#[cfg(feature = "native-ssh")]
pub(crate) fn gewyvern_target_key(target: &BootstrapTarget, runtime_id: &RuntimeId) -> String {
    format!("{}#{}", target_key(target), runtime_id.as_str())
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
    #[cfg(feature = "native-ssh")]
    use crate::{BootstrapTrustError, BootstrapTrustRecord, BootstrapTrustStore};
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

    fn target() -> BootstrapTarget {
        request().request.intent.target
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

    #[test]
    fn artifact_and_host_policy_reject_unsafe_configuration() {
        assert!(GewyvernArtifact::new(Arc::<[u8]>::from([]), "/tmp/gewyvern").is_err());
        assert!(
            GewyvernArtifact::new(
                Arc::<[u8]>::from(b"gewyvern".as_slice()),
                "/tmp/../gewyvern"
            )
            .is_err()
        );
        assert!(
            SshGewyvernHostPolicy::new(
                target(),
                RuntimeId::new("runtime-a").unwrap(),
                "deployer",
                "accept-new",
                "https://runtime.example:9443",
                CredentialHandle::new("vault:gewyvern:runtime-api").unwrap(),
                CredentialHandle::new("vault:gewyvern-ca:runtime-ca").unwrap(),
                "system",
            )
            .is_err()
        );
        assert!(
            SshGewyvernHostPolicy::new(
                target(),
                RuntimeId::new("runtime-a").unwrap(),
                "deployer",
                "SHA256:abcdefghijklmnopqrstuvwxyz0123456789ABCDE",
                "https://runtime.example:9443",
                CredentialHandle::new("vault:gewyvern:runtime-api").unwrap(),
                CredentialHandle::new("vault:gewyvern-ca:runtime-ca").unwrap(),
                "test",
            )
            .is_err()
        );
    }

    #[cfg(feature = "native-ssh")]
    #[derive(Default)]
    struct RecordingTrustStore {
        records: Mutex<Vec<(String, BootstrapTrustRecord)>>,
        reject: bool,
    }

    #[cfg(feature = "native-ssh")]
    impl BootstrapTrustStore for RecordingTrustStore {
        fn persist(
            &self,
            handle: &CredentialHandle,
            record: &BootstrapTrustRecord,
        ) -> Result<(), BootstrapTrustError> {
            if self.reject {
                return Err(BootstrapTrustError::Storage);
            }
            self.records
                .lock()
                .unwrap()
                .push((handle.as_str().into(), record.clone()));
            Ok(())
        }
    }

    #[cfg(feature = "native-ssh")]
    fn installer_request() -> GewyvernInstallerRequest {
        let intent = request().request.intent;
        GewyvernInstallerRequest::new(
            intent.provisioning_id,
            intent.runtime_id,
            "https://runtime.example:9443",
            "system",
            "a".repeat(64),
            CredentialHandle::new("vault:gewyvern:runtime-api").unwrap(),
            CredentialHandle::new("vault:gewyvern-ca:runtime-ca").unwrap(),
            "0123456789abcdef0123456789abcdef",
        )
        .unwrap()
    }

    #[cfg(feature = "native-ssh")]
    fn installer_response(state: GewyvernInstallerServiceState) -> GewyvernInstallerResponse {
        let request = installer_request();
        let ca_pem = rcgen::generate_simple_self_signed(vec!["runtime.example".into()])
            .unwrap()
            .cert
            .pem();
        GewyvernInstallerResponse {
            schema_version:
                leserpent_protocol::gewyvern_installer::GEWYVERN_INSTALLER_SCHEMA_VERSION,
            provisioning_id: request.provisioning_id.clone(),
            runtime_id: request.runtime_id.clone(),
            endpoint: request.endpoint.clone(),
            service_state: state,
            generation: request.artifact_sha256.clone(),
            replayed: false,
            api_credential_handle: request.api_credential_handle.clone(),
            trust_credential_handle: request.trust_credential_handle.clone(),
            tls_ca_sha256: hex(digest(&SHA256, ca_pem.as_bytes()).as_ref()),
            tls_ca_pem: ca_pem,
        }
    }

    #[cfg(feature = "native-ssh")]
    #[test]
    fn installed_response_withholds_trust_and_service_receipt() {
        let trust = RecordingTrustStore::default();
        assert_eq!(
            accept_installer_response(
                &installer_request(),
                installer_response(GewyvernInstallerServiceState::Installed),
                &trust,
            ),
            Err(GewyvernProvisioningTransportError::ServiceUnavailable)
        );
        assert!(trust.records.lock().unwrap().is_empty());
    }

    #[cfg(feature = "native-ssh")]
    #[test]
    fn ready_response_persists_bound_trust_before_returning_receipt() {
        let trust = RecordingTrustStore::default();
        let receipt = accept_installer_response(
            &installer_request(),
            installer_response(GewyvernInstallerServiceState::Ready),
            &trust,
        )
        .unwrap();
        assert_eq!(receipt.runtime_id.as_str(), "runtime-a");
        let records = trust.records.lock().unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].0, "vault:gewyvern-ca:runtime-ca");
        assert_eq!(records[0].1.endpoint, receipt.endpoint);

        let rejecting = RecordingTrustStore {
            records: Mutex::new(Vec::new()),
            reject: true,
        };
        assert_eq!(
            accept_installer_response(
                &installer_request(),
                installer_response(GewyvernInstallerServiceState::Ready),
                &rejecting,
            ),
            Err(GewyvernProvisioningTransportError::TrustPersistence)
        );
    }

    #[cfg(feature = "native-ssh")]
    #[test]
    fn response_identity_drift_fails_before_trust_persistence() {
        let trust = RecordingTrustStore::default();
        let mut response = installer_response(GewyvernInstallerServiceState::Ready);
        response.runtime_id = RuntimeId::new("runtime-attacker").unwrap();
        assert_eq!(
            accept_installer_response(&installer_request(), response, &trust),
            Err(GewyvernProvisioningTransportError::InvalidResponse)
        );
        assert!(trust.records.lock().unwrap().is_empty());
    }
}
