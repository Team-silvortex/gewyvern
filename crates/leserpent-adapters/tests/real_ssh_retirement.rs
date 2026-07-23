#![cfg(feature = "native-ssh")]

use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use leserpent_adapters::{
    ConfiguredSecretStore, EffectAdapter, FileBootstrapTrustStore, GewyvernArtifact,
    GewyvernProvisioningAdapter, GewyvernRetirementAdapter, NativeSshGewyvernProvisioningTransport,
    NativeSshGewyvernRetirementTransport, SecretKey, SecretValue, SshGewyvernHostPolicy,
};
use leserpent_domain::bootstrap::{BootstrapTarget, BootstrapTransport, CredentialHandle};
use leserpent_domain::provisioning::{
    CAPABILITY_RUNTIME_PROVISION, PROVISIONING_DOMAIN_SCHEMA_VERSION, ProvisioningId,
    ProvisioningPhase, RuntimeProvisioningIntent,
};
use leserpent_domain::retirement::{
    CAPABILITY_RUNTIME_RETIRE, RETIREMENT_DOMAIN_SCHEMA_VERSION, RetirementId, RetirementPhase,
    RuntimeRetirementIntent,
};
use leserpent_domain::{CapabilitySet, Principal, RuntimeId};
use leserpent_protocol::provisioning::{
    PROVISIONING_PROTOCOL_SCHEMA_VERSION, ProvisioningRequest, ProvisioningRequestEnvelope,
    ProvisioningResponse, decode_provisioning_response, encode_provisioning_request,
};
use leserpent_protocol::retirement::{
    RETIREMENT_PROTOCOL_SCHEMA_VERSION, RetirementRequest, RetirementRequestEnvelope,
    RetirementResponse, decode_retirement_response, encode_retirement_request,
};
use leserpent_runtime::EffectExecution;

#[test]
#[ignore = "requires an explicitly authorized physical Linux SSH target"]
fn real_ssh_provision_then_retire_is_identity_bound_and_replayable() {
    let config = RealRetirementConfig::from_environment();
    let artifact = GewyvernArtifact::new(
        Arc::<[u8]>::from(fs::read(&config.artifact).expect("read Linux Gewyvern artifact")),
        "/tmp/gewyvern-retirement-proof",
    )
    .expect("validate Linux Gewyvern artifact");
    let target = BootstrapTarget {
        transport: BootstrapTransport::Ssh,
        host: config.host.clone(),
        port: config.ssh_port,
    };
    let runtime_id = RuntimeId::new(config.runtime_id.clone()).unwrap();
    let provisioning_id = ProvisioningId::new(config.provisioning_id.clone()).unwrap();
    let retirement_id = RetirementId::new(config.retirement_id.clone()).unwrap();
    let ssh_handle = CredentialHandle::new("vault:ssh:real-retirement-host").unwrap();
    let api_handle = CredentialHandle::new("vault:gewyvern:real-retirement-api").unwrap();
    let trust_handle = CredentialHandle::new("vault:gewyvern-ca:real-retirement-ca").unwrap();
    let policy = SshGewyvernHostPolicy::new(
        target.clone(),
        runtime_id.clone(),
        config.username,
        config.host_key_sha256,
        config.endpoint.clone(),
        api_handle,
        trust_handle.clone(),
        "user",
    )
    .unwrap();
    let secrets = Arc::new(
        ConfiguredSecretStore::new([
            (
                SecretKey::new("real-retirement-host").unwrap(),
                SecretValue::new(config.password).unwrap(),
            ),
            (
                SecretKey::new("real-retirement-api").unwrap(),
                SecretValue::new(config.api_token).unwrap(),
            ),
        ])
        .unwrap(),
    );
    let trust = Arc::new(FileBootstrapTrustStore::new(&config.trust_root).unwrap());
    let principal = Principal {
        id: "physical-linux-proof".into(),
    };

    let provisioning_request = ProvisioningRequestEnvelope {
        schema_version: PROVISIONING_PROTOCOL_SCHEMA_VERSION,
        request: ProvisioningRequest {
            principal: principal.clone(),
            capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_PROVISION]),
            intent: RuntimeProvisioningIntent {
                schema_version: PROVISIONING_DOMAIN_SCHEMA_VERSION,
                provisioning_id: provisioning_id.clone(),
                runtime_id: runtime_id.clone(),
                target: target.clone(),
                install_credential_handle: ssh_handle.clone(),
                requested_by: principal.id.clone(),
                confirmed: true,
            },
        },
    };
    let provisioning_transport = NativeSshGewyvernProvisioningTransport::new(
        [policy.clone()],
        secrets.clone(),
        trust.clone(),
        artifact.clone(),
    )
    .unwrap();
    let mut provisioning =
        GewyvernProvisioningAdapter::new(secrets.clone(), provisioning_transport);
    let provisioned = completed(
        provisioning.execute(&encode_provisioning_request(&provisioning_request).unwrap()),
    );
    let provisioned = decode_provisioning_response(&provisioned).unwrap();
    let ProvisioningResponse::State(provisioned) = provisioned.response else {
        panic!("real SSH provisioning returned a protocol error");
    };
    assert_eq!(provisioned.phase, ProvisioningPhase::ServiceReady);
    assert_eq!(provisioned.provisioning_id, provisioning_id);
    assert_eq!(provisioned.runtime_id, runtime_id);
    assert_eq!(
        provisioned.endpoint.as_deref(),
        Some(config.endpoint.as_str())
    );
    assert!(!provisioned.install_credential_present);
    assert!(!provisioned.runtime_registered);
    assert!(trust.load(&trust_handle).unwrap().is_some());

    let retirement_request = RetirementRequestEnvelope {
        schema_version: RETIREMENT_PROTOCOL_SCHEMA_VERSION,
        request: RetirementRequest {
            principal: principal.clone(),
            capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_RETIRE]),
            intent: RuntimeRetirementIntent {
                schema_version: RETIREMENT_DOMAIN_SCHEMA_VERSION,
                retirement_id,
                provisioning_id,
                runtime_id,
                target,
                retirement_credential_handle: ssh_handle,
                requested_by: principal.id,
                confirmed: true,
            },
        },
    };
    let retirement_transport =
        NativeSshGewyvernRetirementTransport::new([policy], artifact).unwrap();
    let mut retirement = GewyvernRetirementAdapter::new(secrets, retirement_transport);
    let mut forged_request = retirement_request.clone();
    forged_request.request.intent.retirement_id =
        RetirementId::new(format!("{}-forged", config.retirement_id)).unwrap();
    forged_request.request.intent.provisioning_id =
        ProvisioningId::new(format!("{}-forged", config.provisioning_id)).unwrap();
    let forged = decode_retirement_response(&completed(
        retirement.execute(&encode_retirement_request(&forged_request).unwrap()),
    ))
    .unwrap();
    let RetirementResponse::State(forged) = forged.response else {
        panic!("forged physical-host retirement returned a protocol error");
    };
    assert_eq!(forged.phase, RetirementPhase::Failed);
    assert_eq!(
        forged.fault_code.as_deref(),
        Some("service_retirement_rejected")
    );
    assert!(forged.runtime_registered);
    assert!(!forged.service_retired);

    let encoded = encode_retirement_request(&retirement_request).unwrap();
    for replay in 0..=1 {
        let retired = decode_retirement_response(&completed(retirement.execute(&encoded))).unwrap();
        let RetirementResponse::State(retired) = retired.response else {
            panic!("real SSH retirement returned a protocol error");
        };
        assert_eq!(retired.phase, RetirementPhase::ServiceRetired);
        assert!(retired.service_retired);
        assert!(retired.runtime_registered);
        assert!(!retired.retirement_credential_present);
        if replay == 1 {
            assert!(retired.fault_code.is_none());
        }
    }
}

fn completed(execution: EffectExecution) -> Vec<u8> {
    match execution {
        EffectExecution::Complete(payload) => payload,
        other => panic!("real SSH effect did not complete: {other:?}"),
    }
}

struct RealRetirementConfig {
    host: String,
    ssh_port: u16,
    username: String,
    password: String,
    host_key_sha256: String,
    artifact: PathBuf,
    endpoint: String,
    api_token: String,
    provisioning_id: String,
    runtime_id: String,
    retirement_id: String,
    trust_root: PathBuf,
}

impl RealRetirementConfig {
    fn from_environment() -> Self {
        Self {
            host: required("LESERPENT_REAL_SSH_HOST"),
            ssh_port: required("LESERPENT_REAL_SSH_PORT")
                .parse()
                .expect("LESERPENT_REAL_SSH_PORT is a u16"),
            username: required("LESERPENT_REAL_SSH_USER"),
            password: required("LESERPENT_REAL_SSH_PASSWORD"),
            host_key_sha256: required("LESERPENT_REAL_SSH_HOST_KEY_SHA256"),
            artifact: required("LESERPENT_REAL_RETIREMENT_ARTIFACT").into(),
            endpoint: required("LESERPENT_REAL_RETIREMENT_ENDPOINT"),
            api_token: required("LESERPENT_REAL_RETIREMENT_API_TOKEN"),
            provisioning_id: required("LESERPENT_REAL_RETIREMENT_PROVISIONING_ID"),
            runtime_id: required("LESERPENT_REAL_RETIREMENT_RUNTIME_ID"),
            retirement_id: required("LESERPENT_REAL_RETIREMENT_ID"),
            trust_root: required("LESERPENT_REAL_RETIREMENT_TRUST_ROOT").into(),
        }
    }
}

fn required(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("{name} is required for the ignored real-host test"))
}
