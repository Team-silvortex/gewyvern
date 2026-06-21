use super::*;

pub(super) fn single_output_recommendation_summary(output_json: &str) -> String {
    recommendation_overview_json(&[("latest".to_string(), output_json.to_string())])
}

pub(super) fn daemon_snapshot_json(snapshot: &DaemonSnapshot) -> String {
    let (learning_active, learned_routes) = learned_route_summary_from_recommendation_summary(
        &snapshot.latest_recommendation_summary_json,
    );
    let last_success_unix_ms = snapshot
        .last_success_unix_ms
        .map(|value| value.to_string())
        .unwrap_or_else(|| "null".to_string());
    let last_error = snapshot
        .last_error
        .as_ref()
        .map(|value| format!("\"{}\"", escape_json_string(value)))
        .unwrap_or_else(|| "null".to_string());
    format!(
        "{{\"source\":\"{}\",\"upstream_url\":\"{}\",\"interval_ms\":{},\"cycle\":{},\"analysis_runs\":{},\"cache_hits\":{},\"target_count\":{},\"updated_unix_ms\":{},\"last_success_unix_ms\":{},\"last_error\":{},\"state_hash\":\"{}\",\"learning_active\":{},\"learned_routes\":{},\"recommendation_summary\":{},\"output\":{}}}",
        escape_json_string(&snapshot.source),
        escape_json_string(&snapshot.upstream_url),
        snapshot.interval_ms,
        snapshot.cycle,
        snapshot.analysis_runs,
        snapshot.cache_hits,
        snapshot.target_count,
        snapshot.updated_unix_ms,
        last_success_unix_ms,
        last_error,
        escape_json_string(&snapshot.state_hash),
        if learning_active { "true" } else { "false" },
        learned_routes,
        snapshot.latest_recommendation_summary_json,
        snapshot.latest_output_json
    )
}

pub(super) fn daemon_meta_json(
    snapshot: Option<&DaemonSnapshot>,
    worker_state_json: Option<&str>,
) -> String {
    let memory_state_status = worker_state_json
        .and_then(|json| extract_json_value(json, "status"))
        .unwrap_or_else(|| "null".to_string());
    let memory_model_version = worker_state_json
        .and_then(|json| extract_json_value(json, "model_version"))
        .unwrap_or_else(|| "null".to_string());
    match snapshot {
        Some(snapshot) => {
            let (learning_active, learned_routes) =
                learned_route_summary_from_recommendation_summary(
                    &snapshot.latest_recommendation_summary_json,
                );
            let status = if snapshot.last_error.is_some() {
                "degraded"
            } else {
                "ready"
            };
            let last_success_unix_ms = snapshot
                .last_success_unix_ms
                .map(|value| value.to_string())
                .unwrap_or_else(|| "null".to_string());
            let last_error = snapshot
                .last_error
                .as_ref()
                .map(|value| format!("\"{}\"", escape_json_string(value)))
                .unwrap_or_else(|| "null".to_string());
            let queue_summary_override = if snapshot.target_outputs.is_empty() {
                None
            } else {
                Some(queue_summary_json_from_targets(&snapshot.target_outputs))
            };
            let handoff_summary = handoff_summary_json(
                &snapshot.latest_output_json,
                &snapshot.latest_recommendation_summary_json,
                &snapshot.training_history,
                queue_summary_override.as_deref(),
                "latest",
            );
            format!(
                "{{\"status\":\"{}\",\"source\":\"{}\",\"upstream_url\":\"{}\",\"interval_ms\":{},\"cycle\":{},\"analysis_runs\":{},\"cache_hits\":{},\"target_count\":{},\"updated_unix_ms\":{},\"last_success_unix_ms\":{},\"last_error\":{},\"state_hash\":\"{}\",\"learning_active\":{},\"learned_routes\":{},\"memory_state_status\":{},\"memory_model_version\":{},\"handoff_summary\":{}}}",
                status,
                escape_json_string(&snapshot.source),
                escape_json_string(&snapshot.upstream_url),
                snapshot.interval_ms,
                snapshot.cycle,
                snapshot.analysis_runs,
                snapshot.cache_hits,
                snapshot.target_count,
                snapshot.updated_unix_ms,
                last_success_unix_ms,
                last_error,
                escape_json_string(&snapshot.state_hash),
                if learning_active { "true" } else { "false" },
                learned_routes,
                memory_state_status,
                memory_model_version,
                handoff_summary,
            )
        }
        None => format!(
            "{{\"status\":\"starting\",\"learning_active\":false,\"learned_routes\":0,\"memory_state_status\":{},\"memory_model_version\":{},\"handoff_summary\":{{\"source_scope\":\"latest\",\"has_evidence_chain_enrichment\":false,\"has_diagnostic_opinion\":false,\"handoff_readiness\":null,\"gewyvern_merge_hint\":null,\"primary_status\":null,\"primary_label\":null,\"summary\":null,\"enrichment_strength_band\":null,\"opinion_confidence_band\":null}}}}",
            memory_state_status, memory_model_version,
        ),
    }
}

pub(super) fn daemon_status_json(snapshot: Option<&DaemonSnapshot>) -> String {
    match snapshot {
        Some(snapshot) => {
            let (learning_active, learned_routes) =
                learned_route_summary_from_recommendation_summary(&snapshot.latest_recommendation_summary_json);
            let status = if snapshot.last_error.is_some() {
                "degraded"
            } else {
                "ready"
            };
            let last_success_unix_ms = snapshot
                .last_success_unix_ms
                .map(|value| value.to_string())
                .unwrap_or_else(|| "null".to_string());
            let last_error = snapshot
                .last_error
                .as_ref()
                .map(|value| format!("\"{}\"", escape_json_string(value)))
                .unwrap_or_else(|| "null".to_string());
            format!(
                "{{\"status\":\"{}\",\"source\":\"{}\",\"cycle\":{},\"analysis_runs\":{},\"cache_hits\":{},\"target_count\":{},\"updated_unix_ms\":{},\"last_success_unix_ms\":{},\"last_error\":{},\"learning_active\":{},\"learned_routes\":{}}}",
                status,
                escape_json_string(&snapshot.source),
                snapshot.cycle,
                snapshot.analysis_runs,
                snapshot.cache_hits,
                snapshot.target_count,
                snapshot.updated_unix_ms,
                last_success_unix_ms,
                last_error,
                if learning_active { "true" } else { "false" },
                learned_routes
            )
        }
        None => {
            "{\"status\":\"starting\",\"last_success_unix_ms\":null,\"last_error\":null,\"learning_active\":false,\"learned_routes\":0}"
                .to_string()
        }
    }
}

pub(super) fn daemon_memory_state_json(
    worker_state_json: &str,
    snapshot: Option<&DaemonSnapshot>,
) -> String {
    let resident_training_event_count = snapshot
        .map(|snapshot| snapshot.training_history.len())
        .unwrap_or(0);
    let resident_target_training_event_count = snapshot
        .map(|snapshot| {
            snapshot
                .target_outputs
                .iter()
                .map(|target| target.training_history.len())
                .sum::<usize>()
        })
        .unwrap_or(0);
    format!(
        "{{\"worker\":{},\"resident_training_event_count\":{},\"resident_target_training_event_count\":{}}}",
        worker_state_json, resident_training_event_count, resident_target_training_event_count
    )
}

fn target_memory_flags(target: &TargetDaemonOutput) -> (bool, bool) {
    let has_memory_state = extract_json_value(&target.output_json, "pattern_memory_state")
        .map(|value| value != "null")
        .unwrap_or(false);
    let memory_learning_active = has_memory_state || !target.training_history.is_empty();
    (has_memory_state, memory_learning_active)
}

pub(super) fn daemon_target_index_json(snapshot: &DaemonSnapshot) -> String {
    let now_ms = now_unix_ms().unwrap_or(snapshot.updated_unix_ms);
    let refs = snapshot
        .target_outputs
        .iter()
        .map(|target| {
            let (learning_active, learned_routes) =
                learned_route_summary_from_recommendation_summary(&target.recommendation_summary_json);
            let (has_memory_state, memory_learning_active) = target_memory_flags(target);
            let stale_after_ms = u128::from(snapshot.interval_ms) * 3;
            let basis_ms = target.last_success_unix_ms.unwrap_or(target.updated_unix_ms);
            let stale = target
                .last_error
                .as_ref()
                .map(|_| target.last_success_unix_ms.is_none())
                .unwrap_or(false)
                || now_ms.saturating_sub(basis_ms) > stale_after_ms;
            let stale_for_ms = if stale {
                now_ms.saturating_sub(basis_ms).to_string()
            } else {
                "null".to_string()
            };
            let handoff_summary = handoff_summary_json(
                &target.output_json,
                &target.recommendation_summary_json,
                &target.training_history,
                None,
                "target",
            );
            format!(
                "{{\"path_segment\":\"{}\",\"url_path\":\"/v1/latest/targets/{}/output.json\",\"meta_url_path\":\"/v1/latest/targets/{}/meta.json\",\"updated_unix_ms\":{},\"state_hash\":\"{}\",\"last_success_unix_ms\":{},\"last_error\":{},\"stale\":{},\"stale_after_ms\":{},\"stale_for_ms\":{},\"learning_active\":{},\"learned_routes\":{},\"has_memory_state\":{},\"memory_learning_active\":{},\"handoff_summary\":{}}}",
                escape_json_string(&target.path_segment),
                escape_json_string(&target.path_segment),
                escape_json_string(&target.path_segment),
                target.updated_unix_ms,
                escape_json_string(&target.state_hash),
                target
                    .last_success_unix_ms
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "null".to_string()),
                target
                    .last_error
                    .as_ref()
                    .map(|value| format!("\"{}\"", escape_json_string(value)))
                    .unwrap_or_else(|| "null".to_string()),
                if stale { "true" } else { "false" },
                stale_after_ms,
                stale_for_ms,
                if learning_active { "true" } else { "false" },
                learned_routes,
                if has_memory_state { "true" } else { "false" },
                if memory_learning_active { "true" } else { "false" },
                handoff_summary,
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"target_count\":{},\"target_refs\":[{}]}}",
        snapshot.target_outputs.len(),
        refs
    )
}

pub(super) fn target_daemon_meta_json(target: &TargetDaemonOutput, interval_ms: u64) -> String {
    let (learning_active, learned_routes) =
        learned_route_summary_from_recommendation_summary(&target.recommendation_summary_json);
    let (has_memory_state, memory_learning_active) = target_memory_flags(target);
    let now_ms = now_unix_ms().unwrap_or(target.updated_unix_ms);
    let stale_after_ms = u128::from(interval_ms) * 3;
    let basis_ms = target
        .last_success_unix_ms
        .unwrap_or(target.updated_unix_ms);
    let stale = target
        .last_error
        .as_ref()
        .map(|_| target.last_success_unix_ms.is_none())
        .unwrap_or(false)
        || now_ms.saturating_sub(basis_ms) > stale_after_ms;
    let stale_for_ms = if stale {
        now_ms.saturating_sub(basis_ms).to_string()
    } else {
        "null".to_string()
    };
    let handoff_summary = handoff_summary_json(
        &target.output_json,
        &target.recommendation_summary_json,
        &target.training_history,
        None,
        "target",
    );
    format!(
        "{{\"path_segment\":\"{}\",\"updated_unix_ms\":{},\"state_hash\":\"{}\",\"last_success_unix_ms\":{},\"last_error\":{},\"stale\":{},\"stale_after_ms\":{},\"stale_for_ms\":{},\"learning_active\":{},\"learned_routes\":{},\"has_memory_state\":{},\"memory_learning_active\":{},\"handoff_summary\":{}}}",
        escape_json_string(&target.path_segment),
        target.updated_unix_ms,
        escape_json_string(&target.state_hash),
        target
            .last_success_unix_ms
            .map(|value| value.to_string())
            .unwrap_or_else(|| "null".to_string()),
        target
            .last_error
            .as_ref()
            .map(|value| format!("\"{}\"", escape_json_string(value)))
            .unwrap_or_else(|| "null".to_string()),
        if stale { "true" } else { "false" },
        stale_after_ms,
        stale_for_ms,
        if learning_active { "true" } else { "false" },
        learned_routes,
        if has_memory_state { "true" } else { "false" },
        if memory_learning_active {
            "true"
        } else {
            "false"
        },
        handoff_summary,
    )
}

pub(super) fn now_unix_ms() -> Result<u128, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .map_err(|err| format!("system clock is before unix epoch: {err}"))
}

pub(super) fn state_hash_for_output(
    output_json: &str,
    recommendation_summary_json: &str,
) -> String {
    let mut hasher = DefaultHasher::new();
    output_json.hash(&mut hasher);
    recommendation_summary_json.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

pub(super) fn enrich_target_outputs(
    target_outputs: Vec<TargetDaemonOutput>,
    updated_unix_ms: u128,
) -> Vec<TargetDaemonOutput> {
    target_outputs
        .into_iter()
        .map(|target| {
            let state_hash =
                state_hash_for_output(&target.output_json, &target.recommendation_summary_json);
            let last_success_unix_ms = if target.output_json == "null" {
                None
            } else {
                Some(updated_unix_ms)
            };
            TargetDaemonOutput {
                updated_unix_ms,
                state_hash,
                last_success_unix_ms,
                ..target
            }
        })
        .collect()
}

pub(super) fn daemon_http_response(status_line: &str, body: &str) -> String {
    format!(
        "{status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    )
}

pub(super) fn parse_training_feedback(request_text: &str) -> Result<(String, f64), String> {
    let (_, body) = request_text
        .split_once("\r\n\r\n")
        .ok_or_else(|| "invalid training request: missing body".to_string())?;
    let label_needle = "\"label\":\"";
    let label_index = body
        .find(label_needle)
        .ok_or_else(|| "invalid training request: missing label".to_string())?;
    let label_tail = &body[label_index + label_needle.len()..];
    let label_end = label_tail
        .find('"')
        .ok_or_else(|| "invalid training request: unterminated label".to_string())?;
    let label = normalize_training_label(&label_tail[..label_end])?;
    let weight = if let Some(weight_index) = body.find("\"weight\":") {
        let weight_tail = &body[weight_index + "\"weight\":".len()..];
        let weight_end = weight_tail
            .find(|ch: char| ch == ',' || ch == '}' || ch.is_whitespace())
            .unwrap_or(weight_tail.len());
        weight_tail[..weight_end]
            .trim()
            .parse::<f64>()
            .map_err(|_| "invalid training request: bad weight".to_string())?
    } else {
        1.0
    };
    if weight <= 0.0 {
        return Err("invalid training request: weight must be > 0".to_string());
    }
    Ok((label, weight))
}

pub(super) fn normalize_training_label(input: &str) -> Result<String, String> {
    let normalized = input.trim().to_ascii_lowercase().replace('-', "_");
    training_label_specs()
        .into_iter()
        .find(|spec| spec.canonical == normalized || spec.aliases.iter().any(|alias| *alias == normalized))
        .map(|spec| spec.canonical.to_string())
        .ok_or_else(|| {
            format!(
                "unknown training label '{}'; expected one of: network_observe_longer, targeted_escalation, http_request_followup",
                input
            )
        })
}

pub(super) fn training_label_spec_for(canonical: &str) -> Option<&'static TrainingLabelSpec> {
    training_label_specs()
        .iter()
        .find(|spec| spec.canonical == canonical)
}

#[derive(Clone, Copy)]
pub(super) struct TrainingLabelSpec {
    pub(super) canonical: &'static str,
    pub(super) aliases: &'static [&'static str],
    pub(super) summary: &'static str,
    pub(super) recommended_for: &'static str,
    pub(super) compatible_with: &'static [&'static str],
    pub(super) competes_with: &'static [&'static str],
}

pub(super) fn training_label_specs() -> &'static [TrainingLabelSpec] {
    &[
        TrainingLabelSpec {
            canonical: "network_observe_longer",
            aliases: &[
                "observe_longer",
                "timeout_observe_longer",
                "collect_more_runtime_evidence",
            ],
            summary: "keep observing a timeout-shaped path before taking stronger action",
            recommended_for: "missing-transition or timeout-shaped failures that still need more runtime evidence",
            compatible_with: &["http_request_followup"],
            competes_with: &["targeted_escalation"],
        },
        TrainingLabelSpec {
            canonical: "targeted_escalation",
            aliases: &[
                "escalate",
                "protocol_escalation",
                "safe_to_escalate_protocol_signal",
            ],
            summary: "promote a direct protocol signal into a stronger escalation path",
            recommended_for: "high-confidence direct protocol signals that are ready for downstream routing",
            compatible_with: &[],
            competes_with: &["network_observe_longer", "http_request_followup"],
        },
        TrainingLabelSpec {
            canonical: "http_request_followup",
            aliases: &["request_followup", "followup_request", "http_followup"],
            summary: "bias follow-up handling toward HTTP request/response investigation",
            recommended_for: "HTTP-shaped request flows where operators want a stable follow-up route",
            compatible_with: &["network_observe_longer"],
            competes_with: &["targeted_escalation"],
        },
    ]
}

pub(super) fn training_labels_json() -> String {
    let body = training_label_specs()
        .iter()
        .map(|spec| {
            let aliases = spec
                .aliases
                .iter()
                .map(|alias| format!("\"{}\"", escape_json_string(alias)))
                .collect::<Vec<_>>()
                .join(",");
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
                "{{\"canonical\":\"{}\",\"aliases\":[{}],\"summary\":\"{}\",\"recommended_for\":\"{}\",\"compatible_with\":[{}],\"competes_with\":[{}]}}",
                escape_json_string(spec.canonical),
                aliases,
                escape_json_string(spec.summary),
                escape_json_string(spec.recommended_for),
                compatible_with,
                competes_with,
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("{{\"labels\":[{}]}}", body)
}
