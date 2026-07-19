use std::fs;
#[cfg(test)]
use std::io::{Read, Write};
#[cfg(test)]
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::command::{ValidationError, ValidationReport, default_out_dir};
#[cfg(test)]
use super::http_probe::MAX_HTTP_RESPONSE_BYTES;
use super::http_probe::bounded_http_get_body;

const ETRAGON_ADMIN_TOKEN_HEADER: &str = "X-Etragon-Admin-Token";
const GEWYVERN_ADMIN_TOKEN_HEADER: &str = "X-Gewyvern-Admin-Token";

pub fn run_stack_probe_validation(
    url: &str,
    profile: &str,
    token: Option<&str>,
    output: Option<PathBuf>,
) -> Result<ValidationReport, ValidationError> {
    run_stack_probe_validation_with_header(url, profile, token, ETRAGON_ADMIN_TOKEN_HEADER, output)
}

pub fn run_stack_probe_validation_with_gewyvern_token(
    url: &str,
    profile: &str,
    token: Option<&str>,
    output: Option<PathBuf>,
) -> Result<ValidationReport, ValidationError> {
    run_stack_probe_validation_with_header(url, profile, token, GEWYVERN_ADMIN_TOKEN_HEADER, output)
}

fn run_stack_probe_validation_with_header(
    url: &str,
    profile: &str,
    token: Option<&str>,
    token_header: &str,
    output: Option<PathBuf>,
) -> Result<ValidationReport, ValidationError> {
    let out_dir = default_out_dir("three-module-stack-probe");
    fs::create_dir_all(&out_dir)?;
    let body = wait_for_profile(url, profile, token, token_header, Duration::from_secs(60))?;
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

// The flat signature mirrors the stable native CLI registration contract.
#[allow(clippy::too_many_arguments)]
pub fn run_stack_register_runtime_json(
    name: &str,
    endpoint: &str,
    environment: &str,
    cluster: &str,
    role: &str,
    pairing_token: &str,
    sidecar_endpoint: Option<&str>,
    sidecar_admin_token: Option<&str>,
) -> Result<String, ValidationError> {
    let payload = json!({
        "name": name,
        "endpoint": endpoint,
        "sidecarEndpoint": none_if_empty(sidecar_endpoint),
        "sidecarAdminToken": none_if_empty(sidecar_admin_token),
        "pairingToken": pairing_token,
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
    token_header: &str,
    timeout: Duration,
) -> Result<String, ValidationError> {
    let deadline = Instant::now() + timeout;
    let mut last_error = String::from("no HTTP response received");
    while Instant::now() < deadline {
        match http_get(url, token, token_header) {
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

fn http_get(url: &str, token: Option<&str>, token_header: &str) -> Result<String, ValidationError> {
    if token.is_some_and(|value| value.bytes().any(|byte| byte.is_ascii_control())) {
        return Err(ValidationError::new(
            "stack probe admin token contains control characters",
        ));
    }
    let (host, port, path) = parse_http_url(url)?;
    let host_header = if host.contains(':') {
        format!("[{host}]")
    } else {
        host.clone()
    };
    let mut headers = vec![
        ("Accept", "application/json"),
        ("User-Agent", "gewyvern-validate-stack-probe"),
    ];
    if let Some(token) = token {
        headers.push((token_header, token));
    }
    bounded_http_get_body(
        (host.as_str(), port),
        &format!("{host_header}:{port}"),
        &path,
        &headers,
    )
}

fn parse_http_url(url: &str) -> Result<(String, u16, String), ValidationError> {
    let rest = url
        .strip_prefix("http://")
        .ok_or_else(|| ValidationError::new("only http:// URLs are supported"))?;
    let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
    if authority.is_empty()
        || authority
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte.is_ascii_whitespace())
    {
        return Err(ValidationError::new("HTTP URL authority is invalid"));
    }
    let request_path = format!("/{path}");
    if request_path.bytes().any(|byte| {
        !byte.is_ascii() || byte.is_ascii_control() || byte.is_ascii_whitespace() || byte == b'#'
    }) {
        return Err(ValidationError::new(
            "HTTP URL path must be encoded ASCII without controls, spaces, or fragments",
        ));
    }
    let (host, port) = parse_http_authority(authority)?;
    let port = port
        .parse::<u16>()
        .map_err(|err| ValidationError::new(format!("invalid URL port: {err}")))?;
    Ok((host, port, request_path))
}

fn parse_http_authority(authority: &str) -> Result<(String, &str), ValidationError> {
    if authority.contains('@') {
        return Err(ValidationError::new("HTTP URL userinfo is not supported"));
    }
    if let Some(bracketed) = authority.strip_prefix('[') {
        let (host, suffix) = bracketed
            .split_once(']')
            .ok_or_else(|| ValidationError::new("HTTP URL IPv6 host is missing ']'"))?;
        if host.is_empty() {
            return Err(ValidationError::new("HTTP URL host is empty"));
        }
        let port = if suffix.is_empty() {
            "80"
        } else {
            suffix
                .strip_prefix(':')
                .filter(|value| !value.is_empty())
                .ok_or_else(|| ValidationError::new("HTTP URL IPv6 authority is invalid"))?
        };
        return Ok((host.to_string(), port));
    }
    if authority.matches(':').count() > 1 {
        return Err(ValidationError::new(
            "HTTP URL IPv6 hosts must use brackets",
        ));
    }
    let (host, port) = authority.split_once(':').unwrap_or((authority, "80"));
    if host.is_empty() || port.is_empty() {
        return Err(ValidationError::new("HTTP URL authority is incomplete"));
    }
    Ok((host.to_string(), port))
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    fn read_test_http_headers(stream: &mut TcpStream) -> Vec<u8> {
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("request timeout should configure");
        let mut request = Vec::new();
        while !request.windows(4).any(|window| window == b"\r\n\r\n") {
            let mut chunk = [0u8; 512];
            let size = stream.read(&mut chunk).expect("request should read");
            assert!(size > 0, "request ended before HTTP headers completed");
            request.extend_from_slice(&chunk[..size]);
            assert!(request.len() <= 8192, "request headers exceed test limit");
        }
        request
    }

    #[test]
    fn gewyvern_probe_uses_the_gewyvern_admin_header() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener should bind");
        let addr = listener.local_addr().expect("listener should have address");
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("client should connect");
            let request = read_test_http_headers(&mut stream);
            let request = String::from_utf8_lossy(&request);
            assert!(request.contains("X-Gewyvern-Admin-Token: isolated-token\r\n"));
            assert!(!request.contains("X-Etragon-Admin-Token"));
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}")
                .expect("response should write");
        });

        let body = http_get(
            &format!("http://127.0.0.1:{}/health", addr.port()),
            Some("isolated-token"),
            GEWYVERN_ADMIN_TOKEN_HEADER,
        )
        .expect("authenticated probe should succeed");
        assert_eq!(body, "{}");
        handle.join().expect("server thread should exit cleanly");
    }

    #[test]
    fn probe_rejects_admin_token_header_injection_before_connecting() {
        let err = http_get(
            "http://127.0.0.1:1/health",
            Some("token\r\nInjected: value"),
            GEWYVERN_ADMIN_TOKEN_HEADER,
        )
        .expect_err("control characters should be rejected");
        assert!(err.to_string().contains("control characters"));
        assert!(!err.to_string().contains("Injected"));
    }

    #[test]
    fn probe_rejects_url_request_injection_before_connecting() {
        for url in [
            "http://local host/health",
            "http://localhost/health\r\nInjected:value",
            "http://localhost/a#fragment",
            "http://localhost/雪",
            "http://user@localhost/health",
            "http://::1/health",
            "http://[::1/health",
        ] {
            let error = parse_http_url(url).expect_err("unsafe URL must fail closed");
            assert!(!error.to_string().is_empty(), "{url:?}");
        }
        assert_eq!(
            parse_http_url("http://[::1]:8080/health").unwrap(),
            ("::1".to_string(), 8080, "/health".to_string())
        );
        assert_eq!(
            parse_http_url("http://[::1]/health").unwrap(),
            ("::1".to_string(), 80, "/health".to_string())
        );
    }

    #[test]
    fn probe_rejects_ambiguous_success_status() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener should bind");
        let addr = listener.local_addr().expect("listener should have address");
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("client should connect");
            read_test_http_headers(&mut stream);
            stream
                .write_all(b"HTTP/1.1 200evil NOPE\r\nContent-Length: 2\r\n\r\n{}")
                .expect("response should write");
        });

        let error = http_get(
            &format!("http://127.0.0.1:{}/health", addr.port()),
            None,
            GEWYVERN_ADMIN_TOKEN_HEADER,
        )
        .expect_err("ambiguous status must fail closed");
        assert!(error.to_string().contains("did not return 200"));
        handle.join().expect("server thread should exit cleanly");
    }

    #[test]
    fn probe_rejects_oversized_http_responses() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener should bind");
        let addr = listener.local_addr().expect("listener should have address");
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("client should connect");
            read_test_http_headers(&mut stream);
            let mut response = b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n".to_vec();
            response.resize(MAX_HTTP_RESPONSE_BYTES + 1, b'x');
            stream
                .write_all(&response)
                .expect("oversized response should write");
        });

        let error = http_get(
            &format!("http://127.0.0.1:{}/health", addr.port()),
            None,
            GEWYVERN_ADMIN_TOKEN_HEADER,
        )
        .expect_err("oversized response must fail closed");
        assert!(error.to_string().contains("exceeds"));
        handle.join().expect("server thread should exit cleanly");
    }
}
