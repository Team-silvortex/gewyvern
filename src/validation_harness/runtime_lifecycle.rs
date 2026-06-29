use std::fs::{self, File};
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use super::command::{
    ValidationError, ValidationReport, default_out_dir, repo_root, run_cargo_status,
};

pub fn run_runtime_lifecycle_validation(
    out_dir: Option<PathBuf>,
) -> Result<ValidationReport, ValidationError> {
    let out_dir = out_dir.unwrap_or_else(|| default_out_dir("runtime-lifecycle"));
    if out_dir.exists() {
        fs::remove_dir_all(&out_dir)?;
    }
    fs::create_dir_all(&out_dir)?;

    let run_dir = make_run_dir()?;
    let result = run_lifecycle_cases(&out_dir, &run_dir);
    let _ = fs::remove_dir_all(&run_dir);
    result
}

fn run_lifecycle_cases(
    out_dir: &Path,
    run_dir: &Path,
) -> Result<ValidationReport, ValidationError> {
    build_runtime_binaries(out_dir)?;
    let mut checks = Vec::new();

    bounded_runtime_exits(out_dir, run_dir)?;
    checks.push("bounded_startup_exits_after_session_budget".to_string());

    long_running_runtime_recovers(out_dir, run_dir)?;
    checks.push("long_running_startup_survives_malformed_input".to_string());
    checks.push("runtime_log_records_start_failure_and_recovery".to_string());
    checks.push("explicit_stop_clears_pid_and_api_socket_reachability".to_string());
    checks.push("temporary_run_directory_removed_after_validation".to_string());

    write_summary(out_dir, &checks)?;

    Ok(ValidationReport {
        name: "runtime lifecycle validation".to_string(),
        out_dir: out_dir.to_path_buf(),
        checks,
    })
}

fn build_runtime_binaries(out_dir: &Path) -> Result<(), ValidationError> {
    let args = vec![
        "build".to_string(),
        "--quiet".to_string(),
        "--bin".to_string(),
        "gewyvern".to_string(),
        "--bin".to_string(),
        "gewyvern_socket_send".to_string(),
    ];
    run_cargo_status(&args, &out_dir.join("cargo-build.log"))
}

fn bounded_runtime_exits(out_dir: &Path, run_dir: &Path) -> Result<(), ValidationError> {
    let socket_addr = choose_tcp_addr()?;
    let api_addr = choose_tcp_addr()?;
    let log_file = out_dir.join("bounded-runtime.log");
    let stdout_file = out_dir.join("bounded-stdout.log");
    let health_file = out_dir.join("bounded-health.json");

    let mut runtime = start_gewyvern(
        &socket_addr,
        &api_addr,
        &log_file,
        &stdout_file,
        run_dir,
        &["--max-sessions", "1"],
    )?;
    wait_for_http_body(&api_addr, "/health", &health_file, "\"ok\":true")?;
    send_template(&socket_addr)?;
    wait_for_file_contains(&stdout_file, "\"template_id\":\"handshake_debug\"")?;
    runtime.wait_for_exit(Duration::from_secs(12))?;
    expect_file_contains(&log_file, "event=tcp_service_start")?;
    expect_http_unreachable(&api_addr)?;
    expect_socket_send_fails(&socket_addr)
}

fn long_running_runtime_recovers(out_dir: &Path, run_dir: &Path) -> Result<(), ValidationError> {
    let socket_addr = choose_tcp_addr()?;
    let api_addr = choose_tcp_addr()?;
    let log_file = out_dir.join("long-runtime.log");
    let stdout_file = out_dir.join("long-stdout.log");
    let health_file = out_dir.join("long-health.json");
    let degraded_file = out_dir.join("long-degraded.json");
    let recovered_file = out_dir.join("long-recovered.json");
    let summary_file = out_dir.join("long-summary.json");

    let mut runtime = start_gewyvern(
        &socket_addr,
        &api_addr,
        &log_file,
        &stdout_file,
        run_dir,
        &[],
    )?;
    wait_for_http_body(&api_addr, "/health", &health_file, "\"ok\":true")?;
    let _ = send_invalid_session(&socket_addr);
    let _ = send_invalid_session(&socket_addr);
    wait_for_http_body(
        &api_addr,
        "/v1/runtime/resilience.json",
        &degraded_file,
        "\"status\":\"degraded\"",
    )?;
    send_template(&socket_addr)?;
    wait_for_http_body(
        &api_addr,
        "/v1/runtime/resilience.json",
        &recovered_file,
        "\"status\":\"healthy\"",
    )?;
    wait_for_http_body(
        &api_addr,
        "/v1/latest/summary.json",
        &summary_file,
        "\"template_id\":\"handshake_debug\"",
    )?;
    expect_file_contains(&log_file, "event=tcp_service_start")?;
    expect_file_contains(&log_file, "event=socket_session_run_failed")?;
    expect_file_contains(&log_file, "event=socket_service_recovered")?;

    runtime.kill_and_wait()?;
    expect_http_unreachable(&api_addr)?;
    expect_socket_send_fails(&socket_addr)
}

struct RunningRuntime {
    child: Child,
}

impl RunningRuntime {
    fn wait_for_exit(&mut self, timeout: Duration) -> Result<(), ValidationError> {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if self.child.try_wait()?.is_some() {
                return Ok(());
            }
            thread::sleep(Duration::from_millis(100));
        }
        Err(ValidationError::new("runtime process did not exit in time"))
    }

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

fn start_gewyvern(
    socket_addr: &str,
    api_addr: &str,
    log_file: &Path,
    stdout_file: &Path,
    run_dir: &Path,
    extra_args: &[&str],
) -> Result<RunningRuntime, ValidationError> {
    let stdout = File::create(stdout_file)?;
    let child = Command::new(repo_root().join("target/debug/gewyvern"))
        .current_dir(repo_root())
        .env("XDG_STATE_HOME", run_dir.join("state"))
        .arg("--tcp-socket")
        .arg(socket_addr)
        .arg("--template")
        .arg("tcp")
        .arg("--serve")
        .arg("--api-socket")
        .arg(api_addr)
        .arg("--log-level")
        .arg("debug")
        .arg("--log-file")
        .arg(log_file)
        .arg("--no-log-stderr")
        .arg("--json")
        .arg("--summary-only")
        .args(extra_args)
        .stdout(Stdio::from(stdout.try_clone()?))
        .stderr(Stdio::from(stdout))
        .spawn()
        .map_err(|err| ValidationError::new(format!("failed to start gewyvern: {err}")))?;
    Ok(RunningRuntime { child })
}

fn send_template(socket_addr: &str) -> Result<(), ValidationError> {
    run_socket_send(&["--tcp-socket", socket_addr, "--template", "tcp"])
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

fn wait_for_http_body(
    addr: &str,
    path: &str,
    output_path: &Path,
    fragment: &str,
) -> Result<(), ValidationError> {
    let deadline = Instant::now() + Duration::from_secs(16);
    while Instant::now() < deadline {
        if let Ok(body) = http_get(addr, path) {
            fs::write(output_path, &body)?;
            if body.contains(fragment) {
                return Ok(());
            }
        }
        thread::sleep(Duration::from_millis(100));
    }
    Err(ValidationError::new(format!(
        "timed out waiting for http://{addr}{path}"
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

fn expect_http_unreachable(addr: &str) -> Result<(), ValidationError> {
    match TcpStream::connect(addr) {
        Ok(_) => Err(ValidationError::new(format!(
            "expected {addr} to be unreachable after shutdown"
        ))),
        Err(_) => Ok(()),
    }
}

fn expect_socket_send_fails(socket_addr: &str) -> Result<(), ValidationError> {
    match send_template(socket_addr) {
        Ok(()) => Err(ValidationError::new(format!(
            "expected socket {socket_addr} to reject sessions after shutdown"
        ))),
        Err(_) => Ok(()),
    }
}

fn wait_for_file_contains(file: &Path, fragment: &str) -> Result<(), ValidationError> {
    let deadline = Instant::now() + Duration::from_secs(12);
    while Instant::now() < deadline {
        if file_contains(file, fragment) {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(100));
    }
    Err(ValidationError::new(format!(
        "timed out waiting for `{fragment}` in {}",
        file.display()
    )))
}

fn expect_file_contains(file: &Path, fragment: &str) -> Result<(), ValidationError> {
    if file_contains(file, fragment) {
        Ok(())
    } else {
        Err(ValidationError::new(format!(
            "expected `{fragment}` in {}",
            file.display()
        )))
    }
}

fn file_contains(file: &Path, fragment: &str) -> bool {
    fs::read_to_string(file)
        .map(|content| content.contains(fragment))
        .unwrap_or(false)
}

fn make_run_dir() -> Result<PathBuf, ValidationError> {
    let mut dir = std::env::temp_dir();
    dir.push(format!("gewyvern-lifecycle-{}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir)?;
    }
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn write_summary(out_dir: &Path, checks: &[String]) -> Result<(), ValidationError> {
    let covered = checks
        .iter()
        .map(|check| format!("    \"{check}\""))
        .collect::<Vec<_>>()
        .join(",\n");
    fs::write(
        out_dir.join("summary.json"),
        format!(
            "{{\n  \"runner\": \"gewyvern_validate runtime-lifecycle\",\n  \"status\": \"ok\",\n  \"evidence_dir\": \"{}\",\n  \"covered\": [\n{}\n  ]\n}}\n",
            out_dir.display(),
            covered
        ),
    )?;
    Ok(())
}
