use std::fs;
use std::io::Write;
use std::net::{Shutdown, TcpStream};
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use super::command::{ValidationError, ValidationReport, default_out_dir};

const EVENTS: &[&str] = &[
    "external_analysis_failed",
    "external_analysis_circuit_open",
    "external_analysis_recovered",
    "socket_session_collect_failed",
    "socket_session_run_failed",
    "socket_service_recovered",
];

pub fn run_resilience_log_evidence_validation(
    log_source: PathBuf,
    out_dir: Option<PathBuf>,
) -> Result<ValidationReport, ValidationError> {
    let out_dir = out_dir.unwrap_or_else(|| default_out_dir("resilience-log-evidence"));
    prepare_dir(&out_dir)?;
    extract_log_evidence(&log_source, &out_dir)?;
    Ok(ValidationReport {
        name: "runtime resilience log evidence".to_string(),
        out_dir,
        checks: vec![
            "resilience_events_extracted".to_string(),
            "resilience_summary_written".to_string(),
        ],
    })
}

pub fn run_resilience_roundtrip_validation(
    api_addr: Option<&str>,
    out_dir: Option<PathBuf>,
) -> Result<ValidationReport, ValidationError> {
    let out_dir = out_dir.unwrap_or_else(|| default_out_dir("resilience-roundtrip"));
    prepare_dir(&out_dir)?;
    write_roundtrip_artifacts(api_addr.unwrap_or("127.0.0.1:9910"), &out_dir)?;
    Ok(ValidationReport {
        name: "runtime resilience roundtrip".to_string(),
        out_dir,
        checks: vec![
            "external_engine_helpers_written".to_string(),
            "resilience_config_snippet_written".to_string(),
            "resilience_runbook_written".to_string(),
        ],
    })
}

pub fn run_resilience_bundle_validation(
    api_addr: Option<&str>,
    log_source: PathBuf,
    out_dir: Option<PathBuf>,
) -> Result<ValidationReport, ValidationError> {
    let out_dir = out_dir.unwrap_or_else(|| default_out_dir("resilience-validation"));
    prepare_dir(&out_dir)?;
    let roundtrip_dir = out_dir.join("roundtrip");
    let evidence_dir = out_dir.join("evidence");
    fs::create_dir_all(&roundtrip_dir)?;
    fs::create_dir_all(&evidence_dir)?;
    write_roundtrip_artifacts(api_addr.unwrap_or("127.0.0.1:9910"), &roundtrip_dir)?;
    extract_log_evidence(&log_source, &evidence_dir)?;
    write_bundle_index(api_addr.unwrap_or("127.0.0.1:9910"), &log_source, &out_dir)?;
    Ok(ValidationReport {
        name: "runtime resilience validation bundle".to_string(),
        out_dir,
        checks: vec![
            "roundtrip_artifacts_written".to_string(),
            "log_evidence_extracted".to_string(),
            "bundle_index_written".to_string(),
        ],
    })
}

pub fn run_resilience_emit_helper_validation(
    mode: &str,
    output_path: PathBuf,
) -> Result<ValidationReport, ValidationError> {
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    write_helper(&output_path, mode)?;
    Ok(ValidationReport {
        name: "runtime resilience helper emission".to_string(),
        out_dir: output_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from(".")),
        checks: vec![format!("external_engine_{mode}_helper_written")],
    })
}

pub fn run_resilience_drive_bad_json_validation(
    host: &str,
    port: u16,
    count: usize,
    out_dir: Option<PathBuf>,
) -> Result<ValidationReport, ValidationError> {
    let out_dir = out_dir.unwrap_or_else(|| default_out_dir("resilience-drive-bad-json"));
    prepare_dir(&out_dir)?;
    let mut connected = 0usize;
    for _ in 0..count {
        if send_bad_json(host, port).is_ok() {
            connected += 1;
        }
    }
    fs::write(
        out_dir.join("drive-summary.txt"),
        format!(
            "runtime resilience bad-json drive\n\
             ================================\n\n\
             target: {host}:{port}\n\
             attempted: {count}\n\
             connected: {connected}\n\
             note: connection failures are tolerated so this helper can be used before or after service shutdown.\n"
        ),
    )?;
    Ok(ValidationReport {
        name: "runtime resilience bad-json drive".to_string(),
        out_dir,
        checks: vec![
            "bad_json_payloads_attempted".to_string(),
            "connection_failures_tolerated".to_string(),
        ],
    })
}

fn send_bad_json(host: &str, port: u16) -> Result<(), ValidationError> {
    let mut stream = TcpStream::connect((host, port))?;
    stream.write_all(b"{\"bad\":\"json\"\n")?;
    stream.shutdown(Shutdown::Write).ok();
    Ok(())
}

fn prepare_dir(out_dir: &Path) -> Result<(), ValidationError> {
    if out_dir.exists() {
        fs::remove_dir_all(out_dir)?;
    }
    fs::create_dir_all(out_dir)?;
    Ok(())
}

fn extract_log_evidence(log_source: &Path, out_dir: &Path) -> Result<(), ValidationError> {
    let files = resolve_input_files(log_source)?;
    if files.is_empty() {
        return Err(ValidationError::new(format!(
            "no log files found under {}",
            log_source.display()
        )));
    }

    let mut events = Vec::new();
    for file in files {
        let body = fs::read_to_string(&file)?;
        for line in body.lines() {
            if line_has_resilience_signal(line) {
                events.push(line.to_string());
            }
        }
    }

    let event_log = out_dir.join("resilience-events.log");
    let summary = out_dir.join("resilience-summary.txt");
    fs::write(
        &event_log,
        events.join("\n") + if events.is_empty() { "" } else { "\n" },
    )?;
    write_summary(&summary, &events)?;
    Ok(())
}

fn resolve_input_files(input_path: &Path) -> Result<Vec<PathBuf>, ValidationError> {
    if input_path.is_file() {
        return Ok(vec![input_path.to_path_buf()]);
    }
    if input_path.is_dir() {
        let mut files = fs::read_dir(input_path)?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.is_file())
            .collect::<Vec<_>>();
        files.sort();
        return Ok(files);
    }
    Err(ValidationError::new(format!(
        "input path does not exist: {}",
        input_path.display()
    )))
}

fn line_has_resilience_signal(line: &str) -> bool {
    line.contains("backoff_ms=")
        || EVENTS
            .iter()
            .any(|event| line.contains(&format!("event={event}")))
}

fn write_summary(output_summary: &Path, events: &[String]) -> Result<(), ValidationError> {
    let mut summary = String::from(
        "runtime resilience log evidence summary\n\
         =====================================\n\n\
         source events:\n",
    );
    for event in EVENTS {
        let needle = format!("event={event}");
        let count = events.iter().filter(|line| line.contains(&needle)).count();
        summary.push_str(&format!("- {event}: {count}\n"));
    }
    let backoff_count = events
        .iter()
        .filter(|line| line.contains("backoff_ms="))
        .count();
    summary.push_str(&format!("- backoff_ms fields: {backoff_count}\n"));
    fs::write(output_summary, summary)?;
    Ok(())
}

fn write_roundtrip_artifacts(api_addr: &str, out_dir: &Path) -> Result<(), ValidationError> {
    let timeout = out_dir.join("external-timeout.sh");
    let fail = out_dir.join("external-fail.sh");
    let healthy = out_dir.join("external-healthy.sh");
    let config = out_dir.join("resilience-snippet.toml");
    let runbook = out_dir.join("runbook.txt");

    write_helper(&timeout, "timeout")?;
    write_helper(&fail, "fail")?;
    write_helper(&healthy, "healthy")?;
    fs::write(
        &config,
        format!(
            "[external_engine]\n\
             bin = \"{}\"\n\n\
             [resilience]\n\
             external_failure_circuit_threshold = 2\n\
             external_failure_circuit_cooldown_seconds = 10\n\
             socket_failure_backoff_base_ms = 100\n\
             socket_failure_backoff_cap_ms = 800\n",
            fail.display()
        ),
    )?;
    fs::write(
        &runbook,
        runbook_body(api_addr, &timeout, &fail, &healthy, &config),
    )?;
    Ok(())
}

fn write_helper(path: &Path, mode: &str) -> Result<(), ValidationError> {
    let body = match mode {
        "timeout" => {
            "#!/usr/bin/env bash\nset -euo pipefail\nsleep 6\nprintf 'late\\n'\n".to_string()
        }
        "fail" => {
            "#!/usr/bin/env bash\nset -euo pipefail\nprintf 'simulated external engine failure\\n' >&2\nexit 1\n".to_string()
        }
        "healthy" => {
            "#!/usr/bin/env bash\nset -euo pipefail\ncat >/dev/null\nprintf '%s\\n' '[{\"kind\":\"external-engine\",\"name\":\"healthy_probe\",\"summary\":\"simulated healthy external engine response\",\"confidence\":\"advisory\",\"producer_stage\":\"external\",\"producer_pass\":\"fault-injection-helper\",\"data_json\":\"{\\\"mode\\\":\\\"healthy\\\"}\"}]'\n".to_string()
        }
        _ => return Err(ValidationError::new(format!("unknown helper mode: {mode}"))),
    };
    fs::write(path, body)?;
    set_executable(path)?;
    Ok(())
}

#[cfg(unix)]
fn set_executable(path: &Path) -> Result<(), ValidationError> {
    let mut permissions = fs::metadata(path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)?;
    Ok(())
}

#[cfg(not(unix))]
fn set_executable(_path: &Path) -> Result<(), ValidationError> {
    Ok(())
}

fn runbook_body(
    api_addr: &str,
    timeout: &Path,
    fail: &Path,
    healthy: &Path,
    config: &Path,
) -> String {
    format!(
        "runtime resilience roundtrip\n\
         ============================\n\n\
         prepared helpers:\n\
         - fail:    {}\n\
         - timeout: {}\n\
         - healthy: {}\n\n\
         config snippet:\n\
         - {}\n\n\
         recommended drill:\n\n\
         1. point your runtime config at the fail helper first.\n\
         2. run a diagnostics path enough times to cross the threshold.\n\
         3. check logs for event=external_analysis_failed and event=external_analysis_circuit_open.\n\
         4. switch the helper to healthy, wait for cooldown, and run again.\n\
         5. check logs for event=external_analysis_recovered.\n\
         6. if you already have a serve loop on tcp socket 127.0.0.1:9909, drive repeated invalid payloads.\n\
         7. query the API at http://{}/health.\n\n\
         expected socket-side signals:\n\
         - event=socket_session_collect_failed or event=socket_session_run_failed\n\
         - backoff_ms=...\n\
         - event=socket_service_recovered\n",
        fail.display(),
        timeout.display(),
        healthy.display(),
        config.display(),
        api_addr
    )
}

fn write_bundle_index(
    api_addr: &str,
    log_source: &Path,
    out_dir: &Path,
) -> Result<(), ValidationError> {
    fs::write(
        out_dir.join("README.txt"),
        format!(
            "gewyvern runtime resilience validation bundle\n\
             ============================================\n\n\
             api address:\n\
             - {api_addr}\n\n\
             log source:\n\
             - {}\n\n\
             bundle contents:\n\
             - roundtrip/            prepared helper scripts, config snippet, and runbook\n\
             - evidence/             extracted resilience-events.log and resilience-summary.txt\n\n\
             recommended review order:\n\
             1. roundtrip/runbook.txt\n\
             2. evidence/resilience-summary.txt\n\
             3. evidence/resilience-events.log\n",
            log_source.display()
        ),
    )?;
    Ok(())
}
