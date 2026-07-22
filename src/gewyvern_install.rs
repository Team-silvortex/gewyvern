use std::env;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use leserpent_protocol::gewyvern_installer::{
    GEWYVERN_INSTALLER_SCHEMA_VERSION, GewyvernInstallerRequest, GewyvernInstallerResponse,
    GewyvernInstallerServiceState, MAX_GEWYVERN_INSTALLER_BYTES, decode_gewyvern_installer_request,
    encode_gewyvern_installer_response,
};
use rcgen::{CertifiedKey, generate_simple_self_signed};
use ring::digest::{SHA256, digest};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

const MAX_INSTALL_ARTIFACT_BYTES: u64 = 256 * 1024 * 1024;
const INSTALL_MANIFEST_SCHEMA_VERSION: u32 = 1;
const EXECUTABLE_NAME: &str = "gewyvern";
const TOKEN_NAME: &str = "api.token";
const TLS_CERTIFICATE_NAME: &str = "server.crt";
const TLS_PRIVATE_KEY_NAME: &str = "server.key";
const MANIFEST_NAME: &str = "install.json";
const SERVICE_PLAN_NAME: &str = "service-plan.json";
const CURRENT_NAME: &str = "current";

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GewyvernInstallError {
    InvalidRequest,
    InvalidLayout,
    InvalidArtifact,
    ArtifactDigestMismatch,
    GenerationConflict,
    TlsIdentity,
    Storage,
}

impl fmt::Display for GewyvernInstallError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRequest => formatter.write_str("invalid Gewyvern installer request"),
            Self::InvalidLayout => formatter.write_str("invalid Gewyvern installation layout"),
            Self::InvalidArtifact => formatter.write_str("invalid Gewyvern installation artifact"),
            Self::ArtifactDigestMismatch => {
                formatter.write_str("Gewyvern artifact digest mismatch")
            }
            Self::GenerationConflict => {
                formatter.write_str("Gewyvern installation generation conflict")
            }
            Self::TlsIdentity => formatter.write_str("cannot generate Gewyvern TLS identity"),
            Self::Storage => formatter.write_str("Gewyvern installation storage failed"),
        }
    }
}

impl std::error::Error for GewyvernInstallError {}

#[derive(Clone, Debug)]
pub struct GewyvernInstallLayout {
    root: PathBuf,
}

impl GewyvernInstallLayout {
    #[cfg(test)]
    fn test(root: PathBuf) -> Self {
        Self { root }
    }

    fn runtime_root(&self, request: &GewyvernInstallerRequest) -> PathBuf {
        self.root.join("runtimes").join(request.runtime_id.as_str())
    }
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct InstallManifest {
    schema_version: u32,
    provisioning_id: String,
    runtime_id: String,
    endpoint: String,
    install_profile: String,
    artifact_sha256: String,
    api_credential_handle: String,
    trust_credential_handle: String,
    api_token_sha256: String,
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ServicePlan {
    schema_version: u32,
    runtime_id: String,
    endpoint: String,
    executable: String,
    api_token_file: String,
    tls_certificate_file: String,
    tls_private_key_file: String,
}

pub fn run_gewyvern_install_stdio() -> Result<(), GewyvernInstallError> {
    let request = read_stdio_request(std::io::stdin().lock())?;
    let layout = platform_layout(&request.install_profile)?;
    let source = env::current_exe().map_err(|_| GewyvernInstallError::InvalidArtifact)?;
    let response = install_gewyvern_artifact(&source, &request, &layout)?;
    let encoded = encode_gewyvern_installer_response(&response)
        .map_err(|_| GewyvernInstallError::InvalidRequest)?;
    let mut stdout = std::io::stdout().lock();
    stdout
        .write_all(&encoded)
        .and_then(|()| stdout.flush())
        .map_err(|_| GewyvernInstallError::Storage)
}

pub fn install_gewyvern_artifact(
    source: &Path,
    request: &GewyvernInstallerRequest,
    layout: &GewyvernInstallLayout,
) -> Result<GewyvernInstallerResponse, GewyvernInstallError> {
    request
        .validate()
        .map_err(|_| GewyvernInstallError::InvalidRequest)?;
    validate_root(&layout.root)?;
    let artifact = read_artifact(source)?;
    let artifact_sha256 = hex(digest(&SHA256, &artifact).as_ref());
    if artifact_sha256 != request.artifact_sha256 {
        return Err(GewyvernInstallError::ArtifactDigestMismatch);
    }

    let runtime_root = layout.runtime_root(request);
    let generations = runtime_root.join("generations");
    create_private_dir(&generations)?;
    let destination = generations.join(&artifact_sha256);
    let manifest = manifest(request);
    let replayed = match fs::symlink_metadata(&destination) {
        Ok(metadata) => {
            if !metadata.is_dir() || metadata.file_type().is_symlink() {
                return Err(GewyvernInstallError::InvalidLayout);
            }
            verify_generation(&destination, &artifact, request, &manifest)?;
            true
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            stage_generation(&generations, &destination, &artifact, request, &manifest)?;
            false
        }
        Err(_) => return Err(GewyvernInstallError::Storage),
    };
    commit_current(&runtime_root, &artifact_sha256)?;
    let tls_ca_pem = read_private_text(&destination.join(TLS_CERTIFICATE_NAME), 32 * 1024)?;
    let tls_ca_sha256 = hex(digest(&SHA256, tls_ca_pem.as_bytes()).as_ref());

    Ok(GewyvernInstallerResponse {
        schema_version: GEWYVERN_INSTALLER_SCHEMA_VERSION,
        provisioning_id: request.provisioning_id.clone(),
        runtime_id: request.runtime_id.clone(),
        endpoint: request.endpoint.clone(),
        service_state: GewyvernInstallerServiceState::Installed,
        generation: artifact_sha256,
        replayed,
        api_credential_handle: request.api_credential_handle.clone(),
        trust_credential_handle: request.trust_credential_handle.clone(),
        tls_ca_pem,
        tls_ca_sha256,
    })
}

fn read_stdio_request(
    mut reader: impl Read,
) -> Result<GewyvernInstallerRequest, GewyvernInstallError> {
    let mut bytes = Zeroizing::new(Vec::new());
    reader
        .by_ref()
        .take((MAX_GEWYVERN_INSTALLER_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| GewyvernInstallError::InvalidRequest)?;
    decode_gewyvern_installer_request(&bytes).map_err(|_| GewyvernInstallError::InvalidRequest)
}

fn platform_layout(profile: &str) -> Result<GewyvernInstallLayout, GewyvernInstallError> {
    let root = match profile {
        "system" => PathBuf::from("/var/lib/gewyvern"),
        "user" => {
            let home = env::var_os("HOME").ok_or(GewyvernInstallError::InvalidLayout)?;
            PathBuf::from(home).join(".local/share/gewyvern")
        }
        "test" => PathBuf::from(
            env::var_os("GEWYVERN_INSTALL_ROOT").ok_or(GewyvernInstallError::InvalidLayout)?,
        ),
        _ => return Err(GewyvernInstallError::InvalidLayout),
    };
    validate_root(&root)?;
    Ok(GewyvernInstallLayout { root })
}

fn validate_root(root: &Path) -> Result<(), GewyvernInstallError> {
    if !root.is_absolute()
        || root == Path::new("/")
        || root
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        return Err(GewyvernInstallError::InvalidLayout);
    }
    reject_existing_symlink_components(root)
}

fn manifest(request: &GewyvernInstallerRequest) -> InstallManifest {
    InstallManifest {
        schema_version: INSTALL_MANIFEST_SCHEMA_VERSION,
        provisioning_id: request.provisioning_id.as_str().into(),
        runtime_id: request.runtime_id.as_str().into(),
        endpoint: request.endpoint.clone(),
        install_profile: request.install_profile.clone(),
        artifact_sha256: request.artifact_sha256.clone(),
        api_credential_handle: request.api_credential_handle.as_str().into(),
        trust_credential_handle: request.trust_credential_handle.as_str().into(),
        api_token_sha256: hex(digest(&SHA256, request.api_token().as_bytes()).as_ref()),
    }
}

fn stage_generation(
    generations: &Path,
    destination: &Path,
    artifact: &[u8],
    request: &GewyvernInstallerRequest,
    manifest: &InstallManifest,
) -> Result<(), GewyvernInstallError> {
    let stage = generations.join(format!(".stage-{}-{}", std::process::id(), unique_suffix()));
    create_new_private_dir(&stage)?;
    let result = (|| {
        write_new_file(&stage.join(EXECUTABLE_NAME), artifact, 0o700)?;
        write_new_file(
            &stage.join(TOKEN_NAME),
            request.api_token().as_bytes(),
            0o600,
        )?;
        let (certificate, private_key) = generate_tls_identity(&request.endpoint)?;
        write_new_file(
            &stage.join(TLS_CERTIFICATE_NAME),
            certificate.as_bytes(),
            0o600,
        )?;
        write_new_file(
            &stage.join(TLS_PRIVATE_KEY_NAME),
            private_key.as_bytes(),
            0o600,
        )?;
        let manifest_bytes =
            serde_json::to_vec(manifest).map_err(|_| GewyvernInstallError::Storage)?;
        write_new_file(&stage.join(MANIFEST_NAME), &manifest_bytes, 0o600)?;
        let service_plan = service_plan(destination, request)?;
        let service_bytes =
            serde_json::to_vec(&service_plan).map_err(|_| GewyvernInstallError::Storage)?;
        write_new_file(&stage.join(SERVICE_PLAN_NAME), &service_bytes, 0o600)?;
        sync_dir(&stage)?;
        fs::rename(&stage, destination).map_err(|_| GewyvernInstallError::Storage)?;
        sync_dir(generations)
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&stage);
    }
    result
}

fn service_plan(
    generation: &Path,
    request: &GewyvernInstallerRequest,
) -> Result<ServicePlan, GewyvernInstallError> {
    let path = |name: &str| {
        generation
            .join(name)
            .to_str()
            .map(str::to_string)
            .ok_or(GewyvernInstallError::InvalidLayout)
    };
    Ok(ServicePlan {
        schema_version: INSTALL_MANIFEST_SCHEMA_VERSION,
        runtime_id: request.runtime_id.as_str().into(),
        endpoint: request.endpoint.clone(),
        executable: path(EXECUTABLE_NAME)?,
        api_token_file: path(TOKEN_NAME)?,
        tls_certificate_file: path(TLS_CERTIFICATE_NAME)?,
        tls_private_key_file: path(TLS_PRIVATE_KEY_NAME)?,
    })
}

fn verify_generation(
    generation: &Path,
    artifact: &[u8],
    request: &GewyvernInstallerRequest,
    expected_manifest: &InstallManifest,
) -> Result<(), GewyvernInstallError> {
    require_directory_mode(generation, 0o700)?;
    if read_private_file(
        &generation.join(EXECUTABLE_NAME),
        MAX_INSTALL_ARTIFACT_BYTES,
    )? != artifact
    {
        return Err(GewyvernInstallError::GenerationConflict);
    }
    require_mode(&generation.join(EXECUTABLE_NAME), 0o700)?;
    let token = Zeroizing::new(read_private_file(&generation.join(TOKEN_NAME), 257)?);
    require_mode(&generation.join(TOKEN_NAME), 0o600)?;
    if hex(digest(&SHA256, &token).as_ref()) != expected_manifest.api_token_sha256 {
        return Err(GewyvernInstallError::GenerationConflict);
    }
    let manifest_bytes = read_private_file(&generation.join(MANIFEST_NAME), 64 * 1024)?;
    let actual_manifest = serde_json::from_slice::<InstallManifest>(&manifest_bytes)
        .map_err(|_| GewyvernInstallError::GenerationConflict)?;
    if &actual_manifest != expected_manifest {
        return Err(GewyvernInstallError::GenerationConflict);
    }
    require_mode(&generation.join(MANIFEST_NAME), 0o600)?;
    let expected_plan = service_plan(generation, request)?;
    let plan_bytes = read_private_file(&generation.join(SERVICE_PLAN_NAME), 64 * 1024)?;
    let actual_plan = serde_json::from_slice::<ServicePlan>(&plan_bytes)
        .map_err(|_| GewyvernInstallError::GenerationConflict)?;
    if actual_plan != expected_plan {
        return Err(GewyvernInstallError::GenerationConflict);
    }
    require_mode(&generation.join(SERVICE_PLAN_NAME), 0o600)?;
    let certificate = read_private_text(&generation.join(TLS_CERTIFICATE_NAME), 32 * 1024)?;
    if !certificate.starts_with("-----BEGIN CERTIFICATE-----\n") {
        return Err(GewyvernInstallError::GenerationConflict);
    }
    require_mode(&generation.join(TLS_CERTIFICATE_NAME), 0o600)?;
    let private_key = Zeroizing::new(read_private_text(
        &generation.join(TLS_PRIVATE_KEY_NAME),
        32 * 1024,
    )?);
    if !private_key.starts_with("-----BEGIN PRIVATE KEY-----\n") {
        return Err(GewyvernInstallError::GenerationConflict);
    }
    require_mode(&generation.join(TLS_PRIVATE_KEY_NAME), 0o600)
}

fn generate_tls_identity(
    endpoint: &str,
) -> Result<(String, Zeroizing<String>), GewyvernInstallError> {
    let host = endpoint_host(endpoint).ok_or(GewyvernInstallError::TlsIdentity)?;
    let CertifiedKey { cert, signing_key } =
        generate_simple_self_signed(vec![host]).map_err(|_| GewyvernInstallError::TlsIdentity)?;
    Ok((cert.pem(), Zeroizing::new(signing_key.serialize_pem())))
}

fn endpoint_host(endpoint: &str) -> Option<String> {
    let authority = endpoint.strip_prefix("https://")?;
    if let Some(bracketed) = authority.strip_prefix('[') {
        let (host, suffix) = bracketed.split_once(']')?;
        if host.is_empty() || (!suffix.is_empty() && !suffix.starts_with(':')) {
            return None;
        }
        return Some(host.into());
    }
    match authority.rsplit_once(':') {
        Some((host, port)) if !host.is_empty() && port.parse::<u16>().is_ok() => Some(host.into()),
        None if !authority.is_empty() => Some(authority.into()),
        _ => None,
    }
}

fn commit_current(runtime_root: &Path, generation: &str) -> Result<(), GewyvernInstallError> {
    let temporary = runtime_root.join(format!(
        ".current-{}-{}",
        std::process::id(),
        unique_suffix()
    ));
    let result = (|| {
        write_new_file(&temporary, format!("{generation}\n").as_bytes(), 0o600)?;
        fs::rename(&temporary, runtime_root.join(CURRENT_NAME))
            .map_err(|_| GewyvernInstallError::Storage)?;
        sync_dir(runtime_root)
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}

fn read_artifact(path: &Path) -> Result<Vec<u8>, GewyvernInstallError> {
    let path_metadata =
        fs::symlink_metadata(path).map_err(|_| GewyvernInstallError::InvalidArtifact)?;
    if path_metadata.file_type().is_symlink() || !path_metadata.is_file() {
        return Err(GewyvernInstallError::InvalidArtifact);
    }
    let mut file = File::open(path).map_err(|_| GewyvernInstallError::InvalidArtifact)?;
    let metadata = file
        .metadata()
        .map_err(|_| GewyvernInstallError::InvalidArtifact)?;
    if metadata.len() == 0 || metadata.len() > MAX_INSTALL_ARTIFACT_BYTES {
        return Err(GewyvernInstallError::InvalidArtifact);
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.read_to_end(&mut bytes)
        .map_err(|_| GewyvernInstallError::InvalidArtifact)?;
    if bytes.len() as u64 != metadata.len() {
        return Err(GewyvernInstallError::InvalidArtifact);
    }
    Ok(bytes)
}

fn reject_existing_symlink_components(path: &Path) -> Result<(), GewyvernInstallError> {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(GewyvernInstallError::InvalidLayout);
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(GewyvernInstallError::Storage),
        }
    }
    Ok(())
}

fn create_private_dir(path: &Path) -> Result<(), GewyvernInstallError> {
    reject_existing_symlink_components(path)?;
    fs::create_dir_all(path).map_err(|_| GewyvernInstallError::Storage)?;
    let metadata = fs::symlink_metadata(path).map_err(|_| GewyvernInstallError::Storage)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(GewyvernInstallError::InvalidLayout);
    }
    set_mode(path, 0o700)
}

fn create_new_private_dir(path: &Path) -> Result<(), GewyvernInstallError> {
    fs::create_dir(path).map_err(|_| GewyvernInstallError::Storage)?;
    set_mode(path, 0o700)
}

fn write_new_file(path: &Path, bytes: &[u8], mode: u32) -> Result<(), GewyvernInstallError> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    set_open_mode(&mut options, mode);
    let mut file = options
        .open(path)
        .map_err(|_| GewyvernInstallError::Storage)?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| GewyvernInstallError::Storage)
}

fn read_private_file(path: &Path, limit: u64) -> Result<Vec<u8>, GewyvernInstallError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| GewyvernInstallError::Storage)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > limit {
        return Err(GewyvernInstallError::GenerationConflict);
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    File::open(path)
        .and_then(|mut file| file.read_to_end(&mut bytes))
        .map_err(|_| GewyvernInstallError::Storage)?;
    if bytes.len() as u64 != metadata.len() {
        return Err(GewyvernInstallError::GenerationConflict);
    }
    Ok(bytes)
}

fn read_private_text(path: &Path, limit: u64) -> Result<String, GewyvernInstallError> {
    String::from_utf8(read_private_file(path, limit)?)
        .map_err(|_| GewyvernInstallError::GenerationConflict)
}

fn sync_dir(path: &Path) -> Result<(), GewyvernInstallError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|_| GewyvernInstallError::Storage)
}

fn unique_suffix() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
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

#[cfg(unix)]
fn set_open_mode(options: &mut OpenOptions, mode: u32) {
    use std::os::unix::fs::OpenOptionsExt;
    options.mode(mode);
}

#[cfg(not(unix))]
fn set_open_mode(_options: &mut OpenOptions, _mode: u32) {}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> Result<(), GewyvernInstallError> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
        .map_err(|_| GewyvernInstallError::Storage)
}

#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: u32) -> Result<(), GewyvernInstallError> {
    Ok(())
}

#[cfg(unix)]
fn require_mode(path: &Path, expected: u32) -> Result<(), GewyvernInstallError> {
    use std::os::unix::fs::PermissionsExt;
    let metadata = fs::symlink_metadata(path).map_err(|_| GewyvernInstallError::Storage)?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.permissions().mode() & 0o777 != expected
    {
        return Err(GewyvernInstallError::GenerationConflict);
    }
    Ok(())
}

#[cfg(not(unix))]
fn require_mode(path: &Path, _expected: u32) -> Result<(), GewyvernInstallError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| GewyvernInstallError::Storage)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(GewyvernInstallError::GenerationConflict);
    }
    Ok(())
}

#[cfg(unix)]
fn require_directory_mode(path: &Path, expected: u32) -> Result<(), GewyvernInstallError> {
    use std::os::unix::fs::PermissionsExt;
    let metadata = fs::symlink_metadata(path).map_err(|_| GewyvernInstallError::Storage)?;
    if metadata.file_type().is_symlink()
        || !metadata.is_dir()
        || metadata.permissions().mode() & 0o777 != expected
    {
        return Err(GewyvernInstallError::GenerationConflict);
    }
    Ok(())
}

#[cfg(not(unix))]
fn require_directory_mode(path: &Path, _expected: u32) -> Result<(), GewyvernInstallError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| GewyvernInstallError::Storage)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(GewyvernInstallError::GenerationConflict);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::{PermissionsExt, symlink};

    use leserpent_domain::RuntimeId;
    use leserpent_domain::bootstrap::CredentialHandle;
    use leserpent_domain::provisioning::ProvisioningId;

    use super::*;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(label: &str) -> Self {
            let path = fs::canonicalize(env::temp_dir()).unwrap().join(format!(
                "gewyvern-install-{label}-{}-{}",
                std::process::id(),
                unique_suffix()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn fixture() -> (
        TempDir,
        PathBuf,
        GewyvernInstallerRequest,
        GewyvernInstallLayout,
    ) {
        let temp = TempDir::new("fixture");
        let source = temp.0.join("gewyvern-source");
        fs::write(&source, b"native-gewyvern-artifact").unwrap();
        #[cfg(unix)]
        fs::set_permissions(&source, fs::Permissions::from_mode(0o700)).unwrap();
        let artifact_sha256 = hex(digest(&SHA256, b"native-gewyvern-artifact").as_ref());
        let request = GewyvernInstallerRequest::new(
            ProvisioningId::new("provision-test-1").unwrap(),
            RuntimeId::new("runtime-test-1").unwrap(),
            "https://runtime.example:9443",
            "test",
            artifact_sha256,
            CredentialHandle::new("vault:gewyvern:runtime-api").unwrap(),
            CredentialHandle::new("vault:gewyvern-ca:runtime-ca").unwrap(),
            "0123456789abcdef0123456789abcdef",
        )
        .unwrap();
        let layout = GewyvernInstallLayout::test(temp.0.join("install"));
        (temp, source, request, layout)
    }

    #[test]
    fn preparation_is_private_atomic_and_idempotent() {
        let (_temp, source, request, layout) = fixture();
        let first = install_gewyvern_artifact(&source, &request, &layout).unwrap();
        assert_eq!(
            first.service_state,
            GewyvernInstallerServiceState::Installed
        );
        assert!(!first.replayed);
        let runtime_root = layout.runtime_root(&request);
        let generation = runtime_root.join("generations").join(&first.generation);
        assert_eq!(
            fs::read_to_string(runtime_root.join(CURRENT_NAME)).unwrap(),
            format!("{}\n", first.generation)
        );
        assert_eq!(
            fs::read_to_string(generation.join(TOKEN_NAME)).unwrap(),
            request.api_token()
        );
        let public = serde_json::to_string(&first).unwrap();
        assert!(!public.contains(request.api_token()));
        let manifest = fs::read_to_string(generation.join(MANIFEST_NAME)).unwrap();
        assert!(!manifest.contains(request.api_token()));
        let service = fs::read_to_string(generation.join(SERVICE_PLAN_NAME)).unwrap();
        assert!(!service.contains(request.api_token()));
        #[cfg(unix)]
        {
            assert_eq!(
                fs::metadata(generation.join(EXECUTABLE_NAME))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            );
            assert_eq!(
                fs::metadata(generation.join(TOKEN_NAME))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
            assert_eq!(
                fs::metadata(generation.join(TLS_PRIVATE_KEY_NAME))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }

        let replay = install_gewyvern_artifact(&source, &request, &layout).unwrap();
        assert!(replay.replayed);
        assert_eq!(replay.generation, first.generation);
        assert_eq!(replay.tls_ca_sha256, first.tls_ca_sha256);
    }

    #[test]
    fn digest_and_token_drift_fail_without_replacing_current() {
        let (_temp, source, mut request, layout) = fixture();
        let expected_digest = request.artifact_sha256.clone();
        request.artifact_sha256 = "0".repeat(64);
        assert_eq!(
            install_gewyvern_artifact(&source, &request, &layout),
            Err(GewyvernInstallError::ArtifactDigestMismatch)
        );
        request.artifact_sha256 = expected_digest;
        let first = install_gewyvern_artifact(&source, &request, &layout).unwrap();
        let current = fs::read(layout.runtime_root(&request).join(CURRENT_NAME)).unwrap();
        let replacement = GewyvernInstallerRequest::new(
            request.provisioning_id.clone(),
            request.runtime_id.clone(),
            request.endpoint.clone(),
            request.install_profile.clone(),
            request.artifact_sha256.clone(),
            request.api_credential_handle.clone(),
            request.trust_credential_handle.clone(),
            "abcdef0123456789abcdef0123456789",
        )
        .unwrap();
        assert_eq!(
            install_gewyvern_artifact(&source, &replacement, &layout),
            Err(GewyvernInstallError::GenerationConflict)
        );
        assert_eq!(
            fs::read(layout.runtime_root(&request).join(CURRENT_NAME)).unwrap(),
            current
        );
        assert!(!first.replayed);
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_source_and_layout_are_rejected() {
        let (temp, source, request, layout) = fixture();
        let source_link = temp.0.join("source-link");
        symlink(&source, &source_link).unwrap();
        assert_eq!(
            install_gewyvern_artifact(&source_link, &request, &layout),
            Err(GewyvernInstallError::InvalidArtifact)
        );

        let redirected = temp.0.join("redirected");
        fs::create_dir(&redirected).unwrap();
        let linked_root = temp.0.join("linked-root");
        symlink(&redirected, &linked_root).unwrap();
        assert_eq!(
            install_gewyvern_artifact(&source, &request, &GewyvernInstallLayout::test(linked_root)),
            Err(GewyvernInstallError::InvalidLayout)
        );
    }
}
