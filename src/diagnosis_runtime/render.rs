use super::{
    AnalysisAugmentation, AnalysisSnapshot, ProcessNetworkProfileSummary,
    ProtocolFlowAnalysisSummary, external_capability_summary, external_sidecar_consumption_mode,
    external_sidecar_trust_level, failure_detail_family_label, failure_mode_family_label,
    stage_family_label,
};
use crate::UiLocale;
use crate::render_utils::{
    append_json_string, append_process_json, append_string_list_json, extract_json_string_field,
    push_joined_strings,
};

pub(crate) fn protocol_flow_summaries_json_from_snapshot(snapshot: &AnalysisSnapshot) -> String {
    let mut json = String::from("[");
    append_protocol_flow_summaries_json_from_snapshot(&mut json, snapshot);
    json.push(']');
    json
}

pub(crate) fn append_protocol_flow_summaries_json_from_snapshot(
    json: &mut String,
    snapshot: &AnalysisSnapshot,
) {
    append_protocol_flow_summaries_json_limited(json, snapshot, snapshot.protocol_flows.len());
}

pub(crate) fn append_protocol_flow_summaries_json_limited(
    json: &mut String,
    snapshot: &AnalysisSnapshot,
    limit: usize,
) {
    for (index, flow) in snapshot.protocol_flows.iter().take(limit).enumerate() {
        if index > 0 {
            json.push(',');
        }
        append_protocol_flow_summary_json(json, flow);
    }
}

pub(crate) fn protocol_flow_summaries_text_from_snapshot(snapshot: &AnalysisSnapshot) -> String {
    let locale = UiLocale::detect();
    if snapshot.protocol_flows.is_empty() {
        return locale.none().to_string();
    }
    let mut text = String::new();
    append_protocol_flow_summaries_text_from_snapshot(&mut text, snapshot);
    text
}

pub(crate) fn append_protocol_flow_summaries_text_from_snapshot(
    text: &mut String,
    snapshot: &AnalysisSnapshot,
) {
    append_protocol_flow_summaries_text_limited(text, snapshot, snapshot.protocol_flows.len());
}

pub(crate) fn append_protocol_flow_summaries_text_limited(
    text: &mut String,
    snapshot: &AnalysisSnapshot,
    limit: usize,
) {
    let locale = UiLocale::detect();
    for (index, flow) in snapshot.protocol_flows.iter().take(limit).enumerate() {
        if index > 0 {
            text.push(',');
        }
        text.push_str(&flow.operation);
        text.push_str("[kind=");
        text.push_str(&flow.network_module_kind);
        text.push_str(" status=");
        text.push_str(&flow.status);
        text.push_str(" failure_mode=");
        text.push_str(&flow.failure_mode);
        text.push_str(" failure_detail=");
        text.push_str(&flow.failure_detail);
        text.push_str(" confidence=");
        text.push_str(&flow.failure_confidence);
        text.push_str(" basis=");
        text.push_str(&flow.failure_basis);
        text.push_str(" phases=");
        if flow.phases.is_empty() {
            text.push_str(locale.none());
        } else {
            push_joined_strings(text, &flow.phases, ">");
        }
        if !flow.missing_transitions.is_empty() {
            text.push_str(" missing=");
            push_joined_strings(text, &flow.missing_transitions, "|");
        }
        text.push(']');
    }
}

pub(crate) fn process_network_profiles_json_from_snapshot(snapshot: &AnalysisSnapshot) -> String {
    let mut json = String::from("[");
    append_process_network_profiles_json_from_snapshot(&mut json, snapshot);
    json.push(']');
    json
}

pub(crate) fn append_process_network_profiles_json_from_snapshot(
    json: &mut String,
    snapshot: &AnalysisSnapshot,
) {
    for (index, profile) in snapshot.process_profiles.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        append_process_network_profile_summary_json(json, profile);
    }
}

#[cfg(test)]
pub(crate) fn process_network_profiles_json(export: &gewyvern::export::ExportBundle) -> String {
    process_network_profiles_json_from_snapshot(&super::analysis_snapshot(export))
}

pub(crate) fn process_network_profiles_text_from_snapshot(snapshot: &AnalysisSnapshot) -> String {
    let locale = UiLocale::detect();
    if snapshot.process_profiles.is_empty() {
        return locale.none().to_string();
    }
    let mut text = String::new();
    append_process_network_profiles_text_from_snapshot(&mut text, snapshot);
    text
}

pub(crate) fn append_process_network_profiles_text_from_snapshot(
    text: &mut String,
    snapshot: &AnalysisSnapshot,
) {
    let locale = UiLocale::detect();
    for (index, profile) in snapshot.process_profiles.iter().enumerate() {
        if index > 0 {
            text.push(',');
        }
        text.push_str(&profile.comm);
        text.push_str("(pid=");
        text.push_str(&profile.pid.to_string());
        text.push_str(")[status=");
        text.push_str(&profile.status);
        text.push_str(" ambiguous=");
        text.push_str(if profile.ambiguous { "true" } else { "false" });
        text.push_str(" primary_kind=");
        text.push_str(&profile.primary_module_kind);
        text.push_str(" primary_stage=");
        text.push_str(&profile.primary_failure_stage);
        text.push_str(" failure_mode=");
        text.push_str(&profile.primary_failure_mode);
        text.push_str(" failure_detail=");
        text.push_str(&profile.primary_failure_detail);
        text.push_str(" confidence=");
        text.push_str(&profile.primary_failure_confidence);
        text.push_str(" basis=");
        text.push_str(&profile.primary_failure_basis);
        text.push_str(" competing=");
        if profile.competing_hypotheses.is_empty() {
            text.push_str(locale.none());
        } else {
            push_joined_strings(text, &profile.competing_hypotheses, "|");
        }
        text.push_str(" kinds=");
        if profile.module_kinds.is_empty() {
            text.push_str(locale.none());
        } else {
            push_joined_strings(text, &profile.module_kinds, "|");
        }
        text.push_str(" healthy=");
        text.push_str(&profile.healthy_flows.to_string());
        text.push_str(" attention=");
        text.push_str(&profile.attention_flows.to_string());
        text.push_str(" phases=");
        if profile.phases.is_empty() {
            text.push_str(locale.none());
        } else {
            push_joined_strings(text, &profile.phases, ">");
        }
        if !profile.missing_transitions.is_empty() {
            text.push_str(" missing=");
            push_joined_strings(text, &profile.missing_transitions, "|");
        }
        text.push(']');
    }
}

pub(crate) fn analysis_snapshot_json(snapshot: &AnalysisSnapshot) -> String {
    let mut json = String::with_capacity(estimate_analysis_snapshot_json_capacity(snapshot));
    json.push_str("{\"target_status\":\"");
    json.push_str(snapshot.target_status.label());
    json.push_str("\",\"primary_process_profile\":");
    if let Some(profile) = snapshot.primary_process_profile.as_ref() {
        append_process_network_profile_summary_json(&mut json, profile);
    } else {
        json.push_str("null");
    }
    json.push_str(",\"primary_module_kind\":\"");
    json.push_str(&snapshot.primary_module_kind);
    json.push_str("\",\"primary_module_family\":\"");
    json.push_str(&snapshot.primary_module_family);
    json.push_str("\",\"primary_failure_stage\":\"");
    json.push_str(&snapshot.primary_failure_stage);
    json.push_str("\",\"primary_stage_family\":\"");
    json.push_str(stage_family_label(&snapshot.primary_failure_stage));
    json.push_str("\",\"primary_failure_mode\":\"");
    json.push_str(&snapshot.primary_failure_mode);
    json.push_str("\",\"primary_failure_mode_family\":\"");
    json.push_str(failure_mode_family_label(&snapshot.primary_failure_mode));
    json.push_str("\",\"primary_failure_detail\":\"");
    json.push_str(&snapshot.primary_failure_detail);
    json.push_str("\",\"primary_failure_detail_family\":\"");
    json.push_str(failure_detail_family_label(
        &snapshot.primary_failure_detail,
    ));
    json.push_str("\",\"primary_failure_confidence\":\"");
    json.push_str(&snapshot.primary_failure_confidence);
    json.push_str("\",\"primary_failure_basis\":\"");
    json.push_str(&snapshot.primary_failure_basis);
    json.push_str("\",\"evidence_posture\":\"");
    json.push_str(&snapshot.evidence_posture);
    json.push_str("\",\"automation_outcome\":\"");
    json.push_str(&snapshot.automation_outcome);
    json.push_str("\",\"operator_guidance_status\":\"");
    json.push_str(&snapshot.operator_guidance_status);
    json.push_str("\",\"operator_guidance_action\":\"");
    json.push_str(&snapshot.operator_guidance_action);
    json.push_str("\",\"operator_guidance_reason\":\"");
    json.push_str(&snapshot.operator_guidance_reason);
    json.push_str("\",\"operator_guidance_summary\":\"");
    json.push_str(&snapshot.operator_guidance_summary);
    json.push_str("\",\"ambiguous\":");
    json.push_str(if snapshot.primary_process_profile_ambiguous {
        "true"
    } else {
        "false"
    });
    json.push_str(",\"competing_hypotheses\":");
    append_string_list_json(&mut json, &snapshot.competing_hypotheses);
    json.push_str(",\"operations\":");
    append_string_list_json(&mut json, &snapshot.operations);
    json.push_str(",\"phases\":");
    append_string_list_json(&mut json, &snapshot.phases);
    json.push_str(",\"missing_transitions\":");
    append_string_list_json(&mut json, &snapshot.missing_transitions);
    json.push_str(",\"suspect_areas\":");
    append_string_list_json(&mut json, &snapshot.suspect_areas);
    json.push_str(",\"suspect_modules\":");
    append_string_list_json(&mut json, &snapshot.suspect_modules);
    json.push_str(",\"augmentations\":");
    append_analysis_augmentations_json(&mut json, &snapshot.augmentations);
    json.push_str(",\"external_sidecar_context\":");
    append_external_sidecar_context_json(&mut json, snapshot);
    append_external_sidecar_contract_json(&mut json, snapshot);
    json.push_str(",\"process_network_profiles\":");
    json.push('[');
    append_process_network_profiles_json_from_snapshot(&mut json, snapshot);
    json.push(']');
    json.push_str(",\"protocol_flows\":");
    json.push('[');
    append_protocol_flow_summaries_json_from_snapshot(&mut json, snapshot);
    json.push(']');
    json.push('}');
    json
}

fn estimate_analysis_snapshot_json_capacity(snapshot: &AnalysisSnapshot) -> usize {
    512 + snapshot.primary_module_kind.len()
        + snapshot.primary_module_family.len()
        + snapshot.primary_failure_stage.len()
        + snapshot.primary_failure_mode.len()
        + snapshot.primary_failure_detail.len()
        + snapshot.primary_failure_confidence.len()
        + snapshot.primary_failure_basis.len()
        + snapshot.evidence_posture.len()
        + snapshot.automation_outcome.len()
        + snapshot.operator_guidance_status.len()
        + snapshot.operator_guidance_action.len()
        + snapshot.operator_guidance_reason.len()
        + snapshot.operator_guidance_summary.len()
        + snapshot
            .competing_hypotheses
            .iter()
            .map(String::len)
            .sum::<usize>()
        + snapshot.operations.iter().map(String::len).sum::<usize>()
        + snapshot.phases.iter().map(String::len).sum::<usize>()
        + snapshot
            .missing_transitions
            .iter()
            .map(String::len)
            .sum::<usize>()
        + snapshot
            .suspect_areas
            .iter()
            .map(String::len)
            .sum::<usize>()
        + snapshot
            .suspect_modules
            .iter()
            .map(String::len)
            .sum::<usize>()
        + snapshot.augmentations.len() * 160
        + snapshot.process_profiles.len() * 520
        + snapshot.protocol_flows.len() * 420
}

pub(crate) fn append_external_sidecar_context_json(json: &mut String, snapshot: &AnalysisSnapshot) {
    json.push_str("{\"evidence_chain_enrichment\":");
    append_external_sidecar_item_json(json, snapshot, "external_evidence_chain_enrichment");
    json.push_str(",\"diagnostic_opinion\":");
    append_external_sidecar_item_json(json, snapshot, "external_diagnostic_opinion");
    json.push('}');
}

pub(crate) fn append_external_sidecar_contract_json(
    json: &mut String,
    snapshot: &AnalysisSnapshot,
) {
    let (has_profile, capability_status, hint_status, context_status) =
        external_capability_summary(snapshot);
    let consumption_mode = external_sidecar_consumption_mode(snapshot);
    let trust_level = external_sidecar_trust_level(snapshot);
    json.push_str(",\"has_external_capability_profile\":");
    json.push_str(if has_profile { "true" } else { "false" });
    json.push_str(",\"external_capability_status\":");
    if let Some(value) = capability_status.as_deref() {
        append_json_string(json, value);
    } else {
        json.push_str("null");
    }
    json.push_str(",\"external_hint_status\":");
    if let Some(value) = hint_status.as_deref() {
        append_json_string(json, value);
    } else {
        json.push_str("null");
    }
    json.push_str(",\"external_context_status\":");
    if let Some(value) = context_status.as_deref() {
        append_json_string(json, value);
    } else {
        json.push_str("null");
    }
    json.push_str(",\"external_sidecar_trust_level\":");
    if let Some(value) = trust_level.as_deref() {
        append_json_string(json, value);
    } else {
        json.push_str("null");
    }
    json.push_str(",\"external_sidecar_consumption_mode\":");
    if let Some(value) = consumption_mode.as_deref() {
        append_json_string(json, value);
    } else {
        json.push_str("null");
    }
}

fn append_external_sidecar_item_json(
    json: &mut String,
    snapshot: &AnalysisSnapshot,
    item_name: &str,
) {
    let Some(item) = snapshot
        .augmentations
        .iter()
        .find(|item| item.name == item_name)
    else {
        json.push_str("null");
        return;
    };
    json.push_str("{\"summary\":");
    append_json_string(json, &item.summary);
    json.push_str(",\"confidence\":");
    append_json_string(json, &item.confidence);
    json.push_str(",\"producer_stage\":");
    if let Some(stage) = item.producer_stage.as_deref() {
        append_json_string(json, stage);
    } else {
        json.push_str("null");
    }
    json.push_str(",\"producer_pass\":");
    if let Some(pass) = item.producer_pass.as_deref() {
        append_json_string(json, pass);
    } else {
        json.push_str("null");
    }
    json.push_str(",\"handoff_readiness\":");
    append_optional_embedded_json_string_field(
        json,
        item.data_json.as_deref(),
        "external_handoff_readiness",
    );
    json.push_str(",\"merge_hint\":");
    append_optional_embedded_json_string_field(
        json,
        item.data_json.as_deref(),
        "external_merge_hint",
    );
    json.push_str(",\"context_status\":");
    append_optional_embedded_json_string_field(
        json,
        item.data_json.as_deref(),
        "external_context_status",
    );
    json.push_str(",\"consumption_mode\":");
    if let Some(mode) = crate::diagnosis_runtime::external_sidecar_item_consumption_mode(item) {
        append_json_string(json, mode);
    } else {
        json.push_str("null");
    }
    json.push('}');
}

fn append_optional_embedded_json_string_field(
    json: &mut String,
    data_json: Option<&str>,
    key: &str,
) {
    if let Some(value) = data_json.and_then(|data| extract_embedded_json_string_value(data, key)) {
        append_json_string(json, &value);
    } else {
        json.push_str("null");
    }
}

fn extract_embedded_json_string_value(input: &str, key: &str) -> Option<String> {
    extract_json_string_field(input, key)
}

pub(crate) fn append_analysis_augmentations_json(
    json: &mut String,
    items: &[AnalysisAugmentation],
) {
    json.push('[');
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        json.push_str("{\"kind\":");
        append_json_string(json, &item.kind);
        json.push_str(",\"name\":");
        append_json_string(json, &item.name);
        json.push_str(",\"summary\":");
        append_json_string(json, &item.summary);
        json.push_str(",\"confidence\":");
        append_json_string(json, &item.confidence);
        json.push_str(",\"producer_stage\":");
        if let Some(stage) = item.producer_stage.as_deref() {
            append_json_string(json, stage);
        } else {
            json.push_str("null");
        }
        json.push_str(",\"producer_pass\":");
        if let Some(pass) = item.producer_pass.as_deref() {
            append_json_string(json, pass);
        } else {
            json.push_str("null");
        }
        json.push_str(",\"data\":");
        json.push_str(item.data_json.as_deref().unwrap_or("null"));
        json.push('}');
    }
    json.push(']');
}

fn append_process_network_profile_summary_json(
    json: &mut String,
    profile: &ProcessNetworkProfileSummary,
) {
    json.push_str("{\"pid\":");
    json.push_str(&profile.pid.to_string());
    json.push_str(",\"comm\":\"");
    json.push_str(&profile.comm);
    json.push_str("\",\"status\":\"");
    json.push_str(&profile.status);
    json.push_str("\",\"ambiguous\":");
    json.push_str(if profile.ambiguous { "true" } else { "false" });
    json.push_str(",\"primary_module_kind\":\"");
    json.push_str(&profile.primary_module_kind);
    json.push_str("\",\"primary_module_family\":\"");
    json.push_str(&profile.primary_module_family);
    json.push_str("\",\"primary_failure_stage\":\"");
    json.push_str(&profile.primary_failure_stage);
    json.push_str("\",\"primary_stage_family\":\"");
    json.push_str(&profile.primary_stage_family);
    json.push_str("\",\"primary_failure_mode\":\"");
    json.push_str(&profile.primary_failure_mode);
    json.push_str("\",\"primary_failure_mode_family\":\"");
    json.push_str(failure_mode_family_label(&profile.primary_failure_mode));
    json.push_str("\",\"primary_failure_detail\":\"");
    json.push_str(&profile.primary_failure_detail);
    json.push_str("\",\"primary_failure_detail_family\":\"");
    json.push_str(failure_detail_family_label(&profile.primary_failure_detail));
    json.push_str("\",\"primary_failure_confidence\":\"");
    json.push_str(&profile.primary_failure_confidence);
    json.push_str("\",\"primary_failure_basis\":\"");
    json.push_str(&profile.primary_failure_basis);
    json.push_str("\",\"competing_hypotheses\":");
    append_string_list_json(json, &profile.competing_hypotheses);
    json.push_str(",\"operations\":");
    append_string_list_json(json, &profile.operations);
    json.push_str(",\"module_kinds\":");
    append_string_list_json(json, &profile.module_kinds);
    json.push_str(",\"phases\":");
    append_string_list_json(json, &profile.phases);
    json.push_str(",\"missing_transitions\":");
    append_string_list_json(json, &profile.missing_transitions);
    json.push_str(",\"suspect_areas\":");
    append_string_list_json(json, &profile.suspect_areas);
    json.push_str(",\"suspect_modules\":");
    append_string_list_json(json, &profile.suspect_modules);
    json.push_str(",\"healthy_flows\":");
    json.push_str(&profile.healthy_flows.to_string());
    json.push_str(",\"attention_flows\":");
    json.push_str(&profile.attention_flows.to_string());
    json.push('}');
}

fn append_protocol_flow_summary_json(json: &mut String, flow: &ProtocolFlowAnalysisSummary) {
    json.push_str("{\"program_flow\":");
    json.push_str(&flow.program_flow.to_string());
    json.push_str(",\"process\":");
    append_process_json(json, flow.process.as_ref());
    json.push_str(",\"operation\":\"");
    json.push_str(&flow.operation);
    json.push_str("\",\"network_module_kind\":\"");
    json.push_str(&flow.network_module_kind);
    json.push_str("\",\"network_module_kinds\":");
    append_string_list_json(json, &flow.network_module_kinds);
    json.push_str(",\"status\":\"");
    json.push_str(&flow.status);
    json.push_str("\",\"failure_mode\":\"");
    json.push_str(&flow.failure_mode);
    json.push_str("\",\"failure_mode_family\":\"");
    json.push_str(failure_mode_family_label(&flow.failure_mode));
    json.push_str("\",\"failure_detail\":\"");
    json.push_str(&flow.failure_detail);
    json.push_str("\",\"failure_detail_family\":\"");
    json.push_str(failure_detail_family_label(&flow.failure_detail));
    json.push_str("\",\"failure_confidence\":\"");
    json.push_str(&flow.failure_confidence);
    json.push_str("\",\"failure_basis\":\"");
    json.push_str(&flow.failure_basis);
    json.push_str("\",\"phases\":");
    append_string_list_json(json, &flow.phases);
    json.push_str(",\"last_phase\":");
    if let Some(phase) = flow.last_phase.as_deref() {
        json.push('"');
        json.push_str(phase);
        json.push('"');
    } else {
        json.push_str("null");
    }
    json.push_str(",\"missing_transitions\":");
    append_string_list_json(json, &flow.missing_transitions);
    json.push_str(",\"suspect_areas\":");
    append_string_list_json(json, &flow.suspect_areas);
    json.push('}');
}
