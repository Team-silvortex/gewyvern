use crate::render_utils::append_json_string;

use super::json::api_target_path_segment;
use super::{ApiSnapshot, ApiTargetSnapshot};

pub(super) fn api_debugger_console_json(snapshot: &ApiSnapshot) -> String {
    let mut targets = snapshot
        .target_names
        .iter()
        .filter_map(|name| {
            snapshot
                .target_snapshots
                .get(name)
                .map(|target| (name, target))
        })
        .collect::<Vec<_>>();
    targets.sort_by(|(left_name, left), (right_name, right)| {
        debugger_rank(left)
            .cmp(&debugger_rank(right))
            .then_with(|| left_name.cmp(right_name))
    });

    let mut json = String::with_capacity(1024 + targets.len() * 768);
    json.push_str("{\"surface\":\"debugger_console\",\"snapshot_kind\":");
    append_json_string(&mut json, &snapshot.kind);
    json.push_str(",\"updated_unix_ms\":");
    json.push_str(&snapshot.updated_unix_ms.to_string());
    json.push_str(",\"target_count\":");
    json.push_str(&targets.len().to_string());
    json.push_str(",\"attention_count\":");
    json.push_str(
        &targets
            .iter()
            .filter(|(_, target)| debugger_rank(target) <= 1)
            .count()
            .to_string(),
    );
    json.push_str(",\"recommended_focus\":");
    match targets.first() {
        Some((name, target)) => append_focus_json(&mut json, name, target),
        None => json.push_str("null"),
    }
    json.push_str(",\"targets\":[");
    for (index, (name, target)) in targets.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        append_focus_json(&mut json, name, target);
    }
    json.push_str("]}");
    json
}

fn append_focus_json(json: &mut String, name: &str, target: &ApiTargetSnapshot) {
    let segment = api_target_path_segment(name);
    let summary = DebugSummary::from_target(target);
    json.push('{');
    json.push_str("\"name\":");
    append_json_string(json, name);
    json.push_str(",\"path_segment\":");
    append_json_string(json, &segment);
    json.push_str(",\"rank\":");
    json.push_str(&debugger_rank(target).to_string());
    json.push_str(",\"status\":");
    append_json_string(json, &summary.status);
    json.push_str(",\"evidence_posture\":");
    append_opt_json_string(json, target.evidence_posture.as_deref());
    json.push_str(",\"automation_outcome\":");
    append_opt_json_string(json, target.automation_outcome.as_deref());
    json.push_str(",\"primary_module_family\":");
    append_opt_json_string(json, target.primary_module_family.as_deref());
    json.push_str(",\"primary_failure_stage\":");
    append_json_string(json, &summary.primary_failure_stage);
    json.push_str(",\"primary_failure_mode\":");
    append_json_string(json, &summary.primary_failure_mode);
    json.push_str(",\"primary_failure_detail\":");
    append_json_string(json, &summary.primary_failure_detail);
    json.push_str(",\"operator_guidance_action\":");
    append_json_string(json, &summary.operator_guidance_action);
    json.push_str(",\"operator_guidance_summary\":");
    append_json_string(json, &summary.operator_guidance_summary);
    json.push_str(",\"first_missing_transition\":");
    append_opt_json_string(json, summary.first_missing_transition.as_deref());
    json.push_str(",\"protocol_surface\":");
    append_protocol_json(json, target);
    json.push_str(",\"links\":{");
    append_link(json, "analysis", &segment, "analysis.json", false);
    append_link(json, "anomaly_flow", &segment, "anomaly-flow.json", true);
    append_link(json, "findings", &segment, "findings.json", true);
    append_link(json, "report", &segment, "report.html", true);
    json.push_str("}}");
}

fn append_link(json: &mut String, name: &str, segment: &str, suffix: &str, comma: bool) {
    if comma {
        json.push(',');
    }
    json.push('"');
    json.push_str(name);
    json.push_str("\":\"/v1/latest/targets/");
    json.push_str(segment);
    json.push('/');
    json.push_str(suffix);
    json.push('"');
}

fn append_protocol_json(json: &mut String, target: &ApiTargetSnapshot) {
    if let Some(surface) = target.protocol_surface.as_ref() {
        json.push('{');
        json.push_str("\"protocol\":");
        append_json_string(json, &surface.protocol);
        json.push_str(",\"entry\":");
        append_json_string(json, &surface.entry);
        json.push_str(",\"default_entry\":");
        append_json_string(json, &surface.default_entry);
        json.push('}');
    } else {
        json.push_str("null");
    }
}

fn append_opt_json_string(json: &mut String, value: Option<&str>) {
    match value {
        Some(value) => append_json_string(json, value),
        None => json.push_str("null"),
    }
}

fn debugger_rank(target: &ApiTargetSnapshot) -> usize {
    match target.automation_outcome.as_deref() {
        Some("targeted_escalation") => 0,
        Some("collect_more_evidence") => 1,
        Some("multi_hypothesis") => 2,
        Some("manual_review") => 3,
        Some("advisory_only") => 4,
        _ => match target.evidence_posture.as_deref() {
            Some("direct_protocol_signal") => 0,
            Some("missing_transition") => 1,
            Some("ambiguous_multi_hypothesis") => 2,
            Some("heuristic_summary") => 3,
            _ => 5,
        },
    }
}

struct DebugSummary {
    status: String,
    primary_failure_stage: String,
    primary_failure_mode: String,
    primary_failure_detail: String,
    operator_guidance_action: String,
    operator_guidance_summary: String,
    first_missing_transition: Option<String>,
}

impl DebugSummary {
    fn from_target(target: &ApiTargetSnapshot) -> Self {
        let input = &target.analysis_json;
        Self {
            status: extract_json_string_field(input, "target_status")
                .unwrap_or_else(|| "unknown".into()),
            primary_failure_stage: extract_json_string_field(input, "primary_failure_stage")
                .unwrap_or_else(|| "unknown".into()),
            primary_failure_mode: extract_json_string_field(input, "primary_failure_mode")
                .unwrap_or_else(|| "unknown".into()),
            primary_failure_detail: extract_json_string_field(input, "primary_failure_detail")
                .unwrap_or_else(|| "unknown".into()),
            operator_guidance_action: extract_json_string_field(input, "operator_guidance_action")
                .unwrap_or_else(|| "manual_review".into()),
            operator_guidance_summary: extract_json_string_field(
                input,
                "operator_guidance_summary",
            )
            .unwrap_or_default(),
            first_missing_transition: extract_first_json_string_array_entry(
                input,
                "missing_transitions",
            ),
        }
    }
}

fn extract_json_string_field(input: &str, key: &str) -> Option<String> {
    let needle = format!("\"{}\":", key);
    let start = input.find(&needle)? + needle.len();
    let rest = input[start..].trim_start();
    let mut chars = rest.chars();
    if chars.next()? != '"' {
        return None;
    }
    read_json_string(chars)
}

fn extract_first_json_string_array_entry(input: &str, key: &str) -> Option<String> {
    let needle = format!("\"{}\":[", key);
    let start = input.find(&needle)? + needle.len();
    let rest = input[start..].trim_start();
    let mut chars = rest.chars();
    if chars.next()? != '"' {
        return None;
    }
    read_json_string(chars)
}

fn read_json_string(chars: impl Iterator<Item = char>) -> Option<String> {
    let mut value = String::new();
    let mut escape = false;
    for ch in chars {
        if escape {
            value.push(match ch {
                '"' => '"',
                '\\' => '\\',
                '/' => '/',
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                other => other,
            });
            escape = false;
        } else if ch == '\\' {
            escape = true;
        } else if ch == '"' {
            return Some(value);
        } else {
            value.push(ch);
        }
    }
    None
}
