use gewyvern::export::ExportBundle;
use gewyvern::http::HttpTransactionView;
use std::fmt::Write;

use super::*;
use crate::render_utils::*;

mod debug_session;
mod debug_targets;
mod debugger_console;
mod http_render;
mod scan;
mod scan_surface;
mod sidecar;
mod training;

use self::sidecar::append_external_sidecar_fields;

pub(crate) fn collect_analyses(outputs: &[(String, ExportBundle)]) -> Vec<AnalysisSnapshot> {
    outputs
        .iter()
        .map(|(_, export)| analysis_snapshot(export))
        .collect()
}

pub(super) fn summary_line(name: &str, export: &ExportBundle) -> String {
    self::sidecar::summary_line(name, export)
}

pub(super) fn summary_line_with_analysis(
    name: &str,
    export: &ExportBundle,
    analysis: &AnalysisSnapshot,
) -> String {
    self::sidecar::summary_line_with_analysis(name, export, analysis)
}

pub(super) fn http_transactions_text(transactions: &[HttpTransactionView]) -> String {
    self::http_render::http_transactions_text(transactions)
}

pub(super) fn http_transactions_json(transactions: &[HttpTransactionView]) -> String {
    self::http_render::http_transactions_json(transactions)
}

#[cfg(test)]
pub(super) fn scan_report_json(outputs: &[(String, ExportBundle)]) -> String {
    self::scan::scan_report_json(outputs)
}

pub(super) fn scan_report_json_with_analyses(
    outputs: &[(String, ExportBundle)],
    analyses: &[AnalysisSnapshot],
) -> String {
    self::scan::scan_report_json_with_analyses(outputs, analyses)
}

pub(super) fn single_target_report_json_with_analysis(
    name: &str,
    export: &ExportBundle,
    analysis: &AnalysisSnapshot,
) -> String {
    self::scan::single_target_report_json_with_analysis(name, export, analysis)
}

#[cfg(test)]
pub(super) fn scan_report_html(outputs: &[(String, ExportBundle)]) -> String {
    self::scan::scan_report_html(outputs)
}

pub(super) fn scan_report_html_with_analyses(
    outputs: &[(String, ExportBundle)],
    analyses: &[AnalysisSnapshot],
) -> String {
    self::scan::scan_report_html_with_analyses(outputs, analyses)
}

pub(super) fn single_target_report_html_with_analysis(
    name: &str,
    export: &ExportBundle,
    analysis: &AnalysisSnapshot,
) -> String {
    self::scan::single_target_report_html_with_analysis(name, export, analysis)
}

#[cfg(test)]
pub(super) fn scan_report_text(outputs: &[(String, ExportBundle)]) -> String {
    self::scan::scan_report_text(outputs)
}

pub(super) fn scan_report_text_with_analyses(
    outputs: &[(String, ExportBundle)],
    analyses: &[AnalysisSnapshot],
) -> String {
    self::scan::scan_report_text_with_analyses(outputs, analyses)
}

pub(super) fn render_scan_outputs(cli: &Cli, outputs: &[(String, ExportBundle)]) -> String {
    self::scan::render_scan_outputs(cli, outputs)
}

pub(super) fn render_report_outputs(cli: &Cli, outputs: &[(String, ExportBundle)]) -> String {
    self::scan::render_report_outputs(cli, outputs)
}

pub(super) fn render_debugger_console_outputs(
    cli: &Cli,
    outputs: &[(String, ExportBundle)],
) -> String {
    self::debugger_console::render_debugger_console_outputs(cli, outputs)
}

pub(super) fn render_debug_session_outputs(
    cli: &Cli,
    outputs: &[(String, ExportBundle)],
) -> String {
    self::debug_session::render_debug_session_outputs(cli, outputs)
}

#[cfg(test)]
pub(super) fn training_example_json(name: &str, export: &ExportBundle) -> String {
    self::training::training_example_json(name, export)
}

pub(super) fn training_example_json_with_analysis(
    name: &str,
    export: &ExportBundle,
    analysis: &AnalysisSnapshot,
) -> String {
    self::training::training_example_json_with_analysis(name, export, analysis)
}

pub(super) fn training_example_json_array(
    outputs: &[(String, ExportBundle)],
    analyses: &[AnalysisSnapshot],
) -> String {
    self::training::training_example_json_array(outputs, analyses)
}

pub(super) fn scan_analysis_json_array(
    outputs: &[(String, ExportBundle)],
    analyses: &[AnalysisSnapshot],
) -> String {
    let estimated_capacity = 2 + outputs
        .iter()
        .zip(analyses.iter())
        .map(|((name, _), analysis)| {
            name.len() + estimate_analysis_snapshot_json_capacity(analysis) + 24
        })
        .sum::<usize>();
    let mut json = String::with_capacity(estimated_capacity);
    json.push('[');
    for (index, ((name, _), analysis)) in outputs.iter().zip(analyses.iter()).enumerate() {
        if index > 0 {
            json.push(',');
        }
        json.push_str("{\"target\":");
        append_json_string(&mut json, name);
        json.push_str(",\"analysis\":");
        append_analysis_snapshot_json(&mut json, analysis);
        json.push('}');
    }
    json.push(']');
    json
}

pub(super) fn summary_json(name: &str, export: &ExportBundle) -> String {
    let analysis = analysis_snapshot(export);
    summary_json_with_analysis(name, export, &analysis)
}

pub(super) fn summary_json_with_analysis(
    name: &str,
    export: &ExportBundle,
    analysis: &AnalysisSnapshot,
) -> String {
    use std::fmt::Write;

    let mut json = String::from("{\"kind\":\"single\",\"name\":\"");
    json.push_str(name);
    json.push_str("\",\"demo\":\"");
    json.push_str(name);
    json.push_str("\",\"template_id\":\"");
    json.push_str(&export.template_id);
    json.push_str("\",");
    append_analysis_context_json(&mut json, export, analysis);
    let _ = write!(
        json,
        ",\"fragments_loaded\":{},\"hookpoints_failed\":{},\"accepted_facts\":{},\"rejected_facts\":{},\"flows\":{},\"program_findings\":{},\"module_findings\":{},\"reasons\":{},\"degraded\":{}",
        export.debug_summary.fragments_loaded,
        export.debug_summary.hookpoints_failed,
        export.debug_summary.accepted_facts,
        export.debug_summary.rejected_facts,
        export.debug_summary.flows,
        export.debug_summary.program_findings,
        export.debug_summary.module_findings,
        export.debug_summary.reasons,
        export.debug_summary.degraded
    );
    json.push_str(",\"suspect_modules\":[");
    for (index, finding) in export.program_findings.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        json.push('"');
        json.push_str(&finding.module_label);
        json.push('"');
    }
    json.push_str("],\"protocol_flows\":");
    json.push('[');
    append_protocol_flow_summaries_json_from_snapshot(&mut json, analysis);
    json.push(']');
    json.push_str(",\"process_network_profiles\":");
    json.push('[');
    append_process_network_profiles_json_from_snapshot(&mut json, analysis);
    json.push(']');
    append_external_sidecar_fields(&mut json, analysis);
    json.push('}');
    json
}

pub(super) fn findings_text(name: &str, export: &ExportBundle) -> String {
    let locale = UiLocale::detect();
    if export.module_findings.is_empty() {
        return format!("{name}: {}", locale.none());
    }

    let mut lines = vec![format!("{name}:")];
    for finding in &export.module_findings {
        let process = finding
            .process
            .as_ref()
            .map(|process| format!("{}(pid={})", process.comm, process.pid))
            .unwrap_or_else(|| locale.none().to_string());
        let traces = if finding.evidence_trace.is_empty() {
            locale.none().to_string()
        } else {
            finding.evidence_trace.join("|")
        };
        let phases = if finding.phases.is_empty() {
            locale.none().to_string()
        } else {
            finding.phases.join(",")
        };
        let transitions = if finding.phase_transitions.is_empty() {
            locale.none().to_string()
        } else {
            finding.phase_transitions.join(",")
        };
        let summaries = if finding.summaries.is_empty() {
            locale.none().to_string()
        } else {
            finding.summaries.join("|")
        };
        lines.push(format!(
            "  {}={} {}={} {}={} {}={} {}={} {}={} {}={} {}={} {}={} {}={}",
            locale.label("severity"),
            module_severity_label(&finding.severity),
            locale.label("module"),
            finding.module_label,
            locale.label("phases"),
            phases,
            locale.label("phase_transitions"),
            transitions,
            locale.label("process"),
            process,
            locale.label("operation"),
            operation_label(&finding.operation),
            locale.label("suspect_areas"),
            finding.suspect_areas.join(","),
            locale.label("causes"),
            finding
                .causes
                .iter()
                .map(finding_cause_label)
                .collect::<Vec<_>>()
                .join(","),
            locale.label("supporting"),
            finding.supporting_fragments.join(","),
            locale.label("trace"),
            traces,
        ));
        lines.push(format!("  {}={}", locale.label("summary"), summaries));
    }

    lines.join("\n")
}

pub(super) fn findings_json(name: &str, export: &ExportBundle) -> String {
    let analysis = analysis_snapshot(export);
    findings_json_with_analysis(name, export, &analysis)
}

pub(super) fn findings_json_with_analysis(
    name: &str,
    export: &ExportBundle,
    analysis: &AnalysisSnapshot,
) -> String {
    let mut json = String::from("{\"kind\":\"single\",\"name\":\"");
    json.push_str(name);
    json.push_str("\",\"demo\":\"");
    json.push_str(name);
    json.push_str("\",\"template_id\":\"");
    json.push_str(&export.template_id);
    json.push_str("\",");
    append_analysis_context_json(&mut json, export, analysis);
    json.push_str(",\"module_findings\":[");
    for (index, finding) in export.module_findings.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        append_module_finding_json(&mut json, finding);
    }
    json.push_str("],\"program_findings\":[");
    for (index, finding) in export.program_findings.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        append_program_finding_json(&mut json, finding);
    }
    json.push_str("],\"process_network_profiles\":");
    json.push('[');
    append_process_network_profiles_json_from_snapshot(&mut json, analysis);
    json.push(']');
    append_external_sidecar_fields(&mut json, analysis);
    json.push('}');
    json
}

pub(super) fn append_module_finding_json(
    json: &mut String,
    finding: &gewyvern::flow::ModuleFinding,
) {
    json.push('{');
    let cause_values = finding
        .causes
        .iter()
        .map(finding_cause_label)
        .map(str::to_string)
        .collect::<Vec<_>>();
    json.push_str("\"module_label\":\"");
    json.push_str(&finding.module_label);
    json.push_str("\",\"severity\":\"");
    json.push_str(module_severity_label(&finding.severity));
    json.push_str("\",\"process\":");
    append_process_json(json, finding.process.as_ref());
    json.push_str(",\"operation\":\"");
    append_operation_label(json, &finding.operation);
    json.push_str("\",\"network_module_kinds\":");
    append_string_list_json(json, &finding.network_module_kinds);
    json.push_str(",\"phases\":");
    append_string_list_json(json, &finding.phases);
    json.push_str(",\"phase_transitions\":");
    append_string_list_json(json, &finding.phase_transitions);
    json.push_str(",\"suspect_areas\":");
    append_string_list_json(json, &finding.suspect_areas);
    json.push_str(",\"causes\":");
    append_string_list_json(json, &cause_values);
    json.push_str(",\"supporting_fragments\":");
    append_string_list_json(json, &finding.supporting_fragments);
    json.push_str(",\"program_flows\":[");
    for (index, flow) in finding.program_flows.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        let _ = write!(json, "{}", flow.0);
    }
    json.push_str("],\"summaries\":");
    append_string_list_json(json, &finding.summaries);
    json.push_str(",\"evidence_trace\":");
    append_string_list_json(json, &finding.evidence_trace);
    json.push('}');
}

pub(super) fn append_program_finding_json(
    json: &mut String,
    finding: &gewyvern::flow::ProgramFinding,
) {
    let _ = write!(json, "{{\"program_flow\":{}", finding.program_flow.0);
    json.push_str(",\"module_label\":\"");
    json.push_str(&finding.module_label);
    json.push_str("\",\"network_module_kind\":\"");
    json.push_str(&finding.network_module_kind);
    json.push_str("\",\"phase\":");
    if let Some(phase) = finding.phase.as_deref() {
        json.push('"');
        json.push_str(phase);
        json.push('"');
    } else {
        json.push_str("null");
    }
    json.push_str(",\"phase_transition\":");
    if let Some(transition) = finding.phase_transition.as_deref() {
        json.push('"');
        json.push_str(transition);
        json.push('"');
    } else {
        json.push_str("null");
    }
    json.push_str(",\"suspect_area\":\"");
    json.push_str(&finding.suspect_area);
    json.push_str("\",\"cause\":\"");
    json.push_str(finding_cause_label(&finding.cause));
    json.push_str("\",\"process\":");
    append_process_json(json, finding.process.as_ref());
    json.push_str(",\"operation\":\"");
    append_operation_label(json, &finding.operation);
    json.push_str("\",\"summary\":\"");
    json.push_str(&finding.summary);
    json.push_str("\",\"supporting_fragments\":");
    append_string_list_json(json, &finding.supporting_fragments);
    json.push_str(",\"evidence_trace\":");
    append_string_list_json(json, &finding.evidence_trace);
    json.push('}');
}

pub(super) fn append_http_transaction_json(json: &mut String, transaction: &HttpTransactionView) {
    let _ = write!(json, "{{\"id\":{}", transaction.id.0);
    json.push_str(",\"client_process\":");
    append_process_json(json, transaction.client_process.as_ref());
    json.push_str(",\"server_process\":");
    append_process_json(json, transaction.server_process.as_ref());
    json.push_str(",\"verdict\":\"");
    json.push_str(http_transaction_verdict_label(&transaction.verdict));
    json.push_str("\",\"severity\":");
    if let Some(severity) = transaction.severity.as_ref() {
        json.push('"');
        json.push_str(module_severity_label(severity));
        json.push('"');
    } else {
        json.push_str("null");
    }
    json.push_str(",\"degraded\":");
    json.push_str(if transaction.degraded {
        "true"
    } else {
        "false"
    });
    json.push_str(",\"suspect_sides\":");
    append_str_list_json(
        json,
        transaction
            .suspect_sides
            .iter()
            .map(http_suspect_side_label),
    );
    json.push_str(",\"phases\":");
    append_string_list_json(json, &transaction.phases);
    json.push_str(",\"components\":[");
    for (index, component) in transaction.components.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        append_http_component_json(json, component);
    }
    json.push_str("],\"finding_summaries\":");
    append_string_list_json(json, &transaction.finding_summaries);
    json.push_str(",\"summaries\":");
    append_string_list_json(json, &transaction.summaries);
    json.push('}');
}

pub(super) fn append_http_component_json(
    json: &mut String,
    component: &gewyvern::http::HttpComponentRef,
) {
    json.push_str("{\"template_id\":\"");
    json.push_str(&component.template_id);
    json.push_str("\",\"kind\":\"");
    json.push_str(http_component_kind_label(&component.kind));
    json.push_str("\",\"operation\":\"");
    append_operation_label(json, &component.operation);
    json.push_str("\"}");
}

pub(super) fn append_analysis_context_json(
    json: &mut String,
    export: &ExportBundle,
    analysis: &AnalysisSnapshot,
) {
    append_ingest_context_json(json, export);
    json.push(',');
    append_analysis_spine_json(json, analysis);
}

pub(super) fn append_ingest_context_json(json: &mut String, export: &ExportBundle) {
    json.push_str("\"ingest_mode\":\"");
    json.push_str(ingest_mode_for_export(export));
    json.push_str("\",\"ingest_mode_note\":\"");
    json.push_str(&ingest_mode_note_for_export(export));
    json.push_str("\",\"ingest_trust_mode\":\"");
    json.push_str(&export.ingest_trust_mode);
    json.push_str("\",\"pid_attribution_status\":\"");
    json.push_str(pid_attribution_status_for_export(export));
    json.push_str("\",\"pid_attribution_note\":\"");
    json.push_str(&pid_attribution_note_for_export(export));
    json.push('"');
}

pub(super) fn append_analysis_spine_json(json: &mut String, analysis: &AnalysisSnapshot) {
    json.push_str("\"ambiguous\":");
    json.push_str(if analysis.primary_process_profile_ambiguous {
        "true"
    } else {
        "false"
    });
    json.push_str(",\"competing_hypotheses\":");
    append_string_list_json(json, &analysis.competing_hypotheses);
    json.push_str(",\"operations\":");
    append_string_list_json(json, &analysis.operations);
    json.push_str(",\"phases\":");
    append_string_list_json(json, &analysis.phases);
    json.push_str(",\"missing_transitions\":");
    append_string_list_json(json, &analysis.missing_transitions);
    json.push_str(",\"suspect_areas\":");
    append_string_list_json(json, &analysis.suspect_areas);
    json.push_str(",\"primary_module_kind\":\"");
    json.push_str(&analysis.primary_module_kind);
    json.push_str("\",\"primary_module_family\":\"");
    json.push_str(&analysis.primary_module_family);
    json.push_str("\",\"primary_failure_stage\":\"");
    json.push_str(&analysis.primary_failure_stage);
    json.push_str("\",\"primary_stage_family\":\"");
    json.push_str(stage_family_label(&analysis.primary_failure_stage));
    json.push_str("\",\"primary_failure_mode\":\"");
    json.push_str(&analysis.primary_failure_mode);
    json.push_str("\",\"primary_failure_mode_family\":\"");
    json.push_str(failure_mode_family_label(&analysis.primary_failure_mode));
    json.push_str("\",\"primary_failure_detail\":\"");
    json.push_str(&analysis.primary_failure_detail);
    json.push_str("\",\"primary_failure_detail_family\":\"");
    json.push_str(failure_detail_family_label(
        &analysis.primary_failure_detail,
    ));
    json.push_str("\",\"primary_failure_confidence\":\"");
    json.push_str(&analysis.primary_failure_confidence);
    json.push_str("\",\"primary_failure_basis\":\"");
    json.push_str(&analysis.primary_failure_basis);
    json.push_str("\",\"evidence_posture\":\"");
    json.push_str(&analysis.evidence_posture);
    json.push_str("\",\"automation_outcome\":\"");
    json.push_str(&analysis.automation_outcome);
    json.push_str("\",\"operator_guidance_status\":\"");
    json.push_str(&analysis.operator_guidance_status);
    json.push_str("\",\"operator_guidance_action\":\"");
    json.push_str(&analysis.operator_guidance_action);
    json.push_str("\",\"operator_guidance_reason\":\"");
    json.push_str(&analysis.operator_guidance_reason);
    json.push_str("\",\"operator_guidance_summary\":\"");
    json.push_str(&analysis.operator_guidance_summary);
    json.push_str("\",\"augmentations\":");
    append_analysis_augmentations_json(json, &analysis.augmentations);
    append_external_sidecar_fields(json, analysis);
}
