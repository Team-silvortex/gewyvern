use super::{Cli, IngestMode};
use crate::runtime_config::{apply_runtime_path_overrides, load_runtime_config};
use crate::runtime_logging::LogLevel;
use crate::{SocketTarget, cli::CliDefaults};
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

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

fn temp_dir(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "gewyvern-runtime-config-{label}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

#[test]
fn runtime_config_loads_service_defaults_from_standard_path() {
    let _lock = env_lock().lock().unwrap();
    let root = temp_dir("standard");
    let config_root = root.join("config");
    fs::create_dir_all(&config_root).unwrap();
    fs::write(
        config_root.join("gewyvern.toml"),
        r#"
schema_version = 1

[runtime]
serve = true
socket = "unix:/tmp/gewyvern.sock"
api_socket = "127.0.0.1:9910"
allow_remote_api = false
api_admin_token = "runtime-api-token"
ingest_mode = "local-advisory"
max_sessions = 32
history_retention = 12

[external_engine]
bin = "/opt/etragon/bin/etragon"
worker = "/opt/etragon/bin/worker.py"
python_bin = "/usr/bin/python3"

[paths]
protocol_registry_root = "/srv/gewyvern/protocols"
share_root = "/srv/gewyvern/share"

[certificates]
root = "/srv/gewyvern/certificates"
trust_root = "/srv/gewyvern/certificates/trust"
authority_root = "/srv/gewyvern/certificates/authorities"
identity_root = "/srv/gewyvern/certificates/identities"
state_root = "/srv/gewyvern/state/certificates"
require_explicit_remote_trust = true

[logging]
level = "info"
stderr = false
file = "/srv/gewyvern/state/logs/runtime.log"
max_bytes = 262144
max_files = 6

[resilience]
external_failure_circuit_threshold = 5
external_failure_circuit_cooldown_seconds = 45
socket_failure_backoff_base_ms = 150
socket_failure_backoff_cap_ms = 2500
"#,
    )
    .unwrap();
    let _config_home = EnvGuard::set("GEWY_CONFIG_HOME", config_root.to_string_lossy());
    let _config_file = EnvGuard::remove("GEWY_CONFIG_FILE");
    let _registry_root = EnvGuard::remove("GEWY_PROTOCOL_REGISTRY_ROOT");
    let _share_root = EnvGuard::remove("GEWY_SHARE_ROOT");
    let _certificate_root = EnvGuard::remove("GEWY_CERTIFICATE_ROOT");
    let _trust_root = EnvGuard::remove("GEWY_TRUST_ROOT");
    let _authority_root = EnvGuard::remove("GEWY_AUTHORITY_ROOT");
    let _identity_root = EnvGuard::remove("GEWY_IDENTITY_ROOT");
    let _certificate_state_root = EnvGuard::remove("GEWY_CERTIFICATE_STATE_ROOT");
    let _require_explicit_remote_trust = EnvGuard::remove("GEWY_REQUIRE_EXPLICIT_REMOTE_TRUST");
    let _history_retention = EnvGuard::remove("GEWY_HISTORY_RETENTION");
    let _external_threshold = EnvGuard::remove("GEWY_EXTERNAL_FAILURE_CIRCUIT_THRESHOLD");
    let _external_cooldown = EnvGuard::remove("GEWY_EXTERNAL_FAILURE_CIRCUIT_COOLDOWN_SECONDS");
    let _socket_backoff_base = EnvGuard::remove("GEWY_SOCKET_FAILURE_BACKOFF_BASE_MS");
    let _socket_backoff_cap = EnvGuard::remove("GEWY_SOCKET_FAILURE_BACKOFF_CAP_MS");
    let _api_admin_token = EnvGuard::remove("GEWY_API_ADMIN_TOKEN");

    let config = load_runtime_config().unwrap();
    assert_eq!(config.schema_version, 1);
    assert!(config.schema_version_explicit);
    assert_eq!(config.defaults.serve, Some(true));
    assert_eq!(
        config.defaults.socket_target,
        Some(SocketTarget::Unix("/tmp/gewyvern.sock".into()))
    );
    assert_eq!(
        config.defaults.api_socket.as_deref(),
        Some("127.0.0.1:9910")
    );
    assert_eq!(config.defaults.allow_remote_api, Some(false));
    assert_eq!(
        config.defaults.api_admin_token.as_deref(),
        Some("runtime-api-token")
    );
    assert_eq!(config.defaults.ingest_mode, Some(IngestMode::LocalAdvisory));
    assert_eq!(config.defaults.max_sessions, Some(32));
    assert_eq!(config.history_retention, Some(12));
    assert_eq!(
        config.defaults.external_engine_bin.as_deref(),
        Some("/opt/etragon/bin/etragon")
    );
    assert_eq!(
        config.protocol_registry_root.as_deref(),
        Some("/srv/gewyvern/protocols")
    );
    assert_eq!(config.share_root.as_deref(), Some("/srv/gewyvern/share"));
    assert_eq!(
        config.certificate_root.as_deref(),
        Some("/srv/gewyvern/certificates")
    );
    assert_eq!(
        config.trust_root.as_deref(),
        Some("/srv/gewyvern/certificates/trust")
    );
    assert_eq!(
        config.authority_root.as_deref(),
        Some("/srv/gewyvern/certificates/authorities")
    );
    assert_eq!(
        config.identity_root.as_deref(),
        Some("/srv/gewyvern/certificates/identities")
    );
    assert_eq!(
        config.certificate_state_root.as_deref(),
        Some("/srv/gewyvern/state/certificates")
    );
    assert_eq!(config.require_explicit_remote_trust, Some(true));
    assert_eq!(config.defaults.log_level, Some(LogLevel::Info));
    assert_eq!(config.defaults.log_to_stderr, Some(false));
    assert_eq!(
        config.defaults.log_file.as_deref(),
        Some("/srv/gewyvern/state/logs/runtime.log")
    );
    assert_eq!(config.defaults.log_max_bytes, Some(262144));
    assert_eq!(config.defaults.log_max_files, Some(6));
    assert_eq!(config.external_failure_circuit_threshold, Some(5));
    assert_eq!(config.external_failure_circuit_cooldown_seconds, Some(45));
    assert_eq!(config.socket_failure_backoff_base_ms, Some(150));
    assert_eq!(config.socket_failure_backoff_cap_ms, Some(2500));

    apply_runtime_path_overrides(&config);
    assert_eq!(
        std::env::var("GEWY_PROTOCOL_REGISTRY_ROOT").ok().as_deref(),
        Some("/srv/gewyvern/protocols")
    );
    assert_eq!(
        std::env::var("GEWY_SHARE_ROOT").ok().as_deref(),
        Some("/srv/gewyvern/share")
    );
    assert_eq!(
        std::env::var("GEWY_CERTIFICATE_ROOT").ok().as_deref(),
        Some("/srv/gewyvern/certificates")
    );
    assert_eq!(
        std::env::var("GEWY_TRUST_ROOT").ok().as_deref(),
        Some("/srv/gewyvern/certificates/trust")
    );
    assert_eq!(
        std::env::var("GEWY_AUTHORITY_ROOT").ok().as_deref(),
        Some("/srv/gewyvern/certificates/authorities")
    );
    assert_eq!(
        std::env::var("GEWY_IDENTITY_ROOT").ok().as_deref(),
        Some("/srv/gewyvern/certificates/identities")
    );
    assert_eq!(
        std::env::var("GEWY_CERTIFICATE_STATE_ROOT").ok().as_deref(),
        Some("/srv/gewyvern/state/certificates")
    );
    assert_eq!(
        std::env::var("GEWY_REQUIRE_EXPLICIT_REMOTE_TRUST")
            .ok()
            .as_deref(),
        Some("true")
    );
    assert_eq!(
        std::env::var("GEWY_API_ADMIN_TOKEN").ok().as_deref(),
        Some("runtime-api-token")
    );
    assert_eq!(
        std::env::var("GEWY_HISTORY_RETENTION").ok().as_deref(),
        Some("12")
    );
    assert_eq!(
        std::env::var("GEWY_EXTERNAL_FAILURE_CIRCUIT_THRESHOLD")
            .ok()
            .as_deref(),
        Some("5")
    );
    assert_eq!(
        std::env::var("GEWY_EXTERNAL_FAILURE_CIRCUIT_COOLDOWN_SECONDS")
            .ok()
            .as_deref(),
        Some("45")
    );
    assert_eq!(
        std::env::var("GEWY_SOCKET_FAILURE_BACKOFF_BASE_MS")
            .ok()
            .as_deref(),
        Some("150")
    );
    assert_eq!(
        std::env::var("GEWY_SOCKET_FAILURE_BACKOFF_CAP_MS")
            .ok()
            .as_deref(),
        Some("2500")
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn runtime_config_legacy_path_is_used_as_fallback() {
    let _lock = env_lock().lock().unwrap();
    let home = temp_dir("legacy-home");
    let legacy_root = home.join(".gewyvern");
    fs::create_dir_all(&legacy_root).unwrap();
    fs::write(
        legacy_root.join("config.toml"),
        "[runtime]\nserve = true\nsocket = \"tcp:127.0.0.1:9000\"\n",
    )
    .unwrap();
    let _home = EnvGuard::set("HOME", home.to_string_lossy());
    let _config_home = EnvGuard::remove("GEWY_CONFIG_HOME");
    let _config_file = EnvGuard::remove("GEWY_CONFIG_FILE");
    let _history_retention = EnvGuard::remove("GEWY_HISTORY_RETENTION");

    let config = load_runtime_config().unwrap();
    assert!(config.used_legacy_path);
    assert_eq!(config.schema_version, 1);
    assert!(!config.schema_version_explicit);
    assert_eq!(config.defaults.serve, Some(true));
    assert_eq!(
        config.defaults.socket_target,
        Some(SocketTarget::Tcp("127.0.0.1:9000".into()))
    );

    fs::remove_dir_all(&home).unwrap();
}

#[test]
fn runtime_config_rejects_future_schema_version() {
    let _lock = env_lock().lock().unwrap();
    let root = temp_dir("future-schema");
    let config_root = root.join("config");
    fs::create_dir_all(&config_root).unwrap();
    fs::write(
        config_root.join("gewyvern.toml"),
        "schema_version = 99\n[runtime]\nserve = true\n",
    )
    .unwrap();
    let _config_home = EnvGuard::set("GEWY_CONFIG_HOME", config_root.to_string_lossy());
    let _config_file = EnvGuard::remove("GEWY_CONFIG_FILE");

    let err = load_runtime_config().unwrap_err();
    assert!(err.contains("unsupported runtime config schema_version '99'"));

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn runtime_config_explicit_file_overrides_standard_path() {
    let _lock = env_lock().lock().unwrap();
    let root = temp_dir("explicit-file");
    let config_root = root.join("config");
    fs::create_dir_all(&config_root).unwrap();
    let explicit_path = root.join("explicit.toml");
    fs::write(
        config_root.join("gewyvern.toml"),
        "schema_version = 1\n[runtime]\nserve = false\nmax_sessions = 11\n",
    )
    .unwrap();
    fs::write(
        &explicit_path,
        "schema_version = 1\n[runtime]\nserve = true\nmax_sessions = 29\n",
    )
    .unwrap();
    let _config_home = EnvGuard::set("GEWY_CONFIG_HOME", config_root.to_string_lossy());
    let _config_file = EnvGuard::set("GEWY_CONFIG_FILE", explicit_path.to_string_lossy());

    let config = load_runtime_config().unwrap();
    assert_eq!(config.defaults.serve, Some(true));
    assert_eq!(config.defaults.max_sessions, Some(29));

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn runtime_config_rejects_unknown_section() {
    let _lock = env_lock().lock().unwrap();
    let root = temp_dir("unknown-section");
    let config_root = root.join("config");
    fs::create_dir_all(&config_root).unwrap();
    fs::write(
        config_root.join("gewyvern.toml"),
        "schema_version = 1\n[unknown]\nvalue = true\n",
    )
    .unwrap();
    let _config_home = EnvGuard::set("GEWY_CONFIG_HOME", config_root.to_string_lossy());
    let _config_file = EnvGuard::remove("GEWY_CONFIG_FILE");

    let err = load_runtime_config().unwrap_err();
    assert!(err.contains("unsupported runtime config section 'unknown'"));

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn cli_arguments_override_runtime_config_defaults() {
    let defaults = CliDefaults {
        serve: Some(true),
        socket_target: Some(SocketTarget::Unix("/tmp/default.sock".into())),
        api_socket: Some("127.0.0.1:9910".into()),
        max_sessions: Some(16),
        ingest_mode: Some(IngestMode::LocalAdvisory),
        log_level: Some(LogLevel::Warn),
        log_to_stderr: Some(false),
        log_file: Some("/tmp/default.log".into()),
        log_max_bytes: Some(8192),
        log_max_files: Some(5),
        ..CliDefaults::default()
    };

    let cli = Cli::from_args_with_defaults(
        [
            "--tcp-socket".to_string(),
            "127.0.0.1:9001".to_string(),
            "--ingest-mode".to_string(),
            "remote-advisory".to_string(),
            "--max-sessions".to_string(),
            "24".to_string(),
            "--log-level".to_string(),
            "debug".to_string(),
            "--log-stderr".to_string(),
            "--log-file".to_string(),
            "/tmp/cli.log".to_string(),
        ],
        defaults,
    )
    .unwrap();

    assert!(cli.serve);
    assert_eq!(
        cli.socket_target,
        Some(SocketTarget::Tcp("127.0.0.1:9001".into()))
    );
    assert_eq!(cli.ingest_mode, IngestMode::RemoteAdvisory);
    assert_eq!(cli.max_sessions, Some(24));
    assert_eq!(cli.api_socket.as_deref(), Some("127.0.0.1:9910"));
    assert_eq!(cli.log_level, LogLevel::Debug);
    assert!(cli.log_to_stderr);
    assert_eq!(cli.log_file.as_deref(), Some("/tmp/cli.log"));
    assert_eq!(cli.log_max_bytes, 8192);
    assert_eq!(cli.log_max_files, 5);
}
