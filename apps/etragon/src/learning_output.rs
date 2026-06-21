use super::*;

pub(super) fn evidence_chain_enrichment_json(
    top_learned_label: Option<&str>,
    top_learned_state: &str,
    training_conflict_hint: &str,
    memory_drift_hint: &str,
    queue_pressure_hint: &str,
    feedback_policy_hint: &str,
    recent_label_activity: &str,
) -> String {
    let Some(primary_label) = top_learned_label else {
        return "null".to_string();
    };

    let confidence_hint = extract_json_value(top_learned_state, "confidence_hint")
        .unwrap_or_else(|| "null".to_string());
    let stability_hint = extract_json_value(top_learned_state, "stability_hint")
        .unwrap_or_else(|| "null".to_string());
    let support_score = extract_json_value(top_learned_state, "support_score")
        .unwrap_or_else(|| "null".to_string());
    let train_count =
        extract_json_value(top_learned_state, "train_count").unwrap_or_else(|| "null".to_string());
    let drift_status =
        extract_json_value(memory_drift_hint, "status").unwrap_or_else(|| "null".to_string());
    let pressure_status =
        extract_json_value(queue_pressure_hint, "status").unwrap_or_else(|| "null".to_string());
    let feedback_policy =
        extract_json_value(feedback_policy_hint, "policy").unwrap_or_else(|| "null".to_string());
    let recent_events =
        extract_json_value(recent_label_activity, "event_count").unwrap_or_else(|| "0".to_string());
    let conflict_status = if training_conflict_hint == "null" {
        "\"none\"".to_string()
    } else {
        extract_json_value(training_conflict_hint, "status")
            .unwrap_or_else(|| "\"present\"".to_string())
    };

    let status = if training_conflict_hint != "null" {
        "contested"
    } else if drift_status.contains("\"stable\"") || drift_status.contains("\"converging\"") {
        "reinforced"
    } else {
        "emerging"
    };
    let enrichment_strength_band = if training_conflict_hint != "null" {
        "low"
    } else if drift_status.contains("\"stable\"")
        || (confidence_hint.contains("\"high\"") && stability_hint.contains("\"stable\""))
    {
        "high"
    } else if drift_status.contains("\"converging\"") || confidence_hint.contains("\"high\"") {
        "medium"
    } else {
        "low"
    };
    let handoff_readiness = if training_conflict_hint != "null" || status == "emerging" {
        "advisory_only"
    } else if enrichment_strength_band == "high" {
        "automation_worthy"
    } else {
        "mergeable"
    };
    let gewyvern_merge_hint = match handoff_readiness {
        "advisory_only" => "augmentations_only",
        "mergeable" => "augmentations_and_guidance_context",
        "automation_worthy" => "augmentations_with_operator_guidance_support",
        _ => "augmentations_only",
    };

    let summary = if training_conflict_hint != "null" {
        "learned route exists, but competing feedback is still present in the evidence chain"
    } else if drift_status.contains("\"stable\"") {
        "learned route is being reinforced by recent feedback and stable pattern memory"
    } else if drift_status.contains("\"converging\"") {
        "learned route is aligning with the current pattern memory and becoming a stronger explanation"
    } else {
        "learned route exists and is contributing a higher-level evidence-chain hint, but it is still maturing"
    };

    format!(
        "{{\"status\":\"{}\",\"primary_label\":\"{}\",\"support_score\":{},\"train_count\":{},\"confidence_hint\":{},\"stability_hint\":{},\"drift_status\":{},\"conflict_status\":{},\"pressure_status\":{},\"feedback_policy\":{},\"recent_event_count\":{},\"enrichment_strength_band\":\"{}\",\"handoff_readiness\":\"{}\",\"gewyvern_merge_hint\":\"{}\",\"summary\":\"{}\"}}",
        escape_json_string(status),
        escape_json_string(primary_label),
        support_score,
        train_count,
        confidence_hint,
        stability_hint,
        drift_status,
        conflict_status,
        pressure_status,
        feedback_policy,
        recent_events,
        escape_json_string(enrichment_strength_band),
        escape_json_string(handoff_readiness),
        escape_json_string(gewyvern_merge_hint),
        escape_json_string(summary)
    )
}

pub(super) fn diagnostic_opinion_json(
    top_learned_label: Option<&str>,
    top_learned_state: &str,
    learning_judgement: &str,
    training_conflict_hint: &str,
    source_scope: &str,
) -> String {
    let Some(primary_label) = top_learned_label else {
        return "null".to_string();
    };
    if training_conflict_hint != "null" {
        return "null".to_string();
    }

    let judgement_status = extract_json_value(learning_judgement, "status")
        .map(|value| value.trim_matches('"').to_string())
        .unwrap_or_default();
    let confidence_hint = extract_json_value(top_learned_state, "confidence_hint")
        .map(|value| value.trim_matches('"').to_string())
        .unwrap_or_default();
    let stability_hint = extract_json_value(top_learned_state, "stability_hint")
        .map(|value| value.trim_matches('"').to_string())
        .unwrap_or_default();
    let support_score = extract_json_value(top_learned_state, "support_score")
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or(0.0);
    let score_margin = extract_json_value(top_learned_state, "score_margin")
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or(0.0);

    let (status, diagnosis_kind, summary) = match primary_label {
        "network_observe_longer"
            if judgement_status == "ready"
                || (judgement_status == "leaning"
                    && confidence_hint == "high"
                    && support_score >= 2.0
                    && score_margin >= 1.5) =>
        {
            (
                if judgement_status == "ready" {
                    "ready"
                } else {
                    "tentative"
                },
                "timeout_or_missing_followup",
                "the evidence chain now leans toward a timeout or missing follow-up path, so continuing runtime observation is the most direct diagnosis",
            )
        }
        "targeted_escalation"
            if judgement_status == "ready"
                || (judgement_status == "leaning"
                    && confidence_hint == "high"
                    && support_score >= 2.0
                    && score_margin >= 1.5) =>
        {
            (
                if judgement_status == "ready" {
                    "ready"
                } else {
                    "tentative"
                },
                "direct_protocol_failure",
                "the evidence chain now leans toward a direct protocol failure that is strong enough for higher-level escalation",
            )
        }
        "http_request_followup"
            if judgement_status == "ready"
                || (judgement_status == "leaning"
                    && confidence_hint == "high"
                    && support_score >= 2.0
                    && score_margin >= 1.5) =>
        {
            (
                if judgement_status == "ready" {
                    "ready"
                } else {
                    "tentative"
                },
                "http_request_path_issue",
                "the evidence chain now leans toward an HTTP request-path issue that is worth treating as the more direct diagnostic opinion",
            )
        }
        _ => return "null".to_string(),
    };
    let opinion_confidence_band = if judgement_status == "ready"
        || (confidence_hint == "high" && stability_hint == "stable" && score_margin >= 1.5)
    {
        "high"
    } else {
        "medium"
    };
    let handoff_readiness = if judgement_status == "ready" && opinion_confidence_band == "high" {
        "automation_worthy"
    } else {
        "mergeable"
    };
    let gewyvern_merge_hint = match handoff_readiness {
        "automation_worthy" => "operator_guidance_candidate",
        "mergeable" => "sidecar_only_opinion",
        _ => "sidecar_only_opinion",
    };

    format!(
        "{{\"status\":\"{}\",\"diagnosis_kind\":\"{}\",\"label\":\"{}\",\"summary\":\"{}\",\"source_scope\":\"{}\",\"opinion_confidence_band\":\"{}\",\"handoff_readiness\":\"{}\",\"gewyvern_merge_hint\":\"{}\",\"judgement_status\":\"{}\",\"confidence_hint\":\"{}\",\"stability_hint\":\"{}\",\"support_score\":{},\"score_margin\":{}}}",
        escape_json_string(status),
        escape_json_string(diagnosis_kind),
        escape_json_string(primary_label),
        escape_json_string(summary),
        escape_json_string(source_scope),
        escape_json_string(opinion_confidence_band),
        escape_json_string(handoff_readiness),
        escape_json_string(gewyvern_merge_hint),
        escape_json_string(&judgement_status),
        escape_json_string(&confidence_hint),
        escape_json_string(&stability_hint),
        support_score,
        score_margin
    )
}

pub(super) fn learning_summary_json_from_output_and_history_with_scope(
    output_json: &str,
    summary_json: &str,
    training_history: &[TrainingEvent],
    queue_summary_override: Option<&str>,
    source_scope: &str,
) -> String {
    let (learning_active, learned_routes) =
        learned_route_summary_from_recommendation_summary(summary_json);
    let summary = recommendation_overview_json(&[("latest".to_string(), summary_json.to_string())]);
    let top_candidate_name = extract_named_string_fields(summary_json, "name")
        .into_iter()
        .find(|name| name == "py_ml_candidate_learned_route")
        .map(|value| format!("\"{}\"", escape_json_string(&value)))
        .unwrap_or_else(|| "null".to_string());
    let top_learned_label = training_history
        .iter()
        .rev()
        .find(|event| !event.label.is_empty())
        .map(|event| event.label.clone());
    let top_learned_label_json = top_learned_label
        .as_ref()
        .map(|value| format!("\"{}\"", escape_json_string(value)))
        .unwrap_or_else(|| "null".to_string());
    let top_learned_relationships = top_learned_label
        .as_deref()
        .and_then(training_label_spec_for)
        .map(|spec| {
            let compatible_with = spec
                .compatible_with
                .iter()
                .map(|label| format!("\"{}\"", escape_json_string(label)))
                .collect::<Vec<_>>()
                .join(",");
            let competes_with = spec
                .competes_with
                .iter()
                .map(|label| format!("\"{}\"", escape_json_string(label)))
                .collect::<Vec<_>>()
                .join(",");
            format!(
                "{{\"compatible_with\":[{}],\"competes_with\":[{}]}}",
                compatible_with, competes_with
            )
        })
        .unwrap_or_else(|| "null".to_string());
    let transition_policy_summary =
        transition_policy_summary_json_for_label(top_learned_label.as_deref());
    let top_learned_state = top_learned_state_json_from_recommendation_summary(summary_json);
    let training_conflict_hint = training_conflict_hint_json(training_history);
    let pattern_memory_state = extract_json_value(output_json, "pattern_memory_state")
        .unwrap_or_else(|| "null".to_string());
    let pattern_memory_summary =
        pattern_memory_summary_json(&pattern_memory_state, top_learned_label.as_deref());
    let memory_drift_hint = memory_drift_hint_json(
        top_learned_label.as_deref(),
        &top_learned_state,
        &pattern_memory_summary,
        &training_conflict_hint,
    );
    let learning_judgement = learning_judgement_json(
        top_learned_label.as_deref(),
        &top_learned_state,
        &training_conflict_hint,
        &memory_drift_hint,
    );
    let action_queue_hint =
        action_queue_hint_json(&learning_judgement, top_learned_label.as_deref());
    let queue_summary = queue_summary_override
        .map(|value| value.to_string())
        .unwrap_or_else(|| {
            queue_summary_json_from_action_hints(&[(None, action_queue_hint.clone())])
        });
    let queue_pressure_hint = queue_pressure_hint_json(&queue_summary);
    let feedback_policy_hint = feedback_policy_hint_json(
        &learning_judgement,
        &queue_pressure_hint,
        &training_conflict_hint,
    );
    let recent_label_activity = recent_label_activity_json(training_history);
    let evidence_chain_enrichment = evidence_chain_enrichment_json(
        top_learned_label.as_deref(),
        &top_learned_state,
        &training_conflict_hint,
        &memory_drift_hint,
        &queue_pressure_hint,
        &feedback_policy_hint,
        &recent_label_activity,
    );
    let diagnostic_opinion = diagnostic_opinion_json(
        top_learned_label.as_deref(),
        &top_learned_state,
        &learning_judgement,
        &training_conflict_hint,
        source_scope,
    );
    format!(
        "{{\"learning_active\":{},\"learned_routes\":{},\"top_learned_route\":{},\"top_learned_label\":{},\"top_learned_relationships\":{},\"top_learned_state\":{},\"transition_policy_summary\":{},\"training_conflict_hint\":{},\"pattern_memory_state\":{},\"pattern_memory_summary\":{},\"memory_drift_hint\":{},\"learning_judgement\":{},\"action_queue_hint\":{},\"queue_summary\":{},\"queue_pressure_hint\":{},\"feedback_policy_hint\":{},\"evidence_chain_enrichment\":{},\"diagnostic_opinion\":{},\"recent_training_events\":{},\"recent_label_activity\":{},\"recommendation_summary\":{}}}",
        if learning_active { "true" } else { "false" },
        learned_routes,
        top_candidate_name,
        top_learned_label_json,
        top_learned_relationships,
        top_learned_state,
        transition_policy_summary,
        training_conflict_hint,
        pattern_memory_state,
        pattern_memory_summary,
        memory_drift_hint,
        learning_judgement,
        action_queue_hint,
        queue_summary,
        queue_pressure_hint,
        feedback_policy_hint,
        evidence_chain_enrichment,
        diagnostic_opinion,
        training_history_json(training_history),
        recent_label_activity,
        summary
    )
}

pub(super) fn learning_summary_json_from_output_and_history(
    output_json: &str,
    summary_json: &str,
    training_history: &[TrainingEvent],
    queue_summary_override: Option<&str>,
) -> String {
    learning_summary_json_from_output_and_history_with_scope(
        output_json,
        summary_json,
        training_history,
        queue_summary_override,
        "latest",
    )
}

pub(super) fn learning_summary_field_json(
    output_json: &str,
    summary_json: &str,
    training_history: &[TrainingEvent],
    queue_summary_override: Option<&str>,
    key: &str,
    source_scope: &str,
) -> String {
    let learning_summary = learning_summary_json_from_output_and_history_with_scope(
        output_json,
        summary_json,
        training_history,
        queue_summary_override,
        source_scope,
    );
    extract_json_value(&learning_summary, key).unwrap_or_else(|| "null".to_string())
}

pub(super) fn handoff_summary_json_from_learning_summary(
    learning_summary: &str,
    source_scope: &str,
) -> String {
    let evidence_chain_enrichment =
        extract_json_value(learning_summary, "evidence_chain_enrichment")
            .unwrap_or_else(|| "null".to_string());
    let diagnostic_opinion = extract_json_value(learning_summary, "diagnostic_opinion")
        .unwrap_or_else(|| "null".to_string());

    let has_evidence_chain_enrichment = evidence_chain_enrichment != "null";
    let has_diagnostic_opinion = diagnostic_opinion != "null";
    let primary = if has_diagnostic_opinion {
        &diagnostic_opinion
    } else {
        &evidence_chain_enrichment
    };

    let handoff_readiness =
        extract_json_value(primary, "handoff_readiness").unwrap_or_else(|| "null".to_string());
    let gewyvern_merge_hint =
        extract_json_value(primary, "gewyvern_merge_hint").unwrap_or_else(|| "null".to_string());
    let primary_status =
        extract_json_value(primary, "status").unwrap_or_else(|| "null".to_string());
    let summary = extract_json_value(primary, "summary").unwrap_or_else(|| "null".to_string());
    let primary_label = if has_diagnostic_opinion {
        extract_json_value(primary, "label").unwrap_or_else(|| "null".to_string())
    } else {
        extract_json_value(primary, "primary_label").unwrap_or_else(|| "null".to_string())
    };
    let enrichment_strength_band =
        extract_json_value(&evidence_chain_enrichment, "enrichment_strength_band")
            .unwrap_or_else(|| "null".to_string());
    let opinion_confidence_band =
        extract_json_value(&diagnostic_opinion, "opinion_confidence_band")
            .unwrap_or_else(|| "null".to_string());

    format!(
        "{{\"source_scope\":\"{}\",\"has_evidence_chain_enrichment\":{},\"has_diagnostic_opinion\":{},\"handoff_readiness\":{},\"gewyvern_merge_hint\":{},\"primary_status\":{},\"primary_label\":{},\"summary\":{},\"enrichment_strength_band\":{},\"opinion_confidence_band\":{}}}",
        escape_json_string(source_scope),
        if has_evidence_chain_enrichment {
            "true"
        } else {
            "false"
        },
        if has_diagnostic_opinion {
            "true"
        } else {
            "false"
        },
        handoff_readiness,
        gewyvern_merge_hint,
        primary_status,
        primary_label,
        summary,
        enrichment_strength_band,
        opinion_confidence_band,
    )
}

pub(super) fn handoff_summary_json(
    output_json: &str,
    summary_json: &str,
    training_history: &[TrainingEvent],
    queue_summary_override: Option<&str>,
    source_scope: &str,
) -> String {
    let learning_summary = learning_summary_json_from_output_and_history_with_scope(
        output_json,
        summary_json,
        training_history,
        queue_summary_override,
        source_scope,
    );
    handoff_summary_json_from_learning_summary(&learning_summary, source_scope)
}

pub(super) fn batch_output_json(entries: &[(String, String)]) -> String {
    let body = entries
        .iter()
        .map(|(segment, output)| {
            let rendered_output = match output.strip_prefix("__error__:") {
                Some(error) => format!("{{\"error\":\"{}\"}}", escape_json_string(error)),
                None => output.clone(),
            };
            format!(
                "{{\"path_segment\":\"{}\",\"output\":{}}}",
                escape_json_string(segment),
                rendered_output
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let recommendation_summary = recommendation_summary_array_json(entries);
    format!(
        "{{\"targets\":[{}],\"recommendation_summary\":{}}}",
        body, recommendation_summary
    )
}

pub(super) fn target_results_json(entries: &[(String, String)]) -> String {
    let body = entries
        .iter()
        .map(|(segment, output)| {
            format!(
                "{{\"path_segment\":\"{}\",\"output\":{}}}",
                escape_json_string(segment),
                output
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("{{\"targets\":[{}]}}", body)
}
