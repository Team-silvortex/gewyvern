use std::collections::{HashMap, VecDeque, hash_map::DefaultHasher};
use std::fs;
use std::hash::{Hash, Hasher};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::diagnosis_runtime::{AnalysisAugmentation, AnalysisSnapshot, analysis_snapshot_json};
use crate::runtime_events::{
    EVENT_EXTERNAL_ANALYSIS_CIRCUIT_OPEN, EVENT_EXTERNAL_ANALYSIS_FAILED,
    EVENT_EXTERNAL_ANALYSIS_RECOVERED,
};
use crate::runtime_logging::{log_info_event, log_warn_event};

mod capabilities;
mod parse;

use self::capabilities::{ExternalCapabilityProfile, parse_external_capability_profile};
use self::parse::{parse_external_augmentations, single_json_string_field};

const EXTERNAL_ENGINE_TIMEOUT: Duration = Duration::from_secs(5);
const EXTERNAL_CAPABILITY_RETRY_DELAY: Duration = Duration::from_secs(5);
const MAX_EXTERNAL_ENGINE_STDOUT_BYTES: usize = 1024 * 1024;
const MAX_EXTERNAL_ENGINE_STDERR_BYTES: usize = 256 * 1024;
const MAX_EXTERNAL_CACHE_ENTRIES: usize = 128;
const MAX_EXTERNAL_AUGMENTATIONS: usize = 256;
const EXTERNAL_FAILURE_LOG_EVERY: usize = 10;
const DEFAULT_EXTERNAL_FAILURE_CIRCUIT_THRESHOLD: usize = 3;
const DEFAULT_EXTERNAL_FAILURE_CIRCUIT_COOLDOWN_SECONDS: u64 = 30;
const EXTERNAL_FAILURE_CIRCUIT_THRESHOLD_ENV: &str = "GEWY_EXTERNAL_FAILURE_CIRCUIT_THRESHOLD";
const EXTERNAL_FAILURE_CIRCUIT_COOLDOWN_ENV: &str =
    "GEWY_EXTERNAL_FAILURE_CIRCUIT_COOLDOWN_SECONDS";

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct ExternalAnalysisConfig {
    pub(crate) engine_bin: String,
    pub(crate) python_worker: Option<String>,
    pub(crate) python_bin: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct SnapshotCacheKey {
    len: usize,
    hash: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct ExternalResilienceStatus {
    pub(crate) consecutive_failures: usize,
    pub(crate) total_failures: usize,
    pub(crate) circuit_open: bool,
    pub(crate) cooldown_remaining_ms: u128,
    pub(crate) circuit_threshold: usize,
    pub(crate) circuit_cooldown_seconds: u64,
}

#[derive(Default)]
struct ExternalAnalysisState {
    config: Option<ExternalAnalysisConfig>,
    capabilities: CachedCapabilities,
    circuit_open_until: Option<Instant>,
    cache: HashMap<SnapshotCacheKey, Vec<AnalysisAugmentation>>,
    cache_order: VecDeque<SnapshotCacheKey>,
}

#[derive(Clone, Debug, Default)]
enum CachedCapabilities {
    #[default]
    Unknown,
    Loaded(ExternalCapabilityProfile),
    Unavailable {
        retry_at: Instant,
    },
}

impl CachedCapabilities {
    fn is_current(&self, now: Instant) -> bool {
        match self {
            Self::Loaded(_) => true,
            Self::Unavailable { retry_at } => now < *retry_at,
            Self::Unknown => false,
        }
    }

    fn profile(&self) -> Option<&ExternalCapabilityProfile> {
        match self {
            Self::Loaded(profile) => Some(profile),
            Self::Unknown | Self::Unavailable { .. } => None,
        }
    }
}

#[cfg(test)]
thread_local! {
    static TEST_EXTERNAL_ANALYSIS_CONFIG: std::cell::RefCell<Option<ExternalAnalysisConfig>> =
        const { std::cell::RefCell::new(None) };
}

fn state() -> &'static Mutex<ExternalAnalysisState> {
    static STATE: OnceLock<Mutex<ExternalAnalysisState>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(ExternalAnalysisState::default()))
}

fn lock_state() -> std::sync::MutexGuard<'static, ExternalAnalysisState> {
    state()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn external_failure_counter() -> &'static AtomicUsize {
    static COUNTER: OnceLock<AtomicUsize> = OnceLock::new();
    COUNTER.get_or_init(|| AtomicUsize::new(0))
}

fn external_total_failure_counter() -> &'static AtomicUsize {
    static COUNTER: OnceLock<AtomicUsize> = OnceLock::new();
    COUNTER.get_or_init(|| AtomicUsize::new(0))
}

fn external_failure_circuit_threshold() -> usize {
    std::env::var(EXTERNAL_FAILURE_CIRCUIT_THRESHOLD_ENV)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_EXTERNAL_FAILURE_CIRCUIT_THRESHOLD)
}

fn external_failure_circuit_cooldown() -> Duration {
    Duration::from_secs(
        std::env::var(EXTERNAL_FAILURE_CIRCUIT_COOLDOWN_ENV)
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(DEFAULT_EXTERNAL_FAILURE_CIRCUIT_COOLDOWN_SECONDS),
    )
}

pub(crate) fn current_external_resilience_status() -> ExternalResilienceStatus {
    let now = Instant::now();
    let guard = lock_state();
    let cooldown_remaining_ms = guard
        .circuit_open_until
        .filter(|until| *until > now)
        .map(|until| until.duration_since(now).as_millis())
        .unwrap_or(0);
    let cooldown = external_failure_circuit_cooldown();
    ExternalResilienceStatus {
        consecutive_failures: external_failure_counter().load(Ordering::Acquire),
        total_failures: external_total_failure_counter().load(Ordering::Acquire),
        circuit_open: cooldown_remaining_ms > 0,
        cooldown_remaining_ms,
        circuit_threshold: external_failure_circuit_threshold(),
        circuit_cooldown_seconds: cooldown.as_secs(),
    }
}

fn external_fallback_augmentations(error: &str) -> Vec<AnalysisAugmentation> {
    vec![AnalysisAugmentation {
        kind: "external-engine".into(),
        name: "external_engine_failed".into(),
        summary: "external analysis engine failed; keeping built-in analysis only".into(),
        confidence: "advisory".into(),
        producer_stage: Some("external".into()),
        producer_pass: Some("external-engine-hook".into()),
        data_json: Some(single_json_string_field("message", error)),
    }]
}

fn current_external_circuit_block(engine_bin: &str) -> Option<String> {
    let now = Instant::now();
    let mut guard = lock_state();
    match guard.circuit_open_until {
        Some(until) if until > now => Some(format!(
            "external engine '{}' temporarily bypassed for another {}s after repeated failures",
            engine_bin,
            until.duration_since(now).as_secs().max(1)
        )),
        Some(_) => {
            guard.circuit_open_until = None;
            None
        }
        None => None,
    }
}

fn note_external_analysis_failure(engine_bin: &str, error: &str) {
    let consecutive = external_failure_counter().fetch_add(1, Ordering::AcqRel) + 1;
    let total = external_total_failure_counter().fetch_add(1, Ordering::AcqRel) + 1;
    let threshold = external_failure_circuit_threshold();
    let cooldown = external_failure_circuit_cooldown();
    if consecutive >= threshold {
        let mut guard = lock_state();
        let now = Instant::now();
        let should_open = !matches!(guard.circuit_open_until, Some(until) if until > now);
        if should_open {
            guard.circuit_open_until = Some(now + cooldown);
            log_warn_event(
                "external_analysis",
                EVENT_EXTERNAL_ANALYSIS_CIRCUIT_OPEN,
                &[
                    ("engine", engine_bin.to_string()),
                    ("threshold", threshold.to_string()),
                    ("cooldown_seconds", cooldown.as_secs().to_string()),
                    ("error", error.to_string()),
                ],
                "external analysis circuit opened after repeated failures",
            );
        }
    }
    if consecutive == 1
        || consecutive == 3
        || consecutive.is_multiple_of(EXTERNAL_FAILURE_LOG_EVERY)
    {
        log_warn_event(
            "external_analysis",
            EVENT_EXTERNAL_ANALYSIS_FAILED,
            &[
                ("engine", engine_bin.to_string()),
                ("consecutive_failures", consecutive.to_string()),
                ("total_failures", total.to_string()),
                ("error", error.to_string()),
            ],
            "external analysis degraded; using built-in analysis fallback",
        );
    }
}

fn note_external_analysis_success(engine_bin: &str) {
    let recovered = external_failure_counter().swap(0, Ordering::AcqRel);
    let mut guard = lock_state();
    guard.circuit_open_until = None;
    if recovered > 0 {
        log_info_event(
            "external_analysis",
            EVENT_EXTERNAL_ANALYSIS_RECOVERED,
            &[
                ("engine", engine_bin.to_string()),
                ("recovered_after_failures", recovered.to_string()),
            ],
            "external analysis recovered after prior failures",
        );
    }
}

pub(crate) fn set_external_analysis_config(config: Option<ExternalAnalysisConfig>) {
    #[cfg(test)]
    {
        TEST_EXTERNAL_ANALYSIS_CONFIG.with(|slot| {
            *slot.borrow_mut() = config;
        });
        return;
    }

    #[allow(unreachable_code)]
    {
        let mut guard = lock_state();
        guard.config = config;
        guard.capabilities = CachedCapabilities::Unknown;
        guard.circuit_open_until = None;
        guard.cache.clear();
        guard.cache_order.clear();
    }
}

pub(crate) fn append_external_augmentations(snapshot: &mut AnalysisSnapshot) {
    #[cfg(test)]
    {
        let config = TEST_EXTERNAL_ANALYSIS_CONFIG.with(|slot| slot.borrow().clone());
        let Some(config) = config else {
            return;
        };
        if let Some(reason) = current_external_circuit_block(&config.engine_bin) {
            snapshot
                .augmentations
                .extend(external_fallback_augmentations(&reason));
            return;
        }
        let snapshot_json = analysis_snapshot_json(snapshot);
        let capabilities = query_external_capabilities(&config);
        let items = run_external_analysis(&config, capabilities.as_ref(), &snapshot_json)
            .inspect(|_| {
                note_external_analysis_success(&config.engine_bin);
            })
            .unwrap_or_else(|err| {
                note_external_analysis_failure(&config.engine_bin, &err);
                external_fallback_augmentations(&err)
            });
        snapshot.augmentations.extend(items);
        return;
    }

    #[allow(unreachable_code)]
    {
        let Some(config) = current_external_analysis_config() else {
            return;
        };
        let snapshot_json = analysis_snapshot_json(snapshot);
        let cache_key = snapshot_cache_key(&snapshot_json);
        let cached = {
            let guard = lock_state();
            guard
                .cache
                .get(&cache_key)
                .filter(|_| {
                    guard.config.as_ref() == Some(&config)
                        && guard.capabilities.is_current(Instant::now())
                })
                .cloned()
        };
        let items = if let Some(items) = cached {
            items
        } else if let Some(reason) = current_external_circuit_block(&config.engine_bin) {
            external_fallback_augmentations(&reason)
        } else {
            let capabilities = capability_profile_for_config(&config);
            match run_external_analysis(&config, capabilities.as_ref(), &snapshot_json) {
                Ok(items) => {
                    note_external_analysis_success(&config.engine_bin);
                    let mut guard = lock_state();
                    if guard.config.as_ref() == Some(&config)
                        && guard.capabilities.profile() == capabilities.as_ref()
                    {
                        if !guard.cache.contains_key(&cache_key) {
                            guard.cache_order.push_back(cache_key);
                        }
                        guard.cache.insert(cache_key, items.clone());
                        while guard.cache_order.len() > MAX_EXTERNAL_CACHE_ENTRIES {
                            if let Some(evicted) = guard.cache_order.pop_front() {
                                guard.cache.remove(&evicted);
                            }
                        }
                    }
                    items
                }
                Err(err) => {
                    // A fallback is an observation of failure, not a reusable analysis result.
                    note_external_analysis_failure(&config.engine_bin, &err);
                    external_fallback_augmentations(&err)
                }
            }
        };
        snapshot.augmentations.extend(items);
    }
}

fn run_external_analysis(
    config: &ExternalAnalysisConfig,
    capabilities: Option<&ExternalCapabilityProfile>,
    snapshot_json: &str,
) -> Result<Vec<AnalysisAugmentation>, String> {
    validate_external_analysis_binary(&config.engine_bin)?;
    if let Some(worker) = config.python_worker.as_deref() {
        validate_external_analysis_path_argument(worker)?;
    }
    if let Some(python_bin) = config.python_bin.as_deref() {
        validate_external_analysis_path_argument(python_bin)?;
    }
    let mut command = Command::new(&config.engine_bin);
    if let Some(worker) = config.python_worker.as_deref() {
        command.arg("analyze-python-json").arg("-");
        command.arg("--python-worker").arg(worker);
        if let Some(python_bin) = config.python_bin.as_deref() {
            command.arg("--python-bin").arg(python_bin);
        }
    } else {
        command.arg("analyze-json").arg("-");
    }
    let output = run_external_command(
        command,
        Some(snapshot_json.as_bytes()),
        EXTERNAL_ENGINE_TIMEOUT,
        MAX_EXTERNAL_ENGINE_STDOUT_BYTES,
        MAX_EXTERNAL_ENGINE_STDERR_BYTES,
    )
    .map_err(|err| {
        format!(
            "failed to launch external engine '{}': {err}",
            config.engine_bin
        )
    })?;
    let status = output.status;
    let stdout = output.stdout;
    let stderr = output.stderr;
    if !status.success() {
        let stderr = String::from_utf8_lossy(&stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            format!("external engine exited with {}", status)
        } else {
            format!("external engine exited with {}: {}", status, stderr)
        });
    }
    parse_external_augmentations(&String::from_utf8_lossy(&stdout), capabilities)
}

fn current_external_analysis_config() -> Option<ExternalAnalysisConfig> {
    let guard = lock_state();
    guard.config.clone()
}

fn capability_profile_for_config(
    config: &ExternalAnalysisConfig,
) -> Option<ExternalCapabilityProfile> {
    let cached = {
        let guard = lock_state();
        guard.capabilities.clone()
    };
    if cached.is_current(Instant::now()) {
        return cached.profile().cloned();
    }
    let profile = query_external_capabilities(config);
    let mut guard = lock_state();
    if guard.config.as_ref() == Some(config) {
        // Cached augmentations contain capability-adjusted metadata; never reuse
        // that old downgrade after refreshing a failed handshake.
        guard.cache.clear();
        guard.cache_order.clear();
        guard.capabilities = match profile.clone() {
            Some(profile) => CachedCapabilities::Loaded(profile),
            None => CachedCapabilities::Unavailable {
                retry_at: Instant::now() + EXTERNAL_CAPABILITY_RETRY_DELAY,
            },
        };
    }
    profile
}

fn query_external_capabilities(
    config: &ExternalAnalysisConfig,
) -> Option<ExternalCapabilityProfile> {
    if let Err(error) = validate_external_analysis_binary(&config.engine_bin) {
        log_warn_event(
            "external_analysis",
            EVENT_EXTERNAL_ANALYSIS_FAILED,
            &[
                ("engine", config.engine_bin.to_string()),
                ("error", error.to_string()),
            ],
            "external analysis capabilities query skipped due invalid engine path",
        );
        return None;
    }
    let mut command = Command::new(&config.engine_bin);
    command.arg("protocol-capabilities");
    let output = run_external_command(
        command,
        None,
        EXTERNAL_ENGINE_TIMEOUT,
        MAX_EXTERNAL_ENGINE_STDOUT_BYTES,
        MAX_EXTERNAL_ENGINE_STDERR_BYTES,
    )
    .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_external_capability_profile(&String::from_utf8_lossy(&output.stdout)).ok()
}

type ExternalCommandOutput = std::process::Output;

fn run_external_command(
    mut command: Command,
    stdin_payload: Option<&[u8]>,
    timeout: Duration,
    max_stdout_bytes: usize,
    max_stderr_bytes: usize,
) -> Result<ExternalCommandOutput, String> {
    if stdin_payload.is_none() {
        command.stdin(Stdio::null());
    }
    silvortex_bounded_io::process::run_command_output(
        &mut command,
        stdin_payload,
        timeout,
        silvortex_bounded_io::process::OutputLimits::new(max_stdout_bytes, max_stderr_bytes),
        "external engine",
    )
    .map_err(|error| error.to_string())
}

fn validate_external_analysis_binary(path: &str) -> Result<(), String> {
    validate_external_analysis_path_argument(path)?;
    #[cfg(unix)]
    {
        let metadata = fs::symlink_metadata(Path::new(path)).map_err(|error| {
            format!("failed to access external analysis engine '{path}': {error}")
        })?;
        if metadata.file_type().is_symlink() {
            return Err("external analysis engine path must not be a symlink".into());
        }
        if !metadata.is_file() {
            return Err("external analysis engine path must be a file".into());
        }
        if metadata.permissions().mode() & 0o111 == 0 {
            return Err("external analysis engine path is not executable".into());
        }
    }
    Ok(())
}

fn validate_external_analysis_path_argument(path: &str) -> Result<(), String> {
    if path.as_bytes().contains(&b'\0') {
        return Err("external analysis engine path contains invalid character".into());
    }
    if path.chars().any(|character| character.is_ascii_control()) {
        return Err("external analysis path contains control characters".into());
    }
    if !looks_like_path(path) {
        return Err(
            "external analysis path must reference a filesystem path, not a PATH executable name"
                .into(),
        );
    }

    if path.trim() != path {
        return Err(
            "external analysis path must not contain leading or trailing whitespace".into(),
        );
    }
    if path.is_empty() {
        return Err("external analysis path must not be empty".into());
    }
    #[cfg(unix)]
    {
        let metadata = fs::symlink_metadata(Path::new(path)).map_err(|error| {
            format!("failed to access external analysis engine '{path}': {error}")
        })?;
        if metadata.file_type().is_symlink() {
            return Err("external analysis path must not be a symlink".into());
        }
        if !metadata.is_file() {
            return Err("external analysis path must be a file".into());
        }
        if metadata.permissions().mode() & 0o111 == 0 {
            return Err("external analysis path is not executable".into());
        }
    }
    Ok(())
}

fn looks_like_path(value: &str) -> bool {
    value.contains('/') || value.contains('\\')
}

fn snapshot_cache_key(snapshot_json: &str) -> SnapshotCacheKey {
    let mut hasher = DefaultHasher::new();
    snapshot_json.hash(&mut hasher);
    SnapshotCacheKey {
        len: snapshot_json.len(),
        hash: hasher.finish(),
    }
}

#[cfg(test)]
pub(crate) fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static TEST_GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    TEST_GUARD
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
pub(crate) fn reset_external_fault_state() {
    external_failure_counter().store(0, Ordering::Release);
    external_total_failure_counter().store(0, Ordering::Release);
    let mut guard = lock_state();
    guard.circuit_open_until = None;
}

#[cfg(test)]
mod tests;
