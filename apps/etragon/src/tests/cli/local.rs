use super::*;

#[test]
fn cli_parses_serve_python_targets_with_filter_and_bind() {
    let bind_addr = reserve_bind_addr();
    let mut args = vec![
        "serve-python-targets-url".to_string(),
        "http://127.0.0.1:9910/v1/latest/targets".to_string(),
        "--bind".to_string(),
        bind_addr,
        "--filter".to_string(),
        "scan:".to_string(),
    ];
    args.extend(default_worker_args());
    let _ = parse_daemon_options(&args[2..]).expect("daemon args should parse");
    let err = run_cli(&[]).expect_err("usage path should stay intact");
    assert!(err.contains("serve-python-targets-url"));
}

#[test]
fn python_online_memory_persists_state_atomically() {
    let worker = include_str!("../../../scripts/python_online_memory.py");

    assert!(worker.contains("tmp_path.write_text(payload, encoding=\"utf-8\")"));
    assert!(worker.contains("tmp_path.replace(self.state_file)"));
    assert!(!worker.contains("self.state_file.write_text("));
}

#[test]
fn cli_analyzes_snapshot_file() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("etragon-analysis-{unique}.json"));
    fs::write(
        &path,
        "{\"primary_module_kind\":\"http_request_response\",\"primary_failure_mode\":\"no_response\",\"primary_failure_detail\":\"request_sent_no_reply\",\"primary_failure_confidence\":\"medium\",\"primary_failure_basis\":\"missing_transition\",\"ambiguous\":false,\"competing_hypotheses\":[]}",
    )
    .expect("temp snapshot should write");

    let output = run_cli(&[
        "analyze-json".to_string(),
        path.to_string_lossy().to_string(),
    ])
    .expect("cli should analyze snapshot file");
    assert!(output.contains("\"augmentations\":["));
    assert!(output.contains("\"name\":\"ml_candidate_observe_longer\""));

    let _ = fs::remove_file(path);
}

#[test]
fn native_cli_training_persists_without_python() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("etragon-native-cli-{unique}.json"));
    let state_path = std::env::temp_dir().join(format!("etragon-native-cli-state-{unique}.json"));
    fs::write(
        &path,
        r#"{"primary_module_kind":"http_request_response","primary_failure_mode":"no_response","primary_failure_detail":"request_sent_no_reply","primary_failure_confidence":"medium","primary_failure_basis":"missing_transition","ambiguous":false,"competing_hypotheses":[]}"#,
    )
    .expect("temp snapshot should write");

    let trained = run_cli(&[
        "train-json".to_string(),
        path.to_string_lossy().to_string(),
        "--label".to_string(),
        "request_followup".to_string(),
        "--weight".to_string(),
        "2.5".to_string(),
        "--state".to_string(),
        state_path.to_string_lossy().to_string(),
    ])
    .expect("native training should succeed");
    assert!(trained.contains(r#""backend":"rust-native""#));
    assert!(trained.contains(r#""label":"http_request_followup""#));

    let analyzed = run_cli(&[
        "analyze-json".to_string(),
        path.to_string_lossy().to_string(),
        "--state".to_string(),
        state_path.to_string_lossy().to_string(),
    ])
    .expect("native analysis should reload persisted memory");
    assert!(analyzed.contains(r#""name":"ml_candidate_learned_route""#));
    assert!(analyzed.contains(r#""producer_pass":"etragon_native_memory""#));
    assert!(!analyzed.contains("python_online_memory"));

    let info = run_cli(&[
        "memory-info".to_string(),
        "--state".to_string(),
        state_path.to_string_lossy().to_string(),
    ])
    .expect("native memory info should succeed");
    assert!(info.contains(r#""model_version":"etragon-native-memory-v1""#));
    assert!(info.contains(r#""pattern_count":1"#));

    let snapshot = run_cli(&[
        "memory-snapshot".to_string(),
        "--state".to_string(),
        state_path.to_string_lossy().to_string(),
    ])
    .expect("native memory snapshot should succeed");
    let legacy_snapshot_path =
        std::env::temp_dir().join(format!("etragon-native-cli-legacy-{unique}.json"));
    fs::write(
        &legacy_snapshot_path,
        snapshot.replace("etragon-native-memory-v1", "python-online-memory-v1"),
    )
    .expect("legacy-compatible snapshot should write");
    let plan = run_cli(&[
        "memory-transfer-plan".to_string(),
        legacy_snapshot_path.to_string_lossy().to_string(),
        "--merge".to_string(),
        "--state".to_string(),
        state_path.to_string_lossy().to_string(),
    ])
    .expect("native transfer plan should accept legacy memory");
    assert!(plan.contains(r#""dry_run":true"#));
    assert!(plan.contains(r#""will_import":false"#));
    assert!(plan.contains(r#""compatible":true"#));
    assert!(plan.contains(r#""model_compatible":true"#));

    let _ = fs::remove_file(path);
    let _ = fs::remove_file(state_path);
    let _ = fs::remove_file(legacy_snapshot_path);
}

#[test]
fn cli_analyzes_python_snapshot_file() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("etragon-python-analysis-{unique}.json"));
    fs::write(
        &path,
        "{\"primary_module_kind\":\"http_request_response\",\"primary_failure_mode\":\"no_response\",\"primary_failure_detail\":\"request_sent_no_reply\",\"primary_failure_confidence\":\"medium\",\"primary_failure_basis\":\"missing_transition\",\"ambiguous\":false,\"competing_hypotheses\":[]}",
    )
    .expect("temp snapshot should write");

    let mut args = vec![
        "analyze-python-json".to_string(),
        path.to_string_lossy().to_string(),
    ];
    args.extend(default_worker_args());
    let output = run_cli(&args).expect("cli should analyze snapshot file with python worker");
    assert!(output.contains("\"name\":\"py_ml_candidate_observe_longer\""));
    assert!(output.contains("\"producer_pass\":\"python_baseline_worker\""));

    let _ = fs::remove_file(path);
}

#[test]
fn cli_trains_python_snapshot_file() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("etragon-python-train-{unique}.json"));
    let state_path = std::env::temp_dir().join(format!("etragon-python-train-state-{unique}.json"));
    fs::write(
        &path,
        "{\"primary_module_kind\":\"http_request_response\",\"primary_failure_mode\":\"no_response\",\"primary_failure_detail\":\"request_sent_no_reply\",\"primary_failure_confidence\":\"medium\",\"primary_failure_basis\":\"missing_transition\",\"ambiguous\":false,\"competing_hypotheses\":[]}",
    )
    .expect("temp snapshot should write");

    let mut args = vec![
        "train-python-json".to_string(),
        path.to_string_lossy().to_string(),
        "--label".to_string(),
        "http_request_followup".to_string(),
    ];
    args.extend(default_worker_args());
    args.push("--python-state".to_string());
    args.push(state_path.to_string_lossy().to_string());
    let output = run_cli(&args).expect("cli should train snapshot file with python worker");
    assert!(output.contains("\"status\":\"trained\""));
    assert!(output.contains("\"label\":\"http_request_followup\""));

    let _ = fs::remove_file(path);
    let _ = fs::remove_file(state_path);
}

#[test]
fn cli_trains_python_snapshot_file_with_weight() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("etragon-python-train-weight-{unique}.json"));
    let state_path =
        std::env::temp_dir().join(format!("etragon-python-train-weight-state-{unique}.json"));
    fs::write(
        &path,
        "{\"primary_module_kind\":\"http_request_response\",\"primary_failure_mode\":\"no_response\",\"primary_failure_detail\":\"request_sent_no_reply\",\"primary_failure_confidence\":\"medium\",\"primary_failure_basis\":\"missing_transition\",\"ambiguous\":false,\"competing_hypotheses\":[]}",
    )
    .expect("temp snapshot should write");

    let mut args = vec![
        "train-python-json".to_string(),
        path.to_string_lossy().to_string(),
        "--label".to_string(),
        "http_request_followup".to_string(),
        "--weight".to_string(),
        "2.5".to_string(),
    ];
    args.extend(default_worker_args());
    args.push("--python-state".to_string());
    args.push(state_path.to_string_lossy().to_string());
    let output = run_cli(&args).expect("cli should train snapshot file with weight");
    assert!(output.contains("\"status\":\"trained\""));
    assert!(output.contains("\"weight\":2.5"));
    assert!(output.contains("\"train_count\":1"));
    assert!(output.contains("\"last_trained_unix_ms\":"));

    let _ = fs::remove_file(path);
    let _ = fs::remove_file(state_path);
}

#[test]
fn cli_training_alias_emits_canonical_label() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("etragon-python-train-alias-{unique}.json"));
    let state_path =
        std::env::temp_dir().join(format!("etragon-python-train-alias-state-{unique}.json"));
    fs::write(
        &path,
        "{\"primary_module_kind\":\"http_request_response\",\"primary_failure_mode\":\"no_response\",\"primary_failure_detail\":\"request_sent_no_reply\",\"primary_failure_confidence\":\"medium\",\"primary_failure_basis\":\"missing_transition\",\"ambiguous\":false,\"competing_hypotheses\":[]}",
    )
    .expect("temp snapshot should write");

    let mut args = vec![
        "train-python-json".to_string(),
        path.to_string_lossy().to_string(),
        "--label".to_string(),
        "request_followup".to_string(),
    ];
    args.extend(default_worker_args());
    args.push("--python-state".to_string());
    args.push(state_path.to_string_lossy().to_string());
    let output = run_cli(&args).expect("cli should train snapshot file with alias");
    assert!(output.contains("\"label\":\"http_request_followup\""));

    let _ = fs::remove_file(path);
    let _ = fs::remove_file(state_path);
}

#[test]
fn cli_training_then_analysis_emits_learned_route_candidate() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("etragon-python-train-analyze-{unique}.json"));
    let state_path =
        std::env::temp_dir().join(format!("etragon-python-train-analyze-state-{unique}.json"));
    fs::write(
        &path,
        "{\"primary_module_kind\":\"http_request_response\",\"primary_failure_mode\":\"no_response\",\"primary_failure_detail\":\"request_sent_no_reply\",\"primary_failure_confidence\":\"medium\",\"primary_failure_basis\":\"missing_transition\",\"ambiguous\":false,\"competing_hypotheses\":[]}",
    )
    .expect("temp snapshot should write");

    let mut train_args = vec![
        "train-python-json".to_string(),
        path.to_string_lossy().to_string(),
        "--label".to_string(),
        "http_request_followup".to_string(),
    ];
    train_args.extend(default_worker_args());
    train_args.push("--python-state".to_string());
    train_args.push(state_path.to_string_lossy().to_string());
    let train_output = run_cli(&train_args).expect("training step should succeed");
    assert!(train_output.contains("\"status\":\"trained\""));

    let mut analyze_args = vec![
        "analyze-python-json".to_string(),
        path.to_string_lossy().to_string(),
    ];
    analyze_args.extend(default_worker_args());
    analyze_args.push("--python-state".to_string());
    analyze_args.push(state_path.to_string_lossy().to_string());
    let output = run_cli(&analyze_args).expect("analysis step should succeed");
    assert!(output.contains("\"name\":\"py_ml_candidate_learned_route\""));
    assert!(output.contains("\"learned_label\":\"http_request_followup\""));
    assert!(output.contains("\"producer_pass\":\"python_online_memory\""));
    assert!(output.contains("\"train_count\":1"));
    assert!(output.contains("\"last_trained_unix_ms\":"));
    assert!(output.contains("\"score_margin\":"));

    let _ = fs::remove_file(path);
    let _ = fs::remove_file(state_path);
}

#[test]
fn cli_reports_python_memory_info_and_can_clear_state() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("etragon-python-memory-{unique}.json"));
    let state_path =
        std::env::temp_dir().join(format!("etragon-python-memory-state-{unique}.json"));
    fs::write(
        &path,
        r#"{"primary_module_kind":"http_request_response","primary_failure_mode":"no_response","primary_failure_detail":"request_sent_no_reply","primary_failure_confidence":"medium","primary_failure_basis":"missing_transition","ambiguous":false,"competing_hypotheses":[]}"#,
    )
    .expect("temp snapshot should write");

    let mut train_args = vec![
        "train-python-json".to_string(),
        path.to_string_lossy().to_string(),
        "--label".to_string(),
        "http_request_followup".to_string(),
    ];
    train_args.extend(default_worker_args());
    train_args.push("--python-state".to_string());
    train_args.push(state_path.to_string_lossy().to_string());
    run_cli(&train_args).expect("training step should succeed");

    let mut info_args = vec!["python-memory-info".to_string()];
    info_args.extend(default_worker_args());
    info_args.push("--python-state".to_string());
    info_args.push(state_path.to_string_lossy().to_string());
    let info = run_cli(&info_args).expect("memory info should succeed");
    assert!(info.contains(r#""schema_version":1"#));
    assert!(info.contains(r#""model_version":"python-online-memory-v1""#));
    assert!(info.contains(r#""pattern_count":1"#));
    assert!(info.contains(r#""label_count":1"#));

    let mut model_args = vec!["python-memory-model-info".to_string()];
    model_args.extend(default_worker_args());
    model_args.push("--python-state".to_string());
    model_args.push(state_path.to_string_lossy().to_string());
    let model_info = run_cli(&model_args).expect("memory model info should succeed");
    assert!(model_info.contains(r#""worker_protocol_version":1"#));
    assert!(model_info.contains(
        r#""supported_commands":["ANALYZE","TRAIN","MEMORY_INFO","MODEL_INFO","MEMORY_VERSIONS","MEMORY_EXPORT","MEMORY_IMPORT","MEMORY_SAVE_SLOT","MEMORY_LOAD_SLOT","MEMORY_DELETE_SLOT","CLEAR_MEMORY"]"#
    ));
    assert!(model_info.contains(
        r#""supported_training_labels":["network_observe_longer","targeted_escalation","http_request_followup"]"#
    ));
    assert!(model_info.contains(r#""supported_import_strategies":["replace","merge"]"#));
    assert!(
        model_info.contains(r#""snapshot_slot_metadata_fields":["slot","label","note","source"]"#)
    );

    let mut snapshot_args = vec!["python-memory-snapshot".to_string()];
    snapshot_args.extend(default_worker_args());
    snapshot_args.push("--python-state".to_string());
    snapshot_args.push(state_path.to_string_lossy().to_string());
    let snapshot = run_cli(&snapshot_args).expect("memory snapshot should succeed");
    assert!(snapshot.contains(r#""status":"exported""#));
    assert!(snapshot.contains(r#""pattern_count":1"#));
    assert!(snapshot.contains(r#""label_count":1"#));
    let snapshot_path =
        std::env::temp_dir().join(format!("etragon-python-memory-export-{unique}.json"));
    fs::write(&snapshot_path, &snapshot).expect("snapshot export should write");

    let mut save_slot_args = vec![
        "save-python-memory-slot".to_string(),
        "baseline".to_string(),
        "--label".to_string(),
        "baseline-v1".to_string(),
        "--note".to_string(),
        "manual checkpoint".to_string(),
        "--source".to_string(),
        "operator_cli".to_string(),
    ];
    save_slot_args.extend(default_worker_args());
    save_slot_args.push("--python-state".to_string());
    save_slot_args.push(state_path.to_string_lossy().to_string());
    let saved = run_cli(&save_slot_args).expect("slot save should succeed");
    assert!(saved.contains(r#""status":"saved""#));
    assert!(saved.contains(r#""slot":"baseline""#));
    assert!(saved.contains(r#""label":"baseline-v1""#));
    assert!(saved.contains(r#""note":"manual checkpoint""#));
    assert!(saved.contains(r#""source":"operator_cli""#));

    let mut versions_args = vec!["python-memory-versions".to_string()];
    versions_args.extend(default_worker_args());
    versions_args.push("--python-state".to_string());
    versions_args.push(state_path.to_string_lossy().to_string());
    let versions = run_cli(&versions_args).expect("memory versions should succeed");
    assert!(versions.contains(r#""slot_count":1"#));
    assert!(versions.contains(r#""slot":"baseline""#));
    assert!(versions.contains(r#""label":"baseline-v1""#));
    assert!(versions.contains(r#""note":"manual checkpoint""#));
    assert!(versions.contains(r#""source":"operator_cli""#));
    assert!(versions.contains(r#""history":[{"action":"save_slot""#));

    let mut clear_args = vec!["clear-python-memory".to_string()];
    clear_args.extend(default_worker_args());
    clear_args.push("--python-state".to_string());
    clear_args.push(state_path.to_string_lossy().to_string());
    let cleared = run_cli(&clear_args).expect("clear step should succeed");
    assert!(cleared.contains(r#""status":"cleared""#));
    assert!(cleared.contains(r#""cleared_pattern_count":1"#));
    assert!(cleared.contains(r#""cleared_label_count":1"#));

    let after = run_cli(&info_args).expect("memory info after clear should succeed");
    assert!(after.contains(r#""status":"empty""#));
    assert!(after.contains(r#""pattern_count":0"#));
    assert!(after.contains(r#""label_count":0"#));

    let mut load_slot_args = vec![
        "load-python-memory-slot".to_string(),
        "baseline".to_string(),
    ];
    load_slot_args.push("--merge".to_string());
    load_slot_args.extend(default_worker_args());
    load_slot_args.push("--python-state".to_string());
    load_slot_args.push(state_path.to_string_lossy().to_string());
    let loaded = run_cli(&load_slot_args).expect("slot load should succeed");
    assert!(loaded.contains(r#""status":"loaded""#));
    assert!(loaded.contains(r#""slot":"baseline""#));
    assert!(loaded.contains(r#""strategy":"merge""#));
    assert!(loaded.contains(r#""label":"baseline-v1""#));

    let mut import_args = vec![
        "import-python-memory".to_string(),
        snapshot_path.to_string_lossy().to_string(),
    ];
    import_args.push("--merge".to_string());
    import_args.extend(default_worker_args());
    import_args.push("--python-state".to_string());
    import_args.push(state_path.to_string_lossy().to_string());
    let imported = run_cli(&import_args).expect("memory import should succeed");
    assert!(imported.contains(r#""status":"loaded""#));
    assert!(imported.contains(r#""imported_pattern_count":1"#));
    assert!(imported.contains(r#""imported_label_count":1"#));
    assert!(imported.contains(r#""strategy":"merge""#));

    let mut transfer_plan_args = vec![
        "python-memory-transfer-plan".to_string(),
        snapshot_path.to_string_lossy().to_string(),
    ];
    transfer_plan_args.push("--merge".to_string());
    transfer_plan_args.extend(default_worker_args());
    transfer_plan_args.push("--python-state".to_string());
    transfer_plan_args.push(state_path.to_string_lossy().to_string());
    let transfer_plan = run_cli(&transfer_plan_args).expect("transfer plan should succeed");
    assert!(transfer_plan.contains(r#""kind":"etragon_memory_transfer_plan""#));
    assert!(transfer_plan.contains(r#""dry_run":true"#));
    assert!(transfer_plan.contains(r#""will_import":false"#));
    assert!(transfer_plan.contains(r#""strategy":"merge""#));
    assert!(transfer_plan.contains(r#""compatible":true"#));
    assert!(transfer_plan.contains(r#""incoming":{"label_count":1"#));
    assert!(transfer_plan.contains(r#""schema_version":1"#));
    assert!(transfer_plan.contains(r#""overlap_pattern_count":1"#));

    let mut analyze_args = vec![
        "analyze-python-json".to_string(),
        path.to_string_lossy().to_string(),
    ];
    analyze_args.extend(default_worker_args());
    analyze_args.push("--python-state".to_string());
    analyze_args.push(state_path.to_string_lossy().to_string());
    let relearned = run_cli(&analyze_args).expect("analysis after import should succeed");
    assert!(relearned.contains("\"name\":\"py_ml_candidate_learned_route\""));
    assert!(relearned.contains("\"learned_label\":\"http_request_followup\""));

    let mut delete_slot_args = vec![
        "delete-python-memory-slot".to_string(),
        "baseline".to_string(),
    ];
    delete_slot_args.extend(default_worker_args());
    delete_slot_args.push("--python-state".to_string());
    delete_slot_args.push(state_path.to_string_lossy().to_string());
    let deleted = run_cli(&delete_slot_args).expect("slot delete should succeed");
    assert!(deleted.contains(r#""status":"deleted""#));
    assert!(deleted.contains(r#""slot":"baseline""#));
    assert!(deleted.contains(r#""label":"baseline-v1""#));
    assert!(deleted.contains(r#""slot_count":0"#));

    let _ = fs::remove_file(path);
    let _ = fs::remove_file(state_path);
    let _ = fs::remove_file(snapshot_path);
}

#[test]
fn cli_reports_protocol_capabilities() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after unix epoch")
        .as_nanos();
    let state_path =
        std::env::temp_dir().join(format!("etragon-python-capabilities-state-{unique}.json"));

    let mut args = vec!["protocol-capabilities".to_string()];
    args.push("--state".to_string());
    args.push(state_path.to_string_lossy().to_string());
    let output = run_cli(&args).expect("protocol capabilities should succeed");
    assert!(output.contains(r#""protocol_family":"etragon-resident-protocol""#));
    assert!(output.contains(r#""protocol_version":1"#));
    assert!(output.contains(r#""capability_tier":"resident-sidecar""#));
    assert!(output.contains(r#""daemon_routes":["/health","/v1/training-labels.json","/v1/memory-state.json","/v1/memory-model.json","/v1/memory-versions.json","/v1/memory-snapshot.json""#));
    assert!(output.contains(
        r#""ir_capabilities":{"latest_scope":["recommendation_summary","learning_summary""#
    ));
    assert!(output.contains(
        r#""stable_latest_fields":["recommendation_summary.top_recommendation","recommendation_summary.top_candidates""#
    ));
    assert!(output.contains(
        r#""experimental_latest_fields":["learning_summary.queue_pressure_hint","learning_summary.feedback_policy_hint""#
    ));
    assert!(output.contains(
        r#""merge_capabilities":{"recommendation_summary_merging":true,"target_batch_merging":true"#
    ));
    assert!(output.contains(
        r#""safe_automation_hints":["augmentations_only","augmentations_and_guidance_context"]"#
    ));
    assert!(output.contains(
        r#""operator_review_hints":["augmentations_with_operator_guidance_support","sidecar_only_opinion","operator_guidance_candidate"]"#
    ));
    assert!(output.contains(r#""handoff_capabilities":{"readiness_levels":["advisory_only","mergeable","automation_worthy"]"#));
    assert!(output.contains(r#""minor_release_snapshots":["0.14.x"]"#));
    assert!(output.contains(
        r#""forward_compatibility_rules":["unknown_top_level_fields_must_be_ignored","unknown_ir_fields_must_be_ignored""#
    ));
    assert!(output.contains(r#""worker_protocol_version":1"#));
    assert!(output.contains(r#""model_version":"etragon-native-memory-v1""#));
    assert!(output.contains(r#""backend":"rust-native""#));

    let _ = fs::remove_file(state_path);
}
