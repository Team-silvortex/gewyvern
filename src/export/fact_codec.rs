use super::{ExportError, JsonValue, RejectedFactSummaryItem};
use crate::export::reason_codec::{quic_frame_type_id, quic_packet_type_id};
use crate::ledger::{
    AttachScopeFact, CpuId, DropActionFact, DropVerdict, FactEnvelope, FactId, FactKind,
    FactKindTag, PacketDir, PacketMetaFact, QuicMetaFact, RouteDecisionFact, SessionId,
    SockLineageFact, TcpStateFact, millis_to_system_time, system_time_to_millis,
};
use crate::runtime::{RejectedFact, RejectedFactReason};
use std::collections::BTreeMap;

mod decode;
mod encode;

pub(super) use self::decode::{
    parse_fact, parse_fact_ids, parse_optional_fact_id, parse_optional_u8, parse_optional_u16,
    parse_optional_u32, parse_rejected_fact, parse_rejected_fact_summary,
};
pub(super) use self::encode::{fact_json, rejected_fact_json, rejected_fact_summary_json};

fn comm_to_string(comm: &[u8; 16]) -> String {
    let end = comm
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(comm.len());
    String::from_utf8_lossy(&comm[..end]).to_string()
}

fn string_to_comm(value: &str) -> [u8; 16] {
    let mut comm = [0u8; 16];
    let bytes = value.as_bytes();
    let len = bytes.len().min(comm.len());
    comm[..len].copy_from_slice(&bytes[..len]);
    comm
}

fn parse_optional_u64(value: &JsonValue) -> Result<Option<u64>, ExportError> {
    match value {
        JsonValue::Null => Ok(None),
        JsonValue::Number(value) => Ok(Some(*value as u64)),
        _ => Err(ExportError::InvalidShape("expected optional u64".into())),
    }
}
