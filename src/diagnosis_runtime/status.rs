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

pub(crate) fn suspect_modules_json_from_snapshot(snapshot: &AnalysisSnapshot) -> String {
    crate::render_utils::string_list_json(&snapshot.suspect_modules)
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
