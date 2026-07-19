use super::*;

pub(super) fn training_conflict_hint_json(history: &[TrainingEvent]) -> String {
    let activities_json = recent_label_activity_json(history);
    let labels = extract_named_string_fields(&activities_json, "label");
    let event_counts = extract_named_numeric_fields(&activities_json, "event_count");
    let total_weights = extract_named_numeric_fields(&activities_json, "total_weight");

    for (index, label) in labels.iter().enumerate() {
        let Some(spec) = training_label_spec_for(label) else {
            continue;
        };
        for competing_label in spec.competes_with {
            if let Some(other_index) = labels
                .iter()
                .position(|candidate| candidate == competing_label)
            {
                let primary_event_count = event_counts
                    .get(index)
                    .and_then(|value| value.parse::<usize>().ok())
                    .unwrap_or(0);
                let competing_event_count = event_counts
                    .get(other_index)
                    .and_then(|value| value.parse::<usize>().ok())
                    .unwrap_or(0);
                let primary_total_weight = total_weights
                    .get(index)
                    .cloned()
                    .unwrap_or_else(|| "0".to_string());
                let competing_total_weight = total_weights
                    .get(other_index)
                    .cloned()
                    .unwrap_or_else(|| "0".to_string());
                return format!(
                    "{{\"status\":\"competing_labels\",\"primary_label\":\"{}\",\"competing_label\":\"{}\",\"primary_event_count\":{},\"competing_event_count\":{},\"primary_total_weight\":{},\"competing_total_weight\":{}}}",
                    escape_json_string(label),
                    escape_json_string(competing_label),
                    primary_event_count,
                    competing_event_count,
                    primary_total_weight,
                    competing_total_weight
                );
            }
        }
    }
    "null".to_string()
}

pub(super) fn pattern_memory_summary_json(
    pattern_memory_state: &str,
    top_learned_label: Option<&str>,
) -> String {
    if pattern_memory_state == "null" {
        return "null".to_string();
    }
    let labels = extract_named_string_fields(pattern_memory_state, "label");
    if labels.is_empty() {
        return "null".to_string();
    }
    let support_scores = extract_named_numeric_fields(pattern_memory_state, "support_score");
    let label_count = extract_named_numeric_fields(pattern_memory_state, "label_count")
        .into_iter()
        .next()
        .unwrap_or_else(|| labels.len().to_string());
    let top_label = labels.first().cloned().unwrap_or_default();
    let top_support_score = support_scores
        .first()
        .cloned()
        .unwrap_or_else(|| "0".to_string());
    let runner_up_label = labels
        .get(1)
        .map(|label| format!("\"{}\"", escape_json_string(label)))
        .unwrap_or_else(|| "null".to_string());
    let top_labels = labels
        .iter()
        .take(3)
        .map(|label| format!("\"{}\"", escape_json_string(label)))
        .collect::<Vec<_>>()
        .join(",");
    let learned_label_rank = top_learned_label
        .and_then(|label| labels.iter().position(|candidate| candidate == label))
        .map(|index| (index + 1).to_string())
        .unwrap_or_else(|| "null".to_string());
    format!(
        "{{\"label_count\":{},\"top_pattern_label\":\"{}\",\"top_pattern_support_score\":{},\"runner_up_label\":{},\"top_labels\":[{}],\"learned_label_rank\":{}}}",
        label_count,
        escape_json_string(&top_label),
        top_support_score,
        runner_up_label,
        top_labels,
        learned_label_rank
    )
}

pub(super) fn memory_drift_hint_json(
    top_learned_label: Option<&str>,
    top_learned_state: &str,
    pattern_memory_summary: &str,
    training_conflict_hint: &str,
) -> String {
    let Some(primary_label) = top_learned_label else {
        return "null".to_string();
    };

    let confidence_hint = extract_json_value(top_learned_state, "confidence_hint")
        .unwrap_or_else(|| "null".to_string());
    let stability_hint = extract_json_value(top_learned_state, "stability_hint")
        .unwrap_or_else(|| "null".to_string());
    let score_margin = extract_json_value(top_learned_state, "score_margin")
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or(0.0);
    let top_pattern_label = extract_json_value(pattern_memory_summary, "top_pattern_label")
        .map(|value| value.trim_matches('"').to_string());
    let runner_up_label =
        extract_json_value(pattern_memory_summary, "runner_up_label").and_then(|value| {
            let trimmed = value.trim_matches('"');
            if trimmed.is_empty() || trimmed == "null" {
                None
            } else {
                Some(trimmed.to_string())
            }
        });

    let (status, reason) = if training_conflict_hint != "null" {
        ("conflicted", "recent_competing_training_feedback")
    } else if top_pattern_label
        .as_deref()
        .is_some_and(|pattern_label| pattern_label != primary_label)
    {
        ("switching", "pattern_memory_prefers_different_label")
    } else if runner_up_label.is_some() && score_margin < 0.5 {
        ("volatile", "runner_up_remains_close")
    } else if stability_hint.contains("stable") && confidence_hint.contains("high") {
        ("stable", "lead_is_clear_and_recently_reinforced")
    } else if stability_hint.contains("emerging") {
        ("emerging", "learning_signal_is_still_early")
    } else {
        ("converging", "top_label_and_pattern_memory_are_aligning")
    };

    format!(
        "{{\"status\":\"{}\",\"reason\":\"{}\",\"primary_label\":\"{}\",\"top_pattern_label\":{},\"runner_up_label\":{},\"score_margin\":{},\"confidence_hint\":{},\"stability_hint\":{}}}",
        escape_json_string(status),
        escape_json_string(reason),
        escape_json_string(primary_label),
        top_pattern_label
            .map(|value| format!("\"{}\"", escape_json_string(&value)))
            .unwrap_or_else(|| "null".to_string()),
        runner_up_label
            .map(|value| format!("\"{}\"", escape_json_string(&value)))
            .unwrap_or_else(|| "null".to_string()),
        score_margin,
        confidence_hint,
        stability_hint
    )
}

pub(super) fn learning_judgement_json(
    top_learned_label: Option<&str>,
    top_learned_state: &str,
    training_conflict_hint: &str,
    memory_drift_hint: &str,
) -> String {
    let Some(primary_label) = top_learned_label else {
        return "null".to_string();
    };

    let confidence_hint = extract_json_value(top_learned_state, "confidence_hint")
        .unwrap_or_else(|| "null".to_string());
    let stability_hint = extract_json_value(top_learned_state, "stability_hint")
        .unwrap_or_else(|| "null".to_string());
    let drift_status =
        extract_json_value(memory_drift_hint, "status").unwrap_or_else(|| "null".to_string());
    let drift_reason =
        extract_json_value(memory_drift_hint, "reason").unwrap_or_else(|| "null".to_string());

    let (status, reason) = if training_conflict_hint != "null" {
        ("manual_review", "recent_training_feedback_is_competing")
    } else if drift_status.contains("\"switching\"") {
        (
            "watch_transition",
            "pattern_memory_is_shifting_toward_another_label",
        )
    } else if drift_status.contains("\"volatile\"") {
        (
            "watch_transition",
            "runner_up_remains_close_to_the_current_lead",
        )
    } else if confidence_hint.contains("\"high\"") && stability_hint.contains("\"stable\"") {
        ("ready", "learned_route_lead_is_clear_and_stable")
    } else if stability_hint.contains("\"emerging\"") {
        ("observe", "learned_route_is_present_but_still_early")
    } else {
        ("leaning", "learned_route_is_aligning_but_not_fully_settled")
    };

    format!(
        "{{\"status\":\"{}\",\"reason\":\"{}\",\"primary_label\":\"{}\",\"confidence_hint\":{},\"stability_hint\":{},\"drift_status\":{},\"drift_reason\":{}}}",
        escape_json_string(status),
        escape_json_string(reason),
        escape_json_string(primary_label),
        confidence_hint,
        stability_hint,
        drift_status,
        drift_reason
    )
}

pub(super) fn action_queue_hint_json(
    learning_judgement: &str,
    top_learned_label: Option<&str>,
) -> String {
    let Some(primary_label) = top_learned_label else {
        return "null".to_string();
    };

    let judgement_status =
        extract_json_value(learning_judgement, "status").unwrap_or_else(|| "null".to_string());
    let judgement_reason =
        extract_json_value(learning_judgement, "reason").unwrap_or_else(|| "null".to_string());

    let (action, queue, priority) = if judgement_status.contains("\"manual_review\"") {
        ("manual_review", "human_review", "high")
    } else if judgement_status.contains("\"watch_transition\"") {
        ("queue_transition_check", "transition_watch", "medium")
    } else if judgement_status.contains("\"observe\"") {
        ("keep_observing", "observation", "low")
    } else if judgement_status.contains("\"ready\"") {
        ("promote_learned_route", "automation_ready", "high")
    } else {
        ("keep_observing", "observation", "medium")
    };

    format!(
        "{{\"action\":\"{}\",\"queue\":\"{}\",\"priority\":\"{}\",\"primary_label\":\"{}\",\"reason\":{}}}",
        escape_json_string(action),
        escape_json_string(queue),
        escape_json_string(priority),
        escape_json_string(primary_label),
        judgement_reason
    )
}

pub(super) fn queue_summary_json_from_action_hints(
    action_hints: &[(Option<&str>, String)],
) -> String {
    #[derive(Clone)]
    struct QueueBucket {
        action: String,
        queue: String,
        priority: String,
        count: usize,
        targets: Vec<String>,
    }

    let mut buckets: Vec<QueueBucket> = Vec::new();
    for (target, hint) in action_hints {
        if hint == "null" {
            continue;
        }
        let action = extract_json_value(hint, "action")
            .map(|value| value.trim_matches('"').to_string())
            .unwrap_or_default();
        if action.is_empty() {
            continue;
        }
        let queue = extract_json_value(hint, "queue")
            .map(|value| value.trim_matches('"').to_string())
            .unwrap_or_default();
        let priority = extract_json_value(hint, "priority")
            .map(|value| value.trim_matches('"').to_string())
            .unwrap_or_default();
        if let Some(existing) = buckets.iter_mut().find(|bucket| {
            bucket.action == action && bucket.queue == queue && bucket.priority == priority
        }) {
            existing.count += 1;
            if let Some(target) = target
                && !existing.targets.iter().any(|item| item == target)
            {
                existing.targets.push((*target).to_string());
            }
        } else {
            buckets.push(QueueBucket {
                action,
                queue,
                priority,
                count: 1,
                targets: target.iter().map(|value| (*value).to_string()).collect(),
            });
        }
    }

    if buckets.is_empty() {
        return "null".to_string();
    }

    buckets.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.priority.cmp(&right.priority))
            .then_with(|| left.action.cmp(&right.action))
    });

    let total_actions = buckets.iter().map(|bucket| bucket.count).sum::<usize>();
    let top_action = &buckets[0];
    let actions = buckets
        .iter()
        .map(|bucket| {
            let targets = bucket
                .targets
                .iter()
                .map(|target| format!("\"{}\"", escape_json_string(target)))
                .collect::<Vec<_>>()
                .join(",");
            format!(
                "{{\"action\":\"{}\",\"queue\":\"{}\",\"priority\":\"{}\",\"count\":{},\"targets\":[{}]}}",
                escape_json_string(&bucket.action),
                escape_json_string(&bucket.queue),
                escape_json_string(&bucket.priority),
                bucket.count,
                targets
            )
        })
        .collect::<Vec<_>>()
        .join(",");

    format!(
        "{{\"total_actions\":{},\"top_action\":\"{}\",\"top_queue\":\"{}\",\"top_priority\":\"{}\",\"actions\":[{}]}}",
        total_actions,
        escape_json_string(&top_action.action),
        escape_json_string(&top_action.queue),
        escape_json_string(&top_action.priority),
        actions
    )
}

pub(super) fn queue_summary_json_from_targets(target_outputs: &[TargetDaemonOutput]) -> String {
    let action_hints = target_outputs
        .iter()
        .map(|target| {
            let learning_summary = learning_summary_json_from_output_and_history_with_scope(
                &target.output_json,
                &target.recommendation_summary_json,
                &target.training_history,
                None,
                "target",
            );
            let action_hint = extract_json_value(&learning_summary, "action_queue_hint")
                .unwrap_or_else(|| "null".to_string());
            (Some(target.path_segment.as_str()), action_hint)
        })
        .collect::<Vec<_>>();
    queue_summary_json_from_action_hints(&action_hints)
}

pub(super) fn queue_pressure_hint_json(queue_summary: &str) -> String {
    if queue_summary == "null" {
        return "null".to_string();
    }

    let top_queue = extract_json_value(queue_summary, "top_queue")
        .map(|value| value.trim_matches('"').to_string())
        .unwrap_or_default();
    let top_priority = extract_json_value(queue_summary, "top_priority")
        .map(|value| value.trim_matches('"').to_string())
        .unwrap_or_default();
    let total_actions = extract_json_value(queue_summary, "total_actions")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);

    if top_queue.is_empty() {
        return "null".to_string();
    }

    let (status, reason) = match top_queue.as_str() {
        "human_review" => ("review_backlog", "manual_review_is_currently_dominant"),
        "transition_watch" => (
            "transition_pressure",
            "multiple_targets_are_shifting_labels",
        ),
        "automation_ready" => ("promotion_ready", "learned_routes_look_ready_for_promotion"),
        "observation" if top_priority == "low" => {
            ("monitoring_bias", "most_targets_are_still_in_observation")
        }
        "observation" => (
            "mixed_pressure",
            "observation_queue_is_active_but_not_low_priority",
        ),
        _ => ("mixed_pressure", "queue_mix_requires_operator_attention"),
    };

    format!(
        "{{\"status\":\"{}\",\"reason\":\"{}\",\"top_queue\":\"{}\",\"top_priority\":\"{}\",\"total_actions\":{}}}",
        escape_json_string(status),
        escape_json_string(reason),
        escape_json_string(&top_queue),
        escape_json_string(&top_priority),
        total_actions
    )
}

pub(super) fn feedback_policy_hint_json(
    learning_judgement: &str,
    queue_pressure_hint: &str,
    training_conflict_hint: &str,
) -> String {
    if learning_judgement == "null" {
        return "null".to_string();
    }

    let judgement_status = extract_json_value(learning_judgement, "status")
        .map(|value| value.trim_matches('"').to_string())
        .unwrap_or_default();
    let pressure_status = extract_json_value(queue_pressure_hint, "status")
        .map(|value| value.trim_matches('"').to_string())
        .unwrap_or_default();

    let (policy, reason) = if training_conflict_hint != "null"
        || judgement_status == "manual_review"
    {
        (
            "pause_and_review",
            "competing_feedback_should_be_resolved_before_more_training",
        )
    } else if pressure_status == "promotion_ready" || judgement_status == "ready" {
        (
            "promote_and_monitor",
            "learned_route_looks_ready_for_stronger_automation",
        )
    } else if pressure_status == "transition_pressure" || judgement_status == "watch_transition" {
        (
            "collect_disambiguating_feedback",
            "learning_state_is_shifting_and_needs_more_signal",
        )
    } else if pressure_status == "monitoring_bias" || judgement_status == "observe" {
        (
            "continue_observation",
            "learning_signal_exists_but_should_mature_further",
        )
    } else {
        (
            "reinforce_current_label",
            "learning_state_is_leaning_but_not_fully_settled",
        )
    };

    format!(
        "{{\"policy\":\"{}\",\"reason\":\"{}\",\"judgement_status\":\"{}\",\"pressure_status\":{}}}",
        escape_json_string(policy),
        escape_json_string(reason),
        escape_json_string(&judgement_status),
        if pressure_status.is_empty() {
            "null".to_string()
        } else {
            format!("\"{}\"", escape_json_string(&pressure_status))
        }
    )
}
