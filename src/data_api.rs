use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::render_utils::string_list_json;

pub type ApiState = Arc<Mutex<ApiSnapshot>>;

const API_CLIENT_READ_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Clone, Debug, Default)]
pub struct ApiSnapshot {
    pub updated_unix_ms: u128,
    pub kind: String,
    pub name: Option<String>,
    pub target_count: Option<usize>,
    pub target_names: Vec<String>,
    pub summary_text: Option<String>,
    pub summary_json: Option<String>,
    pub findings_json: Option<String>,
    pub analysis_json: Option<String>,
    pub export_json: Option<String>,
    pub report_json: Option<String>,
    pub report_html: Option<String>,
    pub target_snapshots: HashMap<String, ApiTargetSnapshot>,
}

#[derive(Clone, Debug, Default)]
pub struct ApiTargetSnapshot {
    pub summary_text: String,
    pub summary_json: String,
    pub findings_json: String,
    pub analysis_json: String,
    pub export_json: String,
    pub report_json: String,
    pub report_html: String,
}

#[derive(Clone, Debug)]
pub struct ApiRenderedTarget {
    pub name: String,
    pub summary_text: String,
    pub summary_json: String,
    pub findings_json: String,
    pub analysis_json: String,
    pub export_json: String,
    pub report_json: String,
    pub report_html: String,
}

impl ApiRenderedTarget {
    pub fn into_snapshot(self) -> ApiTargetSnapshot {
        ApiTargetSnapshot {
            summary_text: self.summary_text,
            summary_json: self.summary_json,
            findings_json: self.findings_json,
            analysis_json: self.analysis_json,
            export_json: self.export_json,
            report_json: self.report_json,
            report_html: self.report_html,
        }
    }
}

pub fn update_api_snapshot_for_single(state: &ApiState, rendered: ApiRenderedTarget) {
    let target_name = rendered.name.clone();
    let target_snapshot = rendered.clone().into_snapshot();
    let mut target_snapshots = HashMap::new();
    target_snapshots.insert(target_name.clone(), target_snapshot);
    let mut guard = state.lock().expect("api snapshot mutex poisoned");
    *guard = ApiSnapshot {
        updated_unix_ms: current_unix_ms(),
        kind: "single".into(),
        name: Some(target_name.clone()),
        target_count: Some(1),
        target_names: vec![target_name],
        summary_text: Some(rendered.summary_text),
        summary_json: Some(rendered.summary_json),
        findings_json: Some(rendered.findings_json),
        analysis_json: Some(rendered.analysis_json),
        export_json: Some(rendered.export_json),
        report_json: Some(rendered.report_json),
        report_html: Some(rendered.report_html),
        target_snapshots,
    };
}

pub fn update_api_snapshot_for_scan(
    state: &ApiState,
    targets: Vec<ApiRenderedTarget>,
    summary_text: String,
    summary_json: String,
    analysis_json: String,
    report_json: String,
    report_html: String,
) {
    let mut target_snapshots = HashMap::new();
    let mut target_names = Vec::with_capacity(targets.len());
    for rendered in targets {
        target_names.push(rendered.name.clone());
        target_snapshots.insert(rendered.name.clone(), rendered.into_snapshot());
    }
    let mut guard = state.lock().expect("api snapshot mutex poisoned");
    *guard = ApiSnapshot {
        updated_unix_ms: current_unix_ms(),
        kind: "scan".into(),
        name: None,
        target_count: Some(target_names.len()),
        target_names,
        summary_text: Some(summary_text),
        summary_json: Some(summary_json),
        analysis_json: Some(analysis_json),
        findings_json: None,
        export_json: None,
        report_json: Some(report_json),
        report_html: Some(report_html),
        target_snapshots,
    };
}

pub fn api_snapshot_meta_json(snapshot: &ApiSnapshot) -> String {
    format!(
        "{{\"updated_unix_ms\":{}, {}, {}}}",
        snapshot.updated_unix_ms,
        api_snapshot_index_fields_json(snapshot),
        api_snapshot_presence_fields_json(snapshot),
    )
}

fn api_target_list_json(snapshot: &ApiSnapshot) -> String {
    format!(
        "{{{},\"targets\":{},\"path_segment_encoding\":\"percent-encoding\",\"direct_path_chars\":\"A-Z a-z 0-9 . _ ~ :\"}}",
        api_snapshot_index_fields_json(snapshot),
        string_list_json(&snapshot.target_names),
    )
}

pub fn api_response_for_request(path: &str, snapshot: &ApiSnapshot) -> (u16, &'static str, String) {
    if let Some(rest) = path.strip_prefix("/v1/latest/targets/") {
        if rest.is_empty() {
            return (
                404,
                "application/json; charset=utf-8",
                "{\"error\":\"not_found\"}".into(),
            );
        }
        if let Some((target_name_segment, suffix)) = rest.split_once('/') {
            let target_name = match decode_api_target_path_segment(target_name_segment) {
                Ok(value) => value,
                Err(message) => {
                    return (
                        400,
                        "application/json; charset=utf-8",
                        format!(
                            "{{\"error\":\"invalid_target_path_segment\",\"segment\":{},\"message\":{}}}",
                            json_string(target_name_segment),
                            json_string(message),
                        ),
                    );
                }
            };
            if let Some(target) = snapshot.target_snapshots.get(&target_name) {
                return match suffix {
                    "summary.txt" => (
                        200,
                        "text/plain; charset=utf-8",
                        target.summary_text.clone(),
                    ),
                    "summary.json" => (
                        200,
                        "application/json; charset=utf-8",
                        target.summary_json.clone(),
                    ),
                    "findings.json" => (
                        200,
                        "application/json; charset=utf-8",
                        target.findings_json.clone(),
                    ),
                    "analysis.json" => (
                        200,
                        "application/json; charset=utf-8",
                        target.analysis_json.clone(),
                    ),
                    "export.json" => (
                        200,
                        "application/json; charset=utf-8",
                        target.export_json.clone(),
                    ),
                    "report.json" => (
                        200,
                        "application/json; charset=utf-8",
                        target.report_json.clone(),
                    ),
                    "report.html" => (200, "text/html; charset=utf-8", target.report_html.clone()),
                    _ => (
                        404,
                        "application/json; charset=utf-8",
                        "{\"error\":\"not_found\"}".into(),
                    ),
                };
            }
            return (
                404,
                "application/json; charset=utf-8",
                format!(
                    "{{\"error\":\"unknown_target\",\"target\":{},\"path_segment\":{}}}",
                    json_string(&target_name),
                    json_string(target_name_segment)
                ),
            );
        }
        return (
            400,
            "application/json; charset=utf-8",
            "{\"error\":\"invalid_target_path\",\"expected\":\"/v1/latest/targets/<path-segment>/<resource>\"}".into(),
        );
    }
    match path {
        "/health" => (
            200,
            "application/json; charset=utf-8",
            format!(
                "{{\"ok\":true,\"has_snapshot\":{},\"kind\":{},\"updated_unix_ms\":{}}}",
                !snapshot.kind.is_empty(),
                if snapshot.kind.is_empty() {
                    "null".into()
                } else {
                    json_string(&snapshot.kind)
                },
                snapshot.updated_unix_ms
            ),
        ),
        "/v1/latest/meta" => (
            200,
            "application/json; charset=utf-8",
            api_snapshot_meta_json(snapshot),
        ),
        "/v1/latest/targets" => (
            200,
            "application/json; charset=utf-8",
            api_target_list_json(snapshot),
        ),
        "/v1/capabilities" => (
            200,
            "application/json; charset=utf-8",
            "{\"service\":\"gewyvern-api\",\"version\":\"0.7.0\",\"latest_snapshot\":true,\"serve_required\":true,\"target_path_segment_encoding\":\"percent-encoding\",\"target_direct_path_chars\":\"A-Z a-z 0-9 . _ ~ :\",\"endpoints\":[\"/health\",\"/v1/capabilities\",\"/v1/latest/meta\",\"/v1/latest/targets\",\"/v1/latest/summary.txt\",\"/v1/latest/summary.json\",\"/v1/latest/findings.json\",\"/v1/latest/analysis.json\",\"/v1/latest/export.json\",\"/v1/latest/report.json\",\"/v1/latest/report.html\",\"/v1/latest/targets/<name>/summary.txt\",\"/v1/latest/targets/<name>/summary.json\",\"/v1/latest/targets/<name>/findings.json\",\"/v1/latest/targets/<name>/analysis.json\",\"/v1/latest/targets/<name>/export.json\",\"/v1/latest/targets/<name>/report.json\",\"/v1/latest/targets/<name>/report.html\"]}".into(),
        ),
        "/v1/latest/summary.txt" => match snapshot.summary_text.as_ref() {
            Some(body) => (200, "text/plain; charset=utf-8", body.clone()),
            None => (404, "text/plain; charset=utf-8", "no latest summary available".into()),
        },
        "/v1/latest/summary.json" => match snapshot.summary_json.as_ref() {
            Some(body) => (200, "application/json; charset=utf-8", body.clone()),
            None => (404, "text/plain; charset=utf-8", "no latest summary json available".into()),
        },
        "/v1/latest/findings.json" => match snapshot.findings_json.as_ref() {
            Some(body) => (200, "application/json; charset=utf-8", body.clone()),
            None => (404, "text/plain; charset=utf-8", "no latest findings json available".into()),
        },
        "/v1/latest/analysis.json" => match snapshot.analysis_json.as_ref() {
            Some(body) => (200, "application/json; charset=utf-8", body.clone()),
            None => (404, "text/plain; charset=utf-8", "no latest analysis json available".into()),
        },
        "/v1/latest/export.json" => match snapshot.export_json.as_ref() {
            Some(body) => (200, "application/json; charset=utf-8", body.clone()),
            None => (404, "text/plain; charset=utf-8", "no latest export json available".into()),
        },
        "/v1/latest/report.json" => match snapshot.report_json.as_ref() {
            Some(body) => (200, "application/json; charset=utf-8", body.clone()),
            None => (404, "text/plain; charset=utf-8", "no latest report json available".into()),
        },
        "/v1/latest/report.html" => match snapshot.report_html.as_ref() {
            Some(body) => (200, "text/html; charset=utf-8", body.clone()),
            None => (404, "text/plain; charset=utf-8", "no latest report html available".into()),
        },
        _ => (
            404,
            "application/json; charset=utf-8",
            "{\"error\":\"not_found\",\"paths\":[\"/health\",\"/v1/capabilities\",\"/v1/latest/meta\",\"/v1/latest/targets\",\"/v1/latest/summary.txt\",\"/v1/latest/summary.json\",\"/v1/latest/findings.json\",\"/v1/latest/analysis.json\",\"/v1/latest/export.json\",\"/v1/latest/report.json\",\"/v1/latest/report.html\",\"/v1/latest/targets/<name>/summary.txt\",\"/v1/latest/targets/<name>/summary.json\",\"/v1/latest/targets/<name>/findings.json\",\"/v1/latest/targets/<name>/analysis.json\",\"/v1/latest/targets/<name>/export.json\",\"/v1/latest/targets/<name>/report.json\",\"/v1/latest/targets/<name>/report.html\"]}".into(),
        ),
    }
}

pub fn start_api_service(addr: &str) -> ApiState {
    let listener = TcpListener::bind(addr).unwrap_or_else(|err| {
        eprintln!("failed to bind api socket {}: {}", addr, err);
        std::process::exit(1);
    });
    let state = Arc::new(Mutex::new(ApiSnapshot::default()));
    let thread_state = Arc::clone(&state);
    thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let client_state = Arc::clone(&thread_state);
                    thread::spawn(move || handle_api_client(stream, client_state));
                }
                Err(_) => continue,
            }
        }
    });
    state
}

fn current_unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn json_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn optional_json_string(value: Option<&str>) -> String {
    value.map(json_string).unwrap_or_else(|| "null".into())
}

fn is_api_target_direct_path_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~' | b':')
}

fn api_target_path_segment(name: &str) -> String {
    let mut out = String::new();
    for byte in name.bytes() {
        if is_api_target_direct_path_char(byte) {
            out.push(byte as char);
        } else {
            out.push('%');
            out.push_str(&format!("{:02X}", byte));
        }
    }
    out
}

fn decode_api_target_path_segment(segment: &str) -> Result<String, &'static str> {
    let mut bytes = Vec::with_capacity(segment.len());
    let raw = segment.as_bytes();
    let mut index = 0;
    while index < raw.len() {
        if raw[index] == b'%' {
            if index + 2 >= raw.len() {
                return Err("target path segment ends with an incomplete percent-encoding");
            }
            let hi = (raw[index + 1] as char)
                .to_digit(16)
                .ok_or("target path segment contains an invalid percent-encoding")?;
            let lo = (raw[index + 2] as char)
                .to_digit(16)
                .ok_or("target path segment contains an invalid percent-encoding")?;
            bytes.push(((hi << 4) | lo) as u8);
            index += 3;
        } else {
            bytes.push(raw[index]);
            index += 1;
        }
    }
    String::from_utf8(bytes).map_err(|_| "target path segment is not valid UTF-8")
}

fn api_target_refs_json(target_names: &[String]) -> String {
    format!(
        "[{}]",
        target_names
            .iter()
            .map(|name| {
                let path_segment = api_target_path_segment(name);
                format!(
                    "{{\"name\":{},\"path_segment\":{},\"url_path\":{}}}",
                    json_string(name),
                    json_string(&path_segment),
                    json_string(&format!("/v1/latest/targets/{}", path_segment)),
                )
            })
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn api_snapshot_index_fields_json(snapshot: &ApiSnapshot) -> String {
    format!(
        "\"kind\":{},\"name\":{},\"target_count\":{},\"target_names\":{},\"target_refs\":{}",
        json_string(&snapshot.kind),
        optional_json_string(snapshot.name.as_deref()),
        snapshot
            .target_count
            .map(|value| value.to_string())
            .unwrap_or_else(|| "null".into()),
        string_list_json(&snapshot.target_names),
        api_target_refs_json(&snapshot.target_names),
    )
}

fn api_snapshot_presence_fields_json(snapshot: &ApiSnapshot) -> String {
    format!(
        "\"has_summary_text\":{},\"has_summary_json\":{},\"has_findings_json\":{},\"has_analysis_json\":{},\"has_export_json\":{},\"has_report_json\":{},\"has_report_html\":{}",
        snapshot.summary_text.is_some(),
        snapshot.summary_json.is_some(),
        snapshot.findings_json.is_some(),
        snapshot.analysis_json.is_some(),
        snapshot.export_json.is_some(),
        snapshot.report_json.is_some(),
        snapshot.report_html.is_some(),
    )
}

fn write_http_response(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &str,
) -> std::io::Result<()> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        _ => "OK",
    };
    write!(
        stream,
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        status,
        reason,
        content_type,
        body.len(),
        body
    )
}

fn handle_api_client(mut stream: TcpStream, state: ApiState) {
    let _ = stream.set_read_timeout(Some(API_CLIENT_READ_TIMEOUT));
    let mut buffer = [0u8; 2048];
    let bytes_read = match stream.read(&mut buffer) {
        Ok(bytes) if bytes > 0 => bytes,
        _ => return,
    };
    let request = String::from_utf8_lossy(&buffer[..bytes_read]);
    let first_line = request.lines().next().unwrap_or_default();
    let path = first_line.split_whitespace().nth(1).unwrap_or("/health");
    let snapshot = {
        let guard = state.lock().expect("api snapshot mutex poisoned");
        guard.clone()
    };
    let (status, content_type, body) = api_response_for_request(path, &snapshot);
    let _ = write_http_response(&mut stream, status, content_type, &body);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_target_path_segment_percent_encodes_reserved_bytes() {
        assert_eq!(
            api_target_path_segment("scan/http request"),
            "scan%2Fhttp%20request"
        );
    }

    #[test]
    fn decode_api_target_path_segment_rejects_invalid_escape() {
        let err = decode_api_target_path_segment("%ZZ").expect_err("should reject invalid escape");
        assert!(err.contains("invalid percent-encoding"));
    }
}
