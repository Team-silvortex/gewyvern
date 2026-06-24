use super::*;

fn sample_target(path_segment: &str, output_json: &str) -> TargetDaemonOutput {
    TargetDaemonOutput {
        path_segment: path_segment.to_string(),
        output_json: output_json.to_string(),
        input_json: Some(fixture("missing_transition_analysis.json")),
        recommendation_summary_json: single_output_recommendation_summary(output_json),
        updated_unix_ms: 1234,
        state_hash: "hash".to_string(),
        last_success_unix_ms: Some(1234),
        last_error: None,
        training_history: Vec::new(),
    }
}

#[test]
fn federation_manifest_parses_multiple_runtime_targets() {
    let manifest = r#"{
      "runtimes": [
        {"id": "gw-a", "targets_url": "http://127.0.0.1:9910/v1/latest/targets"},
        {"id": "gw-b", "targets_url": "http://127.0.0.1:9920/v1/latest/targets"}
      ]
    }"#;

    let members = parse_federation_members(manifest).expect("manifest should parse");
    assert_eq!(members.len(), 2);
    assert_eq!(members[0].id, "gw-a");
    assert_eq!(
        members[1].targets_url,
        "http://127.0.0.1:9920/v1/latest/targets"
    );
}

#[test]
fn daemon_federation_summary_rolls_up_resident_targets() {
    let output = "{\"augmentations\":[{\"name\":\"py_ml_candidate_observe_longer\",\"producer_stage\":\"candidate\",\"producer_pass\":\"python_baseline\"}]}";
    let snapshot = DaemonSnapshot {
        source: "python-targets-url".to_string(),
        upstream_url: "http://127.0.0.1:9910/v1/latest/targets".to_string(),
        interval_ms: 1000,
        cycle: 2,
        analysis_runs: 2,
        cache_hits: 0,
        target_count: 2,
        updated_unix_ms: 1234,
        state_hash: "state".to_string(),
        latest_output_json: "null".to_string(),
        latest_input_json: None,
        latest_recommendation_summary_json: "{\"recommendations\":[]}".to_string(),
        target_outputs: vec![
            sample_target("scan:http:request", output),
            sample_target("scan:redis:get", output),
        ],
        last_success_unix_ms: Some(1234),
        last_error: None,
        training_history: vec![TrainingEvent {
            label: "network_observe_longer".to_string(),
            weight: "1".to_string(),
            trained_unix_ms: 1234,
            scope: "latest".to_string(),
        }],
    };

    let summary = federation_summary_json_from_snapshot(&snapshot);
    assert!(summary.contains("\"kind\":\"etragon_federation_summary\""));
    assert!(summary.contains("\"runtime_count\":1"));
    assert!(summary.contains("\"target_count\":2"));
    assert!(summary.contains("\"resident_training_events\":1"));
    assert!(summary.contains("\"target_training_events\":0"));
    assert!(summary.contains("\"py_ml_candidate_observe_longer\""));
}
