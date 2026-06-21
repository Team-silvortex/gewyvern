use std::collections::{HashMap, VecDeque, hash_map::DefaultHasher};
use std::hash::{Hash, Hasher};
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use crate::diagnosis_runtime::{AnalysisAugmentation, AnalysisSnapshot};
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
    Loaded(Option<ExternalCapabilityProfile>),
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
    if consecutive == 1 || consecutive == 3 || consecutive % EXTERNAL_FAILURE_LOG_EVERY == 0 {
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

pub(crate) fn append_external_augmentations(snapshot: &mut AnalysisSnapshot, snapshot_json: &str) {
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
        let capabilities = query_external_capabilities(&config);
        let items = run_external_analysis(&config, capabilities.as_ref(), snapshot_json)
            .map(|items| {
                note_external_analysis_success(&config.engine_bin);
                items
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
        let cache_key = snapshot_cache_key(snapshot_json);
        let Some(config) = current_external_analysis_config() else {
            return;
        };
        let cached = {
            let guard = lock_state();
            if let Some(items) = guard.cache.get(&cache_key) {
                items.clone()
            } else {
                drop(guard);
                if let Some(reason) = current_external_circuit_block(&config.engine_bin) {
                    external_fallback_augmentations(&reason)
                } else {
                    let capabilities = capability_profile_for_config(&config);
                    let items =
                        run_external_analysis(&config, capabilities.as_ref(), snapshot_json)
                            .map(|items| {
                                note_external_analysis_success(&config.engine_bin);
                                items
                            })
                            .unwrap_or_else(|err| {
                                note_external_analysis_failure(&config.engine_bin, &err);
                                external_fallback_augmentations(&err)
                            });
                    let mut guard = lock_state();
                    if !guard.cache.contains_key(&cache_key) {
                        guard.cache_order.push_back(cache_key);
                    }
                    guard.cache.insert(cache_key, items.clone());
                    while guard.cache_order.len() > MAX_EXTERNAL_CACHE_ENTRIES {
                        if let Some(evicted) = guard.cache_order.pop_front() {
                            guard.cache.remove(&evicted);
                        }
                    }
                    items
                }
            }
        };
        snapshot.augmentations.extend(cached);
    }
}

fn run_external_analysis(
    config: &ExternalAnalysisConfig,
    capabilities: Option<&ExternalCapabilityProfile>,
    snapshot_json: &str,
) -> Result<Vec<AnalysisAugmentation>, String> {
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
    match cached {
        CachedCapabilities::Loaded(profile) => profile,
        CachedCapabilities::Unknown => {
            let profile = query_external_capabilities(config);
            let mut guard = lock_state();
            guard.capabilities = CachedCapabilities::Loaded(profile.clone());
            profile
        }
    }
}

fn query_external_capabilities(
    config: &ExternalAnalysisConfig,
) -> Option<ExternalCapabilityProfile> {
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

#[derive(Debug)]
struct ExternalCommandOutput {
    status: std::process::ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

fn run_external_command(
    mut command: Command,
    stdin_payload: Option<&[u8]>,
    timeout: Duration,
    max_stdout_bytes: usize,
    max_stderr_bytes: usize,
) -> Result<ExternalCommandOutput, String> {
    if stdin_payload.is_some() {
        command.stdin(Stdio::piped());
    }
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| format!("failed to launch external engine command: {err}"))?;
    if let Some(payload) = stdin_payload {
        let stdin = child
            .stdin
            .as_mut()
            .ok_or_else(|| "external engine stdin unavailable".to_string())?;
        stdin
            .write_all(payload)
            .map_err(|err| format!("failed to write snapshot to external engine stdin: {err}"))?;
    }
    let _ = child.stdin.take();
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "external engine stdout unavailable".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "external engine stderr unavailable".to_string())?;
    let stdout_reader =
        thread::spawn(move || read_capped_stream(stdout, max_stdout_bytes, "stdout"));
    let stderr_reader =
        thread::spawn(move || read_capped_stream(stderr, max_stderr_bytes, "stderr"));
    let wait_result = wait_for_child_with_timeout(&mut child, timeout);
    let stdout = stdout_reader
        .join()
        .map_err(|_| "external engine stdout reader thread panicked".to_string())??;
    let stderr = stderr_reader
        .join()
        .map_err(|_| "external engine stderr reader thread panicked".to_string())??;
    let status = wait_result?;
    Ok(ExternalCommandOutput {
        status,
        stdout,
        stderr,
    })
}

fn snapshot_cache_key(snapshot_json: &str) -> SnapshotCacheKey {
    let mut hasher = DefaultHasher::new();
    snapshot_json.hash(&mut hasher);
    SnapshotCacheKey {
        len: snapshot_json.len(),
        hash: hasher.finish(),
    }
}

fn wait_for_child_with_timeout(
    child: &mut std::process::Child,
    timeout: Duration,
) -> Result<std::process::ExitStatus, String> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|err| format!("failed waiting for external engine: {err}"))?
        {
            return Ok(status);
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!(
                "external engine timed out after {}s",
                timeout.as_secs().max(1)
            ));
        }
        thread::sleep(Duration::from_millis(25));
    }
}

fn read_capped_stream<R: Read>(
    mut reader: R,
    max_bytes: usize,
    stream_name: &str,
) -> Result<Vec<u8>, String> {
    let mut output = Vec::new();
    let mut buf = [0u8; 8192];
    let mut overflowed = false;
    loop {
        let read = reader
            .read(&mut buf)
            .map_err(|err| format!("failed reading external engine {stream_name}: {err}"))?;
        if read == 0 {
            break;
        }
        if !overflowed {
            let remaining = max_bytes.saturating_sub(output.len());
            let take = remaining.min(read);
            output.extend_from_slice(&buf[..take]);
            if take < read {
                overflowed = true;
            }
        }
    }
    if overflowed {
        Err(format!(
            "external engine {stream_name} exceeded {} bytes",
            max_bytes
        ))
    } else {
        Ok(output)
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
