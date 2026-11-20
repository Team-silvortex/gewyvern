use super::{BTreeMap, ExportError, JsonValue};
use crate::ledger::UdpOffloadSegment;

// Keep a separate wire tag: legacy readers must reject rather than erase the
// provenance and interpret derived lengths/timestamps as wire observations.
pub(super) const TAG: &str = "packet_meta_udp_segment_v1";

pub(super) fn encode(segment: UdpOffloadSegment) -> JsonValue {
    JsonValue::Object(BTreeMap::from([
        (
            "aggregate_id".into(),
            JsonValue::String(segment.aggregate_id.to_string()),
        ),
        (
            "aggregate_payload_len".into(),
            JsonValue::Number(i64::from(segment.aggregate_payload_len)),
        ),
        (
            "segment_size".into(),
            JsonValue::Number(i64::from(segment.segment_size)),
        ),
        (
            "segment_index".into(),
            JsonValue::Number(i64::from(segment.segment_index)),
        ),
        (
            "segment_count".into(),
            JsonValue::Number(i64::from(segment.segment_count)),
        ),
        (
            "ip_header_len".into(),
            JsonValue::Number(i64::from(segment.ip_header_len)),
        ),
    ]))
}

pub(super) fn decode(value: Option<&JsonValue>) -> Result<UdpOffloadSegment, ExportError> {
    let value = value.ok_or_else(invalid)?;
    let object = value.as_object()?;
    let integer = |key: &str| object.get(key).ok_or_else(invalid)?.as_i64();
    let id = object.get("aggregate_id").ok_or_else(invalid)?.as_str()?;
    let segment = UdpOffloadSegment {
        aggregate_id: id.parse().map_err(|_| invalid())?,
        aggregate_payload_len: u32::try_from(integer("aggregate_payload_len")?)
            .map_err(|_| invalid())?,
        segment_size: u16::try_from(integer("segment_size")?).map_err(|_| invalid())?,
        segment_index: u16::try_from(integer("segment_index")?).map_err(|_| invalid())?,
        segment_count: u16::try_from(integer("segment_count")?).map_err(|_| invalid())?,
        ip_header_len: u8::try_from(integer("ip_header_len")?).map_err(|_| invalid())?,
    };
    if encode(segment) != *value {
        return Err(invalid());
    }
    Ok(segment)
}

pub(super) fn invalid() -> ExportError {
    ExportError::InvalidValue("invalid UDP offload provenance".into())
}
