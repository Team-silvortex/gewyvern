use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;
#[cfg(feature = "native-ssh")]
use std::time::Duration;

use leserpent_domain::bootstrap::{
    BootstrapId, BootstrapPhase, BootstrapTarget, BootstrapTransport, CredentialHandle,
    DaemonBootstrapReceipt, DaemonId, DeploymentBootstrap,
};
use leserpent_protocol::bootstrap::{
    BOOTSTRAP_PROTOCOL_SCHEMA_VERSION, BootstrapRequestEnvelope, BootstrapResponse,
    BootstrapResponseEnvelope, decode_bootstrap_request, encode_bootstrap_response,
};
#[cfg(feature = "native-ssh")]
use leserpent_protocol::bootstrap_installer::{
    BootstrapInstallerRequest, BootstrapInstallerServiceState, MAX_BOOTSTRAP_INSTALLER_BYTES,
    decode_bootstrap_installer_response, encode_bootstrap_installer_request,
};
use leserpent_runtime::EffectExecution;
use ring::digest::{SHA256, digest};

#[cfg(feature = "native-ssh")]
use crate::native_ssh::{NativeSshClient, NativeSshError, NativeSshJob};
use crate::{
    BootstrapTrustRecord, BootstrapTrustStore, EffectAdapter, SecretKey, SecretStore, SecretValue,
    validate_id,
};

pub const HOST_BOOTSTRAP_EFFECT_KIND: &str = "leserpent.host.bootstrap";
pub const MAX_BOOTSTRAP_ARTIFACT_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone)]
pub struct BootstrapArtifact {
    bytes: Arc<[u8]>,
    sha256_hex: String,
    staging_prefix: String,
}

impl BootstrapArtifact {
    pub fn new(
        bytes: impl Into<Arc<[u8]>>,
        staging_prefix: impl Into<String>,
    ) -> Result<Self, String> {
        let bytes = bytes.into();
        if bytes.is_empty() || bytes.len() > MAX_BOOTSTRAP_ARTIFACT_BYTES {
            return Err("bootstrap artifact size is invalid".into());
        }
        let staging_prefix = staging_prefix.into();
        if !valid_staging_prefix(&staging_prefix) {
            return Err("bootstrap artifact staging prefix is invalid".into());
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
    fn staging_path(&self, bootstrap_id: &BootstrapId) -> String {
        format!("{}-{}.stage", self.staging_prefix, bootstrap_id.as_str())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SshBootstrapHostPolicy {
    target: BootstrapTarget,
    username: String,
    host_key_sha256: String,
    daemon_id: DaemonId,
    endpoint: String,
    session_credential_handle: CredentialHandle,
    trust_credential_handle: CredentialHandle,
    install_profile: String,
}

impl SshBootstrapHostPolicy {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        target: BootstrapTarget,
        username: impl Into<String>,
        host_key_sha256: impl Into<String>,
        daemon_id: DaemonId,
        endpoint: impl Into<String>,
        session_credential_handle: CredentialHandle,
        trust_credential_handle: CredentialHandle,
        install_profile: impl Into<String>,
    ) -> Result<Self, String> {
        target.validate().map_err(|error| error.to_string())?;
        if target.transport != BootstrapTransport::Ssh {
            return Err("SSH bootstrap policy requires an SSH target".into());
        }
        let username = username.into();
        validate_id("SSH username", &username)?;
        let host_key_sha256 = host_key_sha256.into();
        if !valid_sha256_fingerprint(&host_key_sha256) {
            return Err("SSH host key fingerprint must be pinned as SHA256".into());
        }
        let endpoint = endpoint.into();
        validate_https_origin(&endpoint)?;
        let install_profile = install_profile.into();
        validate_id("bootstrap install profile", &install_profile)?;
        if session_credential_handle.parts().0 != "leserpentd" {
            return Err("session credential handle must use the leserpentd vault provider".into());
        }
        if trust_credential_handle.parts().0 != "leserpent-ca" {
            return Err("trust credential handle must use the leserpent-ca vault provider".into());
        }
        Ok(Self {
            target,
            username,
            host_key_sha256,
            daemon_id,
            endpoint,
            session_credential_handle,
            trust_credential_handle,
            install_profile,
        })
    }

    fn key(&self) -> String {
        target_key(&self.target)
    }
}

pub struct SshBootstrapJob<'a> {
    pub bootstrap_id: &'a BootstrapId,
    pub target: &'a BootstrapTarget,
    pub username: &'a str,
    pub host_key_sha256: &'a str,
    pub bootstrap_password: &'a SecretValue,
    pub session_token: &'a SecretValue,
    pub artifact: &'a BootstrapArtifact,
    pub daemon_id: &'a DaemonId,
    pub endpoint: &'a str,
    pub install_profile: &'a str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SshBootstrapOutcome {
    pub daemon_id: DaemonId,
    pub endpoint: String,
    pub tls_ca_pem: String,
    pub tls_ca_sha256: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SshBootstrapTransportError {
    Authentication,
    HostKeyRejected,
    Transport,
    UploadRejected,
    InstallerRejected,
    InvalidResponse,
}

impl fmt::Display for SshBootstrapTransportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Authentication => "SSH authentication failed",
            Self::HostKeyRejected => "SSH host key was rejected",
            Self::Transport => "SSH transport failed",
            Self::UploadRejected => "SSH artifact upload failed",
            Self::InstallerRejected => "remote bootstrap installer rejected the request",
            Self::InvalidResponse => "remote bootstrap installer returned an invalid response",
        })
    }
}

impl std::error::Error for SshBootstrapTransportError {}

pub trait SshBootstrapTransport: Send {
    fn deploy(
        &mut self,
        job: SshBootstrapJob<'_>,
    ) -> Result<SshBootstrapOutcome, SshBootstrapTransportError>;
}

pub struct SshBootstrapAdapter<T> {
    policies: BTreeMap<String, SshBootstrapHostPolicy>,
    secrets: Arc<dyn SecretStore>,
    trust: Arc<dyn BootstrapTrustStore>,
    artifact: BootstrapArtifact,
    transport: T,
}

impl<T: SshBootstrapTransport> SshBootstrapAdapter<T> {
    pub fn new(
        policies: impl IntoIterator<Item = SshBootstrapHostPolicy>,
        secrets: Arc<dyn SecretStore>,
        trust: Arc<dyn BootstrapTrustStore>,
        artifact: BootstrapArtifact,
        transport: T,
    ) -> Result<Self, String> {
        let mut normalized = BTreeMap::new();
        for policy in policies {
            if normalized.insert(policy.key(), policy).is_some() {
                return Err("duplicate SSH bootstrap host policy".into());
            }
        }
        if normalized.is_empty() {
            return Err("at least one SSH bootstrap host policy is required".into());
        }
        Ok(Self {
            policies: normalized,
            secrets,
            trust,
            artifact,
            transport,
        })
    }

    fn execute_request(&mut self, payload: &[u8]) -> Result<Vec<u8>, &'static str> {
        let envelope =
            decode_bootstrap_request(payload).map_err(|_| "invalid bootstrap payload")?;
        let BootstrapRequestEnvelope { request, .. } = envelope;
        let mut bootstrap = DeploymentBootstrap::plan(
            &request.principal,
            &request.capabilities,
            request.intent.clone(),
        )
        .map_err(|_| "invalid bootstrap authorization")?;
        bootstrap.begin().map_err(|_| "invalid bootstrap state")?;

        let intent = &request.intent;
        if intent.target.transport != BootstrapTransport::Ssh {
            return encode_failed(bootstrap, "transport_not_supported");
        }
        let Some(policy) = self.policies.get(&target_key(&intent.target)) else {
            return encode_failed(bootstrap, "target_policy_missing");
        };
        if policy.target != intent.target {
            return encode_failed(bootstrap, "target_policy_mismatch");
        }
        let (provider, bootstrap_key) = intent.credential_handle.parts();
        if provider != "ssh" {
            return encode_failed(bootstrap, "credential_provider_invalid");
        }
        let bootstrap_password = match load_secret(self.secrets.as_ref(), bootstrap_key) {
            Ok(secret) => secret,
            Err(code) => return encode_failed(bootstrap, code),
        };
        let (_, session_key) = policy.session_credential_handle.parts();
        let session_token = match load_secret(self.secrets.as_ref(), session_key) {
            Ok(secret) => secret,
            Err(code) => return encode_failed(bootstrap, code),
        };
        let outcome = self.transport.deploy(SshBootstrapJob {
            bootstrap_id: &intent.bootstrap_id,
            target: &intent.target,
            username: &policy.username,
            host_key_sha256: &policy.host_key_sha256,
            bootstrap_password: &bootstrap_password,
            session_token: &session_token,
            artifact: &self.artifact,
            daemon_id: &policy.daemon_id,
            endpoint: &policy.endpoint,
            install_profile: &policy.install_profile,
        });
        let outcome = match outcome {
            Ok(outcome) => outcome,
            Err(SshBootstrapTransportError::Authentication) => {
                return encode_failed(bootstrap, "authentication_failed");
            }
            Err(SshBootstrapTransportError::HostKeyRejected) => {
                return encode_failed(bootstrap, "host_key_rejected");
            }
            Err(SshBootstrapTransportError::InstallerRejected) => {
                return encode_failed(bootstrap, "installer_rejected");
            }
            Err(_) => return encode_failed(bootstrap, "transport_failure"),
        };
        if outcome.daemon_id != policy.daemon_id || outcome.endpoint != policy.endpoint {
            return encode_failed(bootstrap, "remote_identity_mismatch");
        }
        let trust_record = BootstrapTrustRecord {
            endpoint: outcome.endpoint.clone(),
            ca_pem: outcome.tls_ca_pem,
            ca_sha256: outcome.tls_ca_sha256,
        };
        if self
            .trust
            .persist(&policy.trust_credential_handle, &trust_record)
            .is_err()
        {
            return encode_failed(bootstrap, "trust_persistence_failed");
        }
        let snapshot = bootstrap
            .accept_deployed(DaemonBootstrapReceipt {
                bootstrap_id: intent.bootstrap_id.clone(),
                daemon_id: outcome.daemon_id,
                endpoint: outcome.endpoint,
                session_credential_handle: policy.session_credential_handle.clone(),
                trust_credential_handle: policy.trust_credential_handle.clone(),
            })
            .map_err(|_| "invalid bootstrap receipt")?;
        debug_assert_eq!(snapshot.phase, BootstrapPhase::Bootstrapped);
        encode_state(snapshot)
    }
}

impl<T: SshBootstrapTransport> EffectAdapter for SshBootstrapAdapter<T> {
    fn kind(&self) -> &str {
        HOST_BOOTSTRAP_EFFECT_KIND
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

#[cfg(feature = "native-ssh")]
#[derive(Clone, Debug, Default)]
pub struct NativeSshBootstrapTransport {
    client: NativeSshClient,
}

#[cfg(feature = "native-ssh")]
impl NativeSshBootstrapTransport {
    pub fn with_timeout(timeout: Duration) -> Result<Self, String> {
        Ok(Self {
            client: NativeSshClient::with_timeout(timeout)?,
        })
    }
}

#[cfg(feature = "native-ssh")]
impl SshBootstrapTransport for NativeSshBootstrapTransport {
    fn deploy(
        &mut self,
        job: SshBootstrapJob<'_>,
    ) -> Result<SshBootstrapOutcome, SshBootstrapTransportError> {
        let request = BootstrapInstallerRequest::new(
            job.bootstrap_id.clone(),
            job.daemon_id.clone(),
            job.endpoint,
            job.install_profile,
            job.artifact.sha256_hex(),
            job.session_token.expose_secret(),
        )
        .map_err(|_| SshBootstrapTransportError::Transport)?;
        let payload = encode_bootstrap_installer_request(&request)
            .map_err(|_| SshBootstrapTransportError::Transport)?;
        let staging_path = job.artifact.staging_path(job.bootstrap_id);
        let command = format!("{staging_path} bootstrap-activate-v1");
        let stdout = self
            .client
            .execute(NativeSshJob {
                host: &job.target.host,
                port: job.target.port,
                username: job.username,
                host_key_sha256: job.host_key_sha256,
                password: job.bootstrap_password.expose_secret(),
                staging_path: &staging_path,
                artifact: job.artifact.bytes(),
                artifact_sha256: job.artifact.sha256_hex(),
                command: &command,
                stdin: &payload,
                max_stdout_bytes: MAX_BOOTSTRAP_INSTALLER_BYTES,
            })
            .map_err(map_native_ssh_error)?;
        let response = decode_bootstrap_installer_response(&stdout)
            .map_err(|_| SshBootstrapTransportError::InvalidResponse)?;
        validate_installer_readiness(response, job.bootstrap_id, job.daemon_id, job.endpoint)
    }
}

#[cfg(feature = "native-ssh")]
fn map_native_ssh_error(error: NativeSshError) -> SshBootstrapTransportError {
    match error {
        NativeSshError::Authentication => SshBootstrapTransportError::Authentication,
        NativeSshError::HostKeyRejected => SshBootstrapTransportError::HostKeyRejected,
        NativeSshError::Transport => SshBootstrapTransportError::Transport,
        NativeSshError::UploadRejected => SshBootstrapTransportError::UploadRejected,
        NativeSshError::CommandRejected => SshBootstrapTransportError::InstallerRejected,
        NativeSshError::InvalidResponse => SshBootstrapTransportError::InvalidResponse,
    }
}

#[cfg(feature = "native-ssh")]
fn validate_installer_readiness(
    response: leserpent_protocol::bootstrap_installer::BootstrapInstallerResponse,
    bootstrap_id: &BootstrapId,
    daemon_id: &DaemonId,
    endpoint: &str,
) -> Result<SshBootstrapOutcome, SshBootstrapTransportError> {
    if response.bootstrap_id != *bootstrap_id
        || response.daemon_id != *daemon_id
        || response.endpoint != endpoint
        || response.service_state != BootstrapInstallerServiceState::Ready
    {
        return Err(SshBootstrapTransportError::InvalidResponse);
    }
    Ok(SshBootstrapOutcome {
        daemon_id: response.daemon_id,
        endpoint: response.endpoint,
        tls_ca_pem: response.tls_ca_pem,
        tls_ca_sha256: response.tls_ca_sha256,
    })
}

fn load_secret(store: &dyn SecretStore, key: &str) -> Result<SecretValue, &'static str> {
    let key = SecretKey::new(key).map_err(|_| "credential_handle_invalid")?;
    store
        .load(&key)
        .map_err(|_| "credential_store_unavailable")?
        .ok_or("credential_not_found")
}

fn encode_failed(
    mut bootstrap: DeploymentBootstrap,
    code: &'static str,
) -> Result<Vec<u8>, &'static str> {
    let snapshot = bootstrap
        .record_fault(code)
        .map_err(|_| "invalid bootstrap failure state")?;
    encode_state(snapshot)
}

fn encode_state(
    snapshot: leserpent_domain::bootstrap::DeploymentBootstrapSnapshot,
) -> Result<Vec<u8>, &'static str> {
    encode_bootstrap_response(&BootstrapResponseEnvelope {
        schema_version: BOOTSTRAP_PROTOCOL_SCHEMA_VERSION,
        response: BootstrapResponse::State(snapshot),
    })
    .map_err(|_| "bootstrap response encoding failed")
}

pub(crate) fn target_key(target: &BootstrapTarget) -> String {
    format!("{}:{}", target.host, target.port)
}

pub(crate) fn valid_staging_prefix(value: &str) -> bool {
    value.starts_with("/tmp/")
        && value.len() <= 180
        && !value.contains("..")
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'.' | b'_' | b'-'))
}

pub(crate) fn valid_sha256_fingerprint(value: &str) -> bool {
    let Some(encoded) = value.strip_prefix("SHA256:") else {
        return false;
    };
    (40..=44).contains(&encoded.len())
        && encoded.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/' | b'_' | b'-' | b'=')
        })
}

pub(crate) fn validate_https_origin(value: &str) -> Result<(), String> {
    let Some(authority) = value.strip_prefix("https://") else {
        return Err("daemon endpoint must be an HTTPS origin".into());
    };
    let valid = !authority.is_empty()
        && authority.len() <= 320
        && !authority.contains(['/', '?', '#', '@'])
        && !authority.chars().any(char::is_whitespace);
    valid
        .then_some(())
        .ok_or_else(|| "daemon endpoint must be an HTTPS origin".into())
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
mod tests {
    use std::sync::Mutex;

    use leserpent_domain::bootstrap::{BOOTSTRAP_DOMAIN_SCHEMA_VERSION, CAPABILITY_HOST_BOOTSTRAP};
    use leserpent_domain::{CapabilitySet, Principal};
    use leserpent_protocol::bootstrap::{
        BootstrapRequest, decode_bootstrap_response, encode_bootstrap_request,
    };

    use crate::{BootstrapTrustError, ConfiguredSecretStore, SecretValue};

    use super::*;

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct SeenJob {
        username: String,
        fingerprint: String,
        bootstrap_password: String,
        session_token: String,
        artifact_hash: String,
    }

    struct RecordingTransport {
        seen: Arc<Mutex<Vec<SeenJob>>>,
        result: Result<SshBootstrapOutcome, SshBootstrapTransportError>,
    }

    #[derive(Default)]
    struct RecordingTrustStore {
        records: Mutex<Vec<(String, BootstrapTrustRecord)>>,
        reject: bool,
    }

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

    fn trust() -> Arc<RecordingTrustStore> {
        Arc::new(RecordingTrustStore::default())
    }

    fn successful_outcome(daemon_id: &str) -> SshBootstrapOutcome {
        let ca = "-----BEGIN CERTIFICATE-----\nY2VydA==\n-----END CERTIFICATE-----\n";
        SshBootstrapOutcome {
            daemon_id: DaemonId::new(daemon_id).unwrap(),
            endpoint: "https://host.example:7443".into(),
            tls_ca_pem: ca.into(),
            tls_ca_sha256: hex(digest(&SHA256, ca.as_bytes()).as_ref()),
        }
    }

    impl SshBootstrapTransport for RecordingTransport {
        fn deploy(
            &mut self,
            job: SshBootstrapJob<'_>,
        ) -> Result<SshBootstrapOutcome, SshBootstrapTransportError> {
            self.seen.lock().unwrap().push(SeenJob {
                username: job.username.into(),
                fingerprint: job.host_key_sha256.into(),
                bootstrap_password: job.bootstrap_password.expose_secret().into(),
                session_token: job.session_token.expose_secret().into(),
                artifact_hash: job.artifact.sha256_hex().into(),
            });
            self.result.clone()
        }
    }

    fn target() -> BootstrapTarget {
        BootstrapTarget {
            transport: BootstrapTransport::Ssh,
            host: "host.example".into(),
            port: 22,
        }
    }

    fn policy() -> SshBootstrapHostPolicy {
        SshBootstrapHostPolicy::new(
            target(),
            "deployer",
            "SHA256:abcdefghijklmnopqrstuvwxyz0123456789ABCDE",
            DaemonId::new("daemon-host-example").unwrap(),
            "https://host.example:7443",
            CredentialHandle::new("vault:leserpentd:host-example-session").unwrap(),
            CredentialHandle::new("vault:leserpent-ca:host-example-trust").unwrap(),
            "system",
        )
        .unwrap()
    }

    fn request(target: BootstrapTarget, credential_handle: &str) -> Vec<u8> {
        encode_bootstrap_request(&BootstrapRequestEnvelope {
            schema_version: BOOTSTRAP_PROTOCOL_SCHEMA_VERSION,
            request: BootstrapRequest {
                principal: Principal {
                    id: "operator-a".into(),
                },
                capabilities: CapabilitySet::new([CAPABILITY_HOST_BOOTSTRAP]),
                intent: leserpent_domain::bootstrap::BootstrapIntent {
                    schema_version: BOOTSTRAP_DOMAIN_SCHEMA_VERSION,
                    bootstrap_id: BootstrapId::new("bootstrap-1").unwrap(),
                    target,
                    credential_handle: CredentialHandle::new(credential_handle).unwrap(),
                    requested_by: "operator-a".into(),
                    confirmed: true,
                },
            },
        })
        .unwrap()
    }

    fn secrets() -> Arc<ConfiguredSecretStore> {
        Arc::new(
            ConfiguredSecretStore::new([
                (
                    SecretKey::new("host-example").unwrap(),
                    SecretValue::new("bootstrap-password").unwrap(),
                ),
                (
                    SecretKey::new("host-example-session").unwrap(),
                    SecretValue::new("daemon-session-token").unwrap(),
                ),
            ])
            .unwrap(),
        )
    }

    fn response_state(
        execution: EffectExecution,
    ) -> leserpent_domain::bootstrap::DeploymentBootstrapSnapshot {
        let EffectExecution::Complete(bytes) = execution else {
            panic!("bootstrap execution must produce a typed state");
        };
        let envelope = decode_bootstrap_response(&bytes).unwrap();
        let BootstrapResponse::State(snapshot) = envelope.response else {
            panic!("bootstrap execution must produce a state response");
        };
        snapshot
    }

    #[test]
    fn adapter_resolves_separate_secrets_and_returns_only_the_bootstrapped_state() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let transport = RecordingTransport {
            seen: seen.clone(),
            result: Ok(successful_outcome("daemon-host-example")),
        };
        let artifact = BootstrapArtifact::new(
            Arc::<[u8]>::from(b"native-installer".as_slice()),
            "/tmp/leserpent-bootstrap",
        )
        .unwrap();
        let trust = trust();
        let mut adapter =
            SshBootstrapAdapter::new([policy()], secrets(), trust.clone(), artifact, transport)
                .unwrap();
        let state = response_state(adapter.execute(&request(target(), "vault:ssh:host-example")));

        assert_eq!(state.phase, BootstrapPhase::Bootstrapped);
        assert!(!state.mutation_authorized);
        assert!(state.bootstrap_credential_present);
        assert_eq!(
            state.session_credential_handle.as_ref().unwrap().as_str(),
            "vault:leserpentd:host-example-session"
        );
        assert_eq!(
            state.trust_credential_handle.as_ref().unwrap().as_str(),
            "vault:leserpent-ca:host-example-trust"
        );
        assert_eq!(trust.records.lock().unwrap().len(), 1);
        let seen = seen.lock().unwrap();
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].bootstrap_password, "bootstrap-password");
        assert_eq!(seen[0].session_token, "daemon-session-token");
        let encoded = serde_json::to_string(&state).unwrap();
        assert!(!encoded.contains("bootstrap-password"));
        assert!(!encoded.contains("daemon-session-token"));
        assert!(!encoded.contains("BEGIN CERTIFICATE"));
    }

    #[test]
    fn policy_and_credential_failures_never_open_ssh() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let transport = RecordingTransport {
            seen: seen.clone(),
            result: Err(SshBootstrapTransportError::Transport),
        };
        let artifact = BootstrapArtifact::new(
            Arc::<[u8]>::from(b"native-installer".as_slice()),
            "/tmp/leserpent-bootstrap",
        )
        .unwrap();
        let mut adapter =
            SshBootstrapAdapter::new([policy()], secrets(), trust(), artifact, transport).unwrap();
        let mut unknown = target();
        unknown.host = "unknown.example".into();
        let state = response_state(adapter.execute(&request(unknown, "vault:ssh:host-example")));
        assert_eq!(state.phase, BootstrapPhase::Failed);
        assert_eq!(state.fault_code.as_deref(), Some("target_policy_missing"));

        let state = response_state(adapter.execute(&request(target(), "vault:other:host-example")));
        assert_eq!(
            state.fault_code.as_deref(),
            Some("credential_provider_invalid")
        );
        assert!(seen.lock().unwrap().is_empty());
    }

    #[test]
    fn host_key_and_remote_identity_fail_closed_without_session_authority() {
        for (result, expected) in [
            (
                Err(SshBootstrapTransportError::HostKeyRejected),
                "host_key_rejected",
            ),
            (
                Ok(successful_outcome("daemon-attacker")),
                "remote_identity_mismatch",
            ),
        ] {
            let artifact = BootstrapArtifact::new(
                Arc::<[u8]>::from(b"native-installer".as_slice()),
                "/tmp/leserpent-bootstrap",
            )
            .unwrap();
            let transport = RecordingTransport {
                seen: Arc::new(Mutex::new(Vec::new())),
                result,
            };
            let mut adapter =
                SshBootstrapAdapter::new([policy()], secrets(), trust(), artifact, transport)
                    .unwrap();
            let state =
                response_state(adapter.execute(&request(target(), "vault:ssh:host-example")));
            assert_eq!(state.phase, BootstrapPhase::Failed);
            assert_eq!(state.fault_code.as_deref(), Some(expected));
            assert!(!state.bootstrap_credential_present);
            assert!(state.session_credential_handle.is_none());
            assert!(!state.mutation_authorized);
        }
    }

    #[test]
    fn artifact_and_host_policy_reject_unsafe_configuration() {
        assert!(BootstrapArtifact::new(Arc::<[u8]>::from([]), "/tmp/bootstrap").is_err());
        assert!(
            BootstrapArtifact::new(Arc::<[u8]>::from(b"x".as_slice()), "/tmp/../bootstrap")
                .is_err()
        );
        assert!(
            SshBootstrapHostPolicy::new(
                target(),
                "deployer",
                "accept-new",
                DaemonId::new("daemon-host-example").unwrap(),
                "https://host.example:7443",
                CredentialHandle::new("vault:leserpentd:host-example-session").unwrap(),
                CredentialHandle::new("vault:leserpent-ca:host-example-trust").unwrap(),
                "system",
            )
            .is_err()
        );
    }

    #[test]
    fn trust_persistence_failure_withholds_bootstrapped_authority() {
        let artifact = BootstrapArtifact::new(
            Arc::<[u8]>::from(b"native-installer".as_slice()),
            "/tmp/leserpent-bootstrap",
        )
        .unwrap();
        let transport = RecordingTransport {
            seen: Arc::new(Mutex::new(Vec::new())),
            result: Ok(successful_outcome("daemon-host-example")),
        };
        let trust = Arc::new(RecordingTrustStore {
            records: Mutex::new(Vec::new()),
            reject: true,
        });
        let mut adapter =
            SshBootstrapAdapter::new([policy()], secrets(), trust, artifact, transport).unwrap();
        let state = response_state(adapter.execute(&request(target(), "vault:ssh:host-example")));
        assert_eq!(state.phase, BootstrapPhase::Failed);
        assert_eq!(
            state.fault_code.as_deref(),
            Some("trust_persistence_failed")
        );
        assert!(state.session_credential_handle.is_none());
        assert!(state.trust_credential_handle.is_none());
    }

    #[cfg(feature = "native-ssh")]
    #[test]
    fn native_transport_rejects_installed_until_the_service_is_ready() {
        use leserpent_protocol::bootstrap_installer::{
            BOOTSTRAP_INSTALLER_SCHEMA_VERSION, BootstrapInstallerResponse,
            BootstrapInstallerServiceState,
        };

        let bootstrap_id = BootstrapId::new("bootstrap-1").unwrap();
        let daemon_id = DaemonId::new("daemon-host-example").unwrap();
        let tls_ca_pem = "-----BEGIN CERTIFICATE-----\nY2VydA==\n-----END CERTIFICATE-----\n";
        let response = BootstrapInstallerResponse {
            schema_version: BOOTSTRAP_INSTALLER_SCHEMA_VERSION,
            bootstrap_id: bootstrap_id.clone(),
            daemon_id: daemon_id.clone(),
            endpoint: "https://host.example:7443".into(),
            service_state: BootstrapInstallerServiceState::Installed,
            generation: "a".repeat(64),
            replayed: false,
            tls_ca_pem: tls_ca_pem.into(),
            tls_ca_sha256: hex(digest(&SHA256, tls_ca_pem.as_bytes()).as_ref()),
        };
        assert_eq!(
            validate_installer_readiness(
                response,
                &bootstrap_id,
                &daemon_id,
                "https://host.example:7443"
            ),
            Err(SshBootstrapTransportError::InvalidResponse)
        );
    }
}
