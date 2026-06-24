use super::super::*;

pub(super) fn explain_focus_text(focus: ExplainFocus) -> &'static str {
    match focus {
        ExplainFocus::Parse => "parse",
        ExplainFocus::Frontend => "frontend",
        ExplainFocus::Binding => "binding",
        ExplainFocus::Ir => "ir",
        ExplainFocus::Validation => "validation",
        ExplainFocus::Diagnostics => "diagnostics",
        ExplainFocus::Findings => "findings",
    }
}

pub(super) fn explain_focus_text_lines(report: &ExplainReport, focus: ExplainFocus) -> Vec<String> {
    // Each focus renders as a self-contained troubleshooting slice so callers can
    // inspect one phase without dragging the whole explain surface along.
    match focus {
        ExplainFocus::Parse => {
            let mut lines = vec![
                format!("parse_ok={}", report.stages.parse.ok),
                format!(
                    "parse_finding={}",
                    finding_text(report.stages.parse.finding.as_ref())
                ),
            ];
            if let Some(excerpt) = &report.parse_source_excerpt {
                lines.push(format!("parse_source_excerpt={}", excerpt.line_text));
                lines.push(format!("parse_source_marker={}", excerpt.marker));
            }
            lines
        }
        ExplainFocus::Frontend => match &report.frontend {
            Some(frontend) => {
                let mut lines = vec!["frontend:".to_string()];
                lines.extend(
                    frontend_report_text(frontend, None)
                        .lines()
                        .map(|line| line.to_string()),
                );
                lines
            }
            None => vec!["frontend=none".into()],
        },
        ExplainFocus::Binding => {
            let mut lines = vec![format!("binding_present={}", report.binding.is_some())];
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
            } else {
                lines.push("lowered_binding_summary=none".into());
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
            if let Some(binding) = &report.binding {
                lines.extend(binding_text(binding).lines().map(|line| line.to_string()));
            } else {
                lines.push("binding=none".into());
            }
            lines
        }
        ExplainFocus::Ir => match &report.ir_report {
            Some(ir_report) => {
                let mut lines = vec!["ir:".to_string()];
                if let Some(delta) = &report.ir_lowering_delta {
                    lines.extend(ir_lowering_delta_text_lines(delta));
                } else {
                    lines.push("ir_delta=none".into());
                }
                if let Some(note) = &report.ir_shape_note {
                    lines.push(format!("ir_note={note}"));
                }
                lines.extend(ir_text(ir_report).lines().map(|line| line.to_string()));
                lines
            }
            None => vec!["ir=none".into()],
        },
        ExplainFocus::Validation => {
            vec![
                format!("validation_ok={}", report.stages.validation.ok),
                format!("registry={}", report.stages.validation.registry),
                format!("checks={}", report.stages.validation.checks.join(",")),
                format!(
                    "unsupported_payload_offsets={:?}",
                    report.stages.validation.unsupported_payload_offsets
                ),
                report
                    .validation_excerpt
                    .as_ref()
                    .map(|excerpt| format!(
                        "validation_excerpt=model:{} rule:{} offsets:{:?} supporting_fragments:{}",
                        excerpt.model,
                        excerpt.rule_index,
                        excerpt.unsupported_payload_offsets,
                        excerpt.supporting_fragments.join(",")
                    ))
                    .unwrap_or_else(|| "validation_excerpt=none".into()),
                report
                    .validation_shape_note
                    .as_ref()
                    .map(|note| format!("validation_note={note}"))
                    .unwrap_or_else(|| "validation_note=none".into()),
                format!(
                    "validation_finding={}",
                    finding_text(report.stages.validation.finding.as_ref())
                ),
            ]
        }
        ExplainFocus::Diagnostics => match &report.diagnostics {
            Some(diagnostics) => {
                let mut lines = vec![format!("diagnostics_ok={}", report.stages.diagnostics.ok)];
                lines.push(
                    report
                        .diagnostics_excerpt
                        .as_ref()
                        .map(|excerpt| format!(
                            "diagnostics_excerpt=model:{} rule:{} missing_facts:{} offsets:{:?} supporting_fragments:{}",
                            excerpt.model,
                            excerpt.rule_index,
                            excerpt.missing_facts.join(","),
                            excerpt.unsupported_payload_offsets,
                            excerpt.supporting_fragments.join(",")
                        ))
                        .unwrap_or_else(|| "diagnostics_excerpt=none".into()),
                );
                lines.push(
                    report
                        .diagnostics_shape_note
                        .as_ref()
                        .map(|note| format!("diagnostics_note={note}"))
                        .unwrap_or_else(|| "diagnostics_note=none".into()),
                );
                lines.extend(
                    diagnostics_text(diagnostics)
                        .lines()
                        .map(|line| line.to_string()),
                );
                lines
            }
            None => vec![
                format!("diagnostics_ok={}", report.stages.diagnostics.ok),
                "diagnostics=none".into(),
            ],
        },
        ExplainFocus::Findings => {
            let mut lines = vec![format!("finding_count={}", report.findings.findings.len())];
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
            lines
        }
    }
}

pub(super) fn explain_focus_json(report: &ExplainReport, focus: ExplainFocus) -> String {
    // Keep JSON focus payloads aligned with the text slices above so scripts and
    // humans can pivot across the same explain viewpoints.
    match focus {
        ExplainFocus::Parse => {
            let source_excerpt_json = report
                .parse_source_excerpt
                .as_ref()
                .map(source_excerpt_json)
                .unwrap_or_else(|| "null".to_string());
            format!(
                "{{\"kind\":\"parse\",\"status\":{{\"ok\":{}}},\"analysis\":{{\"finding\":{}}},\"excerpts\":{{\"parse_source\":{}}},\"report\":{{\"finding\":{},\"source_excerpt\":{}}}}}",
                report.stages.parse.ok,
                finding_json(report.stages.parse.finding.as_ref()),
                source_excerpt_json,
                finding_json(report.stages.parse.finding.as_ref()),
                source_excerpt_json,
            )
        }
        ExplainFocus::Frontend => format!(
            "{{\"kind\":\"frontend\",\"status\":{{\"present\":{}}},\"analysis\":{{\"authoring_context\":{}}},\"report\":{}}}",
            report.frontend.is_some(),
            report
                .frontend
                .as_ref()
                .map(frontend_authoring_context_json)
                .unwrap_or_else(|| "null".to_string()),
            frontend_json(report.frontend.as_ref())
        ),
        ExplainFocus::Binding => {
            let lowered_binding_summary_json = report
                .lowered_binding_summary
                .as_ref()
                .map(lowered_binding_summary_json)
                .unwrap_or_else(|| "null".to_string());
            let frontend_lowering_delta_json = report
                .frontend_lowering_delta
                .as_ref()
                .map(frontend_lowering_delta_json)
                .unwrap_or_else(|| "null".to_string());
            let binding_shape_note_json = report
                .binding_shape_note
                .as_ref()
                .map(|note| format!("\"{}\"", json_escape_string(note)))
                .unwrap_or_else(|| "null".to_string());
            format!(
                "{{\"kind\":\"binding\",\"status\":{{\"present\":{}}},\"analysis\":{{\"lowered_binding_summary\":{},\"frontend_lowering_delta\":{}}},\"shape_notes\":{{\"binding\":{}}},\"report\":{}}}",
                report.binding.is_some(),
                lowered_binding_summary_json,
                frontend_lowering_delta_json,
                binding_shape_note_json,
                report
                    .binding
                    .as_ref()
                    .map_or_else(|| "null".to_string(), binding_json)
            )
        }
        ExplainFocus::Ir => {
            let ir_lowering_delta_json = report
                .ir_lowering_delta
                .as_ref()
                .map(ir_lowering_delta_json)
                .unwrap_or_else(|| "null".to_string());
            let ir_shape_note_json = report
                .ir_shape_note
                .as_ref()
                .map(|note| format!("\"{}\"", json_escape_string(note)))
                .unwrap_or_else(|| "null".to_string());
            format!(
                "{{\"kind\":\"ir\",\"status\":{{\"present\":{}}},\"analysis\":{{\"ir_lowering_delta\":{}}},\"shape_notes\":{{\"ir\":{}}},\"report\":{}}}",
                report.ir_report.is_some(),
                ir_lowering_delta_json,
                ir_shape_note_json,
                report
                    .ir_report
                    .as_ref()
                    .map(ir_json)
                    .unwrap_or_else(|| "null".to_string())
            )
        }
        ExplainFocus::Validation => {
            let validation_excerpt_json = report
                .validation_excerpt
                .as_ref()
                .map(validation_excerpt_json)
                .unwrap_or_else(|| "null".to_string());
            let validation_shape_note_json = report
                .validation_shape_note
                .as_ref()
                .map(|note| format!("\"{}\"", json_escape_string(note)))
                .unwrap_or_else(|| "null".to_string());
            format!(
                "{{\"kind\":\"validation\",\"status\":{{\"ok\":{}}},\"shape_notes\":{{\"validation\":{}}},\"excerpts\":{{\"validation\":{}}},\"report\":{}}}",
                report.stages.validation.ok,
                validation_shape_note_json,
                validation_excerpt_json,
                stages_validation_json(&report.stages.validation)
            )
        }
        ExplainFocus::Diagnostics => {
            let diagnostics_excerpt_json = report
                .diagnostics_excerpt
                .as_ref()
                .map(diagnostics_excerpt_json)
                .unwrap_or_else(|| "null".to_string());
            let diagnostics_shape_note_json = report
                .diagnostics_shape_note
                .as_ref()
                .map(|note| format!("\"{}\"", json_escape_string(note)))
                .unwrap_or_else(|| "null".to_string());
            format!(
                "{{\"kind\":\"diagnostics\",\"status\":{{\"ok\":{},\"present\":{}}},\"shape_notes\":{{\"diagnostics\":{}}},\"excerpts\":{{\"diagnostics\":{}}},\"report\":{}}}",
                report.stages.diagnostics.ok,
                report.diagnostics.is_some(),
                diagnostics_shape_note_json,
                diagnostics_excerpt_json,
                report
                    .diagnostics
                    .as_ref()
                    .map_or_else(|| "null".to_string(), diagnostics_json)
            )
        }
        ExplainFocus::Findings => format!(
            "{{\"kind\":\"findings\",\"status\":{{\"count\":{}}},\"report\":{}}}",
            report.findings.findings.len(),
            findings_json(&report.findings)
        ),
    }
}

fn frontend_authoring_context_json(frontend: &FrontendReport) -> String {
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
