use super::*;
use crate::gewyc::ir_focus::support::{narrative_summary, predicate_summary};

pub(super) fn ir_report_from_binding(
    binding: &TemplateBinding,
    diagnostics: Option<&BindingDiagnostics>,
) -> IrReport {
    IrReport {
        template_id: binding.template.id.to_string(),
        program_model: binding
            .template
            .program_model
            .as_ref()
            .map(|model| IrModelReport {
                kind: "program_model".into(),
                id: model.id.to_string(),
                operation: Some(program_operation_text(&model.operation).to_string()),
                rules: model
                    .rules
                    .iter()
                    .enumerate()
                    .map(|(rule_index, rule)| {
                        ir_rule_report(
                            rule_index,
                            rule,
                            diagnostics.and_then(|all| all.program_model.as_ref()),
                        )
                    })
                    .collect(),
            }),
        reason_model: binding.template.reason_profile.as_ref().map(|profile| {
            let (kind, id, rules): (&str, &str, &[ReasonRule]) = match profile {
                ReasonProfile::HandshakeL1 => ("builtin_reason_profile", profile.id(), &[]),
                ReasonProfile::UdpDatagramL1 => ("builtin_reason_profile", profile.id(), &[]),
                ReasonProfile::Declarative(model) => (
                    "declarative_reason_model",
                    model.id.as_str(),
                    model.rules.as_slice(),
                ),
            };
            IrModelReport {
                kind: kind.into(),
                id: id.into(),
                operation: None,
                rules: rules
                    .iter()
                    .enumerate()
                    .map(|(rule_index, rule)| {
                        ir_rule_report(
                            rule_index,
                            rule,
                            diagnostics.and_then(|all| all.reason_model.as_ref()),
                        )
                    })
                    .collect(),
            }
        }),
    }
}

pub(super) fn ir_rule_report(
    rule_index: usize,
    rule: &ProgramRule,
    diagnostics: Option<&ModelDiagnostics>,
) -> IrRuleReport {
    let diagnostics = diagnostics.and_then(|model| rule_diagnostics(model, rule_index));
    let required_facts = rule
        .predicate
        .required_fact_kinds()
        .into_iter()
        .map(|fact| fact.to_string())
        .collect::<Vec<_>>();
    IrRuleReport {
        rule_index,
        predicate: predicate_summary(&rule.predicate),
        signal: rule.signal.as_ref().map(|signal| signal.id().to_string()),
        narrative: narrative_summary(&rule.narrative),
        dedupe: rule.dedupe,
        module: rule.module.clone(),
        phase: rule.phase.clone(),
        phase_kind: rule
            .signal
            .as_ref()
            .and_then(|signal| phase_kind(signal, rule.phase.as_deref()))
            .map(str::to_string),
        required_facts: diagnostics
            .map(|diagnostics| {
                diagnostics
                    .required_facts
                    .iter()
                    .map(|fact| fact.to_string())
                    .collect()
            })
            .unwrap_or(required_facts),
        supporting_fragments: diagnostics
            .map(|diagnostics| diagnostics.supporting_fragments.clone())
            .unwrap_or_default(),
        missing_facts: diagnostics
            .map(|diagnostics| {
                diagnostics
                    .missing_facts
                    .iter()
                    .map(|fact| fact.to_string())
                    .collect()
            })
            .unwrap_or_default(),
        unsupported_payload_offsets: diagnostics
            .map(|diagnostics| diagnostics.unsupported_payload_offsets.clone())
            .unwrap_or_default(),
        supported: diagnostics
            .map(|diagnostics| diagnostics.supported)
            .unwrap_or(true),
    }
}

fn rule_diagnostics(diagnostics: &ModelDiagnostics, rule_index: usize) -> Option<&RuleDiagnostics> {
    diagnostics
        .rules
        .iter()
        .find(|diagnostics| diagnostics.rule_index == rule_index)
}
