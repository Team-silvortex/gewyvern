use std::env;
use std::ffi::OsString;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use leserpent_protocol::gewyvern_installer::{
    GEWYVERN_INSTALLER_SCHEMA_VERSION, GewyvernInstallerRequest, GewyvernInstallerResponse,
    GewyvernInstallerServiceState, MAX_GEWYVERN_INSTALLER_BYTES, decode_gewyvern_installer_request,
    encode_gewyvern_installer_response,
};
use leserpent_protocol::gewyvern_retirement::{
    GEWYVERN_RETIREMENT_SCHEMA_VERSION, GewyvernRetirementRequest, GewyvernRetirementResponse,
    MAX_GEWYVERN_RETIREMENT_BYTES, decode_gewyvern_retirement_request,
    encode_gewyvern_retirement_response,
};
use leserpent_protocol::transport_safety::{
    MAX_HTTP_HEADER_BYTES, connect_with_deadline, is_http_header_name,
};
use rcgen::{CertifiedKey, generate_simple_self_signed};
use ring::digest::{SHA256, digest};
use rustls::pki_types::{CertificateDer, ServerName, pem::PemObject};
use rustls::{ClientConfig, ClientConnection, RootCertStore, StreamOwned};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

const MAX_INSTALL_ARTIFACT_BYTES: u64 = 256 * 1024 * 1024;
const INSTALL_MANIFEST_SCHEMA_VERSION: u32 = 2;
const EXECUTABLE_NAME: &str = "gewyvern";
const TOKEN_NAME: &str = "api.token";
const TLS_CERTIFICATE_NAME: &str = "server.crt";
const TLS_PRIVATE_KEY_NAME: &str = "server.key";
const MANIFEST_NAME: &str = "install.json";
const SERVICE_PLAN_NAME: &str = "service-plan.json";
const CURRENT_NAME: &str = "current";
const RETIREMENT_MARKER_SCHEMA_VERSION: u32 = 1;
#[cfg(target_os = "macos")]
const SERVICE_DESCRIPTOR_NAME: &str = "service.plist";
#[cfg(target_os = "linux")]
const SERVICE_DESCRIPTOR_NAME: &str = "service.service";
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
const SERVICE_DESCRIPTOR_NAME: &str = "service.conf";
const HEALTH_DEADLINE: Duration = Duration::from_secs(8);
const HEALTH_ATTEMPT_TIMEOUT: Duration = Duration::from_millis(750);
const HEALTH_RETRY_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GewyvernInstallError {
    InvalidRequest,
    InvalidLayout,
    InvalidArtifact,
    ArtifactDigestMismatch,
    GenerationConflict,
    TlsIdentity,
    ServiceActivation,
    HealthProof,
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
            Self::ServiceActivation => formatter.write_str("Gewyvern service activation failed"),
            Self::HealthProof => formatter.write_str("Gewyvern authenticated health proof failed"),
            Self::Storage => formatter.write_str("Gewyvern installation storage failed"),
        }
    }
}

impl std::error::Error for GewyvernInstallError {}

#[derive(Clone, Debug)]
pub struct GewyvernInstallLayout {
    root: PathBuf,
    service_directory: PathBuf,
    profile: String,
}

impl GewyvernInstallLayout {
    #[cfg(test)]
    fn test(root: PathBuf) -> Self {
        let service_directory = root.join("services");
        Self {
            root,
            service_directory,
            profile: "test".into(),
        }
    }

    fn runtime_root(&self, request: &GewyvernInstallerRequest) -> PathBuf {
        self.runtime_root_for(request.runtime_id.as_str())
    }

    fn runtime_root_for(&self, runtime_id: &str) -> PathBuf {
        self.root.join("runtimes").join(runtime_id)
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
    listen: String,
    executable: String,
    api_token_file: String,
    tls_certificate_file: String,
    tls_private_key_file: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum RetirementMarkerPhase {
    Retiring,
    ServiceRetired,
    Retired,
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct RetirementMarker {
    schema_version: u32,
    retirement_id: String,
    provisioning_id: String,
    runtime_id: String,
    install_profile: String,
    phase: RetirementMarkerPhase,
}

pub fn run_gewyvern_install_stdio() -> Result<(), GewyvernInstallError> {
    let request = read_stdio_request(std::io::stdin().lock())?;
    let layout = platform_layout(&request.install_profile)?;
    let source = env::current_exe().map_err(|_| GewyvernInstallError::InvalidArtifact)?;
    let response = install_gewyvern_artifact(&source, &request, &layout)?;
    write_stdio_response(&response)
}

pub fn run_gewyvern_activate_stdio() -> Result<(), GewyvernInstallError> {
    let request = read_stdio_request(std::io::stdin().lock())?;
    let layout = platform_layout(&request.install_profile)?;
    let source = env::current_exe().map_err(|_| GewyvernInstallError::InvalidArtifact)?;
    let response = activate_gewyvern_artifact(&source, &request, &layout)?;
    write_stdio_response(&response)
}

pub fn run_gewyvern_retire_stdio() -> Result<(), GewyvernInstallError> {
    let request = read_retirement_stdio_request(std::io::stdin().lock())?;
    let layout = platform_layout(&request.install_profile)?;
    let response = retire_gewyvern(&request, &layout)?;
    let encoded = encode_gewyvern_retirement_response(&response)
        .map_err(|_| GewyvernInstallError::InvalidRequest)?;
    let mut stdout = std::io::stdout().lock();
    stdout
        .write_all(&encoded)
        .and_then(|()| stdout.write_all(b"\n"))
        .and_then(|()| stdout.flush())
        .map_err(|_| GewyvernInstallError::Storage)
}

pub fn run_gewyvern_service() -> Result<(), GewyvernInstallError> {
    let executable = env::current_exe().map_err(|_| GewyvernInstallError::InvalidArtifact)?;
    let generation = executable
        .parent()
        .ok_or(GewyvernInstallError::InvalidLayout)?;
    let plan = read_service_plan(generation)?;
    if Path::new(&plan.executable) != executable {
        return Err(GewyvernInstallError::GenerationConflict);
    }
    let token = Zeroizing::new(read_private_text(Path::new(&plan.api_token_file), 256)?);
    let _service = crate::data_api::start_tls_api_service(
        &plan.listen,
        &plan.tls_certificate_file,
        &plan.tls_private_key_file,
        &token,
    )
    .map_err(|_| GewyvernInstallError::ServiceActivation)?;
    loop {
        thread::park_timeout(Duration::from_secs(60));
    }
}

fn write_stdio_response(response: &GewyvernInstallerResponse) -> Result<(), GewyvernInstallError> {
    let encoded = encode_gewyvern_installer_response(response)
        .map_err(|_| GewyvernInstallError::InvalidRequest)?;
    let mut stdout = std::io::stdout().lock();
    stdout
        .write_all(&encoded)
        .and_then(|()| stdout.write_all(b"\n"))
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
    create_private_dir(&layout.root)?;
    create_private_dir(&layout.root.join("runtimes"))?;
    create_private_dir(&runtime_root)?;
    create_private_dir(&runtime_root.join("logs"))?;
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

pub fn activate_gewyvern_artifact(
    source: &Path,
    request: &GewyvernInstallerRequest,
    layout: &GewyvernInstallLayout,
) -> Result<GewyvernInstallerResponse, GewyvernInstallError> {
    activate_gewyvern_artifact_with(
        source,
        request,
        layout,
        || activate_published_service(request, layout),
        |ca_pem| prove_gewyvern_health(&request.endpoint, ca_pem, request.api_token()),
        |had_previous| rollback_published_service(request, layout, had_previous),
    )
}

fn activate_gewyvern_artifact_with(
    source: &Path,
    request: &GewyvernInstallerRequest,
    layout: &GewyvernInstallLayout,
    activate: impl FnOnce() -> Result<(), GewyvernInstallError>,
    prove_health: impl FnOnce(&str) -> Result<(), GewyvernInstallError>,
    rollback_service: impl FnOnce(bool) -> Result<(), GewyvernInstallError>,
) -> Result<GewyvernInstallerResponse, GewyvernInstallError> {
    let rollback = ActivationRollback::capture(request, layout)?;
    let mut response = match install_gewyvern_artifact(source, request, layout) {
        Ok(response) => response,
        Err(error) => {
            rollback.restore(request, layout)?;
            return Err(error);
        }
    };
    let generation = layout
        .runtime_root(request)
        .join("generations")
        .join(&response.generation);
    if let Err(error) = publish_service_descriptor(&generation, request, layout)
        .and_then(|()| activate())
        .and_then(|()| prove_health(&response.tls_ca_pem))
    {
        let state_restore = rollback.restore(request, layout);
        let service_restore = rollback_service(rollback.previous_descriptor.is_some());
        state_restore?;
        service_restore?;
        return Err(error);
    }
    response.service_state = GewyvernInstallerServiceState::Ready;
    Ok(response)
}

fn retire_gewyvern(
    request: &GewyvernRetirementRequest,
    layout: &GewyvernInstallLayout,
) -> Result<GewyvernRetirementResponse, GewyvernInstallError> {
    retire_gewyvern_with(request, layout, || {
        execute_service_commands(service_retirement_commands(
            request.runtime_id.as_str(),
            layout,
        )?)
    })
}

fn retire_gewyvern_with(
    request: &GewyvernRetirementRequest,
    layout: &GewyvernInstallLayout,
    retire_service: impl FnOnce() -> Result<(), GewyvernInstallError>,
) -> Result<GewyvernRetirementResponse, GewyvernInstallError> {
    request
        .validate()
        .map_err(|_| GewyvernInstallError::InvalidRequest)?;
    if layout.profile != request.install_profile {
        return Err(GewyvernInstallError::InvalidRequest);
    }
    validate_root(&layout.root)?;
    validate_root(&layout.service_directory)?;
    let retirements = layout.root.join("retirements");
    let marker_path = retirements.join(format!("{}.json", request.runtime_id.as_str()));
    let expected = retirement_marker(request, RetirementMarkerPhase::Retiring);
    let existing = read_retirement_marker(&marker_path)?;
    let replayed = existing.is_some();
    let phase = match existing {
        Some(marker) if !same_retirement_identity(&marker, &expected) => {
            return Err(GewyvernInstallError::GenerationConflict);
        }
        Some(marker) if marker.phase == RetirementMarkerPhase::Retired => {
            return Ok(retirement_response(request, true));
        }
        Some(marker) => marker.phase,
        None => {
            verify_retirement_authority(request, layout)?;
            create_private_dir(&layout.root)?;
            create_private_dir(&retirements)?;
            let encoded =
                serde_json::to_vec(&expected).map_err(|_| GewyvernInstallError::Storage)?;
            restore_private_file(&marker_path, Some(&encoded))?;
            RetirementMarkerPhase::Retiring
        }
    };

    let descriptor = published_service_path_for(request.runtime_id.as_str(), layout);
    if phase == RetirementMarkerPhase::Retiring {
        if !descriptor.exists() {
            return Err(GewyvernInstallError::GenerationConflict);
        }
        retire_service()?;
        let stopped = retirement_marker(request, RetirementMarkerPhase::ServiceRetired);
        let encoded = serde_json::to_vec(&stopped).map_err(|_| GewyvernInstallError::Storage)?;
        restore_private_file(&marker_path, Some(&encoded))?;
    }
    if descriptor.exists() {
        restore_private_file(&descriptor, None)?;
    }
    let runtime_root = layout.runtime_root_for(request.runtime_id.as_str());
    match fs::symlink_metadata(&runtime_root) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {
            fs::remove_dir_all(&runtime_root).map_err(|_| GewyvernInstallError::Storage)?;
            sync_dir(
                runtime_root
                    .parent()
                    .ok_or(GewyvernInstallError::InvalidLayout)?,
            )?;
        }
        Ok(_) => return Err(GewyvernInstallError::InvalidLayout),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Err(GewyvernInstallError::Storage),
    }
    let retired = retirement_marker(request, RetirementMarkerPhase::Retired);
    let encoded = serde_json::to_vec(&retired).map_err(|_| GewyvernInstallError::Storage)?;
    restore_private_file(&marker_path, Some(&encoded))?;
    Ok(retirement_response(request, replayed))
}

fn verify_retirement_authority(
    request: &GewyvernRetirementRequest,
    layout: &GewyvernInstallLayout,
) -> Result<(), GewyvernInstallError> {
    let runtime_root = layout.runtime_root_for(request.runtime_id.as_str());
    let metadata =
        fs::symlink_metadata(&runtime_root).map_err(|_| GewyvernInstallError::Storage)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(GewyvernInstallError::InvalidLayout);
    }
    require_directory_mode(&runtime_root, 0o700)?;
    require_directory_mode(&runtime_root.join("generations"), 0o700)?;
    let current = read_private_text(&runtime_root.join(CURRENT_NAME), 128)?;
    require_mode(&runtime_root.join(CURRENT_NAME), 0o600)?;
    let generation_name = current
        .strip_suffix('\n')
        .filter(|value| {
            value.len() == 64
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        })
        .ok_or(GewyvernInstallError::GenerationConflict)?;
    let generation = runtime_root.join("generations").join(generation_name);
    require_directory_mode(&generation, 0o700)?;
    let manifest = serde_json::from_slice::<InstallManifest>(&read_private_file(
        &generation.join(MANIFEST_NAME),
        64 * 1024,
    )?)
    .map_err(|_| GewyvernInstallError::GenerationConflict)?;
    require_mode(&generation.join(MANIFEST_NAME), 0o600)?;
    if manifest.schema_version != INSTALL_MANIFEST_SCHEMA_VERSION
        || manifest.provisioning_id != request.provisioning_id.as_str()
        || manifest.runtime_id != request.runtime_id.as_str()
        || manifest.install_profile != request.install_profile
    {
        return Err(GewyvernInstallError::GenerationConflict);
    }
    let retained = read_private_file(&generation.join(SERVICE_DESCRIPTOR_NAME), 64 * 1024)?;
    require_mode(&generation.join(SERVICE_DESCRIPTOR_NAME), 0o600)?;
    let published_path = published_service_path_for(request.runtime_id.as_str(), layout);
    let published = read_private_file(&published_path, 64 * 1024)?;
    require_mode(&published_path, 0o600)?;
    if retained != published {
        return Err(GewyvernInstallError::GenerationConflict);
    }
    Ok(())
}

fn retirement_marker(
    request: &GewyvernRetirementRequest,
    phase: RetirementMarkerPhase,
) -> RetirementMarker {
    RetirementMarker {
        schema_version: RETIREMENT_MARKER_SCHEMA_VERSION,
        retirement_id: request.retirement_id.as_str().into(),
        provisioning_id: request.provisioning_id.as_str().into(),
        runtime_id: request.runtime_id.as_str().into(),
        install_profile: request.install_profile.clone(),
        phase,
    }
}

fn read_retirement_marker(path: &Path) -> Result<Option<RetirementMarker>, GewyvernInstallError> {
    let Some(bytes) = read_optional_private_file(path, 64 * 1024)? else {
        return Ok(None);
    };
    require_mode(path, 0o600)?;
    let marker = serde_json::from_slice::<RetirementMarker>(&bytes)
        .map_err(|_| GewyvernInstallError::GenerationConflict)?;
    if marker.schema_version != RETIREMENT_MARKER_SCHEMA_VERSION {
        return Err(GewyvernInstallError::GenerationConflict);
    }
    Ok(Some(marker))
}

fn same_retirement_identity(left: &RetirementMarker, right: &RetirementMarker) -> bool {
    left.schema_version == right.schema_version
        && left.retirement_id == right.retirement_id
        && left.provisioning_id == right.provisioning_id
        && left.runtime_id == right.runtime_id
        && left.install_profile == right.install_profile
}

fn retirement_response(
    request: &GewyvernRetirementRequest,
    replayed: bool,
) -> GewyvernRetirementResponse {
    GewyvernRetirementResponse {
        schema_version: GEWYVERN_RETIREMENT_SCHEMA_VERSION,
        retirement_id: request.retirement_id.clone(),
        provisioning_id: request.provisioning_id.clone(),
        runtime_id: request.runtime_id.clone(),
        service_retired: true,
        replayed,
    }
}

struct ActivationRollback {
    previous_current: Option<Vec<u8>>,
    previous_descriptor: Option<Vec<u8>>,
    generation_preexisted: bool,
}

impl ActivationRollback {
    fn capture(
        request: &GewyvernInstallerRequest,
        layout: &GewyvernInstallLayout,
    ) -> Result<Self, GewyvernInstallError> {
        let generation = layout
            .runtime_root(request)
            .join("generations")
            .join(&request.artifact_sha256);
        Ok(Self {
            previous_current: read_optional_private_file(
                &layout.runtime_root(request).join(CURRENT_NAME),
                128,
            )?,
            previous_descriptor: read_optional_private_file(
                &published_service_path(request, layout),
                64 * 1024,
            )?,
            generation_preexisted: generation.exists(),
        })
    }

    fn restore(
        &self,
        request: &GewyvernInstallerRequest,
        layout: &GewyvernInstallLayout,
    ) -> Result<(), GewyvernInstallError> {
        restore_private_file(
            &layout.runtime_root(request).join(CURRENT_NAME),
            self.previous_current.as_deref(),
        )?;
        restore_private_file(
            &published_service_path(request, layout),
            self.previous_descriptor.as_deref(),
        )?;
        if !self.generation_preexisted {
            let generation = layout
                .runtime_root(request)
                .join("generations")
                .join(&request.artifact_sha256);
            if generation.exists() {
                fs::remove_dir_all(&generation).map_err(|_| GewyvernInstallError::Storage)?;
                sync_dir(
                    generation
                        .parent()
                        .ok_or(GewyvernInstallError::InvalidLayout)?,
                )?;
            }
        }
        Ok(())
    }
}

fn publish_service_descriptor(
    generation: &Path,
    request: &GewyvernInstallerRequest,
    layout: &GewyvernInstallLayout,
) -> Result<(), GewyvernInstallError> {
    reject_existing_symlink_components(&layout.service_directory)?;
    fs::create_dir_all(&layout.service_directory).map_err(|_| GewyvernInstallError::Storage)?;
    let metadata = fs::symlink_metadata(&layout.service_directory)
        .map_err(|_| GewyvernInstallError::Storage)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(GewyvernInstallError::InvalidLayout);
    }
    let retained = read_private_file(&generation.join(SERVICE_DESCRIPTOR_NAME), 64 * 1024)?;
    let destination = published_service_path(request, layout);
    reject_non_regular_destination(&destination)?;
    let temporary = layout.service_directory.join(format!(
        ".{}-{}-{}",
        service_descriptor_file_name(request),
        std::process::id(),
        unique_suffix()
    ));
    let result = (|| {
        write_new_file(&temporary, &retained, 0o600)?;
        fs::rename(&temporary, &destination).map_err(|_| GewyvernInstallError::Storage)?;
        sync_dir(&layout.service_directory)?;
        if read_private_file(&destination, 64 * 1024)? != retained {
            return Err(GewyvernInstallError::GenerationConflict);
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}

fn reject_non_regular_destination(path: &Path) -> Result<(), GewyvernInstallError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            Err(GewyvernInstallError::InvalidLayout)
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(GewyvernInstallError::Storage),
    }
}

fn published_service_path(
    request: &GewyvernInstallerRequest,
    layout: &GewyvernInstallLayout,
) -> PathBuf {
    published_service_path_for(request.runtime_id.as_str(), layout)
}

fn published_service_path_for(runtime_id: &str, layout: &GewyvernInstallLayout) -> PathBuf {
    layout
        .service_directory
        .join(service_descriptor_file_name_for(runtime_id))
}

#[cfg(target_os = "macos")]
fn service_descriptor_file_name(request: &GewyvernInstallerRequest) -> String {
    service_descriptor_file_name_for(request.runtime_id.as_str())
}

#[cfg(target_os = "macos")]
fn service_descriptor_file_name_for(runtime_id: &str) -> String {
    format!("org.gewyvern.runtime.{runtime_id}.plist")
}

#[cfg(target_os = "linux")]
fn service_descriptor_file_name(request: &GewyvernInstallerRequest) -> String {
    service_descriptor_file_name_for(request.runtime_id.as_str())
}

#[cfg(target_os = "linux")]
fn service_descriptor_file_name_for(runtime_id: &str) -> String {
    format!("gewyvern-{runtime_id}.service")
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn service_descriptor_file_name(request: &GewyvernInstallerRequest) -> String {
    service_descriptor_file_name_for(request.runtime_id.as_str())
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn service_descriptor_file_name_for(runtime_id: &str) -> String {
    format!("gewyvern-{runtime_id}.conf")
}

struct ServiceManagerCommand {
    program: PathBuf,
    arguments: Vec<OsString>,
    tolerate_failure: bool,
}

fn activate_published_service(
    request: &GewyvernInstallerRequest,
    layout: &GewyvernInstallLayout,
) -> Result<(), GewyvernInstallError> {
    verify_published_service(request, layout)?;
    execute_service_commands(service_activation_commands(request, layout)?)
}

fn rollback_published_service(
    request: &GewyvernInstallerRequest,
    layout: &GewyvernInstallLayout,
    had_previous: bool,
) -> Result<(), GewyvernInstallError> {
    execute_service_commands(service_rollback_commands(request, layout, had_previous)?)
}

fn verify_published_service(
    request: &GewyvernInstallerRequest,
    layout: &GewyvernInstallLayout,
) -> Result<(), GewyvernInstallError> {
    let current = read_private_file(&layout.runtime_root(request).join(CURRENT_NAME), 128)?;
    if current != format!("{}\n", request.artifact_sha256).as_bytes() {
        return Err(GewyvernInstallError::GenerationConflict);
    }
    let generation = layout
        .runtime_root(request)
        .join("generations")
        .join(&request.artifact_sha256);
    let retained = read_private_file(&generation.join(SERVICE_DESCRIPTOR_NAME), 64 * 1024)?;
    let published = read_private_file(&published_service_path(request, layout), 64 * 1024)?;
    if retained != published {
        return Err(GewyvernInstallError::GenerationConflict);
    }
    Ok(())
}

fn execute_service_commands(
    commands: Vec<ServiceManagerCommand>,
) -> Result<(), GewyvernInstallError> {
    for command in commands {
        let status = Command::new(&command.program)
            .args(&command.arguments)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|_| GewyvernInstallError::ServiceActivation)?;
        if !status.success() && !command.tolerate_failure {
            return Err(GewyvernInstallError::ServiceActivation);
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn service_activation_commands(
    request: &GewyvernInstallerRequest,
    layout: &GewyvernInstallLayout,
) -> Result<Vec<ServiceManagerCommand>, GewyvernInstallError> {
    use std::os::unix::fs::MetadataExt;

    let domain = match layout.profile.as_str() {
        "system" => "system".to_string(),
        "user" => format!(
            "gui/{}",
            fs::metadata(&layout.service_directory)
                .map_err(|_| GewyvernInstallError::ServiceActivation)?
                .uid()
        ),
        _ => return Err(GewyvernInstallError::ServiceActivation),
    };
    let label = format!("org.gewyvern.runtime.{}", request.runtime_id.as_str());
    let target = format!("{domain}/{label}");
    Ok(vec![
        ServiceManagerCommand {
            program: PathBuf::from("/bin/launchctl"),
            arguments: vec!["bootout".into(), target.clone().into()],
            tolerate_failure: true,
        },
        ServiceManagerCommand {
            program: PathBuf::from("/bin/launchctl"),
            arguments: vec![
                "bootstrap".into(),
                domain.into(),
                published_service_path(request, layout).into_os_string(),
            ],
            tolerate_failure: false,
        },
        ServiceManagerCommand {
            program: PathBuf::from("/bin/launchctl"),
            arguments: vec!["kickstart".into(), "-k".into(), target.into()],
            tolerate_failure: false,
        },
    ])
}

#[cfg(target_os = "macos")]
fn service_rollback_commands(
    request: &GewyvernInstallerRequest,
    layout: &GewyvernInstallLayout,
    had_previous: bool,
) -> Result<Vec<ServiceManagerCommand>, GewyvernInstallError> {
    use std::os::unix::fs::MetadataExt;

    let domain = match layout.profile.as_str() {
        "system" => "system".to_string(),
        "user" => format!(
            "gui/{}",
            fs::metadata(&layout.service_directory)
                .map_err(|_| GewyvernInstallError::ServiceActivation)?
                .uid()
        ),
        _ => return Err(GewyvernInstallError::ServiceActivation),
    };
    let label = format!("org.gewyvern.runtime.{}", request.runtime_id.as_str());
    let target = format!("{domain}/{label}");
    let mut commands = vec![ServiceManagerCommand {
        program: PathBuf::from("/bin/launchctl"),
        arguments: vec!["bootout".into(), target.clone().into()],
        tolerate_failure: true,
    }];
    if had_previous {
        commands.extend([
            ServiceManagerCommand {
                program: PathBuf::from("/bin/launchctl"),
                arguments: vec![
                    "bootstrap".into(),
                    domain.into(),
                    published_service_path(request, layout).into_os_string(),
                ],
                tolerate_failure: false,
            },
            ServiceManagerCommand {
                program: PathBuf::from("/bin/launchctl"),
                arguments: vec!["kickstart".into(), "-k".into(), target.into()],
                tolerate_failure: false,
            },
        ]);
    }
    Ok(commands)
}

#[cfg(target_os = "macos")]
fn service_retirement_commands(
    runtime_id: &str,
    layout: &GewyvernInstallLayout,
) -> Result<Vec<ServiceManagerCommand>, GewyvernInstallError> {
    use std::os::unix::fs::MetadataExt;

    let domain = match layout.profile.as_str() {
        "system" => "system".to_string(),
        "user" => format!(
            "gui/{}",
            fs::metadata(&layout.service_directory)
                .map_err(|_| GewyvernInstallError::ServiceActivation)?
                .uid()
        ),
        _ => return Err(GewyvernInstallError::ServiceActivation),
    };
    let label = format!("org.gewyvern.runtime.{runtime_id}");
    Ok(vec![ServiceManagerCommand {
        program: PathBuf::from("/bin/launchctl"),
        arguments: vec!["bootout".into(), format!("{domain}/{label}").into()],
        tolerate_failure: false,
    }])
}

#[cfg(target_os = "linux")]
fn service_activation_commands(
    request: &GewyvernInstallerRequest,
    layout: &GewyvernInstallLayout,
) -> Result<Vec<ServiceManagerCommand>, GewyvernInstallError> {
    let (program, prefix) = systemctl(layout)?;
    let unit = service_descriptor_file_name(request);
    let command = |verb: &str, include_unit: bool, tolerate_failure: bool| {
        let mut arguments = prefix.clone();
        arguments.push(verb.into());
        if include_unit {
            arguments.push(unit.clone().into());
        }
        ServiceManagerCommand {
            program: program.clone(),
            arguments,
            tolerate_failure,
        }
    };
    Ok(vec![
        command("daemon-reload", false, false),
        command("enable", true, false),
        command("restart", true, false),
    ])
}

#[cfg(target_os = "linux")]
fn service_rollback_commands(
    request: &GewyvernInstallerRequest,
    layout: &GewyvernInstallLayout,
    had_previous: bool,
) -> Result<Vec<ServiceManagerCommand>, GewyvernInstallError> {
    let (program, prefix) = systemctl(layout)?;
    let unit = service_descriptor_file_name(request);
    let command = |verb: &str, include_unit: bool, tolerate_failure: bool| {
        let mut arguments = prefix.clone();
        arguments.push(verb.into());
        if include_unit {
            arguments.push(unit.clone().into());
        }
        ServiceManagerCommand {
            program: program.clone(),
            arguments,
            tolerate_failure,
        }
    };
    let mut commands = vec![command("stop", true, true)];
    if had_previous {
        commands.extend([
            command("daemon-reload", false, false),
            command("enable", true, false),
            command("restart", true, false),
        ]);
    } else {
        commands.extend([
            command("disable", true, true),
            command("daemon-reload", false, false),
        ]);
    }
    Ok(commands)
}

#[cfg(target_os = "linux")]
fn service_retirement_commands(
    runtime_id: &str,
    layout: &GewyvernInstallLayout,
) -> Result<Vec<ServiceManagerCommand>, GewyvernInstallError> {
    let (program, prefix) = systemctl(layout)?;
    let unit = service_descriptor_file_name_for(runtime_id);
    let command = |verb: &str, include_unit: bool, tolerate_failure: bool| {
        let mut arguments = prefix.clone();
        arguments.push(verb.into());
        if include_unit {
            arguments.push(unit.clone().into());
        }
        ServiceManagerCommand {
            program: program.clone(),
            arguments,
            tolerate_failure,
        }
    };
    Ok(vec![
        command("stop", true, false),
        command("disable", true, true),
        command("daemon-reload", false, false),
    ])
}

#[cfg(target_os = "linux")]
fn systemctl(
    layout: &GewyvernInstallLayout,
) -> Result<(PathBuf, Vec<OsString>), GewyvernInstallError> {
    let program = ["/usr/bin/systemctl", "/bin/systemctl"]
        .into_iter()
        .map(PathBuf::from)
        .find(|path| path.is_file())
        .ok_or(GewyvernInstallError::ServiceActivation)?;
    let prefix = match layout.profile.as_str() {
        "system" => Vec::new(),
        "user" => vec![OsString::from("--user")],
        _ => return Err(GewyvernInstallError::ServiceActivation),
    };
    Ok((program, prefix))
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn service_activation_commands(
    _request: &GewyvernInstallerRequest,
    _layout: &GewyvernInstallLayout,
) -> Result<Vec<ServiceManagerCommand>, GewyvernInstallError> {
    Err(GewyvernInstallError::ServiceActivation)
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn service_retirement_commands(
    _runtime_id: &str,
    _layout: &GewyvernInstallLayout,
) -> Result<Vec<ServiceManagerCommand>, GewyvernInstallError> {
    Err(GewyvernInstallError::ServiceActivation)
}

fn prove_gewyvern_health(
    endpoint: &str,
    ca_pem: &str,
    token: &str,
) -> Result<(), GewyvernInstallError> {
    let endpoint = HealthEndpoint::parse(endpoint)?;
    let tls = health_client_config(ca_pem)?;
    let deadline = Instant::now() + HEALTH_DEADLINE;
    loop {
        if probe_health_once(&endpoint, Arc::clone(&tls), token).is_ok() {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(GewyvernInstallError::HealthProof);
        }
        thread::sleep(HEALTH_RETRY_INTERVAL);
    }
}

fn health_client_config(ca_pem: &str) -> Result<Arc<ClientConfig>, GewyvernInstallError> {
    let certificates = CertificateDer::pem_slice_iter(ca_pem.as_bytes())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| GewyvernInstallError::TlsIdentity)?;
    if certificates.is_empty() {
        return Err(GewyvernInstallError::TlsIdentity);
    }
    let mut roots = RootCertStore::empty();
    for certificate in certificates {
        roots
            .add(certificate)
            .map_err(|_| GewyvernInstallError::TlsIdentity)?;
    }
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut config = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|_| GewyvernInstallError::TlsIdentity)?
        .with_root_certificates(roots)
        .with_no_client_auth();
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    Ok(Arc::new(config))
}

fn probe_health_once(
    endpoint: &HealthEndpoint,
    tls: Arc<ClientConfig>,
    token: &str,
) -> Result<(), GewyvernInstallError> {
    let socket = connect_with_deadline(
        SocketAddr::from((Ipv4Addr::LOCALHOST, endpoint.port)),
        HEALTH_ATTEMPT_TIMEOUT,
    )
    .map_err(|_| GewyvernInstallError::HealthProof)?;
    socket
        .set_read_timeout(Some(HEALTH_ATTEMPT_TIMEOUT))
        .and_then(|()| socket.set_write_timeout(Some(HEALTH_ATTEMPT_TIMEOUT)))
        .map_err(|_| GewyvernInstallError::HealthProof)?;
    let connection = ClientConnection::new(tls, endpoint.server_name.clone())
        .map_err(|_| GewyvernInstallError::HealthProof)?;
    let mut stream = StreamOwned::new(connection, socket);
    let header = Zeroizing::new(format!(
        "GET /health HTTP/1.1\r\nHost: {}\r\nX-Gewyvern-Admin-Token: {}\r\nAccept: application/json\r\nConnection: close\r\n\r\n",
        endpoint.authority, token
    ));
    stream
        .write_all(header.as_bytes())
        .and_then(|()| stream.flush())
        .map_err(|_| GewyvernInstallError::HealthProof)?;
    let body = read_health_response(&mut stream)?;
    let value = serde_json::from_slice::<serde_json::Value>(&body)
        .map_err(|_| GewyvernInstallError::HealthProof)?;
    if value.get("ok") != Some(&serde_json::Value::Bool(true)) {
        return Err(GewyvernInstallError::HealthProof);
    }
    Ok(())
}

fn read_health_response(reader: &mut impl Read) -> Result<Vec<u8>, GewyvernInstallError> {
    const MAX_HEALTH_BODY_BYTES: usize = 64 * 1024;
    let mut bytes = Vec::with_capacity(2048);
    let header_end = loop {
        if let Some(position) = bytes
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .map(|position| position + 4)
        {
            break position;
        }
        if bytes.len() > MAX_HTTP_HEADER_BYTES {
            return Err(GewyvernInstallError::HealthProof);
        }
        let mut chunk = [0_u8; 1024];
        let read = reader
            .read(&mut chunk)
            .map_err(|_| GewyvernInstallError::HealthProof)?;
        if read == 0 {
            return Err(GewyvernInstallError::HealthProof);
        }
        bytes.extend_from_slice(&chunk[..read]);
    };
    if header_end > MAX_HTTP_HEADER_BYTES || !bytes[..header_end - 4].is_ascii() {
        return Err(GewyvernInstallError::HealthProof);
    }
    let header = std::str::from_utf8(&bytes[..header_end - 4])
        .map_err(|_| GewyvernInstallError::HealthProof)?;
    let mut lines = header.split("\r\n");
    if lines.next().and_then(|line| line.split_whitespace().nth(1)) != Some("200") {
        return Err(GewyvernInstallError::HealthProof);
    }
    let mut content_length = None;
    let mut content_type = None;
    for line in lines {
        let (name, value) = line
            .split_once(':')
            .ok_or(GewyvernInstallError::HealthProof)?;
        if !is_http_header_name(name) {
            return Err(GewyvernInstallError::HealthProof);
        }
        let value = value.trim();
        if name.eq_ignore_ascii_case("content-length") {
            if content_length.replace(value).is_some() {
                return Err(GewyvernInstallError::HealthProof);
            }
        } else if name.eq_ignore_ascii_case("content-type") {
            if content_type.replace(value).is_some() {
                return Err(GewyvernInstallError::HealthProof);
            }
        } else if name.eq_ignore_ascii_case("transfer-encoding") {
            return Err(GewyvernInstallError::HealthProof);
        }
    }
    if !content_type.is_some_and(|value| {
        value
            .split(';')
            .next()
            .is_some_and(|kind| kind.trim().eq_ignore_ascii_case("application/json"))
    }) {
        return Err(GewyvernInstallError::HealthProof);
    }
    let content_length = content_length
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|length| *length <= MAX_HEALTH_BODY_BYTES)
        .ok_or(GewyvernInstallError::HealthProof)?;
    let mut body = bytes.split_off(header_end);
    if body.len() > content_length {
        return Err(GewyvernInstallError::HealthProof);
    }
    let initial = body.len();
    body.resize(content_length, 0);
    reader
        .read_exact(&mut body[initial..])
        .map_err(|_| GewyvernInstallError::HealthProof)?;
    Ok(body)
}

struct HealthEndpoint {
    authority: String,
    port: u16,
    server_name: ServerName<'static>,
}

impl HealthEndpoint {
    fn parse(endpoint: &str) -> Result<Self, GewyvernInstallError> {
        let authority = endpoint
            .strip_prefix("https://")
            .filter(|value| {
                !value.is_empty()
                    && value.len() <= 320
                    && !value.contains(['/', '?', '#', '@'])
                    && !value.bytes().any(|byte| byte <= 0x20)
            })
            .ok_or(GewyvernInstallError::InvalidRequest)?;
        let (host, port) = if let Some(bracketed) = authority.strip_prefix('[') {
            let (host, suffix) = bracketed
                .split_once(']')
                .ok_or(GewyvernInstallError::InvalidRequest)?;
            let port = suffix
                .strip_prefix(':')
                .map(str::parse::<u16>)
                .transpose()
                .map_err(|_| GewyvernInstallError::InvalidRequest)?
                .unwrap_or(443);
            (host.to_string(), port)
        } else {
            match authority.rsplit_once(':') {
                Some((host, port)) => (
                    host.to_string(),
                    port.parse::<u16>()
                        .map_err(|_| GewyvernInstallError::InvalidRequest)?,
                ),
                None => (authority.to_string(), 443),
            }
        };
        if port == 0 {
            return Err(GewyvernInstallError::InvalidRequest);
        }
        let server_name =
            ServerName::try_from(host).map_err(|_| GewyvernInstallError::InvalidRequest)?;
        Ok(Self {
            authority: authority.into(),
            port,
            server_name,
        })
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn service_rollback_commands(
    _request: &GewyvernInstallerRequest,
    _layout: &GewyvernInstallLayout,
    _had_previous: bool,
) -> Result<Vec<ServiceManagerCommand>, GewyvernInstallError> {
    Err(GewyvernInstallError::ServiceActivation)
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

fn read_retirement_stdio_request(
    mut reader: impl Read,
) -> Result<GewyvernRetirementRequest, GewyvernInstallError> {
    let mut bytes = Zeroizing::new(Vec::new());
    reader
        .by_ref()
        .take((MAX_GEWYVERN_RETIREMENT_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| GewyvernInstallError::InvalidRequest)?;
    decode_gewyvern_retirement_request(&bytes).map_err(|_| GewyvernInstallError::InvalidRequest)
}

fn platform_layout(profile: &str) -> Result<GewyvernInstallLayout, GewyvernInstallError> {
    let (root, service_directory) = match profile {
        "system" => {
            #[cfg(target_os = "macos")]
            let pair = (
                PathBuf::from("/Library/Application Support/Gewyvern"),
                PathBuf::from("/Library/LaunchDaemons"),
            );
            #[cfg(target_os = "linux")]
            let pair = (
                PathBuf::from("/var/lib/gewyvern"),
                PathBuf::from("/etc/systemd/system"),
            );
            #[cfg(not(any(target_os = "macos", target_os = "linux")))]
            return Err(GewyvernInstallError::ServiceActivation);
            pair
        }
        "user" => {
            let home = env::var_os("HOME").ok_or(GewyvernInstallError::InvalidLayout)?;
            let home = PathBuf::from(home);
            #[cfg(target_os = "macos")]
            let pair = (
                home.join("Library/Application Support/Gewyvern"),
                home.join("Library/LaunchAgents"),
            );
            #[cfg(target_os = "linux")]
            let pair = (
                home.join(".local/share/gewyvern"),
                home.join(".config/systemd/user"),
            );
            #[cfg(not(any(target_os = "macos", target_os = "linux")))]
            return Err(GewyvernInstallError::ServiceActivation);
            pair
        }
        "test" => {
            let root = PathBuf::from(
                env::var_os("GEWYVERN_INSTALL_ROOT").ok_or(GewyvernInstallError::InvalidLayout)?,
            );
            let service_directory = root.join("services");
            (root, service_directory)
        }
        _ => return Err(GewyvernInstallError::InvalidLayout),
    };
    validate_root(&root)?;
    validate_root(&service_directory)?;
    Ok(GewyvernInstallLayout {
        root,
        service_directory,
        profile: profile.into(),
    })
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
        let service_descriptor = render_service_descriptor(destination, request, &service_plan)?;
        write_new_file(
            &stage.join(SERVICE_DESCRIPTOR_NAME),
            service_descriptor.as_bytes(),
            0o600,
        )?;
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
        listen: format!("0.0.0.0:{}", endpoint_port(&request.endpoint)?),
        executable: path(EXECUTABLE_NAME)?,
        api_token_file: path(TOKEN_NAME)?,
        tls_certificate_file: path(TLS_CERTIFICATE_NAME)?,
        tls_private_key_file: path(TLS_PRIVATE_KEY_NAME)?,
    })
}

fn endpoint_port(endpoint: &str) -> Result<u16, GewyvernInstallError> {
    let authority = endpoint
        .strip_prefix("https://")
        .ok_or(GewyvernInstallError::InvalidRequest)?;
    let value = if authority.starts_with('[') {
        authority
            .rsplit_once("]:")
            .map(|(_, port)| port)
            .unwrap_or("443")
    } else {
        authority
            .rsplit_once(':')
            .map(|(_, port)| port)
            .unwrap_or("443")
    };
    value
        .parse::<u16>()
        .ok()
        .filter(|port| *port != 0)
        .ok_or(GewyvernInstallError::InvalidRequest)
}

fn path_text(path: &Path) -> Result<String, GewyvernInstallError> {
    path.to_str()
        .filter(|value| !value.chars().any(char::is_control))
        .map(str::to_owned)
        .ok_or(GewyvernInstallError::InvalidLayout)
}

#[cfg(target_os = "macos")]
fn render_service_descriptor(
    generation: &Path,
    request: &GewyvernInstallerRequest,
    plan: &ServicePlan,
) -> Result<String, GewyvernInstallError> {
    let runtime_root = generation
        .parent()
        .and_then(Path::parent)
        .ok_or(GewyvernInstallError::InvalidLayout)?;
    let stdout = path_text(&runtime_root.join("logs/gewyvern.stdout.log"))?;
    let stderr = path_text(&runtime_root.join("logs/gewyvern.stderr.log"))?;
    let label = format!("org.gewyvern.runtime.{}", request.runtime_id.as_str());
    Ok(format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \
\"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
<plist version=\"1.0\">\n<dict>\n\
  <key>Label</key>\n  <string>{}</string>\n\
  <key>ProgramArguments</key>\n  <array>\n    <string>{}</string>\n    <string>gewyvern-service-v1</string>\n  </array>\n\
  <key>RunAtLoad</key>\n  <true/>\n\
  <key>KeepAlive</key>\n  <true/>\n\
  <key>StandardOutPath</key>\n  <string>{}</string>\n\
  <key>StandardErrorPath</key>\n  <string>{}</string>\n\
</dict>\n</plist>\n",
        xml_escape(&label),
        xml_escape(&plan.executable),
        xml_escape(&stdout),
        xml_escape(&stderr),
    ))
}

#[cfg(target_os = "linux")]
fn render_service_descriptor(
    generation: &Path,
    request: &GewyvernInstallerRequest,
    plan: &ServicePlan,
) -> Result<String, GewyvernInstallError> {
    let runtime_root = generation
        .parent()
        .and_then(Path::parent)
        .ok_or(GewyvernInstallError::InvalidLayout)?;
    let wanted_by = if request.install_profile == "system" {
        "multi-user.target"
    } else {
        "default.target"
    };
    Ok(format!(
        "[Unit]\nDescription=Gewyvern runtime ({})\nAfter=network-online.target\nWants=network-online.target\n\n\
[Service]\nType=simple\nExecStart={} gewyvern-service-v1\nRestart=on-failure\nRestartSec=2\n\
NoNewPrivileges=true\nPrivateTmp=true\nProtectSystem=strict\nReadWritePaths={}\n\n\
[Install]\nWantedBy={wanted_by}\n",
        request.runtime_id.as_str(),
        systemd_quote(&plan.executable)?,
        systemd_quote(&path_text(runtime_root)?)?,
    ))
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn render_service_descriptor(
    _generation: &Path,
    _request: &GewyvernInstallerRequest,
    _plan: &ServicePlan,
) -> Result<String, GewyvernInstallError> {
    Err(GewyvernInstallError::ServiceActivation)
}

#[cfg(target_os = "macos")]
fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(target_os = "linux")]
fn systemd_quote(value: &str) -> Result<String, GewyvernInstallError> {
    if value.is_empty() || value.chars().any(char::is_control) {
        return Err(GewyvernInstallError::InvalidLayout);
    }
    Ok(format!(
        "\"{}\"",
        value.replace('\\', "\\\\").replace('"', "\\\"")
    ))
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
    let expected_descriptor = render_service_descriptor(generation, request, &expected_plan)?;
    let descriptor = read_private_text(&generation.join(SERVICE_DESCRIPTOR_NAME), 64 * 1024)?;
    if descriptor != expected_descriptor {
        return Err(GewyvernInstallError::GenerationConflict);
    }
    require_mode(&generation.join(SERVICE_DESCRIPTOR_NAME), 0o600)?;
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

fn read_service_plan(generation: &Path) -> Result<ServicePlan, GewyvernInstallError> {
    require_directory_mode(generation, 0o700)?;
    let bytes = read_private_file(&generation.join(SERVICE_PLAN_NAME), 64 * 1024)?;
    require_mode(&generation.join(SERVICE_PLAN_NAME), 0o600)?;
    let plan = serde_json::from_slice::<ServicePlan>(&bytes)
        .map_err(|_| GewyvernInstallError::GenerationConflict)?;
    if plan.schema_version != INSTALL_MANIFEST_SCHEMA_VERSION
        || plan.runtime_id.is_empty()
        || plan.endpoint.len() > 2048
        || plan.listen.len() > 320
    {
        return Err(GewyvernInstallError::GenerationConflict);
    }
    for path in [
        &plan.executable,
        &plan.api_token_file,
        &plan.tls_certificate_file,
        &plan.tls_private_key_file,
    ] {
        let path = Path::new(path);
        if path.parent() != Some(generation) {
            return Err(GewyvernInstallError::GenerationConflict);
        }
    }
    Ok(plan)
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

fn read_optional_private_file(
    path: &Path,
    limit: u64,
) -> Result<Option<Vec<u8>>, GewyvernInstallError> {
    match fs::symlink_metadata(path) {
        Ok(_) => read_private_file(path, limit).map(Some),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(GewyvernInstallError::Storage),
    }
}

fn restore_private_file(path: &Path, previous: Option<&[u8]>) -> Result<(), GewyvernInstallError> {
    let Some(parent) = path.parent() else {
        return Err(GewyvernInstallError::InvalidLayout);
    };
    if previous.is_none() && !parent.exists() {
        return Ok(());
    }
    reject_non_regular_destination(path)?;
    match previous {
        Some(bytes) => {
            fs::create_dir_all(parent).map_err(|_| GewyvernInstallError::Storage)?;
            let temporary = parent.join(format!(
                ".restore-{}-{}",
                std::process::id(),
                unique_suffix()
            ));
            let result = (|| {
                write_new_file(&temporary, bytes, 0o600)?;
                fs::rename(&temporary, path).map_err(|_| GewyvernInstallError::Storage)?;
                sync_dir(parent)
            })();
            if result.is_err() {
                let _ = fs::remove_file(temporary);
            }
            result
        }
        None => match fs::remove_file(path) {
            Ok(()) => sync_dir(parent),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(GewyvernInstallError::Storage),
        },
    }
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
    use leserpent_domain::retirement::RetirementId;

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

    fn retirement_fixture() -> (TempDir, GewyvernRetirementRequest, GewyvernInstallLayout) {
        let temp = TempDir::new("retirement");
        let source = temp.0.join("gewyvern-source");
        fs::write(&source, b"native-gewyvern-artifact").unwrap();
        #[cfg(unix)]
        fs::set_permissions(&source, fs::Permissions::from_mode(0o700)).unwrap();
        let artifact_sha256 = hex(digest(&SHA256, b"native-gewyvern-artifact").as_ref());
        let install = GewyvernInstallerRequest::new(
            ProvisioningId::new("provision-retire-1").unwrap(),
            RuntimeId::new("runtime-retire-1").unwrap(),
            "https://runtime.example:9443",
            "user",
            artifact_sha256,
            CredentialHandle::new("vault:gewyvern:runtime-api").unwrap(),
            CredentialHandle::new("vault:gewyvern-ca:runtime-ca").unwrap(),
            "0123456789abcdef0123456789abcdef",
        )
        .unwrap();
        let layout = GewyvernInstallLayout {
            root: temp.0.join("install"),
            service_directory: temp.0.join("services"),
            profile: "user".into(),
        };
        let response = install_gewyvern_artifact(&source, &install, &layout).unwrap();
        let generation = layout
            .runtime_root(&install)
            .join("generations")
            .join(response.generation);
        publish_service_descriptor(&generation, &install, &layout).unwrap();
        let retirement = GewyvernRetirementRequest {
            schema_version: GEWYVERN_RETIREMENT_SCHEMA_VERSION,
            retirement_id: RetirementId::new("retire-1").unwrap(),
            provisioning_id: install.provisioning_id.clone(),
            runtime_id: install.runtime_id.clone(),
            install_profile: "user".into(),
        };
        (temp, retirement, layout)
    }

    #[test]
    fn retirement_is_identity_bound_private_idempotent_and_runtime_scoped() {
        let (_temp, request, layout) = retirement_fixture();
        let other_runtime = layout.root.join("runtimes/runtime-other");
        create_private_dir(&other_runtime).unwrap();
        let calls = std::cell::Cell::new(0_u32);
        let first = retire_gewyvern_with(&request, &layout, || {
            calls.set(calls.get() + 1);
            Ok(())
        })
        .unwrap();
        assert!(first.service_retired);
        assert!(!first.replayed);
        assert_eq!(calls.get(), 1);
        assert!(
            !layout
                .runtime_root_for(request.runtime_id.as_str())
                .exists()
        );
        assert!(other_runtime.exists());
        assert!(!published_service_path_for(request.runtime_id.as_str(), &layout).exists());
        let marker = read_retirement_marker(
            &layout
                .root
                .join("retirements")
                .join(format!("{}.json", request.runtime_id.as_str())),
        )
        .unwrap()
        .unwrap();
        assert_eq!(marker.phase, RetirementMarkerPhase::Retired);

        let replay = retire_gewyvern_with(&request, &layout, || {
            calls.set(calls.get() + 1);
            Ok(())
        })
        .unwrap();
        assert!(replay.replayed);
        assert_eq!(calls.get(), 1);
    }

    #[test]
    fn retirement_failure_is_restart_safe_and_identity_confusion_is_rejected() {
        let (_temp, request, layout) = retirement_fixture();
        let mut confused = request.clone();
        confused.provisioning_id = ProvisioningId::new("provision-other").unwrap();
        assert_eq!(
            retire_gewyvern_with(&confused, &layout, || Ok(())).unwrap_err(),
            GewyvernInstallError::GenerationConflict
        );
        assert!(
            layout
                .runtime_root_for(request.runtime_id.as_str())
                .exists()
        );

        assert_eq!(
            retire_gewyvern_with(&request, &layout, || {
                Err(GewyvernInstallError::ServiceActivation)
            })
            .unwrap_err(),
            GewyvernInstallError::ServiceActivation
        );
        assert!(
            layout
                .runtime_root_for(request.runtime_id.as_str())
                .exists()
        );
        assert!(published_service_path_for(request.runtime_id.as_str(), &layout).exists());
        let marker_path = layout
            .root
            .join("retirements")
            .join(format!("{}.json", request.runtime_id.as_str()));
        assert_eq!(
            read_retirement_marker(&marker_path).unwrap().unwrap().phase,
            RetirementMarkerPhase::Retiring
        );

        let resumed = retire_gewyvern_with(&request, &layout, || Ok(())).unwrap();
        assert!(resumed.replayed);
        assert!(
            !layout
                .runtime_root_for(request.runtime_id.as_str())
                .exists()
        );
        assert_eq!(
            read_retirement_marker(&marker_path).unwrap().unwrap().phase,
            RetirementMarkerPhase::Retired
        );
    }

    #[cfg(unix)]
    #[test]
    fn retirement_rejects_relaxed_authority_files_before_stopping_service() {
        let (_temp, request, layout) = retirement_fixture();
        let runtime_root = layout.runtime_root_for(request.runtime_id.as_str());
        let generation = fs::read_to_string(runtime_root.join(CURRENT_NAME))
            .unwrap()
            .trim()
            .to_string();
        fs::set_permissions(
            runtime_root
                .join("generations")
                .join(generation)
                .join(MANIFEST_NAME),
            fs::Permissions::from_mode(0o644),
        )
        .unwrap();
        let called = std::cell::Cell::new(false);
        assert_eq!(
            retire_gewyvern_with(&request, &layout, || {
                called.set(true);
                Ok(())
            })
            .unwrap_err(),
            GewyvernInstallError::GenerationConflict
        );
        assert!(!called.get());
        assert!(
            !layout
                .root
                .join("retirements")
                .join(format!("{}.json", request.runtime_id.as_str()))
                .exists()
        );
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
        let descriptor = fs::read_to_string(generation.join(SERVICE_DESCRIPTOR_NAME)).unwrap();
        assert!(descriptor.contains("gewyvern-service-v1"));
        assert!(!descriptor.contains(request.api_token()));
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
    fn ready_requires_service_activation_and_authenticated_health() {
        let (_temp, source, request, layout) = fixture();
        let response = activate_gewyvern_artifact_with(
            &source,
            &request,
            &layout,
            || Ok(()),
            |ca_pem| {
                assert!(ca_pem.starts_with("-----BEGIN CERTIFICATE-----\n"));
                Ok(())
            },
            |_| panic!("successful activation must not roll back"),
        )
        .unwrap();

        assert_eq!(response.service_state, GewyvernInstallerServiceState::Ready);
        assert!(published_service_path(&request, &layout).is_file());
        assert_eq!(
            fs::read_to_string(layout.runtime_root(&request).join(CURRENT_NAME)).unwrap(),
            format!("{}\n", response.generation)
        );
    }

    #[test]
    fn health_failure_restores_current_descriptor_and_new_generation() {
        let (_temp, source, request, layout) = fixture();
        let mut rolled_back = false;
        let result = activate_gewyvern_artifact_with(
            &source,
            &request,
            &layout,
            || Ok(()),
            |_| Err(GewyvernInstallError::HealthProof),
            |had_previous| {
                assert!(!had_previous);
                rolled_back = true;
                Ok(())
            },
        );

        assert_eq!(result, Err(GewyvernInstallError::HealthProof));
        assert!(rolled_back);
        assert!(!layout.runtime_root(&request).join(CURRENT_NAME).exists());
        assert!(!published_service_path(&request, &layout).exists());
        assert!(
            !layout
                .runtime_root(&request)
                .join("generations")
                .join(&request.artifact_sha256)
                .exists()
        );
    }

    #[test]
    fn failed_upgrade_restores_the_previous_generation_and_descriptor() {
        let (temp, source, request, layout) = fixture();
        let first = activate_gewyvern_artifact_with(
            &source,
            &request,
            &layout,
            || Ok(()),
            |_| Ok(()),
            |_| panic!("successful activation must not roll back"),
        )
        .unwrap();
        let runtime_root = layout.runtime_root(&request);
        let previous_current = fs::read(runtime_root.join(CURRENT_NAME)).unwrap();
        let previous_descriptor = fs::read(published_service_path(&request, &layout)).unwrap();

        let replacement_source = temp.0.join("gewyvern-replacement");
        fs::write(&replacement_source, b"replacement-gewyvern-artifact").unwrap();
        #[cfg(unix)]
        fs::set_permissions(&replacement_source, fs::Permissions::from_mode(0o700)).unwrap();
        let replacement_digest = hex(digest(&SHA256, b"replacement-gewyvern-artifact").as_ref());
        let replacement = GewyvernInstallerRequest::new(
            request.provisioning_id.clone(),
            request.runtime_id.clone(),
            request.endpoint.clone(),
            request.install_profile.clone(),
            replacement_digest.clone(),
            request.api_credential_handle.clone(),
            request.trust_credential_handle.clone(),
            request.api_token(),
        )
        .unwrap();
        let mut rolled_back = false;

        let result = activate_gewyvern_artifact_with(
            &replacement_source,
            &replacement,
            &layout,
            || Ok(()),
            |_| Err(GewyvernInstallError::HealthProof),
            |had_previous| {
                assert!(had_previous);
                rolled_back = true;
                Ok(())
            },
        );

        assert_eq!(result, Err(GewyvernInstallError::HealthProof));
        assert!(rolled_back);
        assert_eq!(
            fs::read(runtime_root.join(CURRENT_NAME)).unwrap(),
            previous_current
        );
        assert_eq!(
            fs::read(published_service_path(&request, &layout)).unwrap(),
            previous_descriptor
        );
        assert!(
            runtime_root
                .join("generations")
                .join(first.generation)
                .is_dir()
        );
        assert!(
            !runtime_root
                .join("generations")
                .join(replacement_digest)
                .exists()
        );
    }

    #[test]
    fn service_rollback_is_attempted_even_when_state_restore_fails() {
        let (_temp, source, request, layout) = fixture();
        let current = layout.runtime_root(&request).join(CURRENT_NAME);
        let mut service_rollback_attempted = false;

        let result = activate_gewyvern_artifact_with(
            &source,
            &request,
            &layout,
            || Ok(()),
            |_| {
                fs::remove_file(&current).unwrap();
                fs::create_dir(&current).unwrap();
                Err(GewyvernInstallError::HealthProof)
            },
            |_| {
                service_rollback_attempted = true;
                Ok(())
            },
        );

        assert_eq!(result, Err(GewyvernInstallError::InvalidLayout));
        assert!(service_rollback_attempted);
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
