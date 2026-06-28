use super::*;

#[test]
fn daemon_training_route_invalidates_cache_and_emits_learned_route() {
    let _guard = lock_daemon_test_guard();
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after unix epoch")
        .as_nanos();
    let state_path = std::env::temp_dir().join(format!("etragon-daemon-train-state-{unique}.json"));
    let bind_addr = reserve_bind_addr();
    let bind_addr_for_thread = bind_addr.clone();
    let stop = Arc::new(AtomicBool::new(false));
    let stop_for_thread = Arc::clone(&stop);
    let worker_config = PythonWorkerConfig {
        state_file: Some(state_path.clone()),
        ..PythonWorkerConfig::default()
    };
    let daemon = thread::spawn(move || {
        run_python_daemon_until(
            &bind_addr_for_thread,
            10,
            &worker_config,
            None,
            "python-url",
            "http://example.test/v1/latest/analysis.json",
            |_, worker| {
                let analysis_json = fixture("missing_transition_analysis.json");
                let output_json = worker.analyze_json(&analysis_json)?;
                Ok(PolledDaemonOutput {
                    input_fingerprint: analysis_json.clone(),
                    latest_input_json: Some(analysis_json),
                    recommendation_summary_json: single_output_recommendation_summary(&output_json),
                    output_json,
                    target_outputs: Vec::new(),
                })
            },
            stop_for_thread,
        )
    });

    wait_for_daemon_health(&bind_addr).expect("daemon should publish health endpoint");
    wait_for_daemon_ready(&bind_addr).expect("daemon should publish ready status");

    wait_for_body(
        &format!("http://{}/v1/latest/output.json", bind_addr),
        |body| body.contains("\"py_ml_candidate_observe_longer\""),
    )
    .expect("daemon should publish initial output");

    let trained = post_json(
        &format!("http://{}/v1/train/latest", bind_addr),
        "{\"label\":\"network_observe_longer\"}",
    )
    .expect("daemon should accept training request");
    assert!(trained.contains("\"status\":\"trained\""));

    let retrained = wait_for_body(
        &format!("http://{}/v1/latest/output.json", bind_addr),
        |body| body.contains("\"py_ml_candidate_learned_route\""),
    )
    .expect("daemon should republish learned route after training");
    assert!(retrained.contains("\"learned_label\":\"network_observe_longer\""));

    let summary = wait_for_body(
        &format!("http://{}/v1/latest/recommendation-summary.json", bind_addr),
        |body| body.contains("\"train_count\":1"),
    )
    .expect("daemon should expose enriched recommendation summary");
    assert!(summary.contains("\"support_score\":"));
    assert!(summary.contains("\"train_count\":1"));
    assert!(summary.contains("\"last_trained_unix_ms\":"));
    assert!(summary.contains("\"score_margin\":"));

    let learning = wait_for_body(
        &format!("http://{}/v1/latest/learning-summary.json", bind_addr),
        |body| body.contains("\"top_learned_label\":\"network_observe_longer\""),
    )
    .expect("daemon should expose enriched learning summary");
    assert!(learning.contains("\"learning_active\":true"));
    assert!(learning.contains("\"learned_routes\":1"));
    assert!(learning.contains("\"top_learned_route\":\"py_ml_candidate_learned_route\""));
    assert!(learning.contains("\"top_learned_label\":\"network_observe_longer\""));
    assert!(learning.contains("\"top_learned_relationships\":{\"compatible_with\":[\"http_request_followup\"],\"competes_with\":[\"targeted_escalation\"]}"));
    assert!(learning.contains("\"top_learned_state\":{\"route_count\":1"));
    assert!(learning.contains("\"support_score\":1"));
    assert!(learning.contains("\"runner_up_state\":null"));
    assert!(learning.contains("\"confidence_hint\":\"medium\""));
    assert!(learning.contains("\"stability_hint\":\"emerging\""));
    assert!(learning.contains("\"transition_policy_summary\":{\"policy_bias\":\"balanced\",\"compatible_count\":1,\"competing_count\":1"));
    assert!(learning.contains("\"training_conflict_hint\":null"));
    assert!(learning.contains("\"pattern_memory_state\":{\"pattern_key\":"));
    assert!(learning.contains("\"pattern_memory_summary\":{\"label_count\":1,\"top_pattern_label\":\"network_observe_longer\""));
    assert!(learning.contains("\"memory_drift_hint\":{\"status\":\"emerging\""));
    assert!(learning.contains("\"reason\":\"learning_signal_is_still_early\""));
    assert!(learning.contains("\"learning_judgement\":{\"status\":\"observe\""));
    assert!(learning.contains("\"reason\":\"learned_route_is_present_but_still_early\""));
    assert!(learning.contains("\"action_queue_hint\":{\"action\":\"keep_observing\""));
    assert!(learning.contains("\"queue\":\"observation\""));
    assert!(learning.contains("\"queue_summary\":{\"total_actions\":1"));
    assert!(learning.contains("\"top_action\":\"keep_observing\""));
    assert!(learning.contains("\"queue_pressure_hint\":{\"status\":\"monitoring_bias\""));
    assert!(learning.contains("\"feedback_policy_hint\":{\"policy\":\"continue_observation\""));
    assert!(learning.contains("\"evidence_chain_enrichment\":{\"status\":\"emerging\""));
    assert!(learning.contains("\"enrichment_strength_band\":\"low\""));
    assert!(learning.contains("\"handoff_readiness\":\"advisory_only\""));
    assert!(learning.contains("\"gewyvern_merge_hint\":\"augmentations_only\""));
    assert!(learning.contains("\"primary_label\":\"network_observe_longer\""));
    assert!(learning.contains("\"diagnostic_opinion\":null"));
    assert!(learning.contains("\"learned_label_rank\":1"));
    assert!(learning.contains("\"label_count\":1"));
    assert!(learning.contains("\"labels\":[{\"label\":\"network_observe_longer\""));
    assert!(learning.contains("\"recent_training_events\":["));
    assert!(learning.contains("\"recent_label_activity\":[{\"label\":\"network_observe_longer\",\"event_count\":1,\"total_weight\":1"));
    assert!(learning.contains("\"label\":\"network_observe_longer\""));
    assert!(learning.contains("\"scope\":\"latest\""));

    let enrichment = wait_for_body(
        &format!(
            "http://{}/v1/latest/evidence-chain-enrichment.json",
            bind_addr
        ),
        |body| body.contains("\"primary_label\":\"network_observe_longer\""),
    )
    .expect("daemon should expose standalone evidence-chain enrichment");
    assert!(enrichment.contains("\"status\":\"emerging\""));
    assert!(enrichment.contains("\"enrichment_strength_band\":\"low\""));
    assert!(enrichment.contains("\"handoff_readiness\":\"advisory_only\""));
    assert!(enrichment.contains("\"gewyvern_merge_hint\":\"augmentations_only\""));
    assert!(enrichment.contains("\"primary_label\":\"network_observe_longer\""));

    let opinion = read_url(&format!(
        "http://{}/v1/latest/diagnostic-opinion.json",
        bind_addr
    ))
    .expect("daemon should expose standalone diagnostic opinion");
    assert_eq!(opinion, "null");

    let meta = read_url(&format!("http://{}/v1/latest/meta", bind_addr))
        .expect("daemon should expose learned-route activity in meta");
    assert!(meta.contains("\"learning_active\":true"));
    assert!(meta.contains("\"learned_routes\":1"));
    assert!(
        meta.contains("\"memory_state_status\":\"ready\"")
            && meta.contains("\"memory_model_version\":\"python-online-memory-v1\"")
    );
    assert!(meta.contains("\"handoff_summary\":{"));
    assert!(meta.contains("\"has_evidence_chain_enrichment\":true"));
    assert!(meta.contains("\"has_diagnostic_opinion\":false"));

    let status = read_url(&format!("http://{}/v1/latest/status", bind_addr))
        .expect("daemon should expose learned-route activity in status");
    assert!(status.contains("\"learning_active\":true"));
    assert!(status.contains("\"learned_routes\":1"));

    let handoff = wait_for_body(
        &format!("http://{}/v1/latest/handoff-summary.json", bind_addr),
        |body| body.contains("\"has_evidence_chain_enrichment\":true"),
    )
    .expect("daemon should expose latest handoff summary");
    assert!(handoff.contains("\"source_scope\":\"latest\""));
    assert!(handoff.contains("\"has_evidence_chain_enrichment\":true"));
    assert!(handoff.contains("\"has_diagnostic_opinion\":false"));
    assert!(handoff.contains("\"handoff_readiness\":\"advisory_only\""));
    assert!(handoff.contains("\"gewyvern_merge_hint\":\"augmentations_only\""));
    assert!(handoff.contains("\"enrichment_strength_band\":\"low\""));

    stop.store(true, Ordering::Relaxed);
    daemon
        .join()
        .expect("daemon thread should join")
        .expect("daemon should exit cleanly");
    let _ = fs::remove_file(state_path);
}

#[test]
fn daemon_diagnostic_opinion_route_emits_ready_direct_protocol_failure() {
    let _guard = lock_daemon_test_guard();
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after unix epoch")
        .as_nanos();
    let state_path =
        std::env::temp_dir().join(format!("etragon-daemon-opinion-state-{unique}.json"));
    let bind_addr = reserve_bind_addr();
    let bind_addr_for_thread = bind_addr.clone();
    let stop = Arc::new(AtomicBool::new(false));
    let stop_for_thread = Arc::clone(&stop);
    let worker_config = PythonWorkerConfig {
        state_file: Some(state_path.clone()),
        ..PythonWorkerConfig::default()
    };
    let daemon = thread::spawn(move || {
        run_python_daemon_until(
            &bind_addr_for_thread,
            10,
            &worker_config,
            None,
            "python-url",
            "http://example.test/v1/latest/analysis.json",
            |_, worker| {
                let analysis_json = fixture("direct_signal_analysis.json");
                let output_json = worker.analyze_json(&analysis_json)?;
                Ok(PolledDaemonOutput {
                    input_fingerprint: analysis_json.clone(),
                    latest_input_json: Some(analysis_json),
                    recommendation_summary_json: single_output_recommendation_summary(&output_json),
                    output_json,
                    target_outputs: Vec::new(),
                })
            },
            stop_for_thread,
        )
    });

    wait_for_daemon_health(&bind_addr).expect("daemon should publish health endpoint");
    wait_for_daemon_ready(&bind_addr).expect("daemon should publish ready status");

    wait_for_body(
        &format!("http://{}/v1/latest/output.json", bind_addr),
        |body| body.contains("\"py_ml_candidate_targeted_escalation\""),
    )
    .expect("daemon should publish initial direct-signal output");

    let trained = post_json(
        &format!("http://{}/v1/train/latest", bind_addr),
        "{\"label\":\"targeted_escalation\"}",
    )
    .expect("daemon should accept first targeted-escalation training request");
    assert!(trained.contains("\"status\":\"trained\""));

    let trained_again = post_json(
        &format!("http://{}/v1/train/latest", bind_addr),
        "{\"label\":\"targeted_escalation\"}",
    )
    .expect("daemon should accept second targeted-escalation training request");
    assert!(trained_again.contains("\"status\":\"trained\""));

    let opinion = wait_for_body(
        &format!("http://{}/v1/latest/diagnostic-opinion.json", bind_addr),
        |body| body.contains("\"diagnosis_kind\":\"direct_protocol_failure\""),
    )
    .expect("daemon should publish a stable direct-protocol diagnostic opinion");
    assert!(opinion.contains("\"status\":\"ready\""));
    assert!(opinion.contains("\"label\":\"targeted_escalation\""));
    assert!(opinion.contains("\"diagnosis_kind\":\"direct_protocol_failure\""));
    assert!(opinion.contains("\"source_scope\":\"latest\""));
    assert!(opinion.contains("\"opinion_confidence_band\":\"high\""));
    assert!(opinion.contains("\"handoff_readiness\":\"automation_worthy\""));
    assert!(opinion.contains("\"gewyvern_merge_hint\":\"operator_guidance_candidate\""));

    let enrichment = wait_for_body(
        &format!(
            "http://{}/v1/latest/evidence-chain-enrichment.json",
            bind_addr
        ),
        |body| body.contains("\"status\":\"reinforced\""),
    )
    .expect("daemon should expose reinforced evidence-chain enrichment");
    assert!(enrichment.contains("\"status\":\"reinforced\""));
    assert!(enrichment.contains("\"enrichment_strength_band\":\"high\""));
    assert!(enrichment.contains("\"handoff_readiness\":\"automation_worthy\""));
    assert!(
        enrichment
            .contains("\"gewyvern_merge_hint\":\"augmentations_with_operator_guidance_support\"")
    );
    assert!(enrichment.contains("\"primary_label\":\"targeted_escalation\""));

    let handoff = wait_for_body(
        &format!("http://{}/v1/latest/handoff-summary.json", bind_addr),
        |body| body.contains("\"has_diagnostic_opinion\":true"),
    )
    .expect("daemon should expose latest handoff summary");
    assert!(handoff.contains("\"source_scope\":\"latest\""));
    assert!(handoff.contains("\"has_evidence_chain_enrichment\":true"));
    assert!(handoff.contains("\"has_diagnostic_opinion\":true"));
    assert!(handoff.contains("\"handoff_readiness\":\"automation_worthy\""));
    assert!(handoff.contains("\"gewyvern_merge_hint\":\"operator_guidance_candidate\""));
    assert!(handoff.contains("\"opinion_confidence_band\":\"high\""));

    stop.store(true, Ordering::Relaxed);
    daemon
        .join()
        .expect("daemon thread should join")
        .expect("daemon should exit cleanly");
    let _ = fs::remove_file(state_path);
}

#[test]
fn daemon_target_training_route_emits_learned_route_for_target() {
    let _guard = lock_daemon_test_guard();
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after unix epoch")
        .as_nanos();
    let state_path =
        std::env::temp_dir().join(format!("etragon-daemon-target-train-state-{unique}.json"));
    let bind_addr = reserve_bind_addr();
    let bind_addr_for_thread = bind_addr.clone();
    let stop = Arc::new(AtomicBool::new(false));
    let stop_for_thread = Arc::clone(&stop);
    let worker_config = PythonWorkerConfig {
        state_file: Some(state_path.clone()),
        ..PythonWorkerConfig::default()
    };
    let daemon = thread::spawn(move || {
        run_python_daemon_until(
            &bind_addr_for_thread,
            10,
            &worker_config,
            None,
            "python-targets-url",
            "http://example.test/v1/latest/targets",
            |_, worker| {
                let left_input = fixture("missing_transition_analysis.json");
                let left_output = worker.analyze_json(&left_input)?;
                let right_input = fixture("direct_signal_analysis.json");
                let right_output = worker.analyze_json(&right_input)?;
                let target_outputs = vec![
                    TargetDaemonOutput {
                        path_segment: "scan:http:request".to_string(),
                        input_json: Some(left_input.clone()),
                        recommendation_summary_json: single_output_recommendation_summary(
                            &left_output,
                        ),
                        output_json: left_output.clone(),
                        updated_unix_ms: 0,
                        state_hash: String::new(),
                        last_success_unix_ms: None,
                        last_error: None,
                        training_history: Vec::new(),
                    },
                    TargetDaemonOutput {
                        path_segment: "socket_session".to_string(),
                        input_json: Some(right_input.clone()),
                        recommendation_summary_json: single_output_recommendation_summary(
                            &right_output,
                        ),
                        output_json: right_output.clone(),
                        updated_unix_ms: 0,
                        state_hash: String::new(),
                        last_success_unix_ms: None,
                        last_error: None,
                        training_history: Vec::new(),
                    },
                ];
                Ok(PolledDaemonOutput {
                    input_fingerprint: format!("{}\n{}", left_input, right_input),
                    latest_input_json: None,
                    output_json: batch_output_json(&[
                        ("scan:http:request".to_string(), left_output),
                        ("socket_session".to_string(), right_output),
                    ]),
                    recommendation_summary_json: recommendation_overview_json(&[
                        (
                            "scan:http:request".to_string(),
                            target_outputs[0].output_json.clone(),
                        ),
                        (
                            "socket_session".to_string(),
                            target_outputs[1].output_json.clone(),
                        ),
                    ]),
                    target_outputs,
                })
            },
            stop_for_thread,
        )
    });

    wait_for_daemon_health(&bind_addr).expect("daemon should publish health endpoint");

    wait_for_body(
        &format!(
            "http://{}/v1/latest/targets/scan:http:request/output.json",
            bind_addr
        ),
        |body| body.contains("\"py_ml_candidate_observe_longer\""),
    )
    .expect("daemon should publish initial target output");

    let trained = post_json(
        &format!("http://{}/v1/train/targets/scan:http:request", bind_addr),
        "{\"label\":\"network_observe_longer\"}",
    )
    .expect("daemon should accept target training request");
    assert!(trained.contains("\"status\":\"trained\""));

    let retrained = wait_for_body(
        &format!(
            "http://{}/v1/latest/targets/scan:http:request/output.json",
            bind_addr
        ),
        |body| body.contains("\"py_ml_candidate_learned_route\""),
    )
    .expect("daemon should republish learned route for target after training");
    assert!(retrained.contains("\"learned_label\":\"network_observe_longer\""));

    let summary = wait_for_body(
        &format!(
            "http://{}/v1/latest/targets/scan:http:request/recommendation-summary.json",
            bind_addr
        ),
        |body| body.contains("\"train_count\":1"),
    )
    .expect("daemon should expose enriched target recommendation summary");
    assert!(summary.contains("\"support_score\":"));
    assert!(summary.contains("\"train_count\":1"));
    assert!(summary.contains("\"last_trained_unix_ms\":"));
    assert!(summary.contains("\"score_margin\":"));

    let learning = wait_for_body(
        &format!(
            "http://{}/v1/latest/targets/scan:http:request/learning-summary.json",
            bind_addr
        ),
        |body| body.contains("\"top_learned_label\":\"network_observe_longer\""),
    )
    .expect("daemon should expose enriched target learning summary");
    assert!(learning.contains("\"learning_active\":true"));
    assert!(learning.contains("\"learned_routes\":1"));
    assert!(learning.contains("\"top_learned_route\":\"py_ml_candidate_learned_route\""));
    assert!(learning.contains("\"top_learned_label\":\"network_observe_longer\""));
    assert!(learning.contains("\"top_learned_relationships\":{\"compatible_with\":[\"http_request_followup\"],\"competes_with\":[\"targeted_escalation\"]}"));
    assert!(learning.contains("\"top_learned_state\":{\"route_count\":1"));
    assert!(learning.contains("\"support_score\":1"));
    assert!(learning.contains("\"runner_up_state\":null"));
    assert!(learning.contains("\"confidence_hint\":\"medium\""));
    assert!(learning.contains("\"stability_hint\":\"emerging\""));
    assert!(learning.contains("\"transition_policy_summary\":{\"policy_bias\":\"balanced\",\"compatible_count\":1,\"competing_count\":1"));
    assert!(learning.contains("\"training_conflict_hint\":null"));
    assert!(learning.contains("\"pattern_memory_state\":{\"pattern_key\":"));
    assert!(learning.contains("\"pattern_memory_summary\":{\"label_count\":1,\"top_pattern_label\":\"network_observe_longer\""));
    assert!(learning.contains("\"memory_drift_hint\":{\"status\":\"emerging\""));
    assert!(learning.contains("\"reason\":\"learning_signal_is_still_early\""));
    assert!(learning.contains("\"learning_judgement\":{\"status\":\"observe\""));
    assert!(learning.contains("\"reason\":\"learned_route_is_present_but_still_early\""));
    assert!(learning.contains("\"action_queue_hint\":{\"action\":\"keep_observing\""));
    assert!(learning.contains("\"queue\":\"observation\""));
    assert!(learning.contains("\"queue_summary\":{\"total_actions\":1"));
    assert!(learning.contains("\"top_action\":\"keep_observing\""));
    assert!(learning.contains("\"queue_pressure_hint\":{\"status\":\"monitoring_bias\""));
    assert!(learning.contains("\"feedback_policy_hint\":{\"policy\":\"continue_observation\""));
    assert!(learning.contains("\"evidence_chain_enrichment\":{\"status\":\"emerging\""));
    assert!(learning.contains("\"enrichment_strength_band\":\"low\""));
    assert!(learning.contains("\"handoff_readiness\":\"advisory_only\""));
    assert!(learning.contains("\"gewyvern_merge_hint\":\"augmentations_only\""));
    assert!(learning.contains("\"primary_label\":\"network_observe_longer\""));
    assert!(learning.contains("\"diagnostic_opinion\":null"));
    assert!(learning.contains("\"learned_label_rank\":1"));
    assert!(learning.contains("\"label_count\":1"));
    assert!(learning.contains("\"labels\":[{\"label\":\"network_observe_longer\""));
    assert!(learning.contains("\"recent_training_events\":["));
    assert!(learning.contains("\"recent_label_activity\":[{\"label\":\"network_observe_longer\",\"event_count\":1,\"total_weight\":1"));
    assert!(learning.contains("\"label\":\"network_observe_longer\""));
    assert!(learning.contains("\"scope\":\"target\""));

    let target_enrichment = wait_for_body(
        &format!(
            "http://{}/v1/latest/targets/scan:http:request/evidence-chain-enrichment.json",
            bind_addr
        ),
        |body| body.contains("\"primary_label\":\"network_observe_longer\""),
    )
    .expect("daemon should expose target evidence-chain enrichment");
    assert!(target_enrichment.contains("\"status\":\"emerging\""));
    assert!(target_enrichment.contains("\"enrichment_strength_band\":\"low\""));
    assert!(target_enrichment.contains("\"handoff_readiness\":\"advisory_only\""));
    assert!(target_enrichment.contains("\"gewyvern_merge_hint\":\"augmentations_only\""));
    assert!(target_enrichment.contains("\"primary_label\":\"network_observe_longer\""));

    let target_opinion = read_url(&format!(
        "http://{}/v1/latest/targets/scan:http:request/diagnostic-opinion.json",
        bind_addr
    ))
    .expect("daemon should expose target diagnostic opinion");
    assert_eq!(target_opinion, "null");

    let batch_learning = wait_for_body(
        &format!("http://{}/v1/latest/learning-summary.json", bind_addr),
        |body| body.contains("\"targets\":[\"scan:http:request\"]"),
    )
    .expect("daemon should expose batch learning summary with queue aggregation");
    assert!(batch_learning.contains("\"queue_summary\":{\"total_actions\":1"));
    assert!(batch_learning.contains("\"top_action\":\"keep_observing\""));
    assert!(batch_learning.contains("\"targets\":[\"scan:http:request\"]"));
    assert!(batch_learning.contains("\"queue_pressure_hint\":{\"status\":\"monitoring_bias\""));
    assert!(
        batch_learning.contains("\"feedback_policy_hint\":{\"policy\":\"continue_observation\"")
    );

    let target_meta = read_url(&format!(
        "http://{}/v1/latest/targets/scan:http:request/meta.json",
        bind_addr
    ))
    .expect("daemon should expose learned-route activity in target meta");
    assert!(target_meta.contains("\"learning_active\":true"));
    assert!(target_meta.contains("\"learned_routes\":1"));
    assert!(target_meta.contains("\"has_memory_state\":true"));
    assert!(target_meta.contains("\"memory_learning_active\":true"));
    assert!(target_meta.contains("\"handoff_summary\":{"));
    assert!(target_meta.contains("\"has_evidence_chain_enrichment\":true"));
    assert!(target_meta.contains("\"has_diagnostic_opinion\":false"));

    let index = read_url(&format!("http://{}/v1/latest/targets", bind_addr))
        .expect("daemon should expose learned-route activity in target index");
    assert!(index.contains("\"learning_active\":true"));
    assert!(index.contains("\"learned_routes\":1"));
    assert!(index.contains("\"has_memory_state\":true"));
    assert!(index.contains("\"memory_learning_active\":true"));
    assert!(index.contains("\"handoff_summary\":{"));
    assert!(index.contains("\"has_evidence_chain_enrichment\":true"));

    let target_handoff = wait_for_body(
        &format!(
            "http://{}/v1/latest/targets/scan:http:request/handoff-summary.json",
            bind_addr
        ),
        |body| body.contains("\"has_evidence_chain_enrichment\":true"),
    )
    .expect("daemon should expose target handoff summary");
    assert!(target_handoff.contains("\"source_scope\":\"target\""));
    assert!(target_handoff.contains("\"has_evidence_chain_enrichment\":true"));
    assert!(target_handoff.contains("\"has_diagnostic_opinion\":false"));
    assert!(target_handoff.contains("\"handoff_readiness\":\"advisory_only\""));
    assert!(target_handoff.contains("\"gewyvern_merge_hint\":\"augmentations_only\""));

    stop.store(true, Ordering::Relaxed);
    daemon
        .join()
        .expect("daemon thread should join")
        .expect("daemon should exit cleanly");
    let _ = fs::remove_file(state_path);
}
