use std::io::{self, BufRead, BufReader, BufWriter, Write};

use leserpent_protocol::MAX_PROTOCOL_MESSAGE_BYTES;
use leserpent_protocol::compatibility_v1::{
    decode_orchestra_persistence, decode_runtime_collection, decode_runtime_deployment_request,
    decode_status_refresh, normalize_runtime_deployment_request,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Deserialize)]
struct BridgeRequest {
    request_id: String,
    operation: BridgeOperation,
    payload: Value,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum BridgeOperation {
    NormalizeRuntimeDeploymentRequest,
    ValidateOrchestraPersistence,
    ValidateRuntimeList,
    ValidateRuntimeDeploymentRequest,
    ValidateStatusRefresh,
}

#[derive(Debug, Serialize)]
struct BridgeResponse {
    request_id: Option<String>,
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    payload: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<BridgeError>,
}

#[derive(Debug, Serialize)]
struct BridgeError {
    code: &'static str,
    message: String,
}

enum Frame {
    Eof,
    Data(Vec<u8>),
    Oversized,
}

fn main() -> io::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    serve(
        &mut BufReader::new(stdin.lock()),
        &mut BufWriter::new(stdout.lock()),
    )
}

fn serve(reader: &mut impl BufRead, writer: &mut impl Write) -> io::Result<()> {
    loop {
        let response = match read_capped_line(reader, MAX_PROTOCOL_MESSAGE_BYTES)? {
            Frame::Eof => return Ok(()),
            Frame::Oversized => BridgeResponse {
                request_id: None,
                ok: false,
                payload: None,
                error: Some(BridgeError {
                    code: "oversized_request",
                    message: format!(
                        "bridge request exceeds {} bytes",
                        MAX_PROTOCOL_MESSAGE_BYTES
                    ),
                }),
            },
            Frame::Data(frame) => process_frame(&frame),
        };
        serde_json::to_writer(&mut *writer, &response)?;
        writer.write_all(b"\n")?;
        writer.flush()?;
    }
}

fn process_frame(frame: &[u8]) -> BridgeResponse {
    let request = match serde_json::from_slice::<BridgeRequest>(frame) {
        Ok(request) => request,
        Err(error) => {
            return BridgeResponse {
                request_id: None,
                ok: false,
                payload: None,
                error: Some(BridgeError {
                    code: "invalid_request",
                    message: error.to_string(),
                }),
            };
        }
    };
    let payload = match serde_json::to_vec(&request.payload) {
        Ok(payload) => payload,
        Err(error) => {
            return failure(request.request_id, "invalid_payload", error.to_string());
        }
    };
    let result: Result<Option<Value>, _> = match request.operation {
        BridgeOperation::NormalizeRuntimeDeploymentRequest => {
            normalize_runtime_deployment_request(&payload).and_then(|normalized| {
                serde_json::to_value(normalized).map(Some).map_err(|error| {
                    leserpent_protocol::compatibility_v1::CompatibilityError::InvalidJson(
                        error.to_string(),
                    )
                })
            })
        }
        BridgeOperation::ValidateOrchestraPersistence => {
            decode_orchestra_persistence(&payload).map(|_| None)
        }
        BridgeOperation::ValidateRuntimeList => decode_runtime_collection(&payload).map(|_| None),
        BridgeOperation::ValidateRuntimeDeploymentRequest => {
            decode_runtime_deployment_request(&payload).map(|_| None)
        }
        BridgeOperation::ValidateStatusRefresh => decode_status_refresh(&payload).map(|_| None),
    };
    match result {
        Ok(payload) => BridgeResponse {
            request_id: Some(request.request_id),
            ok: true,
            payload,
            error: None,
        },
        Err(error) => failure(
            request.request_id,
            "compatibility_rejected",
            error.to_string(),
        ),
    }
}

fn failure(request_id: String, code: &'static str, message: String) -> BridgeResponse {
    BridgeResponse {
        request_id: Some(request_id),
        ok: false,
        payload: None,
        error: Some(BridgeError { code, message }),
    }
}

fn read_capped_line(reader: &mut impl BufRead, limit: usize) -> io::Result<Frame> {
    let mut output = Vec::new();
    let mut oversized = false;
    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            return if output.is_empty() && !oversized {
                Ok(Frame::Eof)
            } else if oversized {
                Ok(Frame::Oversized)
            } else {
                Ok(Frame::Data(output))
            };
        }
        let newline = available.iter().position(|byte| *byte == b'\n');
        let consumed = newline.map_or(available.len(), |index| index + 1);
        let content_len = newline.unwrap_or(available.len());
        if !oversized {
            if output.len() + content_len > limit {
                oversized = true;
                output.clear();
            } else {
                output.extend_from_slice(&available[..content_len]);
            }
        }
        reader.consume(consumed);
        if newline.is_some() {
            if output.last() == Some(&b'\r') {
                output.pop();
            }
            return if oversized {
                Ok(Frame::Oversized)
            } else {
                Ok(Frame::Data(output))
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bridge_accepts_all_canonical_legacy_operations() {
        for (operation, payload) in [
            (
                "validate_orchestra_persistence",
                include_str!("../../tests/fixtures/legacy-orchestra-persistence-v1.json"),
            ),
            (
                "validate_runtime_list",
                include_str!("../../tests/fixtures/legacy-runtime-list-response-v1.json"),
            ),
            (
                "validate_runtime_deployment_request",
                include_str!("../../tests/fixtures/legacy-runtime-deployment-request-v1.json"),
            ),
            (
                "validate_status_refresh",
                include_str!("../../tests/fixtures/legacy-runtime-status-refresh-response-v1.json"),
            ),
        ] {
            let frame = format!(
                "{{\"request_id\":\"request-1\",\"operation\":\"{operation}\",\"payload\":{payload}}}"
            );
            let response = process_frame(frame.as_bytes());
            assert!(response.ok, "{operation} should pass");
            assert_eq!(response.request_id.as_deref(), Some("request-1"));
        }
    }

    #[test]
    fn bridge_returns_the_rust_normalized_deployment_envelope() {
        let frame = br#"{"request_id":"request-1","operation":"normalize_runtime_deployment_request","payload":{"runtimeId":"runtime-alpha","request":{"pipelineKind":" capture/http ","requestedBy":" operator-a ","confirmed":true,"requestId":" deploy-001 ","target":" "}}}"#;
        let response = process_frame(frame);
        assert!(response.ok);
        let payload = response.payload.unwrap();
        assert_eq!(payload["request"]["pipelineKind"], "capture/http");
        assert_eq!(payload["request"]["requestedBy"], "operator-a");
        assert_eq!(payload["request"]["requestId"], "deploy-001");
        assert!(payload["request"]["target"].is_null());
    }

    #[test]
    fn capped_reader_drains_an_oversized_frame_before_the_next_request() {
        let source = format!("{}\n{{}}\n", "x".repeat(9));
        let mut reader = BufReader::new(source.as_bytes());
        assert!(matches!(
            read_capped_line(&mut reader, 8).unwrap(),
            Frame::Oversized
        ));
        let Frame::Data(next) = read_capped_line(&mut reader, 8).unwrap() else {
            panic!("second frame should remain readable");
        };
        assert_eq!(next, b"{}");
    }
}
