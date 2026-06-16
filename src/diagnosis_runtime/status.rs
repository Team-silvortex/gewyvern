use gewyvern::export::ExportBundle;

use super::{AnalysisAugmentation, AnalysisSnapshot};
use crate::UiLocale;

#[derive(Clone, Copy, Default)]
pub(crate) enum ScanTargetStatus {
    #[default]
    Idle,
    Healthy,
    Attention,
}

impl ScanTargetStatus {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Healthy => "healthy",
            Self::Attention => "attention",
        }
    }
}

pub(crate) fn protocol_flow_phases(flow: &gewyvern::flow::ProgramFlow) -> Vec<String> {
    let mut phases = Vec::new();
    for phase in flow.stages.iter().filter_map(|stage| stage.phase.as_ref()) {
        if phases.last() != Some(phase) {
            phases.push(phase.clone());
        }
    }
    phases
}

pub(crate) fn protocol_flow_last_phase(flow: &gewyvern::flow::ProgramFlow) -> Option<String> {
    flow.stages
        .iter()
        .rev()
        .find_map(|stage| stage.phase.clone())
}

fn terminal_failure_phase(phase: &str) -> bool {
    let lowered = phase.to_ascii_lowercase();
    lowered == "denied"
        || lowered == "auth_required"
        || lowered == "constraint"
        || lowered == "error"
        || lowered == "closed"
        || lowered.contains("denied")
        || lowered.contains("auth_required")
        || lowered.contains("constraint")
        || lowered.contains("error")
        || lowered.contains("close")
}

pub(crate) fn protocol_flow_has_terminal_failure(flow: &gewyvern::flow::ProgramFlow) -> bool {
    flow.stages
        .iter()
        .filter_map(|stage| stage.phase.as_deref())
        .any(terminal_failure_phase)
}

pub(crate) fn analysis_augmentation_names_text(items: &[AnalysisAugmentation]) -> String {
    if items.is_empty() {
        UiLocale::detect().none().to_string()
    } else {
        items
            .iter()
            .map(|item| {
                if let Some(pass) = item.producer_pass.as_deref() {
                    format!("{}@{}", item.name, pass)
                } else {
                    item.name.clone()
                }
            })
            .collect::<Vec<_>>()
            .join("|")
    }
}

pub(crate) fn external_sidecar_presence(snapshot: &AnalysisSnapshot) -> (bool, bool, bool) {
    let has_enrichment = snapshot
        .augmentations
        .iter()
        .any(|item| item.name == "external_evidence_chain_enrichment");
    let has_opinion = snapshot
        .augmentations
        .iter()
        .any(|item| item.name == "external_diagnostic_opinion");
    (has_enrichment || has_opinion, has_enrichment, has_opinion)
}

pub(crate) fn external_capability_summary(
    snapshot: &AnalysisSnapshot,
) -> (bool, Option<String>, Option<String>, Option<String>) {
    let Some(item) = snapshot
        .augmentations
        .iter()
        .find(|item| item.name == "external_capability_profile")
    else {
        return (false, None, None, None);
    };
    let compatibility_status = item.data_json.as_deref().and_then(|data| {
        crate::render_utils::extract_json_string_field(data, "compatibility_status")
    });
    let hint_status = item
        .data_json
        .as_deref()
        .and_then(|data| crate::render_utils::extract_json_string_field(data, "hint_status"));
    let context_status = item
        .data_json
        .as_deref()
        .and_then(|data| crate::render_utils::extract_json_string_field(data, "context_status"));
    (true, compatibility_status, hint_status, context_status)
}

pub(crate) fn external_sidecar_item_consumption_mode(
    item: &AnalysisAugmentation,
) -> Option<&'static str> {
    let merge_hint = item.data_json.as_deref().and_then(|data| {
        crate::render_utils::extract_json_string_field(data, "external_merge_hint")
    });
    match (item.name.as_str(), merge_hint.as_deref()) {
        ("external_evidence_chain_enrichment", Some("augmentations_only")) => Some("append_only"),
        ("external_evidence_chain_enrichment", Some("augmentations_and_guidance_context")) => {
            Some("guidance_context")
        }
        (
            "external_evidence_chain_enrichment",
            Some("augmentations_with_operator_guidance_support"),
        ) => Some("operator_guidance_support"),
        ("external_diagnostic_opinion", Some("sidecar_only_opinion")) => Some("operator_review"),
        ("external_diagnostic_opinion", Some("operator_guidance_candidate")) => {
            Some("guidance_candidate")
        }
        _ => None,
    }
}

pub(crate) fn external_sidecar_consumption_mode(snapshot: &AnalysisSnapshot) -> Option<String> {
    let mut best_rank = 0u8;
    let mut best_mode = None;
    for item in &snapshot.augmentations {
        let Some(mode) = external_sidecar_item_consumption_mode(item) else {
            continue;
        };
        let rank = match mode {
            "append_only" => 1,
            "guidance_context" => 2,
            "operator_review" => 3,
            "operator_guidance_support" => 4,
            "guidance_candidate" => 5,
            _ => 0,
        };
        if rank > best_rank {
            best_rank = rank;
            best_mode = Some(mode.to_string());
        }
    }
    best_mode
}

pub(crate) fn external_sidecar_trust_level(snapshot: &AnalysisSnapshot) -> Option<String> {
    let (has_profile, capability_status, hint_status, context_status) =
        external_capability_summary(snapshot);
    let (has_sidecar_context, _, _) = external_sidecar_presence(snapshot);
    if !has_sidecar_context && !has_profile {
        return None;
    }
    match (
        capability_status.as_deref(),
        hint_status.as_deref(),
        context_status.as_deref(),
        has_sidecar_context,
    ) {
        (Some("verified"), Some("declared"), Some("declared"), true) => Some("trusted".to_string()),
        (Some("verified"), _, _, true) => Some("degraded".to_string()),
        (Some(_), _, _, true) => Some("unverified".to_string()),
        (Some("verified"), Some("declared"), Some("declared"), false) => {
            Some("trusted".to_string())
        }
        (Some("verified"), _, _, false) => Some("degraded".to_string()),
        (Some(_), _, _, false) => Some("unverified".to_string()),
        _ if has_sidecar_context => Some("unverified".to_string()),
        _ => Some("unverified".to_string()),
    }
}

pub(crate) fn suspect_modules_json_from_snapshot(snapshot: &AnalysisSnapshot) -> String {
    crate::render_utils::string_list_json(&snapshot.suspect_modules)
}

pub(crate) fn analysis_evidence_posture(
    export: &ExportBundle,
    snapshot: &AnalysisSnapshot,
) -> &'static str {
    if export.ingest_trust_mode.starts_with("unverified") {
        "unverified_ingest"
    } else if snapshot.primary_process_profile_ambiguous {
        "ambiguous_multi_hypothesis"
    } else if snapshot.primary_failure_basis == "direct_protocol_signal" {
        "direct_protocol_signal"
    } else if snapshot.primary_failure_basis == "missing_transition" {
        "missing_transition"
    } else {
        "heuristic_summary"
    }
}

pub(crate) fn analysis_automation_outcome(
    export: &ExportBundle,
    snapshot: &AnalysisSnapshot,
) -> &'static str {
    if export.ingest_trust_mode.starts_with("unverified") {
        "advisory_only"
    } else if snapshot.operator_guidance_status == "targeted_ready"
        && !snapshot.primary_process_profile_ambiguous
    {
        "targeted_escalation"
    } else if snapshot.primary_process_profile_ambiguous
        || snapshot.operator_guidance_status == "ambiguous"
    {
        "multi_hypothesis"
    } else if snapshot.operator_guidance_status == "observe_more"
        || snapshot.operator_guidance_reason == "missing_transition"
    {
        "collect_more_evidence"
    } else {
        "manual_review"
    }
}

pub(crate) fn scan_target_status(export: &ExportBundle) -> ScanTargetStatus {
    if export.program_flows.is_empty() {
        ScanTargetStatus::Idle
    } else if export.program_findings.is_empty()
        && !export
            .program_flows
            .iter()
            .any(protocol_flow_has_terminal_failure)
    {
        ScanTargetStatus::Healthy
    } else {
        ScanTargetStatus::Attention
    }
}
