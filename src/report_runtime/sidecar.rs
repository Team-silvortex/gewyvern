use std::borrow::Cow;

use super::*;

#[derive(Default)]
pub(super) struct ExternalSidecarDerivedState<'a> {
    enrichment_handoff: Option<Cow<'a, str>>,
    enrichment_merge_hint: Option<Cow<'a, str>>,
    opinion_handoff: Option<Cow<'a, str>>,
    opinion_merge_hint: Option<Cow<'a, str>>,
}

#[derive(Default)]
struct ExternalSidecarItemJsonState<'a> {
    item: Option<&'a AnalysisAugmentation>,
    handoff: Option<Cow<'a, str>>,
    merge_hint: Option<Cow<'a, str>>,
    context_status: Option<Cow<'a, str>>,
}

impl ExternalSidecarDerivedState<'_> {
    pub(super) fn collaboration_note(&self) -> Option<(&'static str, &'static str)> {
        match self.opinion_handoff.as_deref() {
            Some("automation_worthy") => Some((
                "automation_worthy_sidecar_opinion",
                "external sidecar offers an automation-worthy diagnostic opinion that can be treated as strong nearby context",
            )),
            Some("mergeable") => Some((
                "mergeable_sidecar_opinion",
                "external sidecar offers a mergeable diagnostic opinion that can safely enrich operator-facing interpretation",
            )),
            Some(_) => Some((
                "advisory_only_sidecar_context",
                "external sidecar is contributing only advisory diagnostic context and should not be treated as a direct merged conclusion",
            )),
            None => match self.enrichment_handoff.as_deref() {
                Some("automation_worthy") | Some("mergeable") => Some((
                    "mergeable_sidecar_enrichment",
                    "external sidecar is reinforcing the evidence chain strongly enough to be treated as mergeable context",
                )),
                Some(_) => Some((
                    "advisory_only_sidecar_context",
                    "external sidecar is contributing only advisory evidence-chain context and should remain additive",
                )),
                None => None,
            },
        }
    }

    pub(super) fn operator_guidance_support_note(&self) -> Option<(&'static str, &'static str)> {
        match self.opinion_merge_hint.as_deref() {
            Some("operator_guidance_candidate") => Some((
                "operator_guidance_candidate",
                "external sidecar opinion is strong enough to be treated as a nearby operator-guidance candidate",
            )),
            _ => match self.enrichment_merge_hint.as_deref() {
                Some("augmentations_with_operator_guidance_support") => Some((
                    "guidance_supporting_enrichment",
                    "external sidecar enrichment reinforces the current built-in operator guidance without replacing it",
                )),
                Some("augmentations_and_guidance_context") => Some((
                    "guidance_context_only",
                    "external sidecar adds operator-guidance context but should still be read as additive support only",
                )),
                _ => None,
            },
        }
    }

    pub(super) fn item_hints(&self, item_name: &str) -> (&str, &str) {
        match item_name {
            "external_evidence_chain_enrichment" => (
                self.enrichment_handoff
                    .as_deref()
                    .unwrap_or("advisory_only"),
                self.enrichment_merge_hint
                    .as_deref()
                    .unwrap_or("augmentations_only"),
            ),
            "external_diagnostic_opinion" => (
                self.opinion_handoff.as_deref().unwrap_or("mergeable"),
                self.opinion_merge_hint
                    .as_deref()
                    .unwrap_or("sidecar_only_opinion"),
            ),
            _ => ("unknown", "unknown"),
        }
    }
}

pub(super) fn external_sidecar_derived_state<'a>(
    analysis: &'a AnalysisSnapshot,
) -> ExternalSidecarDerivedState<'a> {
    let mut state = ExternalSidecarDerivedState::default();
    for item in &analysis.augmentations {
        match item.name.as_str() {
            "external_evidence_chain_enrichment" => {
                state.enrichment_handoff = item.data_json.as_deref().and_then(|data| {
                    extract_json_string_field_cow(data, "external_handoff_readiness")
                });
                state.enrichment_merge_hint = item
                    .data_json
                    .as_deref()
                    .and_then(|data| extract_json_string_field_cow(data, "external_merge_hint"));
            }
            "external_diagnostic_opinion" => {
                state.opinion_handoff = item.data_json.as_deref().and_then(|data| {
                    extract_json_string_field_cow(data, "external_handoff_readiness")
                });
                state.opinion_merge_hint = item
                    .data_json
                    .as_deref()
                    .and_then(|data| extract_json_string_field_cow(data, "external_merge_hint"));
            }
            _ => {}
        }
    }
    state
}

fn external_sidecar_hint_summary(analysis: &AnalysisSnapshot) -> (String, String) {
    let state = external_sidecar_derived_state(analysis);
    let (enrichment_handoff, enrichment_merge_hint) =
        state.item_hints("external_evidence_chain_enrichment");
    let (opinion_handoff, opinion_merge_hint) = state.item_hints("external_diagnostic_opinion");
    (
        if state.enrichment_handoff.is_some() {
            format!("{}+{}", enrichment_handoff, enrichment_merge_hint)
        } else {
            "none".to_string()
        },
        if state.opinion_handoff.is_some() || state.opinion_merge_hint.is_some() {
            format!("{}+{}", opinion_handoff, opinion_merge_hint)
        } else {
            "none".to_string()
        },
    )
}

pub(super) fn external_sidecar_collaboration_note(
    analysis: &AnalysisSnapshot,
) -> Option<(&'static str, &'static str)> {
    external_sidecar_derived_state(analysis).collaboration_note()
}

pub(super) fn external_sidecar_rollup_counts(
    analyses: &[AnalysisSnapshot],
) -> (usize, usize, usize) {
    let mut mergeable = 0;
    let mut automation_worthy = 0;
    let mut advisory_only = 0;
    for analysis in analyses {
        if let Some((state, _)) = external_sidecar_collaboration_note(analysis) {
            match state {
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
) -> Option<(&'static str, &'static str)> {
    external_sidecar_derived_state(analysis).operator_guidance_support_note()
}

pub(super) fn append_external_sidecar_fields(json: &mut String, analysis: &AnalysisSnapshot) {
    let contract = crate::diagnosis_runtime::external_sidecar_contract_state(analysis);
    let context = external_sidecar_json_state(analysis);
    append_external_sidecar_context_field_from_state(json, &context);
    append_external_sidecar_contract_fields_from_contract(json, &contract);
}

fn external_sidecar_json_state<'a>(
    analysis: &'a AnalysisSnapshot,
) -> (
    ExternalSidecarItemJsonState<'a>,
    ExternalSidecarItemJsonState<'a>,
) {
    let mut enrichment = ExternalSidecarItemJsonState::default();
    let mut opinion = ExternalSidecarItemJsonState::default();
    for item in &analysis.augmentations {
        match item.name.as_str() {
            "external_evidence_chain_enrichment" => {
                enrichment.item = Some(item);
                enrichment.handoff = item.data_json.as_deref().and_then(|data| {
                    extract_json_string_field_cow(data, "external_handoff_readiness")
                });
                enrichment.merge_hint = item
                    .data_json
                    .as_deref()
                    .and_then(|data| extract_json_string_field_cow(data, "external_merge_hint"));
                enrichment.context_status = item.data_json.as_deref().and_then(|data| {
                    extract_json_string_field_cow(data, "external_context_status")
                });
            }
            "external_diagnostic_opinion" => {
                opinion.item = Some(item);
                opinion.handoff = item.data_json.as_deref().and_then(|data| {
                    extract_json_string_field_cow(data, "external_handoff_readiness")
                });
                opinion.merge_hint = item
                    .data_json
                    .as_deref()
                    .and_then(|data| extract_json_string_field_cow(data, "external_merge_hint"));
                opinion.context_status = item.data_json.as_deref().and_then(|data| {
                    extract_json_string_field_cow(data, "external_context_status")
                });
            }
            _ => {}
        }
    }
    (enrichment, opinion)
}

fn append_external_sidecar_context_field_from_state(
    json: &mut String,
    state: &(
        ExternalSidecarItemJsonState<'_>,
        ExternalSidecarItemJsonState<'_>,
    ),
) {
    json.push_str(",\"external_sidecar_context\":");
    json.push_str("{\"evidence_chain_enrichment\":");
    append_external_sidecar_item_json(
        json,
        state.0.item,
        state.0.handoff.as_deref(),
        state.0.merge_hint.as_deref(),
        state.0.context_status.as_deref(),
    );
    json.push_str(",\"diagnostic_opinion\":");
    append_external_sidecar_item_json(
        json,
        state.1.item,
        state.1.handoff.as_deref(),
        state.1.merge_hint.as_deref(),
        state.1.context_status.as_deref(),
    );
    json.push('}');
}

fn append_external_sidecar_contract_fields_from_contract(
    json: &mut String,
    contract: &crate::diagnosis_runtime::ExternalSidecarContractState<'_>,
) {
    let trust_level = contract.trust_level();
    json.push_str(",\"has_external_capability_profile\":");
    json.push_str(if contract.has_profile {
        "true"
    } else {
        "false"
    });
    json.push_str(",\"external_capability_status\":");
    if let Some(value) = contract.capability_status.as_deref() {
        append_json_string(json, value);
    } else {
        json.push_str("null");
    }
    json.push_str(",\"external_hint_status\":");
    if let Some(value) = contract.hint_status.as_deref() {
        append_json_string(json, value);
    } else {
        json.push_str("null");
    }
    json.push_str(",\"external_context_status\":");
    if let Some(value) = contract.context_status.as_deref() {
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
    if let Some(value) = contract.consumption_mode.as_deref() {
        append_json_string(json, value);
    } else {
        json.push_str("null");
    }
}

fn append_external_sidecar_item_json(
    json: &mut String,
    item: Option<&AnalysisAugmentation>,
    handoff: Option<&str>,
    merge_hint: Option<&str>,
    context_status: Option<&str>,
) {
    let Some(item) = item else {
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
    append_optional_json_string(json, handoff);
    json.push_str(",\"merge_hint\":");
    append_optional_json_string(json, merge_hint);
    json.push_str(",\"context_status\":");
    append_optional_json_string(json, context_status);
    json.push_str(",\"consumption_mode\":");
    if let Some(mode) = crate::diagnosis_runtime::external_sidecar_consumption_mode_for(
        item.name.as_str(),
        merge_hint,
    ) {
        append_json_string(json, mode);
    } else {
        json.push_str("null");
    }
    json.push('}');
}

fn append_optional_json_string(json: &mut String, value: Option<&str>) {
    if let Some(value) = value {
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
        .unwrap_or("none");
    let sidecar_guidance_support = external_operator_guidance_support_note(analysis)
        .map(|(state, _)| state)
        .unwrap_or("none");
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
    let mut text = String::new();
    append_diagnosis_spine_text(&mut text, analysis);
    text
}

pub(super) fn append_diagnosis_spine_text(text: &mut String, analysis: &AnalysisSnapshot) {
    text.push_str("family=");
    text.push_str(&analysis.primary_module_family);
    text.push_str(" posture=");
    text.push_str(&analysis.evidence_posture);
    text.push_str(" outcome=");
    text.push_str(&analysis.automation_outcome);
}
