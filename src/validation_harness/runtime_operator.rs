use std::fs::{self, File};
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;

use super::command::{
    ValidationError, ValidationReport, assert_eq_str, default_out_dir, repo_root, run_cargo_status,
    value_at,
};

pub fn run_runtime_operator_validation(
    out_dir: Option<PathBuf>,
    json_out: Option<PathBuf>,
) -> Result<ValidationReport, ValidationError> {
    let out_dir = out_dir.unwrap_or_else(|| default_out_dir("runtime-operator"));
    if out_dir.exists() {
        fs::remove_dir_all(&out_dir)?;
    }
    fs::create_dir_all(&out_dir)?;

    let run_dir = make_run_dir()?;
    let result = run_operator_cases(&out_dir, &run_dir, json_out.as_deref());
    let _ = fs::remove_dir_all(&run_dir);
    result
}

fn run_operator_cases(
    out_dir: &Path,
    run_dir: &Path,
    json_out: Option<&Path>,
) -> Result<ValidationReport, ValidationError> {
    build_runtime_binaries(out_dir)?;
    let mut checks = Vec::new();

    validate_tcp_operator_path(out_dir, run_dir)?;
    checks.push("tcp_repeated_sessions_refresh_latest_snapshot".to_string());
    checks.push("malformed_ingest_does_not_kill_service_loop".to_string());

    validate_udp_operator_path(out_dir, run_dir)?;
    checks.push("udp_datagram_latest_snapshot_remains_readable".to_string());
    checks.push("training_dataset_sample_ids_roundtrip".to_string());

    write_summary(out_dir, json_out, &checks)?;

    Ok(ValidationReport {
        name: "runtime operator validation".to_string(),
        out_dir: out_dir.to_path_buf(),
        checks,
    })
}

fn validate_tcp_operator_path(out_dir: &Path, run_dir: &Path) -> Result<(), ValidationError> {
    let socket_addr = choose_tcp_addr()?;
    let api_addr = choose_tcp_addr()?;
    let log_file = out_dir.join("tcp-serve.log");
    let mut runtime = start_server("tcp", &socket_addr, &api_addr, &log_file, run_dir)?;

    wait_for_http_fragment(
        &api_addr,
        "/health",
        &out_dir.join("tcp-health.json"),
        "\"ok\":true",
    )?;
    send_template(&socket_addr, "tcp")?;

    let summary = wait_for_json_fragment(
        &api_addr,
        "/v1/latest/summary.json",
        &out_dir.join("tcp-summary.json"),
        "\"primary_module_kind\":\"connection_establishment\"",
    )?;
    assert_eq_str(
        &summary,
        &["primary_module_kind"],
        "connection_establishment",
    )?;
    assert_eq_str(
        &summary,
        &["operator_guidance_action"],
        "avoid_pid_strong_actions",
    )?;

    let export = wait_for_json_fragment(
        &api_addr,
        "/v1/latest/export.json",
        &out_dir.join("tcp-export.json"),
        "\"template_id\":\"handshake_debug\"",
    )?;
    assert_eq_str(&export, &["template_id"], "handshake_debug")?;

    send_template(&socket_addr, "tcp")?;
    let repeated = wait_for_json_fragment(
        &api_addr,
        "/v1/latest/summary.json",
        &out_dir.join("tcp-summary-repeated.json"),
        "\"accepted_facts\":3",
    )?;
    require_number(&repeated, &["accepted_facts"], 3)?;

    let _ = send_invalid_session(&socket_addr);
    wait_for_http_fragment(
        &api_addr,
        "/health",
        &out_dir.join("tcp-health-after-bad.json"),
        "\"ok\":true",
    )?;

    send_template(&socket_addr, "tcp")?;
    wait_for_json_fragment(
        &api_addr,
        "/v1/latest/summary.json",
        &out_dir.join("tcp-summary-after-bad.json"),
        "\"template_id\":\"handshake_debug\"",
    )?;
    runtime.kill_and_wait()
}

fn validate_udp_operator_path(out_dir: &Path, run_dir: &Path) -> Result<(), ValidationError> {
    let socket_addr = choose_tcp_addr()?;
    let api_addr = choose_tcp_addr()?;
    let log_file = out_dir.join("udp-serve.log");
    let mut runtime = start_server("udp", &socket_addr, &api_addr, &log_file, run_dir)?;

    wait_for_http_fragment(
        &api_addr,
        "/health",
        &out_dir.join("udp-health.json"),
        "\"ok\":true",
    )?;
    send_template(&socket_addr, "udp")?;

    let summary = wait_for_json_fragment(
        &api_addr,
        "/v1/latest/summary.json",
        &out_dir.join("udp-summary.json"),
        "\"primary_module_kind\":\"datagram_exchange\"",
    )?;
    assert_eq_str(&summary, &["primary_module_kind"], "datagram_exchange")?;
    assert_eq_str(
        &summary,
        &["operator_guidance_action"],
        "avoid_pid_strong_actions",
    )?;

    let analysis = wait_for_json_fragment(
        &api_addr,
        "/v1/latest/analysis.json",
        &out_dir.join("udp-analysis.json"),
        "\"primary_failure_mode\":\"none\"",
    )?;
    assert_eq_str(&analysis, &["primary_failure_mode"], "none")?;
    value_at(&analysis, &["protocol_flows"])?;

    validate_training_dataset(&api_addr, out_dir)?;
    runtime.kill_and_wait()
}

fn validate_training_dataset(api_addr: &str, out_dir: &Path) -> Result<(), ValidationError> {
    let manifest = wait_for_json_fragment(
        api_addr,
        "/v1/latest/training-dataset.json",
        &out_dir.join("training-dataset.json"),
        "\"kind\":\"training_dataset_manifest\"",
    )?;
    assert_eq_str(&manifest, &["kind"], "training_dataset_manifest")?;
    assert_eq_str(
        &manifest,
        &["split_policies", "default"],
        "name_bucket_mod_10",
    )?;

    let Some(samples) = value_at(&manifest, &["samples"])?.as_array() else {
        return Err(ValidationError::new(
            "training manifest samples is not an array",
        ));
    };
    let sample = samples
        .first()
        .ok_or_else(|| ValidationError::new("training manifest has no samples"))?;
    let sample_id = value_at(sample, &["sample_id"])?
        .as_str()
        .ok_or_else(|| ValidationError::new("training sample id is not a string"))?;
    let sample_path = value_at(sample, &["sample_path"])?
        .as_str()
        .ok_or_else(|| ValidationError::new("training sample path is not a string"))?;
    let sample_body = wait_for_json_fragment(
        api_addr,
        sample_path,
        &out_dir.join("training-sample-000.json"),
        "\"kind\":\"training_example\"",
    )?;
    assert_eq_str(&sample_body, &["sample_id"], sample_id)
}

fn build_runtime_binaries(out_dir: &Path) -> Result<(), ValidationError> {
    run_cargo_status(
        &[
            "build".to_string(),
            "--quiet".to_string(),
            "--bin".to_string(),
            "gewyvern".to_string(),
            "--bin".to_string(),
            "gewyvern_socket_send".to_string(),
        ],
        &out_dir.join("cargo-build.log"),
    )
}

struct RunningRuntime {
    child: Child,
}

impl RunningRuntime {
    fn kill_and_wait(&mut self) -> Result<(), ValidationError> {
        if self.child.try_wait()?.is_none() {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
        Ok(())
    }
}

impl Drop for RunningRuntime {
    fn drop(&mut self) {
        if matches!(self.child.try_wait(), Ok(None)) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn start_server(
    template: &str,
    socket_addr: &str,
    api_addr: &str,
    log_file: &Path,
    run_dir: &Path,
) -> Result<RunningRuntime, ValidationError> {
    let output = File::create(log_file)?;
    let child = Command::new(repo_root().join("target/debug/gewyvern"))
        .current_dir(repo_root())
        .env("XDG_STATE_HOME", run_dir.join("state"))
        .arg("--tcp-socket")
        .arg(socket_addr)
        .arg("--template")
        .arg(template)
        .arg("--serve")
        .arg("--api-socket")
        .arg(api_addr)
        .arg("--json")
        .arg("--summary-only")
        .stdout(Stdio::from(output.try_clone()?))
        .stderr(Stdio::from(output))
        .spawn()
        .map_err(|err| ValidationError::new(format!("failed to start gewyvern: {err}")))?;
    Ok(RunningRuntime { child })
}

fn send_template(socket_addr: &str, template: &str) -> Result<(), ValidationError> {
    run_socket_send(&["--tcp-socket", socket_addr, "--template", template])
}

fn send_invalid_session(socket_addr: &str) -> Result<(), ValidationError> {
    run_socket_send(&[
        "--tcp-socket",
        socket_addr,
        "--raw-line",
        "{\"broken\":true",
    ])
}

fn run_socket_send(args: &[&str]) -> Result<(), ValidationError> {
    let output = Command::new(repo_root().join("target/debug/gewyvern_socket_send"))
        .current_dir(repo_root())
        .args(args)
        .output()
        .map_err(|err| ValidationError::new(format!("failed to run socket sender: {err}")))?;
    if output.status.success() {
        return Ok(());
    }
    Err(ValidationError::new(format!(
        "socket sender failed: {}",
        String::from_utf8_lossy(&output.stderr)
    )))
}

fn choose_tcp_addr() -> Result<String, ValidationError> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    Ok(listener.local_addr()?.to_string())
}

fn wait_for_json_fragment(
    addr: &str,
    path: &str,
    output_path: &Path,
    fragment: &str,
) -> Result<Value, ValidationError> {
    let body = wait_for_http_fragment(addr, path, output_path, fragment)?;
    let split = body
        .split_once("\r\n\r\n")
        .map(|(_, payload)| payload)
        .unwrap_or(body.as_str());
    serde_json::from_str(split).map_err(|err| {
        ValidationError::new(format!(
            "failed to parse JSON response from http://{addr}{path}: {err}"
        ))
    })
}

fn wait_for_http_fragment(
    addr: &str,
    path: &str,
    output_path: &Path,
    fragment: &str,
) -> Result<String, ValidationError> {
    let deadline = Instant::now() + Duration::from_secs(16);
    while Instant::now() < deadline {
        if let Ok(body) = http_get(addr, path) {
            fs::write(output_path, &body)?;
            if body.contains(fragment) {
                return Ok(body);
            }
        }
        thread::sleep(Duration::from_millis(100));
    }
    Err(ValidationError::new(format!(
        "timed out waiting for {fragment} at http://{addr}{path}"
    )))
}

fn http_get(addr: &str, path: &str) -> Result<String, ValidationError> {
    let mut stream = TcpStream::connect(addr)?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"
    )?;
    stream.shutdown(Shutdown::Write).ok();

    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    if !response.starts_with("HTTP/1.1 200") && !response.starts_with("HTTP/1.0 200") {
        return Err(ValidationError::new("HTTP endpoint did not return 200"));
    }
    Ok(response)
}

fn require_number(value: &Value, path: &[&str], expected: i64) -> Result<(), ValidationError> {
    let actual = value_at(value, path)?
        .as_i64()
        .ok_or_else(|| ValidationError::new(format!("expected number at `{}`", path.join("."))))?;
    if actual == expected {
        Ok(())
    } else {
        Err(ValidationError::new(format!(
            "expected `{}` to be {expected}, got {actual}",
            path.join(".")
        )))
    }
}

fn make_run_dir() -> Result<PathBuf, ValidationError> {
    let mut dir = std::env::temp_dir();
    dir.push(format!("gewyvern-runtime-operator-{}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir)?;
    }
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn write_summary(
    out_dir: &Path,
    json_out: Option<&Path>,
    checks: &[String],
) -> Result<(), ValidationError> {
    let covered = checks
        .iter()
        .map(|check| format!("    \"{check}\""))
        .collect::<Vec<_>>()
        .join(",\n");
    let body = format!(
        "{{\n  \"runner\": \"gewyvern_validate runtime-operator\",\n  \"status\": \"ok\",\n  \"evidence_dir\": \"{}\",\n  \"covered_by_script\": [\n{}\n  ],\n  \"requires_operator_confirmation\": [\n    \"ingest_mode_matches_deployment_trust_intent\",\n    \"remote_api_exposure_is_avoided_or_explicitly_opted_in\",\n    \"external_engine_wiring_is_intentional_and_bounded\",\n    \"custom_registry_roots_are_trusted_and_scoped\",\n    \"surrounding_automation_handles_404_and_503_paths\"\n  ]\n}}\n",
        out_dir.display(),
        covered
    );
    fs::write(out_dir.join("summary.json"), &body)?;
    if let Some(json_out) = json_out {
        if let Some(parent) = json_out.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(json_out, body)?;
    }
    Ok(())
}
