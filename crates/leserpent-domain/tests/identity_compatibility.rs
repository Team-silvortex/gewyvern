use leselang_host_contract::{
    CapabilitySet as HostCapabilitySet, CommandOrigin as HostCommandOrigin,
    Confirmation as HostConfirmation, Principal as HostPrincipal, Revision as HostRevision,
    RuntimeListFilter as HostRuntimeListFilter,
};
use leserpent_domain::bootstrap::{BootstrapError, CredentialHandle as DomainCredentialHandle};
use leserpent_domain::provisioning::{ProvisioningError, ProvisioningId as DomainProvisioningId};
use leserpent_domain::retirement::{RetirementError, RetirementId as DomainRetirementId};
use leserpent_domain::{
    CapabilitySet as DomainCapabilitySet, CommandOrigin as DomainCommandOrigin,
    Confirmation as DomainConfirmation, DomainError, Principal as DomainPrincipal,
    Revision as DomainRevision, RuntimeId as DomainRuntimeId,
    RuntimeListFilter as DomainRuntimeListFilter,
};
use silvortex_identity::{
    CredentialHandle, IdentityError, ProvisioningId, RetirementId, RuntimeId,
};

fn accepts_runtime(_: RuntimeId) {}
fn accepts_provisioning(_: ProvisioningId) {}
fn accepts_retirement(_: RetirementId) {}
fn accepts_credential(_: CredentialHandle) {}
fn accepts_capabilities(_: HostCapabilitySet) {}
fn accepts_origin(_: HostCommandOrigin) {}
fn accepts_confirmation(_: HostConfirmation) {}
fn accepts_principal(_: HostPrincipal) {}
fn accepts_revision(_: HostRevision) {}
fn accepts_filter(_: HostRuntimeListFilter) {}

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

#[test]
fn legacy_domain_paths_preserve_leselang_host_type_identity() {
    let capabilities: DomainCapabilitySet = HostCapabilitySet::new(["runtime.read"]);
    let origin: DomainCommandOrigin = HostCommandOrigin::Leselang;
    let confirmation: DomainConfirmation = HostConfirmation::Confirmed;
    let principal: DomainPrincipal = HostPrincipal {
        id: "operator-a".into(),
    };
    let revision: DomainRevision = HostRevision(7);
    let filter: DomainRuntimeListFilter = HostRuntimeListFilter {
        environment: Some("production".into()),
        cluster: None,
        role: Some("edge".into()),
    };

    accepts_capabilities(capabilities);
    accepts_origin(origin);
    accepts_confirmation(confirmation);
    accepts_principal(principal);
    accepts_revision(revision);
    accepts_filter(filter);
}
