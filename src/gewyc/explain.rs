use super::explain_support::*;
use super::*;

pub(super) fn explain_text(report: &ExplainReport, focus: Option<ExplainFocus>) -> String {
    let next_step = explain_next_step_hint(report);
    let mut lines = vec![
        "surface=explain".to_string(),
        format!("ok={}", report.ok),
        format!("parse_ok={}", report.stages.parse.ok),
        format!("validation_ok={}", report.stages.validation.ok),
        format!("diagnostics_ok={}", report.stages.diagnostics.ok),
        format!("next_step={next_step}"),
    ];
    if let Some(focus) = focus {
        lines.push(format!("focus={}", explain_focus_text(focus)));
    }

    if let Some(focus) = focus {
        lines.extend(explain_focus_text_lines(report, focus));
        return lines.join("\n");
    }

    if let Some(excerpt) = &report.parse_source_excerpt {
        lines.push(format!("parse_source_excerpt={}", excerpt.line_text));
        lines.push(format!("parse_source_marker={}", excerpt.marker));
    }

    match &report.binding {
        Some(binding) => {
            lines.push(format!("template={}", binding.template_id));
            if let Some(model) = &binding.program_model {
                lines.push(format!("operation={}", model.operation));
                lines.push(format!("program_rules={}", model.rules));
            } else {
                lines.push("operation=none".into());
            }
            lines.push(format!("fragments={}", binding.fragments.join(",")));
            if let Some(summary) = &report.lowered_binding_summary {
                lines.push(format!(
                    "lowered_binding_summary=fragments:{} window:{} reason:{} program_model:{} program_rules:{} params:{} evidence:{}",
                    summary.fragment_count,
                    summary.has_window,
                    summary.has_reason_profile,
                    summary.has_program_model,
                    summary.program_rule_count,
                    summary.fragment_param_count,
                    summary.evidence_override_count
                ));
            }
            if let Some(delta) = &report.frontend_lowering_delta {
                lines.push(format!(
                    "frontend_lowering_delta=functions:{} merged_steps:{} use_edges:{} includes:{} => fragments:{} program_rules:{} params:{} evidence:{}",
                    delta.frontend_function_count,
                    delta.frontend_merged_step_count,
                    delta.frontend_use_edge_count,
                    delta.frontend_include_source_count,
                    delta.lowered_fragment_count,
                    delta.lowered_program_rule_count,
                    delta.lowered_fragment_param_count,
                    delta.lowered_evidence_override_count
                ));
            }
            if let Some(note) = &report.binding_shape_note {
                lines.push(format!("binding_shape_note={note}"));
            }
        }
        None => {
            lines.push("template=none".into());
            lines.push("operation=none".into());
            lines.push("fragments=none".into());
            lines.push("lowered_binding_summary=none".into());
            lines.push("frontend_lowering_delta=none".into());
            lines.push("binding_shape_note=none".into());
        }
    }

    if let Some(frontend) = &report.frontend {
        lines.push("frontend:".into());
        lines.push(format!("- kind={}", frontend.kind));
        lines.push(format!("- function_count={}", frontend.function_count));
        lines.push(format!(
            "- merged_step_count={}",
            frontend.merged_step_count
        ));
        lines.push(format!(
            "- include_sources={}",
            frontend.include_sources.len()
        ));
        lines.push(format!("- use_edges={}", frontend.use_edges.len()));
        lines.push(format!("- graph_nodes={}", frontend.graph_nodes.len()));
        lines.push(format!("- graph_edges={}", frontend.graph_edges.len()));
    } else {
        lines.push("frontend=none".into());
    }

    lines.push("validation:".into());
    lines.push(format!("- registry={}", report.stages.validation.registry));
    lines.push(format!(
        "- fragments={}",
        report.stages.validation.fragment_count
    ));
    lines.push(format!(
        "- program_rules={}",
        report.stages.validation.program_rule_count
    ));
    lines.push(format!(
        "- reason_rules={}",
        report.stages.validation.reason_rule_count
    ));
    lines.push(format!(
        "- unsupported_payload_offsets={:?}",
        report.stages.validation.unsupported_payload_offsets
    ));
    if let Some(excerpt) = &report.validation_excerpt {
        lines.push(format!(
            "- validation_excerpt=model:{} rule:{} offsets:{:?} supporting_fragments:{}",
            excerpt.model,
            excerpt.rule_index,
            excerpt.unsupported_payload_offsets,
            excerpt.supporting_fragments.join(",")
        ));
    }
    if let Some(note) = &report.validation_shape_note {
        lines.push(format!("- validation_note={note}"));
    }

    match &report.diagnostics {
        Some(diagnostics) => {
            lines.push("diagnostics:".into());
            lines.push(format!("- template={}", diagnostics.template_id));
            lines.push(format!(
                "- program_model_rules={}",
                diagnostics
                    .program_model
                    .as_ref()
                    .map(|model| model.rules.len())
                    .unwrap_or(0)
            ));
            lines.push(format!(
                "- reason_model_rules={}",
                diagnostics
                    .reason_model
                    .as_ref()
                    .map(|model| model.rules.len())
                    .unwrap_or(0)
            ));
            if let Some(excerpt) = &report.diagnostics_excerpt {
                lines.push(format!(
                    "- diagnostics_excerpt=model:{} rule:{} missing_facts:{} offsets:{:?} supporting_fragments:{}",
                    excerpt.model,
                    excerpt.rule_index,
                    excerpt.missing_facts.join(","),
                    excerpt.unsupported_payload_offsets,
                    excerpt.supporting_fragments.join(",")
                ));
            }
            if let Some(note) = &report.diagnostics_shape_note {
                lines.push(format!("- diagnostics_note={note}"));
            }
        }
        None => lines.push("diagnostics=none".into()),
    }

    if report.findings.findings.is_empty() {
        lines.push("findings=none".into());
    } else {
        lines.push("findings:".into());
        lines.extend(
            report
                .findings
                .findings
                .iter()
                .map(|finding| format!("- {}", finding_text_record(finding))),
        );
    }

    lines.join("\n")
}

pub(super) fn explain_text_compact(report: &ExplainReport, focus: Option<ExplainFocus>) -> String {
    let mut lines = vec![format!(
        "surface=explain ok={} parse_ok={} validation_ok={} diagnostics_ok={} findings={} next_step={}",
        report.ok,
        report.stages.parse.ok,
        report.stages.validation.ok,
        report.stages.diagnostics.ok,
        report.findings.findings.len(),
        explain_next_step_hint(report),
    )];
    if let Some(focus) = focus {
        lines.push(format!("focus={}", explain_focus_text(focus)));
        match focus {
            ExplainFocus::Parse => {
                lines.push(format!(
                    "parse_finding={}",
                    finding_text(report.stages.parse.finding.as_ref())
                ));
            }
            ExplainFocus::Frontend => {
                if let Some(frontend) = &report.frontend {
                    lines.push(format!(
                        "frontend kind={} functions={} includes={} use_edges={} graph_nodes={} graph_edges={}",
                        frontend.kind,
                        frontend.function_count,
                        frontend.include_sources.len(),
                        frontend.use_edges.len(),
                        frontend.graph_nodes.len(),
                        frontend.graph_edges.len()
                    ));
                } else {
                    lines.push("frontend=none".into());
                }
            }
            ExplainFocus::Binding => {
                if let Some(summary) = &report.lowered_binding_summary {
                    lines.push(format!(
                        "binding fragments={} window={} reason={} program_model={} rules={} params={} evidence={}",
                        summary.fragment_count,
                        summary.has_window,
                        summary.has_reason_profile,
                        summary.has_program_model,
                        summary.program_rule_count,
                        summary.fragment_param_count,
                        summary.evidence_override_count
                    ));
                } else {
                    lines.push("binding=none".into());
                }
                if let Some(delta) = &report.frontend_lowering_delta {
                    lines.push(format!(
                        "binding_delta frontend_functions={} frontend_steps={} frontend_use_edges={} frontend_includes={} lowered_fragments={} lowered_rules={} lowered_params={} lowered_evidence={}",
                        delta.frontend_function_count,
                        delta.frontend_merged_step_count,
                        delta.frontend_use_edge_count,
                        delta.frontend_include_source_count,
                        delta.lowered_fragment_count,
                        delta.lowered_program_rule_count,
                        delta.lowered_fragment_param_count,
                        delta.lowered_evidence_override_count
                    ));
                }
                if let Some(note) = &report.binding_shape_note {
                    lines.push(format!("binding_note={note}"));
                }
            }
            ExplainFocus::Validation => {
                lines.push(format!(
                    "validation registry={} unsupported_payload_offsets={:?}",
                    report.stages.validation.registry,
                    report.stages.validation.unsupported_payload_offsets
                ));
                if let Some(note) = &report.validation_shape_note {
                    lines.push(format!("validation_note={note}"));
                }
            }
            ExplainFocus::Diagnostics => {
                lines.push(format!(
                    "diagnostics excerpt={}",
                    report
                        .diagnostics_excerpt
                        .as_ref()
                        .map(|excerpt| format!(
                            "{}#{} missing={} offsets={:?}",
                            excerpt.model,
                            excerpt.rule_index,
                            excerpt.missing_facts.join(","),
                            excerpt.unsupported_payload_offsets
                        ))
                        .unwrap_or_else(|| "none".into())
                ));
                if let Some(note) = &report.diagnostics_shape_note {
                    lines.push(format!("diagnostics_note={note}"));
                }
            }
            ExplainFocus::Findings => {
                lines.push(format!("findings={}", report.findings.findings.len()));
            }
        }
        return lines.join("\n");
    }

    lines.push(format!(
        "template={} operation={} fragments={}",
        report
            .binding
            .as_ref()
            .map(|binding| binding.template_id.as_str())
            .unwrap_or("none"),
        report
            .binding
            .as_ref()
            .and_then(|binding| binding.program_model.as_ref())
            .map(|model| model.operation.as_str())
            .unwrap_or("none"),
        report
            .binding
            .as_ref()
            .map(|binding| binding.fragments.len().to_string())
            .unwrap_or_else(|| "0".into())
    ));
    if let Some(summary) = &report.lowered_binding_summary {
        lines.push(format!(
            "lowered=window:{} reason:{} program_model:{} rules:{} params:{} evidence:{}",
            summary.has_window,
            summary.has_reason_profile,
            summary.has_program_model,
            summary.program_rule_count,
            summary.fragment_param_count,
            summary.evidence_override_count
        ));
    }
    if let Some(delta) = &report.frontend_lowering_delta {
        lines.push(format!(
            "delta=frontend_functions:{} frontend_steps:{} lowered_fragments:{} lowered_rules:{}",
            delta.frontend_function_count,
            delta.frontend_merged_step_count,
            delta.lowered_fragment_count,
            delta.lowered_program_rule_count
        ));
    }
    if let Some(note) = &report.binding_shape_note {
        lines.push(format!("binding_note={note}"));
    }
    if let Some(excerpt) = &report.parse_source_excerpt {
        lines.push(format!(
            "parse_source={} {}",
            excerpt.line_text, excerpt.marker
        ));
    }
    if let Some(excerpt) = &report.validation_excerpt {
        lines.push(format!(
            "validation_excerpt={}#{} offsets={:?}",
            excerpt.model, excerpt.rule_index, excerpt.unsupported_payload_offsets
        ));
    }
    if let Some(note) = &report.validation_shape_note {
        lines.push(format!("validation_note={note}"));
    }
    if let Some(excerpt) = &report.diagnostics_excerpt {
        lines.push(format!(
            "diagnostics_excerpt={}#{} missing={} offsets={:?}",
            excerpt.model,
            excerpt.rule_index,
            excerpt.missing_facts.join(","),
            excerpt.unsupported_payload_offsets
        ));
    }
    if let Some(note) = &report.diagnostics_shape_note {
        lines.push(format!("diagnostics_note={note}"));
    }
    lines.join("\n")
}

pub(super) fn explain_json(report: &ExplainReport, focus: Option<ExplainFocus>) -> String {
    let next_step = explain_next_step_hint(report);
    let template_id = report
        .binding
        .as_ref()
        .map(|binding| json_string(&binding.template_id))
        .unwrap_or_else(|| "null".into());
    let operation = report
        .binding
        .as_ref()
        .and_then(|binding| binding.program_model.as_ref())
        .map(|model| json_string(&model.operation))
        .unwrap_or_else(|| "null".into());
    let focus_json = focus
        .map(|focus| json_string(explain_focus_text(focus)))
        .unwrap_or_else(|| "null".into());
    let focused_report_json = focus
        .map(|focus| explain_focus_json(report, focus))
        .unwrap_or_else(|| "null".into());
    let parse_source_excerpt_json = report
        .parse_source_excerpt
        .as_ref()
        .map(source_excerpt_json)
        .unwrap_or_else(|| "null".into());
    let validation_excerpt_json = report
        .validation_excerpt
        .as_ref()
        .map(validation_excerpt_json)
        .unwrap_or_else(|| "null".into());
    let diagnostics_excerpt_json = report
        .diagnostics_excerpt
        .as_ref()
        .map(diagnostics_excerpt_json)
        .unwrap_or_else(|| "null".into());
    let lowered_binding_summary_json = report
        .lowered_binding_summary
        .as_ref()
        .map(lowered_binding_summary_json)
        .unwrap_or_else(|| "null".into());
    let frontend_lowering_delta_json = report
        .frontend_lowering_delta
        .as_ref()
        .map(frontend_lowering_delta_json)
        .unwrap_or_else(|| "null".into());
    let binding_shape_note_json = report
        .binding_shape_note
        .as_ref()
        .map(|note| format!("\"{}\"", json_escape_string(note)))
        .unwrap_or_else(|| "null".into());
    let validation_shape_note_json = report
        .validation_shape_note
        .as_ref()
        .map(|note| format!("\"{}\"", json_escape_string(note)))
        .unwrap_or_else(|| "null".into());
    let diagnostics_shape_note_json = report
        .diagnostics_shape_note
        .as_ref()
        .map(|note| format!("\"{}\"", json_escape_string(note)))
        .unwrap_or_else(|| "null".into());
    format!(
        "{{\"ok\":{},\"summary\":{{\"parse_ok\":{},\"validation_ok\":{},\"diagnostics_ok\":{},\"template_id\":{},\"operation\":{},\"finding_count\":{},\"next_step\":\"{}\",\"focus\":{},\"lowered_binding_summary\":{},\"frontend_lowering_delta\":{},\"binding_shape_note\":{},\"validation_shape_note\":{},\"diagnostics_shape_note\":{},\"parse_source_excerpt\":{},\"validation_excerpt\":{},\"diagnostics_excerpt\":{}}},\"focused_report\":{},\"frontend\":{},\"binding\":{},\"validation\":{},\"diagnostics\":{},\"findings\":{}}}",
        report.ok,
        report.stages.parse.ok,
        report.stages.validation.ok,
        report.stages.diagnostics.ok,
        template_id,
        operation,
        report.findings.findings.len(),
        json_escape_string(next_step),
        focus_json,
        lowered_binding_summary_json,
        frontend_lowering_delta_json,
        binding_shape_note_json,
        validation_shape_note_json,
        diagnostics_shape_note_json,
        parse_source_excerpt_json,
        validation_excerpt_json,
        diagnostics_excerpt_json,
        focused_report_json,
        frontend_json(report.frontend.as_ref()),
        report
            .binding
            .as_ref()
            .map_or_else(|| "null".to_string(), binding_json),
        stages_validation_json(&report.stages.validation),
        report
            .diagnostics
            .as_ref()
            .map_or_else(|| "null".to_string(), diagnostics_json),
        findings_json(&report.findings),
    )
}

pub(super) fn stages_validation_json(report: &ValidationReport) -> String {
    format!(
        "{{\"ok\":{},\"registry\":\"{}\",\"fragment_count\":{},\"program_rule_count\":{},\"reason_rule_count\":{},\"checks\":[{}],\"sampled_payload_offsets\":[{}],\"required_payload_offsets\":[{}],\"unsupported_payload_offsets\":[{}],\"finding\":{}}}",
        report.ok,
        json_escape_string(&report.registry),
        report.fragment_count,
        report.program_rule_count,
        report.reason_rule_count,
        string_json_list(&report.checks),
        u16_json_list(&report.sampled_payload_offsets),
        u16_json_list(&report.required_payload_offsets),
        u16_json_list(&report.unsupported_payload_offsets),
        finding_json(report.finding.as_ref()),
    )
}

pub(super) fn explain_report(envelope: CompilerEnvelope, source: Option<&str>) -> ExplainReport {
    let ok = envelope.stages.parse.ok
        && envelope.stages.validation.ok
        && envelope.stages.diagnostics.ok
        && envelope.findings.findings.is_empty();
    let parse_source_excerpt = source.and_then(|source| {
        envelope
            .stages
            .parse
            .finding
            .as_ref()
            .and_then(|finding| source_excerpt_for_finding(source, finding))
    });
    let validation_excerpt = envelope
        .diagnostics
        .as_ref()
        .and_then(validation_excerpt_from_diagnostics);
    let diagnostics_excerpt = envelope
        .diagnostics
        .as_ref()
        .and_then(diagnostics_excerpt_from_diagnostics);
    let lowered_binding_summary = envelope
        .binding
        .as_ref()
        .map(lowered_binding_summary_from_binding);
    let frontend_lowering_delta = envelope
        .stages
        .parse
        .frontend
        .as_ref()
        .zip(lowered_binding_summary.as_ref())
        .map(|(frontend, lowered)| frontend_lowering_delta(frontend, lowered));
    let binding_shape_note = frontend_lowering_delta
        .as_ref()
        .map(binding_shape_note_from_delta);
    let validation_shape_note = validation_excerpt
        .as_ref()
        .map(validation_shape_note_from_excerpt);
    let diagnostics_shape_note = diagnostics_excerpt
        .as_ref()
        .map(diagnostics_shape_note_from_excerpt);
    ExplainReport {
        ok,
        binding: envelope.binding,
        frontend: envelope.stages.parse.frontend.clone(),
        diagnostics: envelope.diagnostics,
        findings: envelope.findings,
        stages: envelope.stages,
        lowered_binding_summary,
        frontend_lowering_delta,
        binding_shape_note,
        validation_shape_note,
        diagnostics_shape_note,
        parse_source_excerpt,
        validation_excerpt,
        diagnostics_excerpt,
    }
}
