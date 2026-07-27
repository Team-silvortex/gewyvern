#![cfg(feature = "native-ssh")]

use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use leserpent_adapters::{
    BootstrapArtifact, BootstrapTrustError, BootstrapTrustRecord, BootstrapTrustStore,
    ConfiguredSecretStore, EffectAdapter, FileBootstrapTrustStore, NativeSshBootstrapTransport,
    SecretKey, SecretValue, SshBootstrapAdapter, SshBootstrapHostPolicy, SshBootstrapJob,
    SshBootstrapRetirementJob, SshBootstrapRetirementTransport,
    SshBootstrapRetirementTransportError, SshBootstrapTransport, SshBootstrapTransportError,
};
use leserpent_domain::bootstrap::{
    BOOTSTRAP_DOMAIN_SCHEMA_VERSION, BOOTSTRAP_SESSION_PROTOCOL_VERSION, BootstrapId,
    BootstrapIntent, BootstrapPhase, BootstrapTarget, BootstrapTransport,
    CAPABILITY_HOST_BOOTSTRAP, CredentialHandle, DaemonBootstrapReceipt, DaemonId,
    DaemonSessionProof, DeploymentBootstrap,
};
use leserpent_domain::{CapabilitySet, Principal};
use leserpent_protocol::bootstrap::{
    BOOTSTRAP_PROTOCOL_SCHEMA_VERSION, BootstrapRequest, BootstrapRequestEnvelope,
    BootstrapResponse, decode_bootstrap_response, encode_bootstrap_request,
};
use leserpent_protocol::bootstrap_retirement::BootstrapRetirementRequest;
use leserpent_runtime::EffectExecution;

#[test]
#[ignore = "requires an explicitly authorized Linux SSH target"]
fn real_ssh_bootstrap_binds_trust_before_session_authority() {
    let config = RealSshConfig::from_environment();
    let artifact = BootstrapArtifact::new(
        Arc::<[u8]>::from(fs::read(&config.artifact).expect("read Linux leserpentd artifact")),
        "/tmp/leserpent-bootstrap-real",
    )
    .expect("validate Linux leserpentd artifact");
    let bootstrap_key = SecretKey::new("real-host-bootstrap").unwrap();
    let session_key = SecretKey::new("real-host-session").unwrap();
    let secrets = || {
        Arc::new(
            ConfiguredSecretStore::new([
                (
                    bootstrap_key.clone(),
                    SecretValue::new(config.password.clone()).expect("validate SSH password"),
                ),
                (
                    session_key.clone(),
                    SecretValue::new(config.session_token.clone()).expect("validate session token"),
                ),
            ])
            .unwrap(),
        )
    };
    let target = BootstrapTarget {
        transport: BootstrapTransport::Ssh,
        host: config.host.clone(),
        port: config.ssh_port,
    };
    let bootstrap_id = BootstrapId::new(config.bootstrap_id.clone()).unwrap();
    let daemon_id = DaemonId::new(config.daemon_id.clone()).unwrap();
    let session_handle = CredentialHandle::new("vault:leserpentd:real-host-session").unwrap();
    let trust_handle = CredentialHandle::new("vault:leserpent-ca:real-host-trust").unwrap();
    let policy = SshBootstrapHostPolicy::new(
        target.clone(),
        config.username.clone(),
        config.host_key_sha256.clone(),
        daemon_id.clone(),
        config.endpoint.clone(),
        session_handle.clone(),
        trust_handle.clone(),
        "user",
    )
    .unwrap();
    let trust = Arc::new(FileBootstrapTrustStore::new(&config.trust_root).unwrap());
    let mut adapter = SshBootstrapAdapter::new(
        [policy.clone()],
        secrets(),
        trust.clone(),
        artifact.clone(),
        NativeSshBootstrapTransport::default(),
    )
    .unwrap();
    let principal = Principal {
        id: "real-host-operator".into(),
    };
    let capabilities = CapabilitySet::new([CAPABILITY_HOST_BOOTSTRAP]);
    let intent = BootstrapIntent {
        schema_version: BOOTSTRAP_DOMAIN_SCHEMA_VERSION,
        bootstrap_id: bootstrap_id.clone(),
        target: target.clone(),
        credential_handle: CredentialHandle::new("vault:ssh:real-host-bootstrap").unwrap(),
        requested_by: principal.id.clone(),
        confirmed: true,
    };
    let request = encode_bootstrap_request(&BootstrapRequestEnvelope {
        schema_version: BOOTSTRAP_PROTOCOL_SCHEMA_VERSION,
        request: BootstrapRequest {
            principal: principal.clone(),
            capabilities: capabilities.clone(),
            intent: intent.clone(),
        },
    })
    .unwrap();
    let response = match adapter.execute(&request) {
        EffectExecution::Complete(payload) => decode_bootstrap_response(&payload).unwrap(),
        other => panic!("real SSH bootstrap did not complete: {other:?}"),
    };
    let snapshot = match response.response {
        BootstrapResponse::State(snapshot) => snapshot,
        BootstrapResponse::Error(error) => panic!("real SSH bootstrap failed: {}", error.code),
    };
    assert_eq!(snapshot.phase, BootstrapPhase::Bootstrapped);
    assert!(!snapshot.mutation_authorized);
    assert_eq!(snapshot.daemon_id.as_ref(), Some(&daemon_id));
    assert_eq!(snapshot.endpoint.as_deref(), Some(config.endpoint.as_str()));
    assert_eq!(
        snapshot.session_credential_handle.as_ref(),
        Some(&session_handle)
    );
    assert_eq!(
        snapshot.trust_credential_handle.as_ref(),
        Some(&trust_handle)
    );
    let record = trust
        .load(&trust_handle)
        .expect("load controller trust record")
        .expect("controller trust record exists");
    assert_eq!(record.endpoint, config.endpoint);
    assert!(!record.ca_pem.is_empty());

    let mut rejecting_adapter = SshBootstrapAdapter::new(
        [policy.clone()],
        secrets(),
        Arc::new(RejectingTrustStore),
        artifact.clone(),
        NativeSshBootstrapTransport::default(),
    )
    .unwrap();
    let rejected = response_state(rejecting_adapter.execute(&request));
    assert_eq!(rejected.phase, BootstrapPhase::Failed);
    assert_eq!(
        rejected.fault_code.as_deref(),
        Some("trust_persistence_failed")
    );
    assert!(rejected.daemon_id.is_none());
    assert!(rejected.session_credential_handle.is_none());
    assert!(rejected.trust_credential_handle.is_none());

    let rollback_bootstrap_id = BootstrapId::new(config.rollback_bootstrap_id.clone()).unwrap();
    let rollback_daemon_id = DaemonId::new(config.rollback_daemon_id.clone()).unwrap();
    let bootstrap_password = SecretValue::new(config.password.clone()).unwrap();
    let rollback_session_token = SecretValue::new(config.rollback_session_token.clone()).unwrap();
    let mut rollback_transport = NativeSshBootstrapTransport::default();
    let rollback = rollback_transport.deploy(SshBootstrapJob {
        bootstrap_id: &rollback_bootstrap_id,
        target: &target,
        username: &config.username,
        host_key_sha256: &config.host_key_sha256,
        bootstrap_password: &bootstrap_password,
        session_token: &rollback_session_token,
        artifact: &artifact,
        daemon_id: &rollback_daemon_id,
        endpoint: &config.endpoint,
        install_profile: "user",
    });
    assert_eq!(rollback, Err(SshBootstrapTransportError::InstallerRejected));

    let mut timeout_adapter = SshBootstrapAdapter::new(
        [policy],
        secrets(),
        trust,
        artifact.clone(),
        NativeSshBootstrapTransport::with_timeout(Duration::from_millis(1)).unwrap(),
    )
    .unwrap();
    let timed_out = response_state(timeout_adapter.execute(&request));
    assert_eq!(timed_out.phase, BootstrapPhase::Failed);
    assert_eq!(timed_out.fault_code.as_deref(), Some("transport_failure"));
    assert!(timed_out.daemon_id.is_none());
    assert!(timed_out.session_credential_handle.is_none());
    assert!(timed_out.trust_credential_handle.is_none());

    let primary_session_token = SecretValue::new(config.session_token.clone()).unwrap();
    let mut retirement_transport = NativeSshBootstrapTransport::default();
    let deployment = retirement_transport
        .deploy(SshBootstrapJob {
            bootstrap_id: &bootstrap_id,
            target: &target,
            username: &config.username,
            host_key_sha256: &config.host_key_sha256,
            bootstrap_password: &bootstrap_password,
            session_token: &primary_session_token,
            artifact: &artifact,
            daemon_id: &daemon_id,
            endpoint: &config.endpoint,
            install_profile: "user",
        })
        .expect("replay the ready deployment before retirement");
    let retirement_request = BootstrapRetirementRequest::new(
        format!("{}-retirement", config.bootstrap_id),
        bootstrap_id.clone(),
        daemon_id.clone(),
        deployment.generation,
        "user",
    )
    .unwrap();

    let mut handoff = DeploymentBootstrap::plan(&principal, &capabilities, intent).unwrap();
    handoff.begin().unwrap();
    let bootstrapped = handoff
        .accept_deployed(DaemonBootstrapReceipt {
            bootstrap_id: bootstrap_id.clone(),
            daemon_id: daemon_id.clone(),
            endpoint: config.endpoint.clone(),
            generation: retirement_request.generation.clone(),
            install_profile: "user".into(),
            session_credential_handle: session_handle.clone(),
            trust_credential_handle: trust_handle.clone(),
        })
        .unwrap();
    assert!(!bootstrapped.mutation_authorized);
    let bound = handoff
        .bind_session(DaemonSessionProof {
            bootstrap_id,
            daemon_id,
            session_credential_handle: session_handle,
            trust_credential_handle: trust_handle,
            authority_owned: true,
            protocol_schema_version: BOOTSTRAP_SESSION_PROTOCOL_VERSION,
        })
        .unwrap();
    assert_eq!(bound.phase, BootstrapPhase::SessionBound);
    assert!(bound.mutation_authorized);

    let mut forged_retirement = retirement_request.clone();
    forged_retirement.generation = "f".repeat(64);
    assert_eq!(
        retirement_transport.retire(SshBootstrapRetirementJob {
            request: &forged_retirement,
            target: &target,
            username: &config.username,
            host_key_sha256: &config.host_key_sha256,
            ssh_password: &bootstrap_password,
            artifact: &artifact,
        }),
        Err(SshBootstrapRetirementTransportError::RetirementRejected)
    );

    let retired = retirement_transport
        .retire(SshBootstrapRetirementJob {
            request: &retirement_request,
            target: &target,
            username: &config.username,
            host_key_sha256: &config.host_key_sha256,
            ssh_password: &bootstrap_password,
            artifact: &artifact,
        })
        .expect("retire the identity-bound daemon generation");
    assert!(retired.service_retired);
    assert!(!retired.replayed);

    let replayed = retirement_transport
        .retire(SshBootstrapRetirementJob {
            request: &retirement_request,
            target: &target,
            username: &config.username,
            host_key_sha256: &config.host_key_sha256,
            ssh_password: &bootstrap_password,
            artifact: &artifact,
        })
        .expect("replay the exact daemon retirement");
    assert!(replayed.service_retired);
    assert!(replayed.replayed);

    let resurrection = retirement_transport.deploy(SshBootstrapJob {
        bootstrap_id: &retirement_request.bootstrap_id,
        target: &target,
        username: &config.username,
        host_key_sha256: &config.host_key_sha256,
        bootstrap_password: &bootstrap_password,
        session_token: &primary_session_token,
        artifact: &artifact,
        daemon_id: &retirement_request.daemon_id,
        endpoint: &config.endpoint,
        install_profile: "user",
    });
    if let Ok(resurrected) = &resurrection {
        let cleanup = BootstrapRetirementRequest::new(
            format!("{}-resurrection-cleanup", config.bootstrap_id),
            retirement_request.bootstrap_id.clone(),
            retirement_request.daemon_id.clone(),
            resurrected.generation.clone(),
            "user",
        )
        .unwrap();
        let _ = retirement_transport.retire(SshBootstrapRetirementJob {
            request: &cleanup,
            target: &target,
            username: &config.username,
            host_key_sha256: &config.host_key_sha256,
            ssh_password: &bootstrap_password,
            artifact: &artifact,
        });
        panic!("retired bootstrap generation was resurrected");
    }
    assert_eq!(
        resurrection,
        Err(SshBootstrapTransportError::InstallerRejected)
    );
}

struct RejectingTrustStore;

impl BootstrapTrustStore for RejectingTrustStore {
    fn persist(
        &self,
        _handle: &CredentialHandle,
        _record: &BootstrapTrustRecord,
    ) -> Result<(), BootstrapTrustError> {
        Err(BootstrapTrustError::Storage)
    }
}

fn response_state(
    execution: EffectExecution,
) -> leserpent_domain::bootstrap::DeploymentBootstrapSnapshot {
    let response = match execution {
        EffectExecution::Complete(payload) => decode_bootstrap_response(&payload).unwrap(),
        other => panic!("real SSH bootstrap did not complete: {other:?}"),
    };
    match response.response {
        BootstrapResponse::State(snapshot) => snapshot,
        BootstrapResponse::Error(error) => panic!("real SSH bootstrap failed: {}", error.code),
    }
}

struct RealSshConfig {
    host: String,
    ssh_port: u16,
    username: String,
    password: String,
    host_key_sha256: String,
    artifact: PathBuf,
    endpoint: String,
    session_token: String,
    bootstrap_id: String,
    daemon_id: String,
    trust_root: PathBuf,
    rollback_bootstrap_id: String,
    rollback_daemon_id: String,
    rollback_session_token: String,
}

impl RealSshConfig {
    fn from_environment() -> Self {
        Self {
            host: required("LESERPENT_REAL_SSH_HOST"),
            ssh_port: required("LESERPENT_REAL_SSH_PORT")
                .parse()
                .expect("LESERPENT_REAL_SSH_PORT is a u16"),
            username: required("LESERPENT_REAL_SSH_USER"),
            password: required("LESERPENT_REAL_SSH_PASSWORD"),
            host_key_sha256: required("LESERPENT_REAL_SSH_HOST_KEY_SHA256"),
            artifact: required("LESERPENT_REAL_SSH_ARTIFACT").into(),
            endpoint: required("LESERPENT_REAL_SSH_ENDPOINT"),
            session_token: required("LESERPENT_REAL_SESSION_TOKEN"),
            bootstrap_id: required("LESERPENT_REAL_BOOTSTRAP_ID"),
            daemon_id: required("LESERPENT_REAL_DAEMON_ID"),
            trust_root: required("LESERPENT_REAL_TRUST_ROOT").into(),
            rollback_bootstrap_id: required("LESERPENT_REAL_ROLLBACK_BOOTSTRAP_ID"),
            rollback_daemon_id: required("LESERPENT_REAL_ROLLBACK_DAEMON_ID"),
            rollback_session_token: required("LESERPENT_REAL_ROLLBACK_SESSION_TOKEN"),
        }
    }
}

fn required(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("{name} is required for the ignored real-host test"))
}
