use crate::render_utils::append_json_string;

use super::json::api_target_path_segment;
use super::{ApiSnapshot, ApiTargetSnapshot};

pub(super) fn training_dataset_manifest_json(snapshot: &ApiSnapshot) -> String {
    let mut json = String::with_capacity(768 + snapshot.target_names.len() * 256);
    json.push_str("{\"kind\":\"training_dataset_manifest\",\"schema_version\":1,");
    json.push_str("\"snapshot_kind\":");
    append_json_string(&mut json, &snapshot.kind);
    json.push_str(",\"target_count\":");
    json.push_str(&snapshot.target_names.len().to_string());
    json.push_str(",\"sample_format\":\"training_example_json\",\"sample_schema_version\":1");
    json.push_str(",\"split_policies\":");
    append_split_policies_json(&mut json);
    json.push_str(",\"latest_sample_path\":\"/v1/latest/training-example.json\"");
    json.push_str(",\"targets_path\":\"/v1/latest/targets\"");
    json.push_str(",\"supervision_heads\":");
    append_training_supervision_heads_json(&mut json);
    json.push_str(",\"samples\":[");
    append_training_dataset_samples_json(
        &mut json,
        &snapshot.target_names,
        Some(&snapshot.target_snapshots),
    );
    json.push_str("]}");
    json
}

pub(super) fn target_training_dataset_manifest_json(
    target_name: &str,
    target: &ApiTargetSnapshot,
) -> String {
    let mut json = String::with_capacity(640);
    json.push_str("{\"kind\":\"training_dataset_manifest\",\"schema_version\":1,");
    json.push_str("\"snapshot_kind\":\"target\",\"target_count\":1");
    json.push_str(",\"sample_format\":\"training_example_json\",\"sample_schema_version\":1");
    json.push_str(",\"split_policies\":");
    append_split_policies_json(&mut json);
    json.push_str(",\"supervision_heads\":");
    append_training_supervision_heads_json(&mut json);
    json.push_str(",\"samples\":[");
    append_training_dataset_samples_json(
        &mut json,
        &[target_name.to_string()],
        Some(&std::collections::HashMap::from([(
            target_name.to_string(),
            target.clone(),
        )])),
    );
    json.push_str("]}");
    json
}

fn append_split_policies_json(target: &mut String) {
    target.push_str("{\"default\":\"name_bucket_mod_10\",");
    target.push_str("\"available\":[\"name_bucket_mod_10\",\"protocol_bucket_mod_10\"]}");
}

fn append_training_supervision_heads_json(target: &mut String) {
    target.push_str("{\"diagnosis\":[");
    append_training_head_fields_json(
        target,
        &[
            "target_status",
            "primary_module_kind",
            "primary_failure_stage",
            "primary_failure_mode",
            "primary_failure_detail",
            "primary_failure_confidence",
            "primary_failure_basis",
            "ambiguous",
        ],
    );
    target.push_str("],\"guidance\":[");
    append_training_head_fields_json(target, &["status", "action", "reason"]);
    target.push_str("],\"automation\":[");
    append_training_head_fields_json(
        target,
        &[
            "posture",
            "requires_human_review",
            "collect_more_evidence_first",
            "targeted_escalation_allowed",
        ],
    );
    target.push_str("],\"ranking\":[");
    append_training_head_fields_json(
        target,
        &["attention_priority", "ambiguity_bucket", "evidence_posture"],
    );
    target.push_str("]}");
}

fn append_training_head_fields_json(target: &mut String, fields: &[&str]) {
    for (index, field) in fields.iter().enumerate() {
        if index > 0 {
            target.push(',');
        }
        append_json_string(target, field);
    }
}

fn append_training_dataset_samples_json(
    target: &mut String,
    target_names: &[String],
    target_snapshots: Option<&std::collections::HashMap<String, ApiTargetSnapshot>>,
) {
    for (index, name) in target_names.iter().enumerate() {
        if index > 0 {
            target.push(',');
        }
        let path_segment = api_target_path_segment(name);
        let sample_id = training_sample_id(name);
        let protocol_group = target_snapshots
            .and_then(|items| items.get(name))
            .and_then(|item| item.protocol_surface.as_ref())
            .map_or("unknown", |surface| surface.protocol.as_str());
        target.push_str("{\"name\":");
        append_json_string(target, name);
        target.push_str(",\"sample_id\":");
        append_json_string(target, &sample_id);
        target.push_str(",\"path_segment\":");
        append_json_string(target, &path_segment);
        target.push_str(",\"group_key\":");
        append_json_string(target, protocol_group);
        target.push_str(",\"split_hints\":{");
        target.push_str("\"name_bucket_mod_10\":");
        append_json_string(target, split_bucket_label(name_bucket_hash(name)));
        target.push_str(",\"protocol_bucket_mod_10\":");
        append_json_string(
            target,
            split_bucket_label(protocol_bucket_hash(protocol_group, name)),
        );
        target.push_str("},\"sample_path\":\"/v1/latest/targets/");
        target.push_str(&path_segment);
        target.push_str("/training-example.json\",\"dataset_path\":\"/v1/latest/targets/");
        target.push_str(&path_segment);
        target.push_str("/training-dataset.json\"}");
    }
}

pub(crate) fn training_sample_id(name: &str) -> String {
    format!("gewy:{:016x}", fnv1a64(name.as_bytes()))
}

fn name_bucket_hash(name: &str) -> u64 {
    fnv1a64(name.as_bytes()) % 10
}

fn protocol_bucket_hash(protocol_group: &str, name: &str) -> u64 {
    let composite = format!("{protocol_group}:{name}");
    fnv1a64(composite.as_bytes()) % 10
}

fn split_bucket_label(bucket: u64) -> &'static str {
    match bucket {
        0 => "test",
        1 => "validation",
        _ => "train",
    }
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}
