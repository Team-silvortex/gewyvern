use super::debug_targets::{rank, ranked_targets, scan_target_protocol_entry};
use super::*;
use gewyvern::protocol_profiles::protocol_surface;

pub(super) fn render_debug_session_outputs(
    cli: &Cli,
    outputs: &[(String, ExportBundle)],
) -> String {
    let analyses = collect_analyses(outputs);
    if cli.json {
        debug_session_json(outputs, &analyses)
    } else {
        debug_session_text(outputs, &analyses)
    }
}

fn debug_session_json(outputs: &[(String, ExportBundle)], analyses: &[AnalysisSnapshot]) -> String {
    let indexed = ranked_targets(outputs, analyses);
    let mut json = String::with_capacity(768 + indexed.len() * 768);
    let _ = write!(
        json,
        "{{\"surface\":\"local_debug_session\",\"scope\":\"cli\",\"target_count\":{},\"recommended_focus\":",
        indexed.len()
    );
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
    let mut text = String::with_capacity(96 + indexed.len() * 224);
    let _ = write!(text, "debug_session: targets={}", indexed.len());
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
        text.push_str(" posture=");
        let state = debugger_posture_state(analysis);
        text.push_str(state);
        text.push_str(" route=");
        text.push_str(debugger_route_primary_step(state, name));
        text.push_str(" escalation=");
        text.push_str(if state == "ready_to_escalate" {
            "true"
        } else {
            "false"
        });
        text.push_str(" next=");
        let steps = next_step_kinds(name, analysis);
        for (index, step) in steps.iter().enumerate() {
            if index > 0 {
                text.push(',');
            }
            text.push_str(step);
        }
    }
    text
}

fn append_target_json(json: &mut String, name: &str, analysis: &AnalysisSnapshot) {
    json.push('{');
    json.push_str("\"name\":");
    append_json_string(json, name);
    let _ = write!(json, ",\"rank\":{}", rank(analysis));
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
    json.push_str("},\"debugger_posture\":");
    append_debugger_posture_json(json, analysis);
    json.push_str(",\"debugger_route\":");
    append_debugger_route_json(json, name, analysis);
    json.push_str(",\"first_missing_transition\":");
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
        json.push_str(",\"command\":");
        append_json_string(json, &next_step_command(step, name));
        json.push_str(",\"reason\":");
        append_json_string(json, next_step_reason(step));
        json.push('}');
    }
    json.push(']');
}

fn append_debugger_posture_json(json: &mut String, analysis: &AnalysisSnapshot) {
    json.push('{');
    json.push_str("\"state\":");
    append_json_string(json, debugger_posture_state(analysis));
    json.push_str(",\"confidence\":");
    append_json_string(json, debugger_posture_confidence(analysis));
    json.push_str(",\"recommended_action\":");
    append_json_string(json, debugger_posture_action(analysis));
    json.push_str(",\"reason\":");
    append_json_string(json, debugger_posture_reason(analysis));
    json.push('}');
}

fn append_debugger_route_json(json: &mut String, name: &str, analysis: &AnalysisSnapshot) {
    let state = debugger_posture_state(analysis);
    let primary_step = debugger_route_primary_step(state, name);
    let fallback_step = debugger_route_fallback_step(state);
    let primary_command = debugger_route_command(primary_step, name);
    let fallback_command = debugger_route_command(fallback_step, name);
    json.push('{');
    append_route_step(json, "primary_step", primary_step, &primary_command, false);
    append_route_step(
        json,
        "fallback_step",
        fallback_step,
        &fallback_command,
        true,
    );
    json.push_str(",\"escalation_allowed\":");
    json.push_str(if state == "ready_to_escalate" {
        "true"
    } else {
        "false"
    });
    json.push_str(",\"reason\":");
    append_json_string(json, debugger_route_reason(state, analysis));
    json.push('}');
}

fn append_route_step(json: &mut String, field: &str, kind: &str, command: &str, comma: bool) {
    if comma {
        json.push(',');
    }
    json.push('"');
    json.push_str(field);
    json.push_str("\":{\"kind\":");
    append_json_string(json, kind);
    json.push_str(",\"command\":");
    append_json_string(json, command);
    json.push('}');
}

fn next_step_kinds(name: &str, analysis: &AnalysisSnapshot) -> Vec<&'static str> {
    let mut steps = vec!["read_analysis"];
    if has_protocol_surface(name) {
        steps.push("read_protocol_plan");
    }
    if !analysis.missing_transitions.is_empty() {
        steps.push("collect_missing_evidence");
    }
    if !has_protocol_surface(name) && name.starts_with("scan:") {
        steps.push("check_protocol_entry");
    }
    steps
}

fn next_step_reason(step: &str) -> &'static str {
    match step {
        "read_protocol_plan" => "follow companion protocol reading order",
        "collect_missing_evidence" => "inspect missing transitions before escalation",
        "check_protocol_entry" => "target name looks protocol-shaped but no surface resolved",
        _ => "inspect the diagnosis spine",
    }
}

fn next_step_command(step: &str, name: &str) -> String {
    match step {
        "read_protocol_plan" | "check_protocol_entry" => {
            if let Some((protocol, _)) = scan_target_protocol_entry(name) {
                let mut command = String::with_capacity(32 + protocol.len());
                command.push_str("cargo run -- --list-entries ");
                command.push_str(protocol);
                command
            } else {
                "cargo run -- --list-protocols".into()
            }
        }
        "collect_missing_evidence" => rerun_target_command(name, false),
        _ => rerun_target_command(name, false),
    }
}

fn debugger_route_primary_step(state: &str, name: &str) -> &'static str {
    match state {
        "healthy" => "observe",
        "ready_to_escalate" => "open_protocol_reading",
        "needs_evidence" => "open_anomaly_flow",
        "needs_hypothesis_review" => "compare_hypotheses",
        _ if has_protocol_surface(name) => "open_protocol_reading",
        _ => "open_analysis",
    }
}

fn debugger_route_fallback_step(state: &str) -> &'static str {
    match state {
        "healthy" => "open_summary",
        "ready_to_escalate" => "open_analysis",
        "needs_evidence" => "open_analysis",
        "needs_hypothesis_review" => "open_protocol_reading",
        _ => "open_summary",
    }
}

fn debugger_route_reason(state: &str, analysis: &AnalysisSnapshot) -> &'static str {
    match state {
        "healthy" => "target is stable; keep the baseline visible",
        "ready_to_escalate" => {
            "direct evidence is strong enough to follow protocol-specific reading"
        }
        "needs_evidence" if !analysis.missing_transitions.is_empty() => {
            "missing transitions should be inspected before escalation"
        }
        "needs_evidence" => "runtime evidence is too thin for escalation",
        "needs_hypothesis_review" => "competing hypotheses should be compared before action",
        _ => "analysis is the safest shared starting point",
    }
}

fn debugger_route_command(step: &str, name: &str) -> String {
    match step {
        "observe" | "open_summary" => rerun_target_command(name, true),
        "open_protocol_reading" => {
            if let Some((protocol, _)) = scan_target_protocol_entry(name) {
                let mut command = String::with_capacity(32 + protocol.len());
                command.push_str("cargo run -- --list-entries ");
                command.push_str(protocol);
                command
            } else {
                rerun_target_command(name, false)
            }
        }
        "compare_hypotheses" => rerun_target_findings_command(name),
        _ => rerun_target_command(name, false),
    }
}

fn rerun_target_command(name: &str, summary_only: bool) -> String {
    let mut command = target_cli_prefix(name);
    command.push_str(" --json");
    if summary_only {
        command.push_str(" --summary-only");
    }
    command
}

fn rerun_target_findings_command(name: &str) -> String {
    let mut command = target_cli_prefix(name);
    command.push_str(" --findings --json");
    command
}

fn target_cli_prefix(name: &str) -> String {
    if let Some((protocol, entry)) = scan_target_protocol_entry(name) {
        let mut command = String::with_capacity(40 + protocol.len() + entry.len());
        command.push_str("cargo run -- --protocol ");
        command.push_str(protocol);
        command.push_str(" --entry ");
        command.push_str(entry);
        command
    } else {
        "cargo run -- --debug-session".into()
    }
}

fn has_protocol_surface(name: &str) -> bool {
    let Some((protocol, entry)) = scan_target_protocol_entry(name) else {
        return false;
    };
    protocol_surface(protocol, entry).is_some()
}

fn debugger_posture_state(analysis: &AnalysisSnapshot) -> &'static str {
    if !analysis.missing_transitions.is_empty()
        || analysis.automation_outcome == "collect_more_evidence"
        || analysis.evidence_posture == "missing_transition"
    {
        "needs_evidence"
    } else if analysis.automation_outcome == "targeted_escalation"
        || analysis.evidence_posture == "direct_protocol_signal"
    {
        "ready_to_escalate"
    } else if analysis.automation_outcome == "multi_hypothesis"
        || analysis.evidence_posture == "ambiguous_multi_hypothesis"
    {
        "needs_hypothesis_review"
    } else if analysis.automation_outcome == "advisory_only" {
        "advisory"
    } else if analysis.target_status.label() == "healthy" {
        "healthy"
    } else {
        "needs_human_review"
    }
}

fn debugger_posture_confidence(analysis: &AnalysisSnapshot) -> &'static str {
    match debugger_posture_state(analysis) {
        "ready_to_escalate" => "high",
        "needs_evidence" | "needs_hypothesis_review" => "medium",
        "healthy" => "high",
        _ => "low",
    }
}

fn debugger_posture_action(analysis: &AnalysisSnapshot) -> &'static str {
    match debugger_posture_state(analysis) {
        "ready_to_escalate" => "escalate_protocol_signal",
        "needs_evidence" => "collect_missing_runtime_evidence",
        "needs_hypothesis_review" => "compare_competing_hypotheses",
        "healthy" => "observe_stable_baseline",
        "advisory" => "observe_only",
        _ => "read_analysis_before_automation",
    }
}

fn debugger_posture_reason(analysis: &AnalysisSnapshot) -> &'static str {
    match debugger_posture_state(analysis) {
        "ready_to_escalate" => "direct protocol evidence supports a targeted next step",
        "needs_evidence" => "the current conclusion depends on missing runtime evidence",
        "needs_hypothesis_review" => "multiple plausible explanations remain active",
        "healthy" => "no debugger action is required for this target",
        "advisory" => "the signal is useful but not strong enough for automation",
        _ => "the debugger cannot narrow the conclusion without operator review",
    }
}
