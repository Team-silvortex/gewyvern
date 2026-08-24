use gewyvern::export::ExportBundle;
use std::collections::HashSet;

use crate::external_analysis::append_external_augmentations;
use crate::render_utils::*;

mod labels;
mod profiles;
mod render;
mod status;

#[cfg(test)]
pub(crate) use self::labels::{
    failure_basis_label, failure_confidence_label, failure_detail_label, failure_mode_label,
};
pub(crate) use self::labels::{
    failure_detail_family_label, failure_labels, failure_mode_family_label, first_non_none,
    first_or_none, module_family_label, reduce_confidence_level, stage_family_label,
};
use self::profiles::{
    primary_process_profile_from_profiles, process_network_profile_summaries_from_flow_summaries,
    protocol_flow_analysis_summaries,
};
#[cfg(test)]
pub(crate) use self::render::process_network_profiles_json;
pub(crate) use self::render::{
    analysis_snapshot_json, append_analysis_augmentations_json, append_analysis_snapshot_json,
    append_external_sidecar_context_json, append_external_sidecar_contract_json,
    append_process_network_profiles_json_from_snapshot,
    append_process_network_profiles_text_from_snapshot,
    append_protocol_flow_summaries_json_from_snapshot, append_protocol_flow_summaries_json_limited,
    append_protocol_flow_summaries_text_limited, estimate_analysis_snapshot_json_capacity,
    process_network_profiles_text_from_snapshot, protocol_flow_summaries_text_from_snapshot,
};
use self::status::protocol_flow_stage_summary;
pub(crate) use self::status::{
    ExternalSidecarContractState, ScanTargetStatus, analysis_augmentation_names_text,
    analysis_automation_outcome, analysis_evidence_posture, external_capability_summary,
    external_sidecar_consumption_mode, external_sidecar_consumption_mode_for,
    external_sidecar_contract_state, external_sidecar_item_consumption_mode,
    external_sidecar_presence, external_sidecar_trust_level, scan_target_status,
};

#[derive(Default)]
pub(super) struct ProtocolFlowFindingSummary {
    pub(super) has_findings: bool,
    pub(super) missing_transitions: Vec<String>,
    pub(super) network_module_kinds: Vec<String>,
    pub(super) suspect_areas: Vec<String>,
}

#[derive(Default)]
struct ProtocolFlowFindingAccumulator<'a> {
    summary: ProtocolFlowFindingSummary,
    seen_missing_transitions: HashSet<&'a str>,
    seen_network_module_kinds: HashSet<&'a str>,
    seen_suspect_areas: HashSet<&'a str>,
}

#[derive(Clone, Default)]
pub(crate) struct ProtocolFlowAnalysisSummary {
    pub(crate) program_flow: u64,
    pub(crate) process: Option<gewyvern::flow::ProcessView>,
    pub(crate) operation: String,
    pub(crate) network_module_kind: String,
    pub(crate) network_module_kinds: Vec<String>,
    pub(crate) status: String,
    pub(crate) failure_mode: String,
    pub(crate) failure_detail: String,
    pub(crate) failure_confidence: String,
    pub(crate) failure_basis: String,
    pub(crate) phases: Vec<String>,
    pub(crate) last_phase: Option<String>,
    pub(crate) missing_transitions: Vec<String>,
    pub(crate) suspect_areas: Vec<String>,
}

#[derive(Clone, Default)]
pub(crate) struct ProcessNetworkProfileSummary {
    pub(crate) pid: u32,
    pub(crate) comm: String,
    pub(crate) status: String,
    pub(crate) primary_module_kind: String,
    pub(crate) primary_module_family: String,
    pub(crate) primary_failure_stage: String,
    pub(crate) primary_stage_family: String,
    pub(crate) primary_failure_mode: String,
    pub(crate) primary_failure_detail: String,
    pub(crate) primary_failure_confidence: String,
    pub(crate) primary_failure_basis: String,
    pub(crate) ambiguous: bool,
    pub(crate) competing_hypotheses: Vec<String>,
    pub(crate) operations: Vec<String>,
    pub(crate) module_kinds: Vec<String>,
    pub(crate) phases: Vec<String>,
    pub(crate) missing_transitions: Vec<String>,
    pub(crate) suspect_areas: Vec<String>,
    pub(crate) suspect_modules: Vec<String>,
    pub(crate) healthy_flows: usize,
    pub(crate) attention_flows: usize,
}

#[derive(Default)]
struct ProcessNetworkProfileAccumulator {
    summary: ProcessNetworkProfileSummary,
    seen_operations: HashSet<String>,
    seen_module_kinds: HashSet<String>,
    seen_phases: HashSet<String>,
    seen_missing_transitions: HashSet<String>,
    seen_suspect_areas: HashSet<String>,
    seen_suspect_modules: HashSet<String>,
    module_scores: std::collections::HashMap<String, u32>,
    stage_scores: std::collections::HashMap<String, u32>,
    suspect_module_scores: std::collections::HashMap<String, u32>,
}

#[derive(Clone, Default)]
pub(crate) struct AnalysisSnapshot {
    pub(crate) target_status: ScanTargetStatus,
    pub(crate) primary_process_profile: Option<ProcessNetworkProfileSummary>,
    pub(crate) primary_module_kind: String,
    pub(crate) primary_module_family: String,
    pub(crate) primary_failure_stage: String,
    pub(crate) primary_failure_mode: String,
    pub(crate) primary_failure_detail: String,
    pub(crate) primary_failure_confidence: String,
    pub(crate) primary_failure_basis: String,
    pub(crate) evidence_posture: String,
    pub(crate) automation_outcome: String,
    pub(crate) operator_guidance_status: String,
    pub(crate) operator_guidance_action: String,
    pub(crate) operator_guidance_reason: String,
    pub(crate) operator_guidance_summary: String,
    pub(crate) primary_process_profile_ambiguous: bool,
    pub(crate) competing_hypotheses: Vec<String>,
    pub(crate) operations: Vec<String>,
    pub(crate) phases: Vec<String>,
    pub(crate) missing_transitions: Vec<String>,
    pub(crate) suspect_areas: Vec<String>,
    pub(crate) suspect_modules: Vec<String>,
    pub(crate) augmentations: Vec<AnalysisAugmentation>,
    pub(crate) process_profiles: Vec<ProcessNetworkProfileSummary>,
    pub(crate) protocol_flows: Vec<ProtocolFlowAnalysisSummary>,
}

#[derive(Clone, Default)]
pub(crate) struct AnalysisAugmentation {
    pub(crate) kind: String,
    pub(crate) name: String,
    pub(crate) summary: String,
    pub(crate) confidence: String,
    pub(crate) producer_stage: Option<String>,
    pub(crate) producer_pass: Option<String>,
    pub(crate) data_json: Option<String>,
}

pub(crate) trait AnalysisAugmenter {
    fn augment(&self, export: &ExportBundle, snapshot: &mut AnalysisSnapshot);
}

struct BuiltInAdvisoryAugmenter;
struct BuiltInRecommendationAugmenter;

fn built_in_operator_guidance(
    snapshot: &AnalysisSnapshot,
) -> (&'static str, &'static str, &'static str, &'static str) {
    if snapshot
        .augmentations
        .iter()
        .any(|item| item.name == "unverified_ingest_lineage")
    {
        (
            "advisory_only",
            "avoid_pid_strong_actions",
            "unverified_ingest_lineage",
            "avoid strong pid-scoped automation until lineage can be verified",
        )
    } else if snapshot.primary_process_profile_ambiguous
        && snapshot
            .augmentations
            .iter()
            .any(|item| item.name == "competing_hypotheses")
    {
        (
            "ambiguous",
            "keep_multiple_hypotheses",
            "competing_hypotheses",
            "preserve multiple hypotheses and avoid collapsing to a single remediation path",
        )
    } else if snapshot.primary_failure_confidence == "high"
        && snapshot.primary_failure_basis == "direct_protocol_signal"
    {
        (
            "targeted_ready",
            "safe_to_escalate_protocol_signal",
            "direct_protocol_signal",
            "direct protocol evidence is strong enough for targeted downstream escalation",
        )
    } else if snapshot.primary_failure_confidence == "medium"
        && snapshot.primary_failure_basis == "missing_transition"
    {
        (
            "observe_more",
            "collect_more_runtime_evidence",
            "missing_transition",
            "collect another observation window before taking a strong automated action",
        )
    } else {
        (
            "manual_review",
            "manual_review",
            "heuristic_summary",
            "fall back to a human-oriented review path because the current signal is advisory",
        )
    }
}

impl AnalysisAugmenter for BuiltInAdvisoryAugmenter {
    fn augment(&self, export: &ExportBundle, snapshot: &mut AnalysisSnapshot) {
        if matches!(
            export.ingest_trust_mode.as_str(),
            "unverified-local" | "unverified-remote"
        ) {
            push_analysis_augmentation(
                snapshot,
                "trust",
                "unverified_ingest_lineage",
                "pid-scoped conclusions are advisory because ingest lineage is unverified",
                "advisory",
                Some("advisory".into()),
                Some("BuiltInAdvisoryAugmenter".into()),
                Some(format!(
                    "{{\"ingest_trust_mode\":\"{}\",\"pid_attribution_status\":\"unverified\"}}",
                    export.ingest_trust_mode
                )),
            );
        }

        if snapshot.primary_process_profile_ambiguous && !snapshot.competing_hypotheses.is_empty() {
            push_analysis_augmentation(
                snapshot,
                "analysis",
                "competing_hypotheses",
                "multiple plausible hypotheses remain; downstream automation should treat the primary conclusion as advisory",
                "advisory",
                Some("advisory".into()),
                Some("BuiltInAdvisoryAugmenter".into()),
                Some(format!(
                    "{{\"primary_module_kind\":\"{}\",\"primary_failure_confidence\":\"{}\",\"competing_hypotheses\":{}}}",
                    snapshot.primary_module_kind,
                    snapshot.primary_failure_confidence,
                    string_list_json(&snapshot.competing_hypotheses)
                )),
            );
        }
    }
}

impl AnalysisAugmenter for BuiltInRecommendationAugmenter {
    fn augment(&self, _export: &ExportBundle, snapshot: &mut AnalysisSnapshot) {
        let (_, action, reason, summary) = built_in_operator_guidance(snapshot);

        push_analysis_augmentation(
            snapshot,
            "recommendation",
            "automation_recommendation",
            summary,
            "advisory",
            Some("recommendation".into()),
            Some("BuiltInRecommendationAugmenter".into()),
            Some(format!(
                "{{\"action\":\"{}\",\"reason\":\"{}\",\"primary_failure_confidence\":\"{}\",\"primary_failure_basis\":\"{}\",\"ambiguous\":{}}}",
                action,
                reason,
                snapshot.primary_failure_confidence,
                snapshot.primary_failure_basis,
                snapshot.primary_process_profile_ambiguous
            )),
        );
    }
}

// Kept as an explicit extension helper so future rule-based or ML passes can
// append machine-readable annotations without re-shaping the core snapshot.
#[allow(dead_code)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn push_analysis_augmentation(
    snapshot: &mut AnalysisSnapshot,
    kind: impl Into<String>,
    name: impl Into<String>,
    summary: impl Into<String>,
    confidence: impl Into<String>,
    producer_stage: Option<String>,
    producer_pass: Option<String>,
    data_json: Option<String>,
) {
    snapshot.augmentations.push(AnalysisAugmentation {
        kind: kind.into(),
        name: name.into(),
        summary: summary.into(),
        confidence: confidence.into(),
        producer_stage,
        producer_pass,
        data_json,
    });
}

pub(crate) fn analysis_snapshot(export: &ExportBundle) -> AnalysisSnapshot {
    analysis_snapshot_with_augmenters(export, &[])
}

pub(crate) fn analysis_snapshot_with_augmenters(
    export: &ExportBundle,
    augmenters: &[&dyn AnalysisAugmenter],
) -> AnalysisSnapshot {
    let built_in = BuiltInAdvisoryAugmenter;
    let recommendation = BuiltInRecommendationAugmenter;
    let mut all_augmenters = vec![
        &built_in as &dyn AnalysisAugmenter,
        &recommendation as &dyn AnalysisAugmenter,
    ];
    all_augmenters.extend_from_slice(augmenters);
    analysis_snapshot_with(export, &all_augmenters)
}

pub(crate) fn analysis_snapshot_with(
    export: &ExportBundle,
    augmenters: &[&dyn AnalysisAugmenter],
) -> AnalysisSnapshot {
    let protocol_flows = protocol_flow_analysis_summaries(export);
    let process_profiles =
        process_network_profile_summaries_from_flow_summaries(export, &protocol_flows);
    let primary_process_profile = primary_process_profile_from_profiles(&process_profiles).cloned();
    let target_status = scan_target_status(export);
    let primary_module_kind = if let Some(profile) = primary_process_profile.as_ref() {
        profile.primary_module_kind.clone()
    } else if let Some(finding) = export.program_findings.first() {
        finding.network_module_kind.clone()
    } else {
        protocol_flows
            .first()
            .map(|flow| flow.network_module_kind.clone())
            .unwrap_or_else(|| "none".into())
    };
    let primary_failure_stage = if let Some(profile) = primary_process_profile.as_ref() {
        profile.primary_failure_stage.clone()
    } else if let Some(finding) = export.program_findings.first() {
        finding
            .phase_transition
            .clone()
            .or_else(|| finding.phase.clone())
            .unwrap_or_else(|| "none".into())
    } else {
        protocol_flows
            .iter()
            .filter_map(|flow| flow.last_phase.clone())
            .next_back()
            .unwrap_or_else(|| "none".into())
    };
    let suspect_areas = export
        .program_findings
        .iter()
        .map(|finding| finding.suspect_area.clone())
        .collect::<Vec<_>>();
    let (
        primary_failure_mode,
        primary_failure_detail,
        primary_failure_confidence,
        primary_failure_basis,
    ) = if let Some(profile) = primary_process_profile.as_ref() {
        (
            profile.primary_failure_mode.clone(),
            profile.primary_failure_detail.clone(),
            profile.primary_failure_confidence.clone(),
            profile.primary_failure_basis.clone(),
        )
    } else {
        let labels = failure_labels(
            target_status.label(),
            &primary_module_kind,
            &primary_failure_stage,
            &suspect_areas,
        );
        (
            labels.mode.to_string(),
            labels.detail.to_string(),
            labels.confidence.to_string(),
            labels.basis.to_string(),
        )
    };
    let primary_module_family = module_family_label(&primary_module_kind).to_string();
    let suspect_modules = if let Some(profile) = primary_process_profile.as_ref() {
        profile.suspect_modules.clone()
    } else {
        export
            .program_findings
            .iter()
            .map(|finding| finding.module_label.clone())
            .collect()
    };
    let competing_hypotheses = primary_process_profile
        .as_ref()
        .map(|profile| profile.competing_hypotheses.clone())
        .unwrap_or_default();
    let operations = if let Some(profile) = primary_process_profile.as_ref() {
        profile.operations.clone()
    } else {
        collect_unique_flow_strings(&protocol_flows, |flow| {
            std::slice::from_ref(&flow.operation)
        })
    };
    let phases = if let Some(profile) = primary_process_profile.as_ref() {
        profile.phases.clone()
    } else {
        collect_unique_flow_strings(&protocol_flows, |flow| &flow.phases)
    };
    let missing_transitions = if let Some(profile) = primary_process_profile.as_ref() {
        profile.missing_transitions.clone()
    } else {
        collect_unique_flow_strings(&protocol_flows, |flow| &flow.missing_transitions)
    };
    let suspect_areas = if let Some(profile) = primary_process_profile.as_ref() {
        profile.suspect_areas.clone()
    } else {
        collect_unique_flow_strings(&protocol_flows, |flow| &flow.suspect_areas)
    };
    let mut snapshot = AnalysisSnapshot {
        target_status,
        primary_process_profile_ambiguous: primary_process_profile
            .as_ref()
            .map(|profile| profile.ambiguous)
            .unwrap_or(false),
        primary_module_kind,
        primary_module_family,
        primary_failure_stage,
        primary_failure_mode,
        primary_failure_detail,
        primary_failure_confidence,
        primary_failure_basis,
        evidence_posture: String::new(),
        automation_outcome: String::new(),
        operator_guidance_status: String::new(),
        operator_guidance_action: String::new(),
        operator_guidance_reason: String::new(),
        operator_guidance_summary: String::new(),
        competing_hypotheses,
        operations,
        phases,
        missing_transitions,
        suspect_areas,
        suspect_modules,
        augmentations: Vec::new(),
        primary_process_profile,
        process_profiles,
        protocol_flows,
    };
    for augmenter in augmenters {
        augmenter.augment(export, &mut snapshot);
    }
    let (guidance_status, guidance_action, guidance_reason, guidance_summary) =
        built_in_operator_guidance(&snapshot);
    snapshot.operator_guidance_status = guidance_status.into();
    snapshot.operator_guidance_action = guidance_action.into();
    snapshot.operator_guidance_reason = guidance_reason.into();
    snapshot.operator_guidance_summary = guidance_summary.into();
    snapshot.evidence_posture = analysis_evidence_posture(export, &snapshot).into();
    snapshot.automation_outcome = analysis_automation_outcome(export, &snapshot).into();
    append_external_augmentations(&mut snapshot);
    snapshot
}

fn collect_unique_flow_strings(
    flows: &[ProtocolFlowAnalysisSummary],
    selector: impl Fn(&ProtocolFlowAnalysisSummary) -> &[String],
) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut values = Vec::new();
    for flow in flows {
        for value in selector(flow) {
            if seen.insert(value.clone()) {
                values.push(value.clone());
            }
        }
    }
    values
}

#[cfg(test)]
pub(super) fn primary_module_kind_for_export(export: &ExportBundle) -> String {
    analysis_snapshot(export).primary_module_kind
}

#[cfg(test)]
pub(super) fn primary_failure_stage_for_export(export: &ExportBundle) -> String {
    analysis_snapshot(export).primary_failure_stage
}

#[cfg(test)]
pub(super) fn primary_failure_mode_for_export(export: &ExportBundle) -> String {
    analysis_snapshot(export).primary_failure_mode
}

#[cfg(test)]
pub(super) fn primary_failure_detail_for_export(export: &ExportBundle) -> String {
    analysis_snapshot(export).primary_failure_detail
}

#[cfg(test)]
pub(super) fn suspect_modules_for_export(export: &ExportBundle) -> String {
    let snapshot = analysis_snapshot(export);
    if !snapshot.suspect_modules.is_empty() {
        return snapshot.suspect_modules.join(" | ");
    }
    if export.program_findings.is_empty() {
        "none".into()
    } else {
        export
            .program_findings
            .iter()
            .map(|finding| finding.module_label.clone())
            .collect::<Vec<_>>()
            .join(" | ")
    }
}
