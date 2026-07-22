use super::*;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug)]
struct MemoryShape {
    schema_version: Option<u64>,
    model_version: Option<String>,
    pattern_labels: BTreeMap<String, BTreeSet<String>>,
    pattern_count: usize,
    label_count: usize,
}

pub(super) fn learning_backend_memory_transfer_plan(
    memory_snapshot_json: &str,
    strategy: &str,
    config: &LearningBackendConfig,
) -> Result<String, String> {
    if !matches!(strategy, "replace" | "merge") {
        return Err("strategy must be one of: replace, merge".to_string());
    }
    let (model_info_json, current_snapshot_json) = with_learning_backend(config, |backend| {
        Ok((backend.model_info_json()?, backend.export_memory_json()?))
    })?;
    let model_info = parse_json(&model_info_json, "backend model info")?;
    let current = memory_shape_from_json(&current_snapshot_json)?;
    let incoming = memory_shape_from_json(memory_snapshot_json)?;

    let current_schema = model_info.get("schema_version").and_then(Value::as_u64);
    let current_model = model_info
        .get("model_version")
        .and_then(Value::as_str)
        .map(str::to_string);
    let compatible_models = model_info
        .get("compatible_model_versions")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_else(|| current_model.iter().cloned().collect());
    let schema_compatible = incoming.schema_version == current_schema;
    let model_compatible = incoming
        .model_version
        .as_ref()
        .is_some_and(|model| compatible_models.contains(model));
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

    let recommendation = if !compatible {
        "do_not_import_until_schema_and_model_versions_match"
    } else if strategy == "replace" {
        "replace_will_discard_current_memory_after_operator_confirmation"
    } else if conflicting_patterns.is_empty() {
        "merge_is_low_risk_no_overlapping_pattern_conflicts"
    } else {
        "merge_requires_review_overlapping_patterns_have_different_labels"
    };
    serde_json::to_string(&json!({
        "kind": "etragon_memory_transfer_plan",
        "status": if compatible { "ready" } else { "blocked" },
        "dry_run": true,
        "will_import": false,
        "strategy": strategy,
        "compatible": compatible,
        "schema_compatible": schema_compatible,
        "model_compatible": model_compatible,
        "current": memory_shape_json(&current),
        "incoming": memory_shape_json(&incoming),
        "overlap_pattern_count": overlap.len(),
        "new_pattern_count": new_patterns.len(),
        "conflicting_pattern_count": conflicting_patterns.len(),
        "conflicting_patterns": conflicting_patterns,
        "recommendation": recommendation,
    }))
    .map_err(|err| format!("failed to encode memory transfer plan: {err}"))
}

pub(super) fn python_memory_transfer_plan(
    memory_snapshot_json: &str,
    strategy: &str,
    config: &PythonWorkerConfig,
) -> Result<String, String> {
    learning_backend_memory_transfer_plan(
        memory_snapshot_json,
        strategy,
        &LearningBackendConfig::Python(config.clone()),
    )
}

fn memory_shape_from_json(input: &str) -> Result<MemoryShape, String> {
    let payload = parse_json(input, "memory snapshot")?;
    let snapshot = payload.get("snapshot").unwrap_or(&payload);
    let object = snapshot
        .as_object()
        .ok_or_else(|| "memory snapshot must be a JSON object".to_string())?;
    let pattern_labels = object
        .get("pattern_labels")
        .and_then(Value::as_object)
        .map(|patterns| {
            patterns
                .iter()
                .map(|(pattern, labels)| {
                    let labels = labels
                        .as_object()
                        .map(|labels| labels.keys().cloned().collect())
                        .unwrap_or_default();
                    (pattern.clone(), labels)
                })
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();
    let pattern_count = object
        .get("pattern_count")
        .and_then(Value::as_u64)
        .map(|value| value as usize)
        .unwrap_or(pattern_labels.len());
    let label_count = object
        .get("label_count")
        .and_then(Value::as_u64)
        .map(|value| value as usize)
        .unwrap_or_else(|| pattern_labels.values().map(BTreeSet::len).sum());
    Ok(MemoryShape {
        schema_version: object.get("schema_version").and_then(Value::as_u64),
        model_version: object
            .get("model_version")
            .and_then(Value::as_str)
            .map(str::to_string),
        pattern_labels,
        pattern_count,
        label_count,
    })
}

fn memory_shape_json(shape: &MemoryShape) -> Value {
    json!({
        "schema_version": shape.schema_version,
        "model_version": shape.model_version,
        "pattern_count": shape.pattern_count,
        "label_count": shape.label_count,
    })
}

fn parse_json(input: &str, description: &str) -> Result<Value, String> {
    serde_json::from_str(input).map_err(|err| format!("failed to parse {description}: {err}"))
}
