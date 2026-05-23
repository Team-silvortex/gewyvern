use std::borrow::Cow;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::render_utils::append_string_list_json;

pub type ApiState = Arc<Mutex<Arc<ApiSnapshot>>>;

const API_CLIENT_READ_TIMEOUT: Duration = Duration::from_secs(3);
const API_VERSION: &str = env!("CARGO_PKG_VERSION");
const API_ENDPOINTS_JSON: &str = "[\"/health\",\"/v1/capabilities\",\"/v1/latest/meta\",\"/v1/latest/targets\",\"/v1/latest/summary.txt\",\"/v1/latest/summary.json\",\"/v1/latest/findings.json\",\"/v1/latest/analysis.json\",\"/v1/latest/export.json\",\"/v1/latest/report.json\",\"/v1/latest/report.html\",\"/v1/latest/targets/<name>/summary.txt\",\"/v1/latest/targets/<name>/summary.json\",\"/v1/latest/targets/<name>/findings.json\",\"/v1/latest/targets/<name>/analysis.json\",\"/v1/latest/targets/<name>/export.json\",\"/v1/latest/targets/<name>/report.json\",\"/v1/latest/targets/<name>/report.html\"]";

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
    *guard = Arc::new(ApiSnapshot {
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
    });
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
    *guard = Arc::new(ApiSnapshot {
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
    });
}

pub fn api_snapshot_meta_json(snapshot: &ApiSnapshot) -> String {
    let mut json = String::with_capacity(estimate_api_snapshot_meta_capacity(snapshot));
    json.push_str("{\"updated_unix_ms\":");
    json.push_str(&snapshot.updated_unix_ms.to_string());
    json.push_str(", ");
    append_api_snapshot_index_fields_json(&mut json, snapshot);
    json.push_str(", ");
    append_api_snapshot_presence_fields_json(&mut json, snapshot);
    json.push('}');
    json
}

fn api_target_list_json(snapshot: &ApiSnapshot) -> String {
    let mut json = String::with_capacity(estimate_api_target_list_capacity(snapshot));
    json.push('{');
    append_api_snapshot_index_fields_json(&mut json, snapshot);
    json.push_str(",\"targets\":");
    append_string_list_json(&mut json, &snapshot.target_names);
    json.push_str(",\"path_segment_encoding\":\"percent-encoding\",\"direct_path_chars\":\"A-Z a-z 0-9 . _ ~ :\"}");
    json
}

pub fn api_response_for_request<'a>(
    path: &str,
    snapshot: &'a ApiSnapshot,
) -> (u16, &'static str, Cow<'a, str>) {
    if let Some(rest) = path.strip_prefix("/v1/latest/targets/") {
        if rest.is_empty() {
            return (
                404,
                "application/json; charset=utf-8",
                Cow::Borrowed("{\"error\":\"not_found\"}"),
            );
        }
        if let Some((target_name_segment, suffix)) = rest.split_once('/') {
            let target_name = match decode_api_target_path_segment(target_name_segment) {
                Ok(value) => value,
                Err(message) => {
                    return (
                        400,
                        "application/json; charset=utf-8",
                        Cow::Owned(format!(
                            "{{\"error\":\"invalid_target_path_segment\",\"segment\":{},\"message\":{}}}",
                            json_string(target_name_segment),
                            json_string(message),
                        )),
                    );
                }
            };
            if let Some(target) = snapshot.target_snapshots.get(&target_name) {
                return match suffix {
                    "summary.txt" => (
                        200,
                        "text/plain; charset=utf-8",
                        Cow::Borrowed(target.summary_text.as_str()),
                    ),
                    "summary.json" => (
                        200,
                        "application/json; charset=utf-8",
                        Cow::Borrowed(target.summary_json.as_str()),
                    ),
                    "findings.json" => (
                        200,
                        "application/json; charset=utf-8",
                        Cow::Borrowed(target.findings_json.as_str()),
                    ),
                    "analysis.json" => (
                        200,
                        "application/json; charset=utf-8",
                        Cow::Borrowed(target.analysis_json.as_str()),
                    ),
                    "export.json" => (
                        200,
                        "application/json; charset=utf-8",
                        Cow::Borrowed(target.export_json.as_str()),
                    ),
                    "report.json" => (
                        200,
                        "application/json; charset=utf-8",
                        Cow::Borrowed(target.report_json.as_str()),
                    ),
                    "report.html" => (
                        200,
                        "text/html; charset=utf-8",
                        Cow::Borrowed(target.report_html.as_str()),
                    ),
                    _ => (
                        404,
                        "application/json; charset=utf-8",
                        Cow::Borrowed("{\"error\":\"not_found\"}"),
                    ),
                };
            }
            return (
                404,
                "application/json; charset=utf-8",
                Cow::Owned(format!(
                    "{{\"error\":\"unknown_target\",\"target\":{},\"path_segment\":{}}}",
                    json_string(&target_name),
                    json_string(target_name_segment)
                )),
            );
        }
        return (
            400,
            "application/json; charset=utf-8",
            Cow::Borrowed(
                "{\"error\":\"invalid_target_path\",\"expected\":\"/v1/latest/targets/<path-segment>/<resource>\"}",
            ),
        );
    }
    match path {
        "/health" => (
            200,
            "application/json; charset=utf-8",
            Cow::Owned(format!(
                "{{\"ok\":true,\"has_snapshot\":{},\"kind\":{},\"updated_unix_ms\":{}}}",
                !snapshot.kind.is_empty(),
                if snapshot.kind.is_empty() {
                    "null".into()
                } else {
                    json_string(&snapshot.kind)
                },
                snapshot.updated_unix_ms
            )),
        ),
        "/v1/latest/meta" => (
            200,
            "application/json; charset=utf-8",
            Cow::Owned(api_snapshot_meta_json(snapshot)),
        ),
        "/v1/latest/targets" => (
            200,
            "application/json; charset=utf-8",
            Cow::Owned(api_target_list_json(snapshot)),
        ),
        "/v1/capabilities" => (
            200,
            "application/json; charset=utf-8",
            Cow::Owned(format!(
                "{{\"service\":\"gewyvern-api\",\"version\":{},\"latest_snapshot\":true,\"serve_required\":true,\"target_path_segment_encoding\":\"percent-encoding\",\"target_direct_path_chars\":\"A-Z a-z 0-9 . _ ~ :\",\"endpoints\":{}}}",
                json_string(API_VERSION),
                API_ENDPOINTS_JSON,
            )),
        ),
        "/v1/latest/summary.txt" => match snapshot.summary_text.as_ref() {
            Some(body) => (
                200,
                "text/plain; charset=utf-8",
                Cow::Borrowed(body.as_str()),
            ),
            None => (
                404,
                "text/plain; charset=utf-8",
                Cow::Borrowed("no latest summary available"),
            ),
        },
        "/v1/latest/summary.json" => match snapshot.summary_json.as_ref() {
            Some(body) => (
                200,
                "application/json; charset=utf-8",
                Cow::Borrowed(body.as_str()),
            ),
            None => (
                404,
                "text/plain; charset=utf-8",
                Cow::Borrowed("no latest summary json available"),
            ),
        },
        "/v1/latest/findings.json" => match snapshot.findings_json.as_ref() {
            Some(body) => (
                200,
                "application/json; charset=utf-8",
                Cow::Borrowed(body.as_str()),
            ),
            None => (
                404,
                "text/plain; charset=utf-8",
                Cow::Borrowed("no latest findings json available"),
            ),
        },
        "/v1/latest/analysis.json" => match snapshot.analysis_json.as_ref() {
            Some(body) => (
                200,
                "application/json; charset=utf-8",
                Cow::Borrowed(body.as_str()),
            ),
            None => (
                404,
                "text/plain; charset=utf-8",
                Cow::Borrowed("no latest analysis json available"),
            ),
        },
        "/v1/latest/export.json" => match snapshot.export_json.as_ref() {
            Some(body) => (
                200,
                "application/json; charset=utf-8",
                Cow::Borrowed(body.as_str()),
            ),
            None => (
                404,
                "text/plain; charset=utf-8",
                Cow::Borrowed("no latest export json available"),
            ),
        },
        "/v1/latest/report.json" => match snapshot.report_json.as_ref() {
            Some(body) => (
                200,
                "application/json; charset=utf-8",
                Cow::Borrowed(body.as_str()),
            ),
            None => (
                404,
                "text/plain; charset=utf-8",
                Cow::Borrowed("no latest report json available"),
            ),
        },
        "/v1/latest/report.html" => match snapshot.report_html.as_ref() {
            Some(body) => (
                200,
                "text/html; charset=utf-8",
                Cow::Borrowed(body.as_str()),
            ),
            None => (
                404,
                "text/plain; charset=utf-8",
                Cow::Borrowed("no latest report html available"),
            ),
        },
        _ => (
            404,
            "application/json; charset=utf-8",
            Cow::Borrowed(
                "{\"error\":\"not_found\",\"paths\":[\"/health\",\"/v1/capabilities\",\"/v1/latest/meta\",\"/v1/latest/targets\",\"/v1/latest/summary.txt\",\"/v1/latest/summary.json\",\"/v1/latest/findings.json\",\"/v1/latest/analysis.json\",\"/v1/latest/export.json\",\"/v1/latest/report.json\",\"/v1/latest/report.html\",\"/v1/latest/targets/<name>/summary.txt\",\"/v1/latest/targets/<name>/summary.json\",\"/v1/latest/targets/<name>/findings.json\",\"/v1/latest/targets/<name>/analysis.json\",\"/v1/latest/targets/<name>/export.json\",\"/v1/latest/targets/<name>/report.json\",\"/v1/latest/targets/<name>/report.html\"]}",
            ),
        ),
    }
}

pub fn start_api_service(addr: &str) -> ApiState {
    let listener = TcpListener::bind(addr).unwrap_or_else(|err| {
        eprintln!("failed to bind api socket {}: {}", addr, err);
        std::process::exit(1);
    });
    let state = Arc::new(Mutex::new(Arc::new(ApiSnapshot::default())));
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
    let mut json = String::new();
    append_json_string(&mut json, value);
    json
}

fn append_json_string(target: &mut String, value: &str) {
    target.push('"');
    for ch in value.chars() {
        match ch {
            '\\' => target.push_str("\\\\"),
            '"' => target.push_str("\\\""),
            _ => target.push(ch),
        }
    }
    target.push('"');
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

fn append_api_target_refs_json(target: &mut String, target_names: &[String]) {
    target.push('[');
    for (index, name) in target_names.iter().enumerate() {
        if index > 0 {
            target.push(',');
        }
        let path_segment = api_target_path_segment(name);
        target.push_str("{\"name\":");
        append_json_string(target, name);
        target.push_str(",\"path_segment\":");
        append_json_string(target, &path_segment);
        target.push_str(",\"url_path\":\"/v1/latest/targets/");
        target.push_str(&path_segment);
        target.push_str("\"}");
    }
    target.push(']');
}

fn append_api_snapshot_index_fields_json(target: &mut String, snapshot: &ApiSnapshot) {
    target.push_str("\"kind\":");
    append_json_string(target, &snapshot.kind);
    target.push_str(",\"name\":");
    if let Some(name) = snapshot.name.as_deref() {
        append_json_string(target, name);
    } else {
        target.push_str("null");
    }
    target.push_str(",\"target_count\":");
    if let Some(count) = snapshot.target_count {
        target.push_str(&count.to_string());
    } else {
        target.push_str("null");
    }
    target.push_str(",\"target_names\":");
    append_string_list_json(target, &snapshot.target_names);
    target.push_str(",\"target_refs\":");
    append_api_target_refs_json(target, &snapshot.target_names);
}

fn append_api_snapshot_presence_fields_json(target: &mut String, snapshot: &ApiSnapshot) {
    target.push_str("\"has_summary_text\":");
    target.push_str(if snapshot.summary_text.is_some() {
        "true"
    } else {
        "false"
    });
    target.push_str(",\"has_summary_json\":");
    target.push_str(if snapshot.summary_json.is_some() {
        "true"
    } else {
        "false"
    });
    target.push_str(",\"has_findings_json\":");
    target.push_str(if snapshot.findings_json.is_some() {
        "true"
    } else {
        "false"
    });
    target.push_str(",\"has_analysis_json\":");
    target.push_str(if snapshot.analysis_json.is_some() {
        "true"
    } else {
        "false"
    });
    target.push_str(",\"has_export_json\":");
    target.push_str(if snapshot.export_json.is_some() {
        "true"
    } else {
        "false"
    });
    target.push_str(",\"has_report_json\":");
    target.push_str(if snapshot.report_json.is_some() {
        "true"
    } else {
        "false"
    });
    target.push_str(",\"has_report_html\":");
    target.push_str(if snapshot.report_html.is_some() {
        "true"
    } else {
        "false"
    });
}

fn estimate_api_snapshot_meta_capacity(snapshot: &ApiSnapshot) -> usize {
    192 + snapshot.kind.len()
        + snapshot.name.as_ref().map_or(4, String::len)
        + snapshot.target_names.iter().map(String::len).sum::<usize>() * 3
}

fn estimate_api_target_list_capacity(snapshot: &ApiSnapshot) -> usize {
    128 + snapshot.kind.len() + snapshot.target_names.iter().map(String::len).sum::<usize>() * 4
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
    let _ = write_http_response(&mut stream, status, content_type, body.as_ref());
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
