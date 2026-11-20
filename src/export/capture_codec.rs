use super::{ExportError, JsonValue};
use crate::live_capture::{CaptureSnapshot, CaptureState};
use std::collections::BTreeMap;

pub(super) fn decode(value: &JsonValue) -> Result<CaptureSnapshot, ExportError> {
    let capture: CaptureSnapshot = serde_json::from_str(&value.render())
        .map_err(|error| ExportError::InvalidValue(format!("capture: {error}")))?;
    if capture.schema_version != 1
        || capture.udp_port == 0
        || capture.run_id.is_empty()
        || (capture.state == CaptureState::Stopped && !capture.final_accounted)
        || (capture.final_accounted
            && (!capture.reconciles() || capture.state == CaptureState::Running))
        || ((capture.has_loss() || capture.state == CaptureState::Failed) && !capture.incomplete)
    {
        return Err(ExportError::InvalidValue(
            "capture accounting or version invalid".into(),
        ));
    }
    Ok(capture)
}

pub(super) fn encode(capture: &CaptureSnapshot) -> JsonValue {
    let counter = |n: u64| JsonValue::String(n.to_string());
    JsonValue::Object(BTreeMap::from([
        (
            "schema_version".into(),
            JsonValue::Number(i64::from(capture.schema_version)),
        ),
        ("run_id".into(), JsonValue::String(capture.run_id.clone())),
        ("cgroup".into(), JsonValue::String(capture.cgroup.clone())),
        (
            "udp_port".into(),
            JsonValue::Number(i64::from(capture.udp_port)),
        ),
        (
            "state".into(),
            JsonValue::String(
                match capture.state {
                    CaptureState::Running => "running",
                    CaptureState::Stopped => "stopped",
                    CaptureState::Failed => "failed",
                }
                .into(),
            ),
        ),
        (
            "kernel".into(),
            JsonValue::Object(BTreeMap::from([
                ("matched".into(), counter(capture.kernel.matched)),
                ("ring_dropped".into(), counter(capture.kernel.ring_dropped)),
                ("unsupported".into(), counter(capture.kernel.unsupported)),
                ("malformed".into(), counter(capture.kernel.malformed)),
                ("no_cookie".into(), counter(capture.kernel.no_cookie)),
            ])),
        ),
        ("received".into(), counter(capture.received)),
        ("retained".into(), counter(capture.retained)),
        (
            "userspace_dropped".into(),
            counter(capture.userspace_dropped),
        ),
        ("decode_rejected".into(), counter(capture.decode_rejected)),
        (
            "final_accounted".into(),
            JsonValue::Bool(capture.final_accounted),
        ),
        ("incomplete".into(), JsonValue::Bool(capture.incomplete)),
    ]))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn capture_metadata_roundtrips_and_rejects_false_completeness() {
        let mut capture = CaptureSnapshot {
            schema_version: 1,
            run_id: "test".into(),
            cgroup: "/test".into(),
            udp_port: 53,
            state: CaptureState::Stopped,
            kernel: crate::live_capture::KernelCounters {
                matched: 5,
                ring_dropped: 1,
                ..Default::default()
            },
            received: 4,
            retained: 3,
            userspace_dropped: 1,
            decode_rejected: 0,
            final_accounted: true,
            incomplete: true,
        };
        assert_eq!(decode(&encode(&capture)).unwrap(), capture);
        capture.incomplete = false;
        assert!(decode(&encode(&capture)).is_err());
        capture.incomplete = true;
        capture.received = 3;
        assert!(decode(&encode(&capture)).is_err());
        capture.received = 4;
        capture.final_accounted = false;
        assert!(decode(&encode(&capture)).is_err());
        capture.final_accounted = true;
        capture.state = CaptureState::Running;
        assert!(decode(&encode(&capture)).is_err());
    }
}
