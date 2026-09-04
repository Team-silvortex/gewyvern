use std::fmt;

use serde::{Deserialize, Serialize};
pub use silvortex_identity::RetirementId;

use crate::bootstrap::{BootstrapTarget, CredentialHandle};
use crate::provisioning::ProvisioningId;
use crate::{CapabilitySet, Principal, RuntimeId};

pub const RETIREMENT_DOMAIN_SCHEMA_VERSION: u32 = 1;
pub const RETIREMENT_CHECKPOINT_SCHEMA_VERSION: u32 = 1;
pub const CAPABILITY_RUNTIME_RETIRE: &str = "runtime.retire";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeRetirementIntent {
    pub schema_version: u32,
    pub retirement_id: RetirementId,
    pub provisioning_id: ProvisioningId,
    pub runtime_id: RuntimeId,
    pub target: BootstrapTarget,
    pub retirement_credential_handle: CredentialHandle,
    pub requested_by: String,
    pub confirmed: bool,
}

impl RuntimeRetirementIntent {
    pub fn validate(&self) -> Result<(), RetirementError> {
        if self.schema_version != RETIREMENT_DOMAIN_SCHEMA_VERSION {
            return Err(RetirementError::InvalidSchemaVersion {
                actual: self.schema_version,
                expected: RETIREMENT_DOMAIN_SCHEMA_VERSION,
            });
        }
        validate_identifier("requested_by", self.requested_by.clone())?;
        self.target
            .validate()
            .map_err(|_| RetirementError::InvalidTarget)?;
        if self.retirement_credential_handle.parts().0 != "ssh" {
            return Err(RetirementError::InvalidCredentialHandle);
        }
        if !self.confirmed {
            return Err(RetirementError::ConfirmationRequired);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RetirementPhase {
    Planned,
    RetiringService,
    ServiceRetired,
    RuntimeUnregistered,
    Failed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GewyvernRetirementReceipt {
    pub retirement_id: RetirementId,
    pub provisioning_id: ProvisioningId,
    pub runtime_id: RuntimeId,
    pub service_retired: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeRetirementSnapshot {
    pub retirement_id: RetirementId,
    pub provisioning_id: ProvisioningId,
    pub runtime_id: RuntimeId,
    pub phase: RetirementPhase,
    pub target: BootstrapTarget,
    pub retirement_credential_present: bool,
    pub service_retired: bool,
    pub runtime_registered: bool,
    pub fault_code: Option<String>,
}

impl RuntimeRetirementSnapshot {
    pub fn validate(&self) -> Result<(), RetirementError> {
        self.target
            .validate()
            .map_err(|_| RetirementError::InvalidTarget)?;
        match self.phase {
            RetirementPhase::Planned | RetirementPhase::RetiringService
                if self.retirement_credential_present
                    && !self.service_retired
                    && self.runtime_registered
                    && self.fault_code.is_none() =>
            {
                Ok(())
            }
            RetirementPhase::ServiceRetired
                if !self.retirement_credential_present
                    && self.service_retired
                    && self.runtime_registered
                    && self.fault_code.is_none() =>
            {
                Ok(())
            }
            RetirementPhase::RuntimeUnregistered
                if !self.retirement_credential_present
                    && self.service_retired
                    && !self.runtime_registered
                    && self.fault_code.is_none() =>
            {
                Ok(())
            }
            RetirementPhase::Failed
                if !self.retirement_credential_present
                    && !self.service_retired
                    && self.runtime_registered
                    && self
                        .fault_code
                        .as_deref()
                        .is_some_and(|value| validate_fault(value).is_ok()) =>
            {
                Ok(())
            }
            _ => Err(RetirementError::InvalidSnapshot),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeRetirementCheckpoint {
    pub schema_version: u32,
    pub revision: u64,
    pub state: RuntimeRetirementSnapshot,
    pub retirement_credential_handle: Option<CredentialHandle>,
}

impl RuntimeRetirementCheckpoint {
    pub fn new(
        revision: u64,
        state: RuntimeRetirementSnapshot,
        retirement_credential_handle: Option<CredentialHandle>,
    ) -> Result<Self, RetirementError> {
        let checkpoint = Self {
            schema_version: RETIREMENT_CHECKPOINT_SCHEMA_VERSION,
            revision,
            state,
            retirement_credential_handle,
        };
        checkpoint.validate()?;
        Ok(checkpoint)
    }

    pub fn validate(&self) -> Result<(), RetirementError> {
        if self.schema_version != RETIREMENT_CHECKPOINT_SCHEMA_VERSION {
            return Err(RetirementError::InvalidSchemaVersion {
                actual: self.schema_version,
                expected: RETIREMENT_CHECKPOINT_SCHEMA_VERSION,
            });
        }
        if self.revision == 0
            || self.state.retirement_credential_present
                != self.retirement_credential_handle.is_some()
        {
            return Err(RetirementError::InvalidCheckpoint);
        }
        if self
            .retirement_credential_handle
            .as_ref()
            .is_some_and(|handle| handle.parts().0 != "ssh")
        {
            return Err(RetirementError::InvalidCredentialHandle);
        }
        self.state.validate()
    }
}

#[derive(Clone, Debug)]
pub struct RuntimeRetirement {
    retirement_id: RetirementId,
    provisioning_id: ProvisioningId,
    runtime_id: RuntimeId,
    target: BootstrapTarget,
    retirement_credential_handle: Option<CredentialHandle>,
    phase: RetirementPhase,
    service_retired: bool,
    runtime_registered: bool,
    fault_code: Option<String>,
}

impl RuntimeRetirement {
    pub fn plan(
        principal: &Principal,
        capabilities: &CapabilitySet,
        intent: RuntimeRetirementIntent,
    ) -> Result<Self, RetirementError> {
        intent.validate()?;
        validate_identifier("principal.id", principal.id.clone())?;
        if principal.id != intent.requested_by {
            return Err(RetirementError::PrincipalMismatch);
        }
        if !capabilities.contains(CAPABILITY_RUNTIME_RETIRE) {
            return Err(RetirementError::Unauthorized);
        }
        Ok(Self {
            retirement_id: intent.retirement_id,
            provisioning_id: intent.provisioning_id,
            runtime_id: intent.runtime_id,
            target: intent.target,
            retirement_credential_handle: Some(intent.retirement_credential_handle),
            phase: RetirementPhase::Planned,
            service_retired: false,
            runtime_registered: true,
            fault_code: None,
        })
    }

    pub fn begin(&mut self) -> Result<RuntimeRetirementSnapshot, RetirementError> {
        self.require_phase(RetirementPhase::Planned)?;
        self.phase = RetirementPhase::RetiringService;
        Ok(self.snapshot())
    }

    pub fn accept_service_retirement(
        &mut self,
        receipt: GewyvernRetirementReceipt,
    ) -> Result<RuntimeRetirementSnapshot, RetirementError> {
        self.require_phase(RetirementPhase::RetiringService)?;
        if receipt.retirement_id != self.retirement_id
            || receipt.provisioning_id != self.provisioning_id
            || receipt.runtime_id != self.runtime_id
            || !receipt.service_retired
        {
            return Err(RetirementError::IdentityMismatch);
        }
        self.retirement_credential_handle = None;
        self.service_retired = true;
        self.phase = RetirementPhase::ServiceRetired;
        Ok(self.snapshot())
    }

    pub fn accept_runtime_unregistration(
        &mut self,
    ) -> Result<RuntimeRetirementSnapshot, RetirementError> {
        if self.phase == RetirementPhase::RuntimeUnregistered {
            return Ok(self.snapshot());
        }
        self.require_phase(RetirementPhase::ServiceRetired)?;
        self.runtime_registered = false;
        self.phase = RetirementPhase::RuntimeUnregistered;
        Ok(self.snapshot())
    }

    pub fn record_fault(
        &mut self,
        fault_code: impl Into<String>,
    ) -> Result<RuntimeRetirementSnapshot, RetirementError> {
        if !matches!(
            self.phase,
            RetirementPhase::Planned | RetirementPhase::RetiringService
        ) {
            return Err(RetirementError::InvalidTransition { actual: self.phase });
        }
        let fault_code = fault_code.into();
        validate_fault(&fault_code)?;
        self.retirement_credential_handle = None;
        self.fault_code = Some(fault_code);
        self.phase = RetirementPhase::Failed;
        Ok(self.snapshot())
    }

    pub fn checkpoint(
        &self,
        revision: u64,
    ) -> Result<RuntimeRetirementCheckpoint, RetirementError> {
        RuntimeRetirementCheckpoint::new(
            revision,
            self.snapshot(),
            self.retirement_credential_handle.clone(),
        )
    }

    pub fn resume(checkpoint: &RuntimeRetirementCheckpoint) -> Result<Self, RetirementError> {
        checkpoint.validate()?;
        Ok(Self {
            retirement_id: checkpoint.state.retirement_id.clone(),
            provisioning_id: checkpoint.state.provisioning_id.clone(),
            runtime_id: checkpoint.state.runtime_id.clone(),
            target: checkpoint.state.target.clone(),
            retirement_credential_handle: checkpoint.retirement_credential_handle.clone(),
            phase: checkpoint.state.phase,
            service_retired: checkpoint.state.service_retired,
            runtime_registered: checkpoint.state.runtime_registered,
            fault_code: checkpoint.state.fault_code.clone(),
        })
    }

    pub fn snapshot(&self) -> RuntimeRetirementSnapshot {
        RuntimeRetirementSnapshot {
            retirement_id: self.retirement_id.clone(),
            provisioning_id: self.provisioning_id.clone(),
            runtime_id: self.runtime_id.clone(),
            phase: self.phase,
            target: self.target.clone(),
            retirement_credential_present: self.retirement_credential_handle.is_some(),
            service_retired: self.service_retired,
            runtime_registered: self.runtime_registered,
            fault_code: self.fault_code.clone(),
        }
    }

    fn require_phase(&self, expected: RetirementPhase) -> Result<(), RetirementError> {
        if self.phase != expected {
            return Err(RetirementError::InvalidTransition { actual: self.phase });
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RetirementError {
    InvalidIdentifier { field: &'static str },
    InvalidSchemaVersion { actual: u32, expected: u32 },
    InvalidTarget,
    InvalidCredentialHandle,
    InvalidFaultCode,
    InvalidSnapshot,
    InvalidCheckpoint,
    ConfirmationRequired,
    PrincipalMismatch,
    Unauthorized,
    InvalidTransition { actual: RetirementPhase },
    IdentityMismatch,
}

impl fmt::Display for RetirementError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidIdentifier { field } => write!(formatter, "invalid {field}"),
            Self::InvalidSchemaVersion { actual, expected } => write!(
                formatter,
                "unsupported retirement schema {actual}, expected {expected}"
            ),
            Self::InvalidTarget => formatter.write_str("invalid retirement target"),
            Self::InvalidCredentialHandle => {
                formatter.write_str("retirement credential must use the SSH vault provider")
            }
            Self::InvalidFaultCode => formatter.write_str("invalid retirement fault code"),
            Self::InvalidSnapshot => formatter.write_str("invalid retirement snapshot"),
            Self::InvalidCheckpoint => formatter.write_str("invalid retirement checkpoint"),
            Self::ConfirmationRequired => formatter.write_str("retirement requires confirmation"),
            Self::PrincipalMismatch => formatter.write_str("retirement principal mismatch"),
            Self::Unauthorized => formatter.write_str("runtime retirement capability is required"),
            Self::InvalidTransition { actual } => {
                write!(formatter, "invalid retirement transition from {actual:?}")
            }
            Self::IdentityMismatch => formatter.write_str("retirement identity mismatch"),
        }
    }
}

impl std::error::Error for RetirementError {}

impl From<silvortex_identity::IdentityError> for RetirementError {
    fn from(error: silvortex_identity::IdentityError) -> Self {
        match error {
            silvortex_identity::IdentityError::InvalidIdentifier { field } => {
                Self::InvalidIdentifier { field }
            }
            silvortex_identity::IdentityError::InvalidCredentialHandle => {
                Self::InvalidCredentialHandle
            }
        }
    }
}

fn validate_identifier(field: &'static str, value: String) -> Result<String, RetirementError> {
    silvortex_identity::validate_identifier(field, value).map_err(RetirementError::from)
}

fn validate_fault(value: &str) -> Result<(), RetirementError> {
    if value.is_empty()
        || value.len() > 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return Err(RetirementError::InvalidFaultCode);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::bootstrap::{BootstrapTransport, CAPABILITY_HOST_BOOTSTRAP};

    use super::*;

    fn intent() -> RuntimeRetirementIntent {
        RuntimeRetirementIntent {
            schema_version: RETIREMENT_DOMAIN_SCHEMA_VERSION,
            retirement_id: RetirementId::new("retire-1").unwrap(),
            provisioning_id: ProvisioningId::new("provision-1").unwrap(),
            runtime_id: RuntimeId::new("runtime-1").unwrap(),
            target: BootstrapTarget {
                transport: BootstrapTransport::Ssh,
                host: "runtime.example".into(),
                port: 22,
            },
            retirement_credential_handle: CredentialHandle::new("vault:ssh:runtime").unwrap(),
            requested_by: "operator-a".into(),
            confirmed: true,
        }
    }

    fn retirement() -> RuntimeRetirement {
        RuntimeRetirement::plan(
            &Principal {
                id: "operator-a".into(),
            },
            &CapabilitySet::new([CAPABILITY_RUNTIME_RETIRE]),
            intent(),
        )
        .unwrap()
    }

    fn receipt() -> GewyvernRetirementReceipt {
        GewyvernRetirementReceipt {
            retirement_id: RetirementId::new("retire-1").unwrap(),
            provisioning_id: ProvisioningId::new("provision-1").unwrap(),
            runtime_id: RuntimeId::new("runtime-1").unwrap(),
            service_retired: true,
        }
    }

    #[test]
    fn retirement_requires_confirmation_capability_and_matching_principal() {
        let principal = Principal {
            id: "operator-a".into(),
        };
        let capabilities = CapabilitySet::new([CAPABILITY_RUNTIME_RETIRE]);
        assert!(RuntimeRetirement::plan(&principal, &capabilities, intent()).is_ok());
        assert!(matches!(
            RuntimeRetirement::plan(
                &principal,
                &capabilities,
                RuntimeRetirementIntent {
                    confirmed: false,
                    ..intent()
                }
            ),
            Err(RetirementError::ConfirmationRequired)
        ));
        assert!(matches!(
            RuntimeRetirement::plan(
                &principal,
                &CapabilitySet::new([CAPABILITY_HOST_BOOTSTRAP]),
                intent()
            ),
            Err(RetirementError::Unauthorized)
        ));
    }

    #[test]
    fn service_is_retired_before_runtime_unregistration() {
        let mut retirement = retirement();
        assert_eq!(
            retirement.begin().unwrap().phase,
            RetirementPhase::RetiringService
        );
        assert_eq!(
            retirement.accept_runtime_unregistration(),
            Err(RetirementError::InvalidTransition {
                actual: RetirementPhase::RetiringService
            })
        );
        let retired = retirement.accept_service_retirement(receipt()).unwrap();
        assert!(retired.service_retired && retired.runtime_registered);
        assert!(!retired.retirement_credential_present);
        let unregistered = retirement.accept_runtime_unregistration().unwrap();
        assert_eq!(unregistered.phase, RetirementPhase::RuntimeUnregistered);
        assert!(unregistered.service_retired && !unregistered.runtime_registered);
    }

    #[test]
    fn retirement_receipt_is_bound_to_all_operation_identities() {
        let mut retirement = retirement();
        retirement.begin().unwrap();
        let mut wrong = receipt();
        wrong.runtime_id = RuntimeId::new("runtime-other").unwrap();
        assert_eq!(
            retirement.accept_service_retirement(wrong),
            Err(RetirementError::IdentityMismatch)
        );
    }

    #[test]
    fn failure_preserves_control_plane_registration_and_retires_credential() {
        let mut retirement = retirement();
        retirement.begin().unwrap();
        let failed = retirement.record_fault("service_stop_failed").unwrap();
        assert_eq!(failed.phase, RetirementPhase::Failed);
        assert!(failed.runtime_registered && !failed.service_retired);
        assert!(!failed.retirement_credential_present);
    }

    #[test]
    fn checkpoints_restore_each_safe_boundary() {
        let mut retirement = retirement();
        retirement.begin().unwrap();
        let installing = retirement.checkpoint(2).unwrap();
        assert_eq!(
            RuntimeRetirement::resume(&installing).unwrap().snapshot(),
            installing.state
        );
        retirement.accept_service_retirement(receipt()).unwrap();
        let retired = retirement.checkpoint(3).unwrap();
        assert_eq!(
            RuntimeRetirement::resume(&retired).unwrap().snapshot(),
            retired.state
        );
    }

    #[test]
    fn retirement_rejects_non_ssh_handles_and_inconsistent_snapshots() {
        assert!(matches!(
            RuntimeRetirement::plan(
                &Principal {
                    id: "operator-a".into()
                },
                &CapabilitySet::new([CAPABILITY_RUNTIME_RETIRE]),
                RuntimeRetirementIntent {
                    retirement_credential_handle: CredentialHandle::new("vault:gewyvern:runtime")
                        .unwrap(),
                    ..intent()
                }
            ),
            Err(RetirementError::InvalidCredentialHandle)
        ));
        let mut invalid = retirement().snapshot();
        invalid.runtime_registered = false;
        assert_eq!(invalid.validate(), Err(RetirementError::InvalidSnapshot));
    }
}
