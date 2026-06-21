mod support;

use std::process::Command;
use support::{read_fixture, spawn_http_server};

fn worker_path() -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("scripts")
        .join("python_baseline_worker.py")
        .to_string_lossy()
        .to_string()
}

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
fn binary_analyzes_live_snapshot_with_python_worker() {
    let (addr, handle) = spawn_http_server(vec![(
        "/v1/latest/analysis.json".to_string(),
        read_fixture("direct_signal_analysis.json"),
    )]);

    let stdout = run_bin(&[
        "analyze-python-url",
        &format!("http://{addr}/v1/latest/analysis.json"),
        "--python-worker",
        &worker_path(),
    ]);
    assert!(stdout.contains("\"name\":\"py_ml_candidate_targeted_escalation\""));
    assert!(stdout.contains("\"producer_pass\":\"python_baseline_worker\""));

    handle.join().expect("server thread should exit");
}

#[test]
fn binary_watches_live_snapshot_with_python_worker() {
    let (addr, handle) = spawn_http_server(vec![(
        "/v1/latest/analysis.json".to_string(),
        read_fixture("missing_transition_analysis.json"),
    )]);

    let stdout = run_bin(&[
        "watch-python-url",
        &format!("http://{addr}/v1/latest/analysis.json"),
        "--cycles",
        "1",
        "--interval-ms",
        "1",
        "--python-worker",
        &worker_path(),
    ]);
    assert!(stdout.contains("\"cycle\":1"));
    assert!(stdout.contains("\"source\":\"python-url\""));
    assert!(stdout.contains("\"py_ml_candidate_observe_longer\""));

    handle.join().expect("server thread should exit");
}
