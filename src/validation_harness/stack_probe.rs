use std::fs;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::command::{ValidationError, ValidationReport, default_out_dir};

pub fn run_stack_probe_validation(
    url: &str,
    profile: &str,
    token: Option<&str>,
    output: Option<PathBuf>,
) -> Result<ValidationReport, ValidationError> {
    let out_dir = default_out_dir("three-module-stack-probe");
    fs::create_dir_all(&out_dir)?;
    let body = wait_for_profile(url, profile, token, Duration::from_secs(60))?;
    let output = output.unwrap_or_else(|| out_dir.join(format!("{profile}.json")));
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&output, body)?;
    Ok(ValidationReport {
        name: "three module stack probe".to_string(),
        out_dir,
        checks: vec![format!("profile_{profile}_ok")],
    })
}

pub fn run_stack_register_runtime_json(
    name: &str,
    endpoint: &str,
    environment: &str,
    cluster: &str,
    role: &str,
    sidecar_endpoint: Option<&str>,
    sidecar_admin_token: Option<&str>,
) -> Result<String, ValidationError> {
    let payload = json!({
        "name": name,
        "endpoint": endpoint,
        "sidecarEndpoint": none_if_empty(sidecar_endpoint),
        "sidecarAdminToken": none_if_empty(sidecar_admin_token),
        "pairingToken": "stack-smoke",
        "capabilities": [],
        "tags": {
            "environment": environment,
            "cluster": cluster,
            "role": role,
        },
        "fetchCapabilities": true,
    });
    serde_json::to_string(&payload).map_err(ValidationError::from)
}

pub fn run_stack_json_file_validation(
    input: &Path,
    profile: &str,
) -> Result<ValidationReport, ValidationError> {
    let payload = read_json(input)?;
    check_profile(profile, &payload)?;
    Ok(ValidationReport {
        name: "three module stack JSON check".to_string(),
        out_dir: input
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from(".")),
        checks: vec![format!("profile_{profile}_ok")],
    })
}

pub fn write_stack_resilience_summary(
    healthy_a: &Path,
    healthy_b: &Path,
    degraded_b: &Path,
    output: &Path,
) -> Result<ValidationReport, ValidationError> {
    let a = read_json(healthy_a)?;
    let b = read_json(healthy_b)?;
    let d = read_json(degraded_b)?;
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        output,
        format!(
            "three-module resilience summary\n\
             gw-a healthy: status={} severity={} summary={}\n\
             gw-b healthy: status={} severity={} summary={}\n\
             gw-b degraded: status={} severity={} summary={}\n\
             gw-b degraded socket: failures={} backoff_ms={} status={}\n\
             gw-b degraded actions: {}\n",
            string_at(&a, &["status"])?,
            string_at(&a, &["severity"])?,
            string_at(&a, &["summary"])?,
            string_at(&b, &["status"])?,
            string_at(&b, &["severity"])?,
            string_at(&b, &["summary"])?,
            string_at(&d, &["status"])?,
            string_at(&d, &["severity"])?,
            string_at(&d, &["summary"])?,
            value_at(&d, &["socket_service", "consecutive_failures"])?,
            value_at(&d, &["socket_service", "current_backoff_ms"])?,
            string_at(&d, &["socket_service", "status"])?,
            string_array_at(&d, &["recommended_actions"])?.join(" | "),
        ),
    )?;
    Ok(ValidationReport {
        name: "three module stack resilience summary".to_string(),
        out_dir: output
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from(".")),
        checks: vec!["resilience_summary_written".to_string()],
    })
}

fn wait_for_profile(
    url: &str,
    profile: &str,
    token: Option<&str>,
    timeout: Duration,
) -> Result<String, ValidationError> {
    let deadline = Instant::now() + timeout;
    let mut last_error = String::from("no HTTP response received");
    while Instant::now() < deadline {
        match http_get(url, token) {
            Ok(body) => {
                if profile == "http-ready" {
                    return Ok(body);
                }
                match serde_json::from_str::<Value>(&body) {
                    Ok(payload) => match check_profile(profile, &payload) {
                        Ok(()) => return Ok(body),
                        Err(err) => last_error = err.to_string(),
                    },
                    Err(err) => last_error = format!("invalid JSON response: {err}"),
                }
            }
            Err(err) => last_error = err.to_string(),
        }
        thread::sleep(Duration::from_millis(250));
    }
    Err(ValidationError::new(format!(
        "timed out waiting for stack profile `{profile}` at {url}; last error: {last_error}"
    )))
}

fn check_profile(profile: &str, payload: &Value) -> Result<(), ValidationError> {
    match profile {
        "meta-has-analysis" => expect_bool(payload, &["has_analysis_json"], true),
        "health-degraded" => expect_bool(payload, &["resilience_degraded"], true),
        "resilience-healthy" => {
            expect_string(payload, &["surface"], "runtime_resilience")?;
            expect_string(payload, &["status"], "healthy")?;
            expect_string(payload, &["severity"], "ok")?;
            expect_bool(payload, &["degraded"], false)?;
            expect_string(payload, &["external_analysis", "status"], "healthy")?;
            expect_string(payload, &["socket_service", "status"], "healthy")
        }
        "resilience-degraded" => {
            expect_string(payload, &["surface"], "runtime_resilience")?;
            expect_string(payload, &["status"], "degraded")?;
            expect_string(payload, &["severity"], "warning")?;
            expect_bool(payload, &["degraded"], true)?;
            expect_string(payload, &["socket_service", "status"], "backing_off")?;
            expect_string(payload, &["external_analysis", "status"], "healthy")
        }
        "etragon-status" => {
            let status = string_at(payload, &["status"])?;
            if status == "ready" || status == "degraded" {
                Ok(())
            } else {
                Err(ValidationError::new(
                    "etragon status was not ready/degraded",
                ))
            }
        }
        "etragon-output" => {
            value_at(payload, &["output", "augmentations"])?;
            Ok(())
        }
        "leserpent-runtimes-sidecar" => expect_runtime_detail(payload),
        "leserpent-runtime-detail" => expect_runtime_detail(payload),
        "leserpent-summary" => {
            expect_u64(payload, &["summary", "runtimeCount"], 2)?;
            expect_u64(payload, &["summary", "runtimesWithLatestSnapshot"], 2)?;
            expect_u64(payload, &["summary", "runtimesWithAnalysisJson"], 2)?;
            expect_u64(payload, &["summary", "runtimesWithPairedSidecar"], 1)
        }
        other => Err(ValidationError::new(format!(
            "unknown stack profile: {other}"
        ))),
    }
}

fn expect_runtime_detail(payload: &Value) -> Result<(), ValidationError> {
    let runtimes = value_at(payload, &["runtimes"])?
        .as_array()
        .ok_or_else(|| ValidationError::new("runtimes is not an array"))?;
    let a = runtime_by_name(runtimes, "gw-stack-a")?;
    let b = runtime_by_name(runtimes, "gw-stack-b")?;
    expect_bool(a, &["status", "hasLatestSnapshot"], true)?;
    expect_bool(a, &["status", "hasAnalysisJson"], true)?;
    value_at(a, &["sidecarEndpoint"])?;
    expect_bool(a, &["sidecarStatus", "healthy"], true)?;
    expect_bool(b, &["status", "hasLatestSnapshot"], true)?;
    expect_bool(b, &["status", "hasAnalysisJson"], true)?;
    if !value_at(b, &["sidecarEndpoint"])?.is_null() {
        return Err(ValidationError::new("gw-stack-b should not have a sidecar"));
    }
    Ok(())
}

fn runtime_by_name<'a>(runtimes: &'a [Value], name: &str) -> Result<&'a Value, ValidationError> {
    runtimes
        .iter()
        .find(|runtime| runtime.get("name").and_then(Value::as_str) == Some(name))
        .ok_or_else(|| ValidationError::new(format!("runtime not found: {name}")))
}

fn http_get(url: &str, token: Option<&str>) -> Result<String, ValidationError> {
    let (host, port, path) = parse_http_url(url)?;
    let mut stream = TcpStream::connect((host.as_str(), port))?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: {host}:{port}\r\nAccept: application/json\r\nUser-Agent: gewyvern-validate-stack-probe\r\nConnection: close\r\n"
    )?;
    if let Some(token) = token {
        write!(stream, "X-Etragon-Admin-Token: {token}\r\n")?;
    }
    write!(stream, "\r\n")?;
    stream.shutdown(Shutdown::Write).ok();
    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    if !response.starts_with("HTTP/1.1 200") && !response.starts_with("HTTP/1.0 200") {
        let status = response.lines().next().unwrap_or("<missing status line>");
        return Err(ValidationError::new(format!(
            "HTTP endpoint did not return 200: {status}"
        )));
    }
    let (headers, body) = response
        .split_once("\r\n\r\n")
        .map(|(headers, body)| (headers, body))
        .unwrap_or(("", response.as_str()));
    if headers
        .to_ascii_lowercase()
        .contains("transfer-encoding: chunked")
    {
        return decode_chunked_body(body);
    }
    Ok(body.to_string())
}

fn decode_chunked_body(body: &str) -> Result<String, ValidationError> {
    let mut remaining = body;
    let mut decoded = String::new();
    loop {
        let Some((size_hex, after_size)) = remaining.split_once("\r\n") else {
            return Err(ValidationError::new("invalid chunked HTTP body"));
        };
        let size = usize::from_str_radix(size_hex.trim(), 16)
            .map_err(|err| ValidationError::new(format!("invalid chunk size: {err}")))?;
        if size == 0 {
            return Ok(decoded);
        }
        if after_size.len() < size + 2 {
            return Err(ValidationError::new("truncated chunked HTTP body"));
        }
        decoded.push_str(&after_size[..size]);
        remaining = &after_size[size + 2..];
    }
}

fn parse_http_url(url: &str) -> Result<(String, u16, String), ValidationError> {
    let rest = url
        .strip_prefix("http://")
        .ok_or_else(|| ValidationError::new("only http:// URLs are supported"))?;
    let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
    let (host, port) = authority.split_once(':').unwrap_or((authority, "80"));
    let port = port
        .parse::<u16>()
        .map_err(|err| ValidationError::new(format!("invalid URL port: {err}")))?;
    Ok((host.to_string(), port, format!("/{path}")))
}

fn read_json(path: &Path) -> Result<Value, ValidationError> {
    serde_json::from_str(&fs::read_to_string(path)?).map_err(ValidationError::from)
}

fn none_if_empty(value: Option<&str>) -> Option<&str> {
    value.filter(|item| !item.is_empty())
}

fn value_at<'a>(value: &'a Value, path: &[&str]) -> Result<&'a Value, ValidationError> {
    let mut cursor = value;
    for key in path {
        cursor = cursor
            .get(*key)
            .ok_or_else(|| ValidationError::new(format!("missing JSON key: {}", path.join("."))))?;
    }
    Ok(cursor)
}

fn string_at(value: &Value, path: &[&str]) -> Result<String, ValidationError> {
    value_at(value, path)?
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| {
            ValidationError::new(format!("JSON key is not a string: {}", path.join(".")))
        })
}

fn string_array_at(value: &Value, path: &[&str]) -> Result<Vec<String>, ValidationError> {
    value_at(value, path)?
        .as_array()
        .ok_or_else(|| {
            ValidationError::new(format!("JSON key is not an array: {}", path.join(".")))
        })?
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_string)
                .ok_or_else(|| ValidationError::new("JSON array item is not a string"))
        })
        .collect()
}

fn expect_string(value: &Value, path: &[&str], expected: &str) -> Result<(), ValidationError> {
    let actual = string_at(value, path)?;
    if actual == expected {
        Ok(())
    } else {
        Err(ValidationError::new(format!(
            "expected {}={expected}, got {actual}",
            path.join(".")
        )))
    }
}

fn expect_bool(value: &Value, path: &[&str], expected: bool) -> Result<(), ValidationError> {
    let actual = value_at(value, path)?.as_bool().ok_or_else(|| {
        ValidationError::new(format!("JSON key is not a bool: {}", path.join(".")))
    })?;
    if actual == expected {
        Ok(())
    } else {
        Err(ValidationError::new(format!(
            "expected {}={expected}, got {actual}",
            path.join(".")
        )))
    }
}

fn expect_u64(value: &Value, path: &[&str], expected: u64) -> Result<(), ValidationError> {
    let actual = value_at(value, path)?.as_u64().ok_or_else(|| {
        ValidationError::new(format!("JSON key is not a number: {}", path.join(".")))
    })?;
    if actual == expected {
        Ok(())
    } else {
        Err(ValidationError::new(format!(
            "expected {}={expected}, got {actual}",
            path.join(".")
        )))
    }
}
