use super::*;

fn wait_for_memory_output<F>(url: &str, predicate: F) -> Option<String>
where
    F: Fn(&str) -> bool,
{
    for _ in 0..12000 {
        if let Ok(body) = read_url(url)
            && predicate(&body)
        {
            return Some(body);
        }
        thread::sleep(Duration::from_millis(25));
    }
    None
}

#[test]
fn daemon_memory_state_route_and_clear_route_manage_online_memory() {
    let _guard = lock_daemon_test_guard();
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after unix epoch")
        .as_nanos();
    let state_path =
        std::env::temp_dir().join(format!("etragon-daemon-memory-state-{unique}.json"));
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

    wait_for_memory_output(
        &format!("http://{}/v1/latest/output.json", bind_addr),
        |body| body.contains(r#""py_ml_candidate_observe_longer""#),
    )
    .expect("daemon should publish initial output");

    post_json(
        &format!("http://{}/v1/train/latest", bind_addr),
        r#"{"label":"network_observe_longer"}"#,
    )
    .expect("daemon should accept training request");

    wait_for_body(
        &format!("http://{}/v1/latest/output.json", bind_addr),
        |body| body.contains(r#""py_ml_candidate_learned_route""#),
    )
    .expect("daemon should republish learned route after training");

    let memory = wait_for_body(
        &format!("http://{}/v1/memory-state.json", bind_addr),
        |body| {
            body.contains(r#""pattern_count":1"#)
                && body.contains(r#""label_count":1"#)
                && body.contains(r#""resident_training_event_count":1"#)
        },
    )
    .expect("daemon should expose memory-state route");
    assert!(memory.contains(r#""worker":{"#));
    assert!(memory.contains(r#""schema_version":1"#));
    assert!(memory.contains(r#""model_version":"python-online-memory-v1""#));
    assert!(memory.contains(r#""resident_training_event_count":1"#));

    let model_info = wait_for_body(
        &format!("http://{}/v1/memory-model.json", bind_addr),
        |body| body.contains(r#""worker_protocol_version":1"#),
    )
    .expect("daemon should expose memory-model route");
    assert!(model_info.contains(r#""model_version":"python-online-memory-v1""#));
    assert!(model_info.contains(
        r#""supported_training_labels":["network_observe_longer","targeted_escalation","http_request_followup"]"#
    ));
    assert!(
        model_info.contains(r#""snapshot_slot_metadata_fields":["slot","label","note","source"]"#)
    );

    let snapshot_export = read_url(&format!("http://{}/v1/memory-snapshot.json", bind_addr))
        .expect("daemon should expose memory snapshot route");
    assert!(snapshot_export.contains(r#""status":"exported""#));
    assert!(snapshot_export.contains(r#""pattern_count":1"#));
    assert!(snapshot_export.contains(r#""label_count":1"#));
    assert!(snapshot_export.contains(r#""pattern_labels":{"#));

    let saved = post_json(
        &format!("http://{}/v1/memory-admin/save", bind_addr),
        r#"{"slot":"baseline","label":"baseline-v1","note":"manual checkpoint","source":"daemon_test"}"#,
    )
    .expect("daemon should save memory slot");
    assert!(saved.contains(r#""status":"saved""#));
    assert!(saved.contains(r#""slot":"baseline""#));
    assert!(saved.contains(r#""label":"baseline-v1""#));
    assert!(saved.contains(r#""note":"manual checkpoint""#));
    assert!(saved.contains(r#""source":"daemon_test""#));

    let versions = read_url(&format!("http://{}/v1/memory-versions.json", bind_addr))
        .expect("daemon should expose memory versions route");
    assert!(versions.contains(r#""slot_count":1"#));
    assert!(versions.contains(r#""slot":"baseline""#));
    assert!(versions.contains(r#""label":"baseline-v1""#));
    assert!(versions.contains(r#""note":"manual checkpoint""#));
    assert!(versions.contains(r#""source":"daemon_test""#));
    assert!(versions.contains(r#""history":[{"action":"save_slot""#));

    let cleared = post_json(&format!("http://{}/v1/memory-admin/clear", bind_addr), "{}")
        .expect("daemon should clear online memory state");
    assert!(cleared.contains(r#""status":"cleared""#));
    assert!(cleared.contains(r#""resident_training_event_count":0"#));

    let empty = wait_for_body(
        &format!("http://{}/v1/memory-state.json", bind_addr),
        |body| {
            body.contains(r#""status":"empty""#)
                && body.contains(r#""label_count":0"#)
                && body.contains(r#""resident_training_event_count":0"#)
        },
    )
    .expect("daemon should expose cleared memory-state route");
    assert!(empty.contains(r#""resident_training_event_count":0"#));

    wait_for_body(
        &format!("http://{}/v1/latest/output.json", bind_addr),
        |body| {
            !body.contains(r#""py_ml_candidate_learned_route""#)
                && body.contains(r#""py_ml_candidate_observe_longer""#)
        },
    )
    .expect("daemon should republish non-learned output after memory clear");

    let loaded = post_json(
        &format!("http://{}/v1/memory-admin/load", bind_addr),
        r#"{"slot":"baseline","strategy":"merge"}"#,
    )
    .expect("daemon should load saved memory slot");
    assert!(loaded.contains(r#""status":"loaded""#));
    assert!(loaded.contains(r#""slot":"baseline""#));
    assert!(loaded.contains(r#""strategy":"merge""#));
    assert!(loaded.contains(r#""label":"baseline-v1""#));

    let imported = post_json(
        &format!("http://{}/v1/memory-admin/load", bind_addr),
        &format!(r#"{{"strategy":"merge","snapshot":{}}}"#, snapshot_export),
    )
    .expect("daemon should import online memory state");
    assert!(imported.contains(r#""status":"loaded""#));
    assert!(imported.contains(r#""imported_pattern_count":1"#));
    assert!(imported.contains(r#""imported_label_count":1"#));
    assert!(imported.contains(r#""strategy":"merge""#));

    let restored = wait_for_body(
        &format!("http://{}/v1/memory-state.json", bind_addr),
        |body| {
            body.contains(r#""pattern_count":1"#)
                && body.contains(r#""label_count":1"#)
                && body.contains(r#""resident_training_event_count":0"#)
        },
    )
    .expect("daemon should restore imported memory-state route");
    assert!(restored.contains(r#""resident_training_event_count":0"#));
    assert!(restored.contains(r#""snapshot_slot_count":1"#));

    wait_for_body(
        &format!("http://{}/v1/latest/output.json", bind_addr),
        |body| {
            body.contains(r#""py_ml_candidate_learned_route""#)
                && body.contains(r#""learned_label":"network_observe_longer""#)
        },
    )
    .expect("daemon should republish learned output after memory import");

    let deleted = post_json(
        &format!("http://{}/v1/memory-admin/delete", bind_addr),
        r#"{"slot":"baseline"}"#,
    )
    .expect("daemon should delete saved memory slot");
    assert!(deleted.contains(r#""status":"deleted""#));
    assert!(deleted.contains(r#""slot":"baseline""#));
    assert!(deleted.contains(r#""label":"baseline-v1""#));
    assert!(deleted.contains(r#""slot_count":0"#));

    stop.store(true, Ordering::Relaxed);
    daemon
        .join()
        .expect("daemon thread should join")
        .expect("daemon should exit cleanly");
    let _ = fs::remove_file(state_path);
}
