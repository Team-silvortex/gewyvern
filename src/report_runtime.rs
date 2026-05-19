use gewyvern::export::ExportBundle;
use gewyvern::http::HttpTransactionView;

use super::*;
use crate::render_utils::*;

pub(super) fn summary_line(name: &str, export: &ExportBundle) -> String {
    let analysis = analysis_snapshot(export);
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
    let protocol_flows = protocol_flow_summaries_text_from_snapshot(&analysis);
    let process_profiles = process_network_profiles_text_from_snapshot(&analysis);
    let ingest_mode_note = ingest_mode_note_for_export(export);
    format!(
        "{name}: {}={} ingest_mode={} ingest_mode_note={} {}={} pid_attribution_status={} ambiguous={} competing_hypotheses={} {}={} {}={} {}={} {}={} {}={} {}={} {}={} {}={} {}={} {}={} {}={} protocol_flows={} process_network_profiles={}",
        locale.label("template"),
        export.template_id,
        ingest_mode_for_export(export),
        ingest_mode_note,
        "ingest_trust_mode",
        export.ingest_trust_mode,
        pid_attribution_status_for_export(export),
        analysis.primary_process_profile_ambiguous,
        string_list_json(&analysis.competing_hypotheses),
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
    let total_targets = outputs.len();
    let healthy_targets = analyses
        .iter()
        .filter(|analysis| matches!(analysis.target_status, ScanTargetStatus::Healthy))
        .count();
    let attention_targets = analyses
        .iter()
        .filter(|analysis| matches!(analysis.target_status, ScanTargetStatus::Attention))
        .count();
    let idle_targets = analyses
        .iter()
        .filter(|analysis| matches!(analysis.target_status, ScanTargetStatus::Idle))
        .count();

    let items = outputs
        .iter()
        .zip(analyses.iter())
        .map(|((name, export), analysis)| {
            let pid_attribution_status = pid_attribution_status_for_export(export);
            let pid_attribution_note = pid_attribution_note_for_export(export);
            let ingest_mode_note = ingest_mode_note_for_export(export);
            format!(
                "{{\"target\":\"{}\",\"status\":\"{}\",\"ingest_mode\":\"{}\",\"ingest_mode_note\":\"{}\",\"ingest_trust_mode\":\"{}\",\"pid_attribution_status\":\"{}\",\"pid_attribution_note\":\"{}\",\"ambiguous\":{},\"competing_hypotheses\":{},\"primary_module_kind\":\"{}\",\"primary_module_family\":\"{}\",\"primary_failure_stage\":\"{}\",\"primary_stage_family\":\"{}\",\"primary_failure_mode\":\"{}\",\"primary_failure_mode_family\":\"{}\",\"primary_failure_detail\":\"{}\",\"primary_failure_detail_family\":\"{}\",\"primary_failure_confidence\":\"{}\",\"primary_failure_basis\":\"{}\",\"suspect_modules\":{},\"process_network_profiles\":{},\"protocol_flows\":{}}}",
                name,
                analysis.target_status.label(),
                ingest_mode_for_export(export),
                ingest_mode_note,
                export.ingest_trust_mode,
                pid_attribution_status,
                pid_attribution_note,
                analysis.primary_process_profile_ambiguous,
                string_list_json(&analysis.competing_hypotheses),
                analysis.primary_module_kind,
                module_family_label(&analysis.primary_module_kind),
                analysis.primary_failure_stage,
                stage_family_label(&analysis.primary_failure_stage),
                analysis.primary_failure_mode,
                failure_mode_family_label(&analysis.primary_failure_mode),
                analysis.primary_failure_detail,
                failure_detail_family_label(&analysis.primary_failure_detail),
                analysis.primary_failure_confidence,
                analysis.primary_failure_basis,
                suspect_modules_for_export(export),
                process_network_profiles_json_from_snapshot(analysis),
                protocol_flow_summaries_json_from_snapshot(analysis),
            )
        })
        .collect::<Vec<_>>()
        .join(",");

    format!(
        "{{\"scan_all\":true,\"total_targets\":{},\"healthy_targets\":{},\"attention_targets\":{},\"idle_targets\":{},\"targets\":[{}]}}",
        total_targets, healthy_targets, attention_targets, idle_targets, items
    )
}

pub(super) fn scan_report_html(outputs: &[(String, ExportBundle)]) -> String {
    let total_targets = outputs.len();
    let healthy_targets = outputs
        .iter()
        .filter(|(_, export)| matches!(scan_target_status(export), ScanTargetStatus::Healthy))
        .count();
    let attention_targets = outputs
        .iter()
        .filter(|(_, export)| matches!(scan_target_status(export), ScanTargetStatus::Attention))
        .count();
    let idle_targets = outputs
        .iter()
        .filter(|(_, export)| matches!(scan_target_status(export), ScanTargetStatus::Idle))
        .count();

    let mut family_counts = std::collections::BTreeMap::<String, usize>::new();
    for (_, export) in outputs {
        let family = module_family_label(&primary_module_kind_for_export(export)).to_string();
        *family_counts.entry(family).or_default() += 1;
    }
    let family_summary = family_counts
        .into_iter()
        .map(|(family, count)| {
            format!(
                "<div class=\"pill\"><span class=\"tag family-{}\">{}</span> {}</div>",
                family, family, count
            )
        })
        .collect::<Vec<_>>()
        .join("");

    let mut sorted_outputs = outputs
        .iter()
        .map(|(name, export)| (name, export))
        .collect::<Vec<_>>();
    sorted_outputs.sort_by(|(left_name, left), (right_name, right)| {
        let left_rank = match scan_target_status(left) {
            ScanTargetStatus::Attention => 0,
            ScanTargetStatus::Healthy => 1,
            ScanTargetStatus::Idle => 2,
        };
        let right_rank = match scan_target_status(right) {
            ScanTargetStatus::Attention => 0,
            ScanTargetStatus::Healthy => 1,
            ScanTargetStatus::Idle => 2,
        };
        left_rank
            .cmp(&right_rank)
            .then_with(|| {
                primary_module_kind_for_export(left).cmp(&primary_module_kind_for_export(right))
            })
            .then_with(|| left_name.cmp(right_name))
    });

    let cards = sorted_outputs
        .into_iter()
        .map(|(name, export)| {
            let status = scan_target_status(export).label();
            let details_open = if matches!(scan_target_status(export), ScanTargetStatus::Attention)
            {
                " open"
            } else {
                ""
            };
            let profiles = process_network_profile_summaries(export)
                .into_iter()
                .map(|profile| {
                    let suspect_modules = first_or_none(&profile.suspect_modules);
                    format!(
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
                        html_escape(&profile.module_kinds.join(" | ")),
                        profile.healthy_flows,
                        profile.attention_flows,
                        html_escape(&profile.phases.join(" > ")),
                        html_escape(&profile.missing_transitions.join(" | ")),
                    )
                })
                .collect::<Vec<_>>()
                .join("");
            let primary_module_kind = primary_module_kind_for_export(export);
            let primary_failure_stage = primary_failure_stage_for_export(export);
            let primary_failure_mode = primary_failure_mode_for_export(export);
            let primary_failure_detail = primary_failure_detail_for_export(export);
            let primary_failure_confidence = primary_failure_confidence_for_export(export);
            let primary_failure_basis = primary_failure_basis_for_export(export);
            let pid_attribution_status = pid_attribution_status_for_export(export);
            let pid_attribution_note = pid_attribution_note_for_export(export);
            let ingest_mode_note = ingest_mode_note_for_export(export);
            let ambiguous = primary_process_profile_ambiguous_for_export(export);
            let competing_hypotheses = primary_process_profile_for_export(export)
                .map(|profile| profile.competing_hypotheses.join(" | "))
                .unwrap_or_else(|| "none".into());
            let suspect_modules = suspect_modules_for_export(export);
            let primary_module_family = module_family_label(&primary_module_kind);
            let primary_stage_family = stage_family_label(&primary_failure_stage);
            let primary_failure_mode_family = failure_mode_family_label(&primary_failure_mode);
            let flow_finding_summaries = protocol_flow_finding_summaries(export);
            let flow_lines = export
                .program_flows
                .iter()
                .map(|flow| {
                    let phase_text = protocol_flow_phases(flow).join(" > ");
                    let failure_mode =
                        protocol_flow_failure_mode(flow, flow_finding_summaries.get(&flow.id));
                    let failure_detail =
                        protocol_flow_failure_detail(flow, flow_finding_summaries.get(&flow.id));
                    let failure_confidence = protocol_flow_failure_confidence(
                        flow,
                        flow_finding_summaries.get(&flow.id),
                    );
                    let failure_basis =
                        protocol_flow_failure_basis(flow, flow_finding_summaries.get(&flow.id));
                    format!(
                        "<li>{}: last_phase={} <span class=\"tag failure-{}\">{}</span> <span class=\"tag failure-{}\">{}</span> confidence={} basis={} phases={}</li>",
                        html_escape(&operation_label(&flow.operation)),
                        html_escape(&protocol_flow_last_phase(flow).unwrap_or_else(|| "none".into())),
                        html_escape(failure_mode_family_label(&failure_mode)),
                        html_escape(&failure_mode),
                        html_escape(failure_detail_family_label(&failure_detail)),
                        html_escape(&failure_detail),
                        html_escape(&failure_confidence),
                        html_escape(&failure_basis),
                        html_escape(&phase_text),
                    )
                })
                .collect::<Vec<_>>()
                .join("");
            format!(
                "<details class=\"card status-{status}\"{details_open}><summary><div class=\"card-title\"><h2>{}</h2><p><strong>status:</strong> {} | <strong>mode:</strong> {} | <strong>trust:</strong> {} | <strong>pid attribution:</strong> {} | <strong>ambiguous:</strong> {} | <strong>flows:</strong> {} | <strong>findings:</strong> {} | <strong>modules:</strong> {}</p></div><div class=\"conclusion\"><div class=\"pill\"><strong>primary module:</strong> <span class=\"tag family-{}\">{}</span></div><div class=\"pill\"><strong>primary stage:</strong> <span class=\"tag stage-{}\">{}</span></div><div class=\"pill\"><strong>failure mode:</strong> <span class=\"tag failure-{}\">{}</span></div><div class=\"pill\"><strong>failure detail:</strong> <span class=\"tag failure-{}\">{}</span></div><div class=\"pill\"><strong>confidence:</strong> {}</div><div class=\"pill\"><strong>basis:</strong> {}</div><div class=\"pill\"><strong>suspect modules:</strong> {}</div></div></summary><div class=\"card-body\"><p><strong>Mode note:</strong> {}</p><p><strong>PID attribution note:</strong> {}</p><p><strong>Competing hypotheses:</strong> {}</p><h3>Process Profiles</h3><ul>{}</ul><h3>Protocol Flows</h3><ul>{}</ul></div></details>",
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
                html_escape(&primary_module_kind),
                primary_stage_family,
                html_escape(&primary_failure_stage),
                primary_failure_mode_family,
                html_escape(&primary_failure_mode),
                failure_detail_family_label(&primary_failure_detail),
                html_escape(&primary_failure_detail),
                html_escape(&primary_failure_confidence),
                html_escape(&primary_failure_basis),
                html_escape(&suspect_modules),
                html_escape(ingest_mode_note),
                html_escape(pid_attribution_note),
                html_escape(&competing_hypotheses),
                profiles,
                flow_lines,
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

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
    let total_targets = outputs.len();
    let healthy_targets = analyses
        .iter()
        .filter(|analysis| matches!(analysis.target_status, ScanTargetStatus::Healthy))
        .count();
    let attention_targets = analyses
        .iter()
        .filter(|analysis| matches!(analysis.target_status, ScanTargetStatus::Attention))
        .count();
    let idle_targets = analyses
        .iter()
        .filter(|analysis| matches!(analysis.target_status, ScanTargetStatus::Idle))
        .count();
    let mut lines = vec![format!(
        "scan_all_report: total_targets={} healthy_targets={} attention_targets={} idle_targets={}",
        total_targets, healthy_targets, attention_targets, idle_targets
    )];
    lines.extend(
        outputs
            .iter()
            .zip(analyses.iter())
            .map(|((name, export), analysis)| {
                format!(
                    "{} status={} flows={} findings={} modules={} profiles={} protocol_flows={}",
                    name,
                    analysis.target_status.label(),
                    export.program_flows.len(),
                    export.program_findings.len(),
                    export.module_findings.len(),
                    process_network_profiles_text_from_snapshot(analysis),
                    protocol_flow_summaries_text_from_snapshot(analysis),
                )
            }),
    );
    lines.join("\n")
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
    let pid_attribution_status = pid_attribution_status_for_export(export);
    let pid_attribution_note = pid_attribution_note_for_export(export);
    let ingest_mode_note = ingest_mode_note_for_export(export);
    let suspect_modules = format!(
        "[{}]",
        export
            .program_findings
            .iter()
            .map(|finding| format!("\"{}\"", finding.module_label))
            .collect::<Vec<_>>()
            .join(",")
    );
    format!(
        "{{\"demo\":\"{name}\",\"template_id\":\"{}\",\"ingest_mode\":\"{}\",\"ingest_mode_note\":\"{}\",\"ingest_trust_mode\":\"{}\",\"pid_attribution_status\":\"{}\",\"pid_attribution_note\":\"{}\",\"ambiguous\":{},\"competing_hypotheses\":{},\"primary_module_kind\":\"{}\",\"primary_module_family\":\"{}\",\"primary_failure_stage\":\"{}\",\"primary_stage_family\":\"{}\",\"primary_failure_mode\":\"{}\",\"primary_failure_mode_family\":\"{}\",\"primary_failure_detail\":\"{}\",\"primary_failure_detail_family\":\"{}\",\"primary_failure_confidence\":\"{}\",\"primary_failure_basis\":\"{}\",\"fragments_loaded\":{},\"hookpoints_failed\":{},\"accepted_facts\":{},\"rejected_facts\":{},\"flows\":{},\"program_findings\":{},\"module_findings\":{},\"reasons\":{},\"degraded\":{},\"suspect_modules\":{},\"protocol_flows\":{},\"process_network_profiles\":{}}}",
        export.template_id,
        ingest_mode_for_export(export),
        ingest_mode_note,
        export.ingest_trust_mode,
        pid_attribution_status,
        pid_attribution_note,
        analysis.primary_process_profile_ambiguous,
        string_list_json(&analysis.competing_hypotheses),
        analysis.primary_module_kind,
        module_family_label(&analysis.primary_module_kind),
        analysis.primary_failure_stage,
        stage_family_label(&analysis.primary_failure_stage),
        analysis.primary_failure_mode,
        failure_mode_family_label(&analysis.primary_failure_mode),
        analysis.primary_failure_detail,
        failure_detail_family_label(&analysis.primary_failure_detail),
        analysis.primary_failure_confidence,
        analysis.primary_failure_basis,
        export.debug_summary.fragments_loaded,
        export.debug_summary.hookpoints_failed,
        export.debug_summary.accepted_facts,
        export.debug_summary.rejected_facts,
        export.debug_summary.flows,
        export.debug_summary.program_findings,
        export.debug_summary.module_findings,
        export.debug_summary.reasons,
        export.debug_summary.degraded,
        suspect_modules,
        protocol_flow_summaries_json_from_snapshot(&analysis),
        process_network_profiles_json_from_snapshot(&analysis),
    )
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
    let pid_attribution_status = pid_attribution_status_for_export(export);
    let pid_attribution_note = pid_attribution_note_for_export(export);
    let ingest_mode_note = ingest_mode_note_for_export(export);
    format!(
        "{{\"demo\":\"{name}\",\"template_id\":\"{}\",\"ingest_mode\":\"{}\",\"ingest_mode_note\":\"{}\",\"ingest_trust_mode\":\"{}\",\"pid_attribution_status\":\"{}\",\"pid_attribution_note\":\"{}\",\"ambiguous\":{},\"competing_hypotheses\":{},\"primary_module_kind\":\"{}\",\"primary_module_family\":\"{}\",\"primary_failure_stage\":\"{}\",\"primary_stage_family\":\"{}\",\"primary_failure_mode\":\"{}\",\"primary_failure_mode_family\":\"{}\",\"primary_failure_detail\":\"{}\",\"primary_failure_detail_family\":\"{}\",\"primary_failure_confidence\":\"{}\",\"primary_failure_basis\":\"{}\",\"module_findings\":[{}],\"program_findings\":[{}],\"process_network_profiles\":{}}}",
        export.template_id,
        ingest_mode_for_export(export),
        ingest_mode_note,
        export.ingest_trust_mode,
        pid_attribution_status,
        pid_attribution_note,
        analysis.primary_process_profile_ambiguous,
        string_list_json(&analysis.competing_hypotheses),
        analysis.primary_module_kind,
        module_family_label(&analysis.primary_module_kind),
        analysis.primary_failure_stage,
        stage_family_label(&analysis.primary_failure_stage),
        analysis.primary_failure_mode,
        failure_mode_family_label(&analysis.primary_failure_mode),
        analysis.primary_failure_detail,
        failure_detail_family_label(&analysis.primary_failure_detail),
        analysis.primary_failure_confidence,
        analysis.primary_failure_basis,
        export
            .module_findings
            .iter()
            .map(module_finding_json)
            .collect::<Vec<_>>()
            .join(","),
        export
            .program_findings
            .iter()
            .map(program_finding_json)
            .collect::<Vec<_>>()
            .join(","),
        process_network_profiles_json_from_snapshot(&analysis),
    )
}

pub(super) fn http_transactions_text(transactions: &[HttpTransactionView]) -> String {
    let locale = UiLocale::detect();
    if transactions.is_empty() {
        return locale.none().into();
    }

    transactions
        .iter()
        .map(|tx| {
            format!(
                "http_transaction#{}: client={} server={} verdict={} severity={} degraded={} suspect_sides={} phases={} components={} summaries={}",
                tx.id.0,
                tx.client_process
                    .as_ref()
                    .map(|p| format!("{}(pid={})", p.comm, p.pid))
                    .unwrap_or_else(|| locale.none().to_string()),
                tx.server_process
                    .as_ref()
                    .map(|p| format!("{}(pid={})", p.comm, p.pid))
                    .unwrap_or_else(|| locale.none().to_string()),
                http_transaction_verdict_label(&tx.verdict),
                tx.severity
                    .as_ref()
                    .map(module_severity_label)
                    .unwrap_or_else(|| locale.none()),
                tx.degraded,
                if tx.suspect_sides.is_empty() {
                    locale.none().to_string()
                } else {
                    tx.suspect_sides
                        .iter()
                        .map(http_suspect_side_label)
                        .collect::<Vec<_>>()
                        .join(",")
                },
                if tx.phases.is_empty() {
                    locale.none().to_string()
                } else {
                    tx.phases.join(",")
                },
                tx.components
                    .iter()
                    .map(|component| format!("{}:{}", http_component_kind_label(&component.kind), operation_label(&component.operation)))
                    .collect::<Vec<_>>()
                    .join(","),
                if tx.finding_summaries.is_empty() {
                    tx.summaries.join("|")
                } else {
                    tx.finding_summaries.join("|")
                }
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn http_transactions_json(transactions: &[HttpTransactionView]) -> String {
    format!(
        "[{}]",
        transactions
            .iter()
            .map(http_transaction_json)
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn http_transaction_json(transaction: &HttpTransactionView) -> String {
    format!(
        "{{\"id\":{},\"client_process\":{},\"server_process\":{},\"verdict\":\"{}\",\"severity\":{},\"degraded\":{},\"suspect_sides\":{},\"phases\":{},\"components\":{},\"finding_summaries\":{},\"summaries\":{}}}",
        transaction.id.0,
        process_json(transaction.client_process.as_ref()),
        process_json(transaction.server_process.as_ref()),
        http_transaction_verdict_label(&transaction.verdict),
        transaction
            .severity
            .as_ref()
            .map(|severity| format!("\"{}\"", module_severity_label(severity)))
            .unwrap_or_else(|| "null".into()),
        transaction.degraded,
        string_list_json(
            &transaction
                .suspect_sides
                .iter()
                .map(|side| http_suspect_side_label(side).to_string())
                .collect::<Vec<_>>()
        ),
        string_list_json(&transaction.phases),
        format!(
            "[{}]",
            transaction
                .components
                .iter()
                .map(http_component_json)
                .collect::<Vec<_>>()
                .join(",")
        ),
        string_list_json(&transaction.finding_summaries),
        string_list_json(&transaction.summaries),
    )
}

fn http_component_json(component: &gewyvern::http::HttpComponentRef) -> String {
    format!(
        "{{\"template_id\":\"{}\",\"kind\":\"{}\",\"operation\":\"{}\"}}",
        component.template_id,
        http_component_kind_label(&component.kind),
        operation_label(&component.operation),
    )
}

fn module_finding_json(finding: &gewyvern::flow::ModuleFinding) -> String {
    format!(
        "{{\"module_label\":\"{}\",\"severity\":\"{}\",\"process\":{},\"operation\":\"{}\",\"network_module_kinds\":{},\"phases\":{},\"phase_transitions\":{},\"suspect_areas\":{},\"causes\":{},\"supporting_fragments\":{},\"program_flows\":{},\"summaries\":{},\"evidence_trace\":{}}}",
        finding.module_label,
        module_severity_label(&finding.severity),
        process_json(finding.process.as_ref()),
        operation_label(&finding.operation),
        string_list_json(&finding.network_module_kinds),
        string_list_json(&finding.phases),
        string_list_json(&finding.phase_transitions),
        string_list_json(&finding.suspect_areas),
        string_list_json(
            &finding
                .causes
                .iter()
                .map(finding_cause_label)
                .map(str::to_string)
                .collect::<Vec<_>>()
        ),
        string_list_json(&finding.supporting_fragments),
        format!(
            "[{}]",
            finding
                .program_flows
                .iter()
                .map(|flow| flow.0.to_string())
                .collect::<Vec<_>>()
                .join(",")
        ),
        string_list_json(&finding.summaries),
        string_list_json(&finding.evidence_trace),
    )
}

fn program_finding_json(finding: &gewyvern::flow::ProgramFinding) -> String {
    format!(
        "{{\"program_flow\":{},\"module_label\":\"{}\",\"network_module_kind\":\"{}\",\"phase\":{},\"phase_transition\":{},\"suspect_area\":\"{}\",\"cause\":\"{}\",\"process\":{},\"operation\":\"{}\",\"summary\":\"{}\",\"supporting_fragments\":{},\"evidence_trace\":{}}}",
        finding.program_flow.0,
        finding.module_label,
        finding.network_module_kind,
        finding
            .phase
            .as_ref()
            .map_or("null".to_string(), |phase| format!("\"{}\"", phase)),
        finding
            .phase_transition
            .as_ref()
            .map_or("null".to_string(), |transition| format!(
                "\"{}\"",
                transition
            )),
        finding.suspect_area,
        finding_cause_label(&finding.cause),
        process_json(finding.process.as_ref()),
        operation_label(&finding.operation),
        finding.summary,
        string_list_json(&finding.supporting_fragments),
        string_list_json(&finding.evidence_trace),
    )
}
