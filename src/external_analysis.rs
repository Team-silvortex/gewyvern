use std::collections::{HashMap, VecDeque, hash_map::DefaultHasher};
use std::hash::{Hash, Hasher};
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use crate::diagnosis_runtime::{AnalysisAugmentation, AnalysisSnapshot};

mod capabilities;
mod parse;

use self::capabilities::{ExternalCapabilityProfile, parse_external_capability_profile};
use self::parse::{parse_external_augmentations, single_json_string_field};

const EXTERNAL_ENGINE_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_EXTERNAL_ENGINE_STDOUT_BYTES: usize = 1024 * 1024;
const MAX_EXTERNAL_ENGINE_STDERR_BYTES: usize = 256 * 1024;
const MAX_EXTERNAL_CACHE_ENTRIES: usize = 128;
const MAX_EXTERNAL_AUGMENTATIONS: usize = 256;

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

#[derive(Default)]
struct ExternalAnalysisState {
    config: Option<ExternalAnalysisConfig>,
    capabilities: CachedCapabilities,
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
        let mut guard = state().lock().expect("external analysis mutex poisoned");
        guard.config = config;
        guard.capabilities = CachedCapabilities::Unknown;
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
        let capabilities = query_external_capabilities(&config);
        let items = run_external_analysis(&config, capabilities.as_ref(), snapshot_json)
            .unwrap_or_else(|err| {
                vec![AnalysisAugmentation {
                    kind: "external-engine".into(),
                    name: "external_engine_failed".into(),
                    summary: "external analysis engine failed; keeping built-in analysis only"
                        .into(),
                    confidence: "advisory".into(),
                    producer_stage: Some("external".into()),
                    producer_pass: Some("external-engine-hook".into()),
                    data_json: Some(single_json_string_field("message", &err)),
                }]
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
            let guard = state().lock().expect("external analysis mutex poisoned");
            if let Some(items) = guard.cache.get(&cache_key) {
                items.clone()
            } else {
                drop(guard);
                let capabilities = capability_profile_for_config(&config);
                let items = run_external_analysis(&config, capabilities.as_ref(), snapshot_json)
                    .unwrap_or_else(|err| {
                        vec![AnalysisAugmentation {
                            kind: "external-engine".into(),
                            name: "external_engine_failed".into(),
                            summary:
                                "external analysis engine failed; keeping built-in analysis only"
                                    .into(),
                            confidence: "advisory".into(),
                            producer_stage: Some("external".into()),
                            producer_pass: Some("external-engine-hook".into()),
                            data_json: Some(single_json_string_field("message", &err)),
                        }]
                    });
                let mut guard = state().lock().expect("external analysis mutex poisoned");
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
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| {
            format!(
                "failed to launch external engine '{}': {err}",
                config.engine_bin
            )
        })?;
    {
        let stdin = child
            .stdin
            .as_mut()
            .ok_or_else(|| "external engine stdin unavailable".to_string())?;
        stdin
            .write_all(snapshot_json.as_bytes())
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
    let stdout_reader = thread::spawn(move || {
        read_capped_stream(stdout, MAX_EXTERNAL_ENGINE_STDOUT_BYTES, "stdout")
    });
    let stderr_reader = thread::spawn(move || {
        read_capped_stream(stderr, MAX_EXTERNAL_ENGINE_STDERR_BYTES, "stderr")
    });
    let wait_result = wait_for_child_with_timeout(&mut child, EXTERNAL_ENGINE_TIMEOUT);
    let stdout = stdout_reader
        .join()
        .map_err(|_| "external engine stdout reader thread panicked".to_string())??;
    let stderr = stderr_reader
        .join()
        .map_err(|_| "external engine stderr reader thread panicked".to_string())??;
    let status = wait_result?;
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
    let guard = state().lock().expect("external analysis mutex poisoned");
    guard.config.clone()
}

fn capability_profile_for_config(
    config: &ExternalAnalysisConfig,
) -> Option<ExternalCapabilityProfile> {
    let cached = {
        let guard = state().lock().expect("external analysis mutex poisoned");
        guard.capabilities.clone()
    };
    match cached {
        CachedCapabilities::Loaded(profile) => profile,
        CachedCapabilities::Unknown => {
            let profile = query_external_capabilities(config);
            let mut guard = state().lock().expect("external analysis mutex poisoned");
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
    let output = command.output().ok()?;
    if !output.status.success() {
        return None;
    }
    parse_external_capability_profile(&String::from_utf8_lossy(&output.stdout)).ok()
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
mod tests {
    use super::*;

    #[test]
    fn read_capped_stream_rejects_oversized_output() {
        let result = read_capped_stream("abcdef".as_bytes(), 4, "stdout");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("exceeded"));
    }
}
