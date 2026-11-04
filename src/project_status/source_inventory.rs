use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use silvortex_bounded_io::read_bounded_utf8_regular_file;

pub const SOURCE_INVENTORY_SCHEMA_VERSION: u32 = 1;
pub const MAX_SOURCE_INVENTORY_BYTES: u64 = 1024 * 1024;
pub const MAX_SOURCE_FILE_BYTES: u64 = 4 * 1024 * 1024;
pub const MAX_SOURCE_INVENTORY_FILES: usize = 4096;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SourceInventory {
    pub schema_version: u32,
    pub repository: String,
    pub revision: String,
    pub files: Vec<SourceInventoryFile>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SourceInventoryFile {
    pub path: String,
    pub sha256: String,
    pub bytes: u64,
}

pub fn is_repository_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 1024
        && !path
            .chars()
            .any(|c| c.is_control() || matches!(c, '\\' | ':'))
        && path
            .split('/')
            .all(|part| !matches!(part, "" | "." | "..") && !part.ends_with([' ', '.']))
}

fn is_digest(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

impl SourceInventory {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, String> {
        let source = read_bounded_utf8_regular_file(path.as_ref(), MAX_SOURCE_INVENTORY_BYTES)
            .map_err(|error| format!("cannot read source inventory: {error}"))?;
        let inventory: Self = serde_json::from_str(&source)
            .map_err(|error| format!("cannot decode source inventory: {error}"))?;
        inventory.validate()?;
        Ok(inventory)
    }

    pub fn load_from_repository(root: &Path, path: &str) -> Result<Self, String> {
        if !is_repository_path(path) {
            return Err("source inventory path must stay inside the repository".into());
        }
        let mut resolved = PathBuf::from(root);
        for component in path.split('/') {
            resolved.push(component);
            let metadata = std::fs::symlink_metadata(&resolved)
                .map_err(|error| format!("cannot inspect source inventory path: {error}"))?;
            if metadata.file_type().is_symlink() {
                return Err("source inventory path must not follow symlinks".into());
            }
        }
        Self::load(resolved)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != SOURCE_INVENTORY_SCHEMA_VERSION {
            return Err("unsupported source inventory schema_version".into());
        }
        let repository = self
            .repository
            .parse::<http::Uri>()
            .map_err(|_| "source inventory repository must be an HTTPS URL")?;
        if self.repository.len() > 512
            || repository.scheme_str() != Some("https")
            || repository
                .authority()
                .is_none_or(|authority| authority.as_str().contains('@'))
            || repository.query().is_some()
            || repository
                .path()
                .strip_prefix('/')
                .is_none_or(|path| !is_repository_path(path))
            || self.repository.contains(['#', '%'])
        {
            return Err("source inventory requires a credential-free HTTPS repository URL".into());
        }
        if !is_digest(&self.revision, 40) && !is_digest(&self.revision, 64) {
            return Err("source inventory revision must be a full lowercase Git digest".into());
        }
        if self.files.is_empty() || self.files.len() > MAX_SOURCE_INVENTORY_FILES {
            return Err("source inventory file count is out of bounds".into());
        }
        let mut previous = None;
        for file in &self.files {
            if !is_repository_path(&file.path) {
                return Err(format!("invalid source inventory file path: {}", file.path));
            }
            if previous.is_some_and(|path| path >= file.path.as_str()) {
                return Err("source inventory paths must be unique and sorted".into());
            }
            if !is_digest(&file.sha256, 64) {
                return Err(format!("invalid source inventory SHA-256: {}", file.path));
            }
            if file.bytes > MAX_SOURCE_FILE_BYTES {
                return Err(format!("source inventory file is too large: {}", file.path));
            }
            previous = Some(file.path.as_str());
        }
        Ok(())
    }

    pub fn file(&self, path: &str) -> Option<&SourceInventoryFile> {
        self.files
            .binary_search_by(|file| file.path.as_str().cmp(path))
            .ok()
            .map(|index| &self.files[index])
    }

    /// Checks supplied bytes, without fetching sources or assuming that tests ran.
    pub fn verify_file(&self, path: &str, bytes: &[u8]) -> Result<(), String> {
        let file = self
            .file(path)
            .ok_or_else(|| format!("unrecorded source file: {path}"))?;
        if file.bytes != bytes.len() as u64 {
            return Err(format!("source inventory byte length mismatch: {path}"));
        }
        let digest = ring::digest::digest(&ring::digest::SHA256, bytes);
        let actual: String = digest
            .as_ref()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        if file.sha256 != actual {
            return Err(format!("source inventory SHA-256 mismatch: {path}"));
        }
        Ok(())
    }
}
