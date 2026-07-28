use std::fmt;

use serde::{Deserialize, Serialize};

use crate::bootstrap::{
    BootstrapId, BootstrapPhase, BootstrapTarget, CredentialHandle, DaemonId,
    DeploymentBootstrapCheckpoint,
};
use crate::retirement::RetirementId;
use crate::{CapabilitySet, Principal};

pub const DAEMON_RETIREMENT_DOMAIN_SCHEMA_VERSION: u32 = 1;
pub const DAEMON_RETIREMENT_CHECKPOINT_SCHEMA_VERSION: u32 = 1;
pub const CAPABILITY_HOST_RETIRE: &str = "host.retire";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DaemonRetirementIntent {
    pub schema_version: u32,
    pub retirement_id: RetirementId,
    pub bootstrap_id: BootstrapId,
    pub retirement_credential_handle: CredentialHandle,
    pub requested_by: String,
    pub confirmed: bool,
}

impl DaemonRetirementIntent {
    pub fn validate(&self) -> Result<(), DaemonRetirementError> {
        if self.schema_version != DAEMON_RETIREMENT_DOMAIN_SCHEMA_VERSION {
            return Err(DaemonRetirementError::InvalidSchemaVersion {
                actual: self.schema_version,
                expected: DAEMON_RETIREMENT_DOMAIN_SCHEMA_VERSION,
            });
        }
        validate_identifier("requested_by", &self.requested_by)?;
        if self.retirement_credential_handle.parts().0 != "ssh" {
            return Err(DaemonRetirementError::InvalidCredentialHandle);
        }
        if !self.confirmed {
            return Err(DaemonRetirementError::ConfirmationRequired);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DaemonRetirementPhase {
    Planned,
    RetiringService,
    ServiceRetired,
    Failed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DaemonRetirementReceipt {
    pub retirement_id: RetirementId,
    pub bootstrap_id: BootstrapId,
    pub daemon_id: DaemonId,
    pub generation: String,
    pub service_retired: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DaemonRetirementSnapshot {
    pub retirement_id: RetirementId,
    pub bootstrap_id: BootstrapId,
    pub daemon_id: DaemonId,
    pub phase: DaemonRetirementPhase,
    pub target: BootstrapTarget,
    pub generation: String,
    pub install_profile: String,
    pub retirement_credential_present: bool,
    pub service_retired: bool,
    pub fault_code: Option<String>,
}

impl DaemonRetirementSnapshot {
    pub fn validate(&self) -> Result<(), DaemonRetirementError> {
        self.target
            .validate()
            .map_err(|_| DaemonRetirementError::InvalidTarget)?;
        validate_generation(&self.generation)?;
        validate_install_profile(&self.install_profile)?;
        match self.phase {
            DaemonRetirementPhase::Planned | DaemonRetirementPhase::RetiringService
                if self.retirement_credential_present
                    && !self.service_retired
                    && self.fault_code.is_none() =>
            {
                Ok(())
            }
            DaemonRetirementPhase::ServiceRetired
                if !self.retirement_credential_present
                    && self.service_retired
                    && self.fault_code.is_none() =>
            {
                Ok(())
            }
            DaemonRetirementPhase::Failed
                if !self.retirement_credential_present
                    && !self.service_retired
                    && self
                        .fault_code
                        .as_deref()
                        .is_some_and(|value| validate_fault(value).is_ok()) =>
            {
                Ok(())
            }
            _ => Err(DaemonRetirementError::InvalidSnapshot),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DaemonRetirementCheckpoint {
    pub schema_version: u32,
    pub revision: u64,
    pub state: DaemonRetirementSnapshot,
    pub retirement_credential_handle: Option<CredentialHandle>,
}

impl DaemonRetirementCheckpoint {
    pub fn new(
        revision: u64,
        state: DaemonRetirementSnapshot,
        retirement_credential_handle: Option<CredentialHandle>,
    ) -> Result<Self, DaemonRetirementError> {
        let checkpoint = Self {
            schema_version: DAEMON_RETIREMENT_CHECKPOINT_SCHEMA_VERSION,
            revision,
            state,
            retirement_credential_handle,
        };
        checkpoint.validate()?;
        Ok(checkpoint)
    }

    pub fn validate(&self) -> Result<(), DaemonRetirementError> {
        if self.schema_version != DAEMON_RETIREMENT_CHECKPOINT_SCHEMA_VERSION {
            return Err(DaemonRetirementError::InvalidSchemaVersion {
                actual: self.schema_version,
                expected: DAEMON_RETIREMENT_CHECKPOINT_SCHEMA_VERSION,
            });
        }
        if self.revision == 0
            || self.state.retirement_credential_present
                != self.retirement_credential_handle.is_some()
        {
            return Err(DaemonRetirementError::InvalidCheckpoint);
        }
        if self
            .retirement_credential_handle
            .as_ref()
            .is_some_and(|handle| handle.parts().0 != "ssh")
        {
            return Err(DaemonRetirementError::InvalidCredentialHandle);
        }
        self.state.validate()
    }
}

#[derive(Clone, Debug)]
pub struct DaemonRetirement {
    retirement_id: RetirementId,
    bootstrap_id: BootstrapId,
    daemon_id: DaemonId,
    target: BootstrapTarget,
    generation: String,
    install_profile: String,
    retirement_credential_handle: Option<CredentialHandle>,
    phase: DaemonRetirementPhase,
    service_retired: bool,
    fault_code: Option<String>,
}

impl DaemonRetirement {
    pub fn plan(
        principal: &Principal,
        capabilities: &CapabilitySet,
        intent: DaemonRetirementIntent,
        deployment: &DeploymentBootstrapCheckpoint,
    ) -> Result<Self, DaemonRetirementError> {
        intent.validate()?;
        validate_identifier("principal.id", &principal.id)?;
        if principal.id != intent.requested_by {
            return Err(DaemonRetirementError::PrincipalMismatch);
        }
        if !capabilities.contains(CAPABILITY_HOST_RETIRE) {
            return Err(DaemonRetirementError::Unauthorized);
        }
        deployment
            .validate()
            .map_err(|_| DaemonRetirementError::DeploymentAuthorityRejected)?;
        if deployment.state.bootstrap_id != intent.bootstrap_id
            || deployment.state.phase != BootstrapPhase::SessionBound
            || !deployment.state.mutation_authorized
        {
            return Err(DaemonRetirementError::DeploymentAuthorityRejected);
        }
        let daemon_id = deployment
            .state
            .daemon_id
            .clone()
            .ok_or(DaemonRetirementError::DeploymentAuthorityRejected)?;
        let generation = deployment
            .state
            .generation
            .clone()
            .ok_or(DaemonRetirementError::LegacyDeploymentIneligible)?;
        let install_profile = deployment
            .state
            .install_profile
            .clone()
            .ok_or(DaemonRetirementError::LegacyDeploymentIneligible)?;
        validate_generation(&generation)?;
        validate_install_profile(&install_profile)?;
        Ok(Self {
            retirement_id: intent.retirement_id,
            bootstrap_id: intent.bootstrap_id,
            daemon_id,
            target: deployment.state.target.clone(),
            generation,
            install_profile,
            retirement_credential_handle: Some(intent.retirement_credential_handle),
            phase: DaemonRetirementPhase::Planned,
            service_retired: false,
            fault_code: None,
        })
    }

    pub fn begin(&mut self) -> Result<DaemonRetirementSnapshot, DaemonRetirementError> {
        self.require_phase(DaemonRetirementPhase::Planned)?;
        self.phase = DaemonRetirementPhase::RetiringService;
        Ok(self.snapshot())
    }

    pub fn accept_service_retirement(
        &mut self,
        receipt: DaemonRetirementReceipt,
    ) -> Result<DaemonRetirementSnapshot, DaemonRetirementError> {
        self.require_phase(DaemonRetirementPhase::RetiringService)?;
        if receipt.retirement_id != self.retirement_id
            || receipt.bootstrap_id != self.bootstrap_id
            || receipt.daemon_id != self.daemon_id
            || receipt.generation != self.generation
            || !receipt.service_retired
        {
            return Err(DaemonRetirementError::IdentityMismatch);
        }
        self.retirement_credential_handle = None;
        self.service_retired = true;
        self.phase = DaemonRetirementPhase::ServiceRetired;
        Ok(self.snapshot())
    }

    pub fn record_fault(
        &mut self,
        fault_code: impl Into<String>,
    ) -> Result<DaemonRetirementSnapshot, DaemonRetirementError> {
        if !matches!(
            self.phase,
            DaemonRetirementPhase::Planned | DaemonRetirementPhase::RetiringService
        ) {
            return Err(DaemonRetirementError::InvalidTransition { actual: self.phase });
        }
        let fault_code = fault_code.into();
        validate_fault(&fault_code)?;
        self.retirement_credential_handle = None;
        self.fault_code = Some(fault_code);
        self.phase = DaemonRetirementPhase::Failed;
        Ok(self.snapshot())
    }

    pub fn checkpoint(
        &self,
        revision: u64,
    ) -> Result<DaemonRetirementCheckpoint, DaemonRetirementError> {
        DaemonRetirementCheckpoint::new(
            revision,
            self.snapshot(),
            self.retirement_credential_handle.clone(),
        )
    }

    pub fn resume(checkpoint: &DaemonRetirementCheckpoint) -> Result<Self, DaemonRetirementError> {
        checkpoint.validate()?;
        Ok(Self {
            retirement_id: checkpoint.state.retirement_id.clone(),
            bootstrap_id: checkpoint.state.bootstrap_id.clone(),
            daemon_id: checkpoint.state.daemon_id.clone(),
            target: checkpoint.state.target.clone(),
            generation: checkpoint.state.generation.clone(),
            install_profile: checkpoint.state.install_profile.clone(),
            retirement_credential_handle: checkpoint.retirement_credential_handle.clone(),
            phase: checkpoint.state.phase,
            service_retired: checkpoint.state.service_retired,
            fault_code: checkpoint.state.fault_code.clone(),
        })
    }

    pub fn snapshot(&self) -> DaemonRetirementSnapshot {
        DaemonRetirementSnapshot {
            retirement_id: self.retirement_id.clone(),
            bootstrap_id: self.bootstrap_id.clone(),
            daemon_id: self.daemon_id.clone(),
            phase: self.phase,
            target: self.target.clone(),
            generation: self.generation.clone(),
            install_profile: self.install_profile.clone(),
            retirement_credential_present: self.retirement_credential_handle.is_some(),
            service_retired: self.service_retired,
            fault_code: self.fault_code.clone(),
        }
    }

    fn require_phase(&self, expected: DaemonRetirementPhase) -> Result<(), DaemonRetirementError> {
        if self.phase != expected {
            return Err(DaemonRetirementError::InvalidTransition { actual: self.phase });
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DaemonRetirementError {
    InvalidIdentifier { field: &'static str },
    InvalidSchemaVersion { actual: u32, expected: u32 },
    InvalidTarget,
    InvalidCredentialHandle,
    InvalidGeneration,
    InvalidInstallProfile,
    InvalidFaultCode,
    InvalidSnapshot,
    InvalidCheckpoint,
    ConfirmationRequired,
    PrincipalMismatch,
    Unauthorized,
    DeploymentAuthorityRejected,
    LegacyDeploymentIneligible,
    InvalidTransition { actual: DaemonRetirementPhase },
    IdentityMismatch,
}

impl fmt::Display for DaemonRetirementError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidIdentifier { field } => write!(formatter, "invalid {field}"),
            Self::InvalidSchemaVersion { actual, expected } => write!(
                formatter,
                "unsupported daemon retirement schema {actual}, expected {expected}"
            ),
            Self::InvalidTarget => formatter.write_str("invalid daemon retirement target"),
            Self::InvalidCredentialHandle => {
                formatter.write_str("daemon retirement credential must use the SSH vault provider")
            }
            Self::InvalidGeneration => formatter.write_str("invalid daemon generation"),
            Self::InvalidInstallProfile => formatter.write_str("invalid daemon install profile"),
            Self::InvalidFaultCode => formatter.write_str("invalid daemon retirement fault code"),
            Self::InvalidSnapshot => formatter.write_str("invalid daemon retirement snapshot"),
            Self::InvalidCheckpoint => formatter.write_str("invalid daemon retirement checkpoint"),
            Self::ConfirmationRequired => {
                formatter.write_str("daemon retirement requires confirmation")
            }
            Self::PrincipalMismatch => formatter.write_str("daemon retirement principal mismatch"),
            Self::Unauthorized => formatter.write_str("host retirement capability is required"),
            Self::DeploymentAuthorityRejected => {
                formatter.write_str("bound deployment authority is required")
            }
            Self::LegacyDeploymentIneligible => {
                formatter.write_str("legacy deployment lacks retirement authority")
            }
            Self::InvalidTransition { actual } => {
                write!(
                    formatter,
                    "invalid daemon retirement transition from {actual:?}"
                )
            }
            Self::IdentityMismatch => formatter.write_str("daemon retirement identity mismatch"),
        }
    }
}

impl std::error::Error for DaemonRetirementError {}

fn validate_identifier(field: &'static str, value: &str) -> Result<(), DaemonRetirementError> {
    let valid = !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':' | b'.'));
    valid
        .then_some(())
        .ok_or(DaemonRetirementError::InvalidIdentifier { field })
}

fn validate_generation(value: &str) -> Result<(), DaemonRetirementError> {
    let valid = value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'));
    valid
        .then_some(())
        .ok_or(DaemonRetirementError::InvalidGeneration)
}

fn validate_install_profile(value: &str) -> Result<(), DaemonRetirementError> {
    matches!(value, "system" | "user" | "test")
        .then_some(())
        .ok_or(DaemonRetirementError::InvalidInstallProfile)
}

fn validate_fault(value: &str) -> Result<(), DaemonRetirementError> {
    let valid = !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_');
    valid
        .then_some(())
        .ok_or(DaemonRetirementError::InvalidFaultCode)
}

#[cfg(test)]
mod tests {
    use crate::bootstrap::{
        BOOTSTRAP_CHECKPOINT_SCHEMA_VERSION, BootstrapTransport, DeploymentBootstrapSnapshot,
    };

    use super::*;

    fn deployment() -> DeploymentBootstrapCheckpoint {
        DeploymentBootstrapCheckpoint {
            schema_version: BOOTSTRAP_CHECKPOINT_SCHEMA_VERSION,
            revision: 3,
            state: DeploymentBootstrapSnapshot {
                bootstrap_id: BootstrapId::new("bootstrap-1").unwrap(),
                phase: BootstrapPhase::SessionBound,
                target: BootstrapTarget {
                    transport: BootstrapTransport::Ssh,
                    host: "host.example".into(),
                    port: 22,
                },
                bootstrap_credential_present: false,
                daemon_id: Some(DaemonId::new("daemon-host").unwrap()),
                endpoint: Some("https://host.example:9443/".into()),
                generation: Some("a".repeat(64)),
                install_profile: Some("system".into()),
                session_credential_handle: Some(
                    CredentialHandle::new("vault:leserpentd:host").unwrap(),
                ),
                trust_credential_handle: Some(
                    CredentialHandle::new("vault:leserpent-ca:host").unwrap(),
                ),
                fault_code: None,
                mutation_authorized: true,
            },
            bootstrap_credential_handle: None,
        }
    }

    fn intent() -> DaemonRetirementIntent {
        DaemonRetirementIntent {
            schema_version: DAEMON_RETIREMENT_DOMAIN_SCHEMA_VERSION,
            retirement_id: RetirementId::new("retire-daemon-1").unwrap(),
            bootstrap_id: BootstrapId::new("bootstrap-1").unwrap(),
            retirement_credential_handle: CredentialHandle::new("vault:ssh:host").unwrap(),
            requested_by: "operator-a".into(),
            confirmed: true,
        }
    }

    fn plan() -> DaemonRetirement {
        DaemonRetirement::plan(
            &Principal {
                id: "operator-a".into(),
            },
            &CapabilitySet::new([CAPABILITY_HOST_RETIRE]),
            intent(),
            &deployment(),
        )
        .unwrap()
    }

    #[test]
    fn plan_derives_all_target_authority_from_the_bound_deployment() {
        let state = plan().snapshot();
        assert_eq!(state.target.host, "host.example");
        assert_eq!(state.daemon_id.as_str(), "daemon-host");
        assert_eq!(state.generation, "a".repeat(64));
        assert_eq!(state.install_profile, "system");
        assert!(state.retirement_credential_present);
        state.validate().unwrap();
    }

    #[test]
    fn legacy_or_unbound_deployments_are_ineligible() {
        let mut legacy = deployment();
        legacy.state.generation = None;
        legacy.state.install_profile = None;
        assert!(matches!(
            DaemonRetirement::plan(
                &Principal {
                    id: "operator-a".into()
                },
                &CapabilitySet::new([CAPABILITY_HOST_RETIRE]),
                intent(),
                &legacy,
            ),
            Err(DaemonRetirementError::LegacyDeploymentIneligible)
        ));

        let mut unbound = deployment();
        unbound.state.phase = BootstrapPhase::Bootstrapped;
        unbound.state.bootstrap_credential_present = true;
        unbound.state.mutation_authorized = false;
        unbound.bootstrap_credential_handle =
            Some(CredentialHandle::new("vault:ssh:host").unwrap());
        assert!(matches!(
            DaemonRetirement::plan(
                &Principal {
                    id: "operator-a".into()
                },
                &CapabilitySet::new([CAPABILITY_HOST_RETIRE]),
                intent(),
                &unbound,
            ),
            Err(DaemonRetirementError::DeploymentAuthorityRejected)
        ));
    }

    #[test]
    fn receipt_is_bound_to_all_derived_authority() {
        let mut retirement = plan();
        retirement.begin().unwrap();
        let mut receipt = DaemonRetirementReceipt {
            retirement_id: RetirementId::new("retire-daemon-1").unwrap(),
            bootstrap_id: BootstrapId::new("bootstrap-1").unwrap(),
            daemon_id: DaemonId::new("daemon-host").unwrap(),
            generation: "a".repeat(64),
            service_retired: true,
        };
        receipt.generation = "b".repeat(64);
        assert_eq!(
            retirement.accept_service_retirement(receipt),
            Err(DaemonRetirementError::IdentityMismatch)
        );
        let retired = retirement
            .accept_service_retirement(DaemonRetirementReceipt {
                retirement_id: RetirementId::new("retire-daemon-1").unwrap(),
                bootstrap_id: BootstrapId::new("bootstrap-1").unwrap(),
                daemon_id: DaemonId::new("daemon-host").unwrap(),
                generation: "a".repeat(64),
                service_retired: true,
            })
            .unwrap();
        assert_eq!(retired.phase, DaemonRetirementPhase::ServiceRetired);
        assert!(!retired.retirement_credential_present);
    }

    #[test]
    fn public_intent_requires_confirmation_capability_and_ssh_handle() {
        let principal = Principal {
            id: "operator-a".into(),
        };
        let mut invalid = intent();
        invalid.confirmed = false;
        assert!(matches!(
            DaemonRetirement::plan(
                &principal,
                &CapabilitySet::new([CAPABILITY_HOST_RETIRE]),
                invalid,
                &deployment(),
            ),
            Err(DaemonRetirementError::ConfirmationRequired)
        ));
        assert!(matches!(
            DaemonRetirement::plan(
                &principal,
                &CapabilitySet::default(),
                intent(),
                &deployment(),
            ),
            Err(DaemonRetirementError::Unauthorized)
        ));
    }
}
