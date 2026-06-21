mod support;

use std::process::Command;
use support::{fixture_path, read_fixture, spawn_http_server};

fn run_bin(args: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_etragon"))
        .args(args)
        .output()
        .expect("binary should run");
    assert!(
        output.status.success(),
        "binary should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("stdout should be utf-8")
}

#[test]
fn binary_analyzes_fixture_file() {
    let path = fixture_path("missing_transition_analysis.json");
    let stdout = run_bin(&["analyze-json", path.to_string_lossy().as_ref()]);

    assert!(stdout.contains("\"augmentations\":["));
    assert!(stdout.contains("\"name\":\"ml_candidate_observe_longer\""));
}

#[test]
fn binary_analyzes_live_snapshot_url() {
    let (addr, handle) = spawn_http_server(vec![(
        "/v1/latest/analysis.json".to_string(),
        read_fixture("direct_signal_analysis.json"),
    )]);

    let stdout = run_bin(&[
        "analyze-url",
        &format!("http://{addr}/v1/latest/analysis.json"),
    ]);
    assert!(stdout.contains("\"name\":\"ml_candidate_targeted_escalation\""));

    handle.join().expect("server thread should exit");
}

#[test]
fn binary_analyzes_target_index_batch_url() {
    let (addr, handle) = spawn_http_server(vec![
        (
            "/v1/latest/targets".to_string(),
            read_fixture("targets_index.json"),
        ),
        (
            "/v1/latest/targets/socket_session/analysis.json".to_string(),
            read_fixture("direct_signal_analysis.json"),
        ),
        (
            "/v1/latest/targets/scan:http:request/analysis.json".to_string(),
            read_fixture("missing_transition_analysis.json"),
        ),
    ]);

    let stdout = run_bin(&[
        "analyze-targets-url",
        &format!("http://{addr}/v1/latest/targets"),
    ]);
    assert!(stdout.contains("\"path_segment\":\"socket_session\""));
    assert!(stdout.contains("\"path_segment\":\"scan:http:request\""));
    assert!(stdout.contains("\"name\":\"ml_candidate_targeted_escalation\""));
    assert!(stdout.contains("\"name\":\"ml_candidate_observe_longer\""));
    assert!(stdout.contains("\"recommendation_summary\":["));

    handle.join().expect("server thread should exit");
}

#[test]
fn binary_filters_target_index_batch_by_prefix() {
    let (addr, handle) = spawn_http_server(vec![
        (
            "/v1/latest/targets".to_string(),
            read_fixture("targets_index.json"),
        ),
        (
            "/v1/latest/targets/scan:http:request/analysis.json".to_string(),
            read_fixture("missing_transition_analysis.json"),
        ),
    ]);

    let stdout = run_bin(&[
        "analyze-targets-url",
        &format!("http://{addr}/v1/latest/targets"),
        "--filter",
        "scan:",
    ]);
    assert!(stdout.contains("\"path_segment\":\"scan:http:request\""));
    assert!(!stdout.contains("\"path_segment\":\"socket_session\""));
    assert!(stdout.contains("\"producer_stage\":\"candidate\""));
    assert!(stdout.contains("\"producer_pass\":\"MockMlAdvisoryEngine\""));
    assert!(stdout.contains("\"recommendation_summary\":["));

    handle.join().expect("server thread should exit");
}
