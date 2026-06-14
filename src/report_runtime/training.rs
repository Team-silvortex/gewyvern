use gewyvern::export::ExportBundle;

use super::*;
use crate::data_api::training_sample_id;
use crate::render_utils::{append_json_string, append_string_list_json};

#[cfg(test)]
pub(super) fn training_example_json(name: &str, export: &ExportBundle) -> String {
    let analysis = analysis_snapshot(export);
    training_example_json_with_analysis(name, export, &analysis)
}

pub(super) fn training_example_json_array(
    outputs: &[(String, ExportBundle)],
    analyses: &[AnalysisSnapshot],
) -> String {
    let mut json = String::from("[");
    for (index, ((name, export), analysis)) in outputs.iter().zip(analyses.iter()).enumerate() {
        if index > 0 {
            json.push(',');
        }
        json.push_str(&training_example_json_with_analysis(name, export, analysis));
    }
    json.push(']');
    json
}

pub(super) fn training_example_json_with_analysis(
    name: &str,
    export: &ExportBundle,
    analysis: &AnalysisSnapshot,
) -> String {
    let mut json = String::from("{\"kind\":\"training_example\",\"schema_version\":1,\"name\":");
    append_json_string(&mut json, name);
    json.push_str(",\"sample_id\":");
    append_json_string(&mut json, &training_sample_id(name));
    json.push_str(",\"template_id\":");
    append_json_string(&mut json, &export.template_id);
    json.push_str(",\"input\":{");
    json.push_str("\"target_status\":");
    append_json_string(&mut json, analysis.target_status.label());
    json.push_str(",\"primary_module_kind\":");
    append_json_string(&mut json, &analysis.primary_module_kind);
    json.push_str(",\"primary_failure_stage\":");
    append_json_string(&mut json, &analysis.primary_failure_stage);
    json.push_str(",\"primary_failure_mode\":");
    append_json_string(&mut json, &analysis.primary_failure_mode);
    json.push_str(",\"primary_failure_detail\":");
    append_json_string(&mut json, &analysis.primary_failure_detail);
    json.push_str(",\"primary_failure_confidence\":");
    append_json_string(&mut json, &analysis.primary_failure_confidence);
    json.push_str(",\"primary_failure_basis\":");
    append_json_string(&mut json, &analysis.primary_failure_basis);
    json.push_str(",\"ambiguous\":");
    json.push_str(if analysis.primary_process_profile_ambiguous {
        "true"
    } else {
        "false"
    });
    json.push_str(",\"competing_hypotheses\":");
    append_string_list_json(&mut json, &analysis.competing_hypotheses);
    json.push_str(",\"suspect_modules\":");
    append_string_list_json(&mut json, &analysis.suspect_modules);
    json.push_str(",\"protocol_flows\":");
    append_training_protocol_flow_slice(&mut json, analysis);
    json.push_str(",\"process_network_profiles\":");
    append_training_process_profile_slice(&mut json, analysis);
    json.push_str(",\"augmentations\":");
    append_training_augmentation_slice(&mut json, analysis);
    json.push_str(",\"external_sidecar_context\":");
    append_external_sidecar_context_json(&mut json, analysis);
    append_external_sidecar_contract_json(&mut json, analysis);
    json.push('}');
    json.push_str(",\"supervision\":{");
    json.push_str("\"operator_guidance_status\":");
    append_json_string(&mut json, &analysis.operator_guidance_status);
    json.push_str(",\"operator_guidance_action\":");
    append_json_string(&mut json, &analysis.operator_guidance_action);
    json.push_str(",\"operator_guidance_reason\":");
    append_json_string(&mut json, &analysis.operator_guidance_reason);
    json.push_str(",\"operator_guidance_summary\":");
    append_json_string(&mut json, &analysis.operator_guidance_summary);
    json.push_str(",\"targets\":{");
    append_training_targets_json(&mut json, export, analysis);
    json.push('}');
    json.push_str(",\"provenance\":{");
    json.push_str("\"ingest_mode\":");
    append_json_string(&mut json, ingest_mode_for_export(export));
    json.push_str(",\"ingest_mode_note\":");
    append_json_string(&mut json, &ingest_mode_note_for_export(export));
    json.push_str(",\"ingest_trust_mode\":");
    append_json_string(&mut json, &export.ingest_trust_mode);
    json.push_str(",\"pid_attribution_status\":");
    append_json_string(&mut json, pid_attribution_status_for_export(export));
    json.push_str(",\"fragments_loaded\":");
    json.push_str(&export.debug_summary.fragments_loaded.to_string());
    json.push_str(",\"flows\":");
    json.push_str(&export.debug_summary.flows.to_string());
    json.push_str(",\"program_findings\":");
    json.push_str(&export.debug_summary.program_findings.to_string());
    json.push_str(",\"module_findings\":");
    json.push_str(&export.debug_summary.module_findings.to_string());
    json.push('}');
    json.push('}');
    json
}

fn append_training_targets_json(
    json: &mut String,
    export: &ExportBundle,
    analysis: &AnalysisSnapshot,
) {
    json.push_str("\"diagnosis\":{");
    json.push_str("\"target_status\":");
    append_json_string(json, analysis.target_status.label());
    json.push_str(",\"primary_module_kind\":");
    append_json_string(json, &analysis.primary_module_kind);
    json.push_str(",\"primary_failure_stage\":");
    append_json_string(json, &analysis.primary_failure_stage);
    json.push_str(",\"primary_failure_mode\":");
    append_json_string(json, &analysis.primary_failure_mode);
    json.push_str(",\"primary_failure_detail\":");
    append_json_string(json, &analysis.primary_failure_detail);
    json.push_str(",\"primary_failure_confidence\":");
    append_json_string(json, &analysis.primary_failure_confidence);
    json.push_str(",\"primary_failure_basis\":");
    append_json_string(json, &analysis.primary_failure_basis);
    json.push_str(",\"ambiguous\":");
    json.push_str(if analysis.primary_process_profile_ambiguous {
        "true"
    } else {
        "false"
    });
    json.push('}');
    json.push_str(",\"guidance\":{");
    json.push_str("\"status\":");
    append_json_string(json, &analysis.operator_guidance_status);
    json.push_str(",\"action\":");
    append_json_string(json, &analysis.operator_guidance_action);
    json.push_str(",\"reason\":");
    append_json_string(json, &analysis.operator_guidance_reason);
    json.push('}');
    json.push_str(",\"automation\":{");
    json.push_str("\"posture\":");
    append_json_string(json, training_automation_posture(export, analysis));
    json.push_str(",\"requires_human_review\":");
    json.push_str(if training_requires_human_review(export, analysis) {
        "true"
    } else {
        "false"
    });
    json.push_str(",\"collect_more_evidence_first\":");
    json.push_str(if training_collect_more_evidence_first(export, analysis) {
        "true"
    } else {
        "false"
    });
    json.push_str(",\"targeted_escalation_allowed\":");
    json.push_str(if training_targeted_escalation_allowed(export, analysis) {
        "true"
    } else {
        "false"
    });
    json.push('}');
    json.push_str(",\"ranking\":{");
    json.push_str("\"attention_priority\":");
    append_json_string(json, training_attention_priority(analysis));
    json.push_str(",\"ambiguity_bucket\":");
    append_json_string(json, training_ambiguity_bucket(analysis));
    json.push_str(",\"evidence_posture\":");
    append_json_string(json, training_evidence_posture(export, analysis));
    json.push('}');
    json.push('}');
}

fn append_training_protocol_flow_slice(json: &mut String, analysis: &AnalysisSnapshot) {
    json.push('[');
    for (index, flow) in analysis.protocol_flows.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        json.push_str("{\"operation\":");
        append_json_string(json, &flow.operation);
        json.push_str(",\"network_module_kind\":");
        append_json_string(json, &flow.network_module_kind);
        json.push_str(",\"status\":");
        append_json_string(json, &flow.status);
        json.push_str(",\"failure_mode\":");
        append_json_string(json, &flow.failure_mode);
        json.push_str(",\"failure_detail\":");
        append_json_string(json, &flow.failure_detail);
        json.push_str(",\"phases\":");
        append_string_list_json(json, &flow.phases);
        json.push('}');
    }
    json.push(']');
}

fn append_training_process_profile_slice(json: &mut String, analysis: &AnalysisSnapshot) {
    json.push('[');
    for (index, profile) in analysis.process_profiles.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        json.push_str("{\"status\":");
        append_json_string(json, &profile.status);
        json.push_str(",\"primary_module_kind\":");
        append_json_string(json, &profile.primary_module_kind);
        json.push_str(",\"primary_failure_mode\":");
        append_json_string(json, &profile.primary_failure_mode);
        json.push_str(",\"suspect_modules\":");
        append_string_list_json(json, &profile.suspect_modules);
        json.push('}');
    }
    json.push(']');
}

fn append_training_augmentation_slice(json: &mut String, analysis: &AnalysisSnapshot) {
    json.push('[');
    for (index, item) in analysis.augmentations.iter().enumerate() {
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
        json.push('}');
    }
    json.push(']');
}

fn training_automation_posture(export: &ExportBundle, analysis: &AnalysisSnapshot) -> &'static str {
    if training_targeted_escalation_allowed(export, analysis) {
        "targeted_escalation"
    } else if training_collect_more_evidence_first(export, analysis) {
        "collect_more_evidence"
    } else if analysis.primary_process_profile_ambiguous {
        "human_review_with_hypotheses"
    } else if export.ingest_trust_mode.starts_with("unverified") {
        "advisory_only"
    } else {
        "manual_review"
    }
}

fn training_requires_human_review(export: &ExportBundle, analysis: &AnalysisSnapshot) -> bool {
    !training_targeted_escalation_allowed(export, analysis)
}

fn training_collect_more_evidence_first(
    export: &ExportBundle,
    analysis: &AnalysisSnapshot,
) -> bool {
    analysis.operator_guidance_status == "observe_more"
        || analysis.operator_guidance_reason == "missing_transition"
        || export.ingest_trust_mode.starts_with("unverified")
}

fn training_targeted_escalation_allowed(
    export: &ExportBundle,
    analysis: &AnalysisSnapshot,
) -> bool {
    analysis.operator_guidance_status == "targeted_ready"
        && !analysis.primary_process_profile_ambiguous
        && !export.ingest_trust_mode.starts_with("unverified")
}

fn training_attention_priority(analysis: &AnalysisSnapshot) -> &'static str {
    if analysis.primary_failure_confidence == "high" && !analysis.primary_process_profile_ambiguous
    {
        "high"
    } else if analysis.target_status.label() != "healthy"
        || analysis.primary_process_profile_ambiguous
    {
        "medium"
    } else {
        "low"
    }
}

fn training_ambiguity_bucket(analysis: &AnalysisSnapshot) -> &'static str {
    if analysis.primary_process_profile_ambiguous {
        "multi_hypothesis"
    } else {
        "single_hypothesis"
    }
}

fn training_evidence_posture(export: &ExportBundle, analysis: &AnalysisSnapshot) -> &'static str {
    if export.ingest_trust_mode.starts_with("unverified") {
        "unverified_ingest"
    } else if analysis.primary_process_profile_ambiguous {
        "ambiguous_multi_hypothesis"
    } else if analysis.primary_failure_basis == "direct_protocol_signal" {
        "direct_protocol_signal"
    } else if analysis.primary_failure_basis == "missing_transition" {
        "missing_transition"
    } else {
        "heuristic_summary"
    }
}
