use super::debug_targets::{rank, ranked_targets, scan_target_protocol_entry};
use super::*;

pub(super) fn render_debugger_console_outputs(
    cli: &Cli,
    outputs: &[(String, ExportBundle)],
) -> String {
    let analyses = collect_analyses(outputs);
    if cli.json {
        debugger_console_json(outputs, &analyses)
    } else {
        debugger_console_text(outputs, &analyses)
    }
}

pub(super) fn debugger_console_json(
    outputs: &[(String, ExportBundle)],
    analyses: &[AnalysisSnapshot],
) -> String {
    let indexed = ranked_targets(outputs, analyses);
    let attention_count = indexed
        .iter()
        .filter(|(_, analysis)| rank(analysis) <= 1)
        .count();
    let mut json = String::with_capacity(512 + indexed.len() * 512);
    let _ = write!(
        json,
        "{{\"surface\":\"local_debugger_console\",\"target_count\":{},\"attention_count\":{},\"recommended_focus\":",
        indexed.len(),
        attention_count
    );
    match indexed.first() {
        Some((name, analysis)) => append_target_json(&mut json, name, analysis),
        None => json.push_str("null"),
    }
    json.push_str(",\"commands\":{");
    json.push_str("\"rerun_scan\":");
    append_json_string(
        &mut json,
        "cargo run -- --scan-all --debugger-console --json",
    );
    json.push_str(",\"focus_debug_session\":");
    match indexed.first() {
        Some((name, _)) => append_json_string(&mut json, &target_debug_session_command(name)),
        None => json.push_str("null"),
    }
    json.push('}');
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

pub(super) fn debugger_console_text(
    outputs: &[(String, ExportBundle)],
    analyses: &[AnalysisSnapshot],
) -> String {
    let indexed = ranked_targets(outputs, analyses);
    let attention_count = indexed
        .iter()
        .filter(|(_, analysis)| rank(analysis) <= 1)
        .count();
    let mut text = String::with_capacity(96 + indexed.len() * 160);
    let _ = write!(
        text,
        "debugger_console: targets={} attention={}",
        indexed.len(),
        attention_count
    );
    if let Some((name, analysis)) = indexed.first() {
        text.push_str("\nrecommended_focus: ");
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
        text.push_str(" posture=");
        text.push_str(&analysis.evidence_posture);
        text.push_str(" outcome=");
        text.push_str(&analysis.automation_outcome);
        text.push_str(" stage=");
        text.push_str(&analysis.primary_failure_stage);
        text.push_str(" mode=");
        text.push_str(&analysis.primary_failure_mode);
        if let Some(transition) = analysis.missing_transitions.first() {
            text.push_str(" missing=");
            text.push_str(transition);
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
    json.push_str(",\"primary_module_family\":");
    append_json_string(json, &analysis.primary_module_family);
    json.push_str(",\"primary_failure_stage\":");
    append_json_string(json, &analysis.primary_failure_stage);
    json.push_str(",\"primary_failure_mode\":");
    append_json_string(json, &analysis.primary_failure_mode);
    json.push_str(",\"primary_failure_detail\":");
    append_json_string(json, &analysis.primary_failure_detail);
    json.push_str(",\"operator_guidance_action\":");
    append_json_string(json, &analysis.operator_guidance_action);
    json.push_str(",\"operator_guidance_summary\":");
    append_json_string(json, &analysis.operator_guidance_summary);
    json.push_str(",\"debug_session_command\":");
    append_json_string(json, &target_debug_session_command(name));
    json.push_str(",\"first_missing_transition\":");
    match analysis.missing_transitions.first() {
        Some(value) => append_json_string(json, value),
        None => json.push_str("null"),
    }
    json.push('}');
}

fn target_debug_session_command(name: &str) -> String {
    if let Some((protocol, entry)) = scan_target_protocol_entry(name) {
        let mut command = String::with_capacity(62 + protocol.len() + entry.len());
        command.push_str("cargo run -- --protocol ");
        command.push_str(protocol);
        command.push_str(" --entry ");
        command.push_str(entry);
        command.push_str(" --debug-session --json");
        command
    } else {
        "cargo run -- --debug-session --json".into()
    }
}
