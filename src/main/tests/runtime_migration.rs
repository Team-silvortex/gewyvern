use crate::runtime_migration::prepare_runtime_layout;
use gewyvern::runtime_layout::runtime_layout;
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};
#[cfg(unix)]
use std::os::unix::fs::symlink;

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

fn temp_dir(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "gewyvern-runtime-migration-{label}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

#[test]
fn runtime_migration_creates_standard_roots_and_copies_legacy_config() {
    let _lock = env_lock()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let home = temp_dir("config-home");
    let legacy_root = home.join(".gewyvern");
    let config_root = home.join("config-root");
    let data_root = home.join("data-root");
    let state_root = home.join("state-root");
    let cache_root = home.join("cache-root");
    fs::create_dir_all(&legacy_root).unwrap();
    fs::write(
        legacy_root.join("config.toml"),
        "[runtime]\nserve = true\nsocket = \"unix:/tmp/gewyvern.sock\"\n",
    )
    .unwrap();
    let _home = EnvGuard::set("HOME", home.to_string_lossy());
    let _config = EnvGuard::set("GEWY_CONFIG_HOME", config_root.to_string_lossy());
    let _data = EnvGuard::set("GEWY_DATA_HOME", data_root.to_string_lossy());
    let _state = EnvGuard::set("GEWY_STATE_HOME", state_root.to_string_lossy());
    let _cache = EnvGuard::set("GEWY_CACHE_HOME", cache_root.to_string_lossy());

    let report = prepare_runtime_layout().unwrap();
    let migrated = config_root.join("gewyvern.toml");

    assert!(config_root.exists());
    assert!(data_root.exists());
    assert!(state_root.exists());
    assert!(cache_root.exists());
    assert!(config_root.join("certificates").exists());
    assert!(config_root.join("certificates").join("trust").exists());
    assert!(
        config_root
            .join("certificates")
            .join("authorities")
            .exists()
    );
    assert!(config_root.join("certificates").join("identities").exists());
    assert!(state_root.join("certificates").exists());
    assert_eq!(report.copied_config_to.as_deref(), Some(migrated.as_path()));
    assert_eq!(
        fs::read_to_string(&migrated).unwrap(),
        fs::read_to_string(legacy_root.join("config.toml")).unwrap()
    );

    fs::remove_dir_all(&home).unwrap();
}

#[test]
fn runtime_migration_preserves_existing_standard_config() {
    let _lock = env_lock()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let home = temp_dir("config-preserve");
    let legacy_root = home.join(".gewyvern");
    let config_root = home.join("config-root");
    fs::create_dir_all(&legacy_root).unwrap();
    fs::create_dir_all(&config_root).unwrap();
    fs::write(legacy_root.join("config.toml"), "legacy = true\n").unwrap();
    fs::write(config_root.join("gewyvern.toml"), "legacy = false\n").unwrap();
    let _home = EnvGuard::set("HOME", home.to_string_lossy());
    let _config = EnvGuard::set("GEWY_CONFIG_HOME", config_root.to_string_lossy());

    let report = prepare_runtime_layout().unwrap();

    assert!(report.copied_config_to.is_none());
    assert_eq!(
        fs::read_to_string(config_root.join("gewyvern.toml")).unwrap(),
        "legacy = false\n"
    );

    fs::remove_dir_all(&home).unwrap();
}

#[test]
fn runtime_migration_prepares_default_roots_when_config_home_is_invalid() {
    let _lock = env_lock()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let home = temp_dir("migration-invalid-config-home");
    let config_root = home.join("config-root");
    let invalid_config_home = format!(" {} ", config_root.display());
    let legacy_root = home.join(".gewyvern");
    fs::create_dir_all(&legacy_root).unwrap();
    fs::create_dir_all(&config_root).unwrap();
    let _home = EnvGuard::set("HOME", home.to_string_lossy());
    let _config = EnvGuard::set("GEWY_CONFIG_HOME", &invalid_config_home);
    let expected_layout = runtime_layout();

    let report = prepare_runtime_layout().unwrap();

    assert!(
        report
            .created_roots
            .contains(&expected_layout.config_root)
    );
    assert!(
        report
            .created_roots
            .contains(&expected_layout.data_root)
    );
    assert!(
        report
            .created_roots
            .contains(&expected_layout.state_root)
    );
    assert!(
        report
            .created_roots
            .contains(&expected_layout.cache_root)
    );
    assert!(
        !report
            .created_roots
            .iter()
            .any(|path| path == &config_root)
    );

    fs::remove_dir_all(&home).unwrap();
}

#[test]
fn runtime_migration_copies_missing_protocol_and_dsl_entries_without_overwrite() {
    let _lock = env_lock()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let home = temp_dir("content-home");
    let legacy_root = home.join(".gewyvern");
    let protocol_source = legacy_root.join("protocols").join("redis");
    let dsl_source = legacy_root.join("dsl");
    let config_root = home.join("config-root");
    let data_root = home.join("data-root");
    let state_root = home.join("state-root");
    let cache_root = home.join("cache-root");
    let protocol_target = data_root.join("protocols").join("redis");
    let dsl_target = data_root.join("dsl");
    fs::create_dir_all(&protocol_source).unwrap();
    fs::create_dir_all(&dsl_source).unwrap();
    fs::create_dir_all(&protocol_target).unwrap();
    fs::write(protocol_source.join("zadd.gewy"), "legacy-zadd").unwrap();
    fs::write(protocol_target.join("zadd.gewy"), "new-zadd").unwrap();
    fs::write(protocol_source.join("xadd.gewy"), "legacy-xadd").unwrap();
    fs::write(dsl_source.join("shelves.gewy"), "legacy-dsl").unwrap();
    let _home = EnvGuard::set("HOME", home.to_string_lossy());
    let _config = EnvGuard::set("GEWY_CONFIG_HOME", config_root.to_string_lossy());
    let _data = EnvGuard::set("GEWY_DATA_HOME", data_root.to_string_lossy());
    let _state = EnvGuard::set("GEWY_STATE_HOME", state_root.to_string_lossy());
    let _cache = EnvGuard::set("GEWY_CACHE_HOME", cache_root.to_string_lossy());

    let report = prepare_runtime_layout().unwrap();

    assert_eq!(report.copied_protocol_entries, 1);
    assert_eq!(report.copied_dsl_entries, 1);
    assert_eq!(
        fs::read_to_string(protocol_target.join("zadd.gewy")).unwrap(),
        "new-zadd"
    );
    assert_eq!(
        fs::read_to_string(protocol_target.join("xadd.gewy")).unwrap(),
        "legacy-xadd"
    );
    assert_eq!(
        fs::read_to_string(dsl_target.join("shelves.gewy")).unwrap(),
        "legacy-dsl"
    );

    fs::remove_dir_all(&home).unwrap();
}

#[test]
fn runtime_migration_copies_legacy_certificate_assets_without_overwrite() {
    let _lock = env_lock()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let home = temp_dir("certificate-home");
    let legacy_root = home.join(".gewyvern");
    let legacy_cert_root = legacy_root.join("certificates");
    let legacy_state_root = legacy_root.join("state").join("certificates");
    let config_root = home.join("config-root");
    let data_root = home.join("data-root");
    let state_root = home.join("state-root");
    let cache_root = home.join("cache-root");
    let trust_source = legacy_cert_root.join("trust").join("anchors");
    let authority_source = legacy_cert_root.join("authorities");
    let identity_source = legacy_cert_root.join("identities").join("prod");
    let trust_target = config_root
        .join("certificates")
        .join("trust")
        .join("anchors");
    let identity_target = config_root
        .join("certificates")
        .join("identities")
        .join("prod");
    let state_target = state_root.join("certificates");
    fs::create_dir_all(&trust_source).unwrap();
    fs::create_dir_all(&authority_source).unwrap();
    fs::create_dir_all(&identity_source).unwrap();
    fs::create_dir_all(&legacy_state_root).unwrap();
    fs::create_dir_all(&trust_target).unwrap();
    fs::create_dir_all(&identity_target).unwrap();
    fs::write(trust_source.join("root-ca.pem"), "legacy-root").unwrap();
    fs::write(authority_source.join("issuing-ca.pem"), "legacy-issuing").unwrap();
    fs::write(identity_source.join("runtime.pem"), "legacy-runtime").unwrap();
    fs::write(legacy_state_root.join("index.json"), "{\"legacy\":true}").unwrap();
    fs::write(identity_target.join("runtime.pem"), "new-runtime").unwrap();
    let _home = EnvGuard::set("HOME", home.to_string_lossy());
    let _config = EnvGuard::set("GEWY_CONFIG_HOME", config_root.to_string_lossy());
    let _data = EnvGuard::set("GEWY_DATA_HOME", data_root.to_string_lossy());
    let _state = EnvGuard::set("GEWY_STATE_HOME", state_root.to_string_lossy());
    let _cache = EnvGuard::set("GEWY_CACHE_HOME", cache_root.to_string_lossy());

    let report = prepare_runtime_layout().unwrap();

    assert_eq!(report.copied_certificate_entries, 2);
    assert_eq!(report.copied_certificate_state_entries, 1);
    assert_eq!(
        fs::read_to_string(trust_target.join("root-ca.pem")).unwrap(),
        "legacy-root"
    );
    assert_eq!(
        fs::read_to_string(
            config_root
                .join("certificates")
                .join("authorities")
                .join("issuing-ca.pem")
        )
        .unwrap(),
        "legacy-issuing"
    );
    assert_eq!(
        fs::read_to_string(identity_target.join("runtime.pem")).unwrap(),
        "new-runtime"
    );
    assert_eq!(
        fs::read_to_string(state_target.join("index.json")).unwrap(),
        "{\"legacy\":true}"
    );

    fs::remove_dir_all(&home).unwrap();
}

#[test]
#[cfg(unix)]
fn runtime_migration_does_not_follow_legacy_protocols_symlink() {
    let _lock = env_lock()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let home = temp_dir("protocol-symlink-home");
    let legacy_root = home.join(".gewyvern");
    let external = home.join("external-protocol-store");
    let linked_protocols = legacy_root.join("protocols");
    let data_root = home.join("data-root");
    let _home = EnvGuard::set("HOME", home.to_string_lossy());
    let _config = EnvGuard::set("GEWY_CONFIG_HOME", home.join("config-root").to_string_lossy());
    let _data = EnvGuard::set("GEWY_DATA_HOME", data_root.to_string_lossy());
    let _state = EnvGuard::set("GEWY_STATE_HOME", home.join("state-root").to_string_lossy());
    let _cache = EnvGuard::set("GEWY_CACHE_HOME", home.join("cache-root").to_string_lossy());

    fs::create_dir_all(&legacy_root).unwrap();
    fs::create_dir_all(&external).unwrap();
    fs::write(external.join("redis.gewy"), "legacy-overlink").unwrap();
    symlink(&external, &linked_protocols).unwrap();

    let report = prepare_runtime_layout().unwrap();

    assert_eq!(report.copied_protocol_entries, 0);
    assert!(!data_root.join("protocols").exists());
    assert_eq!(fs::read_dir(external).unwrap().count(), 1);

    fs::remove_dir_all(&home).unwrap();
}
