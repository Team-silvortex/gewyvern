use std::env;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use leserpent_protocol::bootstrap_installer::{
    BOOTSTRAP_INSTALLER_SCHEMA_VERSION, BootstrapInstallerRequest, BootstrapInstallerResponse,
    BootstrapInstallerServiceState, MAX_BOOTSTRAP_INSTALLER_BYTES,
    decode_bootstrap_installer_request, encode_bootstrap_installer_response,
};
use rcgen::{CertifiedKey, generate_simple_self_signed};
use ring::digest::{Context, SHA256, digest};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

const MAX_INSTALL_ARTIFACT_BYTES: u64 = 64 * 1024 * 1024;
const EXECUTABLE_NAME: &str = "leserpentd";
const TOKEN_NAME: &str = "session.token";
const MANIFEST_NAME: &str = "install.json";
const CURRENT_NAME: &str = "current";
const TLS_CERTIFICATE_NAME: &str = "server.crt";
const TLS_PRIVATE_KEY_NAME: &str = "server.key";
#[cfg(target_os = "macos")]
const SERVICE_DESCRIPTOR_NAME: &str = "service.plist";
#[cfg(target_os = "linux")]
const SERVICE_DESCRIPTOR_NAME: &str = "service.service";
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
const SERVICE_DESCRIPTOR_NAME: &str = "service.conf";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BootstrapInstallLayout {
    root: PathBuf,
    profile: String,
}

impl BootstrapInstallLayout {
    pub fn new(root: impl Into<PathBuf>, profile: impl Into<String>) -> Result<Self, String> {
        let root = root.into();
        let profile = profile.into();
        let valid_path = root.is_absolute()
            && root != Path::new("/")
            && root
                .components()
                .all(|component| !matches!(component, Component::ParentDir | Component::CurDir));
        if !valid_path || !matches!(profile.as_str(), "system" | "user" | "test") {
            return Err("invalid bootstrap install layout".into());
        }
        Ok(Self { root, profile })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn profile(&self) -> &str {
        &self.profile
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BootstrapInstallError {
    InvalidRequest,
    UnsupportedProfile,
    InvalidLayout,
    InvalidArtifact,
    ArtifactDigestMismatch,
    GenerationConflict,
    TlsIdentity,
    Storage,
    ResponseEncoding,
}

impl fmt::Display for BootstrapInstallError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidRequest => "invalid bootstrap installer request",
            Self::UnsupportedProfile => "bootstrap install profile is unsupported on this host",
            Self::InvalidLayout => "bootstrap install layout is unsafe",
            Self::InvalidArtifact => "bootstrap installer artifact is invalid",
            Self::ArtifactDigestMismatch => "bootstrap installer artifact digest mismatch",
            Self::GenerationConflict => {
                "bootstrap installer generation conflicts with retained state"
            }
            Self::TlsIdentity => "bootstrap installer TLS identity is invalid",
            Self::Storage => "bootstrap installer storage operation failed",
            Self::ResponseEncoding => "bootstrap installer response encoding failed",
        })
    }
}

impl std::error::Error for BootstrapInstallError {}

#[derive(Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct InstallManifest {
    schema_version: u32,
    bootstrap_id: String,
    daemon_id: String,
    endpoint: String,
    artifact_sha256: String,
    generation: String,
}

pub fn install_bootstrap_artifact(
    source: &Path,
    request: &BootstrapInstallerRequest,
    layout: &BootstrapInstallLayout,
) -> Result<BootstrapInstallerResponse, BootstrapInstallError> {
    request
        .validate()
        .map_err(|_| BootstrapInstallError::InvalidRequest)?;
    if request.install_profile != layout.profile {
        return Err(BootstrapInstallError::InvalidLayout);
    }
    let artifact = read_regular_bounded(source)?;
    let artifact_sha256 = hex(digest(&SHA256, &artifact).as_ref());
    if artifact_sha256 != request.artifact_sha256 {
        return Err(BootstrapInstallError::ArtifactDigestMismatch);
    }
    let generation = generation_id(request);
    prepare_layout(layout)?;
    let generations = layout.root.join("generations");
    let destination = generations.join(&generation);
    let manifest = InstallManifest {
        schema_version: BOOTSTRAP_INSTALLER_SCHEMA_VERSION,
        bootstrap_id: request.bootstrap_id.as_str().into(),
        daemon_id: request.daemon_id.as_str().into(),
        endpoint: request.endpoint.clone(),
        artifact_sha256,
        generation: generation.clone(),
    };

    let replayed = if destination.exists() {
        verify_generation(&destination, request, layout, &manifest)?;
        true
    } else {
        install_generation(
            &generations,
            &destination,
            &artifact,
            request,
            layout,
            &manifest,
        )?;
        false
    };
    commit_current(&layout.root, &generation)?;
    let (tls_ca_pem, tls_ca_sha256) = read_tls_identity(&destination)?;

    Ok(BootstrapInstallerResponse {
        schema_version: BOOTSTRAP_INSTALLER_SCHEMA_VERSION,
        bootstrap_id: request.bootstrap_id.clone(),
        daemon_id: request.daemon_id.clone(),
        endpoint: request.endpoint.clone(),
        service_state: BootstrapInstallerServiceState::Installed,
        generation,
        replayed,
        tls_ca_pem,
        tls_ca_sha256,
    })
}

pub fn run_bootstrap_install_stdio() -> Result<(), BootstrapInstallError> {
    let request = read_stdio_request(std::io::stdin().lock())?;
    let layout = platform_layout(&request.install_profile)?;
    let source = env::current_exe().map_err(|_| BootstrapInstallError::InvalidArtifact)?;
    let response = install_bootstrap_artifact(&source, &request, &layout)?;
    let encoded = encode_bootstrap_installer_response(&response)
        .map_err(|_| BootstrapInstallError::ResponseEncoding)?;
    let mut stdout = std::io::stdout().lock();
    stdout
        .write_all(&encoded)
        .and_then(|_| stdout.write_all(b"\n"))
        .and_then(|_| stdout.flush())
        .map_err(|_| BootstrapInstallError::Storage)
}

fn read_stdio_request(
    mut reader: impl Read,
) -> Result<BootstrapInstallerRequest, BootstrapInstallError> {
    let mut bytes = Zeroizing::new(Vec::new());
    reader
        .by_ref()
        .take((MAX_BOOTSTRAP_INSTALLER_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| BootstrapInstallError::InvalidRequest)?;
    if bytes.len() > MAX_BOOTSTRAP_INSTALLER_BYTES {
        return Err(BootstrapInstallError::InvalidRequest);
    }
    decode_bootstrap_installer_request(&bytes).map_err(|_| BootstrapInstallError::InvalidRequest)
}

fn platform_layout(profile: &str) -> Result<BootstrapInstallLayout, BootstrapInstallError> {
    match profile {
        "system" => {
            #[cfg(target_os = "macos")]
            let root = PathBuf::from("/Library/Application Support/Leserpent/bootstrap");
            #[cfg(target_os = "linux")]
            let root = PathBuf::from("/var/lib/leserpent/bootstrap");
            #[cfg(not(any(target_os = "macos", target_os = "linux")))]
            return Err(BootstrapInstallError::UnsupportedProfile);
            BootstrapInstallLayout::new(root, profile)
                .map_err(|_| BootstrapInstallError::InvalidLayout)
        }
        "user" => {
            let home = env::var_os("HOME")
                .map(PathBuf::from)
                .filter(|path| path.is_absolute())
                .ok_or(BootstrapInstallError::InvalidLayout)?;
            #[cfg(target_os = "macos")]
            let root = home.join("Library/Application Support/Leserpent/bootstrap");
            #[cfg(target_os = "linux")]
            let root = home.join(".local/share/leserpent/bootstrap");
            #[cfg(not(any(target_os = "macos", target_os = "linux")))]
            return Err(BootstrapInstallError::UnsupportedProfile);
            BootstrapInstallLayout::new(root, profile)
                .map_err(|_| BootstrapInstallError::InvalidLayout)
        }
        _ => Err(BootstrapInstallError::UnsupportedProfile),
    }
}

fn prepare_layout(layout: &BootstrapInstallLayout) -> Result<(), BootstrapInstallError> {
    reject_existing_symlink_components(&layout.root)?;
    create_private_dir(&layout.root)?;
    create_private_dir(&layout.root.join("generations"))?;
    create_private_dir(&layout.root.join("state"))?;
    create_private_dir(&layout.root.join("logs"))?;
    Ok(())
}

fn reject_existing_symlink_components(path: &Path) -> Result<(), BootstrapInstallError> {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(BootstrapInstallError::InvalidLayout);
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(BootstrapInstallError::Storage),
        }
    }
    Ok(())
}

fn create_private_dir(path: &Path) -> Result<(), BootstrapInstallError> {
    fs::create_dir_all(path).map_err(|_| BootstrapInstallError::Storage)?;
    let metadata = fs::symlink_metadata(path).map_err(|_| BootstrapInstallError::Storage)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(BootstrapInstallError::InvalidLayout);
    }
    set_mode(path, 0o700)
}

fn install_generation(
    generations: &Path,
    destination: &Path,
    artifact: &[u8],
    request: &BootstrapInstallerRequest,
    layout: &BootstrapInstallLayout,
    manifest: &InstallManifest,
) -> Result<(), BootstrapInstallError> {
    let stage = generations.join(format!(
        ".stage-{}-{}-{}",
        request.bootstrap_id.as_str(),
        std::process::id(),
        unique_suffix()
    ));
    fs::create_dir(&stage).map_err(|_| BootstrapInstallError::Storage)?;
    set_mode(&stage, 0o700)?;
    let result = (|| {
        write_new_file(&stage.join(EXECUTABLE_NAME), artifact, 0o700)?;
        write_new_file(
            &stage.join(TOKEN_NAME),
            request.session_token().as_bytes(),
            0o600,
        )?;
        let manifest_bytes =
            serde_json::to_vec(manifest).map_err(|_| BootstrapInstallError::Storage)?;
        write_new_file(&stage.join(MANIFEST_NAME), &manifest_bytes, 0o600)?;
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
        let service_descriptor = render_service_descriptor(layout, destination, request)?;
        write_new_file(
            &stage.join(SERVICE_DESCRIPTOR_NAME),
            service_descriptor.as_bytes(),
            0o600,
        )?;
        sync_directory(&stage)?;
        fs::rename(&stage, destination).map_err(|_| BootstrapInstallError::Storage)?;
        sync_directory(generations)
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&stage);
    }
    result
}

fn verify_generation(
    destination: &Path,
    request: &BootstrapInstallerRequest,
    layout: &BootstrapInstallLayout,
    expected_manifest: &InstallManifest,
) -> Result<(), BootstrapInstallError> {
    require_directory_mode(destination, 0o700)?;
    let executable_path = destination.join(EXECUTABLE_NAME);
    require_mode(&executable_path, 0o700)?;
    let executable = read_regular_bounded(&executable_path)?;
    if hex(digest(&SHA256, &executable).as_ref()) != request.artifact_sha256 {
        return Err(BootstrapInstallError::GenerationConflict);
    }
    let token = read_private_file(&destination.join(TOKEN_NAME), 256)?;
    if token.as_slice() != request.session_token().as_bytes() {
        return Err(BootstrapInstallError::GenerationConflict);
    }
    let manifest = read_private_file(&destination.join(MANIFEST_NAME), 4096)?;
    let actual: InstallManifest =
        serde_json::from_slice(&manifest).map_err(|_| BootstrapInstallError::GenerationConflict)?;
    if &actual != expected_manifest {
        return Err(BootstrapInstallError::GenerationConflict);
    }
    read_tls_identity(destination)?;
    let descriptor = read_private_file(&destination.join(SERVICE_DESCRIPTOR_NAME), 64 * 1024)?;
    let expected_descriptor = render_service_descriptor(layout, destination, request)?;
    if descriptor.as_slice() != expected_descriptor.as_bytes() {
        return Err(BootstrapInstallError::GenerationConflict);
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn render_service_descriptor(
    layout: &BootstrapInstallLayout,
    destination: &Path,
    request: &BootstrapInstallerRequest,
) -> Result<String, BootstrapInstallError> {
    let executable = path_text(&destination.join(EXECUTABLE_NAME))?;
    let database = path_text(
        &layout
            .root
            .join("state")
            .join(format!("{}.sqlite", request.daemon_id.as_str())),
    )?;
    let certificate = path_text(&destination.join(TLS_CERTIFICATE_NAME))?;
    let private_key = path_text(&destination.join(TLS_PRIVATE_KEY_NAME))?;
    let token = path_text(&destination.join(TOKEN_NAME))?;
    let stdout = path_text(&layout.root.join("logs/leserpentd.stdout.log"))?;
    let stderr = path_text(&layout.root.join("logs/leserpentd.stderr.log"))?;
    let listen = format!("0.0.0.0:{}", endpoint_port(&request.endpoint)?);
    let label = format!("org.gewyvern.leserpentd.{}", request.daemon_id.as_str());
    let arguments = [
        executable.as_str(),
        "--database",
        database.as_str(),
        "--remote-listen",
        listen.as_str(),
        "--remote-cert",
        certificate.as_str(),
        "--remote-key",
        private_key.as_str(),
        "--remote-token-file",
        token.as_str(),
    ];
    let arguments = arguments
        .iter()
        .map(|argument| format!("    <string>{}</string>\n", xml_escape(argument)))
        .collect::<String>();
    Ok(format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \
\"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
<plist version=\"1.0\">\n<dict>\n\
  <key>Label</key>\n  <string>{}</string>\n\
  <key>ProgramArguments</key>\n  <array>\n{arguments}  </array>\n\
  <key>RunAtLoad</key>\n  <true/>\n\
  <key>KeepAlive</key>\n  <true/>\n\
  <key>StandardOutPath</key>\n  <string>{}</string>\n\
  <key>StandardErrorPath</key>\n  <string>{}</string>\n\
</dict>\n</plist>\n",
        xml_escape(&label),
        xml_escape(&stdout),
        xml_escape(&stderr),
    ))
}

#[cfg(target_os = "linux")]
fn render_service_descriptor(
    layout: &BootstrapInstallLayout,
    destination: &Path,
    request: &BootstrapInstallerRequest,
) -> Result<String, BootstrapInstallError> {
    let executable = path_text(&destination.join(EXECUTABLE_NAME))?;
    let database = path_text(
        &layout
            .root
            .join("state")
            .join(format!("{}.sqlite", request.daemon_id.as_str())),
    )?;
    let certificate = path_text(&destination.join(TLS_CERTIFICATE_NAME))?;
    let private_key = path_text(&destination.join(TLS_PRIVATE_KEY_NAME))?;
    let token = path_text(&destination.join(TOKEN_NAME))?;
    let writable_root = path_text(&layout.root)?;
    let listen = format!("0.0.0.0:{}", endpoint_port(&request.endpoint)?);
    let arguments = [
        executable.as_str(),
        "--database",
        database.as_str(),
        "--remote-listen",
        listen.as_str(),
        "--remote-cert",
        certificate.as_str(),
        "--remote-key",
        private_key.as_str(),
        "--remote-token-file",
        token.as_str(),
    ];
    let command = arguments
        .iter()
        .map(|argument| systemd_quote(argument))
        .collect::<Result<Vec<_>, _>>()?
        .join(" ");
    Ok(format!(
        "[Unit]\nDescription=Leserpent orchestra daemon ({})\nAfter=network-online.target\nWants=network-online.target\n\n\
[Service]\nType=simple\nExecStart={command}\nRestart=on-failure\nRestartSec=2\n\
NoNewPrivileges=true\nPrivateTmp=true\nProtectSystem=strict\nReadWritePaths={}\n\n\
[Install]\nWantedBy=default.target\n",
        request.daemon_id.as_str(),
        systemd_quote(&writable_root)?,
    ))
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn render_service_descriptor(
    _layout: &BootstrapInstallLayout,
    _destination: &Path,
    _request: &BootstrapInstallerRequest,
) -> Result<String, BootstrapInstallError> {
    Err(BootstrapInstallError::UnsupportedProfile)
}

fn path_text(path: &Path) -> Result<String, BootstrapInstallError> {
    path.to_str()
        .filter(|value| !value.chars().any(char::is_control))
        .map(str::to_owned)
        .ok_or(BootstrapInstallError::InvalidLayout)
}

fn endpoint_port(endpoint: &str) -> Result<u16, BootstrapInstallError> {
    let authority = endpoint
        .strip_prefix("https://")
        .ok_or(BootstrapInstallError::InvalidRequest)?;
    let port = if authority.starts_with('[') {
        authority
            .rsplit_once("]:")
            .map(|(_, port)| port)
            .ok_or(BootstrapInstallError::InvalidRequest)?
    } else {
        authority
            .rsplit_once(':')
            .map(|(_, port)| port)
            .unwrap_or("443")
    };
    port.parse()
        .map_err(|_| BootstrapInstallError::InvalidRequest)
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
fn systemd_quote(value: &str) -> Result<String, BootstrapInstallError> {
    if value.chars().any(char::is_control) {
        return Err(BootstrapInstallError::InvalidLayout);
    }
    Ok(format!(
        "\"{}\"",
        value
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('%', "%%")
    ))
}

fn generate_tls_identity(
    endpoint: &str,
) -> Result<(String, Zeroizing<String>), BootstrapInstallError> {
    let host = endpoint_host(endpoint).ok_or(BootstrapInstallError::TlsIdentity)?;
    let CertifiedKey { cert, signing_key } =
        generate_simple_self_signed(vec![host]).map_err(|_| BootstrapInstallError::TlsIdentity)?;
    let certificate = cert.pem();
    let private_key = Zeroizing::new(signing_key.serialize_pem());
    validate_tls_pair(certificate.as_bytes(), private_key.as_bytes())?;
    Ok((certificate, private_key))
}

fn read_tls_identity(destination: &Path) -> Result<(String, String), BootstrapInstallError> {
    let certificate = read_private_file(&destination.join(TLS_CERTIFICATE_NAME), 32 * 1024)?;
    let private_key = read_private_file(&destination.join(TLS_PRIVATE_KEY_NAME), 64 * 1024)?;
    validate_tls_pair(&certificate, &private_key)?;
    let certificate =
        String::from_utf8(certificate.to_vec()).map_err(|_| BootstrapInstallError::TlsIdentity)?;
    let certificate_sha256 = hex(digest(&SHA256, certificate.as_bytes()).as_ref());
    Ok((certificate, certificate_sha256))
}

fn validate_tls_pair(certificate: &[u8], private_key: &[u8]) -> Result<(), BootstrapInstallError> {
    let certificates = CertificateDer::pem_slice_iter(certificate)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| BootstrapInstallError::TlsIdentity)?;
    if certificates.is_empty() {
        return Err(BootstrapInstallError::TlsIdentity);
    }
    let private_key = PrivateKeyDer::from_pem_slice(private_key)
        .map_err(|_| BootstrapInstallError::TlsIdentity)?;
    let provider = std::sync::Arc::new(rustls::crypto::ring::default_provider());
    rustls::ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|_| BootstrapInstallError::TlsIdentity)?
        .with_no_client_auth()
        .with_single_cert(certificates, private_key)
        .map_err(|_| BootstrapInstallError::TlsIdentity)?;
    Ok(())
}

fn endpoint_host(endpoint: &str) -> Option<String> {
    let authority = endpoint.strip_prefix("https://")?;
    if let Some(bracketed) = authority.strip_prefix('[') {
        let (host, suffix) = bracketed.split_once(']')?;
        if host.is_empty() || !(suffix.is_empty() || suffix.starts_with(':')) {
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

fn commit_current(root: &Path, generation: &str) -> Result<(), BootstrapInstallError> {
    let temporary = root.join(format!(
        ".current-{}-{}",
        std::process::id(),
        unique_suffix()
    ));
    let result = (|| {
        write_new_file(&temporary, format!("{generation}\n").as_bytes(), 0o600)?;
        fs::rename(&temporary, root.join(CURRENT_NAME))
            .map_err(|_| BootstrapInstallError::Storage)?;
        sync_directory(root)
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}

fn read_regular_bounded(path: &Path) -> Result<Vec<u8>, BootstrapInstallError> {
    let mut file = File::open(path).map_err(|_| BootstrapInstallError::InvalidArtifact)?;
    let metadata = file
        .metadata()
        .map_err(|_| BootstrapInstallError::InvalidArtifact)?;
    let path_metadata =
        fs::symlink_metadata(path).map_err(|_| BootstrapInstallError::InvalidArtifact)?;
    if !metadata.is_file()
        || path_metadata.file_type().is_symlink()
        || !same_file_identity(&metadata, &path_metadata)
        || metadata.len() == 0
        || metadata.len() > MAX_INSTALL_ARTIFACT_BYTES
    {
        return Err(BootstrapInstallError::InvalidArtifact);
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.read_to_end(&mut bytes)
        .map_err(|_| BootstrapInstallError::InvalidArtifact)?;
    if bytes.len() as u64 != metadata.len() {
        return Err(BootstrapInstallError::InvalidArtifact);
    }
    Ok(bytes)
}

fn read_private_file(path: &Path, limit: u64) -> Result<Zeroizing<Vec<u8>>, BootstrapInstallError> {
    require_mode(path, 0o600)?;
    let mut file = File::open(path).map_err(|_| BootstrapInstallError::GenerationConflict)?;
    let metadata = file
        .metadata()
        .map_err(|_| BootstrapInstallError::GenerationConflict)?;
    if !metadata.is_file() || metadata.len() > limit {
        return Err(BootstrapInstallError::GenerationConflict);
    }
    let mut bytes = Zeroizing::new(Vec::with_capacity(metadata.len() as usize));
    file.read_to_end(&mut bytes)
        .map_err(|_| BootstrapInstallError::GenerationConflict)?;
    Ok(bytes)
}

fn write_new_file(path: &Path, bytes: &[u8], mode: u32) -> Result<(), BootstrapInstallError> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    set_open_mode(&mut options, mode);
    let mut file = options
        .open(path)
        .map_err(|_| BootstrapInstallError::Storage)?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| BootstrapInstallError::Storage)?;
    require_mode(path, mode)
}

fn generation_id(request: &BootstrapInstallerRequest) -> String {
    let mut context = Context::new(&SHA256);
    context.update(request.artifact_sha256.as_bytes());
    context.update(&[0]);
    context.update(request.daemon_id.as_str().as_bytes());
    context.update(&[0]);
    context.update(request.endpoint.as_bytes());
    hex(context.finish().as_ref())
}

fn unique_suffix() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos())
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
fn set_mode(path: &Path, mode: u32) -> Result<(), BootstrapInstallError> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
        .map_err(|_| BootstrapInstallError::Storage)
}

#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: u32) -> Result<(), BootstrapInstallError> {
    Err(BootstrapInstallError::UnsupportedProfile)
}

#[cfg(unix)]
fn require_mode(path: &Path, expected: u32) -> Result<(), BootstrapInstallError> {
    use std::os::unix::fs::PermissionsExt;
    let metadata = fs::symlink_metadata(path).map_err(|_| BootstrapInstallError::Storage)?;
    if metadata.file_type().is_symlink() || metadata.permissions().mode() & 0o777 != expected {
        return Err(BootstrapInstallError::GenerationConflict);
    }
    Ok(())
}

#[cfg(unix)]
fn require_directory_mode(path: &Path, expected: u32) -> Result<(), BootstrapInstallError> {
    use std::os::unix::fs::PermissionsExt;
    let metadata = fs::symlink_metadata(path).map_err(|_| BootstrapInstallError::Storage)?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.permissions().mode() & 0o777 != expected
    {
        return Err(BootstrapInstallError::GenerationConflict);
    }
    Ok(())
}

#[cfg(not(unix))]
fn require_directory_mode(_path: &Path, _expected: u32) -> Result<(), BootstrapInstallError> {
    Err(BootstrapInstallError::UnsupportedProfile)
}

#[cfg(unix)]
fn same_file_identity(opened: &fs::Metadata, path: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    opened.dev() == path.dev() && opened.ino() == path.ino()
}

#[cfg(not(unix))]
fn same_file_identity(_opened: &fs::Metadata, _path: &fs::Metadata) -> bool {
    true
}

#[cfg(not(unix))]
fn require_mode(_path: &Path, _expected: u32) -> Result<(), BootstrapInstallError> {
    Err(BootstrapInstallError::UnsupportedProfile)
}

fn sync_directory(path: &Path) -> Result<(), BootstrapInstallError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|_| BootstrapInstallError::Storage)
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::{PermissionsExt, symlink};

    use leserpent_domain::bootstrap::{BootstrapId, DaemonId};

    use super::*;

    struct TempTree(PathBuf);

    impl TempTree {
        fn new(label: &str) -> Self {
            let path = env::temp_dir().join(format!(
                "leserpent-bootstrap-{label}-{}-{}",
                std::process::id(),
                unique_suffix()
            ));
            fs::create_dir(&path).unwrap();
            Self(fs::canonicalize(path).unwrap())
        }
    }

    impl Drop for TempTree {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn fixture() -> (
        TempTree,
        PathBuf,
        BootstrapInstallerRequest,
        BootstrapInstallLayout,
    ) {
        let temp = TempTree::new("install");
        let source = temp.0.join("source-leserpentd");
        fs::write(&source, b"native-leserpentd-fixture").unwrap();
        fs::set_permissions(&source, fs::Permissions::from_mode(0o700)).unwrap();
        let artifact_sha256 = hex(digest(&SHA256, b"native-leserpentd-fixture").as_ref());
        let request = BootstrapInstallerRequest::new(
            BootstrapId::new("bootstrap-1").unwrap(),
            DaemonId::new("daemon-1").unwrap(),
            "https://host.example:7443",
            "test",
            artifact_sha256,
            "0123456789abcdef0123456789abcdef",
        )
        .unwrap();
        let layout = BootstrapInstallLayout::new(temp.0.join("root"), "test").unwrap();
        (temp, source, request, layout)
    }

    #[test]
    fn generation_install_is_atomic_private_and_idempotent() {
        let (_temp, source, request, layout) = fixture();
        let first = install_bootstrap_artifact(&source, &request, &layout).unwrap();
        assert_eq!(
            first.service_state,
            BootstrapInstallerServiceState::Installed
        );
        assert!(!first.replayed);
        let generation = layout.root.join("generations").join(&first.generation);
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
        let descriptor = fs::read_to_string(generation.join(SERVICE_DESCRIPTOR_NAME)).unwrap();
        assert!(descriptor.contains("--remote-token-file"));
        assert!(!descriptor.contains(request.session_token()));
        assert!(descriptor.contains("--remote-cert"));
        assert!(descriptor.contains("--remote-key"));
        assert_eq!(
            fs::read_to_string(layout.root.join(CURRENT_NAME)).unwrap(),
            format!("{}\n", first.generation)
        );

        let replay = install_bootstrap_artifact(&source, &request, &layout).unwrap();
        assert!(replay.replayed);
        assert_eq!(replay.generation, first.generation);
    }

    #[test]
    fn digest_or_retained_token_conflict_preserves_current_generation() {
        let (_temp, source, mut request, layout) = fixture();
        let expected_digest = request.artifact_sha256.clone();
        request.artifact_sha256 = "0".repeat(64);
        assert_eq!(
            install_bootstrap_artifact(&source, &request, &layout),
            Err(BootstrapInstallError::ArtifactDigestMismatch)
        );

        request.artifact_sha256 = expected_digest;
        let installed = install_bootstrap_artifact(&source, &request, &layout).unwrap();
        let current = fs::read(layout.root.join(CURRENT_NAME)).unwrap();
        let replacement = BootstrapInstallerRequest::new(
            request.bootstrap_id.clone(),
            request.daemon_id.clone(),
            request.endpoint.clone(),
            "test",
            request.artifact_sha256.clone(),
            "fedcba9876543210fedcba9876543210",
        )
        .unwrap();
        assert_eq!(
            install_bootstrap_artifact(&source, &replacement, &layout),
            Err(BootstrapInstallError::GenerationConflict)
        );
        assert_eq!(fs::read(layout.root.join(CURRENT_NAME)).unwrap(), current);
        assert!(!installed.replayed);
    }

    #[test]
    fn symlinked_layout_and_source_are_rejected_before_commit() {
        let (temp, source, request, layout) = fixture();
        let linked_source = temp.0.join("linked-source");
        symlink(&source, &linked_source).unwrap();
        assert_eq!(
            install_bootstrap_artifact(&linked_source, &request, &layout),
            Err(BootstrapInstallError::InvalidArtifact)
        );

        let real_root = temp.0.join("real-root");
        fs::create_dir(&real_root).unwrap();
        symlink(&real_root, layout.root()).unwrap();
        assert_eq!(
            install_bootstrap_artifact(&source, &request, &layout),
            Err(BootstrapInstallError::InvalidLayout)
        );
        assert!(!real_root.join(CURRENT_NAME).exists());
    }

    #[test]
    fn replay_rejects_relaxed_executable_permissions() {
        let (_temp, source, request, layout) = fixture();
        let installed = install_bootstrap_artifact(&source, &request, &layout).unwrap();
        let executable = layout
            .root
            .join("generations")
            .join(installed.generation)
            .join(EXECUTABLE_NAME);
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(
            install_bootstrap_artifact(&source, &request, &layout),
            Err(BootstrapInstallError::GenerationConflict)
        );
    }

    #[test]
    fn replay_rejects_a_modified_service_descriptor() {
        let (_temp, source, request, layout) = fixture();
        let installed = install_bootstrap_artifact(&source, &request, &layout).unwrap();
        let descriptor = layout
            .root
            .join("generations")
            .join(installed.generation)
            .join(SERVICE_DESCRIPTOR_NAME);
        fs::write(&descriptor, b"untrusted service command\n").unwrap();
        fs::set_permissions(&descriptor, fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(
            install_bootstrap_artifact(&source, &request, &layout),
            Err(BootstrapInstallError::GenerationConflict)
        );
    }
}
