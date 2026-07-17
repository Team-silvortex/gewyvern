use std::io::{self, BufRead, BufReader, BufWriter, Write};

use leserpent_protocol::MAX_PROTOCOL_MESSAGE_BYTES;
use leserpent_protocol::compatibility_v1::{decode_runtime_collection, decode_status_refresh};
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
    ValidateRuntimeList,
    ValidateStatusRefresh,
}

#[derive(Debug, Serialize)]
struct BridgeResponse {
    request_id: Option<String>,
    ok: bool,
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
    let result = match request.operation {
        BridgeOperation::ValidateRuntimeList => decode_runtime_collection(&payload).map(|_| ()),
        BridgeOperation::ValidateStatusRefresh => decode_status_refresh(&payload).map(|_| ()),
    };
    match result {
        Ok(()) => BridgeResponse {
            request_id: Some(request.request_id),
            ok: true,
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
    fn bridge_accepts_both_canonical_legacy_operations() {
        for (operation, payload) in [
            (
                "validate_runtime_list",
                include_str!("../../tests/fixtures/legacy-runtime-list-response-v1.json"),
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
