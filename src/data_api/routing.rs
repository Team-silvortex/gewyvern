use std::borrow::Cow;
use std::io::{Read, Write};
use std::net::TcpStream;

use super::json::{api_target_list_json, decode_api_target_path_segment, json_string};
use super::training_manifest::{
    target_training_dataset_manifest_json, training_dataset_manifest_json,
};
use super::{
    API_CLIENT_READ_TIMEOUT, API_ENDPOINTS_JSON, API_MAX_RESPONSE_BODY_BYTES, API_VERSION,
    ApiSnapshot, ApiState,
};

pub(crate) fn api_response_for_request<'a>(
    path: &str,
    snapshot: &'a ApiSnapshot,
) -> (u16, &'static str, Cow<'a, str>) {
    let response = api_response_for_request_uncapped(path, snapshot);
    cap_api_response(path, response)
}

fn api_response_for_request_uncapped<'a>(
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
                    "training-example.json" => (
                        200,
                        "application/json; charset=utf-8",
                        Cow::Borrowed(target.training_example_json.as_str()),
                    ),
                    "training-dataset.json" => (
                        200,
                        "application/json; charset=utf-8",
                        Cow::Owned(target_training_dataset_manifest_json(&target_name, target)),
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
                    "protocol-surface.json" => match target.protocol_surface_json.as_deref() {
                        Some(body) => (200, "application/json; charset=utf-8", Cow::Borrowed(body)),
                        None => (
                            404,
                            "text/plain; charset=utf-8",
                            Cow::Borrowed("no protocol surface available for target"),
                        ),
                    },
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
            Cow::Owned(super::json::api_snapshot_meta_json(snapshot)),
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
                "{{\"service\":\"gewyvern-api\",\"version\":{},\"latest_snapshot\":true,\"serve_required\":true,\"training_example\":true,\"training_dataset_manifest\":true,\"external_sidecar_context\":true,\"external_capability_profile\":true,\"external_context_status\":true,\"external_sidecar_trust_level\":true,\"external_sidecar_consumption_mode\":true,\"target_path_segment_encoding\":\"percent-encoding\",\"target_direct_path_chars\":\"A-Z a-z 0-9 . _ ~ :\",\"endpoints\":{}}}",
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
        "/v1/latest/training-example.json" => match snapshot.training_example_json.as_ref() {
            Some(body) => (
                200,
                "application/json; charset=utf-8",
                Cow::Borrowed(body.as_str()),
            ),
            None => (
                404,
                "text/plain; charset=utf-8",
                Cow::Borrowed("no latest training example json available"),
            ),
        },
        "/v1/latest/training-dataset.json" => match snapshot.training_example_json.as_ref() {
            Some(_) => (
                200,
                "application/json; charset=utf-8",
                Cow::Owned(training_dataset_manifest_json(snapshot)),
            ),
            None => (
                404,
                "text/plain; charset=utf-8",
                Cow::Borrowed("no latest training dataset manifest available"),
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
            Cow::Owned(format!(
                "{{\"error\":\"not_found\",\"paths\":{}}}",
                API_ENDPOINTS_JSON
            )),
        ),
    }
}

fn cap_api_response<'a>(
    path: &str,
    response: (u16, &'static str, Cow<'a, str>),
) -> (u16, &'static str, Cow<'a, str>) {
    let (status, content_type, body) = response;
    if status == 200 && body.len() > API_MAX_RESPONSE_BODY_BYTES {
        return (
            503,
            "application/json; charset=utf-8",
            Cow::Owned(format!(
                "{{\"error\":\"response_too_large\",\"path\":{},\"bytes\":{},\"max_bytes\":{}}}",
                json_string(path),
                body.len(),
                API_MAX_RESPONSE_BODY_BYTES,
            )),
        );
    }
    (status, content_type, body)
}

pub(super) fn handle_api_client(mut stream: TcpStream, state: ApiState) {
    let _ = stream.set_read_timeout(Some(API_CLIENT_READ_TIMEOUT));
    let _ = stream.set_write_timeout(Some(super::API_CLIENT_WRITE_TIMEOUT));
    let mut buffer = [0u8; 2048];
    let bytes_read = match stream.read(&mut buffer) {
        Ok(bytes) if bytes > 0 => bytes,
        _ => return,
    };
    let request = String::from_utf8_lossy(&buffer[..bytes_read]);
    let first_line = request.lines().next().unwrap_or_default();
    let mut parts = first_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let path = parts.next().unwrap_or("/health");
    if method != "GET" {
        let _ = write_http_response(
            &mut stream,
            405,
            "application/json; charset=utf-8",
            "{\"error\":\"method_not_allowed\",\"allowed\":\"GET\"}",
        );
        return;
    }
    let snapshot = {
        let guard = state.lock().expect("api snapshot mutex poisoned");
        guard.clone()
    };
    let (status, content_type, body) = api_response_for_request(path, &snapshot);
    let _ = write_http_response(&mut stream, status, content_type, body.as_ref());
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
        405 => "Method Not Allowed",
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
