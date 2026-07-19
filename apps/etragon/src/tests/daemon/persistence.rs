use super::*;

fn persistence_test_snapshot(state_hash: &str) -> DaemonSnapshot {
    DaemonSnapshot {
        source: "python-url".to_string(),
        upstream_url: "http://example.test/v1/latest/analysis.json".to_string(),
        interval_ms: 1000,
        cycle: 1,
        analysis_runs: 1,
        cache_hits: 0,
        target_count: 0,
        updated_unix_ms: 1234,
        state_hash: state_hash.to_string(),
        latest_output_json: "{\"augmentations\":[]}".to_string(),
        latest_input_json: None,
        latest_recommendation_summary_json: "{\"recommendations\":[]}".to_string(),
        target_outputs: Vec::new(),
        last_success_unix_ms: Some(1234),
        last_error: None,
        training_history: Vec::new(),
    }
}

fn persistence_test_dir(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("etragon-{label}-{}-{unique}", std::process::id()))
}

#[test]
fn daemon_state_write_is_durable_private_and_leaves_no_temporary_file() {
    let dir = persistence_test_dir("durable-state");
    let path = dir.join("state.json");
    write_daemon_state(&path, &persistence_test_snapshot("durable"))
        .expect("daemon state should write");
    let restored = read_daemon_state(&path)
        .expect("daemon state should read")
        .expect("daemon state should exist");
    assert_eq!(restored.state_hash, "durable");
    assert_eq!(
        fs::read_dir(&dir)
            .expect("state directory should read")
            .count(),
        1
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&path)
                .expect("state metadata should read")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn concurrent_daemon_state_writes_remain_parseable_and_isolated() {
    let dir = persistence_test_dir("concurrent-state");
    let path = Arc::new(dir.join("state.json"));
    let writers = (0..8)
        .map(|index| {
            let path = Arc::clone(&path);
            thread::spawn(move || {
                write_daemon_state(
                    &path,
                    &persistence_test_snapshot(&format!("writer-{index}")),
                )
            })
        })
        .collect::<Vec<_>>();
    for writer in writers {
        writer
            .join()
            .expect("writer thread should join")
            .expect("concurrent state write should succeed");
    }
    let restored = read_daemon_state(&path)
        .expect("concurrent state should parse")
        .expect("concurrent state should exist");
    assert!(restored.state_hash.starts_with("writer-"));
    assert_eq!(
        fs::read_dir(&dir)
            .expect("state directory should read")
            .count(),
        1
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn daemon_state_io_rejects_oversized_and_non_file_inputs() {
    let dir = persistence_test_dir("bounded-state");
    fs::create_dir_all(&dir).expect("state directory should create");
    let oversized = dir.join("oversized.json");
    fs::write(&oversized, vec![b'x'; DAEMON_STATE_FILE_LIMIT + 1])
        .expect("oversized fixture should write");
    assert!(
        read_daemon_state(&oversized)
            .expect_err("oversized state should fail")
            .contains("exceeds")
    );
    assert!(
        read_daemon_state(&dir)
            .expect_err("directory state should fail")
            .contains("regular non-symlink")
    );
    let mut oversized_snapshot = persistence_test_snapshot("oversized");
    oversized_snapshot.latest_recommendation_summary_json = format!(
        "{{\"padding\":\"{}\"}}",
        "x".repeat(DAEMON_STATE_FILE_LIMIT)
    );
    assert!(
        write_daemon_state(&dir.join("write.json"), &oversized_snapshot)
            .expect_err("oversized state write should fail")
            .contains("exceeds")
    );
    let _ = fs::remove_dir_all(dir);
}

#[cfg(unix)]
#[test]
fn daemon_state_io_rejects_symlink_targets() {
    use std::os::unix::fs::symlink;

    let dir = persistence_test_dir("symlink-state");
    fs::create_dir_all(&dir).expect("state directory should create");
    let target = dir.join("target.json");
    fs::write(&target, "{}\n").expect("symlink target should write");
    let link = dir.join("state.json");
    symlink(&target, &link).expect("state symlink should create");
    assert!(
        read_daemon_state(&link)
            .expect_err("symlink read should fail")
            .contains("regular non-symlink")
    );
    assert!(
        write_daemon_state(&link, &persistence_test_snapshot("blocked"))
            .expect_err("symlink write should fail")
            .contains("regular non-symlink")
    );
    assert_eq!(fs::read_to_string(target).unwrap(), "{}\n");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn daemon_snapshot_persistence_round_trips_learning_state() {
    let snapshot = DaemonSnapshot {
        source: "python-url".to_string(),
        upstream_url: "http://example.test/v1/latest/analysis.json".to_string(),
        interval_ms: 1000,
        cycle: 3,
        analysis_runs: 2,
        cache_hits: 1,
        target_count: 1,
        updated_unix_ms: 1234,
        state_hash: "deadbeef".to_string(),
        latest_output_json: "{\"augmentations\":[{\"name\":\"py_ml_candidate_learned_route\",\"producer_stage\":\"candidate\",\"producer_pass\":\"python_online_memory\",\"data\":{\"support_score\":1,\"train_count\":1,\"last_trained_unix_ms\":1234,\"score_margin\":1,\"compatible_with\":[\"http_request_followup\"],\"competes_with\":[\"targeted_escalation\"]}}],\"pattern_memory_state\":{\"pattern_key\":\"demo\",\"label_count\":1,\"labels\":[{\"label\":\"network_observe_longer\",\"support_score\":1,\"train_count\":1,\"last_trained_unix_ms\":1234,\"compatible_with\":[\"http_request_followup\"],\"competes_with\":[\"targeted_escalation\"]}]}}".to_string(),
        latest_input_json: Some(fixture("missing_transition_analysis.json")),
        latest_recommendation_summary_json: "{\"recommendations\":[{\"name\":\"py_ml_candidate_learned_route\",\"producer_stage\":\"candidate\",\"producer_pass\":\"python_online_memory\",\"count\":1,\"support_score\":1,\"train_count\":1,\"last_trained_unix_ms\":1234,\"score_margin\":1}],\"top_recommendation\":{\"name\":\"py_ml_candidate_learned_route\"},\"top_candidates\":[{\"name\":\"py_ml_candidate_learned_route\"}]}".to_string(),
        target_outputs: vec![TargetDaemonOutput {
            path_segment: "scan:http:request".to_string(),
            output_json: "{\"augmentations\":[]}".to_string(),
            input_json: Some(fixture("missing_transition_analysis.json")),
            recommendation_summary_json: "{\"recommendations\":[]}".to_string(),
            updated_unix_ms: 1234,
            state_hash: "beadfeed".to_string(),
            last_success_unix_ms: Some(1234),
            last_error: None,
            training_history: vec![TrainingEvent {
                label: "network_observe_longer".to_string(),
                weight: "1".to_string(),
                trained_unix_ms: 1234,
                scope: "target".to_string(),
            }],
        }],
        last_success_unix_ms: Some(1234),
        last_error: None,
        training_history: vec![TrainingEvent {
            label: "network_observe_longer".to_string(),
            weight: "1".to_string(),
            trained_unix_ms: 1234,
            scope: "latest".to_string(),
        }],
    };
    let parsed = parse_daemon_snapshot_from_json(&daemon_snapshot_persistence_json(&snapshot))
        .expect("persisted snapshot should parse");
    assert_eq!(parsed.source, snapshot.source);
    assert_eq!(parsed.upstream_url, snapshot.upstream_url);
    assert_eq!(parsed.analysis_runs, snapshot.analysis_runs);
    assert_eq!(parsed.cache_hits, snapshot.cache_hits);
    assert_eq!(parsed.training_history.len(), 1);
    assert_eq!(parsed.target_outputs.len(), 1);
    assert_eq!(parsed.target_outputs[0].path_segment, "scan:http:request");
    assert!(
        parsed
            .latest_output_json
            .contains("\"pattern_memory_state\"")
    );
}

#[test]
fn daemon_snapshot_persistence_compacts_large_target_batches() {
    let huge_input = format!(
        "{{\"payload\":\"{}\"}}",
        "a".repeat(DAEMON_STATE_INPUT_JSON_LIMIT + 128)
    );
    let target_outputs = (0..(DAEMON_STATE_TARGET_LIMIT + 6))
        .map(|index| TargetDaemonOutput {
            path_segment: format!("scan:target:{index:02}"),
            output_json: if index == 0 {
                format!(
                    "{{\"augmentations\":[],\"pattern_memory_state\":{{\"pattern_key\":\"demo-{index}\",\"label_count\":1,\"labels\":[{{\"label\":\"network_observe_longer\",\"support_score\":1.0}}]}},\"padding\":\"{}\"}}",
                    "x".repeat(DAEMON_STATE_TARGET_OUTPUT_JSON_LIMIT + 64)
                )
            } else {
                "{\"augmentations\":[]}".to_string()
            },
            input_json: Some(huge_input.clone()),
            recommendation_summary_json: format!(
                "{{\"recommendations\":[{{\"name\":\"py_ml_candidate_observe_longer\",\"producer_stage\":\"recommendation\",\"producer_pass\":\"test\",\"count\":1}}],\"top_recommendation\":{{\"name\":\"py_ml_candidate_observe_longer\"}},\"top_candidates\":[{{\"name\":\"py_ml_candidate_observe_longer\"}}],\"target\":\"{}\"}}",
                index
            ),
            updated_unix_ms: 10_000 + index as u128,
            state_hash: format!("hash-{index}"),
            last_success_unix_ms: Some(10_000 + index as u128),
            last_error: if index == 1 {
                Some("upstream error".to_string())
            } else {
                None
            },
            training_history: if index == 0 {
                vec![TrainingEvent {
                    label: "network_observe_longer".to_string(),
                    weight: "1".to_string(),
                    trained_unix_ms: 1234,
                    scope: "target".to_string(),
                }]
            } else {
                Vec::new()
            },
        })
        .collect::<Vec<_>>();
    let snapshot = DaemonSnapshot {
        source: "python-targets-url".to_string(),
        upstream_url: "http://example.test/v1/latest/targets".to_string(),
        interval_ms: 1000,
        cycle: 5,
        analysis_runs: 3,
        cache_hits: 1,
        target_count: target_outputs.len(),
        updated_unix_ms: 20_000,
        state_hash: "batch".to_string(),
        latest_output_json: batch_output_json(
            &target_outputs
                .iter()
                .map(batch_entry_for_target_persistence)
                .collect::<Vec<_>>(),
        ),
        latest_input_json: Some(huge_input),
        latest_recommendation_summary_json: recommendation_overview_json(
            &target_outputs
                .iter()
                .map(batch_entry_for_target_persistence)
                .collect::<Vec<_>>(),
        ),
        target_outputs,
        last_success_unix_ms: Some(20_000),
        last_error: None,
        training_history: vec![TrainingEvent {
            label: "network_observe_longer".to_string(),
            weight: "1".to_string(),
            trained_unix_ms: 20_000,
            scope: "latest".to_string(),
        }],
    };

    let parsed = parse_daemon_snapshot_from_json(&daemon_snapshot_persistence_json(&snapshot))
        .expect("compacted persisted snapshot should parse");
    assert_eq!(parsed.target_count, DAEMON_STATE_TARGET_LIMIT + 6);
    assert_eq!(parsed.target_outputs.len(), DAEMON_STATE_TARGET_LIMIT);
    assert!(parsed.latest_input_json.is_none());
    assert!(
        parsed
            .target_outputs
            .iter()
            .any(|target| target.path_segment == "scan:target:00")
    );
    let retained_target = parsed
        .target_outputs
        .iter()
        .find(|target| target.path_segment == "scan:target:00")
        .expect("trained target should be retained");
    assert!(retained_target.input_json.is_none());
    assert!(
        retained_target
            .output_json
            .contains("\"pattern_memory_state\"")
    );
    assert!(parsed.latest_output_json.contains("\"targets\":["));
    assert!(
        parsed
            .latest_recommendation_summary_json
            .contains("\"recommendations\":[")
    );
}

#[test]
fn daemon_restores_persisted_learning_state_on_startup() {
    let _guard = lock_daemon_test_guard();
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after unix epoch")
        .as_nanos();
    let daemon_state_path =
        std::env::temp_dir().join(format!("etragon-daemon-state-{unique}.json"));
    let persisted_output = "{\"augmentations\":[{\"name\":\"py_ml_candidate_learned_route\",\"producer_stage\":\"candidate\",\"producer_pass\":\"python_online_memory\",\"data\":{\"learned_label\":\"network_observe_longer\",\"support_score\":1,\"train_count\":1,\"last_trained_unix_ms\":1234,\"score_margin\":1,\"compatible_with\":[\"http_request_followup\"],\"competes_with\":[\"targeted_escalation\"]}}],\"pattern_memory_state\":{\"pattern_key\":\"demo\",\"label_count\":1,\"labels\":[{\"label\":\"network_observe_longer\",\"support_score\":1,\"train_count\":1,\"last_trained_unix_ms\":1234,\"compatible_with\":[\"http_request_followup\"],\"competes_with\":[\"targeted_escalation\"]}]}}".to_string();
    let persisted_summary = "{\"recommendations\":[{\"name\":\"py_ml_candidate_learned_route\",\"producer_stage\":\"candidate\",\"producer_pass\":\"python_online_memory\",\"count\":1,\"support_score\":1,\"train_count\":1,\"last_trained_unix_ms\":1234,\"score_margin\":1}],\"top_recommendation\":{\"name\":\"py_ml_candidate_learned_route\"},\"top_candidates\":[{\"name\":\"py_ml_candidate_learned_route\"}]}".to_string();
    let persisted_snapshot = DaemonSnapshot {
        source: "python-url".to_string(),
        upstream_url: "http://example.test/v1/latest/analysis.json".to_string(),
        interval_ms: 10,
        cycle: 1,
        analysis_runs: 1,
        cache_hits: 0,
        target_count: 0,
        updated_unix_ms: 1234,
        state_hash: "persisted".to_string(),
        latest_output_json: persisted_output,
        latest_input_json: Some(fixture("missing_transition_analysis.json")),
        latest_recommendation_summary_json: persisted_summary,
        target_outputs: Vec::new(),
        last_success_unix_ms: Some(1234),
        last_error: None,
        training_history: vec![TrainingEvent {
            label: "network_observe_longer".to_string(),
            weight: "1".to_string(),
            trained_unix_ms: 1234,
            scope: "latest".to_string(),
        }],
    };
    write_daemon_state(&daemon_state_path, &persisted_snapshot)
        .expect("persisted daemon state should write");

    let bind_addr = reserve_bind_addr();
    let bind_addr_for_thread = bind_addr.clone();
    let stop = Arc::new(AtomicBool::new(false));
    let stop_for_thread = Arc::clone(&stop);
    let worker_config = PythonWorkerConfig::default();
    let daemon_state_path_for_thread = daemon_state_path.clone();
    let daemon = thread::spawn(move || {
        run_python_daemon_until(
            &bind_addr_for_thread,
            10,
            &worker_config,
            Some(daemon_state_path_for_thread.as_path()),
            "python-url",
            "http://example.test/v1/latest/analysis.json",
            |_, _worker| Err("upstream temporary error".to_string()),
            stop_for_thread,
        )
    });

    wait_for_daemon_health(&bind_addr).expect("daemon should publish health endpoint");
    wait_for_body(&format!("http://{}/v1/latest/meta", bind_addr), |body| {
        body.contains("\"learning_active\":true") && body.contains("\"state_hash\":\"persisted\"")
    })
    .expect("daemon should restore persisted meta before fresh polls succeed");

    let learning = wait_for_body(
        &format!("http://{}/v1/latest/learning-summary.json", bind_addr),
        |body| body.contains("\"top_learned_label\":\"network_observe_longer\""),
    )
    .expect("daemon should restore persisted learning summary before fresh polls succeed");
    assert!(learning.contains("\"learning_active\":true"));
    assert!(learning.contains("\"top_learned_route\":\"py_ml_candidate_learned_route\""));
    assert!(learning.contains("\"feedback_policy_hint\":"));
    assert!(learning.contains("\"evidence_chain_enrichment\":{"));
    assert!(learning.contains("\"diagnostic_opinion\":null"));

    let status = wait_for_body(&format!("http://{}/v1/latest/status", bind_addr), |body| {
        body.contains("\"status\":\"degraded\"")
    })
    .expect("daemon should still mark status degraded when polling fails");
    assert!(status.contains("\"status\":\"degraded\""));

    stop.store(true, Ordering::Relaxed);
    daemon
        .join()
        .expect("daemon thread should join")
        .expect("daemon should exit cleanly");
    let _ = fs::remove_file(daemon_state_path);
}
