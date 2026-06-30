use super::*;
use gewyvern::protocol_profiles::protocol_surface;

pub(super) fn render_debug_session_outputs(
    cli: &Cli,
    outputs: &[(String, ExportBundle)],
) -> String {
    let analyses = outputs
        .iter()
        .map(|(_, export)| analysis_snapshot(export))
        .collect::<Vec<_>>();
    if cli.json {
        debug_session_json(outputs, &analyses)
    } else {
        debug_session_text(outputs, &analyses)
    }
}

fn debug_session_json(outputs: &[(String, ExportBundle)], analyses: &[AnalysisSnapshot]) -> String {
    let indexed = ranked_targets(outputs, analyses);
    let mut json = String::with_capacity(768 + indexed.len() * 768);
    json.push_str("{\"surface\":\"local_debug_session\",\"scope\":\"cli\",\"target_count\":");
    json.push_str(&indexed.len().to_string());
    json.push_str(",\"recommended_focus\":");
    match indexed.first() {
        Some((name, analysis)) => append_target_json(&mut json, name, analysis),
        None => json.push_str("null"),
    }
    json.push_str(",\"targets\":[");
    for (index, (name, analysis)) in indexed.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        append_target_json(&mut json, name, analysis);
    }
    json.push_str("]}");
    json
}

fn debug_session_text(outputs: &[(String, ExportBundle)], analyses: &[AnalysisSnapshot]) -> String {
    let indexed = ranked_targets(outputs, analyses);
    let mut text = format!("debug_session: targets={}", indexed.len());
    if let Some((name, analysis)) = indexed.first() {
        text.push_str("\nfocus: ");
        text.push_str(name);
        text.push_str(" action=");
        text.push_str(&analysis.operator_guidance_action);
        text.push_str(" stage=");
        text.push_str(&analysis.primary_failure_stage);
    }
    for (name, analysis) in indexed {
        text.push('\n');
        text.push_str("- target=");
        text.push_str(name);
        text.push_str(" status=");
        text.push_str(analysis.target_status.label());
        text.push_str(" failure=");
        text.push_str(&analysis.primary_failure_mode);
        text.push('/');
        text.push_str(&analysis.primary_failure_detail);
        text.push_str(" next=");
        text.push_str(&next_step_kinds(name, analysis).join(","));
    }
    text
}

fn ranked_targets<'a>(
    outputs: &'a [(String, ExportBundle)],
    analyses: &'a [AnalysisSnapshot],
) -> Vec<(&'a str, &'a AnalysisSnapshot)> {
    let mut indexed = outputs
        .iter()
        .zip(analyses.iter())
        .map(|((name, _), analysis)| (name.as_str(), analysis))
        .collect::<Vec<_>>();
    indexed.sort_by(|(left_name, left), (right_name, right)| {
        rank(left)
            .cmp(&rank(right))
            .then_with(|| left_name.cmp(right_name))
    });
    indexed
}

fn append_target_json(json: &mut String, name: &str, analysis: &AnalysisSnapshot) {
    json.push('{');
    json.push_str("\"name\":");
    append_json_string(json, name);
    json.push_str(",\"rank\":");
    json.push_str(&rank(analysis).to_string());
    json.push_str(",\"status\":");
    append_json_string(json, analysis.target_status.label());
    json.push_str(",\"evidence_posture\":");
    append_json_string(json, &analysis.evidence_posture);
    json.push_str(",\"automation_outcome\":");
    append_json_string(json, &analysis.automation_outcome);
    json.push_str(",\"failure_spine\":{");
    json.push_str("\"stage\":");
    append_json_string(json, &analysis.primary_failure_stage);
    json.push_str(",\"mode\":");
    append_json_string(json, &analysis.primary_failure_mode);
    json.push_str(",\"detail\":");
    append_json_string(json, &analysis.primary_failure_detail);
    json.push_str(",\"basis\":");
    append_json_string(json, &analysis.primary_failure_basis);
    json.push_str("},\"operator_guidance\":{");
    json.push_str("\"action\":");
    append_json_string(json, &analysis.operator_guidance_action);
    json.push_str(",\"summary\":");
    append_json_string(json, &analysis.operator_guidance_summary);
    json.push_str("},\"first_missing_transition\":");
    match analysis.missing_transitions.first() {
        Some(value) => append_json_string(json, value),
        None => json.push_str("null"),
    }
    json.push_str(",\"protocol_surface\":");
    append_protocol_surface_json(json, name);
    json.push_str(",\"next_steps\":");
    append_next_steps_json(json, name, analysis);
    json.push('}');
}

fn append_protocol_surface_json(json: &mut String, name: &str) {
    let Some((protocol, entry)) = scan_target_protocol_entry(name) else {
        json.push_str("null");
        return;
    };
    let Some(surface) = protocol_surface(protocol, entry) else {
        json.push_str("null");
        return;
    };
    json.push_str("{\"protocol\":");
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

fn append_next_steps_json(json: &mut String, name: &str, analysis: &AnalysisSnapshot) {
    let steps = next_step_kinds(name, analysis);
    json.push('[');
    for (index, step) in steps.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        json.push('{');
        json.push_str("\"kind\":");
        append_json_string(json, step);
        json.push_str(",\"reason\":");
        append_json_string(json, next_step_reason(step));
        json.push('}');
    }
    json.push(']');
}

fn next_step_kinds(name: &str, analysis: &AnalysisSnapshot) -> Vec<&'static str> {
    let mut steps = vec!["read_analysis"];
    if scan_target_protocol_entry(name).is_some() {
        steps.push("read_protocol_plan");
    }
    if !analysis.missing_transitions.is_empty() {
        steps.push("collect_missing_evidence");
    }
    steps
}

fn next_step_reason(step: &str) -> &'static str {
    match step {
        "read_protocol_plan" => "follow companion protocol reading order",
        "collect_missing_evidence" => "inspect missing transitions before escalation",
        _ => "inspect the diagnosis spine",
    }
}

fn scan_target_protocol_entry(name: &str) -> Option<(&str, &str)> {
    let mut parts = name.splitn(3, ':');
    if parts.next()? != "scan" {
        return None;
    }
    Some((parts.next()?, parts.next()?))
}

fn rank(analysis: &AnalysisSnapshot) -> usize {
    match analysis.automation_outcome.as_str() {
        "targeted_escalation" => 0,
        "collect_more_evidence" => 1,
        "multi_hypothesis" => 2,
        "manual_review" => 3,
        "advisory_only" => 4,
        _ => match analysis.evidence_posture.as_str() {
            "direct_protocol_signal" => 0,
            "missing_transition" => 1,
            "ambiguous_multi_hypothesis" => 2,
            "heuristic_summary" => 3,
            _ => 5,
        },
    }
}
