use super::super::explain_support::*;
use super::super::*;

pub(super) fn explain_text(report: &ExplainReport, focus: Option<ExplainFocus>) -> String {
    // The full surface optimizes for "what failed and what to inspect next"
    // instead of mirroring every internal structure one-to-one.
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
    if let Some(frontend) = &report.frontend {
        lines.push(format!("authoring={}", explain_authoring_context(frontend)));
    }

    if let Some(focus) = focus {
        // Focus mode intentionally short-circuits into a single-phase view so
        // shell users do not have to scroll past unrelated sections.
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
        lines.push(format!(
            "- module_doc={}",
            explain_doc_text(frontend.module_doc.as_deref(), "none")
        ));
        lines.push(format!(
            "- template_doc={}",
            explain_doc_text(frontend.template_doc.as_deref(), "none")
        ));
        lines.push(format!("- function_count={}", frontend.function_count));
        lines.push(format!(
            "- documented_functions={}",
            explain_documented_functions(frontend, "none")
        ));
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
    // Compact mode keeps stable one-line summaries for scripts and snapshot tests.
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
                        "frontend kind={} module_doc={} template_doc={} functions={} documented_functions={} includes={} use_edges={} graph_nodes={} graph_edges={}",
                        frontend.kind,
                        explain_doc_text(frontend.module_doc.as_deref(), "none"),
                        explain_doc_text(frontend.template_doc.as_deref(), "none"),
                        frontend.function_count,
                        explain_documented_functions(frontend, "none"),
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
            ExplainFocus::Ir => {
                if let Some(ir_report) = &report.ir_report {
                    let program_rules = ir_report
                        .program_model
                        .as_ref()
                        .map(|model| model.rules.len())
                        .unwrap_or(0);
                    let reason_rules = ir_report
                        .reason_model
                        .as_ref()
                        .map(|model| model.rules.len())
                        .unwrap_or(0);
                    lines.push(format!(
                        "ir program_rules={} reason_rules={}",
                        program_rules, reason_rules
                    ));
                    if let Some(compare) = ir_report.compare_models() {
                        lines.push(format!(
                            "ir_compare rule_delta={} supported_delta={} shared_modules={} shared_phases={}",
                            compare.rule_count_delta,
                            compare.supported_rule_count_delta,
                            compare.shared_modules.join(","),
                            compare.shared_phases.join(",")
                        ));
                    }
                    if let Some(delta) = &report.ir_lowering_delta {
                        lines.push(format!(
                            "ir_delta frontend_functions={} frontend_includes={} frontend_use_edges={} frontend_graph_nodes={} frontend_graph_edges={} lowered_program_rules={} lowered_reason_rules={} lowered_supported_rules={} lowered_unsupported_rules={}",
                            delta.frontend_function_count,
                            delta.frontend_include_source_count,
                            delta.frontend_use_edge_count,
                            delta.frontend_graph_node_count,
                            delta.frontend_graph_edge_count,
                            delta.lowered_program_rule_count,
                            delta.lowered_reason_rule_count,
                            delta.lowered_supported_rule_count,
                            delta.lowered_unsupported_rule_count,
                        ));
                    }
                    if let Some(note) = &report.ir_shape_note {
                        lines.push(format!("ir_note={note}"));
                    }
                } else {
                    lines.push("ir=none".into());
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

trait ExplainStringExt {
    fn if_empty_then(self, fallback: &str) -> String;
}

impl ExplainStringExt for String {
    fn if_empty_then(self, fallback: &str) -> String {
        if self.is_empty() {
            fallback.to_string()
        } else {
            self
        }
    }
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
    let authoring_context_json = report
        .frontend
        .as_ref()
        .map(explain_authoring_context_json)
        .unwrap_or_else(|| "null".into());
    let stage_status_json = format!(
        "{{\"parse\":{},\"validation\":{},\"diagnostics\":{}}}",
        report.stages.parse.ok, report.stages.validation.ok, report.stages.diagnostics.ok
    );
    let analysis_json = format!(
        "{{\"authoring_context\":{},\"lowered_binding_summary\":{},\"frontend_lowering_delta\":{}}}",
        authoring_context_json, lowered_binding_summary_json, frontend_lowering_delta_json
    );
    let shape_notes_json = format!(
        "{{\"binding\":{},\"validation\":{},\"diagnostics\":{}}}",
        binding_shape_note_json, validation_shape_note_json, diagnostics_shape_note_json
    );
    let excerpts_json = format!(
        "{{\"parse_source\":{},\"validation\":{},\"diagnostics\":{}}}",
        parse_source_excerpt_json, validation_excerpt_json, diagnostics_excerpt_json
    );
    format!(
        "{{\"ok\":{},\"summary\":{{\"parse_ok\":{},\"validation_ok\":{},\"diagnostics_ok\":{},\"template_id\":{},\"operation\":{},\"finding_count\":{},\"next_step\":\"{}\",\"focus\":{},\"stage_status\":{},\"analysis\":{},\"shape_notes\":{},\"excerpts\":{},\"authoring_context\":{},\"lowered_binding_summary\":{},\"frontend_lowering_delta\":{},\"binding_shape_note\":{},\"validation_shape_note\":{},\"diagnostics_shape_note\":{},\"parse_source_excerpt\":{},\"validation_excerpt\":{},\"diagnostics_excerpt\":{}}},\"focused_report\":{},\"frontend\":{},\"binding\":{},\"validation\":{},\"diagnostics\":{},\"findings\":{}}}",
        report.ok,
        report.stages.parse.ok,
        report.stages.validation.ok,
        report.stages.diagnostics.ok,
        template_id,
        operation,
        report.findings.findings.len(),
        json_escape_string(next_step),
        focus_json,
        stage_status_json,
        analysis_json,
        shape_notes_json,
        excerpts_json,
        authoring_context_json,
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

fn explain_authoring_context(frontend: &FrontendReport) -> String {
    let module_doc = explain_doc_text(frontend.module_doc.as_deref(), "no module doc");
    let template_doc = explain_doc_text(frontend.template_doc.as_deref(), "no template doc");
    let functions = explain_documented_functions(frontend, "none");
    format!(
        "module_doc={} ; template_doc={} ; documented_functions={}",
        module_doc, template_doc, functions
    )
}

fn explain_authoring_context_json(frontend: &FrontendReport) -> String {
    format!(
        "{{\"module_doc\":{},\"template_doc\":{},\"documented_functions\":[{}]}}",
        frontend
            .module_doc
            .as_deref()
            .map(json_string)
            .unwrap_or_else(|| "null".into()),
        frontend
            .template_doc
            .as_deref()
            .map(json_string)
            .unwrap_or_else(|| "null".into()),
        frontend
            .function_nodes
            .iter()
            .filter_map(|node| node.doc.as_ref().map(|_| json_string(&node.name)))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn explain_doc_text(doc: Option<&str>, fallback: &str) -> String {
    doc.unwrap_or(fallback).replace('\n', " / ")
}

fn explain_documented_functions(frontend: &FrontendReport, fallback: &str) -> String {
    frontend
        .function_nodes
        .iter()
        .filter_map(|node| node.doc.as_ref().map(|_| node.name.as_str()))
        .collect::<Vec<_>>()
        .join(",")
        .if_empty_then(fallback)
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
