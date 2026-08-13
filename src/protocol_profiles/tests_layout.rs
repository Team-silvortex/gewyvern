use crate::runtime_layout::{protocol_registry_roots, runtime_layout};
use std::path::{Path, PathBuf};

use super::tests_env::EnvGuard;

#[test]
fn runtime_layout_prefers_explicit_standard_root_overrides() {
    let _lock = super::tests_env::lock();
    let _config = EnvGuard::set("GEWY_CONFIG_HOME", "/tmp/gewy-config");
    let _data = EnvGuard::set("GEWY_DATA_HOME", "/tmp/gewy-data");
    let _state = EnvGuard::set("GEWY_STATE_HOME", "/tmp/gewy-state");
    let _cache = EnvGuard::set("GEWY_CACHE_HOME", "/tmp/gewy-cache");

    let layout = runtime_layout();

    assert_eq!(layout.config_root, PathBuf::from("/tmp/gewy-config"));
    assert_eq!(layout.data_root, PathBuf::from("/tmp/gewy-data"));
    assert_eq!(layout.state_root, PathBuf::from("/tmp/gewy-state"));
    assert_eq!(layout.cache_root, PathBuf::from("/tmp/gewy-cache"));
    assert_eq!(
        layout.certificate_root,
        PathBuf::from("/tmp/gewy-config/certificates")
    );
    assert_eq!(
        layout.trust_root,
        PathBuf::from("/tmp/gewy-config/certificates/trust")
    );
    assert_eq!(
        layout.authority_root,
        PathBuf::from("/tmp/gewy-config/certificates/authorities")
    );
    assert_eq!(
        layout.identity_root,
        PathBuf::from("/tmp/gewy-config/certificates/identities")
    );
    assert_eq!(
        layout.certificate_state_root,
        PathBuf::from("/tmp/gewy-state/certificates")
    );
}

#[test]
fn protocol_registry_roots_keep_legacy_home_as_fallback() {
    let _lock = super::tests_env::lock();
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
    let _lock = super::tests_env::lock();
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

#[test]
fn protocol_registry_root_rejects_invalid_env_path() {
    let _lock = super::tests_env::lock();
    let _registry = EnvGuard::set("GEWY_PROTOCOL_REGISTRY_ROOT", "/tmp/custom-registry\n");
    let _data = EnvGuard::set("GEWY_DATA_HOME", "/tmp/gewy-data");
    let _home = EnvGuard::set("HOME", "/tmp/gewy-home");

    let roots = protocol_registry_roots(
        Path::new("/repo/protocols"),
        Path::new("/usr/share/gewyvern"),
    );

    assert_ne!(roots[0], PathBuf::from("/tmp/custom-registry\n"));
    assert_eq!(roots[0], PathBuf::from("/tmp/gewy-data/protocols"));
}

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
#[test]
fn runtime_layout_defaults_when_xdg_home_env_is_unsafe() {
    let _lock = super::tests_env::lock();
    let _bad_config = EnvGuard::set("XDG_CONFIG_HOME", "/tmp/config\nbad");
    let _bad_data = EnvGuard::set("XDG_DATA_HOME", " /tmp/data");
    let _bad_state = EnvGuard::set("XDG_STATE_HOME", "/tmp/state\n");
    let _bad_cache = EnvGuard::set("XDG_CACHE_HOME", "");
    let _home = EnvGuard::set("HOME", "/tmp/gewy-home");

    let layout = runtime_layout();

    assert_eq!(layout.config_root, PathBuf::from("/tmp/gewy-home/.config/gewyvern"));
    assert_eq!(layout.data_root, PathBuf::from("/tmp/gewy-home/.local/share/gewyvern"));
    assert_eq!(layout.state_root, PathBuf::from("/tmp/gewy-home/.local/state/gewyvern"));
    assert_eq!(layout.cache_root, PathBuf::from("/tmp/gewy-home/.cache/gewyvern"));
}

#[cfg(target_os = "windows")]
#[test]
fn runtime_layout_defaults_when_windows_env_is_unsafe() {
    let _lock = super::tests_env::lock();
    let _bad_config = EnvGuard::set("APPDATA", "C:\\Windows\\System32\n");
    let _bad_local = EnvGuard::set("LOCALAPPDATA", " /tmp/localapp");

    let layout = runtime_layout();

    assert_eq!(layout.config_root, PathBuf::from(".\\gewyvern\\config"));
    assert_eq!(layout.data_root, PathBuf::from(".\\gewyvern\\data"));
    assert_eq!(layout.state_root, PathBuf::from(".\\gewyvern\\state"));
    assert_eq!(layout.cache_root, PathBuf::from(".\\gewyvern\\cache"));
}

#[cfg(target_os = "macos")]
#[test]
fn runtime_layout_defaults_when_macos_env_is_unsafe() {
    let _lock = super::tests_env::lock();
    let _home = EnvGuard::set("HOME", "/tmp/gewy-home");
    let _bad_data = EnvGuard::set("GEWY_DATA_HOME", "/tmp/data\n");
    let _bad_state = EnvGuard::set("GEWY_STATE_HOME", "");
    let _bad_cache = EnvGuard::set("GEWY_CACHE_HOME", "/tmp/cache\n");
    let _bad_config = EnvGuard::set("GEWY_CONFIG_HOME", " /tmp/config");

    let layout = runtime_layout();

    assert_eq!(
        layout.config_root,
        PathBuf::from("/tmp/gewy-home/Library/Application Support/gewyvern/config")
    );
    assert_eq!(
        layout.data_root,
        PathBuf::from("/tmp/gewy-home/Library/Application Support/gewyvern/data")
    );
    assert_eq!(
        layout.state_root,
        PathBuf::from("/tmp/gewy-home/Library/Application Support/gewyvern/state")
    );
    assert_eq!(layout.cache_root, PathBuf::from("/tmp/gewy-home/Library/Caches/gewyvern"));
}
