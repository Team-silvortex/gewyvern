use crate::cli::{CliDefaults, IngestMode, SocketTarget};
use crate::runtime_logging::LogLevel;
use gewyvern::runtime_layout::runtime_layout;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

const DEFAULT_CONFIG_NAME: &str = "gewyvern.toml";
const LEGACY_CONFIG_NAME: &str = "config.toml";
const EXTERNAL_FAILURE_CIRCUIT_THRESHOLD_ENV: &str = "GEWY_EXTERNAL_FAILURE_CIRCUIT_THRESHOLD";
const EXTERNAL_FAILURE_CIRCUIT_COOLDOWN_ENV: &str =
    "GEWY_EXTERNAL_FAILURE_CIRCUIT_COOLDOWN_SECONDS";
const SOCKET_FAILURE_BACKOFF_BASE_ENV: &str = "GEWY_SOCKET_FAILURE_BACKOFF_BASE_MS";
const SOCKET_FAILURE_BACKOFF_CAP_ENV: &str = "GEWY_SOCKET_FAILURE_BACKOFF_CAP_MS";

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct RuntimeConfigFile {
    pub(crate) defaults: CliDefaults,
    pub(crate) history_retention: Option<usize>,
    pub(crate) protocol_registry_root: Option<String>,
    pub(crate) share_root: Option<String>,
    pub(crate) external_failure_circuit_threshold: Option<usize>,
    pub(crate) external_failure_circuit_cooldown_seconds: Option<usize>,
    pub(crate) socket_failure_backoff_base_ms: Option<usize>,
    pub(crate) socket_failure_backoff_cap_ms: Option<usize>,
    pub(crate) source_path: Option<PathBuf>,
    pub(crate) used_legacy_path: bool,
}

pub(crate) fn load_runtime_config() -> Result<RuntimeConfigFile, String> {
    let Some(path) = select_runtime_config_path() else {
        return Ok(RuntimeConfigFile::default());
    };
    let input = fs::read_to_string(&path)
        .map_err(|err| format!("failed to read runtime config '{}': {err}", path.display()))?;
    let mut config = parse_runtime_config(&input)?;
    config.used_legacy_path = is_legacy_path(&path);
    config.source_path = Some(path);
    Ok(config)
}

pub(crate) fn apply_runtime_path_overrides(config: &RuntimeConfigFile) {
    if let Some(retention) = config.history_retention {
        if std::env::var_os("GEWY_HISTORY_RETENTION").is_none() {
            unsafe {
                std::env::set_var("GEWY_HISTORY_RETENTION", retention.to_string());
            }
        }
    }
    if let Some(root) = config.protocol_registry_root.as_deref() {
        if std::env::var_os("GEWY_PROTOCOL_REGISTRY_ROOT").is_none() {
            unsafe {
                std::env::set_var("GEWY_PROTOCOL_REGISTRY_ROOT", root);
            }
        }
    }
    if let Some(root) = config.share_root.as_deref() {
        if std::env::var_os("GEWY_SHARE_ROOT").is_none() {
            unsafe {
                std::env::set_var("GEWY_SHARE_ROOT", root);
            }
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
}

fn apply_env_usize_override(key: &str, value: Option<usize>) {
    if let Some(value) = value {
        if std::env::var_os(key).is_none() {
            unsafe {
                std::env::set_var(key, value.to_string());
            }
        }
    }
}

fn select_runtime_config_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("GEWY_CONFIG_FILE").map(PathBuf::from) {
        return Some(path);
    }
    let layout = runtime_layout();
    let standard = layout.config_root.join(DEFAULT_CONFIG_NAME);
    if standard.exists() {
        return Some(standard);
    }
    let Some(legacy_root) = layout.legacy_root else {
        return None;
    };
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
    if let Some(runtime) = sections.get("runtime") {
        apply_runtime_section(runtime, &mut config)?;
    }
    if let Some(external) = sections.get("external_engine") {
        apply_external_engine_section(external, &mut config)?;
    }
    if let Some(paths) = sections.get("paths") {
        apply_paths_section(paths, &mut config)?;
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
            "runtime" | "external_engine" | "paths" | "logging" | "resilience"
        ) {
            return Err(format!("unsupported runtime config section '{section}'"));
        }
    }
    Ok(config)
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
            "bin" => config.defaults.external_engine_bin = Some(parse_string(value)),
            "worker" => config.defaults.external_engine_worker = Some(parse_string(value)),
            "python_bin" => config.defaults.external_engine_python_bin = Some(parse_string(value)),
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
                if path.is_empty() {
                    return Err("logging.file must not be empty".to_string());
                }
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
