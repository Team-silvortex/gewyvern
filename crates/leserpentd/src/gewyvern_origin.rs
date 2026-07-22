use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use leserpent_adapters::{
    BootstrapTrustStore, GewyvernArtifact, GewyvernProvisioningAdapter,
    MAX_GEWYVERN_ARTIFACT_BYTES, NativeSshGewyvernProvisioningTransport, SecretStore,
    SshGewyvernHostPolicy,
};
use leserpent_domain::RuntimeId;
use leserpent_domain::bootstrap::{BootstrapTarget, BootstrapTransport, CredentialHandle};
use serde::Deserialize;

use crate::bootstrap_origin::{
    load_executable_artifact, read_private_origin_config, require_absolute_clean_path,
};

pub const GEWYVERN_ORIGIN_CONFIG_SCHEMA_VERSION: u32 = 1;
const MAX_GEWYVERN_HOST_POLICIES: usize = 64;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GewyvernOriginConfig {
    schema_version: u32,
    artifact_path: PathBuf,
    staging_prefix: String,
    secret_service: String,
    hosts: Vec<GewyvernOriginHost>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GewyvernOriginHost {
    host: String,
    port: u16,
    runtime_id: RuntimeId,
    username: String,
    host_key_sha256: String,
    endpoint: String,
    api_credential_handle: CredentialHandle,
    trust_credential_handle: CredentialHandle,
    install_profile: String,
}

struct ValidatedGewyvernOrigin {
    artifact: GewyvernArtifact,
    policies: Vec<SshGewyvernHostPolicy>,
}

impl GewyvernOriginConfig {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, String> {
        let bytes = read_private_origin_config(
            path.as_ref(),
            "Gewyvern provisioning origin configuration",
        )?;
        let config = serde_json::from_slice::<Self>(&bytes)
            .map_err(|_| "Gewyvern provisioning origin configuration is invalid".to_string())?;
        config.validate_shape()?;
        Ok(config)
    }

    pub fn secret_service(&self) -> &str {
        &self.secret_service
    }

    pub fn into_native_adapter(
        self,
        secrets: Arc<dyn SecretStore>,
        trust: Arc<dyn BootstrapTrustStore>,
    ) -> Result<GewyvernProvisioningAdapter<NativeSshGewyvernProvisioningTransport>, String> {
        let validated = self.validate_and_load()?;
        let transport = NativeSshGewyvernProvisioningTransport::new(
            validated.policies,
            secrets.clone(),
            trust,
            validated.artifact,
        )?;
        Ok(GewyvernProvisioningAdapter::new(secrets, transport))
    }

    fn validate_shape(&self) -> Result<(), String> {
        if self.schema_version != GEWYVERN_ORIGIN_CONFIG_SCHEMA_VERSION {
            return Err(format!(
                "Gewyvern provisioning origin schema version must be {GEWYVERN_ORIGIN_CONFIG_SCHEMA_VERSION}"
            ));
        }
        if self.hosts.is_empty() || self.hosts.len() > MAX_GEWYVERN_HOST_POLICIES {
            return Err(format!(
                "Gewyvern provisioning origin must define between 1 and {MAX_GEWYVERN_HOST_POLICIES} hosts"
            ));
        }
        leserpent_adapters::PlatformSecretStore::new(&self.secret_service)
            .map_err(|_| "Gewyvern provisioning secret service is invalid".to_string())?;
        require_absolute_clean_path(&self.artifact_path, "Gewyvern artifact")
    }

    fn validate_and_load(self) -> Result<ValidatedGewyvernOrigin, String> {
        self.validate_shape()?;
        let bytes = load_executable_artifact(
            &self.artifact_path,
            MAX_GEWYVERN_ARTIFACT_BYTES,
            "Gewyvern artifact",
        )?;
        let artifact = GewyvernArtifact::new(bytes, self.staging_prefix)?;
        let mut api_handles = BTreeSet::new();
        let mut trust_handles = BTreeSet::new();
        let mut targets = BTreeSet::new();
        let mut policies = Vec::with_capacity(self.hosts.len());
        for host in self.hosts {
            let target = BootstrapTarget {
                transport: BootstrapTransport::Ssh,
                host: host.host,
                port: host.port,
            };
            if !targets.insert(format!(
                "{}:{}#{}",
                target.host,
                target.port,
                host.runtime_id.as_str()
            )) || !api_handles.insert(host.api_credential_handle.as_str().to_string())
                || !trust_handles.insert(host.trust_credential_handle.as_str().to_string())
            {
                return Err("Gewyvern target, API, and trust identities must be unique".into());
            }
            policies.push(SshGewyvernHostPolicy::new(
                target,
                host.runtime_id,
                host.username,
                host.host_key_sha256,
                host.endpoint,
                host.api_credential_handle,
                host.trust_credential_handle,
                host.install_profile,
            )?);
        }
        Ok(ValidatedGewyvernOrigin { artifact, policies })
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use std::time::{SystemTime, UNIX_EPOCH};

    use leserpent_adapters::{
        EffectAdapter, EmptySecretStore, FileBootstrapTrustStore, GEWYVERN_PROVISIONING_EFFECT_KIND,
    };

    use super::*;

    fn temp_path(label: &str) -> PathBuf {
        fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "leserpent-gewyvern-origin-{label}-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ))
    }

    fn config_json(artifact: &Path) -> String {
        serde_json::to_string_pretty(&serde_json::json!({
            "schema_version": 1,
            "artifact_path": artifact,
            "staging_prefix": "/tmp/gewyvern-provision",
            "secret_service": "org.gewyvern.leserpent.adapters",
            "hosts": [{
                "host": "host.example",
                "port": 22,
                "runtime_id": "runtime-a",
                "username": "deployer",
                "host_key_sha256": "SHA256:abcdefghijklmnopqrstuvwxyz0123456789ABCDE",
                "endpoint": "https://runtime.example:9443",
                "api_credential_handle": "vault:gewyvern:runtime-api",
                "trust_credential_handle": "vault:gewyvern-ca:runtime-ca",
                "install_profile": "system"
            }]
        }))
        .unwrap()
    }

    fn write_private(path: &Path, bytes: impl AsRef<[u8]>) {
        fs::write(path, bytes).unwrap();
        #[cfg(unix)]
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    }

    #[test]
    fn config_is_strict_secret_free_and_builds_the_native_adapter() {
        let artifact = temp_path("artifact");
        fs::write(&artifact, b"native-gewyvern").unwrap();
        #[cfg(unix)]
        fs::set_permissions(&artifact, fs::Permissions::from_mode(0o755)).unwrap();
        let config = temp_path("config");
        write_private(&config, config_json(&artifact));
        let loaded = GewyvernOriginConfig::load(&config).unwrap();
        assert_eq!(loaded.secret_service(), "org.gewyvern.leserpent.adapters");
        let trust = Arc::new(FileBootstrapTrustStore::new(temp_path("trust")).unwrap());
        let adapter = loaded
            .into_native_adapter(Arc::new(EmptySecretStore), trust)
            .unwrap();
        assert_eq!(adapter.kind(), GEWYVERN_PROVISIONING_EFFECT_KIND);

        let with_token = config_json(&artifact).replacen(
            "\"username\": \"deployer\"",
            "\"username\": \"deployer\", \"api_token\": \"forbidden\"",
            1,
        );
        write_private(&config, with_token);
        assert!(GewyvernOriginConfig::load(&config).is_err());
        fs::remove_file(config).unwrap();
        fs::remove_file(artifact).unwrap();
    }

    #[test]
    fn duplicate_target_or_authority_handle_is_rejected() {
        let artifact = temp_path("duplicate-artifact");
        fs::write(&artifact, b"native-gewyvern").unwrap();
        #[cfg(unix)]
        fs::set_permissions(&artifact, fs::Permissions::from_mode(0o755)).unwrap();
        let config = temp_path("duplicate-config");
        let mut json = serde_json::from_str::<serde_json::Value>(&config_json(&artifact)).unwrap();
        let hosts = json["hosts"].as_array_mut().unwrap();
        hosts.push(hosts[0].clone());
        write_private(&config, serde_json::to_vec(&json).unwrap());
        assert!(
            GewyvernOriginConfig::load(&config)
                .unwrap()
                .validate_and_load()
                .is_err()
        );
        fs::remove_file(config).unwrap();
        fs::remove_file(artifact).unwrap();
    }

    #[test]
    fn one_ssh_host_can_manage_multiple_runtime_identities() {
        let artifact = temp_path("multi-runtime-artifact");
        fs::write(&artifact, b"native-gewyvern").unwrap();
        #[cfg(unix)]
        fs::set_permissions(&artifact, fs::Permissions::from_mode(0o755)).unwrap();
        let config = temp_path("multi-runtime-config");
        let mut json = serde_json::from_str::<serde_json::Value>(&config_json(&artifact)).unwrap();
        let hosts = json["hosts"].as_array_mut().unwrap();
        let mut second = hosts[0].clone();
        second["runtime_id"] = "runtime-b".into();
        second["endpoint"] = "https://runtime-b.example:9443".into();
        second["api_credential_handle"] = "vault:gewyvern:runtime-b-api".into();
        second["trust_credential_handle"] = "vault:gewyvern-ca:runtime-b-ca".into();
        hosts.push(second);
        write_private(&config, serde_json::to_vec(&json).unwrap());

        let validated = GewyvernOriginConfig::load(&config)
            .unwrap()
            .validate_and_load()
            .unwrap();
        assert_eq!(validated.policies.len(), 2);

        fs::remove_file(config).unwrap();
        fs::remove_file(artifact).unwrap();
    }
}
