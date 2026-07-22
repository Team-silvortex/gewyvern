use std::fmt;

use http::Uri;
use serde::{Deserialize, Serialize};

use crate::bootstrap::{BootstrapTarget, CredentialHandle};
use crate::{CapabilitySet, Principal, RuntimeId};

pub const PROVISIONING_DOMAIN_SCHEMA_VERSION: u32 = 1;
pub const PROVISIONING_CHECKPOINT_SCHEMA_VERSION: u32 = 1;
pub const PROVISIONING_SERVICE_PROTOCOL_VERSION: u32 = 1;
pub const CAPABILITY_RUNTIME_PROVISION: &str = "runtime.provision";

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct ProvisioningId(String);

impl ProvisioningId {
    pub fn new(value: impl Into<String>) -> Result<Self, ProvisioningError> {
        validate_identifier("provisioning_id", value.into()).map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for ProvisioningId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeProvisioningIntent {
    pub schema_version: u32,
    pub provisioning_id: ProvisioningId,
    pub runtime_id: RuntimeId,
    pub target: BootstrapTarget,
    pub install_credential_handle: CredentialHandle,
    pub requested_by: String,
    pub confirmed: bool,
}

impl RuntimeProvisioningIntent {
    pub fn validate(&self) -> Result<(), ProvisioningError> {
        if self.schema_version != PROVISIONING_DOMAIN_SCHEMA_VERSION {
            return Err(ProvisioningError::InvalidSchemaVersion {
                actual: self.schema_version,
                expected: PROVISIONING_DOMAIN_SCHEMA_VERSION,
            });
        }
        validate_identifier("requested_by", self.requested_by.clone())?;
        self.target
            .validate()
            .map_err(|_| ProvisioningError::InvalidTarget)?;
        if !self.confirmed {
            return Err(ProvisioningError::ConfirmationRequired);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvisioningPhase {
    Planned,
    Installing,
    ServiceReady,
    RuntimeRegistered,
    Failed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GewyvernServiceReceipt {
    pub provisioning_id: ProvisioningId,
    pub runtime_id: RuntimeId,
    pub endpoint: String,
    pub api_credential_handle: CredentialHandle,
    pub trust_credential_handle: CredentialHandle,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeRegistrationProof {
    pub provisioning_id: ProvisioningId,
    pub runtime_id: RuntimeId,
    pub endpoint: String,
    pub api_credential_handle: CredentialHandle,
    pub trust_credential_handle: CredentialHandle,
    pub authority_owned: bool,
    pub protocol_schema_version: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeProvisioningSnapshot {
    pub provisioning_id: ProvisioningId,
    pub runtime_id: RuntimeId,
    pub phase: ProvisioningPhase,
    pub target: BootstrapTarget,
    pub install_credential_present: bool,
    pub endpoint: Option<String>,
    pub api_credential_handle: Option<CredentialHandle>,
    pub trust_credential_handle: Option<CredentialHandle>,
    pub fault_code: Option<String>,
    pub runtime_registered: bool,
}

impl RuntimeProvisioningSnapshot {
    pub fn validate(&self) -> Result<(), ProvisioningError> {
        self.target
            .validate()
            .map_err(|_| ProvisioningError::InvalidTarget)?;
        match self.phase {
            ProvisioningPhase::Planned | ProvisioningPhase::Installing => {
                if !self.install_credential_present
                    || self.endpoint.is_some()
                    || self.api_credential_handle.is_some()
                    || self.trust_credential_handle.is_some()
                    || self.fault_code.is_some()
                    || self.runtime_registered
                {
                    return Err(ProvisioningError::InvalidSnapshot);
                }
            }
            ProvisioningPhase::ServiceReady => {
                if self.install_credential_present
                    || !self.has_valid_service_identity()
                    || self.fault_code.is_some()
                    || self.runtime_registered
                {
                    return Err(ProvisioningError::InvalidSnapshot);
                }
            }
            ProvisioningPhase::RuntimeRegistered => {
                if self.install_credential_present
                    || !self.has_valid_service_identity()
                    || self.fault_code.is_some()
                    || !self.runtime_registered
                {
                    return Err(ProvisioningError::InvalidSnapshot);
                }
            }
            ProvisioningPhase::Failed => {
                if self.install_credential_present
                    || self.endpoint.is_some()
                    || self.api_credential_handle.is_some()
                    || self.trust_credential_handle.is_some()
                    || self
                        .fault_code
                        .as_deref()
                        .is_none_or(|value| validate_fault(value).is_err())
                    || self.runtime_registered
                {
                    return Err(ProvisioningError::InvalidSnapshot);
                }
            }
        }
        Ok(())
    }

    fn has_valid_service_identity(&self) -> bool {
        self.endpoint
            .as_deref()
            .is_some_and(|value| validate_endpoint(value).is_ok())
            && self.api_credential_handle.is_some()
            && self.trust_credential_handle.is_some()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeProvisioningCheckpoint {
    pub schema_version: u32,
    pub revision: u64,
    pub state: RuntimeProvisioningSnapshot,
    pub install_credential_handle: Option<CredentialHandle>,
}

impl RuntimeProvisioningCheckpoint {
    pub fn new(
        revision: u64,
        state: RuntimeProvisioningSnapshot,
        install_credential_handle: Option<CredentialHandle>,
    ) -> Result<Self, ProvisioningError> {
        let checkpoint = Self {
            schema_version: PROVISIONING_CHECKPOINT_SCHEMA_VERSION,
            revision,
            state,
            install_credential_handle,
        };
        checkpoint.validate()?;
        Ok(checkpoint)
    }

    pub fn validate(&self) -> Result<(), ProvisioningError> {
        if self.schema_version != PROVISIONING_CHECKPOINT_SCHEMA_VERSION {
            return Err(ProvisioningError::InvalidSchemaVersion {
                actual: self.schema_version,
                expected: PROVISIONING_CHECKPOINT_SCHEMA_VERSION,
            });
        }
        if self.revision == 0
            || self.state.install_credential_present != self.install_credential_handle.is_some()
        {
            return Err(ProvisioningError::InvalidCheckpoint);
        }
        self.state.validate()
    }
}

#[derive(Clone, Debug)]
pub struct RuntimeProvisioning {
    provisioning_id: ProvisioningId,
    runtime_id: RuntimeId,
    target: BootstrapTarget,
    install_credential_handle: Option<CredentialHandle>,
    phase: ProvisioningPhase,
    endpoint: Option<String>,
    api_credential_handle: Option<CredentialHandle>,
    trust_credential_handle: Option<CredentialHandle>,
    fault_code: Option<String>,
}

impl RuntimeProvisioning {
    pub fn plan(
        principal: &Principal,
        capabilities: &CapabilitySet,
        intent: RuntimeProvisioningIntent,
    ) -> Result<Self, ProvisioningError> {
        intent.validate()?;
        validate_identifier("principal.id", principal.id.clone())?;
        if principal.id != intent.requested_by {
            return Err(ProvisioningError::PrincipalMismatch);
        }
        if !capabilities.contains(CAPABILITY_RUNTIME_PROVISION) {
            return Err(ProvisioningError::Unauthorized);
        }
        Ok(Self {
            provisioning_id: intent.provisioning_id,
            runtime_id: intent.runtime_id,
            target: intent.target,
            install_credential_handle: Some(intent.install_credential_handle),
            phase: ProvisioningPhase::Planned,
            endpoint: None,
            api_credential_handle: None,
            trust_credential_handle: None,
            fault_code: None,
        })
    }

    pub fn begin(&mut self) -> Result<RuntimeProvisioningSnapshot, ProvisioningError> {
        self.require_phase(ProvisioningPhase::Planned)?;
        self.phase = ProvisioningPhase::Installing;
        Ok(self.snapshot())
    }

    pub fn accept_service(
        &mut self,
        receipt: GewyvernServiceReceipt,
    ) -> Result<RuntimeProvisioningSnapshot, ProvisioningError> {
        self.require_phase(ProvisioningPhase::Installing)?;
        if receipt.provisioning_id != self.provisioning_id || receipt.runtime_id != self.runtime_id
        {
            return Err(ProvisioningError::IdentityMismatch);
        }
        validate_endpoint(&receipt.endpoint)?;
        self.install_credential_handle = None;
        self.endpoint = Some(receipt.endpoint);
        self.api_credential_handle = Some(receipt.api_credential_handle);
        self.trust_credential_handle = Some(receipt.trust_credential_handle);
        self.phase = ProvisioningPhase::ServiceReady;
        Ok(self.snapshot())
    }

    pub fn accept_registration(
        &mut self,
        proof: RuntimeRegistrationProof,
    ) -> Result<RuntimeProvisioningSnapshot, ProvisioningError> {
        if self.phase == ProvisioningPhase::RuntimeRegistered {
            self.validate_registration_proof(&proof)?;
            return Ok(self.snapshot());
        }
        self.require_phase(ProvisioningPhase::ServiceReady)?;
        self.validate_registration_proof(&proof)?;
        self.phase = ProvisioningPhase::RuntimeRegistered;
        Ok(self.snapshot())
    }

    pub fn record_fault(
        &mut self,
        fault_code: impl Into<String>,
    ) -> Result<RuntimeProvisioningSnapshot, ProvisioningError> {
        if matches!(
            self.phase,
            ProvisioningPhase::RuntimeRegistered | ProvisioningPhase::Failed
        ) {
            return Err(ProvisioningError::InvalidTransition { actual: self.phase });
        }
        let fault_code = fault_code.into();
        validate_fault(&fault_code)?;
        self.install_credential_handle = None;
        self.endpoint = None;
        self.api_credential_handle = None;
        self.trust_credential_handle = None;
        self.fault_code = Some(fault_code);
        self.phase = ProvisioningPhase::Failed;
        Ok(self.snapshot())
    }

    pub fn checkpoint(
        &self,
        revision: u64,
    ) -> Result<RuntimeProvisioningCheckpoint, ProvisioningError> {
        RuntimeProvisioningCheckpoint::new(
            revision,
            self.snapshot(),
            self.install_credential_handle.clone(),
        )
    }

    pub fn resume(checkpoint: &RuntimeProvisioningCheckpoint) -> Result<Self, ProvisioningError> {
        checkpoint.validate()?;
        Ok(Self {
            provisioning_id: checkpoint.state.provisioning_id.clone(),
            runtime_id: checkpoint.state.runtime_id.clone(),
            target: checkpoint.state.target.clone(),
            install_credential_handle: checkpoint.install_credential_handle.clone(),
            phase: checkpoint.state.phase,
            endpoint: checkpoint.state.endpoint.clone(),
            api_credential_handle: checkpoint.state.api_credential_handle.clone(),
            trust_credential_handle: checkpoint.state.trust_credential_handle.clone(),
            fault_code: checkpoint.state.fault_code.clone(),
        })
    }

    pub fn snapshot(&self) -> RuntimeProvisioningSnapshot {
        RuntimeProvisioningSnapshot {
            provisioning_id: self.provisioning_id.clone(),
            runtime_id: self.runtime_id.clone(),
            phase: self.phase,
            target: self.target.clone(),
            install_credential_present: self.install_credential_handle.is_some(),
            endpoint: self.endpoint.clone(),
            api_credential_handle: self.api_credential_handle.clone(),
            trust_credential_handle: self.trust_credential_handle.clone(),
            fault_code: self.fault_code.clone(),
            runtime_registered: self.phase == ProvisioningPhase::RuntimeRegistered,
        }
    }

    fn validate_registration_proof(
        &self,
        proof: &RuntimeRegistrationProof,
    ) -> Result<(), ProvisioningError> {
        if proof.provisioning_id != self.provisioning_id
            || proof.runtime_id != self.runtime_id
            || self.endpoint.as_ref() != Some(&proof.endpoint)
            || self.api_credential_handle.as_ref() != Some(&proof.api_credential_handle)
            || self.trust_credential_handle.as_ref() != Some(&proof.trust_credential_handle)
        {
            return Err(ProvisioningError::IdentityMismatch);
        }
        if !proof.authority_owned
            || proof.protocol_schema_version != PROVISIONING_SERVICE_PROTOCOL_VERSION
        {
            return Err(ProvisioningError::ServiceProofRejected);
        }
        Ok(())
    }

    fn require_phase(&self, expected: ProvisioningPhase) -> Result<(), ProvisioningError> {
        if self.phase != expected {
            return Err(ProvisioningError::InvalidTransition { actual: self.phase });
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProvisioningError {
    InvalidIdentifier { field: &'static str },
    InvalidSchemaVersion { actual: u32, expected: u32 },
    InvalidTarget,
    InvalidEndpoint,
    InvalidFaultCode,
    InvalidSnapshot,
    InvalidCheckpoint,
    ConfirmationRequired,
    PrincipalMismatch,
    Unauthorized,
    InvalidTransition { actual: ProvisioningPhase },
    IdentityMismatch,
    ServiceProofRejected,
}

impl fmt::Display for ProvisioningError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidIdentifier { field } => write!(formatter, "invalid {field}"),
            Self::InvalidSchemaVersion { actual, expected } => write!(
                formatter,
                "unsupported provisioning schema {actual}, expected {expected}"
            ),
            Self::InvalidTarget => formatter.write_str("invalid provisioning target"),
            Self::InvalidEndpoint => formatter.write_str("invalid Gewyvern endpoint"),
            Self::InvalidFaultCode => formatter.write_str("invalid provisioning fault code"),
            Self::InvalidSnapshot => formatter.write_str("invalid provisioning snapshot"),
            Self::InvalidCheckpoint => formatter.write_str("invalid provisioning checkpoint"),
            Self::ConfirmationRequired => formatter.write_str("provisioning requires confirmation"),
            Self::PrincipalMismatch => formatter.write_str("provisioning principal mismatch"),
            Self::Unauthorized => {
                formatter.write_str("runtime provisioning capability is required")
            }
            Self::InvalidTransition { actual } => {
                write!(formatter, "invalid provisioning transition from {actual:?}")
            }
            Self::IdentityMismatch => formatter.write_str("provisioning identity mismatch"),
            Self::ServiceProofRejected => formatter.write_str("Gewyvern service proof rejected"),
        }
    }
}

impl std::error::Error for ProvisioningError {}

fn validate_identifier(field: &'static str, value: String) -> Result<String, ProvisioningError> {
    let valid = !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':' | b'.'));
    valid
        .then_some(value)
        .ok_or(ProvisioningError::InvalidIdentifier { field })
}

fn validate_endpoint(value: &str) -> Result<(), ProvisioningError> {
    let uri = value
        .parse::<Uri>()
        .map_err(|_| ProvisioningError::InvalidEndpoint)?;
    let valid = uri.scheme_str() == Some("https")
        && uri.authority().is_some()
        && !value.contains('@')
        && uri.query().is_none()
        && uri.path() == "/";
    valid
        .then_some(())
        .ok_or(ProvisioningError::InvalidEndpoint)
}

fn validate_fault(value: &str) -> Result<(), ProvisioningError> {
    let valid = !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_');
    valid
        .then_some(())
        .ok_or(ProvisioningError::InvalidFaultCode)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bootstrap::BootstrapTransport;

    fn intent() -> RuntimeProvisioningIntent {
        RuntimeProvisioningIntent {
            schema_version: PROVISIONING_DOMAIN_SCHEMA_VERSION,
            provisioning_id: ProvisioningId::new("provision-1").unwrap(),
            runtime_id: RuntimeId::new("runtime-a").unwrap(),
            target: BootstrapTarget {
                transport: BootstrapTransport::Ssh,
                host: "host.example".into(),
                port: 22,
            },
            install_credential_handle: CredentialHandle::new("vault:ssh:host-example").unwrap(),
            requested_by: "operator-a".into(),
            confirmed: true,
        }
    }

    fn plan() -> RuntimeProvisioning {
        RuntimeProvisioning::plan(
            &Principal {
                id: "operator-a".into(),
            },
            &CapabilitySet::new([CAPABILITY_RUNTIME_PROVISION]),
            intent(),
        )
        .unwrap()
    }

    fn receipt() -> GewyvernServiceReceipt {
        GewyvernServiceReceipt {
            provisioning_id: ProvisioningId::new("provision-1").unwrap(),
            runtime_id: RuntimeId::new("runtime-a").unwrap(),
            endpoint: "https://host.example:9411".into(),
            api_credential_handle: CredentialHandle::new("vault:gewyvern-api:runtime-a").unwrap(),
            trust_credential_handle: CredentialHandle::new("vault:gewyvern-ca:runtime-a").unwrap(),
        }
    }

    fn proof(authority_owned: bool) -> RuntimeRegistrationProof {
        let receipt = receipt();
        RuntimeRegistrationProof {
            provisioning_id: receipt.provisioning_id,
            runtime_id: receipt.runtime_id,
            endpoint: receipt.endpoint,
            api_credential_handle: receipt.api_credential_handle,
            trust_credential_handle: receipt.trust_credential_handle,
            authority_owned,
            protocol_schema_version: PROVISIONING_SERVICE_PROTOCOL_VERSION,
        }
    }

    #[test]
    fn service_readiness_retires_install_authority_before_registration() {
        let mut provisioning = plan();
        assert_eq!(
            provisioning.begin().unwrap().phase,
            ProvisioningPhase::Installing
        );
        let ready = provisioning.accept_service(receipt()).unwrap();
        assert_eq!(ready.phase, ProvisioningPhase::ServiceReady);
        assert!(!ready.install_credential_present);
        assert!(!ready.runtime_registered);

        let registered = provisioning.accept_registration(proof(true)).unwrap();
        assert_eq!(registered.phase, ProvisioningPhase::RuntimeRegistered);
        assert!(registered.runtime_registered);
        assert_eq!(
            provisioning.accept_registration(proof(true)).unwrap(),
            registered
        );
    }

    #[test]
    fn registration_rejects_unowned_or_confused_service_identity() {
        let mut provisioning = plan();
        provisioning.begin().unwrap();
        provisioning.accept_service(receipt()).unwrap();
        assert_eq!(
            provisioning.accept_registration(proof(false)),
            Err(ProvisioningError::ServiceProofRejected)
        );
        let mut confused = proof(true);
        confused.runtime_id = RuntimeId::new("runtime-b").unwrap();
        assert_eq!(
            provisioning.accept_registration(confused),
            Err(ProvisioningError::IdentityMismatch)
        );
        assert_eq!(
            provisioning.snapshot().phase,
            ProvisioningPhase::ServiceReady
        );
    }

    #[test]
    fn plan_requires_confirmation_capability_and_principal_binding() {
        let principal = Principal {
            id: "operator-a".into(),
        };
        let capabilities = CapabilitySet::new([CAPABILITY_RUNTIME_PROVISION]);
        let mut unconfirmed = intent();
        unconfirmed.confirmed = false;
        assert_eq!(
            RuntimeProvisioning::plan(&principal, &capabilities, unconfirmed).unwrap_err(),
            ProvisioningError::ConfirmationRequired
        );
        assert_eq!(
            RuntimeProvisioning::plan(&principal, &CapabilitySet::default(), intent()).unwrap_err(),
            ProvisioningError::Unauthorized
        );
        assert_eq!(
            RuntimeProvisioning::plan(
                &Principal {
                    id: "operator-b".into()
                },
                &capabilities,
                intent()
            )
            .unwrap_err(),
            ProvisioningError::PrincipalMismatch
        );
    }

    #[test]
    fn checkpoint_retains_only_the_authority_required_by_its_phase() {
        let mut provisioning = plan();
        let planned = provisioning.checkpoint(1).unwrap();
        assert!(planned.install_credential_handle.is_some());
        let resumed = RuntimeProvisioning::resume(&planned).unwrap();
        assert_eq!(resumed.snapshot(), provisioning.snapshot());

        provisioning.begin().unwrap();
        provisioning.accept_service(receipt()).unwrap();
        let ready = provisioning.checkpoint(3).unwrap();
        assert!(ready.install_credential_handle.is_none());
        assert_eq!(
            RuntimeProvisioning::resume(&ready).unwrap().snapshot(),
            provisioning.snapshot()
        );
    }

    #[test]
    fn failure_clears_all_authority_and_cannot_replace_registration() {
        let mut provisioning = plan();
        provisioning.begin().unwrap();
        let failed = provisioning.record_fault("transport_failure").unwrap();
        assert_eq!(failed.phase, ProvisioningPhase::Failed);
        assert!(!failed.install_credential_present);
        assert!(failed.api_credential_handle.is_none());
        assert_eq!(failed.fault_code.as_deref(), Some("transport_failure"));
        assert!(matches!(
            provisioning.record_fault("retry"),
            Err(ProvisioningError::InvalidTransition { .. })
        ));
    }

    #[test]
    fn snapshots_reject_early_registration_and_raw_endpoint_confusion() {
        let mut invalid = plan().snapshot();
        invalid.runtime_registered = true;
        assert_eq!(invalid.validate(), Err(ProvisioningError::InvalidSnapshot));

        let mut provisioning = plan();
        provisioning.begin().unwrap();
        let mut invalid_receipt = receipt();
        invalid_receipt.endpoint = "http://host.example:9411".into();
        assert_eq!(
            provisioning.accept_service(invalid_receipt),
            Err(ProvisioningError::InvalidEndpoint)
        );
    }
}
