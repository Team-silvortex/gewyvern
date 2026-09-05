use crate::cli::{CliDefaults, IngestMode, SocketTarget};
use crate::runtime_logging::LogLevel;
use gewyvern::runtime_layout::runtime_layout;
use silvortex_bounded_io::read_bounded_utf8_regular_file;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const DEFAULT_CONFIG_NAME: &str = "gewyvern.toml";
const LEGACY_CONFIG_NAME: &str = "config.toml";
const CURRENT_RUNTIME_CONFIG_SCHEMA_VERSION: usize = 1;
const EXTERNAL_FAILURE_CIRCUIT_THRESHOLD_ENV: &str = "GEWY_EXTERNAL_FAILURE_CIRCUIT_THRESHOLD";
const EXTERNAL_FAILURE_CIRCUIT_COOLDOWN_ENV: &str =
    "GEWY_EXTERNAL_FAILURE_CIRCUIT_COOLDOWN_SECONDS";
const PROTOCOL_REGISTRY_ROOT_ENV: &str = "GEWY_PROTOCOL_REGISTRY_ROOT";
const SHARE_ROOT_ENV: &str = "GEWY_SHARE_ROOT";
const CERTIFICATE_ROOT_ENV: &str = "GEWY_CERTIFICATE_ROOT";
const TRUST_ROOT_ENV: &str = "GEWY_TRUST_ROOT";
const AUTHORITY_ROOT_ENV: &str = "GEWY_AUTHORITY_ROOT";
const IDENTITY_ROOT_ENV: &str = "GEWY_IDENTITY_ROOT";
const CERTIFICATE_STATE_ROOT_ENV: &str = "GEWY_CERTIFICATE_STATE_ROOT";
const REQUIRE_EXPLICIT_REMOTE_TRUST_ENV: &str = "GEWY_REQUIRE_EXPLICIT_REMOTE_TRUST";
const SOCKET_FAILURE_BACKOFF_BASE_ENV: &str = "GEWY_SOCKET_FAILURE_BACKOFF_BASE_MS";
const SOCKET_FAILURE_BACKOFF_CAP_ENV: &str = "GEWY_SOCKET_FAILURE_BACKOFF_CAP_MS";
const RUNTIME_CONFIG_PATH_MAX_LEN: usize = 4096;
const MAX_RUNTIME_CONFIG_BYTES: u64 = 256 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RuntimeConfigFile {
    pub(crate) schema_version: usize,
    pub(crate) schema_version_explicit: bool,
    pub(crate) defaults: CliDefaults,
    pub(crate) history_retention: Option<usize>,
    pub(crate) protocol_registry_root: Option<String>,
    pub(crate) share_root: Option<String>,
    pub(crate) certificate_root: Option<String>,
    pub(crate) trust_root: Option<String>,
    pub(crate) authority_root: Option<String>,
    pub(crate) identity_root: Option<String>,
    pub(crate) certificate_state_root: Option<String>,
    pub(crate) require_explicit_remote_trust: Option<bool>,
    pub(crate) external_failure_circuit_threshold: Option<usize>,
    pub(crate) external_failure_circuit_cooldown_seconds: Option<usize>,
    pub(crate) socket_failure_backoff_base_ms: Option<usize>,
    pub(crate) socket_failure_backoff_cap_ms: Option<usize>,
    pub(crate) source_path: Option<PathBuf>,
    pub(crate) used_legacy_path: bool,
}

impl Default for RuntimeConfigFile {
    fn default() -> Self {
        Self {
            schema_version: CURRENT_RUNTIME_CONFIG_SCHEMA_VERSION,
            schema_version_explicit: false,
            defaults: CliDefaults::default(),
            history_retention: None,
            protocol_registry_root: None,
            share_root: None,
            certificate_root: None,
            trust_root: None,
            authority_root: None,
            identity_root: None,
            certificate_state_root: None,
            require_explicit_remote_trust: None,
            external_failure_circuit_threshold: None,
            external_failure_circuit_cooldown_seconds: None,
            socket_failure_backoff_base_ms: None,
            socket_failure_backoff_cap_ms: None,
            source_path: None,
            used_legacy_path: false,
        }
    }
}

pub(crate) fn load_runtime_config() -> Result<RuntimeConfigFile, String> {
    let Some(path) = select_runtime_config_path() else {
        return Ok(RuntimeConfigFile::default());
    };
    let input = read_bounded_utf8_regular_file(&path, MAX_RUNTIME_CONFIG_BYTES)
        .map_err(|err| format!("failed to read runtime config '{}': {err}", path.display()))?;
    let mut config = parse_runtime_config(&input)?;
    config.used_legacy_path = is_legacy_path(&path);
    config.source_path = Some(path);
    Ok(config)
}

pub(crate) fn apply_runtime_path_overrides(config: &RuntimeConfigFile) -> Result<(), String> {
    if let Some(retention) = config.history_retention
        && std::env::var_os("GEWY_HISTORY_RETENTION").is_none()
    {
        unsafe {
            std::env::set_var("GEWY_HISTORY_RETENTION", retention.to_string());
        }
    }
    if let Some(root) = config.protocol_registry_root.as_deref()
        && std::env::var_os("GEWY_PROTOCOL_REGISTRY_ROOT").is_none()
    {
        apply_env_string_override(PROTOCOL_REGISTRY_ROOT_ENV, Some(root))?;
    }
    if let Some(root) = config.share_root.as_deref()
        && std::env::var_os("GEWY_SHARE_ROOT").is_none()
    {
        apply_env_string_override(SHARE_ROOT_ENV, Some(root))?;
    }
    apply_env_string_override(CERTIFICATE_ROOT_ENV, config.certificate_root.as_deref())?;
    apply_env_string_override(TRUST_ROOT_ENV, config.trust_root.as_deref())?;
    apply_env_string_override(AUTHORITY_ROOT_ENV, config.authority_root.as_deref())?;
    apply_env_string_override(IDENTITY_ROOT_ENV, config.identity_root.as_deref())?;
    apply_env_string_override(
        CERTIFICATE_STATE_ROOT_ENV,
        config.certificate_state_root.as_deref(),
    )?;
    if let Some(value) = config.require_explicit_remote_trust
        && std::env::var_os(REQUIRE_EXPLICIT_REMOTE_TRUST_ENV).is_none()
    {
        unsafe {
            std::env::set_var(
                REQUIRE_EXPLICIT_REMOTE_TRUST_ENV,
                if value { "true" } else { "false" },
            );
        }
    }
    apply_env_usize_override(
        EXTERNAL_FAILURE_CIRCUIT_THRESHOLD_ENV,
        config.external_failure_circuit_threshold,
    );
    apply_env_usize_override(
        EXTERNAL_FAILURE_CIRCUIT_COOLDOWN_ENV,
        config.external_failure_circuit_cooldown_seconds,
    );
    apply_env_usize_override(
        SOCKET_FAILURE_BACKOFF_BASE_ENV,
        config.socket_failure_backoff_base_ms,
    );
    apply_env_usize_override(
        SOCKET_FAILURE_BACKOFF_CAP_ENV,
        config.socket_failure_backoff_cap_ms,
    );
    Ok(())
}

fn apply_env_string_override(key: &str, value: Option<&str>) -> Result<(), String> {
    if let Some(value) = value
        && std::env::var_os(key).is_none()
    {
        validate_runtime_config_path_value(key, value)?;
        unsafe {
            std::env::set_var(key, value);
        }
    }
    Ok(())
}

fn apply_env_usize_override(key: &str, value: Option<usize>) {
    if let Some(value) = value
        && std::env::var_os(key).is_none()
    {
        unsafe {
            std::env::set_var(key, value.to_string());
        }
    }
}

fn validate_runtime_config_path_value(name: &str, value: &str) -> Result<(), String> {
    if value.trim() != value {
        return Err(format!(
            "{name} must not contain leading or trailing whitespace"
        ));
    }
    if value.is_empty() {
        return Err(format!("{name} must not be empty"));
    }
    if value.len() > RUNTIME_CONFIG_PATH_MAX_LEN {
        return Err(format!("{name} path is too long"));
    }
    if value.chars().any(|character| character.is_ascii_control()) {
        return Err(format!("{name} contains invalid control characters"));
    }
    Ok(())
}

fn select_runtime_config_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("GEWY_CONFIG_FILE").and_then(validate_config_file_path) {
        return Some(path);
    }
    let layout = runtime_layout();
    let standard = layout.config_root.join(DEFAULT_CONFIG_NAME);
    if standard.exists() {
        return Some(standard);
    }
    let legacy_root = layout.legacy_root?;
    let legacy_primary = legacy_root.join(LEGACY_CONFIG_NAME);
    if legacy_primary.exists() {
        return Some(legacy_primary);
    }
    let legacy_named = legacy_root.join(DEFAULT_CONFIG_NAME);
    if legacy_named.exists() {
        return Some(legacy_named);
    }
    None
}

fn validate_config_file_path(value: std::ffi::OsString) -> Option<PathBuf> {
    let path = value.to_str()?;
    validate_runtime_config_path_value("GEWY_CONFIG_FILE", path).ok()?;
    Some(PathBuf::from(path))
}

fn is_legacy_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == LEGACY_CONFIG_NAME)
        || path
            .parent()
            .and_then(|parent| parent.file_name())
            .and_then(|name| name.to_str())
            .is_some_and(|name| name == ".gewyvern")
}

fn parse_runtime_config(input: &str) -> Result<RuntimeConfigFile, String> {
    let sections = parse_sections(input)?;
    let mut config = RuntimeConfigFile::default();
    if let Some(root) = sections.get("") {
        apply_root_section(root, &mut config)?;
    }
    if let Some(runtime) = sections.get("runtime") {
        apply_runtime_section(runtime, &mut config)?;
    }
    if let Some(external) = sections.get("external_engine") {
        apply_external_engine_section(external, &mut config)?;
    }
    if let Some(paths) = sections.get("paths") {
        apply_paths_section(paths, &mut config)?;
    }
    if let Some(certificates) = sections.get("certificates") {
        apply_certificates_section(certificates, &mut config)?;
    }
    if let Some(logging) = sections.get("logging") {
        apply_logging_section(logging, &mut config)?;
    }
    if let Some(resilience) = sections.get("resilience") {
        apply_resilience_section(resilience, &mut config)?;
    }
    for section in sections.keys() {
        if section.is_empty() {
            continue;
        }
        if !matches!(
            section.as_str(),
            "runtime" | "external_engine" | "paths" | "certificates" | "logging" | "resilience"
        ) {
            return Err(format!("unsupported runtime config section '{section}'"));
        }
    }
    Ok(config)
}

fn apply_root_section(
    root: &BTreeMap<String, String>,
    config: &mut RuntimeConfigFile,
) -> Result<(), String> {
    for (key, value) in root {
        match key.as_str() {
            "schema_version" => {
                let version = parse_positive_usize(value, "schema_version")?;
                if version > CURRENT_RUNTIME_CONFIG_SCHEMA_VERSION {
                    return Err(format!(
                        "unsupported runtime config schema_version '{version}'; current supported version is {}",
                        CURRENT_RUNTIME_CONFIG_SCHEMA_VERSION
                    ));
                }
                config.schema_version = version;
                config.schema_version_explicit = true;
            }
            other => {
                return Err(format!("unsupported runtime config key '{other}'"));
            }
        }
    }
    Ok(())
}

fn apply_certificates_section(
    certificates: &BTreeMap<String, String>,
    config: &mut RuntimeConfigFile,
) -> Result<(), String> {
    for (key, value) in certificates {
        match key.as_str() {
            "root" => config.certificate_root = Some(parse_string(value)),
            "trust_root" => config.trust_root = Some(parse_string(value)),
            "authority_root" => config.authority_root = Some(parse_string(value)),
            "identity_root" => config.identity_root = Some(parse_string(value)),
            "state_root" => config.certificate_state_root = Some(parse_string(value)),
            "require_explicit_remote_trust" => {
                config.require_explicit_remote_trust = Some(parse_bool(
                    value,
                    "certificates.require_explicit_remote_trust",
                )?)
            }
            other => {
                return Err(format!(
                    "unsupported runtime config key 'certificates.{other}'"
                ));
            }
        }
    }
    Ok(())
}

fn apply_runtime_section(
    runtime: &BTreeMap<String, String>,
    config: &mut RuntimeConfigFile,
) -> Result<(), String> {
    for (key, value) in runtime {
        match key.as_str() {
            "serve" => config.defaults.serve = Some(parse_bool(value, "runtime.serve")?),
            "socket" => config.defaults.socket_target = Some(parse_socket_target(value)?),
            "api_socket" => config.defaults.api_socket = Some(parse_string(value)),
            "allow_remote_api" => {
                config.defaults.allow_remote_api =
                    Some(parse_bool(value, "runtime.allow_remote_api")?)
            }
            "api_admin_token" => config.defaults.api_admin_token = Some(parse_string(value)),
            "ingest_mode" => {
                config.defaults.ingest_mode = Some(
                    IngestMode::from_str(&parse_string(value))
                        .map_err(|err| format!("invalid runtime.ingest_mode in config: {err}"))?,
                )
            }
            "max_sessions" => {
                config.defaults.max_sessions = Some(parse_usize(value, "runtime.max_sessions")?)
            }
            "history_retention" => {
                config.history_retention = Some(parse_usize(value, "runtime.history_retention")?)
            }
            other => return Err(format!("unsupported runtime config key 'runtime.{other}'")),
        }
    }
    Ok(())
}

fn apply_external_engine_section(
    external: &BTreeMap<String, String>,
    config: &mut RuntimeConfigFile,
) -> Result<(), String> {
    for (key, value) in external {
        match key.as_str() {
            "bin" => {
                config.defaults.external_engine_bin =
                    Some(parse_external_engine_path(value, "external_engine.bin")?)
            }
            "worker" => {
                config.defaults.external_engine_worker =
                    Some(parse_external_engine_path(value, "external_engine.worker")?)
            }
            "python_bin" => {
                config.defaults.external_engine_python_bin = Some(parse_external_engine_path(
                    value,
                    "external_engine.python_bin",
                )?)
            }
            other => {
                return Err(format!(
                    "unsupported runtime config key 'external_engine.{other}'"
                ));
            }
        }
    }
    Ok(())
}

fn apply_paths_section(
    paths: &BTreeMap<String, String>,
    config: &mut RuntimeConfigFile,
) -> Result<(), String> {
    for (key, value) in paths {
        match key.as_str() {
            "protocol_registry_root" => config.protocol_registry_root = Some(parse_string(value)),
            "share_root" => config.share_root = Some(parse_string(value)),
            other => return Err(format!("unsupported runtime config key 'paths.{other}'")),
        }
    }
    Ok(())
}

fn apply_logging_section(
    logging: &BTreeMap<String, String>,
    config: &mut RuntimeConfigFile,
) -> Result<(), String> {
    for (key, value) in logging {
        match key.as_str() {
            "level" => {
                config.defaults.log_level = Some(
                    LogLevel::from_str(&parse_string(value))
                        .map_err(|err| format!("invalid logging.level in config: {err}"))?,
                )
            }
            "stderr" => config.defaults.log_to_stderr = Some(parse_bool(value, "logging.stderr")?),
            "file" => {
                let path = parse_string(value);
                validate_runtime_config_path_value("logging.file", &path)?;
                config.defaults.log_file = Some(path);
            }
            "max_bytes" => {
                config.defaults.log_max_bytes =
                    Some(parse_positive_usize(value, "logging.max_bytes")?)
            }
            "max_files" => {
                config.defaults.log_max_files = Some(parse_usize(value, "logging.max_files")?)
            }
            other => return Err(format!("unsupported runtime config key 'logging.{other}'")),
        }
    }
    Ok(())
}

fn apply_resilience_section(
    resilience: &BTreeMap<String, String>,
    config: &mut RuntimeConfigFile,
) -> Result<(), String> {
    for (key, value) in resilience {
        match key.as_str() {
            "external_failure_circuit_threshold" => {
                config.external_failure_circuit_threshold = Some(parse_positive_usize(
                    value,
                    "resilience.external_failure_circuit_threshold",
                )?)
            }
            "external_failure_circuit_cooldown_seconds" => {
                config.external_failure_circuit_cooldown_seconds = Some(parse_positive_usize(
                    value,
                    "resilience.external_failure_circuit_cooldown_seconds",
                )?)
            }
            "socket_failure_backoff_base_ms" => {
                config.socket_failure_backoff_base_ms = Some(parse_positive_usize(
                    value,
                    "resilience.socket_failure_backoff_base_ms",
                )?)
            }
            "socket_failure_backoff_cap_ms" => {
                config.socket_failure_backoff_cap_ms = Some(parse_positive_usize(
                    value,
                    "resilience.socket_failure_backoff_cap_ms",
                )?)
            }
            other => {
                return Err(format!(
                    "unsupported runtime config key 'resilience.{other}'"
                ));
            }
        }
    }
    Ok(())
}

fn parse_sections(input: &str) -> Result<BTreeMap<String, BTreeMap<String, String>>, String> {
    let mut sections = BTreeMap::<String, BTreeMap<String, String>>::new();
    let mut current = String::new();
    sections.insert(current.clone(), BTreeMap::new());
    for (line_no, raw_line) in input.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            current = line[1..line.len() - 1].trim().to_string();
            sections.entry(current.clone()).or_default();
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            return Err(format!(
                "invalid runtime config line {}: expected key = value",
                line_no + 1
            ));
        };
        let key = key.trim();
        if key.is_empty() {
            return Err(format!(
                "invalid runtime config line {}: empty key",
                line_no + 1
            ));
        }
        sections
            .entry(current.clone())
            .or_default()
            .insert(key.to_string(), value.trim().to_string());
    }
    Ok(sections)
}

fn parse_string(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2 {
        trimmed[1..trimmed.len() - 1].to_string()
    } else {
        trimmed.to_string()
    }
}

fn parse_external_engine_path(value: &str, field: &str) -> Result<String, String> {
    let value = parse_string(value);
    if value.trim().is_empty() || value.starts_with("--") {
        return Err(format!("{field} must be a non-empty path"));
    }
    if value.chars().any(char::is_control) {
        return Err(format!("{field} must not contain control characters"));
    }
    if !value.contains('/') && !value.contains('\\') {
        return Err(format!(
            "{field} must be a filesystem path (for example ./engine or /usr/bin/engine)"
        ));
    }
    Ok(value)
}

fn parse_bool(value: &str, context: &str) -> Result<bool, String> {
    match parse_string(value).as_str() {
        "true" => Ok(true),
        "false" => Ok(false),
        other => Err(format!("{context} must be true or false, got '{other}'")),
    }
}

fn parse_usize(value: &str, context: &str) -> Result<usize, String> {
    parse_string(value)
        .parse::<usize>()
        .map_err(|_| format!("{context} must be a positive integer"))
}

fn parse_positive_usize(value: &str, context: &str) -> Result<usize, String> {
    let parsed = parse_usize(value, context)?;
    if parsed == 0 {
        return Err(format!("{context} must be greater than zero"));
    }
    Ok(parsed)
}

fn parse_socket_target(value: &str) -> Result<SocketTarget, String> {
    let value = parse_string(value);
    if let Some(path) = value.strip_prefix("unix:") {
        return Ok(SocketTarget::Unix(path.to_string()));
    }
    if let Some(addr) = value.strip_prefix("tcp:") {
        return Ok(SocketTarget::Tcp(addr.to_string()));
    }
    Err("runtime.socket must start with 'unix:' or 'tcp:'".to_string())
}
