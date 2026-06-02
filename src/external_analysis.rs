use std::collections::{HashMap, VecDeque, hash_map::DefaultHasher};
use std::hash::{Hash, Hasher};
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use crate::diagnosis_runtime::{AnalysisAugmentation, AnalysisSnapshot};
use crate::render_utils::{append_json_string, extract_json_string_field};

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
    cache: HashMap<SnapshotCacheKey, Vec<AnalysisAugmentation>>,
    cache_order: VecDeque<SnapshotCacheKey>,
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
        let items = run_external_analysis(&config, snapshot_json).unwrap_or_else(|err| {
            vec![AnalysisAugmentation {
                kind: "external-engine".into(),
                name: "external_engine_failed".into(),
                summary: "external analysis engine failed; keeping built-in analysis only".into(),
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
    let cached = {
        let guard = state().lock().expect("external analysis mutex poisoned");
        let Some(config) = guard.config.clone() else {
            return;
        };
        if let Some(items) = guard.cache.get(&cache_key) {
            items.clone()
        } else {
            drop(guard);
            let items = run_external_analysis(&config, snapshot_json).unwrap_or_else(|err| {
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
    parse_external_augmentations(&String::from_utf8_lossy(&stdout))
}

fn parse_external_augmentations(input: &str) -> Result<Vec<AnalysisAugmentation>, String> {
    let mut items = Vec::new();
    if let Some(inner) = extract_optional_json_array_contents(input, "augmentations")? {
        let objects = split_top_level_json_objects(inner)?;
        for object in objects {
            if items.len() >= MAX_EXTERNAL_AUGMENTATIONS {
                return Err(format!(
                    "external output contains more than {} augmentations",
                    MAX_EXTERNAL_AUGMENTATIONS
                ));
            }
            items.push(AnalysisAugmentation {
                kind: extract_required_json_string(object, "kind")?,
                name: extract_required_json_string(object, "name")?,
                summary: extract_required_json_string(object, "summary")?,
                confidence: extract_required_json_string(object, "confidence")?,
                producer_stage: extract_optional_json_string(object, "producer_stage"),
                producer_pass: extract_optional_json_string(object, "producer_pass"),
                data_json: extract_optional_json_value(object, "data"),
            });
        }
    }
    append_external_merge_hint_augmentations(&mut items, input)?;
    if items.is_empty() {
        return Err("missing 'augmentations' array in external output".to_string());
    }
    Ok(items)
}

fn extract_optional_json_array_contents<'a>(
    input: &'a str,
    key: &str,
) -> Result<Option<&'a str>, String> {
    let needle = format!("\"{}\":[", key);
    let Some(offset) = input.find(&needle) else {
        return Ok(None);
    };
    let start = offset + needle.len();
    let bytes = input.as_bytes();
    let mut depth = 1usize;
    let mut in_string = false;
    let mut escape = false;
    let mut index = start;
    while index < bytes.len() {
        let ch = bytes[index] as char;
        if in_string {
            if escape {
                escape = false;
            } else if ch == '\\' {
                escape = true;
            } else if ch == '"' {
                in_string = false;
            }
        } else {
            match ch {
                '"' => in_string = true,
                '[' => depth += 1,
                ']' => {
                    depth -= 1;
                    if depth == 0 {
                        return Ok(Some(&input[start..index]));
                    }
                }
                _ => {}
            }
        }
        index += 1;
    }
    Err(format!("unterminated '{}' array in external output", key))
}

fn append_external_merge_hint_augmentations(
    items: &mut Vec<AnalysisAugmentation>,
    input: &str,
) -> Result<(), String> {
    if let Some(object) = extract_optional_json_value(input, "evidence_chain_enrichment") {
        if items.len() >= MAX_EXTERNAL_AUGMENTATIONS {
            return Err(format!(
                "external output contains more than {} augmentations",
                MAX_EXTERNAL_AUGMENTATIONS
            ));
        }
        let summary = extract_required_json_string(&object, "summary")?;
        let handoff_readiness = extract_optional_json_string(&object, "handoff_readiness")
            .unwrap_or_else(|| "advisory_only".to_string());
        let merge_hint = extract_optional_json_string(&object, "gewyvern_merge_hint")
            .unwrap_or_else(|| "augmentations_only".to_string());
        items.push(AnalysisAugmentation {
            kind: "external-enrichment".into(),
            name: "external_evidence_chain_enrichment".into(),
            summary,
            confidence: external_hint_confidence(&handoff_readiness).into(),
            producer_stage: Some("external".into()),
            producer_pass: Some("external-engine-merge-prototype".into()),
            data_json: Some(object_with_merge_metadata(
                &object,
                &handoff_readiness,
                &merge_hint,
            )),
        });
    }
    if let Some(object) = extract_optional_json_value(input, "diagnostic_opinion") {
        if object != "null" {
            if items.len() >= MAX_EXTERNAL_AUGMENTATIONS {
                return Err(format!(
                    "external output contains more than {} augmentations",
                    MAX_EXTERNAL_AUGMENTATIONS
                ));
            }
            let summary = extract_required_json_string(&object, "summary")?;
            let handoff_readiness = extract_optional_json_string(&object, "handoff_readiness")
                .unwrap_or_else(|| "mergeable".to_string());
            let merge_hint = extract_optional_json_string(&object, "gewyvern_merge_hint")
                .unwrap_or_else(|| "sidecar_only_opinion".to_string());
            items.push(AnalysisAugmentation {
                kind: "external-opinion".into(),
                name: "external_diagnostic_opinion".into(),
                summary,
                confidence: external_hint_confidence(&handoff_readiness).into(),
                producer_stage: Some("external".into()),
                producer_pass: Some("external-engine-merge-prototype".into()),
                data_json: Some(object_with_merge_metadata(
                    &object,
                    &handoff_readiness,
                    &merge_hint,
                )),
            });
        }
    }
    Ok(())
}

fn object_with_merge_metadata(object: &str, handoff_readiness: &str, merge_hint: &str) -> String {
    let trimmed = object.trim();
    if !trimmed.ends_with('}') {
        return trimmed.to_string();
    }
    let inner = trimmed.trim_end_matches('}');
    format!(
        "{}{}\"external_handoff_readiness\":\"{}\",\"external_merge_hint\":\"{}\"}}",
        inner,
        if inner.ends_with('{') { "" } else { "," },
        escape_json_string(handoff_readiness),
        escape_json_string(merge_hint)
    )
}

fn external_hint_confidence(handoff_readiness: &str) -> &'static str {
    match handoff_readiness {
        "automation_worthy" => "candidate",
        "mergeable" => "advisory",
        _ => "advisory",
    }
}

fn split_top_level_json_objects(input: &str) -> Result<Vec<&str>, String> {
    let bytes = input.as_bytes();
    let mut objects = Vec::new();
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escape = false;
    let mut start = None;
    for (index, byte) in bytes.iter().enumerate() {
        let ch = *byte as char;
        if in_string {
            if escape {
                escape = false;
            } else if ch == '\\' {
                escape = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '{' => {
                if depth == 0 {
                    start = Some(index);
                }
                depth += 1;
            }
            '}' => {
                if depth == 0 {
                    return Err("invalid external augmentation payload".into());
                }
                depth -= 1;
                if depth == 0 {
                    let object_start = start.ok_or_else(|| {
                        "invalid external augmentation payload: missing object start".to_string()
                    })?;
                    objects.push(&input[object_start..=index]);
                    start = None;
                }
            }
            _ => {}
        }
    }
    if depth != 0 {
        return Err("unterminated external augmentation object".into());
    }
    Ok(objects)
}

fn extract_required_json_string(input: &str, key: &str) -> Result<String, String> {
    extract_optional_json_string(input, key)
        .ok_or_else(|| format!("missing '{}' string in external augmentation", key))
}

fn extract_optional_json_string(input: &str, key: &str) -> Option<String> {
    extract_json_string_field(input, key)
}

fn extract_optional_json_value(input: &str, key: &str) -> Option<String> {
    let needle = format!("\"{}\":", key);
    let start = input.find(&needle)? + needle.len();
    let rest = &input[start..];
    let consumed = consume_json_value(rest)?;
    let value = rest[..consumed].trim();
    if value == "null" {
        None
    } else {
        Some(value.to_string())
    }
}

fn consume_json_value(input: &str) -> Option<usize> {
    let bytes = input.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() && (bytes[index] as char).is_ascii_whitespace() {
        index += 1;
    }
    let start = index;
    let first = *bytes.get(index)? as char;
    match first {
        '"' => {
            index += 1;
            let mut escape = false;
            while index < bytes.len() {
                let ch = bytes[index] as char;
                if escape {
                    escape = false;
                } else if ch == '\\' {
                    escape = true;
                } else if ch == '"' {
                    return Some(index + 1);
                }
                index += 1;
            }
            None
        }
        '{' | '[' => {
            let (open, close) = if first == '{' { ('{', '}') } else { ('[', ']') };
            let mut depth = 0usize;
            let mut in_string = false;
            let mut escape = false;
            while index < bytes.len() {
                let ch = bytes[index] as char;
                if in_string {
                    if escape {
                        escape = false;
                    } else if ch == '\\' {
                        escape = true;
                    } else if ch == '"' {
                        in_string = false;
                    }
                } else {
                    match ch {
                        '"' => in_string = true,
                        c if c == open => depth += 1,
                        c if c == close => {
                            depth -= 1;
                            if depth == 0 {
                                return Some(index + 1);
                            }
                        }
                        _ => {}
                    }
                }
                index += 1;
            }
            None
        }
        _ => {
            while index < bytes.len() {
                let ch = bytes[index] as char;
                if ch == ',' || ch == '}' || ch == ']' {
                    break;
                }
                index += 1;
            }
            Some(index.max(start))
        }
    }
}

fn escape_json_string(input: &str) -> String {
    let mut escaped = String::new();
    append_json_string(&mut escaped, input);
    escaped
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(escaped.as_str())
        .to_string()
}

fn single_json_string_field(key: &str, value: &str) -> String {
    let mut json = String::new();
    json.push('{');
    append_json_string(&mut json, key);
    json.push(':');
    append_json_string(&mut json, value);
    json.push('}');
    json
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

    #[test]
    fn parse_external_augmentations_rejects_too_many_items() {
        let mut payload = String::from("{\"augmentations\":[");
        for index in 0..=MAX_EXTERNAL_AUGMENTATIONS {
            if index > 0 {
                payload.push(',');
            }
            payload.push_str(
                "{\"kind\":\"candidate\",\"name\":\"x\",\"summary\":\"y\",\"confidence\":\"candidate\"}",
            );
        }
        payload.push_str("]}");
        let err = match parse_external_augmentations(&payload) {
            Ok(_) => panic!("should reject oversized list"),
            Err(err) => err,
        };
        assert!(err.contains("more than"));
    }

    #[test]
    fn parse_external_augmentations_includes_merge_hint_contexts() {
        let payload = "{\"augmentations\":[{\"kind\":\"ml-candidate\",\"name\":\"ml_candidate_targeted_escalation\",\"summary\":\"external engine suggests targeted escalation\",\"confidence\":\"candidate\"}],\"evidence_chain_enrichment\":{\"status\":\"reinforced\",\"primary_label\":\"targeted_escalation\",\"summary\":\"reinforced evidence chain\",\"handoff_readiness\":\"automation_worthy\",\"gewyvern_merge_hint\":\"augmentations_with_operator_guidance_support\"},\"diagnostic_opinion\":{\"status\":\"ready\",\"diagnosis_kind\":\"direct_protocol_failure\",\"label\":\"targeted_escalation\",\"summary\":\"direct protocol failure is now the most direct opinion\",\"handoff_readiness\":\"automation_worthy\",\"gewyvern_merge_hint\":\"operator_guidance_candidate\"}}";
        let items = parse_external_augmentations(payload).expect("payload should parse");
        assert_eq!(items.len(), 3);
        assert!(
            items
                .iter()
                .any(|item| item.name == "external_evidence_chain_enrichment"
                    && item.summary == "reinforced evidence chain")
        );
        assert!(
            items
                .iter()
                .any(|item| item.name == "external_diagnostic_opinion"
                    && item.summary == "direct protocol failure is now the most direct opinion")
        );
        let evidence = items
            .iter()
            .find(|item| item.name == "external_evidence_chain_enrichment")
            .expect("synthetic evidence enrichment should exist");
        assert_eq!(evidence.confidence, "candidate");
        assert!(
            evidence.data_json.as_deref().unwrap_or_default().contains(
                "\"external_merge_hint\":\"augmentations_with_operator_guidance_support\""
            )
        );
        let opinion = items
            .iter()
            .find(|item| item.name == "external_diagnostic_opinion")
            .expect("synthetic diagnostic opinion should exist");
        assert_eq!(opinion.confidence, "candidate");
        assert!(
            opinion
                .data_json
                .as_deref()
                .unwrap_or_default()
                .contains("\"external_merge_hint\":\"operator_guidance_candidate\"")
        );
    }

    #[test]
    fn parse_external_augmentations_accepts_merge_only_payload() {
        let payload = "{\"evidence_chain_enrichment\":{\"status\":\"emerging\",\"primary_label\":\"network_observe_longer\",\"summary\":\"still maturing\",\"handoff_readiness\":\"advisory_only\",\"gewyvern_merge_hint\":\"augmentations_only\"}}";
        let items = parse_external_augmentations(payload).expect("merge-only payload should parse");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "external_evidence_chain_enrichment");
        assert_eq!(items[0].confidence, "advisory");
    }

    #[test]
    fn parse_external_augmentations_decodes_escaped_strings() {
        let payload = r#"{"augmentations":[{"kind":"ml-candidate","name":"quoted","summary":"sidecar said \"wait more\"","confidence":"candidate","producer_stage":"candidate","producer_pass":"worker\\runner"}]}"#;
        let items = parse_external_augmentations(payload).expect("escaped strings should parse");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].summary, "sidecar said \"wait more\"");
        assert_eq!(items[0].producer_pass.as_deref(), Some("worker\\runner"));
    }
}
