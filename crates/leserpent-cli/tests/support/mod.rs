use std::time::Duration;

use leserpent_domain::bootstrap::{BootstrapTarget, BootstrapTransport, CredentialHandle};
use leserpent_domain::provisioning::{
    CAPABILITY_RUNTIME_PROVISION, GewyvernServiceReceipt, PROVISIONING_DOMAIN_SCHEMA_VERSION,
    ProvisioningId, RuntimeProvisioning, RuntimeProvisioningIntent,
};
use leserpent_domain::{CapabilitySet, Principal, RuntimeId};
use leserpent_runtime::ControlRuntime;

pub fn seed_registered_runtime(
    runtime: &mut ControlRuntime,
    provisioning_id: &str,
    runtime_id: &str,
    target_host: &str,
) {
    let provisioning_id = ProvisioningId::new(provisioning_id).unwrap();
    let runtime_id = RuntimeId::new(runtime_id).unwrap();
    let target = BootstrapTarget {
        transport: BootstrapTransport::Ssh,
        host: target_host.into(),
        port: 22,
    };
    let mut provisioning = RuntimeProvisioning::plan(
        &Principal {
            id: "integration-test".into(),
        },
        &CapabilitySet::new([CAPABILITY_RUNTIME_PROVISION]),
        RuntimeProvisioningIntent {
            schema_version: PROVISIONING_DOMAIN_SCHEMA_VERSION,
            provisioning_id: provisioning_id.clone(),
            runtime_id: runtime_id.clone(),
            target,
            install_credential_handle: CredentialHandle::new("vault:ssh:runtime-example").unwrap(),
            requested_by: "integration-test".into(),
            confirmed: true,
        },
    )
    .unwrap();
    runtime
        .enqueue_provisioning_effect(
            &format!("seed-{}", provisioning_id.as_str()),
            "gewyvern.runtime.provision",
            b"seed-provisioning",
            1,
            &provisioning.checkpoint(1).unwrap(),
        )
        .unwrap();
    let lease = runtime
        .claim_effect("seed-worker", Duration::from_secs(30))
        .unwrap()
        .unwrap();
    provisioning.begin().unwrap();
    provisioning
        .accept_service(GewyvernServiceReceipt {
            provisioning_id,
            runtime_id,
            endpoint: "https://runtime.example:9411/".into(),
            api_credential_handle: CredentialHandle::new("vault:gewyvern-api:runtime-example")
                .unwrap(),
            trust_credential_handle: CredentialHandle::new("vault:gewyvern-ca:runtime-example")
                .unwrap(),
        })
        .unwrap();
    runtime
        .complete_provisioning_effect_and_register(
            &lease,
            b"seed-ready",
            &provisioning.checkpoint(2).unwrap(),
        )
        .unwrap();
}
