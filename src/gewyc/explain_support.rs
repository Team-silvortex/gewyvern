use super::*;

pub(super) fn explain_focus_text(focus: ExplainFocus) -> &'static str {
    match focus {
        ExplainFocus::Parse => "parse",
        ExplainFocus::Frontend => "frontend",
        ExplainFocus::Binding => "binding",
        ExplainFocus::Validation => "validation",
        ExplainFocus::Diagnostics => "diagnostics",
        ExplainFocus::Findings => "findings",
    }
}

pub(super) fn explain_focus_text_lines(report: &ExplainReport, focus: ExplainFocus) -> Vec<String> {
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
    match focus {
        ExplainFocus::Parse => format!(
            "{{\"kind\":\"parse\",\"ok\":{},\"finding\":{},\"source_excerpt\":{}}}",
            report.stages.parse.ok,
            finding_json(report.stages.parse.finding.as_ref()),
            report
                .parse_source_excerpt
                .as_ref()
                .map(source_excerpt_json)
                .unwrap_or_else(|| "null".to_string())
        ),
        ExplainFocus::Frontend => format!(
            "{{\"kind\":\"frontend\",\"report\":{}}}",
            frontend_json(report.frontend.as_ref())
        ),
        ExplainFocus::Binding => format!(
            "{{\"kind\":\"binding\",\"lowered_binding_summary\":{},\"frontend_lowering_delta\":{},\"binding_shape_note\":{},\"report\":{}}}",
            report
                .lowered_binding_summary
                .as_ref()
                .map(lowered_binding_summary_json)
                .unwrap_or_else(|| "null".to_string()),
            report
                .frontend_lowering_delta
                .as_ref()
                .map(frontend_lowering_delta_json)
                .unwrap_or_else(|| "null".to_string()),
            report
                .binding_shape_note
                .as_ref()
                .map(|note| format!("\"{}\"", json_escape_string(note)))
                .unwrap_or_else(|| "null".to_string()),
            report
                .binding
                .as_ref()
                .map_or_else(|| "null".to_string(), binding_json)
        ),
        ExplainFocus::Validation => format!(
            "{{\"kind\":\"validation\",\"report\":{},\"validation_excerpt\":{},\"validation_shape_note\":{}}}",
            stages_validation_json(&report.stages.validation),
            report
                .validation_excerpt
                .as_ref()
                .map(validation_excerpt_json)
                .unwrap_or_else(|| "null".to_string()),
            report
                .validation_shape_note
                .as_ref()
                .map(|note| format!("\"{}\"", json_escape_string(note)))
                .unwrap_or_else(|| "null".to_string())
        ),
        ExplainFocus::Diagnostics => format!(
            "{{\"kind\":\"diagnostics\",\"ok\":{},\"diagnostics_excerpt\":{},\"diagnostics_shape_note\":{},\"report\":{}}}",
            report.stages.diagnostics.ok,
            report
                .diagnostics_excerpt
                .as_ref()
                .map(diagnostics_excerpt_json)
                .unwrap_or_else(|| "null".to_string()),
            report
                .diagnostics_shape_note
                .as_ref()
                .map(|note| format!("\"{}\"", json_escape_string(note)))
                .unwrap_or_else(|| "null".to_string()),
            report
                .diagnostics
                .as_ref()
                .map_or_else(|| "null".to_string(), diagnostics_json)
        ),
        ExplainFocus::Findings => format!(
            "{{\"kind\":\"findings\",\"report\":{}}}",
            findings_json(&report.findings)
        ),
    }
}

pub(super) fn lowered_binding_summary_from_binding(
    binding: &BindingReport,
) -> LoweredBindingSummary {
    LoweredBindingSummary {
        fragment_count: binding.fragments.len(),
        has_window: binding.window.is_some(),
        has_reason_profile: binding.reason_profile.is_some(),
        has_program_model: binding.program_model.is_some(),
        program_rule_count: binding
            .program_model
            .as_ref()
            .map(|model| model.rules)
            .unwrap_or(0),
        fragment_param_count: binding.fragment_params.len(),
        evidence_override_count: binding.evidence_overrides.len(),
    }
}

pub(super) fn lowered_binding_summary_json(summary: &LoweredBindingSummary) -> String {
    format!(
        "{{\"fragment_count\":{},\"has_window\":{},\"has_reason_profile\":{},\"has_program_model\":{},\"program_rule_count\":{},\"fragment_param_count\":{},\"evidence_override_count\":{}}}",
        summary.fragment_count,
        summary.has_window,
        summary.has_reason_profile,
        summary.has_program_model,
        summary.program_rule_count,
        summary.fragment_param_count,
        summary.evidence_override_count
    )
}

pub(super) fn frontend_lowering_delta(
    frontend: &FrontendReport,
    lowered: &LoweredBindingSummary,
) -> FrontendLoweringDelta {
    FrontendLoweringDelta {
        frontend_function_count: frontend.function_count,
        frontend_merged_step_count: frontend.merged_step_count,
        frontend_use_edge_count: frontend.use_edges.len(),
        frontend_include_source_count: frontend.include_sources.len(),
        lowered_fragment_count: lowered.fragment_count,
        lowered_program_rule_count: lowered.program_rule_count,
        lowered_fragment_param_count: lowered.fragment_param_count,
        lowered_evidence_override_count: lowered.evidence_override_count,
    }
}

pub(super) fn frontend_lowering_delta_json(delta: &FrontendLoweringDelta) -> String {
    format!(
        "{{\"frontend_function_count\":{},\"frontend_merged_step_count\":{},\"frontend_use_edge_count\":{},\"frontend_include_source_count\":{},\"lowered_fragment_count\":{},\"lowered_program_rule_count\":{},\"lowered_fragment_param_count\":{},\"lowered_evidence_override_count\":{}}}",
        delta.frontend_function_count,
        delta.frontend_merged_step_count,
        delta.frontend_use_edge_count,
        delta.frontend_include_source_count,
        delta.lowered_fragment_count,
        delta.lowered_program_rule_count,
        delta.lowered_fragment_param_count,
        delta.lowered_evidence_override_count
    )
}

pub(super) fn binding_shape_note_from_delta(delta: &FrontendLoweringDelta) -> String {
    let mut reasons = Vec::new();
    if delta.frontend_use_edge_count > 0 {
        reasons.push("use(...) edges inline reusable function bodies into one binding");
    }
    if delta.frontend_include_source_count > 0 {
        reasons.push("include(...) pulls filesystem-backed modules into the same compiled entry");
    }
    if delta.lowered_program_rule_count > 0 {
        reasons.push("program_rule(...) calls lower into explicit program-model rules");
    }
    if delta.lowered_fragment_param_count > 0 || delta.lowered_evidence_override_count > 0 {
        reasons.push("param(...) and evidence(...) survive as binding-level overrides");
    }

    if reasons.is_empty() {
        "frontend and lowered binding are close in shape; there are no extra use/include or override layers to explain".into()
    } else {
        format!(
            "lowered binding looks different because {}",
            reasons.join("; ")
        )
    }
}

pub(super) fn validation_shape_note_from_excerpt(excerpt: &ValidationExcerpt) -> String {
    let offset_note = if excerpt.unsupported_payload_offsets.is_empty() {
        "rule support failed without explicit payload offsets".to_string()
    } else {
        format!(
            "the first failing rule asks for payload offsets {:?} that current fragment coverage does not sample",
            excerpt.unsupported_payload_offsets
        )
    };
    if excerpt.supporting_fragments.is_empty() {
        offset_note
    } else {
        format!(
            "{}; current support comes from fragments [{}]",
            offset_note,
            excerpt.supporting_fragments.join(", ")
        )
    }
}

pub(super) fn diagnostics_shape_note_from_excerpt(excerpt: &DiagnosticsExcerpt) -> String {
    let mut reasons = Vec::new();
    if !excerpt.missing_facts.is_empty() {
        reasons.push(format!(
            "the first unsupported rule still misses facts [{}]",
            excerpt.missing_facts.join(", ")
        ));
    }
    if !excerpt.unsupported_payload_offsets.is_empty() {
        reasons.push(format!(
            "it also references unsampled payload offsets {:?}",
            excerpt.unsupported_payload_offsets
        ));
    }
    if !excerpt.supporting_fragments.is_empty() {
        reasons.push(format!(
            "current support comes from fragments [{}]",
            excerpt.supporting_fragments.join(", ")
        ));
    }
    if reasons.is_empty() {
        "the first unsupported rule still lacks enough fragment-backed evidence to be supported"
            .into()
    } else {
        reasons.join("; ")
    }
}

pub(super) fn explain_next_step_hint(report: &ExplainReport) -> &'static str {
    if !report.stages.parse.ok {
        if report.frontend.is_some() {
            return "fix the parse finding first, then inspect the standalone frontend graph with `gewyc frontend`";
        }
        return "fix the parse finding first, then rerun `gewyc explain`";
    }

    if !report.stages.validation.ok {
        if !report
            .stages
            .validation
            .unsupported_payload_offsets
            .is_empty()
        {
            return "inspect `unsupported_payload_offsets` and adjust fragment coverage or payload matchers before rerunning";
        }
        return "inspect the validation section and binding fragments before rerunning";
    }

    if !report.stages.diagnostics.ok {
        return "inspect the diagnostics section and rule support details before rerunning";
    }

    if !report.findings.findings.is_empty() {
        return "inspect the findings list first, then drill into `frontend` or `stages` for the failing phase";
    }

    "binding, frontend, validation, and diagnostics are all healthy; continue with runtime/demo verification"
}

pub(super) fn source_excerpt_for_finding(
    source: &str,
    finding: &CompilerFinding,
) -> Option<SourceExcerpt> {
    let line = finding.line?;
    let line_text = source.lines().nth(line.saturating_sub(1))?.to_string();
    let marker_column = finding.column.unwrap_or(1).max(1);
    let marker = format!("{}^", " ".repeat(marker_column.saturating_sub(1)));
    Some(SourceExcerpt {
        line,
        column: finding.column,
        line_text,
        marker,
    })
}

pub(super) fn source_excerpt_json(excerpt: &SourceExcerpt) -> String {
    format!(
        "{{\"line\":{},\"column\":{},\"line_text\":\"{}\",\"marker\":\"{}\"}}",
        excerpt.line,
        excerpt
            .column
            .map(|column| column.to_string())
            .unwrap_or_else(|| "null".to_string()),
        json_escape_string(&excerpt.line_text),
        json_escape_string(&excerpt.marker),
    )
}

pub(super) fn json_string(value: &str) -> String {
    format!("\"{}\"", json_escape_string(value))
}

pub(super) fn validation_excerpt_from_diagnostics(
    diagnostics: &DiagnosticsReport,
) -> Option<ValidationExcerpt> {
    diagnostics
        .program_model
        .as_ref()
        .into_iter()
        .chain(diagnostics.reason_model.as_ref())
        .find_map(|model| {
            model.rules.iter().find_map(|rule| {
                if rule.unsupported_payload_offsets.is_empty() {
                    None
                } else {
                    Some(ValidationExcerpt {
                        model: model.model.clone(),
                        rule_index: rule.rule_index,
                        unsupported_payload_offsets: rule.unsupported_payload_offsets.clone(),
                        supporting_fragments: rule.supporting_fragments.clone(),
                    })
                }
            })
        })
}

pub(super) fn validation_excerpt_json(excerpt: &ValidationExcerpt) -> String {
    format!(
        "{{\"model\":\"{}\",\"rule_index\":{},\"unsupported_payload_offsets\":[{}],\"supporting_fragments\":[{}]}}",
        json_escape_string(&excerpt.model),
        excerpt.rule_index,
        u16_json_list(&excerpt.unsupported_payload_offsets),
        string_json_list(&excerpt.supporting_fragments),
    )
}

pub(super) fn diagnostics_excerpt_from_diagnostics(
    diagnostics: &DiagnosticsReport,
) -> Option<DiagnosticsExcerpt> {
    diagnostics
        .program_model
        .as_ref()
        .into_iter()
        .chain(diagnostics.reason_model.as_ref())
        .find_map(|model| {
            model.rules.iter().find_map(|rule| {
                if rule.supported {
                    None
                } else {
                    Some(DiagnosticsExcerpt {
                        model: model.model.clone(),
                        rule_index: rule.rule_index,
                        missing_facts: rule.missing_facts.clone(),
                        unsupported_payload_offsets: rule.unsupported_payload_offsets.clone(),
                        supporting_fragments: rule.supporting_fragments.clone(),
                    })
                }
            })
        })
}

pub(super) fn diagnostics_excerpt_json(excerpt: &DiagnosticsExcerpt) -> String {
    format!(
        "{{\"model\":\"{}\",\"rule_index\":{},\"missing_facts\":[{}],\"unsupported_payload_offsets\":[{}],\"supporting_fragments\":[{}]}}",
        json_escape_string(&excerpt.model),
        excerpt.rule_index,
        string_json_list(&excerpt.missing_facts),
        u16_json_list(&excerpt.unsupported_payload_offsets),
        string_json_list(&excerpt.supporting_fragments),
    )
}

pub(super) fn json_escape_string(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}
