use std::collections::BTreeMap;

use crate::flow::{
    ModuleFinding, ModuleSeverity, ProcessView, ProgramFinding, ProgramFindingCause, ProgramFlow,
};
use crate::fragment::{AttachReport, BindingDiagnostics, RuleTier};
use crate::ir::render_phase_transition_kind;

struct FindingRule<'a> {
    rule_index: usize,
    rule: &'a crate::program::ProgramRule,
    signal: &'a crate::ir::SignalKind,
    supporting_fragments: &'a [String],
    cause: ProgramFindingCause,
    external_evidence: Vec<String>,
}

pub(super) fn build_program_findings(
    model: &crate::program::ProgramModel,
    binding_diagnostics: &BindingDiagnostics,
    attach_report: &AttachReport,
    rejected_facts: &[super::RejectedFact],
    program_flows: &[ProgramFlow],
) -> Vec<ProgramFinding> {
    let Some(model_diagnostics) = &binding_diagnostics.program_model else {
        return Vec::new();
    };

    let finding_rules = model_diagnostics
        .rules
        .iter()
        .filter_map(|rule_diag| {
            if rule_diag.tier != RuleTier::CoreRequirement || !rule_diag.supported {
                return None;
            }
            let rule = model.rules.get(rule_diag.rule_index)?;
            let signal = rule.signal.as_ref()?;
            let failed_hookpoints = attach_report
                .hookpoints_failed
                .iter()
                .filter(|hookpoint| {
                    hookpoint.split_once('@').is_some_and(|(fragment_id, _)| {
                        rule_diag
                            .supporting_fragments
                            .iter()
                            .any(|fragment| fragment == fragment_id)
                    })
                })
                .collect::<Vec<_>>();
            let matching_rejected_facts = rejected_facts
                .iter()
                .filter(|rejected| {
                    rule_diag
                        .supporting_fragments
                        .iter()
                        .any(|fragment| fragment == &rejected.fragment_id)
                })
                .collect::<Vec<_>>();
            let cause = if !failed_hookpoints.is_empty() {
                ProgramFindingCause::AttachFailure
            } else if !matching_rejected_facts.is_empty() {
                ProgramFindingCause::RejectedEvidence
            } else {
                ProgramFindingCause::MissingCoreStage
            };
            let external_evidence = failed_hookpoints
                .into_iter()
                .map(|hookpoint| format!("failed_hookpoint:{hookpoint}"))
                .chain(matching_rejected_facts.into_iter().map(|rejected| {
                    format!(
                        "rejected_fact:{}:{}:{}",
                        rejected.id.0,
                        rejected.fragment_id,
                        rejected.reason.label()
                    )
                }))
                .collect();

            Some(FindingRule {
                rule_index: rule_diag.rule_index,
                rule,
                signal,
                supporting_fragments: &rule_diag.supporting_fragments,
                cause,
                external_evidence,
            })
        })
        .collect::<Vec<_>>();

    program_flows
        .iter()
        .flat_map(|flow| {
            finding_rules.iter().filter_map(|finding_rule| {
                let rule = finding_rule.rule;
                let signal = finding_rule.signal;
                if flow
                    .stages
                    .iter()
                    .any(|stage| &stage.kind == signal && stage.phase == rule.phase)
                {
                    return None;
                }
                if !prior_phase_requirements_satisfied(model, finding_rule.rule_index, flow) {
                    return None;
                }

                let suspect_area = signal.suspect_area().to_string();
                let phase_kind = signal.phase_kind(rule.phase.as_deref()).map(str::to_string);
                let (phase_transition, phase_transition_kind) =
                    phase_transition_for_rule(model, finding_rule.rule_index, flow);
                let network_module_kind = crate::flow::infer_network_module_kind(
                    &flow.operation,
                    rule.phase.as_deref(),
                    phase_transition.as_deref(),
                    &suspect_area,
                )
                .to_string();
                let module_label = module_label(
                    rule.module.as_deref(),
                    &flow.operation,
                    &suspect_area,
                    finding_rule.supporting_fragments,
                );
                let evidence_trace =
                    build_evidence_trace(signal, flow, &finding_rule.external_evidence);
                let summary = finding_summary(
                    flow,
                    rule.phase.as_deref(),
                    phase_transition.as_deref(),
                    &suspect_area,
                    &finding_rule.cause,
                );
                Some(ProgramFinding {
                    program_flow: flow.id,
                    process: flow.process.clone(),
                    operation: flow.operation.clone(),
                    module_label,
                    network_module_kind,
                    phase: rule.phase.clone(),
                    phase_kind,
                    phase_transition,
                    phase_transition_kind,
                    summary,
                    suspect_area,
                    cause: finding_rule.cause.clone(),
                    supporting_fragments: finding_rule.supporting_fragments.to_vec(),
                    evidence_trace,
                })
            })
        })
        .collect()
}

pub(super) fn summarize_module_findings(program_findings: &[ProgramFinding]) -> Vec<ModuleFinding> {
    let mut grouped = BTreeMap::<
        (&str, Option<&ProcessView>, &crate::flow::ProgramOperation),
        ModuleFinding,
    >::new();

    for finding in program_findings {
        let key = (
            finding.module_label.as_str(),
            finding.process.as_ref(),
            &finding.operation,
        );
        let entry = grouped.entry(key).or_insert_with(|| ModuleFinding {
            module_label: finding.module_label.clone(),
            process: finding.process.clone(),
            operation: finding.operation.clone(),
            severity: ModuleSeverity::Low,
            network_module_kinds: Vec::new(),
            phases: Vec::new(),
            phase_kinds: Vec::new(),
            phase_transitions: Vec::new(),
            phase_transition_kinds: Vec::new(),
            suspect_areas: Vec::new(),
            causes: Vec::new(),
            supporting_fragments: Vec::new(),
            program_flows: Vec::new(),
            summaries: Vec::new(),
            evidence_trace: Vec::new(),
        });

        if let Some(phase) = &finding.phase {
            entry.phases.push(phase.clone());
        }
        if let Some(phase_kind) = &finding.phase_kind {
            entry.phase_kinds.push(phase_kind.clone());
        }
        if let Some(transition) = &finding.phase_transition {
            entry.phase_transitions.push(transition.clone());
        }
        if let Some(transition_kind) = &finding.phase_transition_kind {
            entry.phase_transition_kinds.push(transition_kind.clone());
        }
        entry
            .network_module_kinds
            .push(finding.network_module_kind.clone());
        entry.suspect_areas.push(finding.suspect_area.clone());
        entry.causes.push(finding.cause.clone());
        entry
            .supporting_fragments
            .extend(finding.supporting_fragments.iter().cloned());
        entry.program_flows.push(finding.program_flow);
        entry.summaries.push(finding.summary.clone());
        entry
            .evidence_trace
            .extend(finding.evidence_trace.iter().cloned());
    }

    let mut findings = grouped
        .into_values()
        .map(|mut finding| {
            finding.suspect_areas.sort();
            finding.suspect_areas.dedup();
            finding.phases.sort();
            finding.phases.dedup();
            finding.network_module_kinds.sort();
            finding.network_module_kinds.dedup();
            finding.phase_kinds.sort();
            finding.phase_kinds.dedup();
            finding.phase_transitions.sort();
            finding.phase_transitions.dedup();
            finding.phase_transition_kinds.sort();
            finding.phase_transition_kinds.dedup();
            finding.causes.sort_by_key(|cause| match cause {
                ProgramFindingCause::AttachFailure => 0,
                ProgramFindingCause::RejectedEvidence => 1,
                ProgramFindingCause::MissingCoreStage => 2,
            });
            finding.causes.dedup();
            finding.supporting_fragments.sort();
            finding.supporting_fragments.dedup();
            finding.program_flows.sort();
            finding.program_flows.dedup();
            finding.summaries.sort();
            finding.summaries.dedup();
            finding.evidence_trace.sort();
            finding.evidence_trace.dedup();
            finding.severity = module_severity(&finding.causes);
            finding
        })
        .collect::<Vec<_>>();

    findings.sort_by(|a, b| {
        severity_rank(&a.severity)
            .cmp(&severity_rank(&b.severity))
            .then_with(|| a.module_label.cmp(&b.module_label))
    });
    findings
}

fn finding_summary(
    flow: &ProgramFlow,
    phase: Option<&str>,
    phase_transition: Option<&str>,
    suspect_area: &str,
    cause: &ProgramFindingCause,
) -> String {
    let scope = flow.process.as_ref().map_or_else(
        || format!("program flow {}", flow.id.0),
        |process| format!("process {} (pid={})", process.comm, process.pid),
    );
    let cause_text = match cause {
        ProgramFindingCause::AttachFailure => "attach failure blocked required evidence",
        ProgramFindingCause::RejectedEvidence => "required evidence was rejected during ingest",
        ProgramFindingCause::MissingCoreStage => "required runtime evidence never materialized",
    };
    let phase_scope = phase
        .map(|phase| format!(" during {phase} phase"))
        .unwrap_or_default();
    let transition_scope = phase_transition
        .map(|transition| format!(" around {transition}"))
        .unwrap_or_default();
    format!(
        "{} may have a {} issue during {:?}{}{}: {}",
        scope, suspect_area, flow.operation, phase_scope, transition_scope, cause_text
    )
}

fn phase_transition_for_rule(
    model: &crate::program::ProgramModel,
    rule_index: usize,
    flow: &ProgramFlow,
) -> (Option<String>, Option<String>) {
    let Some(rule) = model.rules.get(rule_index) else {
        return (None, None);
    };
    let Some(current_phase) = rule.phase.as_ref() else {
        return (None, None);
    };
    let current_module = rule.module.as_deref();
    let Some(current_signal) = rule.signal.as_ref() else {
        return (None, None);
    };
    let previous_rule = model.rules[..rule_index]
        .iter()
        .filter(|rule| rule.phase.is_some() && rule.module.as_deref() == current_module)
        .filter_map(|rule| {
            let signal = rule.signal.as_ref()?;
            flow.stages
                .iter()
                .any(|stage| &stage.kind == signal && stage.phase == rule.phase)
                .then_some((rule.phase.as_deref(), signal))
        })
        .next_back();

    let phase_transition = Some(match previous_rule {
        Some((Some(previous), _)) => format!("{previous}->{current_phase}"),
        _ => format!("start->{current_phase}"),
    });
    let phase_transition_kind = Some(render_phase_transition_kind(
        previous_rule.map(|(previous_phase, previous_signal)| (previous_signal, previous_phase)),
        (current_signal, Some(current_phase)),
    ));

    (phase_transition, phase_transition_kind)
}

fn prior_phase_requirements_satisfied(
    model: &crate::program::ProgramModel,
    rule_index: usize,
    flow: &ProgramFlow,
) -> bool {
    let rule = match model.rules.get(rule_index) {
        Some(rule) => rule,
        None => return false,
    };
    let current_module = rule.module.as_deref();
    let prior_rule = model.rules[..rule_index].iter().rev().find(|candidate| {
        candidate.phase.is_some() && candidate.module.as_deref() == current_module
    });
    let Some(prior_rule) = prior_rule else {
        return true;
    };
    let Some(signal) = prior_rule.signal.as_ref() else {
        return true;
    };
    flow.stages
        .iter()
        .any(|stage| &stage.kind == signal && stage.phase == prior_rule.phase)
}

fn module_label(
    declared_module: Option<&str>,
    operation: &crate::flow::ProgramOperation,
    suspect_area: &str,
    supporting_fragments: &[String],
) -> String {
    if let Some(module) = declared_module {
        return module.to_string();
    }
    let fragment_scope = if supporting_fragments.is_empty() {
        "unknown_fragment".to_string()
    } else {
        supporting_fragments.join("+")
    };
    format!(
        "{}::{}::{}",
        operation_label(operation),
        suspect_area,
        fragment_scope
    )
}

fn operation_label(operation: &crate::flow::ProgramOperation) -> &str {
    match operation {
        crate::flow::ProgramOperation::ConnectFlow => "connect_flow",
        crate::flow::ProgramOperation::DatagramExchange => "datagram_exchange",
        crate::flow::ProgramOperation::Custom(value) => value,
        crate::flow::ProgramOperation::Unknown => "unknown",
    }
}

fn build_evidence_trace(
    signal: &crate::ir::SignalKind,
    flow: &ProgramFlow,
    external_evidence: &[String],
) -> Vec<String> {
    let mut trace = Vec::with_capacity(1 + flow.stages.len() + external_evidence.len());
    trace.push(format!("missing_signal:{}", signal.id()));

    for stage in &flow.stages {
        trace.push(match &stage.phase {
            Some(phase) => format!(
                "observed_stage:{}:{}@{}",
                phase,
                stage.kind.id(),
                stage.at.0
            ),
            None => format!("observed_stage:{}@{}", stage.kind.id(), stage.at.0),
        });
    }

    trace.extend(external_evidence.iter().cloned());

    trace
}

fn module_severity(causes: &[ProgramFindingCause]) -> ModuleSeverity {
    if causes.contains(&ProgramFindingCause::AttachFailure) {
        return ModuleSeverity::High;
    }
    if causes.contains(&ProgramFindingCause::RejectedEvidence) {
        return ModuleSeverity::Medium;
    }
    ModuleSeverity::Low
}

fn severity_rank(severity: &ModuleSeverity) -> u8 {
    match severity {
        ModuleSeverity::High => 0,
        ModuleSeverity::Medium => 1,
        ModuleSeverity::Low => 2,
    }
}
