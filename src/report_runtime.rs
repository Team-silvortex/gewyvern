use gewyvern::export::ExportBundle;
use gewyvern::http::HttpTransactionView;
use std::fmt::Write;

use super::*;
use crate::diagnosis_runtime::append_external_sidecar_context_json;
use crate::render_utils::*;

fn external_sidecar_hint_summary(analysis: &AnalysisSnapshot) -> (String, String) {
    let mut enrichment = "none".to_string();
    let mut opinion = "none".to_string();
    for item in &analysis.augmentations {
        if item.name == "external_evidence_chain_enrichment" {
            let handoff = item
                .data_json
                .as_deref()
                .and_then(|data| extract_json_string_value(data, "external_handoff_readiness"))
                .unwrap_or_else(|| "advisory_only".to_string());
            let merge_hint = item
                .data_json
                .as_deref()
                .and_then(|data| extract_json_string_value(data, "external_merge_hint"))
                .unwrap_or_else(|| "augmentations_only".to_string());
            enrichment = format!("{}+{}", handoff, merge_hint);
        } else if item.name == "external_diagnostic_opinion" {
            let handoff = item
                .data_json
                .as_deref()
                .and_then(|data| extract_json_string_value(data, "external_handoff_readiness"))
                .unwrap_or_else(|| "mergeable".to_string());
            let merge_hint = item
                .data_json
                .as_deref()
                .and_then(|data| extract_json_string_value(data, "external_merge_hint"))
                .unwrap_or_else(|| "sidecar_only_opinion".to_string());
            opinion = format!("{}+{}", handoff, merge_hint);
        }
    }
    (enrichment, opinion)
}

fn extract_json_string_value(input: &str, key: &str) -> Option<String> {
    let needle = format!("\"{}\":\"", key);
    let start = input.find(&needle)? + needle.len();
    let rest = &input[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn append_external_sidecar_context_field(json: &mut String, analysis: &AnalysisSnapshot) {
    json.push_str(",\"external_sidecar_context\":");
    append_external_sidecar_context_json(json, analysis);
}

pub(super) fn summary_line(name: &str, export: &ExportBundle) -> String {
    let analysis = analysis_snapshot(export);
    summary_line_with_analysis(name, export, &analysis)
}

pub(super) fn summary_line_with_analysis(
    name: &str,
    export: &ExportBundle,
    analysis: &AnalysisSnapshot,
) -> String {
    let locale = UiLocale::detect();
    let suspect_areas = if export.program_findings.is_empty() {
        locale.none().to_string()
    } else {
        export
            .program_findings
            .iter()
            .map(|finding| finding.suspect_area.clone())
            .collect::<Vec<_>>()
            .join(",")
    };
    let suspect_modules = if export.program_findings.is_empty() {
        locale.none().to_string()
    } else {
        export
            .program_findings
            .iter()
            .map(|finding| finding.module_label.clone())
            .collect::<Vec<_>>()
            .join(",")
    };
    let protocol_flows = protocol_flow_summaries_text_from_snapshot(analysis);
    let process_profiles = process_network_profiles_text_from_snapshot(analysis);
    let augmentations = analysis_augmentation_names_text(&analysis.augmentations);
    let (sidecar_enrichment, sidecar_opinion) = external_sidecar_hint_summary(analysis);
    let ingest_mode_note = ingest_mode_note_for_export(export);
    format!(
        "{name}: {}={} ingest_mode={} ingest_mode_note={} {}={} pid_attribution_status={} operator_guidance_status={} operator_guidance_action={} operator_guidance_reason={} ambiguous={} competing_hypotheses={} augmentations={} external_enrichment_hint={} external_diagnostic_opinion_hint={} {}={} {}={} {}={} {}={} {}={} {}={} {}={} {}={} {}={} {}={} {}={} protocol_flows={} process_network_profiles={}",
        locale.label("template"),
        export.template_id,
        ingest_mode_for_export(export),
        ingest_mode_note,
        "ingest_trust_mode",
        export.ingest_trust_mode,
        pid_attribution_status_for_export(export),
        analysis.operator_guidance_status,
        analysis.operator_guidance_action,
        analysis.operator_guidance_reason,
        analysis.primary_process_profile_ambiguous,
        string_list_json(&analysis.competing_hypotheses),
        augmentations,
        sidecar_enrichment,
        sidecar_opinion,
        locale.label("fragments_loaded"),
        export.debug_summary.fragments_loaded,
        locale.label("hookpoints_failed"),
        export.debug_summary.hookpoints_failed,
        locale.label("accepted_facts"),
        export.debug_summary.accepted_facts,
        locale.label("rejected_facts"),
        export.debug_summary.rejected_facts,
        locale.label("flows"),
        export.debug_summary.flows,
        locale.label("program_findings"),
        export.debug_summary.program_findings,
        locale.label("module_findings"),
        export.debug_summary.module_findings,
        locale.label("reasons"),
        export.debug_summary.reasons,
        locale.label("degraded"),
        export.debug_summary.degraded,
        locale.label("suspect_areas"),
        suspect_areas,
        locale.label("suspect_modules"),
        suspect_modules,
        protocol_flows,
        process_profiles,
    )
}

pub(super) fn scan_report_json(outputs: &[(String, ExportBundle)]) -> String {
    let analyses = outputs
        .iter()
        .map(|(_, export)| analysis_snapshot(export))
        .collect::<Vec<_>>();
    scan_report_json_with_analyses(outputs, &analyses)
}

pub(super) fn scan_report_json_with_analyses(
    outputs: &[(String, ExportBundle)],
    analyses: &[AnalysisSnapshot],
) -> String {
    let total_targets = outputs.len();
    let (healthy_targets, attention_targets, idle_targets) = scan_target_status_counts(&analyses);
    let estimated_capacity = 160
        + outputs
            .iter()
            .zip(analyses.iter())
            .map(|((name, export), analysis)| {
                estimate_scan_target_json_capacity(name, export, analysis)
            })
            .sum::<usize>();
    let mut json = String::with_capacity(estimated_capacity);
    let _ = write!(
        json,
        "{{\"kind\":\"scan\",\"name\":null,\"target_count\":{},\"scan_all\":true,\"total_targets\":{},\"healthy_targets\":{},\"attention_targets\":{},\"idle_targets\":{},\"targets\":[",
        total_targets, total_targets, healthy_targets, attention_targets, idle_targets
    );
    for (index, ((name, export), analysis)) in outputs.iter().zip(analyses.iter()).enumerate() {
        if index > 0 {
            json.push(',');
        }
        append_scan_target_json(&mut json, name, export, analysis);
    }
    json.push_str("]}");
    json
}

pub(super) fn scan_report_html(outputs: &[(String, ExportBundle)]) -> String {
    let analyses = outputs
        .iter()
        .map(|(_, export)| analysis_snapshot(export))
        .collect::<Vec<_>>();
    let total_targets = outputs.len();
    let (healthy_targets, attention_targets, idle_targets) = scan_target_status_counts(&analyses);

    let mut family_counts = std::collections::BTreeMap::<String, usize>::new();
    for analysis in &analyses {
        let family = module_family_label(&analysis.primary_module_kind).to_string();
        *family_counts.entry(family).or_default() += 1;
    }
    let mut family_summary = String::new();
    for (family, count) in family_counts {
        let _ = write!(
            family_summary,
            "<div class=\"pill\"><span class=\"tag family-{}\">{}</span> {}</div>",
            family, family, count
        );
    }

    let mut sorted_outputs = outputs
        .iter()
        .zip(analyses.iter())
        .map(|((name, export), analysis)| (name, export, analysis))
        .collect::<Vec<_>>();
    sorted_outputs.sort_by(
        |(left_name, _, left_analysis), (right_name, _, right_analysis)| {
            let left_rank = match left_analysis.target_status {
                ScanTargetStatus::Attention => 0,
                ScanTargetStatus::Healthy => 1,
                ScanTargetStatus::Idle => 2,
            };
            let right_rank = match right_analysis.target_status {
                ScanTargetStatus::Attention => 0,
                ScanTargetStatus::Healthy => 1,
                ScanTargetStatus::Idle => 2,
            };
            left_rank
                .cmp(&right_rank)
                .then_with(|| {
                    left_analysis
                        .primary_module_kind
                        .cmp(&right_analysis.primary_module_kind)
                })
                .then_with(|| left_name.cmp(right_name))
        },
    );

    let mut cards = String::with_capacity(
        sorted_outputs
            .iter()
            .map(|(name, export, analysis)| {
                estimate_scan_target_html_capacity(name, export, analysis)
            })
            .sum::<usize>(),
    );
    for (index, (name, export, analysis)) in sorted_outputs.into_iter().enumerate() {
        if index > 0 {
            cards.push('\n');
        }
        append_scan_target_html_card(&mut cards, name, export, analysis);
    }

    format!(
        "<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>gewyvern scan report</title><style>body{{font-family:ui-sans-serif,system-ui,sans-serif;background:#f6f7fb;color:#18202a;margin:0;padding:24px}}h1,h2,h3{{margin:0 0 12px}}.summary{{display:flex;gap:12px;flex-wrap:wrap;margin:16px 0 24px}}.summary-note{{margin:-10px 0 24px;color:#475569;font-size:14px}}.pill{{background:#fff;border:1px solid #d8dee9;border-radius:999px;padding:10px 14px;font-size:14px}}.tag{{display:inline-flex;align-items:center;border-radius:999px;padding:2px 10px;font-size:12px;font-weight:600}}.family-dns{{background:#dbeafe;color:#1d4ed8}}.family-route{{background:#e0f2fe;color:#0369a1}}.family-connect{{background:#ede9fe;color:#6d28d9}}.family-handshake{{background:#fae8ff;color:#a21caf}}.family-request-response{{background:#dcfce7;color:#166534}}.family-database{{background:#fef3c7;color:#92400e}}.family-auth{{background:#fee2e2;color:#b91c1c}}.family-directory{{background:#ecfccb;color:#3f6212}}.family-messaging{{background:#ffedd5;color:#c2410c}}.family-relay{{background:#d1fae5;color:#047857}}.family-service{{background:#e2e8f0;color:#334155}}.family-general{{background:#f3f4f6;color:#374151}}.stage-dns{{background:#dbeafe;color:#1d4ed8}}.stage-connect{{background:#ede9fe;color:#6d28d9}}.stage-handshake{{background:#fae8ff;color:#a21caf}}.stage-request-response{{background:#dcfce7;color:#166534}}.stage-auth{{background:#fee2e2;color:#b91c1c}}.stage-general{{background:#f3f4f6;color:#374151}}.stage-none{{background:#e5e7eb;color:#6b7280}}.failure-blocked{{background:#fef3c7;color:#92400e}}.failure-timeout{{background:#fee2e2;color:#b91c1c}}.failure-setup{{background:#e0e7ff;color:#4338ca}}.failure-semantic{{background:#ffedd5;color:#c2410c}}.failure-denied{{background:#fce7f3;color:#be185d}}.failure-peer{{background:#d1fae5;color:#047857}}.failure-none{{background:#e5e7eb;color:#6b7280}}.failure-general{{background:#f3f4f6;color:#374151}}.grid{{display:grid;grid-template-columns:repeat(auto-fit,minmax(320px,1fr));gap:16px}}.card{{background:#fff;border:1px solid #d8dee9;border-radius:16px;padding:0;box-shadow:0 6px 24px rgba(15,23,42,0.06);overflow:hidden}}.card summary{{list-style:none;cursor:pointer;padding:18px}}.card summary::-webkit-details-marker{{display:none}}.card-title p{{margin:0}}.card-body{{padding:0 18px 18px}}.conclusion{{display:flex;gap:10px;flex-wrap:wrap;margin:14px 0 0}}.status-attention{{border-color:#f0b429}}.status-healthy{{border-color:#68b984}}.status-idle{{border-color:#cbd5e1}}ul{{padding-left:18px}}li{{margin:6px 0}}</style></head><body><h1>gewyvern Scan Report</h1><div class=\"summary\"><div class=\"pill\">total targets: {}</div><div class=\"pill\">healthy: {}</div><div class=\"pill\">attention: {}</div><div class=\"pill\">idle: {}</div></div><p class=\"summary-note\">attention targets are shown first and expanded by default so the highest-risk paths are easier to inspect.</p><div class=\"summary\">{}</div><div class=\"grid\">{}</div></body></html>",
        total_targets, healthy_targets, attention_targets, idle_targets, family_summary, cards
    )
}

pub(super) fn scan_report_text(outputs: &[(String, ExportBundle)]) -> String {
    let analyses = outputs
        .iter()
        .map(|(_, export)| analysis_snapshot(export))
        .collect::<Vec<_>>();
    scan_report_text_with_analyses(outputs, &analyses)
}

pub(super) fn scan_report_text_with_analyses(
    outputs: &[(String, ExportBundle)],
    analyses: &[AnalysisSnapshot],
) -> String {
    let total_targets = outputs.len();
    let (healthy_targets, attention_targets, idle_targets) = scan_target_status_counts(analyses);
    let estimated_capacity = 96
        + outputs
            .iter()
            .zip(analyses.iter())
            .map(|((name, export), analysis)| {
                estimate_scan_target_text_capacity(name, export, analysis)
            })
            .sum::<usize>();
    let mut text = String::with_capacity(estimated_capacity);
    let _ = write!(
        text,
        "scan_all_report: total_targets={} healthy_targets={} attention_targets={} idle_targets={}",
        total_targets, healthy_targets, attention_targets, idle_targets
    );
    for ((name, export), analysis) in outputs.iter().zip(analyses.iter()) {
        text.push('\n');
        append_scan_target_text(&mut text, name, export, analysis);
    }
    text
}

pub(super) fn render_scan_outputs(cli: &Cli, outputs: &[(String, ExportBundle)]) -> String {
    match cli.report_format {
        Some(ReportFormat::Html) => scan_report_html(outputs),
        Some(ReportFormat::Json) => scan_report_json(outputs),
        None if cli.json => scan_report_json(outputs),
        None => scan_report_text(outputs),
    }
}

pub(super) fn render_report_outputs(cli: &Cli, outputs: &[(String, ExportBundle)]) -> String {
    match cli.report_format {
        Some(ReportFormat::Html) => scan_report_html(outputs),
        Some(ReportFormat::Json) => scan_report_json(outputs),
        None => scan_report_text(outputs),
    }
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
    let mut json = String::from("{\"kind\":\"single\",\"name\":\"");
    json.push_str(name);
    json.push_str("\",\"demo\":\"");
    json.push_str(name);
    json.push_str("\",\"template_id\":\"");
    json.push_str(&export.template_id);
    json.push_str("\",");
    append_analysis_context_json(&mut json, export, analysis);
    json.push_str(",\"fragments_loaded\":");
    json.push_str(&export.debug_summary.fragments_loaded.to_string());
    json.push_str(",\"hookpoints_failed\":");
    json.push_str(&export.debug_summary.hookpoints_failed.to_string());
    json.push_str(",\"accepted_facts\":");
    json.push_str(&export.debug_summary.accepted_facts.to_string());
    json.push_str(",\"rejected_facts\":");
    json.push_str(&export.debug_summary.rejected_facts.to_string());
    json.push_str(",\"flows\":");
    json.push_str(&export.debug_summary.flows.to_string());
    json.push_str(",\"program_findings\":");
    json.push_str(&export.debug_summary.program_findings.to_string());
    json.push_str(",\"module_findings\":");
    json.push_str(&export.debug_summary.module_findings.to_string());
    json.push_str(",\"reasons\":");
    json.push_str(&export.debug_summary.reasons.to_string());
    json.push_str(",\"degraded\":");
    json.push_str(&export.debug_summary.degraded.to_string());
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
    json.push_str(&protocol_flow_summaries_json_from_snapshot(analysis));
    json.push_str(",\"process_network_profiles\":");
    json.push_str(&process_network_profiles_json_from_snapshot(analysis));
    append_external_sidecar_context_field(&mut json, analysis);
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
    json.push_str(&process_network_profiles_json_from_snapshot(analysis));
    append_external_sidecar_context_field(&mut json, analysis);
    json.push('}');
    json
}

pub(super) fn http_transactions_text(transactions: &[HttpTransactionView]) -> String {
    let locale = UiLocale::detect();
    if transactions.is_empty() {
        return locale.none().into();
    }

    let mut text = String::new();
    for (index, tx) in transactions.iter().enumerate() {
        if index > 0 {
            text.push('\n');
        }
        text.push_str("http_transaction#");
        text.push_str(&tx.id.0.to_string());
        text.push_str(": client=");
        if let Some(process) = tx.client_process.as_ref() {
            let _ = write!(text, "{}(pid={})", process.comm, process.pid);
        } else {
            text.push_str(locale.none());
        }
        text.push_str(" server=");
        if let Some(process) = tx.server_process.as_ref() {
            let _ = write!(text, "{}(pid={})", process.comm, process.pid);
        } else {
            text.push_str(locale.none());
        }
        text.push_str(" verdict=");
        text.push_str(http_transaction_verdict_label(&tx.verdict));
        text.push_str(" severity=");
        text.push_str(
            tx.severity
                .as_ref()
                .map(module_severity_label)
                .unwrap_or_else(|| locale.none()),
        );
        text.push_str(" degraded=");
        text.push_str(if tx.degraded { "true" } else { "false" });
        text.push_str(" suspect_sides=");
        if tx.suspect_sides.is_empty() {
            text.push_str(locale.none());
        } else {
            for (side_index, side) in tx.suspect_sides.iter().enumerate() {
                if side_index > 0 {
                    text.push(',');
                }
                text.push_str(http_suspect_side_label(side));
            }
        }
        text.push_str(" phases=");
        if tx.phases.is_empty() {
            text.push_str(locale.none());
        } else {
            push_joined_strings(&mut text, &tx.phases, ",");
        }
        text.push_str(" components=");
        if tx.components.is_empty() {
            text.push_str(locale.none());
        } else {
            for (component_index, component) in tx.components.iter().enumerate() {
                if component_index > 0 {
                    text.push(',');
                }
                text.push_str(http_component_kind_label(&component.kind));
                text.push(':');
                text.push_str(&operation_label(&component.operation));
            }
        }
        text.push_str(" summaries=");
        if tx.finding_summaries.is_empty() {
            push_joined_strings(&mut text, &tx.summaries, "|");
        } else {
            push_joined_strings(&mut text, &tx.finding_summaries, "|");
        }
    }
    text
}

pub(super) fn http_transactions_json(transactions: &[HttpTransactionView]) -> String {
    let mut json = String::from("[");
    for (index, transaction) in transactions.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        append_http_transaction_json(&mut json, transaction);
    }
    json.push(']');
    json
}

fn append_module_finding_json(json: &mut String, finding: &gewyvern::flow::ModuleFinding) {
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
        json.push_str(&flow.0.to_string());
    }
    json.push_str("],\"summaries\":");
    append_string_list_json(json, &finding.summaries);
    json.push_str(",\"evidence_trace\":");
    append_string_list_json(json, &finding.evidence_trace);
    json.push('}');
}

fn append_program_finding_json(json: &mut String, finding: &gewyvern::flow::ProgramFinding) {
    json.push_str("{\"program_flow\":");
    json.push_str(&finding.program_flow.0.to_string());
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

fn append_http_transaction_json(json: &mut String, transaction: &HttpTransactionView) {
    let suspect_sides = transaction
        .suspect_sides
        .iter()
        .map(|side| http_suspect_side_label(side).to_string())
        .collect::<Vec<_>>();
    json.push_str("{\"id\":");
    json.push_str(&transaction.id.0.to_string());
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
    append_string_list_json(json, &suspect_sides);
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

fn append_http_component_json(json: &mut String, component: &gewyvern::http::HttpComponentRef) {
    json.push_str("{\"template_id\":\"");
    json.push_str(&component.template_id);
    json.push_str("\",\"kind\":\"");
    json.push_str(http_component_kind_label(&component.kind));
    json.push_str("\",\"operation\":\"");
    append_operation_label(json, &component.operation);
    json.push_str("\"}");
}

fn append_analysis_context_json(
    json: &mut String,
    export: &ExportBundle,
    analysis: &AnalysisSnapshot,
) {
    append_ingest_context_json(json, export);
    json.push(',');
    append_analysis_spine_json(json, analysis);
}

fn append_ingest_context_json(json: &mut String, export: &ExportBundle) {
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

fn append_analysis_spine_json(json: &mut String, analysis: &AnalysisSnapshot) {
    json.push_str("\"ambiguous\":");
    json.push_str(if analysis.primary_process_profile_ambiguous {
        "true"
    } else {
        "false"
    });
    json.push_str(",\"competing_hypotheses\":");
    append_string_list_json(json, &analysis.competing_hypotheses);
    json.push_str(",\"primary_module_kind\":\"");
    json.push_str(&analysis.primary_module_kind);
    json.push_str("\",\"primary_module_family\":\"");
    json.push_str(module_family_label(&analysis.primary_module_kind));
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
    append_external_sidecar_context_field(json, analysis);
}

fn estimate_scan_target_json_capacity(
    name: &str,
    export: &ExportBundle,
    analysis: &AnalysisSnapshot,
) -> usize {
    256 + name.len()
        + export.template_id.len()
        + analysis.primary_module_kind.len()
        + analysis.primary_failure_stage.len()
        + analysis.primary_failure_mode.len()
        + analysis.primary_failure_detail.len()
        + analysis.primary_failure_confidence.len()
        + analysis.primary_failure_basis.len()
        + analysis
            .competing_hypotheses
            .iter()
            .map(String::len)
            .sum::<usize>()
        + analysis
            .suspect_modules
            .iter()
            .map(String::len)
            .sum::<usize>()
        + analysis.process_profiles.len() * 320
        + analysis.protocol_flows.len() * 220
        + analysis.augmentations.len() * 180
}

fn append_scan_target_json(
    json: &mut String,
    name: &str,
    export: &ExportBundle,
    analysis: &AnalysisSnapshot,
) {
    json.push_str("{\"target\":\"");
    json.push_str(name);
    json.push_str("\",\"status\":\"");
    json.push_str(analysis.target_status.label());
    json.push_str("\",");
    append_analysis_context_json(json, export, analysis);
    json.push_str(",\"suspect_modules\":");
    json.push_str(&suspect_modules_json_from_snapshot(analysis));
    json.push_str(",\"process_network_profiles\":");
    json.push('[');
    append_process_network_profiles_json_from_snapshot(json, analysis);
    json.push(']');
    json.push_str(",\"protocol_flows\":");
    json.push('[');
    append_protocol_flow_summaries_json_from_snapshot(json, analysis);
    json.push(']');
    json.push('}');
}

fn estimate_scan_target_text_capacity(
    name: &str,
    export: &ExportBundle,
    analysis: &AnalysisSnapshot,
) -> usize {
    96 + name.len()
        + export.program_flows.len() * 4
        + export.program_findings.len() * 4
        + export.module_findings.len() * 4
        + analysis.process_profiles.len() * 220
        + analysis.protocol_flows.len() * 160
}

fn append_scan_target_text(
    text: &mut String,
    name: &str,
    export: &ExportBundle,
    analysis: &AnalysisSnapshot,
) {
    text.push_str(name);
    text.push_str(" status=");
    text.push_str(analysis.target_status.label());
    text.push_str(" flows=");
    text.push_str(&export.program_flows.len().to_string());
    text.push_str(" findings=");
    text.push_str(&export.program_findings.len().to_string());
    text.push_str(" modules=");
    text.push_str(&export.module_findings.len().to_string());
    text.push_str(" profiles=");
    append_process_network_profiles_text_from_snapshot(text, analysis);
    text.push_str(" protocol_flows=");
    append_protocol_flow_summaries_text_from_snapshot(text, analysis);
}

fn estimate_scan_target_html_capacity(
    name: &str,
    export: &ExportBundle,
    analysis: &AnalysisSnapshot,
) -> usize {
    1400 + name.len()
        + export.ingest_trust_mode.len()
        + export.template_id.len()
        + analysis.primary_module_kind.len()
        + analysis.primary_failure_stage.len()
        + analysis.primary_failure_mode.len()
        + analysis.primary_failure_detail.len()
        + analysis.primary_failure_confidence.len()
        + analysis.primary_failure_basis.len()
        + analysis
            .competing_hypotheses
            .iter()
            .map(String::len)
            .sum::<usize>()
        + analysis
            .suspect_modules
            .iter()
            .map(String::len)
            .sum::<usize>()
        + analysis.process_profiles.len() * 360
        + analysis.protocol_flows.len() * 220
        + analysis.augmentations.len() * 140
}

fn append_scan_target_html_card(
    cards: &mut String,
    name: &str,
    export: &ExportBundle,
    analysis: &AnalysisSnapshot,
) {
    let status = analysis.target_status.label();
    let details_open = if matches!(analysis.target_status, ScanTargetStatus::Attention) {
        " open"
    } else {
        ""
    };
    let mut profiles = String::new();
    for profile in &analysis.process_profiles {
        let suspect_modules = first_or_none(&profile.suspect_modules);
        let module_kinds = if profile.module_kinds.is_empty() {
            String::new()
        } else {
            profile.module_kinds.join(" | ")
        };
        let phases = if profile.phases.is_empty() {
            String::new()
        } else {
            profile.phases.join(" > ")
        };
        let missing = if profile.missing_transitions.is_empty() {
            String::new()
        } else {
            profile.missing_transitions.join(" | ")
        };
        let _ = write!(
            profiles,
            "<li><strong>{}</strong> (pid={}): status={} <span class=\"tag family-{}\">{}</span> <span class=\"tag stage-{}\">{}</span> <span class=\"tag failure-{}\">{}</span> <span class=\"tag failure-{}\">{}</span> confidence={} basis={} suspect_module={} kinds={} healthy_flows={} attention_flows={} phases={} missing={}</li>",
            html_escape(&profile.comm),
            profile.pid,
            html_escape(&profile.status),
            html_escape(&profile.primary_module_family),
            html_escape(&profile.primary_module_kind),
            html_escape(&profile.primary_stage_family),
            html_escape(&profile.primary_failure_stage),
            html_escape(failure_mode_family_label(&profile.primary_failure_mode)),
            html_escape(&profile.primary_failure_mode),
            html_escape(failure_detail_family_label(&profile.primary_failure_detail)),
            html_escape(&profile.primary_failure_detail),
            html_escape(&profile.primary_failure_confidence),
            html_escape(&profile.primary_failure_basis),
            html_escape(&suspect_modules),
            html_escape(&module_kinds),
            profile.healthy_flows,
            profile.attention_flows,
            html_escape(&phases),
            html_escape(&missing),
        );
    }
    let pid_attribution_status = pid_attribution_status_for_export(export);
    let pid_attribution_note = pid_attribution_note_for_export(export);
    let ingest_mode_note = ingest_mode_note_for_export(export);
    let ambiguous = analysis.primary_process_profile_ambiguous;
    let competing_hypotheses = if analysis.competing_hypotheses.is_empty() {
        "none".into()
    } else {
        analysis.competing_hypotheses.join(" | ")
    };
    let suspect_modules = first_or_none(&analysis.suspect_modules);
    let primary_module_family = module_family_label(&analysis.primary_module_kind);
    let primary_stage_family = stage_family_label(&analysis.primary_failure_stage);
    let primary_failure_mode_family = failure_mode_family_label(&analysis.primary_failure_mode);
    let mut augmentations = String::new();
    if analysis.augmentations.is_empty() {
        augmentations.push_str("<li>none</li>");
    } else {
        for item in &analysis.augmentations {
            let producer = item.producer_pass.as_deref().unwrap_or("builtin");
            if matches!(
                item.name.as_str(),
                "external_evidence_chain_enrichment" | "external_diagnostic_opinion"
            ) {
                let handoff = item
                    .data_json
                    .as_deref()
                    .and_then(|data| extract_json_string_value(data, "external_handoff_readiness"))
                    .unwrap_or_else(|| "unknown".to_string());
                let merge_hint = item
                    .data_json
                    .as_deref()
                    .and_then(|data| extract_json_string_value(data, "external_merge_hint"))
                    .unwrap_or_else(|| "unknown".to_string());
                let _ = write!(
                    augmentations,
                    "<li><span class=\"tag family-{}\">{}</span> confidence={} producer={} handoff={} merge_hint={}</li>",
                    html_escape(&item.kind),
                    html_escape(&item.name),
                    html_escape(&item.confidence),
                    html_escape(producer),
                    html_escape(&handoff),
                    html_escape(&merge_hint),
                );
            } else {
                let _ = write!(
                    augmentations,
                    "<li><span class=\"tag family-{}\">{}</span> confidence={} producer={}</li>",
                    html_escape(&item.kind),
                    html_escape(&item.name),
                    html_escape(&item.confidence),
                    html_escape(producer),
                );
            }
        }
    }
    let mut flow_lines = String::new();
    for flow in &analysis.protocol_flows {
        let phase_text = if flow.phases.is_empty() {
            "none".to_string()
        } else {
            flow.phases.join(" > ")
        };
        let _ = write!(
            flow_lines,
            "<li>{}: last_phase={} <span class=\"tag failure-{}\">{}</span> <span class=\"tag failure-{}\">{}</span> confidence={} basis={} phases={}</li>",
            html_escape(&flow.operation),
            html_escape(flow.last_phase.as_deref().unwrap_or("none")),
            html_escape(failure_mode_family_label(&flow.failure_mode)),
            html_escape(&flow.failure_mode),
            html_escape(failure_detail_family_label(&flow.failure_detail)),
            html_escape(&flow.failure_detail),
            html_escape(&flow.failure_confidence),
            html_escape(&flow.failure_basis),
            html_escape(&phase_text),
        );
    }
    let _ = write!(
        cards,
        "<details class=\"card status-{status}\"{details_open}><summary><div class=\"card-title\"><h2>{}</h2><p><strong>status:</strong> {} | <strong>mode:</strong> {} | <strong>trust:</strong> {} | <strong>pid attribution:</strong> {} | <strong>ambiguous:</strong> {} | <strong>flows:</strong> {} | <strong>findings:</strong> {} | <strong>modules:</strong> {}</p></div><div class=\"conclusion\"><div class=\"pill\"><strong>primary module:</strong> <span class=\"tag family-{}\">{}</span></div><div class=\"pill\"><strong>primary stage:</strong> <span class=\"tag stage-{}\">{}</span></div><div class=\"pill\"><strong>failure mode:</strong> <span class=\"tag failure-{}\">{}</span></div><div class=\"pill\"><strong>failure detail:</strong> <span class=\"tag failure-{}\">{}</span></div><div class=\"pill\"><strong>confidence:</strong> {}</div><div class=\"pill\"><strong>basis:</strong> {}</div><div class=\"pill\"><strong>suspect modules:</strong> {}</div></div></summary><div class=\"card-body\"><p><strong>Mode note:</strong> {}</p><p><strong>PID attribution note:</strong> {}</p><p><strong>Competing hypotheses:</strong> {}</p><h3>Process Profiles</h3><ul>{}</ul><h3>Augmentations</h3><ul>{}</ul><h3>Protocol Flows</h3><ul>{}</ul></div></details>",
        html_escape(name),
        status,
        html_escape(ingest_mode_for_export(export)),
        html_escape(&export.ingest_trust_mode),
        html_escape(pid_attribution_status),
        ambiguous,
        export.program_flows.len(),
        export.program_findings.len(),
        export.module_findings.len(),
        primary_module_family,
        html_escape(&analysis.primary_module_kind),
        primary_stage_family,
        html_escape(&analysis.primary_failure_stage),
        primary_failure_mode_family,
        html_escape(&analysis.primary_failure_mode),
        failure_detail_family_label(&analysis.primary_failure_detail),
        html_escape(&analysis.primary_failure_detail),
        html_escape(&analysis.primary_failure_confidence),
        html_escape(&analysis.primary_failure_basis),
        html_escape(&suspect_modules),
        html_escape(ingest_mode_note),
        html_escape(pid_attribution_note),
        html_escape(&competing_hypotheses),
        profiles,
        augmentations,
        flow_lines,
    );
}

fn scan_target_status_counts(analyses: &[AnalysisSnapshot]) -> (usize, usize, usize) {
    let mut healthy = 0;
    let mut attention = 0;
    let mut idle = 0;
    for analysis in analyses {
        match analysis.target_status {
            ScanTargetStatus::Healthy => healthy += 1,
            ScanTargetStatus::Attention => attention += 1,
            ScanTargetStatus::Idle => idle += 1,
        }
    }
    (healthy, attention, idle)
}
