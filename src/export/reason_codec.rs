use super::JsonValue;
use crate::ir::{ObservationScope, PayloadByteMatch, PayloadByteSequenceMatch};
use crate::ledger::{FactId, QuicFrameType, QuicPacketType};

mod encode;
mod parse;

pub(super) use self::encode::{reason_json, reason_profile_json};
pub(super) use self::parse::{
    parse_quic_frame_type, parse_quic_packet_type, parse_reason, parse_reason_profile,
};

#[derive(Clone, Debug, Eq, PartialEq)]
struct ReasonTransportPredicateFields {
    scope: ObservationScope,
    l4_proto: Option<u8>,
    byte_matches: Vec<PayloadByteMatch>,
    byte_sequences: Vec<PayloadByteSequenceMatch>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PacketHeaderQualifiers {
    first_byte_mask: Option<u8>,
    first_byte_value: Option<u8>,
    prefix4: Option<u32>,
    byte4_mask: Option<u8>,
    byte4_value: Option<u8>,
    byte13_mask: Option<u8>,
    byte13_value: Option<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DatagramHeaderQualifiers {
    min_len: Option<u32>,
    first_byte_mask: Option<u8>,
    first_byte_value: Option<u8>,
    prefix2: Option<u16>,
    prefix4: Option<u32>,
    byte13_mask: Option<u8>,
    byte13_value: Option<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct QuicPacketHeaderQualifiers {
    min_len: Option<u32>,
    long_header: Option<bool>,
    packet_type: Option<QuicPacketType>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct QuicFrameHeaderQualifiers {
    packet_type: Option<QuicPacketType>,
    frame_type: QuicFrameType,
}

pub(super) fn quic_packet_type_id(value: &QuicPacketType) -> &'static str {
    match value {
        QuicPacketType::Initial => "initial",
        QuicPacketType::ZeroRtt => "0rtt",
        QuicPacketType::Handshake => "handshake",
        QuicPacketType::Retry => "retry",
    }
}

pub(super) fn quic_frame_type_id(value: &QuicFrameType) -> &'static str {
    match value {
        QuicFrameType::Crypto => "crypto",
        QuicFrameType::Ack => "ack",
        QuicFrameType::Stream => "stream",
        QuicFrameType::Datagram => "datagram",
        QuicFrameType::ConnectionClose => "connection_close",
    }
}

pub(super) fn fact_id_array(ids: &[FactId]) -> JsonValue {
    JsonValue::Array(
        ids.iter()
            .map(|id| JsonValue::Number(id.0 as i64))
            .collect(),
    )
}
