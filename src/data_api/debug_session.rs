use crate::render_utils::append_json_string;

use super::json::api_target_path_segment;
use super::{ApiSnapshot, ApiTargetSnapshot};

pub(super) fn api_debug_session_json(snapshot: &ApiSnapshot) -> String {
    let targets = ranked_targets(snapshot);
    let mut json = String::with_capacity(1024 + targets.len() * 640);
    json.push_str("{\"surface\":\"debug_session\",\"scope\":\"snapshot\",\"snapshot_kind\":");
    append_json_string(&mut json, &snapshot.kind);
    json.push_str(",\"updated_unix_ms\":");
    json.push_str(&snapshot.updated_unix_ms.to_string());
    json.push_str(",\"target_count\":");
    json.push_str(&targets.len().to_string());
    json.push_str(",\"recommended_focus\":");
    match targets.first() {
        Some((name, target)) => append_debug_target_json(&mut json, name, target),
        None => json.push_str("null"),
    }
    json.push_str(",\"links\":{");
    append_static_link(
        &mut json,
        "debugger_console",
        "/v1/latest/debugger-console.json",
        false,
    );
    append_static_link(
        &mut json,
        "runtime_resilience",
        "/v1/runtime/resilience.json",
        true,
    );
    append_static_link(
        &mut json,
        "runtime_capability_digest",
        "/v1/latest/runtime-capability-digest.json",
        true,
    );
    json.push_str("},\"targets\":[");
    for (index, (name, target)) in targets.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        append_debug_target_json(&mut json, name, target);
    }
    json.push_str("]}");
    json
}

pub(super) fn api_target_debug_session_json(name: &str, target: &ApiTargetSnapshot) -> String {
    let mut json = String::with_capacity(1024);
    json.push_str("{\"surface\":\"debug_session\",\"scope\":\"target\",\"target\":");
    append_json_string(&mut json, name);
    json.push_str(",\"recommended_focus\":");
    append_debug_target_json(&mut json, name, target);
    json.push('}');
    json
}

fn ranked_targets(snapshot: &ApiSnapshot) -> Vec<(&String, &ApiTargetSnapshot)> {
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
        debug_rank(left)
            .cmp(&debug_rank(right))
            .then_with(|| left_name.cmp(right_name))
    });
    targets
}

fn append_debug_target_json(json: &mut String, name: &str, target: &ApiTargetSnapshot) {
    let segment = api_target_path_segment(name);
    let summary = DebugSummary::from_target(target);
    json.push('{');
    json.push_str("\"name\":");
    append_json_string(json, name);
    json.push_str(",\"path_segment\":");
    append_json_string(json, &segment);
    json.push_str(",\"rank\":");
    json.push_str(&debug_rank(target).to_string());
    json.push_str(",\"status\":");
    append_json_string(json, &summary.status);
    json.push_str(",\"evidence_posture\":");
    append_opt_json_string(json, target.evidence_posture.as_deref());
    json.push_str(",\"automation_outcome\":");
    append_opt_json_string(json, target.automation_outcome.as_deref());
    json.push_str(",\"failure_spine\":{");
    json.push_str("\"stage\":");
    append_json_string(json, &summary.primary_failure_stage);
    json.push_str(",\"mode\":");
    append_json_string(json, &summary.primary_failure_mode);
    json.push_str(",\"detail\":");
    append_json_string(json, &summary.primary_failure_detail);
    json.push_str(",\"basis\":");
    append_json_string(json, &summary.primary_failure_basis);
    json.push('}');
    json.push_str(",\"operator_guidance\":{");
    json.push_str("\"action\":");
    append_json_string(json, &summary.operator_guidance_action);
    json.push_str(",\"summary\":");
    append_json_string(json, &summary.operator_guidance_summary);
    json.push('}');
    json.push_str(",\"debugger_posture\":");
    append_debugger_posture_json(json, target, &summary);
    json.push_str(",\"first_missing_transition\":");
    append_opt_json_string(json, summary.first_missing_transition.as_deref());
    json.push_str(",\"protocol_surface\":");
    append_protocol_surface_json(json, target);
    json.push_str(",\"next_steps\":");
    append_next_steps_json(json, name, &segment, target, &summary);
    json.push_str(",\"links\":{");
    append_target_link(json, "summary", &segment, "summary.json", false);
    append_target_link(json, "analysis", &segment, "analysis.json", true);
    append_target_link(json, "debug_session", &segment, "debug-session.json", true);
    append_target_link(
        json,
        "protocol_surface",
        &segment,
        "protocol-surface.json",
        true,
    );
    append_target_link(
        json,
        "protocol_reading",
        &segment,
        "protocol-reading.json",
        true,
    );
    append_target_link(json, "anomaly_flow", &segment, "anomaly-flow.json", true);
    json.push_str("}}");
}

fn append_next_steps_json(
    json: &mut String,
    name: &str,
    segment: &str,
    target: &ApiTargetSnapshot,
    summary: &DebugSummary,
) {
    json.push('[');
    append_next_step(
        json,
        "read_analysis",
        &format!("/v1/latest/targets/{segment}/analysis.json"),
        "inspect the diagnosis spine",
        false,
    );
    if target.protocol_surface.is_some() {
        append_next_step(
            json,
            "read_protocol_plan",
            &format!("/v1/latest/targets/{segment}/protocol-reading.json"),
            "follow the target protocol reading order",
            true,
        );
    }
    if summary.first_missing_transition.is_some() {
        append_next_step(
            json,
            "collect_missing_evidence",
            &format!("/v1/latest/targets/{segment}/anomaly-flow.json"),
            "inspect the first missing transition before escalating",
            true,
        );
    }
    if target.protocol_surface.is_none() && name.starts_with("scan:") {
        append_next_step(
            json,
            "check_protocol_entry",
            "/v1/protocols",
            "target name looks protocol-shaped but no surface resolved",
            true,
        );
    }
    json.push(']');
}

fn append_next_step(json: &mut String, kind: &str, path: &str, reason: &str, comma: bool) {
    if comma {
        json.push(',');
    }
    json.push_str("{\"kind\":");
    append_json_string(json, kind);
    json.push_str(",\"path\":");
    append_json_string(json, path);
    json.push_str(",\"reason\":");
    append_json_string(json, reason);
    json.push('}');
}

fn append_protocol_surface_json(json: &mut String, target: &ApiTargetSnapshot) {
    let Some(surface) = target.protocol_surface.as_ref() else {
        json.push_str("null");
        return;
    };
    json.push('{');
    json.push_str("\"protocol\":");
    append_json_string(json, &surface.protocol);
    json.push_str(",\"entry\":");
    append_json_string(json, &surface.entry);
    json.push_str(",\"cluster\":");
    match surface.cluster_hint.as_ref() {
        Some(hint) => append_json_string(json, &hint.key),
        None => json.push_str("null"),
    }
    json.push('}');
}

fn append_debugger_posture_json(
    json: &mut String,
    target: &ApiTargetSnapshot,
    summary: &DebugSummary,
) {
    json.push('{');
    json.push_str("\"state\":");
    append_json_string(json, debugger_posture_state(target, summary));
    json.push_str(",\"confidence\":");
    append_json_string(json, debugger_posture_confidence(target, summary));
    json.push_str(",\"recommended_action\":");
    append_json_string(json, debugger_posture_action(target, summary));
    json.push_str(",\"reason\":");
    append_json_string(json, debugger_posture_reason(target, summary));
    json.push('}');
}

fn append_static_link(json: &mut String, name: &str, path: &str, comma: bool) {
    if comma {
        json.push(',');
    }
    json.push('"');
    json.push_str(name);
    json.push_str("\":");
    append_json_string(json, path);
}

fn append_target_link(json: &mut String, name: &str, segment: &str, suffix: &str, comma: bool) {
    append_static_link(
        json,
        name,
        &format!("/v1/latest/targets/{segment}/{suffix}"),
        comma,
    );
}

fn append_opt_json_string(json: &mut String, value: Option<&str>) {
    match value {
        Some(value) => append_json_string(json, value),
        None => json.push_str("null"),
    }
}

fn debug_rank(target: &ApiTargetSnapshot) -> usize {
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

fn debugger_posture_state(target: &ApiTargetSnapshot, summary: &DebugSummary) -> &'static str {
    if summary.first_missing_transition.is_some()
        || target.automation_outcome.as_deref() == Some("collect_more_evidence")
        || target.evidence_posture.as_deref() == Some("missing_transition")
    {
        "needs_evidence"
    } else if target.automation_outcome.as_deref() == Some("targeted_escalation")
        || target.evidence_posture.as_deref() == Some("direct_protocol_signal")
    {
        "ready_to_escalate"
    } else if target.automation_outcome.as_deref() == Some("multi_hypothesis")
        || target.evidence_posture.as_deref() == Some("ambiguous_multi_hypothesis")
    {
        "needs_hypothesis_review"
    } else if target.automation_outcome.as_deref() == Some("advisory_only") {
        "advisory"
    } else if summary.status == "healthy" {
        "healthy"
    } else {
        "needs_human_review"
    }
}

fn debugger_posture_confidence(target: &ApiTargetSnapshot, summary: &DebugSummary) -> &'static str {
    match debugger_posture_state(target, summary) {
        "ready_to_escalate" => "high",
        "needs_evidence" | "needs_hypothesis_review" => "medium",
        "healthy" => "high",
        _ => "low",
    }
}

fn debugger_posture_action(target: &ApiTargetSnapshot, summary: &DebugSummary) -> &'static str {
    match debugger_posture_state(target, summary) {
        "ready_to_escalate" => "escalate_protocol_signal",
        "needs_evidence" => "collect_missing_runtime_evidence",
        "needs_hypothesis_review" => "compare_competing_hypotheses",
        "healthy" => "observe_stable_baseline",
        "advisory" => "observe_only",
        _ => "read_analysis_before_automation",
    }
}

fn debugger_posture_reason(target: &ApiTargetSnapshot, summary: &DebugSummary) -> &'static str {
    match debugger_posture_state(target, summary) {
        "ready_to_escalate" => "direct protocol evidence supports a targeted next step",
        "needs_evidence" => "the current conclusion depends on missing runtime evidence",
        "needs_hypothesis_review" => "multiple plausible explanations remain active",
        "healthy" => "no debugger action is required for this target",
        "advisory" => "the signal is useful but not strong enough for automation",
        _ => "the debugger cannot narrow the conclusion without operator review",
    }
}

struct DebugSummary {
    status: String,
    primary_failure_stage: String,
    primary_failure_mode: String,
    primary_failure_detail: String,
    primary_failure_basis: String,
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
            primary_failure_basis: extract_json_string_field(input, "primary_failure_basis")
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
    read_json_string(input[start..].trim_start().chars())
}

fn extract_first_json_string_array_entry(input: &str, key: &str) -> Option<String> {
    let needle = format!("\"{}\":[", key);
    let start = input.find(&needle)? + needle.len();
    read_json_string(input[start..].trim_start().chars())
}

fn read_json_string(mut chars: impl Iterator<Item = char>) -> Option<String> {
    if chars.next()? != '"' {
        return None;
    }
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
