use std::borrow::Cow;
use std::io::{Read, Write};
use std::net::{IpAddr, TcpStream};

use super::anomaly_flow_view::api_target_anomaly_flow_json;
use super::certificate_inventory::api_runtime_certificates_json;
use super::certificate_policy::api_runtime_certificate_policy_json;
use super::certificate_state::api_runtime_certificate_state_json;
use super::debug_session::{api_debug_session_json, api_target_debug_session_json};
use super::debugger_console::api_debugger_console_json;
use super::deployment::{accept_deployment, deployment_list_json};
use super::json::{api_target_list_json, decode_api_target_path_segment, json_string};
use super::protocol_catalog::{
    api_protocol_catalog_json, api_protocol_cluster_json, api_protocol_clusters_json,
    api_protocol_reading_for_target_json, api_protocol_summary_json,
    api_protocol_surface_by_name_json,
};
use super::resilience_status::{api_runtime_resilience_json, append_runtime_resilience_flag_json};
use super::runtime_capability_digest::api_runtime_capability_digest_json;
use super::runtime_cluster_attention::{
    api_runtime_cluster_attention_json, api_runtime_cluster_attention_reasons_json,
    api_runtime_cluster_attention_summary_json,
};
use super::runtime_cluster_overview::api_runtime_cluster_overview_json;
use super::training_manifest::{
    target_training_dataset_manifest_json, training_dataset_manifest_json,
};
use super::{
    API_ADMIN_TOKEN_HEADER, API_CLIENT_READ_TIMEOUT, API_ENDPOINTS_JSON,
    API_MAX_RESPONSE_BODY_BYTES, API_VERSION, ApiAccessPolicy, ApiDeploymentState, ApiSnapshot,
    ApiState, api_client_is_loopback,
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
    if let Some(rest) = path.strip_prefix("/v1/protocols/") {
        return protocol_catalog_response(rest);
    }
    if let Some(rest) = path.strip_prefix("/v1/protocol-clusters/") {
        return protocol_cluster_response(rest);
    }
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
                    "anomaly-flow.json" => match api_target_anomaly_flow_json(&target_name, target)
                    {
                        Some(body) => (200, "application/json; charset=utf-8", Cow::Owned(body)),
                        None => (
                            404,
                            "text/plain; charset=utf-8",
                            Cow::Borrowed("no anomaly flow view available for target"),
                        ),
                    },
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
                    "protocol-reading.json" => {
                        match api_protocol_reading_for_target_json(&target_name) {
                            Some(body) => {
                                (200, "application/json; charset=utf-8", Cow::Owned(body))
                            }
                            None => (
                                404,
                                "application/json; charset=utf-8",
                                Cow::Owned(format!(
                                    "{{\"error\":\"protocol_reading_unavailable\",\"target\":{}}}",
                                    json_string(&target_name),
                                )),
                            ),
                        }
                    }
                    "debug-session.json" => (
                        200,
                        "application/json; charset=utf-8",
                        Cow::Owned(api_target_debug_session_json(&target_name, target)),
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
            Cow::Owned({
                let mut body = String::with_capacity(160);
                body.push_str("{\"ok\":true,\"has_snapshot\":");
                body.push_str(if !snapshot.kind.is_empty() {
                    "true"
                } else {
                    "false"
                });
                body.push_str(",\"kind\":");
                if snapshot.kind.is_empty() {
                    body.push_str("null");
                } else {
                    body.push_str(&json_string(&snapshot.kind));
                }
                body.push_str(",\"updated_unix_ms\":");
                body.push_str(&snapshot.updated_unix_ms.to_string());
                body.push(',');
                append_runtime_resilience_flag_json(&mut body);
                body.push('}');
                body
            }),
        ),
        "/v1/runtime/resilience.json" => (
            200,
            "application/json; charset=utf-8",
            Cow::Owned(api_runtime_resilience_json()),
        ),
        "/v1/runtime/certificates.json" => (
            200,
            "application/json; charset=utf-8",
            Cow::Owned(api_runtime_certificates_json()),
        ),
        "/v1/runtime/certificate-policy.json" => (
            200,
            "application/json; charset=utf-8",
            Cow::Owned(api_runtime_certificate_policy_json()),
        ),
        "/v1/runtime/certificate-state.json" => (
            200,
            "application/json; charset=utf-8",
            Cow::Owned(api_runtime_certificate_state_json()),
        ),
        "/v1/protocols" => (
            200,
            "application/json; charset=utf-8",
            Cow::Owned(api_protocol_catalog_json()),
        ),
        "/v1/protocol-clusters" => (
            200,
            "application/json; charset=utf-8",
            Cow::Owned(api_protocol_clusters_json()),
        ),
        "/v1/latest/meta" => (
            200,
            "application/json; charset=utf-8",
            Cow::Owned(super::json::api_snapshot_meta_json(snapshot)),
        ),
        "/v1/latest/runtime-capability-digest.json" => (
            200,
            "application/json; charset=utf-8",
            Cow::Owned(api_runtime_capability_digest_json(snapshot)),
        ),
        "/v1/latest/runtime-cluster-overview.json" => (
            200,
            "application/json; charset=utf-8",
            Cow::Owned(api_runtime_cluster_overview_json(snapshot)),
        ),
        "/v1/latest/runtime-cluster-attention.json" => (
            200,
            "application/json; charset=utf-8",
            Cow::Owned(api_runtime_cluster_attention_json(snapshot)),
        ),
        "/v1/latest/runtime-cluster-attention-reasons.json" => (
            200,
            "application/json; charset=utf-8",
            Cow::Owned(api_runtime_cluster_attention_reasons_json()),
        ),
        "/v1/latest/runtime-cluster-attention-summary.json" => (
            200,
            "application/json; charset=utf-8",
            Cow::Owned(api_runtime_cluster_attention_summary_json(snapshot)),
        ),
        "/v1/latest/debugger-console.json" => (
            200,
            "application/json; charset=utf-8",
            Cow::Owned(api_debugger_console_json(snapshot)),
        ),
        "/v1/latest/debug-session.json" => (
            200,
            "application/json; charset=utf-8",
            Cow::Owned(api_debug_session_json(snapshot)),
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
                "{{\"service\":\"gewyvern-api\",\"version\":{},\"latest_snapshot\":true,\"serve_required\":true,\"training_example\":true,\"training_dataset_manifest\":true,\"protocol_catalog\":true,\"protocol_cluster_catalog\":true,\"protocol_surface_catalog\":true,\"target_protocol_reading\":true,\"debug_session\":true,\"runtime_capability_digest\":true,\"runtime_cluster_overview\":true,\"runtime_cluster_attention\":true,\"runtime_cluster_attention_reasons\":true,\"runtime_cluster_attention_summary\":true,\"debugger_console\":true,\"runtime_certificates\":true,\"runtime_certificate_policy\":true,\"runtime_certificate_state\":true,\"external_sidecar_context\":true,\"external_capability_profile\":true,\"external_context_status\":true,\"external_sidecar_trust_level\":true,\"external_sidecar_consumption_mode\":true,\"target_path_segment_encoding\":\"percent-encoding\",\"target_direct_path_chars\":\"A-Z a-z 0-9 . _ ~ :\",\"endpoints\":{}}}",
                json_string(API_VERSION),
                format_args!(
                    "[\"/v1/deployments\",{}",
                    API_ENDPOINTS_JSON.strip_prefix('[').unwrap_or(API_ENDPOINTS_JSON),
                ),
            )
            .replace(
                "\"latest_snapshot\":true",
                "\"latest_snapshot\":true,\"authenticated_deployment\":true",
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

fn protocol_catalog_response<'a>(rest: &str) -> (u16, &'static str, Cow<'a, str>) {
    if rest.is_empty() {
        return (
            404,
            "application/json; charset=utf-8",
            Cow::Borrowed("{\"error\":\"not_found\"}"),
        );
    }
    if let Some((protocol_name, suffix)) = rest.split_once('/') {
        if let Some(entry_rest) = suffix.strip_prefix("entries/")
            && let Some((entry, tail)) = entry_rest.split_once('/')
        {
            if tail == "surface.json" {
                return match api_protocol_surface_by_name_json(protocol_name, entry) {
                    Some(body) => (200, "application/json; charset=utf-8", Cow::Owned(body)),
                    None => (
                        404,
                        "application/json; charset=utf-8",
                        Cow::Owned(format!(
                            "{{\"error\":\"unknown_protocol_entry\",\"protocol\":{},\"entry\":{}}}",
                            json_string(protocol_name),
                            json_string(entry),
                        )),
                    ),
                };
            }
            return (
                400,
                "application/json; charset=utf-8",
                Cow::Borrowed(
                    "{\"error\":\"invalid_protocol_entry_path\",\"expected\":\"/v1/protocols/<protocol>/entries/<entry>/surface.json\"}",
                ),
            );
        }
        return (
            400,
            "application/json; charset=utf-8",
            Cow::Borrowed(
                "{\"error\":\"invalid_protocol_path\",\"expected\":\"/v1/protocols/<protocol>\"}",
            ),
        );
    }
    match api_protocol_summary_json(rest) {
        Some(body) => (200, "application/json; charset=utf-8", Cow::Owned(body)),
        None => (
            404,
            "application/json; charset=utf-8",
            Cow::Owned(format!(
                "{{\"error\":\"unknown_protocol\",\"protocol\":{}}}",
                json_string(rest),
            )),
        ),
    }
}

fn protocol_cluster_response<'a>(rest: &str) -> (u16, &'static str, Cow<'a, str>) {
    if rest.is_empty() {
        return (
            404,
            "application/json; charset=utf-8",
            Cow::Borrowed("{\"error\":\"unknown_protocol_cluster\"}"),
        );
    }
    match api_protocol_cluster_json(rest) {
        Some(body) => (200, "application/json; charset=utf-8", Cow::Owned(body)),
        None => (
            404,
            "application/json; charset=utf-8",
            Cow::Owned(format!(
                "{{\"error\":\"unknown_protocol_cluster\",\"cluster\":{}}}",
                json_string(rest),
            )),
        ),
    }
}

pub(super) fn handle_api_client(
    mut stream: TcpStream,
    remote_ip: IpAddr,
    state: ApiState,
    deployments: ApiDeploymentState,
    access_policy: ApiAccessPolicy,
) {
    let _ = stream.set_read_timeout(Some(API_CLIENT_READ_TIMEOUT));
    let _ = stream.set_write_timeout(Some(super::API_CLIENT_WRITE_TIMEOUT));
    let request = match read_http_request(&mut stream) {
        Ok(request) => request,
        Err((status, body)) => {
            let _ =
                write_http_response(&mut stream, status, "application/json; charset=utf-8", body);
            return;
        }
    };
    if !request_is_authorized(remote_ip, &request, &access_policy) {
        let _ = write_http_response(
            &mut stream,
            403,
            "application/json; charset=utf-8",
            "{\"error\":\"api_access_denied\",\"reason\":\"gewyvern runtime API requires loopback access or a valid admin token\"}",
        );
        return;
    }
    let first_line = request.lines().next().unwrap_or_default();
    let mut parts = first_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let path = parts.next().unwrap_or("/health");
    if path == "/v1/deployments" && !request_has_valid_admin_token(&request, &access_policy) {
        let _ = write_http_response(
            &mut stream,
            403,
            "application/json; charset=utf-8",
            "{\"error\":\"deployment_auth_required\",\"reason\":\"deployment control always requires a valid gewyvern admin token\"}",
        );
        return;
    }
    if method == "POST" && path == "/v1/deployments" {
        let body = request
            .split_once("\r\n\r\n")
            .map(|(_, body)| body)
            .unwrap_or_default();
        let (status, response) = match deployments.lock() {
            Ok(mut store) => accept_deployment(body, &mut store),
            Err(_) => (
                500,
                "{\"error\":\"deployment_state_unavailable\"}".to_string(),
            ),
        };
        let _ = write_http_response(
            &mut stream,
            status,
            "application/json; charset=utf-8",
            &response,
        );
        return;
    }
    if method == "GET" && path == "/v1/deployments" {
        let (status, response) = match deployments.lock() {
            Ok(store) => (200, deployment_list_json(&store)),
            Err(_) => (
                500,
                "{\"error\":\"deployment_state_unavailable\"}".to_string(),
            ),
        };
        let _ = write_http_response(
            &mut stream,
            status,
            "application/json; charset=utf-8",
            &response,
        );
        return;
    }
    if method != "GET" {
        let _ = write_http_response(
            &mut stream,
            405,
            "application/json; charset=utf-8",
            "{\"error\":\"method_not_allowed\",\"allowed\":\"GET; POST /v1/deployments\"}",
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

fn read_http_request(stream: &mut TcpStream) -> Result<String, (u16, &'static str)> {
    const MAX_HEADER_BYTES: usize = 8 * 1024;
    const MAX_BODY_BYTES: usize = 16 * 1024;
    let mut request = Vec::with_capacity(2048);
    let mut chunk = [0u8; 2048];
    let mut expected_len = None;
    loop {
        let bytes_read = stream
            .read(&mut chunk)
            .map_err(|_| (400, "{\"error\":\"invalid_http_request\"}"))?;
        if bytes_read == 0 {
            break;
        }
        request.extend_from_slice(&chunk[..bytes_read]);
        if request.len() > MAX_HEADER_BYTES + MAX_BODY_BYTES {
            return Err((413, "{\"error\":\"request_too_large\"}"));
        }

        if expected_len.is_none()
            && let Some(header_end) = request.windows(4).position(|window| window == b"\r\n\r\n")
        {
            if header_end > MAX_HEADER_BYTES {
                return Err((413, "{\"error\":\"request_headers_too_large\"}"));
            }
            let headers = std::str::from_utf8(&request[..header_end])
                .map_err(|_| (400, "{\"error\":\"invalid_http_request\"}"))?;
            if request_header_value(headers, "Transfer-Encoding").is_some() {
                return Err((400, "{\"error\":\"chunked_requests_not_supported\"}"));
            }
            let body_len = request_header_value(headers, "Content-Length")
                .map(|value| value.parse::<usize>())
                .transpose()
                .map_err(|_| (400, "{\"error\":\"invalid_content_length\"}"))?
                .unwrap_or(0);
            if body_len > MAX_BODY_BYTES {
                return Err((413, "{\"error\":\"request_body_too_large\"}"));
            }
            expected_len = Some(header_end + 4 + body_len);
        }
        if expected_len.is_some_and(|length| request.len() >= length) {
            request.truncate(expected_len.unwrap());
            break;
        }
    }
    String::from_utf8(request).map_err(|_| (400, "{\"error\":\"invalid_http_request\"}"))
}

fn request_is_authorized(
    remote_ip: IpAddr,
    request_text: &str,
    access_policy: &ApiAccessPolicy,
) -> bool {
    if api_client_is_loopback(remote_ip) {
        return true;
    }
    request_has_valid_admin_token(request_text, access_policy)
}

fn request_has_valid_admin_token(request_text: &str, access_policy: &ApiAccessPolicy) -> bool {
    let Some(expected_token) = access_policy.admin_token.as_deref() else {
        return false;
    };
    request_header_value(request_text, API_ADMIN_TOKEN_HEADER)
        .map(|value| token_equals(value, expected_token))
        .unwrap_or(false)
}

fn token_equals(supplied: &str, expected: &str) -> bool {
    let supplied = supplied.as_bytes();
    let expected = expected.as_bytes();
    let max_len = supplied.len().max(expected.len());
    let mut diff = supplied.len() ^ expected.len();
    for index in 0..max_len {
        let left = supplied.get(index).copied().unwrap_or(0);
        let right = expected.get(index).copied().unwrap_or(0);
        diff |= (left ^ right) as usize;
    }
    diff == 0
}

fn request_header_value<'a>(request_text: &'a str, header_name: &str) -> Option<&'a str> {
    let mut matched = None;
    for line in request_text.lines().skip(1) {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        if name.trim().eq_ignore_ascii_case(header_name) {
            if matched.is_some() {
                return None;
            }
            matched = Some(value.trim());
        }
    }
    matched
}

fn write_http_response(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &str,
) -> std::io::Result<()> {
    let reason = match status {
        200 => "OK",
        202 => "Accepted",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        413 => "Payload Too Large",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "Unknown",
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_requests_require_matching_admin_token() {
        let policy = ApiAccessPolicy {
            allow_remote_bind: true,
            admin_token: Some("secret-token".into()),
        };
        assert!(!request_is_authorized(
            "10.0.0.5".parse().unwrap(),
            "GET /health HTTP/1.1\r\nHost: remote\r\n\r\n",
            &policy,
        ));
        assert!(request_is_authorized(
            "10.0.0.5".parse().unwrap(),
            "GET /health HTTP/1.1\r\nHost: remote\r\nX-Gewyvern-Admin-Token: secret-token\r\n\r\n",
            &policy,
        ));
        assert!(!request_is_authorized(
            "10.0.0.5".parse().unwrap(),
            "GET /health HTTP/1.1\r\nHost: remote\r\nX-Gewyvern-Admin-Token: wrong-token\r\n\r\n",
            &policy,
        ));
    }

    #[test]
    fn remote_token_checks_trim_match_case_insensitively_and_reject_duplicates() {
        let policy = ApiAccessPolicy {
            allow_remote_bind: true,
            admin_token: Some("secret-token".into()),
        };
        assert!(request_is_authorized(
            "10.0.0.5".parse().unwrap(),
            "GET /health HTTP/1.1\r\nHost: remote\r\nx-gewyvern-admin-token:   secret-token   \r\n\r\n",
            &policy,
        ));
        assert!(!request_is_authorized(
            "10.0.0.5".parse().unwrap(),
            "GET /health HTTP/1.1\r\nHost: remote\r\nX-Gewyvern-Admin-Token: secret-token\r\nX-Gewyvern-Admin-Token: secret-token\r\n\r\n",
            &policy,
        ));
    }

    #[test]
    fn loopback_requests_are_allowed_without_token() {
        let policy = ApiAccessPolicy {
            allow_remote_bind: false,
            admin_token: None,
        };
        assert!(request_is_authorized(
            "127.0.0.1".parse().unwrap(),
            "GET /health HTTP/1.1\r\nHost: localhost\r\n\r\n",
            &policy,
        ));
    }

    #[test]
    fn deployment_control_requires_token_even_for_loopback_clients() {
        let policy = ApiAccessPolicy {
            allow_remote_bind: false,
            admin_token: Some("secret-token".into()),
        };
        assert!(!request_has_valid_admin_token(
            "POST /v1/deployments HTTP/1.1\r\nHost: localhost\r\n\r\n",
            &policy,
        ));
        assert!(request_has_valid_admin_token(
            "POST /v1/deployments HTTP/1.1\r\nHost: localhost\r\nX-Gewyvern-Admin-Token: secret-token\r\n\r\n",
            &policy,
        ));
    }

    #[test]
    fn capabilities_advertise_authenticated_deployment_control() {
        let snapshot = ApiSnapshot::default();
        let (status, _, body) = api_response_for_request("/v1/capabilities", &snapshot);
        assert_eq!(status, 200);
        assert!(body.contains("\"authenticated_deployment\":true"));
        assert!(body.contains("\"/v1/deployments\""));
    }
}
