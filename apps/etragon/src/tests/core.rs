use super::*;

#[test]
fn cli_reports_usage_for_missing_arguments() {
    let err = run_cli(&[]).expect_err("missing args should fail");
    assert!(err.contains("usage: etragon analyze-json"));
    assert!(err.contains("train-json"));
    assert!(err.contains("memory-info"));
    assert!(err.contains("python-memory-model-info"));
    assert!(err.contains("protocol-capabilities"));
}

#[test]
fn cli_renders_training_label_dictionary() {
    let output =
        run_cli(&["training-labels".to_string()]).expect("training-labels command should succeed");
    assert!(output.contains("\"canonical\":\"network_observe_longer\""));
    assert!(output.contains("\"aliases\":["));
    assert!(output.contains("\"recommended_for\":"));
    assert!(output.contains("\"compatible_with\":[\"http_request_followup\"]"));
    assert!(output.contains("\"competes_with\":[\"targeted_escalation\"]"));
}

#[test]
fn training_label_aliases_normalize_to_canonical_labels() {
    assert_eq!(
        normalize_training_label("observe-longer").expect("alias should normalize"),
        "network_observe_longer"
    );
    assert_eq!(
        normalize_training_label("escalate").expect("alias should normalize"),
        "targeted_escalation"
    );
    assert_eq!(
        normalize_training_label("request_followup").expect("alias should normalize"),
        "http_request_followup"
    );
}

#[test]
fn training_label_parser_rejects_unknown_labels() {
    let err = normalize_training_label("totally_unknown").expect_err("unknown label should fail");
    assert!(err.contains("unknown training label"));
}

#[test]
fn training_conflict_hint_detects_competing_recent_labels() {
    let history = vec![
        TrainingEvent {
            label: "network_observe_longer".to_string(),
            weight: "1.0".to_string(),
            trained_unix_ms: 100,
            scope: "latest".to_string(),
        },
        TrainingEvent {
            label: "targeted_escalation".to_string(),
            weight: "2.5".to_string(),
            trained_unix_ms: 200,
            scope: "latest".to_string(),
        },
    ];
    let hint = training_conflict_hint_json(&history);
    assert!(hint.contains("\"status\":\"competing_labels\""));
    assert!(hint.contains("\"primary_label\":\"targeted_escalation\""));
    assert!(hint.contains("\"competing_label\":\"network_observe_longer\""));
}

#[test]
fn pattern_memory_summary_ranks_labels_for_latest_shape() {
    let state = "{\"pattern_key\":\"demo\",\"label_count\":2,\"labels\":[{\"label\":\"targeted_escalation\",\"support_score\":2.5},{\"label\":\"network_observe_longer\",\"support_score\":1.0}]}";
    let summary = pattern_memory_summary_json(state, Some("targeted_escalation"));
    assert!(summary.contains("\"label_count\":2"));
    assert!(summary.contains("\"top_pattern_label\":\"targeted_escalation\""));
    assert!(summary.contains("\"runner_up_label\":\"network_observe_longer\""));
    assert!(summary.contains("\"learned_label_rank\":1"));
}

#[test]
fn memory_drift_hint_detects_switching_pattern_preference() {
    let top_state = "{\"route_count\":1,\"support_score\":1.0,\"train_count\":1,\"score_margin\":0.4,\"runner_up_state\":{\"label\":\"targeted_escalation\"},\"confidence_hint\":\"low\",\"stability_hint\":\"emerging\"}";
    let pattern_summary = "{\"label_count\":2,\"top_pattern_label\":\"targeted_escalation\",\"top_pattern_support_score\":2.5,\"runner_up_label\":\"network_observe_longer\",\"top_labels\":[\"targeted_escalation\",\"network_observe_longer\"],\"learned_label_rank\":2}";
    let hint = memory_drift_hint_json(
        Some("network_observe_longer"),
        top_state,
        pattern_summary,
        "null",
    );
    assert!(hint.contains("\"status\":\"switching\""));
    assert!(hint.contains("\"reason\":\"pattern_memory_prefers_different_label\""));
    assert!(hint.contains("\"primary_label\":\"network_observe_longer\""));
    assert!(hint.contains("\"top_pattern_label\":\"targeted_escalation\""));
}

#[test]
fn learning_judgement_prefers_manual_review_for_competing_feedback() {
    let top_state = "{\"route_count\":1,\"support_score\":1.0,\"train_count\":1,\"score_margin\":0.3,\"runner_up_state\":{\"label\":\"targeted_escalation\"},\"confidence_hint\":\"low\",\"stability_hint\":\"contested\"}";
    let drift_hint = "{\"status\":\"conflicted\",\"reason\":\"recent_competing_training_feedback\",\"primary_label\":\"network_observe_longer\"}";
    let conflict_hint = "{\"status\":\"competing_labels\",\"primary_label\":\"network_observe_longer\",\"competing_label\":\"targeted_escalation\"}";
    let judgement = learning_judgement_json(
        Some("network_observe_longer"),
        top_state,
        conflict_hint,
        drift_hint,
    );
    assert!(judgement.contains("\"status\":\"manual_review\""));
    assert!(judgement.contains("\"reason\":\"recent_training_feedback_is_competing\""));
    assert!(judgement.contains("\"drift_status\":\"conflicted\""));
}

#[test]
fn action_queue_hint_promotes_manual_review_for_conflicted_learning() {
    let judgement = "{\"status\":\"manual_review\",\"reason\":\"recent_training_feedback_is_competing\",\"primary_label\":\"network_observe_longer\"}";
    let hint = action_queue_hint_json(judgement, Some("network_observe_longer"));
    assert!(hint.contains("\"action\":\"manual_review\""));
    assert!(hint.contains("\"queue\":\"human_review\""));
    assert!(hint.contains("\"priority\":\"high\""));
    assert!(hint.contains("\"reason\":\"recent_training_feedback_is_competing\""));
}

#[test]
fn queue_summary_groups_action_hints_by_queue() {
    let summary = queue_summary_json_from_action_hints(&[
        (
            Some("scan:http:request"),
            "{\"action\":\"keep_observing\",\"queue\":\"observation\",\"priority\":\"low\",\"primary_label\":\"network_observe_longer\"}".to_string(),
        ),
        (
            Some("socket_session"),
            "{\"action\":\"keep_observing\",\"queue\":\"observation\",\"priority\":\"low\",\"primary_label\":\"network_observe_longer\"}".to_string(),
        ),
        (
            Some("scan:proxy"),
            "{\"action\":\"manual_review\",\"queue\":\"human_review\",\"priority\":\"high\",\"primary_label\":\"targeted_escalation\"}".to_string(),
        ),
    ]);
    assert!(summary.contains("\"total_actions\":3"));
    assert!(summary.contains("\"top_action\":\"keep_observing\""));
    assert!(summary.contains("\"top_queue\":\"observation\""));
    assert!(summary.contains("\"count\":2"));
    assert!(summary.contains("\"targets\":[\"scan:http:request\",\"socket_session\"]"));
}

#[test]
fn queue_pressure_hint_detects_human_review_backlog() {
    let summary = "{\"total_actions\":3,\"top_action\":\"manual_review\",\"top_queue\":\"human_review\",\"top_priority\":\"high\",\"actions\":[{\"action\":\"manual_review\",\"queue\":\"human_review\",\"priority\":\"high\",\"count\":2,\"targets\":[\"scan:http:request\",\"socket_session\"]}]}";
    let hint = queue_pressure_hint_json(summary);
    assert!(hint.contains("\"status\":\"review_backlog\""));
    assert!(hint.contains("\"reason\":\"manual_review_is_currently_dominant\""));
    assert!(hint.contains("\"top_queue\":\"human_review\""));
}

#[test]
fn feedback_policy_hint_prefers_pause_for_conflicting_feedback() {
    let judgement =
        "{\"status\":\"manual_review\",\"reason\":\"recent_training_feedback_is_competing\"}";
    let pressure =
        "{\"status\":\"review_backlog\",\"reason\":\"manual_review_is_currently_dominant\"}";
    let conflict = "{\"status\":\"competing_labels\",\"primary_label\":\"network_observe_longer\",\"competing_label\":\"targeted_escalation\"}";
    let hint = feedback_policy_hint_json(judgement, pressure, conflict);
    assert!(hint.contains("\"policy\":\"pause_and_review\""));
    assert!(
        hint.contains("\"reason\":\"competing_feedback_should_be_resolved_before_more_training\"")
    );
    assert!(hint.contains("\"judgement_status\":\"manual_review\""));
    assert!(hint.contains("\"pressure_status\":\"review_backlog\""));
}

#[test]
fn evidence_chain_enrichment_summarizes_learned_route_state() {
    let top_state = "{\"route_count\":1,\"support_score\":2.5,\"train_count\":3,\"score_margin\":1.8,\"confidence_hint\":\"high\",\"stability_hint\":\"stable\",\"runner_up_state\":null}";
    let drift_hint = "{\"status\":\"stable\",\"reason\":\"lead_is_clear_and_recently_reinforced\"}";
    let pressure_hint =
        "{\"status\":\"promotion_ready\",\"reason\":\"learned_routes_look_ready_for_promotion\"}";
    let feedback_hint = "{\"policy\":\"promote_and_monitor\",\"reason\":\"learned_route_looks_ready_for_stronger_automation\"}";
    let label_activity = "[{\"label\":\"targeted_escalation\",\"event_count\":3,\"total_weight\":4.5,\"last_trained_unix_ms\":1234,\"scopes\":[\"latest\"]}]";
    let enrichment = evidence_chain_enrichment_json(
        Some("targeted_escalation"),
        top_state,
        "null",
        drift_hint,
        pressure_hint,
        feedback_hint,
        label_activity,
    );
    assert!(enrichment.contains("\"status\":\"reinforced\""));
    assert!(enrichment.contains("\"primary_label\":\"targeted_escalation\""));
    assert!(enrichment.contains("\"feedback_policy\":\"promote_and_monitor\""));
    assert!(enrichment.contains("\"recent_event_count\":3"));
    assert!(enrichment.contains("\"enrichment_strength_band\":\"high\""));
    assert!(enrichment.contains("\"handoff_readiness\":\"automation_worthy\""));
    assert!(
        enrichment
            .contains("\"gewyvern_merge_hint\":\"augmentations_with_operator_guidance_support\"")
    );
}

#[test]
fn diagnostic_opinion_requires_stable_directive_state() {
    let stable_state = "{\"route_count\":1,\"support_score\":2.5,\"train_count\":3,\"score_margin\":1.8,\"confidence_hint\":\"high\",\"stability_hint\":\"stable\",\"runner_up_state\":null}";
    let ready_judgement =
        "{\"status\":\"ready\",\"reason\":\"learned_route_lead_is_clear_and_stable\"}";
    let opinion = diagnostic_opinion_json(
        Some("targeted_escalation"),
        stable_state,
        ready_judgement,
        "null",
        "latest",
    );
    assert!(opinion.contains("\"status\":\"ready\""));
    assert!(opinion.contains("\"diagnosis_kind\":\"direct_protocol_failure\""));
    assert!(opinion.contains("\"source_scope\":\"latest\""));
    assert!(opinion.contains("\"opinion_confidence_band\":\"high\""));
    assert!(opinion.contains("\"handoff_readiness\":\"automation_worthy\""));
    assert!(opinion.contains("\"gewyvern_merge_hint\":\"operator_guidance_candidate\""));

    let early_state = "{\"route_count\":1,\"support_score\":1.0,\"train_count\":1,\"score_margin\":1.0,\"confidence_hint\":\"medium\",\"stability_hint\":\"emerging\",\"runner_up_state\":null}";
    let observe_judgement =
        "{\"status\":\"observe\",\"reason\":\"learned_route_is_present_but_still_early\"}";
    assert_eq!(
        diagnostic_opinion_json(
            Some("network_observe_longer"),
            early_state,
            observe_judgement,
            "null",
            "latest",
        ),
        "null"
    );
}
