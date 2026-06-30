use super::*;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug)]
struct MemoryShape {
    schema_version: Option<String>,
    model_version: Option<String>,
    pattern_labels: BTreeMap<String, BTreeSet<String>>,
    pattern_count: usize,
    label_count: usize,
}

pub(super) fn python_memory_transfer_plan(
    memory_snapshot_json: &str,
    strategy: &str,
    config: &PythonWorkerConfig,
) -> Result<String, String> {
    if !matches!(strategy, "replace" | "merge") {
        return Err("strategy must be one of: replace, merge".to_string());
    }
    let mut worker = PythonWorkerClient::spawn(config)?;
    let model_info = worker.model_info_json()?;
    let current_snapshot = worker.export_memory_json()?;
    let current = memory_shape_from_json(&current_snapshot)?;
    let incoming = memory_shape_from_json(memory_snapshot_json)?;

    let current_schema = extract_json_value(&model_info, "schema_version");
    let current_model = extract_json_value(&model_info, "model_version")
        .map(|value| value.trim_matches('"').to_string());
    let schema_compatible = incoming.schema_version == current_schema;
    let model_compatible = incoming.model_version == current_model;
    let compatible = schema_compatible && model_compatible;

    let current_keys = current
        .pattern_labels
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    let incoming_keys = incoming
        .pattern_labels
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    let overlap = current_keys
        .intersection(&incoming_keys)
        .cloned()
        .collect::<BTreeSet<_>>();
    let new_patterns = incoming_keys
        .difference(&current_keys)
        .cloned()
        .collect::<BTreeSet<_>>();
    let conflicting_patterns = overlap
        .iter()
        .filter(|key| current.pattern_labels.get(*key) != incoming.pattern_labels.get(*key))
        .cloned()
        .collect::<Vec<_>>();

    let status = if compatible { "ready" } else { "blocked" };
    let recommendation = if !compatible {
        "do_not_import_until_schema_and_model_versions_match"
    } else if strategy == "replace" {
        "replace_will_discard_current_memory_after_operator_confirmation"
    } else if conflicting_patterns.is_empty() {
        "merge_is_low_risk_no_overlapping_pattern_conflicts"
    } else {
        "merge_requires_review_overlapping_patterns_have_different_labels"
    };

    Ok(format!(
        "{{\"kind\":\"etragon_memory_transfer_plan\",\"status\":\"{}\",\"dry_run\":true,\"will_import\":false,\"strategy\":\"{}\",\"compatible\":{},\"schema_compatible\":{},\"model_compatible\":{},\"current\":{},\"incoming\":{},\"overlap_pattern_count\":{},\"new_pattern_count\":{},\"conflicting_pattern_count\":{},\"conflicting_patterns\":{},\"recommendation\":\"{}\"}}",
        status,
        escape_json_string(strategy),
        compatible,
        schema_compatible,
        model_compatible,
        memory_shape_json(&current),
        memory_shape_json(&incoming),
        overlap.len(),
        new_patterns.len(),
        conflicting_patterns.len(),
        string_array_json(&conflicting_patterns),
        escape_json_string(recommendation)
    ))
}

fn memory_shape_from_json(input: &str) -> Result<MemoryShape, String> {
    let snapshot =
        extract_json_value(input, "snapshot").unwrap_or_else(|| input.trim().to_string());
    let schema_version = extract_json_value(&snapshot, "schema_version");
    let model_version = extract_json_value(&snapshot, "model_version")
        .map(|value| value.trim_matches('"').to_string());
    let pattern_labels_json =
        extract_json_value(&snapshot, "pattern_labels").unwrap_or_else(|| "{}".to_string());
    let pattern_labels = parse_pattern_label_map(&pattern_labels_json);
    let pattern_count = extract_json_value(&snapshot, "pattern_count")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(pattern_labels.len());
    let label_count = extract_json_value(&snapshot, "label_count")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or_else(|| pattern_labels.values().map(BTreeSet::len).sum());
    Ok(MemoryShape {
        schema_version,
        model_version,
        pattern_labels,
        pattern_count,
        label_count,
    })
}

fn parse_pattern_label_map(input: &str) -> BTreeMap<String, BTreeSet<String>> {
    top_level_object_entries(input)
        .into_iter()
        .map(|(pattern, labels_json)| {
            let labels = top_level_object_entries(&labels_json)
                .into_iter()
                .map(|(label, _)| label)
                .collect::<BTreeSet<_>>();
            (pattern, labels)
        })
        .collect()
}

fn top_level_object_entries(input: &str) -> Vec<(String, String)> {
    let mut entries = Vec::new();
    let bytes = input.as_bytes();
    let mut index = match input.find('{') {
        Some(pos) => pos + 1,
        None => return entries,
    };
    while index < bytes.len() {
        while index < bytes.len() && matches!(bytes[index], b' ' | b'\n' | b'\r' | b'\t' | b',') {
            index += 1;
        }
        if index >= bytes.len() || bytes[index] == b'}' {
            break;
        }
        if bytes[index] != b'"' {
            break;
        }
        let (key, after_key) = match parse_json_string_at(input, index) {
            Some(parsed) => parsed,
            None => break,
        };
        index = after_key;
        while index < bytes.len() && matches!(bytes[index], b' ' | b'\n' | b'\r' | b'\t' | b':') {
            index += 1;
        }
        let Some((value, after_value)) = parse_json_value_slice(input, index) else {
            break;
        };
        entries.push((key, value));
        index = after_value;
    }
    entries
}

fn parse_json_string_at(input: &str, start: usize) -> Option<(String, usize)> {
    let mut escaped = false;
    let mut out = String::new();
    for (offset, ch) in input[start + 1..].char_indices() {
        if escaped {
            out.push(ch);
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == '"' {
            return Some((out, start + 1 + offset + 1));
        } else {
            out.push(ch);
        }
    }
    None
}

fn parse_json_value_slice(input: &str, start: usize) -> Option<(String, usize)> {
    let tail = input[start..].trim_start();
    let skipped = input[start..].len() - tail.len();
    let start = start + skipped;
    let first = tail.as_bytes().first().copied()?;
    match first {
        b'{' => balanced_slice(input, start, '{', '}'),
        b'[' => balanced_slice(input, start, '[', ']'),
        b'"' => {
            parse_json_string_at(input, start).map(|(_, end)| (input[start..end].to_string(), end))
        }
        _ => {
            let end = input[start..]
                .find(|ch: char| [',', '}'].contains(&ch))
                .map(|offset| start + offset)
                .unwrap_or(input.len());
            Some((input[start..end].trim().to_string(), end))
        }
    }
}

fn balanced_slice(input: &str, start: usize, open: char, close: char) -> Option<(String, usize)> {
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (offset, ch) in input[start..].char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        if ch == '"' {
            in_string = true;
        } else if ch == open {
            depth += 1;
        } else if ch == close {
            depth -= 1;
            if depth == 0 {
                let end = start + offset + ch.len_utf8();
                return Some((input[start..end].to_string(), end));
            }
        }
    }
    None
}

fn memory_shape_json(shape: &MemoryShape) -> String {
    format!(
        "{{\"schema_version\":{},\"model_version\":{},\"pattern_count\":{},\"label_count\":{}}}",
        shape
            .schema_version
            .clone()
            .unwrap_or_else(|| "null".to_string()),
        shape
            .model_version
            .as_ref()
            .map(|value| format!("\"{}\"", escape_json_string(value)))
            .unwrap_or_else(|| "null".to_string()),
        shape.pattern_count,
        shape.label_count
    )
}

fn string_array_json(values: &[String]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|value| format!("\"{}\"", escape_json_string(value)))
            .collect::<Vec<_>>()
            .join(",")
    )
}
