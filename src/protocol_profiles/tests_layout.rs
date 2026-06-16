use crate::runtime_layout::{protocol_registry_roots, runtime_layout};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

struct EnvGuard {
    key: &'static str,
    previous: Option<String>,
}

impl EnvGuard {
    fn set(key: &'static str, value: impl Into<String>) -> Self {
        let previous = std::env::var(key).ok();
        unsafe {
            std::env::set_var(key, value.into());
        }
        Self { key, previous }
    }

    fn remove(key: &'static str) -> Self {
        let previous = std::env::var(key).ok();
        unsafe {
            std::env::remove_var(key);
        }
        Self { key, previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        match &self.previous {
            Some(value) => unsafe {
                std::env::set_var(self.key, value);
            },
            None => unsafe {
                std::env::remove_var(self.key);
            },
        }
    }
}

#[test]
fn runtime_layout_prefers_explicit_standard_root_overrides() {
    let _lock = env_lock().lock().unwrap();
    let _config = EnvGuard::set("GEWY_CONFIG_HOME", "/tmp/gewy-config");
    let _data = EnvGuard::set("GEWY_DATA_HOME", "/tmp/gewy-data");
    let _state = EnvGuard::set("GEWY_STATE_HOME", "/tmp/gewy-state");
    let _cache = EnvGuard::set("GEWY_CACHE_HOME", "/tmp/gewy-cache");

    let layout = runtime_layout();

    assert_eq!(layout.config_root, PathBuf::from("/tmp/gewy-config"));
    assert_eq!(layout.data_root, PathBuf::from("/tmp/gewy-data"));
    assert_eq!(layout.state_root, PathBuf::from("/tmp/gewy-state"));
    assert_eq!(layout.cache_root, PathBuf::from("/tmp/gewy-cache"));
}

#[test]
fn protocol_registry_roots_keep_legacy_home_as_fallback() {
    let _lock = env_lock().lock().unwrap();
    let _registry = EnvGuard::remove("GEWY_PROTOCOL_REGISTRY_ROOT");
    let _share = EnvGuard::remove("GEWY_SHARE_ROOT");
    let _config = EnvGuard::remove("GEWY_CONFIG_HOME");
    let _state = EnvGuard::remove("GEWY_STATE_HOME");
    let _cache = EnvGuard::remove("GEWY_CACHE_HOME");
    let _data = EnvGuard::set("GEWY_DATA_HOME", "/tmp/gewy-data");
    let _home = EnvGuard::set("HOME", "/tmp/gewy-home");

    let roots = protocol_registry_roots(
        Path::new("/repo/protocols"),
        Path::new("/usr/share/gewyvern"),
    );

    assert_eq!(roots[0], PathBuf::from("/tmp/gewy-data/protocols"));
    assert!(roots.contains(&PathBuf::from("/tmp/gewy-home/.gewyvern/protocols")));
    assert!(roots.contains(&PathBuf::from("/repo/protocols")));
}

#[test]
fn explicit_protocol_registry_root_stays_highest_priority() {
    let _lock = env_lock().lock().unwrap();
    let _registry = EnvGuard::set("GEWY_PROTOCOL_REGISTRY_ROOT", "/tmp/custom-registry");
    let _data = EnvGuard::set("GEWY_DATA_HOME", "/tmp/gewy-data");
    let _home = EnvGuard::set("HOME", "/tmp/gewy-home");

    let roots = protocol_registry_roots(
        Path::new("/repo/protocols"),
        Path::new("/usr/share/gewyvern"),
    );

    assert_eq!(roots[0], PathBuf::from("/tmp/custom-registry"));
    assert_eq!(roots[1], PathBuf::from("/tmp/gewy-data/protocols"));
}
