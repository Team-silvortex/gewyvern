use std::collections::HashMap;
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};

use crate::diagnosis_runtime::{AnalysisAugmentation, AnalysisSnapshot};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct ExternalAnalysisConfig {
    pub(crate) engine_bin: String,
    pub(crate) python_worker: Option<String>,
    pub(crate) python_bin: Option<String>,
}

#[derive(Default)]
struct ExternalAnalysisState {
    config: Option<ExternalAnalysisConfig>,
    cache: HashMap<String, Vec<AnalysisAugmentation>>,
}

fn state() -> &'static Mutex<ExternalAnalysisState> {
    static STATE: OnceLock<Mutex<ExternalAnalysisState>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(ExternalAnalysisState::default()))
}

pub(crate) fn set_external_analysis_config(config: Option<ExternalAnalysisConfig>) {
    let mut guard = state().lock().expect("external analysis mutex poisoned");
    guard.config = config;
    guard.cache.clear();
}

pub(crate) fn append_external_augmentations(snapshot: &mut AnalysisSnapshot, snapshot_json: &str) {
    let cached = {
        let guard = state().lock().expect("external analysis mutex poisoned");
        let Some(config) = guard.config.clone() else {
            return;
        };
        if let Some(items) = guard.cache.get(snapshot_json) {
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
                    data_json: Some(format!("{{\"message\":\"{}\"}}", escape_json_string(&err))),
                }]
            });
            let mut guard = state().lock().expect("external analysis mutex poisoned");
            guard.cache.insert(snapshot_json.to_string(), items.clone());
            items
        }
    };
    snapshot.augmentations.extend(cached);
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
    let output = child
        .wait_with_output()
        .map_err(|err| format!("failed waiting for external engine: {err}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            format!("external engine exited with {}", output.status)
        } else {
            format!("external engine exited with {}: {}", output.status, stderr)
        });
    }
    parse_external_augmentations(&String::from_utf8_lossy(&output.stdout))
}

fn parse_external_augmentations(input: &str) -> Result<Vec<AnalysisAugmentation>, String> {
    let inner = extract_json_array_contents(input, "augmentations")?;
    let objects = split_top_level_json_objects(inner)?;
    let mut items = Vec::new();
    for object in objects {
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
    Ok(items)
}

fn extract_json_array_contents<'a>(input: &'a str, key: &str) -> Result<&'a str, String> {
    let needle = format!("\"{}\":[", key);
    let start = input
        .find(&needle)
        .ok_or_else(|| format!("missing '{}' array in external output", key))?
        + needle.len();
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
                        return Ok(&input[start..index]);
                    }
                }
                _ => {}
            }
        }
        index += 1;
    }
    Err(format!("unterminated '{}' array in external output", key))
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
    let needle = format!("\"{}\":\"", key);
    let start = input.find(&needle)? + needle.len();
    let rest = &input[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
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
    input.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
pub(crate) fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static TEST_GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    TEST_GUARD
        .get_or_init(|| Mutex::new(()))
        .lock()
        .expect("external analysis test guard poisoned")
}
