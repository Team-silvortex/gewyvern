use super::*;

pub(super) fn merge_optional_f64(current: Option<f64>, incoming: Option<f64>) -> Option<f64> {
    match (current, incoming) {
        (Some(left), Some(right)) => Some(left.max(right)),
        (Some(left), None) => Some(left),
        (None, Some(right)) => Some(right),
        (None, None) => None,
    }
}

pub(super) fn merge_optional_u64(current: Option<u64>, incoming: Option<u64>) -> Option<u64> {
    match (current, incoming) {
        (Some(left), Some(right)) => Some(left.max(right)),
        (Some(left), None) => Some(left),
        (None, Some(right)) => Some(right),
        (None, None) => None,
    }
}

pub(super) fn merge_optional_u128(current: Option<u128>, incoming: Option<u128>) -> Option<u128> {
    match (current, incoming) {
        (Some(left), Some(right)) => Some(left.max(right)),
        (Some(left), None) => Some(left),
        (None, Some(right)) => Some(right),
        (None, None) => None,
    }
}

pub(super) fn merged_recommendation_entries(
    entries: &[(String, String)],
) -> Vec<MergedRecommendationEntry> {
    let mut merged: Vec<MergedRecommendationEntry> = Vec::new();
    for (_, output) in entries {
        let names = extract_named_string_fields(output, "name");
        let stages = extract_named_string_fields(output, "producer_stage");
        let passes = extract_named_string_fields(output, "producer_pass");
        let support_scores = extract_named_numeric_fields(output, "support_score");
        let train_counts = extract_named_numeric_fields(output, "train_count");
        let last_trained = extract_named_numeric_fields(output, "last_trained_unix_ms");
        let score_margins = extract_named_numeric_fields(output, "score_margin");
        let runner_up_labels = extract_named_string_fields(output, "runner_up_label");
        let runner_up_scores = extract_named_numeric_fields(output, "runner_up_score");
        let runner_up_train_counts = extract_named_numeric_fields(output, "runner_up_train_count");
        let runner_up_last_trained =
            extract_named_numeric_fields(output, "runner_up_last_trained_unix_ms");
        for (index, ((name, stage), pass)) in names.into_iter().zip(stages).zip(passes).enumerate()
        {
            let incoming_hints = RecommendationHints {
                support_score: support_scores
                    .get(index)
                    .and_then(|value| value.parse::<f64>().ok()),
                train_count: train_counts
                    .get(index)
                    .and_then(|value| value.parse::<u64>().ok()),
                last_trained_unix_ms: last_trained
                    .get(index)
                    .and_then(|value| value.parse::<u128>().ok()),
                score_margin: score_margins
                    .get(index)
                    .and_then(|value| value.parse::<f64>().ok()),
                runner_up_label: runner_up_labels.get(index).cloned(),
                runner_up_score: runner_up_scores
                    .get(index)
                    .and_then(|value| value.parse::<f64>().ok()),
                runner_up_train_count: runner_up_train_counts
                    .get(index)
                    .and_then(|value| value.parse::<u64>().ok()),
                runner_up_last_trained_unix_ms: runner_up_last_trained
                    .get(index)
                    .and_then(|value| value.parse::<u128>().ok()),
            };
            if let Some(existing) = merged.iter_mut().find(|item| {
                item.name == name && item.producer_stage == stage && item.producer_pass == pass
            }) {
                existing.count += 1;
                existing.hints.support_score =
                    merge_optional_f64(existing.hints.support_score, incoming_hints.support_score);
                existing.hints.train_count =
                    merge_optional_u64(existing.hints.train_count, incoming_hints.train_count);
                existing.hints.last_trained_unix_ms = merge_optional_u128(
                    existing.hints.last_trained_unix_ms,
                    incoming_hints.last_trained_unix_ms,
                );
                existing.hints.score_margin =
                    merge_optional_f64(existing.hints.score_margin, incoming_hints.score_margin);
                if existing.hints.runner_up_label.is_none() {
                    existing.hints.runner_up_label = incoming_hints.runner_up_label.clone();
                }
                existing.hints.runner_up_score = merge_optional_f64(
                    existing.hints.runner_up_score,
                    incoming_hints.runner_up_score,
                );
                existing.hints.runner_up_train_count = merge_optional_u64(
                    existing.hints.runner_up_train_count,
                    incoming_hints.runner_up_train_count,
                );
                existing.hints.runner_up_last_trained_unix_ms = merge_optional_u128(
                    existing.hints.runner_up_last_trained_unix_ms,
                    incoming_hints.runner_up_last_trained_unix_ms,
                );
            } else {
                merged.push(MergedRecommendationEntry {
                    name,
                    producer_stage: stage,
                    producer_pass: pass,
                    count: 1,
                    hints: incoming_hints,
                });
            }
        }
    }
    merged.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then(left.producer_stage.cmp(&right.producer_stage))
            .then(left.producer_pass.cmp(&right.producer_pass))
    });
    merged
}

pub(super) fn recommendation_entry_json(entry: &MergedRecommendationEntry) -> String {
    let mut json = format!(
        "{{\"name\":\"{}\",\"producer_stage\":\"{}\",\"producer_pass\":\"{}\",\"count\":{}",
        escape_json_string(&entry.name),
        escape_json_string(&entry.producer_stage),
        escape_json_string(&entry.producer_pass),
        entry.count
    );
    if let Some(value) = entry.hints.support_score {
        json.push_str(&format!(",\"support_score\":{}", value));
    }
    if let Some(value) = entry.hints.train_count {
        json.push_str(&format!(",\"train_count\":{}", value));
    }
    if let Some(value) = entry.hints.last_trained_unix_ms {
        json.push_str(&format!(",\"last_trained_unix_ms\":{}", value));
    }
    if let Some(value) = entry.hints.score_margin {
        json.push_str(&format!(",\"score_margin\":{}", value));
    }
    if let Some(value) = &entry.hints.runner_up_label {
        json.push_str(&format!(
            ",\"runner_up_label\":\"{}\"",
            escape_json_string(value)
        ));
    }
    if let Some(value) = entry.hints.runner_up_score {
        json.push_str(&format!(",\"runner_up_score\":{}", value));
    }
    if let Some(value) = entry.hints.runner_up_train_count {
        json.push_str(&format!(",\"runner_up_train_count\":{}", value));
    }
    if let Some(value) = entry.hints.runner_up_last_trained_unix_ms {
        json.push_str(&format!(",\"runner_up_last_trained_unix_ms\":{}", value));
    }
    json.push('}');
    json
}

pub(super) fn recommendation_summary_array_json(entries: &[(String, String)]) -> String {
    let body = merged_recommendation_entries(entries)
        .into_iter()
        .map(|entry| recommendation_entry_json(&entry))
        .collect::<Vec<_>>()
        .join(",");
    format!("[{}]", body)
}

pub(super) fn recommendation_overview_json(entries: &[(String, String)]) -> String {
    let merged = merged_recommendation_entries(entries);
    let recommendations = merged
        .iter()
        .map(recommendation_entry_json)
        .collect::<Vec<_>>()
        .join(",");
    let top_recommendation = merged
        .iter()
        .max_by(|left, right| {
            left.count
                .cmp(&right.count)
                .then_with(|| right.name.cmp(&left.name))
                .then_with(|| right.producer_stage.cmp(&left.producer_stage))
                .then_with(|| right.producer_pass.cmp(&left.producer_pass))
        })
        .map(recommendation_entry_json)
        .unwrap_or_else(|| "null".to_string());
    let top_candidates = merged
        .into_iter()
        .take(3)
        .map(|entry| recommendation_entry_json(&entry))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"recommendations\":[{}],\"top_recommendation\":{},\"top_candidates\":[{}]}}",
        recommendations, top_recommendation, top_candidates
    )
}

pub(super) fn learned_route_summary_from_recommendation_summary(
    summary_json: &str,
) -> (bool, usize) {
    let names = extract_named_string_fields(summary_json, "name");
    let counts = extract_named_numeric_fields(summary_json, "count");
    let mut learned_routes = 0usize;
    for (index, name) in names.into_iter().enumerate() {
        if name == "py_ml_candidate_learned_route" {
            let count = counts
                .get(index)
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(1);
            learned_routes = learned_routes.max(count);
        }
    }
    (learned_routes > 0, learned_routes)
}

pub(super) fn top_learned_state_json_from_recommendation_summary(summary_json: &str) -> String {
    let route_count = learned_route_summary_from_recommendation_summary(summary_json).1;
    let entries =
        merged_recommendation_entries(&[("latest".to_string(), summary_json.to_string())]);
    let Some(entry) = entries
        .into_iter()
        .find(|entry| entry.name == "py_ml_candidate_learned_route")
    else {
        return "null".to_string();
    };
    let support_score = entry.hints.support_score.unwrap_or(0.0);
    let train_count = entry.hints.train_count.unwrap_or(0);
    let score_margin = entry.hints.score_margin.unwrap_or(0.0);
    let has_runner_up = entry.hints.runner_up_label.is_some();
    let confidence_hint = if !has_runner_up && support_score >= 2.0 {
        "high"
    } else if !has_runner_up {
        "medium"
    } else if score_margin >= 1.5 {
        "high"
    } else if score_margin >= 0.5 {
        "medium"
    } else {
        "low"
    };
    let stability_hint = if !has_runner_up && train_count >= 2 {
        "stable"
    } else if !has_runner_up {
        "emerging"
    } else if score_margin >= 1.5 {
        "stable"
    } else if score_margin >= 0.5 {
        "leaning"
    } else {
        "contested"
    };
    let mut json = format!("{{\"route_count\":{}", route_count);
    if let Some(value) = entry.hints.support_score {
        json.push_str(&format!(",\"support_score\":{}", value));
    }
    if let Some(value) = entry.hints.train_count {
        json.push_str(&format!(",\"train_count\":{}", value));
    }
    if let Some(value) = entry.hints.last_trained_unix_ms {
        json.push_str(&format!(",\"last_trained_unix_ms\":{}", value));
    }
    if let Some(value) = entry.hints.score_margin {
        json.push_str(&format!(",\"score_margin\":{}", value));
    }
    if let Some(value) = &entry.hints.runner_up_label {
        json.push_str(&format!(
            ",\"runner_up_state\":{{\"label\":\"{}\"",
            escape_json_string(value)
        ));
        if let Some(score) = entry.hints.runner_up_score {
            json.push_str(&format!(",\"support_score\":{}", score));
        }
        if let Some(train_count) = entry.hints.runner_up_train_count {
            json.push_str(&format!(",\"train_count\":{}", train_count));
        }
        if let Some(ts) = entry.hints.runner_up_last_trained_unix_ms {
            json.push_str(&format!(",\"last_trained_unix_ms\":{}", ts));
        }
        json.push('}');
    } else {
        json.push_str(",\"runner_up_state\":null");
    }
    json.push_str(&format!(
        ",\"confidence_hint\":\"{}\",\"stability_hint\":\"{}\"",
        escape_json_string(confidence_hint),
        escape_json_string(stability_hint)
    ));
    json.push('}');
    json
}

pub(super) fn transition_policy_summary_json_for_label(label: Option<&str>) -> String {
    let Some(spec) = label.and_then(training_label_spec_for) else {
        return "null".to_string();
    };
    let compatible_count = spec.compatible_with.len();
    let competing_count = spec.competes_with.len();
    let policy_bias = match compatible_count.cmp(&competing_count) {
        std::cmp::Ordering::Greater => "compatible",
        std::cmp::Ordering::Less => "competing",
        std::cmp::Ordering::Equal if compatible_count == 0 => "neutral",
        std::cmp::Ordering::Equal => "balanced",
    };
    let compatible_labels = spec
        .compatible_with
        .iter()
        .map(|label| format!("\"{}\"", escape_json_string(label)))
        .collect::<Vec<_>>()
        .join(",");
    let competing_labels = spec
        .competes_with
        .iter()
        .map(|label| format!("\"{}\"", escape_json_string(label)))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"policy_bias\":\"{}\",\"compatible_count\":{},\"competing_count\":{},\"compatible_labels\":[{}],\"competing_labels\":[{}]}}",
        escape_json_string(policy_bias),
        compatible_count,
        competing_count,
        compatible_labels,
        competing_labels
    )
}
