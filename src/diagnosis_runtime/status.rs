use std::borrow::Cow;

use gewyvern::export::ExportBundle;

use super::{AnalysisAugmentation, AnalysisSnapshot};
use crate::UiLocale;

#[derive(Default)]
pub(crate) struct ExternalSidecarContractState<'a> {
    pub(crate) has_sidecar_context: bool,
    pub(crate) has_enrichment: bool,
    pub(crate) has_opinion: bool,
    pub(crate) has_profile: bool,
    pub(crate) capability_status: Option<Cow<'a, str>>,
    pub(crate) hint_status: Option<Cow<'a, str>>,
    pub(crate) context_status: Option<Cow<'a, str>>,
    pub(crate) consumption_mode: Option<&'static str>,
}

impl ExternalSidecarContractState<'_> {
    pub(crate) fn trust_level(&self) -> Option<&'static str> {
        if !self.has_sidecar_context && !self.has_profile {
            return None;
        }
        match (
            self.capability_status.as_deref(),
            self.hint_status.as_deref(),
            self.context_status.as_deref(),
            self.has_sidecar_context,
        ) {
            (Some("verified"), Some("declared"), Some("declared"), true) => Some("trusted"),
            (Some("verified"), _, _, true) => Some("degraded"),
            (Some(_), _, _, true) => Some("unverified"),
            (Some("verified"), Some("declared"), Some("declared"), false) => Some("trusted"),
            (Some("verified"), _, _, false) => Some("degraded"),
            (Some(_), _, _, false) => Some("unverified"),
            _ if self.has_sidecar_context => Some("unverified"),
            _ => Some("unverified"),
        }
    }
}

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
        || lowered == "authorization_failure"
        || lowered == "unauthorized"
        || lowered == "nak"
        || lowered == "report_pdu"
        || lowered == "constraint"
        || lowered == "error"
        || lowered == "closed"
        || lowered.contains("denied")
        || lowered.contains("auth_required")
        || lowered.contains("authorization_failure")
        || lowered.contains("unauthorized")
        || lowered.contains("nak")
        || lowered.contains("report_pdu")
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
    let state = external_sidecar_contract_state(snapshot);
    (
        state.has_sidecar_context,
        state.has_enrichment,
        state.has_opinion,
    )
}

pub(crate) fn external_capability_summary(
    snapshot: &AnalysisSnapshot,
) -> (bool, Option<String>, Option<String>, Option<String>) {
    let state = external_sidecar_contract_state(snapshot);
    (
        state.has_profile,
        state.capability_status.map(Cow::into_owned),
        state.hint_status.map(Cow::into_owned),
        state.context_status.map(Cow::into_owned),
    )
}

pub(crate) fn external_sidecar_item_consumption_mode(
    item: &AnalysisAugmentation,
) -> Option<&'static str> {
    let merge_hint = item.data_json.as_deref().and_then(|data| {
        crate::render_utils::extract_json_string_field_borrowed(data, "external_merge_hint")
    });
    external_sidecar_consumption_mode_for(item.name.as_str(), merge_hint)
}

pub(crate) fn external_sidecar_consumption_mode_for(
    item_name: &str,
    merge_hint: Option<&str>,
) -> Option<&'static str> {
    match (item_name, merge_hint) {
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
    external_sidecar_contract_state(snapshot)
        .consumption_mode
        .map(str::to_string)
}

pub(crate) fn external_sidecar_contract_state<'a>(
    snapshot: &'a AnalysisSnapshot,
) -> ExternalSidecarContractState<'a> {
    let mut best_rank = 0u8;
    let mut state = ExternalSidecarContractState::default();
    for item in &snapshot.augmentations {
        match item.name.as_str() {
            "external_evidence_chain_enrichment" => state.has_enrichment = true,
            "external_diagnostic_opinion" => state.has_opinion = true,
            "external_capability_profile" => {
                state.has_profile = true;
                state.capability_status = item.data_json.as_deref().and_then(|data| {
                    crate::render_utils::extract_json_string_field_cow(data, "compatibility_status")
                });
                state.hint_status = item.data_json.as_deref().and_then(|data| {
                    crate::render_utils::extract_json_string_field_cow(data, "hint_status")
                });
                state.context_status = item.data_json.as_deref().and_then(|data| {
                    crate::render_utils::extract_json_string_field_cow(data, "context_status")
                });
            }
            _ => {}
        }

        if let Some(mode) = external_sidecar_item_consumption_mode(item) {
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
                state.consumption_mode = Some(mode);
            }
        }
    }
    state.has_sidecar_context = state.has_enrichment || state.has_opinion;
    state
}

pub(crate) fn external_sidecar_trust_level(snapshot: &AnalysisSnapshot) -> Option<String> {
    external_sidecar_contract_state(snapshot)
        .trust_level()
        .map(str::to_string)
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
