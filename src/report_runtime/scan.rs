use gewyvern::protocol_profiles::ProtocolSurfaceSummary;

use super::scan_surface::{
    append_protocol_surface_json, append_protocol_surface_text,
    estimate_protocol_surface_json_capacity, estimate_protocol_surface_text_capacity,
    protocol_surface_for_target, protocol_surface_html,
};
use super::sidecar::{external_sidecar_derived_state, external_sidecar_rollup_counts};
use super::*;

const SCAN_ALL_PROTOCOL_FLOW_DETAIL_LIMIT: usize = 32;

pub(super) fn scan_report_json(outputs: &[(String, ExportBundle)]) -> String {
    let analyses = collect_analyses(outputs);
    scan_report_json_with_analyses(outputs, &analyses)
}

pub(super) fn scan_report_json_with_analyses(
    outputs: &[(String, ExportBundle)],
    analyses: &[AnalysisSnapshot],
) -> String {
    let total_targets = outputs.len();
    let (healthy_targets, attention_targets, idle_targets) = scan_target_status_counts(analyses);
    let flow_limit = scan_report_flow_limit(outputs);
    let protocol_surfaces = outputs
        .iter()
        .map(|(name, _)| protocol_surface_for_target(name))
        .collect::<Vec<_>>();
    let estimated_capacity = 160
        + outputs
            .iter()
            .zip(analyses.iter())
            .zip(protocol_surfaces.iter())
            .map(|(((name, export), analysis), protocol_surface)| {
                estimate_scan_target_json_capacity(
                    name,
                    export,
                    analysis,
                    protocol_surface,
                    flow_limit,
                )
            })
            .sum::<usize>();
    let mut json = String::with_capacity(estimated_capacity);
    let _ = write!(
        json,
        "{{\"kind\":\"scan\",\"name\":null,\"target_count\":{},\"scan_all\":true,\"total_targets\":{},\"healthy_targets\":{},\"attention_targets\":{},\"idle_targets\":{},\"targets\":[",
        total_targets, total_targets, healthy_targets, attention_targets, idle_targets
    );
    for (index, (((name, export), analysis), protocol_surface)) in outputs
        .iter()
        .zip(analyses.iter())
        .zip(protocol_surfaces.iter())
        .enumerate()
    {
        if index > 0 {
            json.push(',');
        }
        append_scan_target_json(
            &mut json,
            name,
            export,
            analysis,
            protocol_surface.as_ref(),
            flow_limit,
        );
    }
    json.push_str("]}");
    json
}

pub(super) fn single_target_report_json_with_analysis(
    name: &str,
    export: &ExportBundle,
    analysis: &AnalysisSnapshot,
) -> String {
    let protocol_surface = protocol_surface_for_target(name);
    let estimated_capacity = 160
        + estimate_scan_target_json_capacity(name, export, analysis, &protocol_surface, usize::MAX);
    let mut json = String::with_capacity(estimated_capacity);
    let _ = write!(
        json,
        "{{\"kind\":\"scan\",\"name\":null,\"target_count\":1,\"scan_all\":true,\"total_targets\":1,\"healthy_targets\":{},\"attention_targets\":{},\"idle_targets\":{},\"targets\":[",
        matches!(analysis.target_status, ScanTargetStatus::Healthy) as usize,
        matches!(analysis.target_status, ScanTargetStatus::Attention) as usize,
        matches!(analysis.target_status, ScanTargetStatus::Idle) as usize,
    );
    append_scan_target_json(
        &mut json,
        name,
        export,
        analysis,
        protocol_surface.as_ref(),
        usize::MAX,
    );
    json.push_str("]}");
    json
}

pub(super) fn scan_report_html(outputs: &[(String, ExportBundle)]) -> String {
    let analyses = collect_analyses(outputs);
    scan_report_html_with_analyses(outputs, &analyses)
}

pub(super) fn scan_report_html_with_analyses(
    outputs: &[(String, ExportBundle)],
    analyses: &[AnalysisSnapshot],
) -> String {
    let total_targets = outputs.len();
    let (healthy_targets, attention_targets, idle_targets) = scan_target_status_counts(analyses);
    let protocol_surfaces = outputs
        .iter()
        .map(|(name, _)| protocol_surface_for_target(name))
        .collect::<Vec<_>>();
    let mut family_counts = std::collections::BTreeMap::<&str, usize>::new();
    for analysis in analyses {
        let family = module_family_label(&analysis.primary_module_kind);
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
    let (mergeable_sidecar_targets, automation_worthy_sidecar_targets, advisory_sidecar_targets) =
        external_sidecar_rollup_counts(analyses);
    if mergeable_sidecar_targets > 0 {
        let _ = write!(
            family_summary,
            "<div class=\"pill\"><strong>mergeable sidecar targets:</strong> {}</div>",
            mergeable_sidecar_targets
        );
    }
    if automation_worthy_sidecar_targets > 0 {
        let _ = write!(
            family_summary,
            "<div class=\"pill\"><strong>automation-worthy sidecar targets:</strong> {}</div>",
            automation_worthy_sidecar_targets
        );
    }
    if advisory_sidecar_targets > 0 {
        let _ = write!(
            family_summary,
            "<div class=\"pill\"><strong>advisory-only sidecar targets:</strong> {}</div>",
            advisory_sidecar_targets
        );
    }
    let mut sorted_outputs = outputs
        .iter()
        .zip(analyses.iter())
        .zip(protocol_surfaces.iter())
        .map(|(((name, export), analysis), protocol_surface)| {
            (name, export, analysis, protocol_surface)
        })
        .collect::<Vec<_>>();
    sorted_outputs.sort_by(
        |(left_name, _, left_analysis, _), (right_name, _, right_analysis, _)| {
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
    let flow_limit = scan_report_flow_limit(outputs);
    let mut cards = String::with_capacity(
        sorted_outputs
            .iter()
            .map(|(name, export, analysis, protocol_surface)| {
                estimate_scan_target_html_capacity(
                    name,
                    export,
                    analysis,
                    protocol_surface.as_ref(),
                    flow_limit,
                )
            })
            .sum::<usize>(),
    );
    for (index, (name, export, analysis, protocol_surface)) in
        sorted_outputs.into_iter().enumerate()
    {
        if index > 0 {
            cards.push('\n');
        }
        append_scan_target_html_card(
            &mut cards,
            name,
            export,
            analysis,
            protocol_surface.as_ref(),
            flow_limit,
        );
    }
    format!(
        "<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>gewyvern scan report</title><style>body{{font-family:ui-sans-serif,system-ui,sans-serif;background:#f6f7fb;color:#18202a;margin:0;padding:24px}}h1,h2,h3{{margin:0 0 12px}}.summary{{display:flex;gap:12px;flex-wrap:wrap;margin:16px 0 24px}}.summary-note{{margin:-10px 0 24px;color:#475569;font-size:14px}}.pill{{background:#fff;border:1px solid #d8dee9;border-radius:999px;padding:10px 14px;font-size:14px}}.tag{{display:inline-flex;align-items:center;border-radius:999px;padding:2px 10px;font-size:12px;font-weight:600}}.family-dns{{background:#dbeafe;color:#1d4ed8}}.family-route{{background:#e0f2fe;color:#0369a1}}.family-connect{{background:#ede9fe;color:#6d28d9}}.family-handshake{{background:#fae8ff;color:#a21caf}}.family-request-response{{background:#dcfce7;color:#166534}}.family-database{{background:#fef3c7;color:#92400e}}.family-auth{{background:#fee2e2;color:#b91c1c}}.family-directory{{background:#ecfccb;color:#3f6212}}.family-messaging{{background:#ffedd5;color:#c2410c}}.family-relay{{background:#d1fae5;color:#047857}}.family-service{{background:#e2e8f0;color:#334155}}.family-general{{background:#f3f4f6;color:#374151}}.stage-dns{{background:#dbeafe;color:#1d4ed8}}.stage-connect{{background:#ede9fe;color:#6d28d9}}.stage-handshake{{background:#fae8ff;color:#a21caf}}.stage-request-response{{background:#dcfce7;color:#166534}}.stage-auth{{background:#fee2e2;color:#b91c1c}}.stage-general{{background:#f3f4f6;color:#374151}}.stage-none{{background:#e5e7eb;color:#6b7280}}.failure-blocked{{background:#fef3c7;color:#92400e}}.failure-timeout{{background:#fee2e2;color:#b91c1c}}.failure-setup{{background:#e0e7ff;color:#4338ca}}.failure-semantic{{background:#ffedd5;color:#c2410c}}.failure-denied{{background:#fce7f3;color:#be185d}}.failure-peer{{background:#d1fae5;color:#047857}}.failure-none{{background:#e5e7eb;color:#6b7280}}.failure-general{{background:#f3f4f6;color:#374151}}.grid{{display:grid;grid-template-columns:repeat(auto-fit,minmax(320px,1fr));gap:16px}}.card{{background:#fff;border:1px solid #d8dee9;border-radius:16px;padding:0;box-shadow:0 6px 24px rgba(15,23,42,0.06);overflow:hidden}}.card summary{{list-style:none;cursor:pointer;padding:18px}}.card summary::-webkit-details-marker{{display:none}}.card-title p{{margin:0}}.card-body{{padding:0 18px 18px}}.conclusion{{display:flex;gap:10px;flex-wrap:wrap;margin:14px 0 0}}.status-attention{{border-color:#f0b429}}.status-healthy{{border-color:#68b984}}.status-idle{{border-color:#cbd5e1}}ul{{padding-left:18px}}li{{margin:6px 0}}</style></head><body><h1>gewyvern Scan Report</h1><div class=\"summary\"><div class=\"pill\">total targets: {}</div><div class=\"pill\">healthy: {}</div><div class=\"pill\">attention: {}</div><div class=\"pill\">idle: {}</div></div><p class=\"summary-note\">attention targets are shown first and expanded by default so the highest-risk paths are easier to inspect.</p><div class=\"summary\">{}</div><div class=\"grid\">{}</div></body></html>",
        total_targets, healthy_targets, attention_targets, idle_targets, family_summary, cards
    )
}

pub(super) fn single_target_report_html_with_analysis(
    name: &str,
    export: &ExportBundle,
    analysis: &AnalysisSnapshot,
) -> String {
    let protocol_surface = protocol_surface_for_target(name);
    let family = module_family_label(&analysis.primary_module_kind);
    let mut family_summary = String::new();
    let _ = write!(
        family_summary,
        "<div class=\"pill\"><span class=\"tag family-{}\">{}</span> 1</div>",
        family, family
    );
    let (mergeable_sidecar_targets, automation_worthy_sidecar_targets, advisory_sidecar_targets) =
        external_sidecar_rollup_counts(std::slice::from_ref(analysis));
    if mergeable_sidecar_targets > 0 {
        let _ = write!(
            family_summary,
            "<div class=\"pill\"><strong>mergeable sidecar targets:</strong> {}</div>",
            mergeable_sidecar_targets
        );
    }
    if automation_worthy_sidecar_targets > 0 {
        let _ = write!(
            family_summary,
            "<div class=\"pill\"><strong>automation-worthy sidecar targets:</strong> {}</div>",
            automation_worthy_sidecar_targets
        );
    }
    if advisory_sidecar_targets > 0 {
        let _ = write!(
            family_summary,
            "<div class=\"pill\"><strong>advisory-only sidecar targets:</strong> {}</div>",
            advisory_sidecar_targets
        );
    }
    let mut cards = String::with_capacity(estimate_scan_target_html_capacity(
        name,
        export,
        analysis,
        protocol_surface.as_ref(),
        usize::MAX,
    ));
    append_scan_target_html_card(
        &mut cards,
        name,
        export,
        analysis,
        protocol_surface.as_ref(),
        usize::MAX,
    );
    format!(
        "<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>gewyvern scan report</title><style>body{{font-family:ui-sans-serif,system-ui,sans-serif;background:#f6f7fb;color:#18202a;margin:0;padding:24px}}h1,h2,h3{{margin:0 0 12px}}.summary{{display:flex;gap:12px;flex-wrap:wrap;margin:16px 0 24px}}.summary-note{{margin:-10px 0 24px;color:#475569;font-size:14px}}.pill{{background:#fff;border:1px solid #d8dee9;border-radius:999px;padding:10px 14px;font-size:14px}}.tag{{display:inline-flex;align-items:center;border-radius:999px;padding:2px 10px;font-size:12px;font-weight:600}}.family-dns{{background:#dbeafe;color:#1d4ed8}}.family-route{{background:#e0f2fe;color:#0369a1}}.family-connect{{background:#ede9fe;color:#6d28d9}}.family-handshake{{background:#fae8ff;color:#a21caf}}.family-request-response{{background:#dcfce7;color:#166534}}.family-database{{background:#fef3c7;color:#92400e}}.family-auth{{background:#fee2e2;color:#b91c1c}}.family-directory{{background:#ecfccb;color:#3f6212}}.family-messaging{{background:#ffedd5;color:#c2410c}}.family-relay{{background:#d1fae5;color:#047857}}.family-service{{background:#e2e8f0;color:#334155}}.family-general{{background:#f3f4f6;color:#374151}}.stage-dns{{background:#dbeafe;color:#1d4ed8}}.stage-connect{{background:#ede9fe;color:#6d28d9}}.stage-handshake{{background:#fae8ff;color:#a21caf}}.stage-request-response{{background:#dcfce7;color:#166534}}.stage-auth{{background:#fee2e2;color:#b91c1c}}.stage-general{{background:#f3f4f6;color:#374151}}.stage-none{{background:#e5e7eb;color:#6b7280}}.failure-blocked{{background:#fef3c7;color:#92400e}}.failure-timeout{{background:#fee2e2;color:#b91c1c}}.failure-setup{{background:#e0e7ff;color:#4338ca}}.failure-semantic{{background:#ffedd5;color:#c2410c}}.failure-denied{{background:#fce7f3;color:#be185d}}.failure-peer{{background:#d1fae5;color:#047857}}.failure-none{{background:#e5e7eb;color:#6b7280}}.failure-general{{background:#f3f4f6;color:#374151}}.grid{{display:grid;grid-template-columns:repeat(auto-fit,minmax(320px,1fr));gap:16px}}.card{{background:#fff;border:1px solid #d8dee9;border-radius:16px;padding:0;box-shadow:0 6px 24px rgba(15,23,42,0.06);overflow:hidden}}.card summary{{list-style:none;cursor:pointer;padding:18px}}.card summary::-webkit-details-marker{{display:none}}.card-title p{{margin:0}}.card-body{{padding:0 18px 18px}}.conclusion{{display:flex;gap:10px;flex-wrap:wrap;margin:14px 0 0}}.status-attention{{border-color:#f0b429}}.status-healthy{{border-color:#68b984}}.status-idle{{border-color:#cbd5e1}}ul{{padding-left:18px}}li{{margin:6px 0}}</style></head><body><h1>gewyvern Scan Report</h1><div class=\"summary\"><div class=\"pill\">total targets: 1</div><div class=\"pill\">healthy: {}</div><div class=\"pill\">attention: {}</div><div class=\"pill\">idle: {}</div></div><p class=\"summary-note\">attention targets are shown first and expanded by default so the highest-risk paths are easier to inspect.</p><div class=\"summary\">{}</div><div class=\"grid\">{}</div></body></html>",
        matches!(analysis.target_status, ScanTargetStatus::Healthy) as usize,
        matches!(analysis.target_status, ScanTargetStatus::Attention) as usize,
        matches!(analysis.target_status, ScanTargetStatus::Idle) as usize,
        family_summary,
        cards
    )
}

pub(super) fn scan_report_text(outputs: &[(String, ExportBundle)]) -> String {
    let analyses = collect_analyses(outputs);
    scan_report_text_with_analyses(outputs, &analyses)
}

pub(super) fn scan_report_text_with_analyses(
    outputs: &[(String, ExportBundle)],
    analyses: &[AnalysisSnapshot],
) -> String {
    let total_targets = outputs.len();
    let (healthy_targets, attention_targets, idle_targets) = scan_target_status_counts(analyses);
    let flow_limit = scan_report_flow_limit(outputs);
    let protocol_surfaces = outputs
        .iter()
        .map(|(name, _)| protocol_surface_for_target(name))
        .collect::<Vec<_>>();
    let estimated_capacity = 96
        + outputs
            .iter()
            .zip(analyses.iter())
            .zip(protocol_surfaces.iter())
            .map(|(((name, export), analysis), protocol_surface)| {
                estimate_scan_target_text_capacity(
                    name,
                    export,
                    analysis,
                    protocol_surface.as_ref(),
                    flow_limit,
                )
            })
            .sum::<usize>();
    let mut text = String::with_capacity(estimated_capacity);
    let _ = write!(
        text,
        "scan_all_report: total_targets={} healthy_targets={} attention_targets={} idle_targets={}",
        total_targets, healthy_targets, attention_targets, idle_targets
    );
    for (((name, export), analysis), protocol_surface) in outputs
        .iter()
        .zip(analyses.iter())
        .zip(protocol_surfaces.iter())
    {
        text.push('\n');
        append_scan_target_text(
            &mut text,
            name,
            export,
            analysis,
            protocol_surface.as_ref(),
            flow_limit,
        );
    }
    text
}

fn scan_report_flow_limit(outputs: &[(String, ExportBundle)]) -> usize {
    if outputs.len() > 1 {
        SCAN_ALL_PROTOCOL_FLOW_DETAIL_LIMIT
    } else {
        usize::MAX
    }
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

pub(super) fn estimate_scan_target_json_capacity(
    name: &str,
    export: &ExportBundle,
    analysis: &AnalysisSnapshot,
    protocol_surface: &Option<ProtocolSurfaceSummary>,
    flow_limit: usize,
) -> usize {
    let flow_count = analysis.protocol_flows.len().min(flow_limit);
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
        + flow_count * 220
        + analysis.augmentations.len() * 180
        + estimate_protocol_surface_json_capacity(protocol_surface.as_ref())
}

pub(super) fn append_scan_target_json(
    json: &mut String,
    name: &str,
    export: &ExportBundle,
    analysis: &AnalysisSnapshot,
    protocol_surface: Option<&ProtocolSurfaceSummary>,
    flow_limit: usize,
) {
    let emitted_protocol_flows = analysis.protocol_flows.len().min(flow_limit);
    let omitted_protocol_flows = analysis.protocol_flows.len() - emitted_protocol_flows;
    json.push_str("{\"target\":");
    append_json_string(json, name);
    json.push_str(",\"status\":\"");
    json.push_str(analysis.target_status.label());
    json.push_str("\",");
    super::append_analysis_context_json(json, export, analysis);
    json.push_str(",\"suspect_modules\":");
    append_string_list_json(json, &analysis.suspect_modules);
    json.push_str(",\"process_network_profiles\":");
    json.push('[');
    append_process_network_profiles_json_from_snapshot(json, analysis);
    json.push(']');
    let _ = write!(
        json,
        ",\"protocol_flow_count\":{},\"protocol_flows_omitted\":{}",
        analysis.protocol_flows.len(),
        omitted_protocol_flows
    );
    json.push_str(",\"protocol_flows\":");
    json.push('[');
    append_protocol_flow_summaries_json_limited(json, analysis, emitted_protocol_flows);
    json.push(']');
    append_protocol_surface_json(json, protocol_surface);
    json.push('}');
}

pub(super) fn estimate_scan_target_text_capacity(
    name: &str,
    export: &ExportBundle,
    analysis: &AnalysisSnapshot,
    protocol_surface: Option<&ProtocolSurfaceSummary>,
    flow_limit: usize,
) -> usize {
    let flow_count = analysis.protocol_flows.len().min(flow_limit);
    192 + name.len()
        + export.program_flows.len() * 4
        + export.program_findings.len() * 4
        + export.module_findings.len() * 4
        + analysis.primary_module_family.len()
        + analysis.evidence_posture.len()
        + analysis.automation_outcome.len()
        + analysis.process_profiles.len() * 320
        + flow_count * 240
        + estimate_protocol_surface_text_capacity(protocol_surface)
}

pub(super) fn append_scan_target_text(
    text: &mut String,
    name: &str,
    export: &ExportBundle,
    analysis: &AnalysisSnapshot,
    protocol_surface: Option<&ProtocolSurfaceSummary>,
    flow_limit: usize,
) {
    use std::fmt::Write;

    let emitted_protocol_flows = analysis.protocol_flows.len().min(flow_limit);
    let omitted_protocol_flows = analysis.protocol_flows.len() - emitted_protocol_flows;
    text.push_str(name);
    text.push_str(" status=");
    text.push_str(analysis.target_status.label());
    text.push_str(" diagnosis_spine=");
    super::sidecar::append_diagnosis_spine_text(text, analysis);
    let _ = write!(
        text,
        " flows={} findings={} modules={}",
        export.program_flows.len(),
        export.program_findings.len(),
        export.module_findings.len()
    );
    text.push_str(" profiles=");
    append_process_network_profiles_text_from_snapshot(text, analysis);
    text.push_str(" protocol_flows=");
    append_protocol_flow_summaries_text_limited(text, analysis, emitted_protocol_flows);
    let _ = write!(
        text,
        " protocol_flow_count={} protocol_flows_omitted={}",
        analysis.protocol_flows.len(),
        omitted_protocol_flows
    );
    text.push(' ');
    append_protocol_surface_text(text, protocol_surface);
}

pub(super) fn estimate_scan_target_html_capacity(
    name: &str,
    export: &ExportBundle,
    analysis: &AnalysisSnapshot,
    protocol_surface: Option<&ProtocolSurfaceSummary>,
    flow_limit: usize,
) -> usize {
    let flow_count = analysis.protocol_flows.len().min(flow_limit);
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
        + flow_count * 220
        + analysis.augmentations.len() * 140
        + estimate_protocol_surface_json_capacity(protocol_surface)
}

pub(super) fn append_scan_target_html_card(
    cards: &mut String,
    name: &str,
    export: &ExportBundle,
    analysis: &AnalysisSnapshot,
    protocol_surface: Option<&ProtocolSurfaceSummary>,
    flow_limit: usize,
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
        let mut module_kinds = String::new();
        push_joined_strings(&mut module_kinds, &profile.module_kinds, " | ");
        let mut phases = String::new();
        push_joined_strings(&mut phases, &profile.phases, " > ");
        let mut missing = String::new();
        push_joined_strings(&mut missing, &profile.missing_transitions, " | ");
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
    let mut competing_hypotheses = String::new();
    if analysis.competing_hypotheses.is_empty() {
        competing_hypotheses.push_str("none");
    } else {
        push_joined_strings(
            &mut competing_hypotheses,
            &analysis.competing_hypotheses,
            " | ",
        );
    }
    let suspect_modules = first_or_none(&analysis.suspect_modules);
    let primary_module_family = module_family_label(&analysis.primary_module_kind);
    let primary_stage_family = stage_family_label(&analysis.primary_failure_stage);
    let primary_failure_mode_family = failure_mode_family_label(&analysis.primary_failure_mode);
    let sidecar_state = external_sidecar_derived_state(analysis);
    let sidecar_collaboration_note = sidecar_state.collaboration_note();
    let sidecar_guidance_support_note = sidecar_state.operator_guidance_support_note();
    let protocol_surface_section = protocol_surface_html(protocol_surface);
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
                let (handoff, merge_hint) = sidecar_state.item_hints(&item.name);
                let _ = write!(
                    augmentations,
                    "<li><span class=\"tag family-{}\">{}</span> confidence={} producer={} handoff={} merge_hint={}</li>",
                    html_escape(&item.kind),
                    html_escape(&item.name),
                    html_escape(&item.confidence),
                    html_escape(producer),
                    html_escape(handoff),
                    html_escape(merge_hint),
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
    let emitted_protocol_flows = analysis.protocol_flows.len().min(flow_limit);
    let omitted_protocol_flows = analysis.protocol_flows.len() - emitted_protocol_flows;
    for flow in analysis.protocol_flows.iter().take(emitted_protocol_flows) {
        let mut phase_text = String::new();
        if flow.phases.is_empty() {
            phase_text.push_str("none");
        } else {
            push_joined_strings(&mut phase_text, &flow.phases, " > ");
        }
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
    if omitted_protocol_flows > 0 {
        let _ = write!(
            flow_lines,
            "<li>{} additional protocol flow summaries omitted from this scan-all report; open the single-target report for full detail.</li>",
            omitted_protocol_flows
        );
    }
    let mut sidecar_collaboration_html = String::new();
    if let Some((state, note)) = sidecar_collaboration_note.as_ref() {
        let _ = write!(
            sidecar_collaboration_html,
            "<p><strong>External sidecar context:</strong> {} ({})</p>",
            html_escape(note),
            html_escape(state)
        );
    }
    let mut sidecar_guidance_support_html = String::new();
    if let Some((state, note)) = sidecar_guidance_support_note.as_ref() {
        let _ = write!(
            sidecar_guidance_support_html,
            "<p><strong>External operator-guidance support:</strong> {} ({})</p>",
            html_escape(note),
            html_escape(state)
        );
    }
    let _ = write!(
        cards,
        "<details class=\"card status-{status}\"{details_open}><summary><div class=\"card-title\"><h2>{}</h2><p><strong>status:</strong> {} | <strong>mode:</strong> {} | <strong>trust:</strong> {} | <strong>pid attribution:</strong> {} | <strong>ambiguous:</strong> {} | <strong>flows:</strong> {} | <strong>findings:</strong> {} | <strong>modules:</strong> {}</p></div><div class=\"conclusion\"><div class=\"pill\"><strong>primary module:</strong> <span class=\"tag family-{}\">{}</span></div><div class=\"pill\"><strong>primary stage:</strong> <span class=\"tag stage-{}\">{}</span></div><div class=\"pill\"><strong>failure mode:</strong> <span class=\"tag failure-{}\">{}</span></div><div class=\"pill\"><strong>failure detail:</strong> <span class=\"tag failure-{}\">{}</span></div><div class=\"pill\"><strong>confidence:</strong> {}</div><div class=\"pill\"><strong>basis:</strong> {}</div><div class=\"pill\"><strong>suspect modules:</strong> {}</div></div></summary><div class=\"card-body\"><p><strong>Mode note:</strong> {}</p><p><strong>PID attribution note:</strong> {}</p><p><strong>Competing hypotheses:</strong> {}</p>{}{}{}<h3>Process Profiles</h3><ul>{}</ul><h3>Augmentations</h3><ul>{}</ul><h3>Protocol Flows</h3><ul>{}</ul></div></details>",
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
        sidecar_collaboration_html,
        sidecar_guidance_support_html,
        protocol_surface_section,
        profiles,
        augmentations,
        flow_lines,
    );
}

pub(super) fn scan_target_status_counts(analyses: &[AnalysisSnapshot]) -> (usize, usize, usize) {
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
