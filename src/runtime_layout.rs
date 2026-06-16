use std::collections::BTreeSet;
use std::env;
use std::path::{Path, PathBuf};

const APP_DIR: &str = "gewyvern";
const LEGACY_APP_DIR: &str = ".gewyvern";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeLayout {
    pub config_root: PathBuf,
    pub data_root: PathBuf,
    pub state_root: PathBuf,
    pub cache_root: PathBuf,
    pub legacy_root: Option<PathBuf>,
}

pub fn runtime_layout() -> RuntimeLayout {
    RuntimeLayout {
        config_root: env_path("GEWY_CONFIG_HOME").unwrap_or_else(default_config_root),
        data_root: env_path("GEWY_DATA_HOME").unwrap_or_else(default_data_root),
        state_root: env_path("GEWY_STATE_HOME").unwrap_or_else(default_state_root),
        cache_root: env_path("GEWY_CACHE_HOME").unwrap_or_else(default_cache_root),
        legacy_root: legacy_root(),
    }
}

pub fn default_runtime_log_path() -> PathBuf {
    runtime_layout().state_root.join("logs").join("runtime.log")
}

pub fn packaged_share_roots(packaged_share_root: &Path) -> Vec<PathBuf> {
    let layout = runtime_layout();
    let mut roots = Vec::new();
    if let Some(root) = env_path("GEWY_SHARE_ROOT") {
        roots.push(root);
    }
    roots.push(layout.data_root.clone());
    if let Some(root) = layout.legacy_root {
        roots.push(root);
    }
    roots.push(packaged_share_root.to_path_buf());
    if let Some(root) = executable_share_root() {
        roots.push(root);
    }
    dedup_paths(roots)
}

pub fn protocol_registry_roots(
    repo_registry_root: &Path,
    packaged_share_root: &Path,
) -> Vec<PathBuf> {
    let layout = runtime_layout();
    let mut roots = Vec::new();
    if let Some(root) = env_path("GEWY_PROTOCOL_REGISTRY_ROOT") {
        roots.push(root);
    }
    roots.push(layout.data_root.join("protocols"));
    if let Some(root) = layout.legacy_root {
        roots.push(root.join("protocols"));
    }
    roots.push(repo_registry_root.to_path_buf());
    for share_root in packaged_share_roots(packaged_share_root) {
        roots.push(share_root.join("protocols"));
    }
    dedup_paths(roots)
}

fn env_path(key: &str) -> Option<PathBuf> {
    env::var_os(key)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            env::var_os("USERPROFILE")
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
        })
        .or_else(|| {
            let drive = env::var_os("HOMEDRIVE")?;
            let path = env::var_os("HOMEPATH")?;
            let mut joined = PathBuf::from(drive);
            joined.push(path);
            Some(joined)
        })
}

#[cfg(target_os = "macos")]
fn default_config_root() -> PathBuf {
    home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Library")
        .join("Application Support")
        .join(APP_DIR)
        .join("config")
}

#[cfg(target_os = "macos")]
fn default_data_root() -> PathBuf {
    home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Library")
        .join("Application Support")
        .join(APP_DIR)
        .join("data")
}

#[cfg(target_os = "macos")]
fn default_state_root() -> PathBuf {
    home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Library")
        .join("Application Support")
        .join(APP_DIR)
        .join("state")
}

#[cfg(target_os = "macos")]
fn default_cache_root() -> PathBuf {
    home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Library")
        .join("Caches")
        .join(APP_DIR)
}

#[cfg(target_os = "windows")]
fn default_config_root() -> PathBuf {
    env_path("APPDATA")
        .unwrap_or_else(|| PathBuf::from("."))
        .join(APP_DIR)
        .join("config")
}

#[cfg(target_os = "windows")]
fn default_data_root() -> PathBuf {
    env_path("APPDATA")
        .unwrap_or_else(|| PathBuf::from("."))
        .join(APP_DIR)
        .join("data")
}

#[cfg(target_os = "windows")]
fn default_state_root() -> PathBuf {
    env_path("LOCALAPPDATA")
        .unwrap_or_else(|| PathBuf::from("."))
        .join(APP_DIR)
        .join("state")
}

#[cfg(target_os = "windows")]
fn default_cache_root() -> PathBuf {
    env_path("LOCALAPPDATA")
        .unwrap_or_else(|| PathBuf::from("."))
        .join(APP_DIR)
        .join("cache")
}

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
fn default_config_root() -> PathBuf {
    env_path("XDG_CONFIG_HOME")
        .or_else(|| home_dir().map(|root| root.join(".config")))
        .unwrap_or_else(|| PathBuf::from("."))
        .join(APP_DIR)
}

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
fn default_data_root() -> PathBuf {
    env_path("XDG_DATA_HOME")
        .or_else(|| home_dir().map(|root| root.join(".local").join("share")))
        .unwrap_or_else(|| PathBuf::from("."))
        .join(APP_DIR)
}

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
fn default_state_root() -> PathBuf {
    env_path("XDG_STATE_HOME")
        .or_else(|| home_dir().map(|root| root.join(".local").join("state")))
        .unwrap_or_else(|| PathBuf::from("."))
        .join(APP_DIR)
}

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
fn default_cache_root() -> PathBuf {
    env_path("XDG_CACHE_HOME")
        .or_else(|| home_dir().map(|root| root.join(".cache")))
        .unwrap_or_else(|| PathBuf::from("."))
        .join(APP_DIR)
}

fn legacy_root() -> Option<PathBuf> {
    home_dir().map(|root| root.join(LEGACY_APP_DIR))
}

fn executable_share_root() -> Option<PathBuf> {
    let exe = env::current_exe().ok()?;
    let bin_dir = exe.parent()?;
    let prefix = bin_dir.parent()?;
    Some(prefix.join("share").join(APP_DIR))
}

fn dedup_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = BTreeSet::new();
    let mut deduped = Vec::new();
    for path in paths {
        let key = path.to_string_lossy().into_owned();
        if seen.insert(key) {
            deduped.push(path);
        }
    }
    deduped
}
