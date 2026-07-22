use std::collections::BTreeSet;
use std::io::Read;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use leserpent_adapters::{
    BootstrapArtifact, BootstrapTrustStore, MAX_BOOTSTRAP_ARTIFACT_BYTES, SecretStore,
    SshBootstrapHostPolicy,
};
#[cfg(feature = "native-ssh")]
use leserpent_adapters::{NativeSshBootstrapTransport, SshBootstrapAdapter};
use leserpent_domain::bootstrap::{
    BootstrapTarget, BootstrapTransport, CredentialHandle, DaemonId,
};
use leserpent_protocol::transport_safety::open_bounded_regular_file;
use serde::Deserialize;

pub const BOOTSTRAP_ORIGIN_CONFIG_SCHEMA_VERSION: u32 = 1;
const MAX_BOOTSTRAP_ORIGIN_CONFIG_BYTES: u64 = 64 * 1024;
const MAX_BOOTSTRAP_HOST_POLICIES: usize = 64;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BootstrapOriginConfig {
    schema_version: u32,
    artifact_path: PathBuf,
    staging_prefix: String,
    secret_service: String,
    hosts: Vec<BootstrapOriginHost>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BootstrapOriginHost {
    host: String,
    port: u16,
    username: String,
    host_key_sha256: String,
    daemon_id: DaemonId,
    endpoint: String,
    session_credential_handle: CredentialHandle,
    trust_credential_handle: CredentialHandle,
    install_profile: String,
}

struct ValidatedBootstrapOrigin {
    artifact: BootstrapArtifact,
    policies: Vec<SshBootstrapHostPolicy>,
}

impl BootstrapOriginConfig {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref();
        require_absolute_clean_path(path, "bootstrap origin configuration")?;
        let mut file = open_bounded_regular_file(path, MAX_BOOTSTRAP_ORIGIN_CONFIG_BYTES)
            .map_err(|error| format!("cannot open bootstrap origin configuration: {error}"))?;
        #[cfg(unix)]
        if file
            .metadata()
            .map_err(|error| format!("cannot inspect bootstrap origin configuration: {error}"))?
            .permissions()
            .mode()
            & 0o777
            != 0o600
        {
            return Err("bootstrap origin configuration must have mode 0600".into());
        }
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)
            .map_err(|error| format!("cannot read bootstrap origin configuration: {error}"))?;
        let config = serde_json::from_slice::<Self>(&bytes)
            .map_err(|_| "bootstrap origin configuration is invalid".to_string())?;
        config.validate_shape()?;
        Ok(config)
    }

    pub fn secret_service(&self) -> &str {
        &self.secret_service
    }

    #[cfg(feature = "native-ssh")]
    pub fn into_native_adapter(
        self,
        secrets: Arc<dyn SecretStore>,
        trust: Arc<dyn BootstrapTrustStore>,
    ) -> Result<SshBootstrapAdapter<NativeSshBootstrapTransport>, String> {
        let validated = self.validate_and_load()?;
        SshBootstrapAdapter::new(
            validated.policies,
            secrets,
            trust,
            validated.artifact,
            NativeSshBootstrapTransport::default(),
        )
    }

    fn validate_shape(&self) -> Result<(), String> {
        if self.schema_version != BOOTSTRAP_ORIGIN_CONFIG_SCHEMA_VERSION {
            return Err(format!(
                "bootstrap origin schema version must be {BOOTSTRAP_ORIGIN_CONFIG_SCHEMA_VERSION}"
            ));
        }
        if self.hosts.is_empty() || self.hosts.len() > MAX_BOOTSTRAP_HOST_POLICIES {
            return Err(format!(
                "bootstrap origin must define between 1 and {MAX_BOOTSTRAP_HOST_POLICIES} hosts"
            ));
        }
        leserpent_adapters::PlatformSecretStore::new(&self.secret_service)
            .map_err(|_| "bootstrap origin secret service is invalid".to_string())?;
        require_absolute_clean_path(&self.artifact_path, "bootstrap artifact")?;
        Ok(())
    }

    fn validate_and_load(self) -> Result<ValidatedBootstrapOrigin, String> {
        self.validate_shape()?;
        let artifact = load_artifact(&self.artifact_path, &self.staging_prefix)?;
        let mut daemon_ids = BTreeSet::new();
        let mut session_handles = BTreeSet::new();
        let mut trust_handles = BTreeSet::new();
        let mut policies = Vec::with_capacity(self.hosts.len());
        for host in self.hosts {
            if !daemon_ids.insert(host.daemon_id.as_str().to_string())
                || !session_handles.insert(host.session_credential_handle.as_str().to_string())
                || !trust_handles.insert(host.trust_credential_handle.as_str().to_string())
            {
                return Err(
                    "bootstrap origin daemon, session, and trust identities must be unique".into(),
                );
            }
            policies.push(SshBootstrapHostPolicy::new(
                BootstrapTarget {
                    transport: BootstrapTransport::Ssh,
                    host: host.host,
                    port: host.port,
                },
                host.username,
                host.host_key_sha256,
                host.daemon_id,
                host.endpoint,
                host.session_credential_handle,
                host.trust_credential_handle,
                host.install_profile,
            )?);
        }
        Ok(ValidatedBootstrapOrigin { artifact, policies })
    }
}

fn load_artifact(path: &Path, staging_prefix: &str) -> Result<BootstrapArtifact, String> {
    let mut file = open_bounded_regular_file(path, MAX_BOOTSTRAP_ARTIFACT_BYTES as u64)
        .map_err(|error| format!("cannot open bootstrap artifact: {error}"))?;
    #[cfg(unix)]
    {
        let mode = file
            .metadata()
            .map_err(|error| format!("cannot inspect bootstrap artifact: {error}"))?
            .permissions()
            .mode();
        if mode & 0o111 == 0 || mode & 0o022 != 0 {
            return Err(
                "bootstrap artifact must be executable and not writable by group or other".into(),
            );
        }
    }
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|error| format!("cannot read bootstrap artifact: {error}"))?;
    BootstrapArtifact::new(Arc::<[u8]>::from(bytes), staging_prefix)
}

fn require_absolute_clean_path(path: &Path, label: &str) -> Result<(), String> {
    let valid = path.is_absolute()
        && path != Path::new("/")
        && path
            .components()
            .all(|component| !matches!(component, Component::CurDir | Component::ParentDir));
    if !valid {
        return Err(format!("{label} path must be absolute and normalized"));
    }
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        let metadata = std::fs::symlink_metadata(&current)
            .map_err(|error| format!("cannot inspect {label} path: {error}"))?;
        if metadata.file_type().is_symlink() {
            return Err(format!("{label} path must not contain symbolic links"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::time::{SystemTime, UNIX_EPOCH};

    use leserpent_adapters::{
        EffectAdapter, EmptySecretStore, FileBootstrapTrustStore, HOST_BOOTSTRAP_EFFECT_KIND,
    };

    use super::*;

    fn temp_path(label: &str) -> PathBuf {
        fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "leserpent-bootstrap-origin-{label}-{}-{}",
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
            "staging_prefix": "/tmp/leserpent-bootstrap",
            "secret_service": "org.gewyvern.leserpent.adapters",
            "hosts": [{
                "host": "host.example",
                "port": 22,
                "username": "deployer",
                "host_key_sha256": "SHA256:abcdefghijklmnopqrstuvwxyz0123456789ABCDE",
                "daemon_id": "daemon-host-example",
                "endpoint": "https://host.example:7443",
                "session_credential_handle": "vault:leserpentd:host-example-session",
                "trust_credential_handle": "vault:leserpent-ca:host-example-trust",
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
    fn config_is_strict_bounded_and_secret_free() {
        let artifact = temp_path("artifact");
        fs::write(&artifact, b"native-artifact").unwrap();
        #[cfg(unix)]
        fs::set_permissions(&artifact, fs::Permissions::from_mode(0o755)).unwrap();
        let config = temp_path("config");
        write_private(&config, config_json(&artifact));
        let loaded = BootstrapOriginConfig::load(&config).unwrap();
        assert_eq!(loaded.secret_service(), "org.gewyvern.leserpent.adapters");
        let trust = Arc::new(FileBootstrapTrustStore::new(temp_path("trust")).unwrap());
        let adapter = loaded
            .into_native_adapter(Arc::new(EmptySecretStore), trust)
            .unwrap();
        assert_eq!(adapter.kind(), HOST_BOOTSTRAP_EFFECT_KIND);

        let with_password = config_json(&artifact).replacen(
            "\"username\": \"deployer\"",
            "\"username\": \"deployer\", \"password\": \"forbidden\"",
            1,
        );
        write_private(&config, with_password);
        assert!(BootstrapOriginConfig::load(&config).is_err());
        fs::remove_file(config).unwrap();
        fs::remove_file(artifact).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn config_and_artifact_fail_closed_on_permissions_and_symlinks() {
        let artifact = temp_path("unsafe-artifact");
        fs::write(&artifact, b"native-artifact").unwrap();
        fs::set_permissions(&artifact, fs::Permissions::from_mode(0o777)).unwrap();
        let config = temp_path("unsafe-config");
        write_private(&config, config_json(&artifact));
        assert!(
            BootstrapOriginConfig::load(&config)
                .unwrap()
                .validate_and_load()
                .is_err()
        );

        fs::set_permissions(&artifact, fs::Permissions::from_mode(0o755)).unwrap();
        fs::set_permissions(&config, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(BootstrapOriginConfig::load(&config).is_err());
        fs::set_permissions(&config, fs::Permissions::from_mode(0o600)).unwrap();
        let link = temp_path("config-link");
        symlink(&config, &link).unwrap();
        assert!(BootstrapOriginConfig::load(&link).is_err());

        fs::remove_file(link).unwrap();
        fs::remove_file(config).unwrap();
        fs::remove_file(artifact).unwrap();
    }

    #[test]
    fn duplicate_authority_identities_are_rejected() {
        let artifact = temp_path("duplicate-artifact");
        fs::write(&artifact, b"native-artifact").unwrap();
        #[cfg(unix)]
        fs::set_permissions(&artifact, fs::Permissions::from_mode(0o755)).unwrap();
        let config = temp_path("duplicate-config");
        let mut json = serde_json::from_str::<serde_json::Value>(&config_json(&artifact)).unwrap();
        let hosts = json["hosts"].as_array_mut().unwrap();
        let duplicate = hosts[0].clone();
        hosts.push(duplicate);
        write_private(&config, serde_json::to_vec(&json).unwrap());
        assert!(
            BootstrapOriginConfig::load(&config)
                .unwrap()
                .validate_and_load()
                .is_err()
        );
        fs::remove_file(config).unwrap();
        fs::remove_file(artifact).unwrap();
    }
}
