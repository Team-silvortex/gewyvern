use super::*;
use crate::diagnosis_runtime::append_external_sidecar_context_json;

fn external_sidecar_hint_summary(analysis: &AnalysisSnapshot) -> (String, String) {
    let mut enrichment = "none".to_string();
    let mut opinion = "none".to_string();
    for item in &analysis.augmentations {
        if item.name == "external_evidence_chain_enrichment" {
            let handoff = item
                .data_json
                .as_deref()
                .and_then(|data| extract_json_string_field(data, "external_handoff_readiness"))
                .unwrap_or_else(|| "advisory_only".to_string());
            let merge_hint = item
                .data_json
                .as_deref()
                .and_then(|data| extract_json_string_field(data, "external_merge_hint"))
                .unwrap_or_else(|| "augmentations_only".to_string());
            enrichment = format!("{}+{}", handoff, merge_hint);
        } else if item.name == "external_diagnostic_opinion" {
            let handoff = item
                .data_json
                .as_deref()
                .and_then(|data| extract_json_string_field(data, "external_handoff_readiness"))
                .unwrap_or_else(|| "mergeable".to_string());
            let merge_hint = item
                .data_json
                .as_deref()
                .and_then(|data| extract_json_string_field(data, "external_merge_hint"))
                .unwrap_or_else(|| "sidecar_only_opinion".to_string());
            opinion = format!("{}+{}", handoff, merge_hint);
        }
    }
    (enrichment, opinion)
}

pub(super) fn external_sidecar_collaboration_note(
    analysis: &AnalysisSnapshot,
) -> Option<(String, String)> {
    let mut enrichment_handoff = None;
    let mut opinion_handoff = None;
    for item in &analysis.augmentations {
        if item.name == "external_evidence_chain_enrichment" {
            enrichment_handoff = item
                .data_json
                .as_deref()
                .and_then(|data| extract_json_string_field(data, "external_handoff_readiness"));
        } else if item.name == "external_diagnostic_opinion" {
            opinion_handoff = item
                .data_json
                .as_deref()
                .and_then(|data| extract_json_string_field(data, "external_handoff_readiness"));
        }
    }
    match opinion_handoff.as_deref() {
        Some("automation_worthy") => Some((
            "automation_worthy_sidecar_opinion".to_string(),
            "external sidecar offers an automation-worthy diagnostic opinion that can be treated as strong nearby context".to_string(),
        )),
        Some("mergeable") => Some((
            "mergeable_sidecar_opinion".to_string(),
            "external sidecar offers a mergeable diagnostic opinion that can safely enrich operator-facing interpretation".to_string(),
        )),
        Some(_) => Some((
            "advisory_only_sidecar_context".to_string(),
            "external sidecar is contributing only advisory diagnostic context and should not be treated as a direct merged conclusion".to_string(),
        )),
        None => match enrichment_handoff.as_deref() {
            Some("automation_worthy") | Some("mergeable") => Some((
                "mergeable_sidecar_enrichment".to_string(),
                "external sidecar is reinforcing the evidence chain strongly enough to be treated as mergeable context".to_string(),
            )),
            Some(_) => Some((
                "advisory_only_sidecar_context".to_string(),
                "external sidecar is contributing only advisory evidence-chain context and should remain additive".to_string(),
            )),
            None => None,
        },
    }
}

pub(super) fn external_sidecar_rollup_counts(
    analyses: &[AnalysisSnapshot],
) -> (usize, usize, usize) {
    let mut mergeable = 0;
    let mut automation_worthy = 0;
    let mut advisory_only = 0;
    for analysis in analyses {
        if let Some((state, _)) = external_sidecar_collaboration_note(analysis) {
            match state.as_str() {
                "automation_worthy_sidecar_opinion" => automation_worthy += 1,
                "mergeable_sidecar_opinion" | "mergeable_sidecar_enrichment" => mergeable += 1,
                "advisory_only_sidecar_context" => advisory_only += 1,
                _ => {}
            }
        }
    }
    (mergeable, automation_worthy, advisory_only)
}

pub(super) fn external_operator_guidance_support_note(
    analysis: &AnalysisSnapshot,
) -> Option<(String, String)> {
    let mut enrichment_merge_hint = None;
    let mut opinion_merge_hint = None;
    for item in &analysis.augmentations {
        if item.name == "external_evidence_chain_enrichment" {
            enrichment_merge_hint = item
                .data_json
                .as_deref()
                .and_then(|data| extract_json_string_field(data, "external_merge_hint"));
        } else if item.name == "external_diagnostic_opinion" {
            opinion_merge_hint = item
                .data_json
                .as_deref()
                .and_then(|data| extract_json_string_field(data, "external_merge_hint"));
        }
    }
    match opinion_merge_hint.as_deref() {
        Some("operator_guidance_candidate") => Some((
            "operator_guidance_candidate".to_string(),
            "external sidecar opinion is strong enough to be treated as a nearby operator-guidance candidate".to_string(),
        )),
        _ => match enrichment_merge_hint.as_deref() {
            Some("augmentations_with_operator_guidance_support") => Some((
                "guidance_supporting_enrichment".to_string(),
                "external sidecar enrichment reinforces the current built-in operator guidance without replacing it".to_string(),
            )),
            Some("augmentations_and_guidance_context") => Some((
                "guidance_context_only".to_string(),
                "external sidecar adds operator-guidance context but should still be read as additive support only".to_string(),
            )),
            _ => None,
        },
    }
}

pub(super) fn append_external_sidecar_context_field(
    json: &mut String,
    analysis: &AnalysisSnapshot,
) {
    json.push_str(",\"external_sidecar_context\":");
    append_external_sidecar_context_json(json, analysis);
}

pub(super) fn append_external_sidecar_contract_fields(
    json: &mut String,
    analysis: &AnalysisSnapshot,
) {
    let (has_profile, capability_status, hint_status, context_status) =
        crate::diagnosis_runtime::external_capability_summary(analysis);
    let consumption_mode = crate::diagnosis_runtime::external_sidecar_consumption_mode(analysis);
    let trust_level = crate::diagnosis_runtime::external_sidecar_trust_level(analysis);
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

pub(super) fn summary_line(name: &str, export: &ExportBundle) -> String {
    let analysis = analysis_snapshot(export);
    summary_line_with_analysis(name, export, &analysis)
}

pub(super) fn summary_line_with_analysis(
    name: &str,
    export: &ExportBundle,
    analysis: &AnalysisSnapshot,
) -> String {
    let locale = UiLocale::detect();
    let suspect_areas = if export.program_findings.is_empty() {
        locale.none().to_string()
    } else {
        export
            .program_findings
            .iter()
            .map(|finding| finding.suspect_area.clone())
            .collect::<Vec<_>>()
            .join(",")
    };
    let suspect_modules = if export.program_findings.is_empty() {
        locale.none().to_string()
    } else {
        export
            .program_findings
            .iter()
            .map(|finding| finding.module_label.clone())
            .collect::<Vec<_>>()
            .join(",")
    };
    let protocol_flows = protocol_flow_summaries_text_from_snapshot(analysis);
    let process_profiles = process_network_profiles_text_from_snapshot(analysis);
    let augmentations = analysis_augmentation_names_text(&analysis.augmentations);
    let (sidecar_enrichment, sidecar_opinion) = external_sidecar_hint_summary(analysis);
    let sidecar_collaboration_state = external_sidecar_collaboration_note(analysis)
        .map(|(state, _)| state)
        .unwrap_or_else(|| "none".to_string());
    let sidecar_guidance_support = external_operator_guidance_support_note(analysis)
        .map(|(state, _)| state)
        .unwrap_or_else(|| "none".to_string());
    let ingest_mode_note = ingest_mode_note_for_export(export);
    let diagnosis_spine = diagnosis_spine_text(analysis);
    format!(
        "{name}: diagnosis_spine={} {}={} ingest_mode={} ingest_mode_note={} {}={} pid_attribution_status={} operator_guidance_status={} operator_guidance_action={} operator_guidance_reason={} ambiguous={} competing_hypotheses={} augmentations={} external_enrichment_hint={} external_diagnostic_opinion_hint={} external_collaboration_state={} external_operator_guidance_support={} {}={} {}={} {}={} {}={} {}={} {}={} {}={} {}={} {}={} {}={} {}={} protocol_flows={} process_network_profiles={}",
        diagnosis_spine,
        locale.label("template"),
        export.template_id,
        ingest_mode_for_export(export),
        ingest_mode_note,
        "ingest_trust_mode",
        export.ingest_trust_mode,
        pid_attribution_status_for_export(export),
        analysis.operator_guidance_status,
        analysis.operator_guidance_action,
        analysis.operator_guidance_reason,
        analysis.primary_process_profile_ambiguous,
        string_list_json(&analysis.competing_hypotheses),
        augmentations,
        sidecar_enrichment,
        sidecar_opinion,
        sidecar_collaboration_state,
        sidecar_guidance_support,
        locale.label("fragments_loaded"),
        export.debug_summary.fragments_loaded,
        locale.label("hookpoints_failed"),
        export.debug_summary.hookpoints_failed,
        locale.label("accepted_facts"),
        export.debug_summary.accepted_facts,
        locale.label("rejected_facts"),
        export.debug_summary.rejected_facts,
        locale.label("flows"),
        export.debug_summary.flows,
        locale.label("program_findings"),
        export.debug_summary.program_findings,
        locale.label("module_findings"),
        export.debug_summary.module_findings,
        locale.label("reasons"),
        export.debug_summary.reasons,
        locale.label("degraded"),
        export.debug_summary.degraded,
        locale.label("suspect_areas"),
        suspect_areas,
        locale.label("suspect_modules"),
        suspect_modules,
        protocol_flows,
        process_profiles,
    )
}

pub(super) fn diagnosis_spine_text(analysis: &AnalysisSnapshot) -> String {
    format!(
        "family={} posture={} outcome={}",
        analysis.primary_module_family, analysis.evidence_posture, analysis.automation_outcome
    )
}
