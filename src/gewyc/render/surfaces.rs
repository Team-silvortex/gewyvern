use super::super::render_support::*;
use super::super::*;

pub(super) fn binding_text(report: &BindingReport) -> String {
    let mut lines = vec![
        format!("template={}", report.template_id),
        format!("fragments={}", report.fragments.join(",")),
    ];

    if let Some(window) = &report.window {
        lines.push(format!(
            "window={} duration_ms={} lateness_ms={}",
            window.id, window.duration_ms, window.lateness_ms
        ));
    }

    if let Some(reason) = &report.reason_profile {
        lines.push(format!("reason={}", reason_profile_text(reason)));
    }

    if let Some(model) = &report.program_model {
        lines.push(format!(
            "program_model={} operation={} rules={}",
            model.id, model.operation, model.rules
        ));
    }

    for param in &report.fragment_params {
        lines.push(format!(
            "param={}.{}={}",
            param.fragment,
            param.key,
            fragment_param_text(&param.value)
        ));
    }

    for evidence in &report.evidence_overrides {
        lines.push(format!("evidence={}:{}", evidence.fact_kind, evidence.tier));
    }

    lines.join("\n")
}

pub(super) fn binding_json(report: &BindingReport) -> String {
    let fragment_params = report
        .fragment_params
        .iter()
        .fold(Vec::<(String, Vec<String>)>::new(), |mut acc, param| {
            if let Some((_, entries)) = acc
                .iter_mut()
                .find(|(fragment, _)| fragment == &param.fragment)
            {
                entries.push(format!(
                    "{}:{}",
                    json_string(&param.key),
                    fragment_param_json(&param.value)
                ));
            } else {
                acc.push((
                    param.fragment.clone(),
                    vec![format!(
                        "{}:{}",
                        json_string(&param.key),
                        fragment_param_json(&param.value)
                    )],
                ));
            }
            acc
        })
        .into_iter()
        .map(|(fragment, entries)| format!("{}:{{{}}}", json_string(&fragment), entries.join(",")))
        .collect::<Vec<_>>()
        .join(",");
    let evidence_overrides = report
        .evidence_overrides
        .iter()
        .map(|evidence| {
            format!(
                "{}:{}",
                json_string(&evidence.fact_kind),
                json_string(&evidence.tier)
            )
        })
        .collect::<Vec<_>>()
        .join(",");

    format!(
        concat!(
            "{{",
            "\"template_id\":\"{}\",",
            "\"status\":{{\"has_window\":{},\"has_reason_profile\":{},\"has_program_model\":{}}},",
            "\"counts\":{{\"fragments\":{},\"fragment_params\":{},\"evidence_overrides\":{}}},",
            "\"fragments\":[{}],",
            "\"window\":{},",
            "\"reason_profile\":{},",
            "\"program_model\":{},",
            "\"fragment_params\":{{{}}},",
            "\"evidence_overrides\":{{{}}}",
            "}}"
        ),
        json_escape_string(&report.template_id),
        report.window.is_some(),
        report.reason_profile.is_some(),
        report.program_model.is_some(),
        report.fragments.len(),
        report.fragment_params.len(),
        report.evidence_overrides.len(),
        string_json_list(&report.fragments),
        report
            .window
            .as_ref()
            .map_or("null".into(), |window| format!(
                "{{\"id\":{},\"duration_ms\":{},\"lateness_ms\":{}}}",
                json_string(&window.id),
                window.duration_ms,
                window.lateness_ms
            )),
        report
            .reason_profile
            .as_ref()
            .map_or("null".into(), reason_profile_json),
        report
            .program_model
            .as_ref()
            .map_or("null".into(), |model| format!(
                "{{\"id\":{},\"operation\":{},\"rules\":{}}}",
                json_string(&model.id),
                json_string(&model.operation),
                model.rules
            )),
        fragment_params,
        evidence_overrides
    )
}

pub(super) fn diagnostics_text(report: &DiagnosticsReport) -> String {
    let mut lines = vec![
        format!("template={}", report.template_id),
        format!("fragments={}", report.fragments.join(",")),
    ];

    if let Some(model) = &report.program_model {
        lines.push(format!("program_model={}", model.model));
        for rule in &model.rules {
            lines.push(format!(
                "  program_rule[{}]: tier={} supported={} required={:?} supporting={:?} missing={:?} unsupported_offsets={:?}",
                rule.rule_index,
                rule.tier,
                rule.supported,
                rule.required_facts,
                rule.supporting_fragments,
                rule.missing_facts,
                rule.unsupported_payload_offsets,
            ));
        }
    }

    if let Some(model) = &report.reason_model {
        lines.push(format!("reason_model={}", model.model));
        for rule in &model.rules {
            lines.push(format!(
                "  reason_rule[{}]: tier={} supported={} required={:?} supporting={:?} missing={:?} unsupported_offsets={:?}",
                rule.rule_index,
                rule.tier,
                rule.supported,
                rule.required_facts,
                rule.supporting_fragments,
                rule.missing_facts,
                rule.unsupported_payload_offsets,
            ));
        }
    }

    lines.join("\n")
}

pub(super) fn diagnostics_json(report: &DiagnosticsReport) -> String {
    format!(
        concat!(
            "{{",
            "\"template_id\":\"{}\",",
            "\"status\":{{\"has_program_model\":{},\"has_reason_model\":{}}},",
            "\"counts\":{{\"fragments\":{},\"program_rules\":{},\"reason_rules\":{}}},",
            "\"fragments\":[{}],",
            "\"program_model\":{},",
            "\"reason_model\":{}",
            "}}"
        ),
        json_escape_string(&report.template_id),
        report.program_model.is_some(),
        report.reason_model.is_some(),
        report.fragments.len(),
        report
            .program_model
            .as_ref()
            .map(|model| model.rules.len())
            .unwrap_or(0),
        report
            .reason_model
            .as_ref()
            .map(|model| model.rules.len())
            .unwrap_or(0),
        string_json_list(&report.fragments),
        report
            .program_model
            .as_ref()
            .map_or("null".into(), model_diagnostics_json),
        report
            .reason_model
            .as_ref()
            .map_or("null".into(), model_diagnostics_json),
    )
}

pub(super) fn findings_text(report: &CompilerFindingsReport) -> String {
    if report.findings.is_empty() {
        return "findings=none".into();
    }

    report
        .findings
        .iter()
        .map(finding_text_record)
        .map(|finding| format!("finding {finding}"))
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn findings_json(report: &CompilerFindingsReport) -> String {
    format!(
        "{{\"findings\":[{}]}}",
        report
            .findings
            .iter()
            .map(finding_json_record)
            .collect::<Vec<_>>()
            .join(",")
    )
}

pub(super) fn envelope_text(report: &CompilerEnvelope) -> String {
    let mut sections = Vec::new();
    sections.push("surface=binding".to_string());
    sections.push(
        report
            .binding
            .as_ref()
            .map_or_else(|| "binding=none".to_string(), binding_text),
    );
    sections.push("surface=diagnostics".to_string());
    sections.push(
        report
            .diagnostics
            .as_ref()
            .map_or_else(|| "diagnostics=none".to_string(), diagnostics_text),
    );
    sections.push("surface=findings".to_string());
    sections.push(findings_text(&report.findings));
    sections.push("surface=stages".to_string());
    sections.push(stages_text(&report.stages));
    sections.join("\n")
}

pub(super) fn envelope_json(report: &CompilerEnvelope) -> String {
    format!(
        "{{\"status\":{{\"has_binding\":{},\"has_diagnostics\":{},\"finding_count\":{}}},\"surfaces\":{{\"binding\":{},\"diagnostics\":{},\"findings\":{},\"stages\":{}}},\"binding\":{},\"diagnostics\":{},\"findings\":{},\"stages\":{}}}",
        report.binding.is_some(),
        report.diagnostics.is_some(),
        report.findings.findings.len(),
        report
            .binding
            .as_ref()
            .map_or_else(|| "null".to_string(), binding_json),
        report
            .diagnostics
            .as_ref()
            .map_or_else(|| "null".to_string(), diagnostics_json),
        findings_json(&report.findings),
        stages_json(&report.stages),
        report
            .binding
            .as_ref()
            .map_or_else(|| "null".to_string(), binding_json),
        report
            .diagnostics
            .as_ref()
            .map_or_else(|| "null".to_string(), diagnostics_json),
        findings_json(&report.findings),
        stages_json(&report.stages),
    )
}

pub(super) fn stages_text(report: &CompilerStagesReport) -> String {
    format!(
        "stage=parse\nok={}\nfrontend={}\nparse_finding={}\n{}\nstage=validation\nok={}\nregistry={}\nfragments={}\nprogram_rules={}\nreason_rules={}\nchecks={}\nsampled_payload_offsets={:?}\nrequired_payload_offsets={:?}\nunsupported_payload_offsets={:?}\nvalidation_finding={}\nstage=diagnostics\nok={}\ndiagnostics_finding={}\n{}",
        report.parse.ok,
        frontend_text(report.parse.frontend.as_ref()),
        finding_text(report.parse.finding.as_ref()),
        report
            .parse
            .report
            .as_ref()
            .map_or_else(String::new, binding_text),
        report.validation.ok,
        report.validation.registry,
        report.validation.fragment_count,
        report.validation.program_rule_count,
        report.validation.reason_rule_count,
        report.validation.checks.join(","),
        report.validation.sampled_payload_offsets,
        report.validation.required_payload_offsets,
        report.validation.unsupported_payload_offsets,
        finding_text(report.validation.finding.as_ref()),
        report.diagnostics.ok,
        finding_text(report.diagnostics.finding.as_ref()),
        report
            .diagnostics
            .report
            .as_ref()
            .map_or_else(String::new, diagnostics_text)
    )
}

pub(super) fn stages_json(report: &CompilerStagesReport) -> String {
    format!(
        "{{\"status\":{{\"parse_ok\":{},\"validation_ok\":{},\"diagnostics_ok\":{}}},\"counts\":{{\"validation_fragments\":{},\"validation_program_rules\":{},\"validation_reason_rules\":{},\"sampled_payload_offsets\":{},\"required_payload_offsets\":{},\"unsupported_payload_offsets\":{}}},\"parse\":{{\"ok\":{},\"frontend\":{},\"finding\":{},\"report\":{}}},\"validation\":{{\"ok\":{},\"registry\":\"{}\",\"fragment_count\":{},\"program_rule_count\":{},\"reason_rule_count\":{},\"checks\":[{}],\"sampled_payload_offsets\":[{}],\"required_payload_offsets\":[{}],\"unsupported_payload_offsets\":[{}],\"finding\":{}}},\"diagnostics\":{{\"ok\":{},\"finding\":{},\"report\":{}}}}}",
        report.parse.ok,
        report.validation.ok,
        report.diagnostics.ok,
        report.validation.fragment_count,
        report.validation.program_rule_count,
        report.validation.reason_rule_count,
        report.validation.sampled_payload_offsets.len(),
        report.validation.required_payload_offsets.len(),
        report.validation.unsupported_payload_offsets.len(),
        report.parse.ok,
        frontend_json(report.parse.frontend.as_ref()),
        finding_json(report.parse.finding.as_ref()),
        report
            .parse
            .report
            .as_ref()
            .map_or_else(|| "null".to_string(), binding_json),
        report.validation.ok,
        report.validation.registry,
        report.validation.fragment_count,
        report.validation.program_rule_count,
        report.validation.reason_rule_count,
        string_json_list(&report.validation.checks),
        u16_json_list(&report.validation.sampled_payload_offsets),
        u16_json_list(&report.validation.required_payload_offsets),
        u16_json_list(&report.validation.unsupported_payload_offsets),
        finding_json(report.validation.finding.as_ref()),
        report.diagnostics.ok,
        finding_json(report.diagnostics.finding.as_ref()),
        report
            .diagnostics
            .report
            .as_ref()
            .map_or_else(|| "null".to_string(), diagnostics_json),
    )
}

pub(super) fn model_diagnostics_json(model: &ModelDiagnosticsReport) -> String {
    format!(
        "{{\"model\":\"{}\",\"rules\":[{}]}}",
        json_escape_string(&model.model),
        model
            .rules
            .iter()
            .map(|rule| format!(
                "{{\"rule_index\":{},\"tier\":\"{}\",\"supported\":{},\"required_facts\":[{}],\"supporting_fragments\":[{}],\"missing_facts\":[{}],\"unsupported_payload_offsets\":[{}]}}",
                rule.rule_index,
                json_escape_string(&rule.tier),
                rule.supported,
                string_json_list(&rule.required_facts),
                string_json_list(&rule.supporting_fragments),
                string_json_list(&rule.missing_facts),
                rule
                    .unsupported_payload_offsets
                    .iter()
                    .map(|offset| offset.to_string())
                    .collect::<Vec<_>>()
                    .join(","),
            ))
            .collect::<Vec<_>>()
            .join(",")
    )
}
