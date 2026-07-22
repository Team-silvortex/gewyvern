use std::path::PathBuf;
use std::sync::Arc;

use leserpent_adapters::{FileBootstrapTrustStore, SecretKey, SecretStore};
use leserpent_domain::bootstrap::{
    BOOTSTRAP_SESSION_PROTOCOL_VERSION, BootstrapPhase, DaemonSessionProof,
    DeploymentBootstrapCheckpoint,
};

use crate::BootstrapSessionVerifier;

pub struct NativeBootstrapSessionVerifier {
    secrets: Arc<dyn SecretStore>,
    trust: FileBootstrapTrustStore,
}

impl NativeBootstrapSessionVerifier {
    pub fn new(
        secrets: Arc<dyn SecretStore>,
        trust_root: impl Into<PathBuf>,
    ) -> Result<Self, String> {
        let trust = FileBootstrapTrustStore::new(trust_root)
            .map_err(|_| "bootstrap trust root is invalid".to_string())?;
        Ok(Self { secrets, trust })
    }
}

impl BootstrapSessionVerifier for NativeBootstrapSessionVerifier {
    fn prove_session(
        &self,
        checkpoint: &DeploymentBootstrapCheckpoint,
    ) -> Result<DaemonSessionProof, String> {
        checkpoint
            .validate()
            .map_err(|_| "bootstrap checkpoint is invalid".to_string())?;
        if !matches!(
            checkpoint.state.phase,
            BootstrapPhase::Bootstrapped | BootstrapPhase::SessionBound
        ) {
            return Err("bootstrap checkpoint has no bindable daemon session".into());
        }
        let daemon_id = checkpoint
            .state
            .daemon_id
            .clone()
            .ok_or("bootstrap checkpoint has no daemon identity")?;
        let endpoint = checkpoint
            .state
            .endpoint
            .as_deref()
            .ok_or("bootstrap checkpoint has no daemon endpoint")?;
        let session_handle = checkpoint
            .state
            .session_credential_handle
            .clone()
            .ok_or("bootstrap checkpoint has no session handle")?;
        let trust_handle = checkpoint
            .state
            .trust_credential_handle
            .clone()
            .ok_or("bootstrap checkpoint has no trust handle")?;
        let (provider, session_key) = session_handle.parts();
        if provider != "leserpentd" {
            return Err("bootstrap session handle provider is invalid".into());
        }
        let session_key = SecretKey::new(session_key)
            .map_err(|_| "bootstrap session handle key is invalid".to_string())?;
        let session_token = self
            .secrets
            .load(&session_key)
            .map_err(|_| "bootstrap session store is unavailable".to_string())?
            .ok_or("bootstrap session credential was not found")?;
        let trust = self
            .trust
            .load(&trust_handle)
            .map_err(|_| "bootstrap trust record is invalid".to_string())?
            .ok_or("bootstrap trust record was not found")?;
        if trust.endpoint != endpoint {
            return Err("bootstrap trust endpoint does not match the checkpoint".into());
        }
        crate::bootstrap_health::prove_remote_bootstrap_health(
            endpoint,
            &trust.ca_pem,
            session_token.expose_secret(),
        )?;
        Ok(DaemonSessionProof {
            bootstrap_id: checkpoint.state.bootstrap_id.clone(),
            daemon_id,
            session_credential_handle: session_handle,
            trust_credential_handle: trust_handle,
            authority_owned: true,
            protocol_schema_version: BOOTSTRAP_SESSION_PROTOCOL_VERSION,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use leserpent_adapters::{
        BootstrapTrustRecord, BootstrapTrustStore, ConfiguredSecretStore, SecretValue,
    };
    use leserpent_domain::bootstrap::{
        BootstrapId, BootstrapTarget, BootstrapTransport, CredentialHandle, DaemonId,
        DeploymentBootstrapSnapshot,
    };
    use rcgen::generate_simple_self_signed;
    use ring::digest::{SHA256, digest};

    use super::*;

    #[test]
    fn verifier_rejects_trust_endpoint_confusion_before_network_access() {
        let root = std::env::temp_dir().join(format!(
            "leserpent-bootstrap-verifier-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        let trust_root = root.join("trust");
        let trust_handle = CredentialHandle::new("vault:leserpent-ca:host-example").unwrap();
        let certificate = generate_simple_self_signed(vec!["host.example".into()])
            .unwrap()
            .cert
            .pem();
        FileBootstrapTrustStore::new(&trust_root)
            .unwrap()
            .persist(
                &trust_handle,
                &BootstrapTrustRecord {
                    endpoint: "https://other.example:9443".into(),
                    ca_sha256: hex(digest(&SHA256, certificate.as_bytes()).as_ref()),
                    ca_pem: certificate,
                },
            )
            .unwrap();
        let secrets = Arc::new(
            ConfiguredSecretStore::new([(
                SecretKey::new("host-example").unwrap(),
                SecretValue::new("0123456789abcdef0123456789abcdef").unwrap(),
            )])
            .unwrap(),
        );
        let verifier = NativeBootstrapSessionVerifier::new(secrets, &trust_root).unwrap();
        let checkpoint = DeploymentBootstrapCheckpoint::new(
            1,
            DeploymentBootstrapSnapshot {
                bootstrap_id: BootstrapId::new("bootstrap-1").unwrap(),
                phase: BootstrapPhase::Bootstrapped,
                target: BootstrapTarget {
                    transport: BootstrapTransport::Ssh,
                    host: "host.example".into(),
                    port: 22,
                },
                bootstrap_credential_present: true,
                daemon_id: Some(DaemonId::new("daemon-host-example").unwrap()),
                endpoint: Some("https://host.example:9443/".into()),
                session_credential_handle: Some(
                    CredentialHandle::new("vault:leserpentd:host-example").unwrap(),
                ),
                trust_credential_handle: Some(trust_handle),
                fault_code: None,
                mutation_authorized: false,
            },
            Some(CredentialHandle::new("vault:ssh:host-example").unwrap()),
        )
        .unwrap();

        assert_eq!(
            verifier.prove_session(&checkpoint),
            Err("bootstrap trust endpoint does not match the checkpoint".into())
        );
        fs::remove_dir_all(root).unwrap();
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
}
