use std::collections::HashMap;

use gewyvern::export::ExportBundle;
use gewyvern::flow::ProgramFlowId;

use super::{
    ProcessNetworkProfileAccumulator, ProcessNetworkProfileSummary, ProtocolFlowAnalysisSummary,
    ProtocolFlowFindingAccumulator, ProtocolFlowFindingSummary, failure_basis_label,
    failure_confidence_label, failure_detail_label, failure_mode_label, first_non_none,
    module_family_label, protocol_flow_has_terminal_failure, protocol_flow_last_phase,
    protocol_flow_phases, reduce_confidence_level, stage_family_label,
};

fn bump_profile_score(scores: &mut HashMap<String, u32>, value: &str, weight: u32) {
    if value == "none" {
        return;
    }
    *scores.entry(value.to_string()).or_default() += weight;
}

fn best_profile_score(scores: &HashMap<String, u32>) -> Option<String> {
    scores
        .iter()
        .max_by(|(left_value, left_score), (right_value, right_score)| {
            left_score
                .cmp(right_score)
                .then_with(|| right_value.cmp(left_value))
        })
        .map(|(value, _)| value.clone())
}

pub(super) fn protocol_flow_finding_summaries(
    export: &ExportBundle,
) -> HashMap<ProgramFlowId, ProtocolFlowFindingSummary> {
    let mut summaries = HashMap::<ProgramFlowId, ProtocolFlowFindingAccumulator>::new();
    for finding in &export.program_findings {
        let entry = summaries.entry(finding.program_flow).or_default();
        entry.summary.has_findings = true;
        if let Some(transition) = &finding.phase_transition {
            if entry.seen_missing_transitions.insert(transition.clone()) {
                entry.summary.missing_transitions.push(transition.clone());
            }
        }
        if entry
            .seen_network_module_kinds
            .insert(finding.network_module_kind.clone())
        {
            entry
                .summary
                .network_module_kinds
                .push(finding.network_module_kind.clone());
        }
        if entry
            .seen_suspect_areas
            .insert(finding.suspect_area.clone())
        {
            entry
                .summary
                .suspect_areas
                .push(finding.suspect_area.clone());
        }
    }
    summaries
        .into_iter()
        .map(|(flow_id, accumulator)| (flow_id, accumulator.summary))
        .collect()
}

fn protocol_flow_analysis_summary(
    flow: &gewyvern::flow::ProgramFlow,
    finding_summary: Option<&ProtocolFlowFindingSummary>,
) -> ProtocolFlowAnalysisSummary {
    let phases = protocol_flow_phases(flow);
    let last_phase = protocol_flow_last_phase(flow);
    let network_module_kind = gewyvern::flow::infer_network_module_kind(
        &flow.operation,
        last_phase.as_deref(),
        None,
        "network_module",
    )
    .to_string();
    let status = if finding_summary.is_some_and(|summary| summary.has_findings)
        || protocol_flow_has_terminal_failure(flow)
    {
        "attention"
    } else {
        "healthy"
    };
    let missing_transitions = finding_summary
        .map(|summary| summary.missing_transitions.clone())
        .unwrap_or_default();
    let network_module_kinds = finding_summary
        .map(|summary| summary.network_module_kinds.clone())
        .filter(|items| !items.is_empty())
        .unwrap_or_else(|| vec![network_module_kind.clone()]);
    let suspect_areas = finding_summary
        .map(|summary| summary.suspect_areas.clone())
        .unwrap_or_default();
    let primary_stage = missing_transitions
        .first()
        .cloned()
        .or_else(|| last_phase.clone())
        .unwrap_or_else(|| "none".into());
    let failure_mode =
        failure_mode_label(status, &network_module_kind, &primary_stage, &suspect_areas)
            .to_string();
    let failure_detail =
        failure_detail_label(status, &network_module_kind, &primary_stage, &suspect_areas)
            .to_string();
    let failure_confidence =
        failure_confidence_label(status, &network_module_kind, &primary_stage, &suspect_areas)
            .to_string();
    let failure_basis =
        failure_basis_label(status, &network_module_kind, &primary_stage, &suspect_areas)
            .to_string();
    ProtocolFlowAnalysisSummary {
        program_flow: flow.id.0,
        process: flow.process.clone(),
        operation: crate::render_utils::operation_label(&flow.operation),
        network_module_kind,
        network_module_kinds,
        status: status.to_string(),
        failure_mode,
        failure_detail,
        failure_confidence,
        failure_basis,
        phases,
        last_phase,
        missing_transitions,
        suspect_areas,
    }
}

pub(super) fn protocol_flow_analysis_summaries(
    export: &ExportBundle,
) -> Vec<ProtocolFlowAnalysisSummary> {
    let finding_summaries = protocol_flow_finding_summaries(export);
    export
        .program_flows
        .iter()
        .map(|flow| protocol_flow_analysis_summary(flow, finding_summaries.get(&flow.id)))
        .collect()
}

pub(super) fn process_network_profile_summaries_from_flow_summaries(
    export: &ExportBundle,
    protocol_flows: &[ProtocolFlowAnalysisSummary],
) -> Vec<ProcessNetworkProfileSummary> {
    let mut profiles = HashMap::<(u32, String), ProcessNetworkProfileAccumulator>::new();

    for flow in protocol_flows {
        let Some(process) = flow.process.as_ref() else {
            continue;
        };
        let key = (process.pid, process.comm.clone());
        let entry =
            profiles
                .entry(key.clone())
                .or_insert_with(|| ProcessNetworkProfileAccumulator {
                    summary: ProcessNetworkProfileSummary {
                        pid: process.pid,
                        comm: process.comm.clone(),
                        status: "idle".into(),
                        primary_module_kind: "none".into(),
                        primary_module_family: "general".into(),
                        primary_failure_stage: "none".into(),
                        primary_stage_family: "none".into(),
                        primary_failure_mode: "none".into(),
                        primary_failure_detail: "none".into(),
                        primary_failure_confidence: "none".into(),
                        primary_failure_basis: "none".into(),
                        ..Default::default()
                    },
                    ..Default::default()
                });
        let summary = &mut entry.summary;

        if entry.seen_operations.insert(flow.operation.clone()) {
            summary.operations.push(flow.operation.clone());
        }

        let inferred_kind = flow.network_module_kind.clone();
        let last_phase = flow.last_phase.clone().unwrap_or_else(|| "none".into());
        if entry.seen_module_kinds.insert(inferred_kind.clone()) {
            summary.module_kinds.push(inferred_kind.clone());
        }
        bump_profile_score(&mut entry.module_scores, &inferred_kind, 1);
        bump_profile_score(&mut entry.stage_scores, &last_phase, 1);

        for phase in &flow.phases {
            if entry.seen_phases.insert(phase.clone()) {
                summary.phases.push(phase.clone());
            }
        }

        if flow.status == "attention" {
            summary.attention_flows += 1;
            summary.status = "attention".into();
            if flow.network_module_kinds.is_empty() {
                bump_profile_score(&mut entry.module_scores, &inferred_kind, 10);
            } else {
                for module_kind in &flow.network_module_kinds {
                    bump_profile_score(&mut entry.module_scores, module_kind, 10);
                    if entry.seen_module_kinds.insert(module_kind.clone()) {
                        summary.module_kinds.push(module_kind.clone());
                    }
                }
            }
            if flow.missing_transitions.is_empty() {
                bump_profile_score(&mut entry.stage_scores, &last_phase, 10);
            } else {
                for transition in &flow.missing_transitions {
                    bump_profile_score(&mut entry.stage_scores, transition, 10);
                    if entry.seen_missing_transitions.insert(transition.clone()) {
                        summary.missing_transitions.push(transition.clone());
                    }
                }
            }
            for suspect_area in &flow.suspect_areas {
                if entry.seen_suspect_areas.insert(suspect_area.clone()) {
                    summary.suspect_areas.push(suspect_area.clone());
                }
            }
        } else {
            summary.healthy_flows += 1;
            if summary.status != "attention" {
                summary.status = "healthy".into();
            }
        }
    }

    for finding in &export.program_findings {
        let Some(process) = finding.process.as_ref() else {
            continue;
        };
        let key = (process.pid, process.comm.clone());
        let entry =
            profiles
                .entry(key.clone())
                .or_insert_with(|| ProcessNetworkProfileAccumulator {
                    summary: ProcessNetworkProfileSummary {
                        pid: process.pid,
                        comm: process.comm.clone(),
                        status: "idle".into(),
                        primary_module_kind: "none".into(),
                        primary_module_family: "general".into(),
                        primary_failure_stage: "none".into(),
                        primary_stage_family: "none".into(),
                        primary_failure_mode: "none".into(),
                        primary_failure_detail: "none".into(),
                        primary_failure_confidence: "none".into(),
                        primary_failure_basis: "none".into(),
                        ..Default::default()
                    },
                    ..Default::default()
                });
        let summary = &mut entry.summary;
        summary.status = "attention".into();
        if entry
            .seen_module_kinds
            .insert(finding.network_module_kind.clone())
        {
            summary
                .module_kinds
                .push(finding.network_module_kind.clone());
        }
        if entry
            .seen_suspect_areas
            .insert(finding.suspect_area.clone())
        {
            summary.suspect_areas.push(finding.suspect_area.clone());
        }
        if entry
            .seen_suspect_modules
            .insert(finding.module_label.clone())
        {
            summary.suspect_modules.push(finding.module_label.clone());
        }
        bump_profile_score(&mut entry.module_scores, &finding.network_module_kind, 20);
        if let Some(phase) = &finding.phase {
            bump_profile_score(&mut entry.stage_scores, phase, 20);
        }
        if let Some(transition) = &finding.phase_transition {
            bump_profile_score(&mut entry.stage_scores, transition, 25);
        }
        bump_profile_score(&mut entry.suspect_module_scores, &finding.module_label, 20);
    }

    let mut profiles = profiles
        .into_values()
        .map(|accumulator| {
            (
                accumulator.summary,
                accumulator.module_scores,
                accumulator.stage_scores,
                accumulator.suspect_module_scores,
            )
        })
        .collect::<Vec<_>>();
    for (profile, module_scores, stage_scores, suspect_module_scores) in &mut profiles {
        profile.operations.sort();
        profile.module_kinds.sort();
        profile.phases.sort();
        profile.missing_transitions.sort();
        profile.suspect_areas.sort();
        profile.suspect_modules.sort();
        profile.primary_module_kind = best_profile_score(module_scores)
            .or_else(|| first_non_none(&profile.module_kinds))
            .unwrap_or_else(|| "none".into());
        profile.primary_module_family =
            module_family_label(&profile.primary_module_kind).to_string();
        profile.primary_failure_stage =
            if profile.status == "attention" && !profile.missing_transitions.is_empty() {
                best_profile_score(stage_scores)
                    .filter(|stage| profile.missing_transitions.contains(stage))
                    .or_else(|| first_non_none(&profile.missing_transitions))
                    .unwrap_or_else(|| "none".into())
            } else {
                best_profile_score(stage_scores)
                    .or_else(|| first_non_none(&profile.missing_transitions))
                    .or_else(|| first_non_none(&profile.phases))
                    .unwrap_or_else(|| "none".into())
            };
        profile.primary_stage_family =
            stage_family_label(&profile.primary_failure_stage).to_string();
        profile.primary_failure_mode = failure_mode_label(
            &profile.status,
            &profile.primary_module_kind,
            &profile.primary_failure_stage,
            &profile.suspect_areas,
        )
        .to_string();
        profile.primary_failure_detail = failure_detail_label(
            &profile.status,
            &profile.primary_module_kind,
            &profile.primary_failure_stage,
            &profile.suspect_areas,
        )
        .to_string();
        let mut confidence = failure_confidence_label(
            &profile.status,
            &profile.primary_module_kind,
            &profile.primary_failure_stage,
            &profile.suspect_areas,
        );
        let basis = failure_basis_label(
            &profile.status,
            &profile.primary_module_kind,
            &profile.primary_failure_stage,
            &profile.suspect_areas,
        );
        let ambiguity_signals = usize::from(profile.module_kinds.len() > 1)
            + usize::from(profile.missing_transitions.len() > 1);
        if ambiguity_signals > 0 {
            confidence = reduce_confidence_level(confidence);
        }
        if ambiguity_signals > 1 {
            confidence = reduce_confidence_level(confidence);
        }
        profile.ambiguous = profile.module_kinds.len() > 1 || profile.missing_transitions.len() > 1;
        let mut competing_hypotheses = Vec::new();
        competing_hypotheses.extend(
            profile
                .module_kinds
                .iter()
                .filter(|kind| kind.as_str() != profile.primary_module_kind.as_str())
                .map(|kind| format!("module:{kind}")),
        );
        competing_hypotheses.extend(
            profile
                .missing_transitions
                .iter()
                .filter(|transition| transition.as_str() != profile.primary_failure_stage.as_str())
                .map(|transition| format!("transition:{transition}")),
        );
        competing_hypotheses.extend(
            profile
                .suspect_modules
                .iter()
                .skip(1)
                .map(|module| format!("suspect_module:{module}")),
        );
        competing_hypotheses.sort();
        competing_hypotheses.dedup();
        profile.competing_hypotheses = competing_hypotheses;
        profile.primary_failure_confidence = confidence.to_string();
        profile.primary_failure_basis = basis.to_string();
        if let Some(primary_suspect_module) = best_profile_score(suspect_module_scores) {
            if let Some(index) = profile
                .suspect_modules
                .iter()
                .position(|module| module == &primary_suspect_module)
            {
                let module = profile.suspect_modules.remove(index);
                profile.suspect_modules.insert(0, module);
            }
        }
    }
    profiles.sort_by(|(left, ..), (right, ..)| {
        left.pid
            .cmp(&right.pid)
            .then_with(|| left.comm.cmp(&right.comm))
    });
    profiles.into_iter().map(|(profile, ..)| profile).collect()
}

fn compare_process_profile_priority(
    left: &ProcessNetworkProfileSummary,
    right: &ProcessNetworkProfileSummary,
) -> std::cmp::Ordering {
    let left_rank = match left.status.as_str() {
        "attention" => 0,
        "healthy" => 1,
        _ => 2,
    };
    let right_rank = match right.status.as_str() {
        "attention" => 0,
        "healthy" => 1,
        _ => 2,
    };
    left_rank
        .cmp(&right_rank)
        .then_with(|| right.attention_flows.cmp(&left.attention_flows))
        .then_with(|| right.healthy_flows.cmp(&left.healthy_flows))
        .then_with(|| left.pid.cmp(&right.pid))
        .then_with(|| left.comm.cmp(&right.comm))
}

pub(super) fn primary_process_profile_from_profiles(
    profiles: &[ProcessNetworkProfileSummary],
) -> Option<&ProcessNetworkProfileSummary> {
    profiles
        .iter()
        .min_by(|left, right| compare_process_profile_priority(left, right))
}
