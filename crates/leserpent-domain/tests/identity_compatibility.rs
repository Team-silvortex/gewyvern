use leserpent_domain::bootstrap::{BootstrapError, CredentialHandle as DomainCredentialHandle};
use leserpent_domain::provisioning::{ProvisioningError, ProvisioningId as DomainProvisioningId};
use leserpent_domain::retirement::{RetirementError, RetirementId as DomainRetirementId};
use leserpent_domain::{DomainError, RuntimeId as DomainRuntimeId};
use silvortex_identity::{
    CredentialHandle, IdentityError, ProvisioningId, RetirementId, RuntimeId,
};

fn accepts_runtime(_: RuntimeId) {}
fn accepts_provisioning(_: ProvisioningId) {}
fn accepts_retirement(_: RetirementId) {}
fn accepts_credential(_: CredentialHandle) {}

#[test]
fn legacy_domain_paths_preserve_identity_type_and_wire_identity() {
    let runtime: DomainRuntimeId = RuntimeId::new("runtime-a").unwrap();
    let provisioning: DomainProvisioningId = ProvisioningId::new("provision-a").unwrap();
    let retirement: DomainRetirementId = RetirementId::new("retire-a").unwrap();
    let credential: DomainCredentialHandle = CredentialHandle::new("vault:ssh:host-a").unwrap();

    assert_eq!(serde_json::to_string(&runtime).unwrap(), "\"runtime-a\"");
    assert_eq!(
        serde_json::to_string(&provisioning).unwrap(),
        "\"provision-a\""
    );
    assert_eq!(serde_json::to_string(&retirement).unwrap(), "\"retire-a\"");
    assert_eq!(
        serde_json::to_string(&credential).unwrap(),
        "\"vault:ssh:host-a\""
    );

    accepts_runtime(runtime);
    accepts_provisioning(provisioning);
    accepts_retirement(retirement);
    accepts_credential(credential);
}

#[test]
fn domain_error_adapters_preserve_rejection_messages() {
    let runtime = RuntimeId::new("runtime/path").unwrap_err();
    assert_eq!(DomainError::from(runtime).to_string(), "invalid runtime_id");

    let provisioning = ProvisioningId::new("provision/path").unwrap_err();
    assert_eq!(
        ProvisioningError::from(provisioning).to_string(),
        "invalid provisioning_id"
    );

    let retirement = RetirementId::new("retire/path").unwrap_err();
    assert_eq!(
        RetirementError::from(retirement).to_string(),
        "invalid retirement_id"
    );

    let credential = CredentialHandle::new("raw-secret").unwrap_err();
    assert_eq!(
        BootstrapError::from(credential).to_string(),
        "invalid credential handle"
    );
}

#[test]
fn legacy_constructors_expose_the_neutral_error_contract() {
    let result: Result<DomainRuntimeId, IdentityError> = DomainRuntimeId::new("runtime-a");
    assert!(result.is_ok());
}
