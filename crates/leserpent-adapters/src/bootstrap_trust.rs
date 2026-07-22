use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use leserpent_domain::bootstrap::CredentialHandle;
use ring::digest::{SHA256, digest};
use rustls::RootCertStore;
use rustls::pki_types::{CertificateDer, pem::PemObject};
use serde::{Deserialize, Serialize};

const MAX_TRUST_RECORD_BYTES: u64 = 64 * 1024;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BootstrapTrustRecord {
    pub endpoint: String,
    pub ca_pem: String,
    pub ca_sha256: String,
}

impl BootstrapTrustRecord {
    pub fn validate(&self) -> Result<(), BootstrapTrustError> {
        let certificates = CertificateDer::pem_slice_iter(self.ca_pem.as_bytes())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| BootstrapTrustError::InvalidRecord)?;
        let mut roots = RootCertStore::empty();
        for certificate in certificates {
            roots
                .add(certificate)
                .map_err(|_| BootstrapTrustError::InvalidRecord)?;
        }
        if !valid_https_origin(&self.endpoint)
            || !self.ca_pem.starts_with("-----BEGIN CERTIFICATE-----\n")
            || !self.ca_pem.ends_with("-----END CERTIFICATE-----\n")
            || self.ca_pem.len() > 32 * 1024
            || roots.is_empty()
            || self.ca_sha256 != hex(digest(&SHA256, self.ca_pem.as_bytes()).as_ref())
        {
            return Err(BootstrapTrustError::InvalidRecord);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BootstrapTrustError {
    InvalidHandle,
    InvalidRecord,
    UnsafeStorage,
    Storage,
}

pub trait BootstrapTrustStore: Send + Sync {
    fn persist(
        &self,
        handle: &CredentialHandle,
        record: &BootstrapTrustRecord,
    ) -> Result<(), BootstrapTrustError>;
}

pub struct FileBootstrapTrustStore {
    root: PathBuf,
}

impl FileBootstrapTrustStore {
    pub fn new(root: impl Into<PathBuf>) -> Result<Self, BootstrapTrustError> {
        let root = root.into();
        let safe = root.is_absolute()
            && root != Path::new("/")
            && root
                .components()
                .all(|part| !matches!(part, Component::ParentDir | Component::CurDir));
        if !safe {
            return Err(BootstrapTrustError::UnsafeStorage);
        }
        if fs::symlink_metadata(&root).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
            return Err(BootstrapTrustError::UnsafeStorage);
        }
        Ok(Self {
            root: normalize_new_path(&root)?,
        })
    }

    pub fn load(
        &self,
        handle: &CredentialHandle,
    ) -> Result<Option<BootstrapTrustRecord>, BootstrapTrustError> {
        let path = self.record_path(handle)?;
        match fs::symlink_metadata(&path) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(BootstrapTrustError::Storage),
        }
        let bytes = read_private_record(&path)?;
        let record = serde_json::from_slice::<BootstrapTrustRecord>(&bytes)
            .map_err(|_| BootstrapTrustError::InvalidRecord)?;
        record.validate()?;
        Ok(Some(record))
    }

    fn record_path(&self, handle: &CredentialHandle) -> Result<PathBuf, BootstrapTrustError> {
        let (provider, key) = handle.parts();
        if provider != "leserpent-ca"
            || key.is_empty()
            || !key
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        {
            return Err(BootstrapTrustError::InvalidHandle);
        }
        Ok(self.root.join(format!("{key}.json")))
    }
}

fn normalize_new_path(path: &Path) -> Result<PathBuf, BootstrapTrustError> {
    let mut existing = path;
    let mut suffix = Vec::new();
    while !existing.exists() {
        suffix.push(
            existing
                .file_name()
                .ok_or(BootstrapTrustError::UnsafeStorage)?
                .to_os_string(),
        );
        existing = existing
            .parent()
            .ok_or(BootstrapTrustError::UnsafeStorage)?;
    }
    let mut normalized = fs::canonicalize(existing).map_err(|_| BootstrapTrustError::Storage)?;
    for component in suffix.into_iter().rev() {
        normalized.push(component);
    }
    Ok(normalized)
}

impl BootstrapTrustStore for FileBootstrapTrustStore {
    fn persist(
        &self,
        handle: &CredentialHandle,
        record: &BootstrapTrustRecord,
    ) -> Result<(), BootstrapTrustError> {
        record.validate()?;
        reject_symlink_components(&self.root)?;
        fs::create_dir_all(&self.root).map_err(|_| BootstrapTrustError::Storage)?;
        let metadata =
            fs::symlink_metadata(&self.root).map_err(|_| BootstrapTrustError::Storage)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(BootstrapTrustError::UnsafeStorage);
        }
        set_mode(&self.root, 0o700)?;
        let destination = self.record_path(handle)?;
        match fs::symlink_metadata(&destination) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
                return Err(BootstrapTrustError::UnsafeStorage);
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(BootstrapTrustError::Storage),
        }
        let bytes = serde_json::to_vec(record).map_err(|_| BootstrapTrustError::InvalidRecord)?;
        let temporary =
            self.root
                .join(format!(".trust-{}-{}", std::process::id(), unique_suffix()));
        let result = (|| {
            write_new_private(&temporary, &bytes)?;
            fs::rename(&temporary, &destination).map_err(|_| BootstrapTrustError::Storage)?;
            File::open(&self.root)
                .and_then(|directory| directory.sync_all())
                .map_err(|_| BootstrapTrustError::Storage)?;
            (read_private_record(&destination)? == bytes)
                .then_some(())
                .ok_or(BootstrapTrustError::Storage)
        })();
        if result.is_err() {
            let _ = fs::remove_file(temporary);
        }
        result
    }
}

fn reject_symlink_components(path: &Path) -> Result<(), BootstrapTrustError> {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(BootstrapTrustError::UnsafeStorage);
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(BootstrapTrustError::Storage),
        }
    }
    Ok(())
}

fn write_new_private(path: &Path, bytes: &[u8]) -> Result<(), BootstrapTrustError> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    set_open_mode(&mut options, 0o600);
    let mut file = options
        .open(path)
        .map_err(|_| BootstrapTrustError::Storage)?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| BootstrapTrustError::Storage)?;
    require_mode(path, 0o600)
}

fn read_private_record(path: &Path) -> Result<Vec<u8>, BootstrapTrustError> {
    let mut options = OpenOptions::new();
    options.read(true);
    set_no_follow(&mut options);
    let mut file = options
        .open(path)
        .map_err(|_| BootstrapTrustError::Storage)?;
    let metadata = file.metadata().map_err(|_| BootstrapTrustError::Storage)?;
    if !metadata.is_file()
        || !metadata_has_mode(&metadata, 0o600)
        || metadata.len() == 0
        || metadata.len() > MAX_TRUST_RECORD_BYTES
    {
        return Err(BootstrapTrustError::UnsafeStorage);
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.read_to_end(&mut bytes)
        .map_err(|_| BootstrapTrustError::Storage)?;
    (bytes.len() as u64 == metadata.len())
        .then_some(bytes)
        .ok_or(BootstrapTrustError::Storage)
}

#[cfg(unix)]
fn metadata_has_mode(metadata: &fs::Metadata, mode: u32) -> bool {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o777 == mode
}

#[cfg(not(unix))]
fn metadata_has_mode(_metadata: &fs::Metadata, _mode: u32) -> bool {
    true
}

fn valid_https_origin(value: &str) -> bool {
    value.strip_prefix("https://").is_some_and(|authority| {
        !authority.is_empty()
            && authority.len() <= 320
            && !authority.contains(['/', '?', '#', '@'])
            && !authority.chars().any(char::is_whitespace)
    })
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
    options.mode(mode).custom_flags(libc::O_NOFOLLOW);
}

#[cfg(not(unix))]
fn set_open_mode(_options: &mut OpenOptions, _mode: u32) {}

#[cfg(unix)]
fn set_no_follow(options: &mut OpenOptions) {
    use std::os::unix::fs::OpenOptionsExt;
    options.custom_flags(libc::O_NOFOLLOW);
}

#[cfg(not(unix))]
fn set_no_follow(_options: &mut OpenOptions) {}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> Result<(), BootstrapTrustError> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
        .map_err(|_| BootstrapTrustError::Storage)
}

#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: u32) -> Result<(), BootstrapTrustError> {
    Ok(())
}

#[cfg(unix)]
fn require_mode(path: &Path, mode: u32) -> Result<(), BootstrapTrustError> {
    use std::os::unix::fs::PermissionsExt;
    let metadata = fs::symlink_metadata(path).map_err(|_| BootstrapTrustError::Storage)?;
    if metadata.file_type().is_symlink() || metadata.permissions().mode() & 0o777 != mode {
        return Err(BootstrapTrustError::UnsafeStorage);
    }
    Ok(())
}

#[cfg(not(unix))]
fn require_mode(path: &Path, _mode: u32) -> Result<(), BootstrapTrustError> {
    if fs::symlink_metadata(path)
        .map_err(|_| BootstrapTrustError::Storage)?
        .file_type()
        .is_symlink()
    {
        return Err(BootstrapTrustError::UnsafeStorage);
    }
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use std::os::unix::fs::{PermissionsExt, symlink};

    use super::*;

    fn record(ca_pem: &str) -> BootstrapTrustRecord {
        BootstrapTrustRecord {
            endpoint: "https://host.example:7443".into(),
            ca_pem: ca_pem.into(),
            ca_sha256: hex(digest(&SHA256, ca_pem.as_bytes()).as_ref()),
        }
    }

    #[test]
    fn file_store_is_private_atomic_and_endpoint_bound() {
        let root = std::env::temp_dir().join(format!("leserpent-trust-{}", unique_suffix()));
        let store = FileBootstrapTrustStore::new(&root).unwrap();
        let handle = CredentialHandle::new("vault:leserpent-ca:host-example").unwrap();
        let ca = rcgen::generate_simple_self_signed(vec!["host.example".into()])
            .unwrap()
            .cert
            .pem();
        store.persist(&handle, &record(&ca)).unwrap();
        assert_eq!(store.load(&handle).unwrap(), Some(record(&ca)));
        assert_eq!(
            fs::metadata(&root).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(root.join("host-example.json"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn file_store_rejects_symlinked_roots_and_wrong_providers() {
        let parent = std::env::temp_dir().join(format!("leserpent-trust-link-{}", unique_suffix()));
        let target = parent.join("target");
        let link = parent.join("link");
        fs::create_dir_all(&target).unwrap();
        symlink(&target, &link).unwrap();
        assert!(matches!(
            FileBootstrapTrustStore::new(&link),
            Err(BootstrapTrustError::UnsafeStorage)
        ));
        let store = FileBootstrapTrustStore::new(parent.join("safe")).unwrap();
        let ca = rcgen::generate_simple_self_signed(vec!["host.example".into()])
            .unwrap()
            .cert
            .pem();
        assert_eq!(
            store.persist(
                &CredentialHandle::new("vault:leserpentd:host-example").unwrap(),
                &record(&ca)
            ),
            Err(BootstrapTrustError::InvalidHandle)
        );
        fs::remove_dir_all(parent).unwrap();
    }
}
