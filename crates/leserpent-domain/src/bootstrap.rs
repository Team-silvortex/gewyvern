use std::fmt;

use http::Uri;
use serde::{Deserialize, Serialize};

use crate::{CapabilitySet, Principal};

pub const BOOTSTRAP_DOMAIN_SCHEMA_VERSION: u32 = 1;
pub const CAPABILITY_HOST_BOOTSTRAP: &str = "host.bootstrap";
pub const BOOTSTRAP_SESSION_PROTOCOL_VERSION: u32 = 1;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct BootstrapId(String);

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct DaemonId(String);

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct CredentialHandle(String);

macro_rules! validated_identifier {
    ($name:ident, $field:literal) => {
        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, BootstrapError> {
                validate_identifier($field, value.into()).map(Self)
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Self::new(value).map_err(serde::de::Error::custom)
            }
        }
    };
}

validated_identifier!(BootstrapId, "bootstrap_id");
validated_identifier!(DaemonId, "daemon_id");

impl CredentialHandle {
    pub fn new(value: impl Into<String>) -> Result<Self, BootstrapError> {
        let value = validate_identifier("credential_handle", value.into())?;
        let Some((provider, key)) = value
            .strip_prefix("vault:")
            .and_then(|suffix| suffix.split_once(':'))
        else {
            return Err(BootstrapError::InvalidCredentialHandle);
        };
        if provider.is_empty() || key.is_empty() {
            return Err(BootstrapError::InvalidCredentialHandle);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn parts(&self) -> (&str, &str) {
        self.0
            .strip_prefix("vault:")
            .and_then(|suffix| suffix.split_once(':'))
            .expect("validated credential handles always contain provider and key")
    }
}

impl<'de> Deserialize<'de> for CredentialHandle {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BootstrapTransport {
    Ssh,
    Winrm,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BootstrapTarget {
    pub transport: BootstrapTransport,
    pub host: String,
    pub port: u16,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BootstrapIntent {
    pub schema_version: u32,
    pub bootstrap_id: BootstrapId,
    pub target: BootstrapTarget,
    pub credential_handle: CredentialHandle,
    pub requested_by: String,
    pub confirmed: bool,
}

impl BootstrapIntent {
    pub fn validate(&self) -> Result<(), BootstrapError> {
        if self.schema_version != BOOTSTRAP_DOMAIN_SCHEMA_VERSION {
            return Err(BootstrapError::InvalidSchemaVersion {
                actual: self.schema_version,
                expected: BOOTSTRAP_DOMAIN_SCHEMA_VERSION,
            });
        }
        validate_identifier("requested_by", self.requested_by.clone())?;
        self.target.validate()?;
        if !self.confirmed {
            return Err(BootstrapError::ConfirmationRequired);
        }
        Ok(())
    }
}

impl BootstrapTarget {
    pub fn validate(&self) -> Result<(), BootstrapError> {
        let host_valid = !self.host.is_empty()
            && self.host.len() <= 253
            && self.host == self.host.trim()
            && self.host.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b':' | b'_')
            });
        if !host_valid || self.port == 0 {
            return Err(BootstrapError::InvalidTarget);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BootstrapPhase {
    Planned,
    Deploying,
    Bootstrapped,
    SessionBound,
    Failed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DaemonBootstrapReceipt {
    pub bootstrap_id: BootstrapId,
    pub daemon_id: DaemonId,
    pub endpoint: String,
    pub session_credential_handle: CredentialHandle,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DaemonSessionProof {
    pub bootstrap_id: BootstrapId,
    pub daemon_id: DaemonId,
    pub session_credential_handle: CredentialHandle,
    pub authority_owned: bool,
    pub protocol_schema_version: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DeploymentBootstrapSnapshot {
    pub bootstrap_id: BootstrapId,
    pub phase: BootstrapPhase,
    pub target: BootstrapTarget,
    pub bootstrap_credential_present: bool,
    pub daemon_id: Option<DaemonId>,
    pub endpoint: Option<String>,
    pub session_credential_handle: Option<CredentialHandle>,
    pub fault_code: Option<String>,
    pub mutation_authorized: bool,
}

impl DeploymentBootstrapSnapshot {
    pub fn validate(&self) -> Result<(), BootstrapError> {
        self.target.validate()?;
        match self.phase {
            BootstrapPhase::Planned | BootstrapPhase::Deploying => {
                if !self.bootstrap_credential_present
                    || self.daemon_id.is_some()
                    || self.endpoint.is_some()
                    || self.session_credential_handle.is_some()
                    || self.fault_code.is_some()
                    || self.mutation_authorized
                {
                    return Err(BootstrapError::InvalidSnapshot);
                }
            }
            BootstrapPhase::Bootstrapped => {
                if !self.bootstrap_credential_present
                    || self.daemon_id.is_none()
                    || self
                        .endpoint
                        .as_deref()
                        .is_none_or(|value| validate_endpoint(value).is_err())
                    || self.session_credential_handle.is_none()
                    || self.fault_code.is_some()
                    || self.mutation_authorized
                {
                    return Err(BootstrapError::InvalidSnapshot);
                }
            }
            BootstrapPhase::SessionBound => {
                if self.bootstrap_credential_present
                    || self.daemon_id.is_none()
                    || self
                        .endpoint
                        .as_deref()
                        .is_none_or(|value| validate_endpoint(value).is_err())
                    || self.session_credential_handle.is_none()
                    || self.fault_code.is_some()
                    || !self.mutation_authorized
                {
                    return Err(BootstrapError::InvalidSnapshot);
                }
            }
            BootstrapPhase::Failed => {
                if self.bootstrap_credential_present
                    || self.daemon_id.is_some()
                    || self.endpoint.is_some()
                    || self.session_credential_handle.is_some()
                    || self
                        .fault_code
                        .as_deref()
                        .is_none_or(|value| validate_fault(value).is_err())
                    || self.mutation_authorized
                {
                    return Err(BootstrapError::InvalidSnapshot);
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct DeploymentBootstrap {
    bootstrap_id: BootstrapId,
    target: BootstrapTarget,
    bootstrap_credential_handle: Option<CredentialHandle>,
    phase: BootstrapPhase,
    daemon_id: Option<DaemonId>,
    endpoint: Option<String>,
    session_credential_handle: Option<CredentialHandle>,
    fault_code: Option<String>,
}

impl DeploymentBootstrap {
    pub fn plan(
        principal: &Principal,
        capabilities: &CapabilitySet,
        intent: BootstrapIntent,
    ) -> Result<Self, BootstrapError> {
        intent.validate()?;
        validate_identifier("principal.id", principal.id.clone())?;
        if principal.id != intent.requested_by {
            return Err(BootstrapError::PrincipalMismatch);
        }
        if !capabilities.contains(CAPABILITY_HOST_BOOTSTRAP) {
            return Err(BootstrapError::Unauthorized);
        }
        Ok(Self {
            bootstrap_id: intent.bootstrap_id,
            target: intent.target,
            bootstrap_credential_handle: Some(intent.credential_handle),
            phase: BootstrapPhase::Planned,
            daemon_id: None,
            endpoint: None,
            session_credential_handle: None,
            fault_code: None,
        })
    }

    pub fn begin(&mut self) -> Result<DeploymentBootstrapSnapshot, BootstrapError> {
        self.require_phase(BootstrapPhase::Planned)?;
        self.phase = BootstrapPhase::Deploying;
        Ok(self.snapshot())
    }

    pub fn accept_deployed(
        &mut self,
        receipt: DaemonBootstrapReceipt,
    ) -> Result<DeploymentBootstrapSnapshot, BootstrapError> {
        self.require_phase(BootstrapPhase::Deploying)?;
        if receipt.bootstrap_id != self.bootstrap_id {
            return Err(BootstrapError::IdentityMismatch);
        }
        validate_endpoint(&receipt.endpoint)?;
        self.daemon_id = Some(receipt.daemon_id);
        self.endpoint = Some(receipt.endpoint);
        self.session_credential_handle = Some(receipt.session_credential_handle);
        self.phase = BootstrapPhase::Bootstrapped;
        Ok(self.snapshot())
    }

    pub fn bind_session(
        &mut self,
        proof: DaemonSessionProof,
    ) -> Result<DeploymentBootstrapSnapshot, BootstrapError> {
        self.require_phase(BootstrapPhase::Bootstrapped)?;
        if proof.bootstrap_id != self.bootstrap_id
            || self.daemon_id.as_ref() != Some(&proof.daemon_id)
            || self.session_credential_handle.as_ref() != Some(&proof.session_credential_handle)
        {
            return Err(BootstrapError::IdentityMismatch);
        }
        if !proof.authority_owned
            || proof.protocol_schema_version != BOOTSTRAP_SESSION_PROTOCOL_VERSION
        {
            return Err(BootstrapError::SessionProofRejected);
        }
        self.bootstrap_credential_handle = None;
        self.phase = BootstrapPhase::SessionBound;
        Ok(self.snapshot())
    }

    pub fn record_fault(
        &mut self,
        fault_code: impl Into<String>,
    ) -> Result<DeploymentBootstrapSnapshot, BootstrapError> {
        if matches!(
            self.phase,
            BootstrapPhase::SessionBound | BootstrapPhase::Failed
        ) {
            return Err(BootstrapError::InvalidTransition { actual: self.phase });
        }
        let fault_code = fault_code.into();
        validate_fault(&fault_code)?;
        self.bootstrap_credential_handle = None;
        self.daemon_id = None;
        self.endpoint = None;
        self.session_credential_handle = None;
        self.fault_code = Some(fault_code);
        self.phase = BootstrapPhase::Failed;
        Ok(self.snapshot())
    }

    pub fn snapshot(&self) -> DeploymentBootstrapSnapshot {
        DeploymentBootstrapSnapshot {
            bootstrap_id: self.bootstrap_id.clone(),
            phase: self.phase,
            target: self.target.clone(),
            bootstrap_credential_present: self.bootstrap_credential_handle.is_some(),
            daemon_id: self.daemon_id.clone(),
            endpoint: self.endpoint.clone(),
            session_credential_handle: self.session_credential_handle.clone(),
            fault_code: self.fault_code.clone(),
            mutation_authorized: self.phase == BootstrapPhase::SessionBound,
        }
    }

    fn require_phase(&self, expected: BootstrapPhase) -> Result<(), BootstrapError> {
        if self.phase != expected {
            return Err(BootstrapError::InvalidTransition { actual: self.phase });
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BootstrapError {
    InvalidIdentifier { field: &'static str },
    InvalidCredentialHandle,
    InvalidSchemaVersion { actual: u32, expected: u32 },
    InvalidTarget,
    InvalidEndpoint,
    InvalidFaultCode,
    InvalidSnapshot,
    ConfirmationRequired,
    PrincipalMismatch,
    Unauthorized,
    InvalidTransition { actual: BootstrapPhase },
    IdentityMismatch,
    SessionProofRejected,
}

impl fmt::Display for BootstrapError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidIdentifier { field } => write!(formatter, "invalid {field}"),
            Self::InvalidCredentialHandle => write!(formatter, "invalid credential handle"),
            Self::InvalidSchemaVersion { actual, expected } => {
                write!(
                    formatter,
                    "unsupported bootstrap schema {actual}, expected {expected}"
                )
            }
            Self::InvalidTarget => write!(formatter, "invalid bootstrap target"),
            Self::InvalidEndpoint => write!(formatter, "invalid daemon endpoint"),
            Self::InvalidFaultCode => write!(formatter, "invalid bootstrap fault code"),
            Self::InvalidSnapshot => write!(formatter, "invalid bootstrap snapshot"),
            Self::ConfirmationRequired => write!(formatter, "bootstrap requires confirmation"),
            Self::PrincipalMismatch => write!(formatter, "bootstrap principal mismatch"),
            Self::Unauthorized => write!(formatter, "host bootstrap capability is required"),
            Self::InvalidTransition { actual } => {
                write!(formatter, "invalid bootstrap transition from {actual:?}")
            }
            Self::IdentityMismatch => write!(formatter, "bootstrap identity mismatch"),
            Self::SessionProofRejected => write!(formatter, "daemon session proof rejected"),
        }
    }
}

impl std::error::Error for BootstrapError {}

fn validate_identifier(field: &'static str, value: String) -> Result<String, BootstrapError> {
    let valid = !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':' | b'.'));
    valid
        .then_some(value)
        .ok_or(BootstrapError::InvalidIdentifier { field })
}

fn validate_endpoint(value: &str) -> Result<(), BootstrapError> {
    let uri = value
        .parse::<Uri>()
        .map_err(|_| BootstrapError::InvalidEndpoint)?;
    let valid = uri.scheme_str() == Some("https")
        && uri.authority().is_some()
        && !value.contains('@')
        && uri.query().is_none()
        && uri.path() == "/";
    valid.then_some(()).ok_or(BootstrapError::InvalidEndpoint)
}

fn validate_fault(value: &str) -> Result<(), BootstrapError> {
    let valid = !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_');
    valid.then_some(()).ok_or(BootstrapError::InvalidFaultCode)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn intent() -> BootstrapIntent {
        BootstrapIntent {
            schema_version: BOOTSTRAP_DOMAIN_SCHEMA_VERSION,
            bootstrap_id: BootstrapId::new("bootstrap-1").unwrap(),
            target: BootstrapTarget {
                transport: BootstrapTransport::Ssh,
                host: "host.example".into(),
                port: 22,
            },
            credential_handle: CredentialHandle::new("vault:ssh:host-example").unwrap(),
            requested_by: "operator-a".into(),
            confirmed: true,
        }
    }

    fn plan() -> DeploymentBootstrap {
        DeploymentBootstrap::plan(
            &Principal {
                id: "operator-a".into(),
            },
            &CapabilitySet::new([CAPABILITY_HOST_BOOTSTRAP]),
            intent(),
        )
        .unwrap()
    }

    #[test]
    fn session_proof_is_the_only_mutation_handoff() {
        let mut bootstrap = plan();
        assert!(!bootstrap.snapshot().mutation_authorized);
        bootstrap.begin().unwrap();
        let bootstrapped = bootstrap
            .accept_deployed(DaemonBootstrapReceipt {
                bootstrap_id: BootstrapId::new("bootstrap-1").unwrap(),
                daemon_id: DaemonId::new("daemon-host-example").unwrap(),
                endpoint: "https://host.example:9443/".into(),
                session_credential_handle: CredentialHandle::new("vault:leserpentd:host-example")
                    .unwrap(),
            })
            .unwrap();
        assert_eq!(bootstrapped.phase, BootstrapPhase::Bootstrapped);
        assert!(!bootstrapped.mutation_authorized);

        let mut wrong = DaemonSessionProof {
            bootstrap_id: BootstrapId::new("bootstrap-1").unwrap(),
            daemon_id: DaemonId::new("daemon-other").unwrap(),
            session_credential_handle: CredentialHandle::new("vault:leserpentd:host-example")
                .unwrap(),
            authority_owned: true,
            protocol_schema_version: BOOTSTRAP_SESSION_PROTOCOL_VERSION,
        };
        assert_eq!(
            bootstrap.bind_session(wrong.clone()),
            Err(BootstrapError::IdentityMismatch)
        );
        assert_eq!(bootstrap.snapshot().phase, BootstrapPhase::Bootstrapped);

        wrong.daemon_id = DaemonId::new("daemon-host-example").unwrap();
        let bound = bootstrap.bind_session(wrong).unwrap();
        assert_eq!(bound.phase, BootstrapPhase::SessionBound);
        assert!(bound.mutation_authorized);
        assert!(!bound.bootstrap_credential_present);
        bound.validate().unwrap();
    }

    #[test]
    fn bootstrap_requires_confirmation_capability_and_principal_binding() {
        let mut unconfirmed = intent();
        unconfirmed.confirmed = false;
        assert!(matches!(
            DeploymentBootstrap::plan(
                &Principal {
                    id: "operator-a".into()
                },
                &CapabilitySet::new([CAPABILITY_HOST_BOOTSTRAP]),
                unconfirmed,
            ),
            Err(BootstrapError::ConfirmationRequired)
        ));
        assert!(matches!(
            DeploymentBootstrap::plan(
                &Principal {
                    id: "operator-b".into()
                },
                &CapabilitySet::new([CAPABILITY_HOST_BOOTSTRAP]),
                intent(),
            ),
            Err(BootstrapError::PrincipalMismatch)
        ));
        assert!(matches!(
            DeploymentBootstrap::plan(
                &Principal {
                    id: "operator-a".into()
                },
                &CapabilitySet::default(),
                intent(),
            ),
            Err(BootstrapError::Unauthorized)
        ));
    }

    #[test]
    fn transport_failure_discards_all_authority_handles() {
        let mut bootstrap = plan();
        bootstrap.begin().unwrap();
        let failed = bootstrap.record_fault("transport_failure").unwrap();
        assert_eq!(failed.phase, BootstrapPhase::Failed);
        assert!(!failed.bootstrap_credential_present);
        assert!(failed.daemon_id.is_none());
        assert!(failed.session_credential_handle.is_none());
        assert!(!failed.mutation_authorized);
        failed.validate().unwrap();
    }

    #[test]
    fn endpoint_and_snapshot_validation_fail_closed() {
        let mut bootstrap = plan();
        bootstrap.begin().unwrap();
        assert_eq!(
            bootstrap.accept_deployed(DaemonBootstrapReceipt {
                bootstrap_id: BootstrapId::new("bootstrap-1").unwrap(),
                daemon_id: DaemonId::new("daemon-host-example").unwrap(),
                endpoint: "https://user:secret@host.example:9443/".into(),
                session_credential_handle: CredentialHandle::new("vault:leserpentd:daemon")
                    .unwrap(),
            }),
            Err(BootstrapError::InvalidEndpoint)
        );
        assert_eq!(bootstrap.snapshot().phase, BootstrapPhase::Deploying);

        let mut invalid = bootstrap.snapshot();
        invalid.mutation_authorized = true;
        assert_eq!(invalid.validate(), Err(BootstrapError::InvalidSnapshot));
    }

    #[test]
    fn credential_handles_cannot_be_raw_secrets() {
        assert_eq!(
            CredentialHandle::new("print123"),
            Err(BootstrapError::InvalidCredentialHandle)
        );
        assert!(CredentialHandle::new("vault:ssh:host-example").is_ok());
    }
}
