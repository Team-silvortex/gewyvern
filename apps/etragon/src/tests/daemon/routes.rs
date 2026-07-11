use super::*;

#[test]
fn daemon_bind_guard_rejects_non_loopback_bind_targets() {
    let local_only = DaemonAccessPolicy::default();
    assert!(validate_daemon_bind_addr("127.0.0.1:4321", &local_only).is_ok());
    assert!(validate_daemon_bind_addr("localhost:4321", &local_only).is_ok());
    let err = validate_daemon_bind_addr("0.0.0.0:4321", &local_only)
        .expect_err("non-loopback daemon bind targets should be rejected");
    assert!(err.contains("loopback-only"));
    let remote_enabled = DaemonAccessPolicy {
        admin_token: Some("secret-token".to_string()),
    };
    assert!(validate_daemon_bind_addr("0.0.0.0:4321", &remote_enabled).is_ok());
}

#[test]
fn daemon_remote_requests_require_matching_admin_token() {
    let policy = DaemonAccessPolicy {
        admin_token: Some("secret-token".to_string()),
    };
    assert!(daemon_request_is_authorized(
        IpAddr::from([127, 0, 0, 1]),
        "GET /health HTTP/1.1\r\nHost: localhost\r\n\r\n",
        &policy,
    ));
    assert!(!daemon_request_is_authorized(
        IpAddr::from([10, 0, 0, 8]),
        "GET /health HTTP/1.1\r\nHost: remote\r\n\r\n",
        &policy,
    ));
    assert!(daemon_request_is_authorized(
        IpAddr::from([10, 0, 0, 8]),
        "GET /health HTTP/1.1\r\nHost: remote\r\nX-Etragon-Admin-Token: secret-token\r\n\r\n",
        &policy,
    ));
    assert!(!daemon_request_is_authorized(
        IpAddr::from([10, 0, 0, 8]),
        "GET /health HTTP/1.1\r\nHost: remote\r\nX-Etragon-Admin-Token: secret-tokeo\r\n\r\n",
        &policy,
    ));
    assert!(!daemon_request_is_authorized(
        IpAddr::from([10, 0, 0, 8]),
        "GET /health HTTP/1.1\r\nHost: remote\r\nX-Etragon-Admin-Token: secret-token-extra\r\n\r\n",
        &policy,
    ));
}

#[test]
fn daemon_remote_token_checks_trim_and_match_headers_case_insensitively() {
    let policy = DaemonAccessPolicy {
        admin_token: Some("secret-token".to_string()),
    };

    assert!(daemon_request_is_authorized(
        IpAddr::from([10, 0, 0, 8]),
        "GET /health HTTP/1.1\r\nHost: remote\r\nx-etragon-admin-token:   secret-token   \r\n\r\n",
        &policy,
    ));
    assert!(daemon_request_is_authorized(
        IpAddr::from([10, 0, 0, 8]),
        "GET /health HTTP/1.1\r\nHost: remote\r\nX-ETRAGON-ADMIN-TOKEN: secret-token\r\n\r\n",
        &policy,
    ));
    assert!(!daemon_request_is_authorized(
        IpAddr::from([10, 0, 0, 8]),
        "GET /health HTTP/1.1\r\nHost: remote\r\nX-Etragon-Admin-Token: secret-token \t extra\r\n\r\n",
        &policy,
    ));
    assert!(!daemon_request_is_authorized(
        IpAddr::from([10, 0, 0, 8]),
        "GET /health HTTP/1.1\r\nHost: remote\r\nX-Etragon-Admin-Token: secret-token\r\nX-Etragon-Admin-Token: secret-token\r\n\r\n",
        &policy,
    ));
    assert!(!daemon_request_is_authorized(
        IpAddr::from([10, 0, 0, 8]),
        "GET /health HTTP/1.1\r\nHost: remote\r\nX-Etragon-Admin-Token: wrong-token\r\nx-etragon-admin-token: secret-token\r\n\r\n",
        &policy,
    ));
}

#[test]
fn daemon_remote_requests_still_fail_without_token_when_policy_is_local_only() {
    let policy = DaemonAccessPolicy::default();

    assert!(!daemon_request_is_authorized(
        IpAddr::from([10, 0, 0, 8]),
        "GET /health HTTP/1.1\r\nHost: remote\r\nX-Etragon-Admin-Token: anything\r\n\r\n",
        &policy,
    ));
    assert!(daemon_request_is_authorized(
        IpAddr::from([127, 0, 0, 1]),
        "GET /health HTTP/1.1\r\nHost: localhost\r\nX-Etragon-Admin-Token: anything\r\n\r\n",
        &policy,
    ));
}

#[test]
fn daemon_bind_guard_rejects_ipv6_unspecified_without_admin_token() {
    let local_only = DaemonAccessPolicy::default();
    let err = validate_daemon_bind_addr("[::]:4321", &local_only)
        .expect_err("ipv6 unspecified bind should be rejected without remote token");
    assert!(err.contains("loopback-only"));

    let remote_enabled = DaemonAccessPolicy {
        admin_token: Some("secret-token".to_string()),
    };
    assert!(validate_daemon_bind_addr("[::]:4321", &remote_enabled).is_ok());
}

#[test]
fn daemon_serves_latest_python_output_and_meta() {
    let _guard = lock_daemon_test_guard();
    let bind_addr = reserve_bind_addr();
    let bind_addr_for_thread = bind_addr.clone();
    let stop = Arc::new(AtomicBool::new(false));
    let stop_for_thread = Arc::clone(&stop);
    let worker_config = PythonWorkerConfig::default();
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

    let latest = wait_for_body(
        &format!("http://{}/v1/latest/output.json", bind_addr),
        |body| body.contains("\"py_ml_candidate_observe_longer\""),
    );
    let latest = latest.expect("daemon should publish latest output");
    assert!(latest.contains("\"source\":\"python-url\""));
    assert!(latest.contains("\"py_ml_candidate_observe_longer\""));
    assert!(latest.contains("\"recommendation_summary\":{"));
    assert!(latest.contains("\"recommendations\":["));
    assert!(latest.contains("\"updated_unix_ms\":"));
    assert!(latest.contains("\"last_success_unix_ms\":"));
    assert!(latest.contains("\"last_error\":null"));
    assert!(latest.contains("\"state_hash\":\""));

    let meta = read_url(&format!("http://{}/v1/latest/meta", bind_addr))
        .expect("daemon should expose meta endpoint");
    assert!(meta.contains("\"status\":\"ready\""));
    assert!(meta.contains("\"cycle\":"));
    assert!(meta.contains("\"analysis_runs\":1"));
    assert!(meta.contains("\"cache_hits\":"));
    assert!(meta.contains("\"target_count\":0"));
    assert!(meta.contains("\"updated_unix_ms\":"));
    assert!(meta.contains("\"last_success_unix_ms\":"));
    assert!(meta.contains("\"last_error\":null"));
    assert!(meta.contains("\"state_hash\":\""));
    assert!(meta.contains("\"learning_active\":false"));
    assert!(meta.contains("\"learned_routes\":0"));
    assert!(meta.contains("\"memory_state_status\":\"empty\""));
    assert!(meta.contains("\"memory_model_version\":\"python-online-memory-v1\""));

    let summary = read_url(&format!(
        "http://{}/v1/latest/recommendation-summary.json",
        bind_addr
    ))
    .expect("daemon should expose recommendation summary endpoint");
    assert!(summary.contains("\"recommendations\":["));
    assert!(summary.contains("\"top_recommendation\":"));
    assert!(summary.contains("\"top_candidates\":["));
    assert!(summary.contains("\"name\":\"py_ml_candidate_observe_longer\""));

    let learning = read_url(&format!(
        "http://{}/v1/latest/learning-summary.json",
        bind_addr
    ))
    .expect("daemon should expose learning summary endpoint");
    assert!(learning.contains("\"learning_active\":false"));
    assert!(learning.contains("\"learned_routes\":0"));
    assert!(learning.contains("\"top_learned_state\":null"));
    assert!(learning.contains("\"training_conflict_hint\":null"));
    assert!(learning.contains("\"pattern_memory_state\":null"));
    assert!(learning.contains("\"pattern_memory_summary\":null"));
    assert!(learning.contains("\"memory_drift_hint\":null"));
    assert!(learning.contains("\"learning_judgement\":null"));
    assert!(learning.contains("\"action_queue_hint\":null"));
    assert!(learning.contains("\"queue_summary\":null"));
    assert!(learning.contains("\"queue_pressure_hint\":null"));
    assert!(learning.contains("\"feedback_policy_hint\":null"));
    assert!(learning.contains("\"evidence_chain_enrichment\":null"));
    assert!(learning.contains("\"diagnostic_opinion\":null"));
    assert!(learning.contains("\"recent_training_events\":[]"));
    assert!(learning.contains("\"recent_label_activity\":[]"));

    let enrichment = read_url(&format!(
        "http://{}/v1/latest/evidence-chain-enrichment.json",
        bind_addr
    ))
    .expect("daemon should expose evidence-chain enrichment endpoint");
    assert_eq!(enrichment, "null");

    let opinion = read_url(&format!(
        "http://{}/v1/latest/diagnostic-opinion.json",
        bind_addr
    ))
    .expect("daemon should expose diagnostic opinion endpoint");
    assert_eq!(opinion, "null");

    let status = read_url(&format!("http://{}/v1/latest/status", bind_addr))
        .expect("daemon should expose status endpoint");
    assert!(status.contains("\"status\":\"ready\""));
    assert!(status.contains("\"last_error\":null"));
    assert!(status.contains("\"last_success_unix_ms\":"));
    assert!(status.contains("\"learning_active\":false"));
    assert!(status.contains("\"learned_routes\":0"));

    let labels = read_url(&format!("http://{}/v1/training-labels.json", bind_addr))
        .expect("daemon should expose training label dictionary");
    assert!(labels.contains("\"canonical\":\"targeted_escalation\""));
    assert!(labels.contains("\"recommended_for\":"));
    assert!(labels.contains("\"compatible_with\":[\"http_request_followup\"]"));
    assert!(
        labels.contains("\"competes_with\":[\"network_observe_longer\",\"http_request_followup\"]")
    );

    let capabilities = read_url(&format!(
        "http://{}/v1/protocol-capabilities.json",
        bind_addr
    ))
    .expect("daemon should expose protocol capabilities endpoint");
    assert!(capabilities.contains("\"protocol_family\":\"etragon-resident-protocol\""));
    assert!(capabilities.contains("\"protocol_version\":1"));
    assert!(capabilities.contains("\"capability_tier\":\"resident-sidecar\""));
    assert!(capabilities.contains("\"/v1/memory-versions.json\""));
    assert!(capabilities.contains("\"/v1/memory-snapshot.json\""));
    assert!(capabilities.contains("\"ir_capabilities\":{"));
    assert!(capabilities.contains(
        "\"resident_memory_annotations\":[\"pattern_memory_state\",\"pattern_memory_summary\""
    ));
    assert!(capabilities.contains(
        "\"stable_target_fields\":[\"target_meta.learning_active\",\"target_meta.learned_routes\""
    ));
    assert!(capabilities.contains("\"merge_capabilities\":{"));
    assert!(capabilities.contains(
        "\"gewyvern_merge_hints\":[\"augmentations_only\",\"augmentations_and_guidance_context\""
    ));
    assert!(capabilities.contains(
        "\"safe_automation_hints\":[\"augmentations_only\",\"augmentations_and_guidance_context\"]"
    ));
    assert!(capabilities.contains("\"handoff_capabilities\":{"));
    assert!(capabilities.contains(
        "\"summary_fields\":[\"has_evidence_chain_enrichment\",\"has_diagnostic_opinion\""
    ));
    assert!(capabilities.contains("\"minor_release_snapshots\":[\"0.14.x\"]"));
    assert!(capabilities.contains(
        "\"forward_compatibility_rules\":[\"unknown_top_level_fields_must_be_ignored\",\"unknown_ir_fields_must_be_ignored\""
    ));
    assert!(capabilities.contains("\"worker\":{"));
    assert!(capabilities.contains("\"supported_training_labels\":[\"network_observe_longer\",\"targeted_escalation\",\"http_request_followup\"]"));
    assert!(capabilities.contains("\"security\":{"));
    assert!(capabilities.contains("\"api_mode\":\"loopback_only\""));
    assert!(capabilities.contains("\"admin_token_header\":\"X-Etragon-Admin-Token\""));

    stop.store(true, Ordering::Relaxed);
    daemon
        .join()
        .expect("daemon thread should join")
        .expect("daemon should exit cleanly");
}

#[test]
fn daemon_reuses_cached_output_when_upstream_input_is_unchanged() {
    let _guard = lock_daemon_test_guard();
    let bind_addr = reserve_bind_addr();
    let bind_addr_for_thread = bind_addr.clone();
    let stop = Arc::new(AtomicBool::new(false));
    let stop_for_thread = Arc::clone(&stop);
    let worker_config = PythonWorkerConfig::default();
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

    let meta = wait_for_body(&format!("http://{}/v1/latest/meta", bind_addr), |body| {
        body.contains("\"cache_hits\":") && !body.contains("\"cache_hits\":0")
    });
    let meta = meta.expect("daemon should record at least one cache hit");
    assert!(meta.contains("\"analysis_runs\":1"));
    assert!(meta.contains("\"cache_hits\":"));

    stop.store(true, Ordering::Relaxed);
    daemon
        .join()
        .expect("daemon thread should join")
        .expect("daemon should exit cleanly");
}

#[test]
fn daemon_reports_last_error_when_polling_fails() {
    let _guard = lock_daemon_test_guard();
    let bind_addr = reserve_bind_addr();
    let bind_addr_for_thread = bind_addr.clone();
    let stop = Arc::new(AtomicBool::new(false));
    let stop_for_thread = Arc::clone(&stop);
    let worker_config = PythonWorkerConfig::default();
    let daemon = thread::spawn(move || {
        run_python_daemon_until(
            &bind_addr_for_thread,
            10,
            &worker_config,
            None,
            "python-url",
            "http://example.test/v1/latest/analysis.json",
            |_, _worker| Err("upstream temporary error".to_string()),
            stop_for_thread,
        )
    });

    wait_for_daemon_health(&bind_addr).expect("daemon should publish health endpoint");

    let status = wait_for_body(&format!("http://{}/v1/latest/status", bind_addr), |body| {
        body.contains("\"status\":\"degraded\"")
            || body.contains("\"last_error\":\"upstream temporary error\"")
    });
    let status = status.expect("daemon should publish degraded status");
    assert!(status.contains("\"status\":\"degraded\""));
    assert!(status.contains("\"last_success_unix_ms\":null"));
    assert!(status.contains("\"last_error\":\"upstream temporary error\""));

    let meta = wait_for_body(&format!("http://{}/v1/latest/meta", bind_addr), |body| {
        body.contains("\"last_error\":\"upstream temporary error\"")
    })
    .expect("daemon should expose meta endpoint");
    assert!(meta.contains("\"last_success_unix_ms\":null"));
    assert!(meta.contains("\"last_error\":\"upstream temporary error\""));

    let latest = wait_for_body(
        &format!("http://{}/v1/latest/output.json", bind_addr),
        |body| body.contains("\"last_error\":\"upstream temporary error\""),
    )
    .expect("daemon should expose placeholder output even after polling errors");
    assert!(latest.contains("\"last_error\":\"upstream temporary error\""));
    assert!(latest.contains("\"output\":null"));

    stop.store(true, Ordering::Relaxed);
    daemon
        .join()
        .expect("daemon thread should join")
        .expect("daemon should exit cleanly");
}

#[test]
fn daemon_serves_target_specific_output_routes() {
    let _guard = lock_daemon_test_guard();
    let bind_addr = reserve_bind_addr();
    let bind_addr_for_thread = bind_addr.clone();
    let stop = Arc::new(AtomicBool::new(false));
    let stop_for_thread = Arc::clone(&stop);
    let worker_config = PythonWorkerConfig::default();
    let daemon = thread::spawn(move || {
        run_python_daemon_until(
            &bind_addr_for_thread,
            500,
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
    wait_for_daemon_ready(&bind_addr).expect("daemon should publish ready status");

    wait_for_body(&format!("http://{}/v1/latest/meta", bind_addr), |body| {
        body.contains("\"target_count\":2")
    })
    .expect("daemon should publish target-aware meta before index reads");

    let index = wait_for_body(&format!("http://{}/v1/latest/targets", bind_addr), |body| {
        body.contains("scan:http:request")
    });
    let index = index.expect("daemon should publish target index");
    assert!(index.contains("\"target_count\":2"));
    assert!(index.contains("/v1/latest/targets/scan:http:request/output.json"));
    assert!(index.contains("/v1/latest/targets/scan:http:request/meta.json"));
    assert!(index.contains("\"updated_unix_ms\":"));
    assert!(index.contains("\"state_hash\":\""));
    assert!(index.contains("\"stale\":"));
    assert!(index.contains("\"stale_after_ms\":1500"));
    assert!(index.contains("\"has_memory_state\":false"));
    assert!(index.contains("\"memory_learning_active\":false"));

    let target_output = read_url(&format!(
        "http://{}/v1/latest/targets/scan:http:request/output.json",
        bind_addr
    ))
    .expect("daemon should expose target-specific output");
    assert!(target_output.contains("\"name\":\"py_ml_candidate_observe_longer\""));

    let target_meta = read_url(&format!(
        "http://{}/v1/latest/targets/scan:http:request/meta.json",
        bind_addr
    ))
    .expect("daemon should expose target-specific meta");
    assert!(target_meta.contains("\"path_segment\":\"scan:http:request\""));
    assert!(target_meta.contains("\"updated_unix_ms\":"));
    assert!(target_meta.contains("\"state_hash\":\""));
    assert!(target_meta.contains("\"last_success_unix_ms\":"));
    assert!(target_meta.contains("\"last_error\":null"));
    assert!(target_meta.contains("\"stale\":false"));
    assert!(target_meta.contains("\"stale_after_ms\":1500"));
    assert!(target_meta.contains("\"learning_active\":false"));
    assert!(target_meta.contains("\"learned_routes\":0"));
    assert!(target_meta.contains("\"has_memory_state\":false"));
    assert!(target_meta.contains("\"memory_learning_active\":false"));

    let target_summary = read_url(&format!(
        "http://{}/v1/latest/targets/socket_session/recommendation-summary.json",
        bind_addr
    ))
    .expect("daemon should expose target-specific recommendation summary");
    assert!(target_summary.contains("\"top_recommendation\":"));
    assert!(target_summary.contains("\"top_candidates\":["));
    assert!(target_summary.contains("\"name\":\"py_ml_candidate_targeted_escalation\""));

    let target_learning = read_url(&format!(
        "http://{}/v1/latest/targets/socket_session/learning-summary.json",
        bind_addr
    ))
    .expect("daemon should expose target-specific learning summary");
    assert!(target_learning.contains("\"learning_active\":false"));
    assert!(target_learning.contains("\"learned_routes\":0"));
    assert!(target_learning.contains("\"top_learned_state\":null"));
    assert!(target_learning.contains("\"training_conflict_hint\":null"));
    assert!(target_learning.contains("\"pattern_memory_state\":null"));
    assert!(target_learning.contains("\"pattern_memory_summary\":null"));
    assert!(target_learning.contains("\"memory_drift_hint\":null"));
    assert!(target_learning.contains("\"learning_judgement\":null"));
    assert!(target_learning.contains("\"action_queue_hint\":null"));
    assert!(target_learning.contains("\"queue_summary\":null"));
    assert!(target_learning.contains("\"queue_pressure_hint\":null"));
    assert!(target_learning.contains("\"feedback_policy_hint\":null"));
    assert!(target_learning.contains("\"evidence_chain_enrichment\":null"));
    assert!(target_learning.contains("\"diagnostic_opinion\":null"));
    assert!(target_learning.contains("\"recent_training_events\":[]"));
    assert!(target_learning.contains("\"recent_label_activity\":[]"));

    let target_enrichment = read_url(&format!(
        "http://{}/v1/latest/targets/socket_session/evidence-chain-enrichment.json",
        bind_addr
    ))
    .expect("daemon should expose target-specific evidence-chain enrichment");
    assert_eq!(target_enrichment, "null");

    let target_opinion = read_url(&format!(
        "http://{}/v1/latest/targets/socket_session/diagnostic-opinion.json",
        bind_addr
    ))
    .expect("daemon should expose target-specific diagnostic opinion");
    assert_eq!(target_opinion, "null");

    let meta = read_url(&format!("http://{}/v1/latest/meta", bind_addr))
        .expect("daemon should expose meta endpoint");
    assert!(meta.contains("\"target_count\":2"));

    stop.store(true, Ordering::Relaxed);
    daemon
        .join()
        .expect("daemon thread should join")
        .expect("daemon should exit cleanly");
}

#[test]
fn daemon_reports_target_specific_error_metadata() {
    let _guard = lock_daemon_test_guard();
    let bind_addr = reserve_bind_addr();
    let bind_addr_for_thread = bind_addr.clone();
    let stop = Arc::new(AtomicBool::new(false));
    let stop_for_thread = Arc::clone(&stop);
    let worker_config = PythonWorkerConfig::default();
    let daemon = thread::spawn(move || {
        run_python_daemon_until(
            &bind_addr_for_thread,
            10,
            &worker_config,
            None,
            "python-targets-url",
            "http://example.test/v1/latest/targets",
            |_, worker| {
                let ok_input = fixture("missing_transition_analysis.json");
                let ok_output = worker.analyze_json(&ok_input)?;
                let target_outputs = vec![
                    TargetDaemonOutput {
                        path_segment: "scan:http:request".to_string(),
                        input_json: Some(ok_input),
                        recommendation_summary_json: single_output_recommendation_summary(
                            &ok_output,
                        ),
                        output_json: ok_output,
                        updated_unix_ms: 0,
                        state_hash: String::new(),
                        last_success_unix_ms: None,
                        last_error: None,
                        training_history: Vec::new(),
                    },
                    TargetDaemonOutput {
                        path_segment: "socket_session".to_string(),
                        input_json: None,
                        recommendation_summary_json: "null".to_string(),
                        output_json: "null".to_string(),
                        updated_unix_ms: 0,
                        state_hash: String::new(),
                        last_success_unix_ms: None,
                        last_error: Some(
                            "python worker error: simulated target failure".to_string(),
                        ),
                        training_history: Vec::new(),
                    },
                ];
                Ok(PolledDaemonOutput {
                    input_fingerprint: "mixed-target-result".to_string(),
                    latest_input_json: None,
                    output_json: batch_output_json(&[
                        (
                            "scan:http:request".to_string(),
                            target_outputs[0].output_json.clone(),
                        ),
                        (
                            "socket_session".to_string(),
                            "__error__:python worker error: simulated target failure".to_string(),
                        ),
                    ]),
                    recommendation_summary_json: recommendation_overview_json(&[
                        (
                            "scan:http:request".to_string(),
                            target_outputs[0].output_json.clone(),
                        ),
                        (
                            "socket_session".to_string(),
                            "__error__:python worker error: simulated target failure".to_string(),
                        ),
                    ]),
                    target_outputs,
                })
            },
            stop_for_thread,
        )
    });

    wait_for_daemon_health(&bind_addr).expect("daemon should publish health endpoint");
    wait_for_daemon_ready(&bind_addr).expect("daemon should publish ready status");

    let target_meta = wait_for_body(
        &format!(
            "http://{}/v1/latest/targets/socket_session/meta.json",
            bind_addr
        ),
        |body| body.contains("\"last_error\":\"python worker error: simulated target failure\""),
    )
    .expect("daemon should publish target-specific error meta");
    assert!(target_meta.contains("\"last_success_unix_ms\":null"));
    assert!(target_meta.contains("\"stale\":true"));
    assert!(target_meta.contains("\"stale_after_ms\":30"));
    assert!(target_meta.contains("\"stale_for_ms\":"));
    assert!(
        target_meta.contains("\"last_error\":\"python worker error: simulated target failure\"")
    );

    stop.store(true, Ordering::Relaxed);
    daemon
        .join()
        .expect("daemon thread should join")
        .expect("daemon should exit cleanly");
}
