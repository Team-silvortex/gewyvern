use super::super::ExportError;
use super::super::fact_codec::{
    parse_fact_ids, parse_optional_u8, parse_optional_u16, parse_optional_u32,
};
use super::super::json::JsonValue;
use super::{
    DatagramHeaderQualifiers, PacketHeaderQualifiers, QuicFrameHeaderQualifiers,
    QuicPacketHeaderQualifiers, ReasonTransportPredicateFields,
};
use crate::flow::FlowId;
use crate::ir::{
    NarrativeTemplate, ObservationScope, PayloadByteMatch, PayloadByteSequenceMatch, SignalKind,
};
use crate::ledger::{FactId, PacketDir, QuicFrameType, QuicPacketType};
use crate::reason::{
    KeyEvent, KeyEventKind, NarrLine, ReasonChain, ReasonId, ReasonKeyEvent, ReasonL1, ReasonL3,
    ReasonModel, ReasonNarrative, ReasonPredicate, ReasonProfile, ReasonRule,
};
use std::collections::BTreeMap;

pub(crate) fn parse_reason_profile(value: &JsonValue) -> Result<ReasonProfile, ExportError> {
    match value {
        JsonValue::String(id) => ReasonProfile::from_id(id)
            .ok_or_else(|| ExportError::InvalidValue(format!("unknown reason profile '{id}'"))),
        JsonValue::Object(object) => {
            let id = object
                .get("id")
                .ok_or_else(|| ExportError::InvalidShape("reason_profile.id".into()))?
                .as_str()?;
            let kind = object
                .get("kind")
                .ok_or_else(|| ExportError::InvalidShape("reason_profile.kind".into()))?
                .as_str()?;
            if kind != "declarative" {
                return Err(ExportError::InvalidValue(format!(
                    "unknown reason profile kind '{kind}'"
                )));
            }
            let rules = object
                .get("rules")
                .ok_or_else(|| ExportError::InvalidShape("reason_profile.rules".into()))?
                .as_array()?
                .iter()
                .map(parse_reason_rule)
                .collect::<Result<Vec<_>, _>>()?;
            Ok(ReasonProfile::Declarative(ReasonModel {
                id: Box::leak(id.to_string().into_boxed_str()),
                rules,
            }))
        }
        _ => Err(ExportError::InvalidShape("reason_profile".into())),
    }
}

fn parse_reason_rule(value: &JsonValue) -> Result<ReasonRule, ExportError> {
    let object = value.as_object()?;
    Ok(ReasonRule {
        predicate: parse_reason_predicate(
            object
                .get("predicate")
                .ok_or_else(|| ExportError::InvalidShape("reason_rule.predicate".into()))?,
        )?,
        signal: match object.get("key_event").unwrap_or(&JsonValue::Null) {
            JsonValue::Null => None,
            value => Some(parse_reason_key_event(value)?),
        },
        narrative: parse_reason_narrative(
            object
                .get("narrative")
                .ok_or_else(|| ExportError::InvalidShape("reason_rule.narrative".into()))?,
        )?,
        dedupe: object
            .get("dedupe")
            .ok_or_else(|| ExportError::InvalidShape("reason_rule.dedupe".into()))?
            .as_bool()?,
        module: match object.get("module").unwrap_or(&JsonValue::Null) {
            JsonValue::Null => None,
            JsonValue::String(value) => Some(value.clone()),
            _ => return Err(ExportError::InvalidShape("reason_rule.module".into())),
        },
        phase: match object.get("phase").unwrap_or(&JsonValue::Null) {
            JsonValue::Null => None,
            JsonValue::String(value) => Some(value.clone()),
            _ => return Err(ExportError::InvalidShape("reason_rule.phase".into())),
        },
    })
}

fn parse_payload_byte_matches(value: &JsonValue) -> Result<Vec<PayloadByteMatch>, ExportError> {
    value
        .as_array()?
        .iter()
        .map(|item| {
            let object = item.as_object()?;
            Ok(PayloadByteMatch {
                offset: object
                    .get("offset")
                    .ok_or_else(|| {
                        ExportError::InvalidShape("reason_predicate.byte_matches.offset".into())
                    })?
                    .as_i64()? as u16,
                mask: object
                    .get("mask")
                    .ok_or_else(|| {
                        ExportError::InvalidShape("reason_predicate.byte_matches.mask".into())
                    })?
                    .as_i64()? as u8,
                value: object
                    .get("value")
                    .ok_or_else(|| {
                        ExportError::InvalidShape("reason_predicate.byte_matches.value".into())
                    })?
                    .as_i64()? as u8,
            })
        })
        .collect()
}

fn parse_payload_byte_sequence_matches(
    value: &JsonValue,
) -> Result<Vec<PayloadByteSequenceMatch>, ExportError> {
    value
        .as_array()?
        .iter()
        .map(|item| {
            let object = item.as_object()?;
            Ok(PayloadByteSequenceMatch {
                offset: object
                    .get("offset")
                    .ok_or_else(|| {
                        ExportError::InvalidShape("reason_predicate.byte_sequences.offset".into())
                    })?
                    .as_i64()? as u16,
                bytes: object
                    .get("bytes")
                    .ok_or_else(|| {
                        ExportError::InvalidShape("reason_predicate.byte_sequences.bytes".into())
                    })?
                    .as_array()?
                    .iter()
                    .map(|value| Ok(value.as_i64()? as u8))
                    .collect::<Result<Vec<_>, _>>()?,
            })
        })
        .collect()
}

fn parse_reason_predicate(value: &JsonValue) -> Result<ReasonPredicate, ExportError> {
    match value {
        JsonValue::String(kind) => match kind.as_str() {
            "process_bound" => Ok(ReasonPredicate::ProcessBound),
            "socket_state_observed" => Ok(ReasonPredicate::SocketStateObserved {
                local_port: None,
                remote_port: None,
                min_new_state: None,
            }),
            "route_resolved" => Ok(ReasonPredicate::RouteResolved),
            other => Err(ExportError::InvalidValue(format!(
                "unknown reason predicate '{other}'"
            ))),
        },
        JsonValue::Object(object) => match object
            .get("kind")
            .ok_or_else(|| ExportError::InvalidShape("reason_predicate.kind".into()))?
            .as_str()?
        {
            "socket_state_observed" => Ok(ReasonPredicate::SocketStateObserved {
                local_port: parse_optional_u16(
                    object.get("local_port").unwrap_or(&JsonValue::Null),
                )?,
                remote_port: parse_optional_u16(
                    object.get("remote_port").unwrap_or(&JsonValue::Null),
                )?,
                min_new_state: parse_optional_u8(
                    object.get("min_new_state").unwrap_or(&JsonValue::Null),
                )?,
            }),
            "packet_observed" => {
                let fields = parse_reason_transport_predicate_fields(object, true)?;
                let qualifiers = parse_packet_header_qualifiers(object)?;
                Ok(ReasonPredicate::PacketObserved {
                    l4_proto: fields.l4_proto.ok_or_else(|| {
                        ExportError::InvalidShape("reason_predicate.l4_proto".into())
                    })?,
                    dir: fields.scope.dir,
                    local_port: fields.scope.local_port,
                    remote_port: fields.scope.remote_port,
                    first_byte_mask: qualifiers.first_byte_mask,
                    first_byte_value: qualifiers.first_byte_value,
                    prefix4: qualifiers.prefix4,
                    byte4_mask: qualifiers.byte4_mask,
                    byte4_value: qualifiers.byte4_value,
                    byte13_mask: qualifiers.byte13_mask,
                    byte13_value: qualifiers.byte13_value,
                    byte_matches: fields.byte_matches,
                    byte_sequences: fields.byte_sequences,
                })
            }
            "datagram_observed" => {
                let fields = parse_reason_transport_predicate_fields(object, true)?;
                let qualifiers = parse_datagram_header_qualifiers(object)?;
                Ok(ReasonPredicate::DatagramObserved {
                    l4_proto: fields.l4_proto.ok_or_else(|| {
                        ExportError::InvalidShape("reason_predicate.l4_proto".into())
                    })?,
                    dir: fields.scope.dir,
                    local_port: fields.scope.local_port,
                    remote_port: fields.scope.remote_port,
                    min_len: qualifiers.min_len,
                    first_byte_mask: qualifiers.first_byte_mask,
                    first_byte_value: qualifiers.first_byte_value,
                    prefix2: qualifiers.prefix2,
                    prefix4: qualifiers.prefix4,
                    byte13_mask: qualifiers.byte13_mask,
                    byte13_value: qualifiers.byte13_value,
                    byte_matches: fields.byte_matches,
                    byte_sequences: fields.byte_sequences,
                })
            }
            "quic_packet_observed" => {
                let scope = parse_reason_predicate_scope(object)?;
                let qualifiers = parse_quic_packet_header_qualifiers(object)?;
                Ok(ReasonPredicate::QuicPacketObserved {
                    dir: scope.dir,
                    local_port: scope.local_port,
                    remote_port: scope.remote_port,
                    min_len: qualifiers.min_len,
                    long_header: qualifiers.long_header,
                    packet_type: qualifiers.packet_type,
                })
            }
            "quic_frame_observed" => {
                let fields = parse_reason_transport_predicate_fields(object, false)?;
                let qualifiers = parse_quic_frame_header_qualifiers(object)?;
                Ok(ReasonPredicate::QuicFrameObserved {
                    dir: fields.scope.dir,
                    local_port: fields.scope.local_port,
                    remote_port: fields.scope.remote_port,
                    packet_type: qualifiers.packet_type,
                    frame_type: qualifiers.frame_type,
                    byte_matches: fields.byte_matches,
                    byte_sequences: fields.byte_sequences,
                })
            }
            "all" => Ok(ReasonPredicate::All(
                object
                    .get("items")
                    .ok_or_else(|| ExportError::InvalidShape("reason_predicate.items".into()))?
                    .as_array()?
                    .iter()
                    .map(parse_reason_predicate)
                    .collect::<Result<Vec<_>, _>>()?,
            )),
            "any" => Ok(ReasonPredicate::Any(
                object
                    .get("items")
                    .ok_or_else(|| ExportError::InvalidShape("reason_predicate.items".into()))?
                    .as_array()?
                    .iter()
                    .map(parse_reason_predicate)
                    .collect::<Result<Vec<_>, _>>()?,
            )),
            other => Err(ExportError::InvalidValue(format!(
                "unknown reason predicate '{other}'"
            ))),
        },
        _ => Err(ExportError::InvalidShape("reason_predicate".into())),
    }
}

fn parse_reason_key_event(value: &JsonValue) -> Result<ReasonKeyEvent, ExportError> {
    let id = value.as_str()?;
    SignalKind::from_id(id)
        .ok_or_else(|| ExportError::InvalidValue(format!("unknown reason key event '{id}'")))
}

fn parse_reason_narrative(value: &JsonValue) -> Result<ReasonNarrative, ExportError> {
    parse_narrative_template(value)
}

fn parse_narrative_template(value: &JsonValue) -> Result<NarrativeTemplate, ExportError> {
    match value {
        JsonValue::String(kind) => match kind.as_str() {
            "none" => Ok(NarrativeTemplate::None),
            "process_bound" => Ok(NarrativeTemplate::ProcessBound),
            "packet_observed" => Ok(NarrativeTemplate::PacketObserved),
            "transport_payload_sent" => Ok(NarrativeTemplate::TransportPayloadSent),
            "transport_payload_received" => Ok(NarrativeTemplate::TransportPayloadReceived),
            "tcp_state_transition" => Ok(NarrativeTemplate::TcpStateTransition),
            "route_changed" => Ok(NarrativeTemplate::RouteChanged),
            "udp_datagram_observed" => Ok(NarrativeTemplate::UdpDatagramObserved),
            "udp_datagram_sent" => Ok(NarrativeTemplate::UdpDatagramSent),
            "udp_datagram_received" => Ok(NarrativeTemplate::UdpDatagramReceived),
            other => Err(ExportError::InvalidValue(format!(
                "unknown narrative template '{other}'"
            ))),
        },
        JsonValue::Object(object) => {
            let kind = object
                .get("kind")
                .ok_or_else(|| ExportError::InvalidShape("narrative.kind".into()))?
                .as_str()?;
            match kind {
                "static" => {
                    let text = object
                        .get("text")
                        .ok_or_else(|| ExportError::InvalidShape("narrative.text".into()))?
                        .as_str()?;
                    Ok(NarrativeTemplate::Static(Box::leak(
                        text.to_string().into_boxed_str(),
                    )))
                }
                other => Err(ExportError::InvalidValue(format!(
                    "unknown narrative template '{other}'"
                ))),
            }
        }
        _ => Err(ExportError::InvalidShape("narrative".into())),
    }
}

pub(crate) fn parse_reason(value: &JsonValue) -> Result<ReasonChain, ExportError> {
    let object = value.as_object()?;
    let l1 = object
        .get("l1")
        .ok_or_else(|| ExportError::InvalidShape("reason.l1".into()))?
        .as_object()?;
    let l3 = object
        .get("l3")
        .ok_or_else(|| ExportError::InvalidShape("reason.l3".into()))?
        .as_object()?;

    Ok(ReasonChain {
        id: ReasonId(
            object
                .get("id")
                .ok_or_else(|| ExportError::InvalidShape("reason.id".into()))?
                .as_i64()? as u64,
        ),
        flow: FlowId(
            object
                .get("flow")
                .ok_or_else(|| ExportError::InvalidShape("reason.flow".into()))?
                .as_i64()? as u64,
        ),
        l0_facts: parse_fact_ids(
            object
                .get("l0_facts")
                .ok_or_else(|| ExportError::InvalidShape("reason.l0_facts".into()))?,
        )?,
        l1: ReasonL1 {
            tcp_state_timeline: parse_fact_ids(l1.get("tcp_state_timeline").ok_or_else(|| {
                ExportError::InvalidShape("reason.l1.tcp_state_timeline".into())
            })?)?,
            path_segments: parse_fact_ids(
                l1.get("path_segments")
                    .ok_or_else(|| ExportError::InvalidShape("reason.l1.path_segments".into()))?,
            )?,
            key_events: l1
                .get("key_events")
                .ok_or_else(|| ExportError::InvalidShape("reason.l1.key_events".into()))?
                .as_array()?
                .iter()
                .map(parse_key_event)
                .collect::<Result<Vec<_>, _>>()?,
        },
        l3: ReasonL3 {
            narrative: l3
                .get("narrative")
                .ok_or_else(|| ExportError::InvalidShape("reason.l3.narrative".into()))?
                .as_array()?
                .iter()
                .map(|item| {
                    let object = item.as_object()?;
                    Ok(NarrLine {
                        at: FactId(
                            object
                                .get("at")
                                .ok_or_else(|| {
                                    ExportError::InvalidShape("reason.l3.narrative.at".into())
                                })?
                                .as_i64()? as u64,
                        ),
                        text: object
                            .get("text")
                            .ok_or_else(|| {
                                ExportError::InvalidShape("reason.l3.narrative.text".into())
                            })?
                            .as_str()?
                            .to_string(),
                    })
                })
                .collect::<Result<Vec<_>, _>>()?,
        },
    })
}

fn parse_key_event(value: &JsonValue) -> Result<KeyEvent, ExportError> {
    let object = value.as_object()?;
    let kind = object
        .get("kind")
        .ok_or_else(|| ExportError::InvalidShape("reason.key_event.kind".into()))?
        .as_str()?;

    Ok(KeyEvent {
        at: FactId(
            object
                .get("at")
                .ok_or_else(|| ExportError::InvalidShape("reason.key_event.at".into()))?
                .as_i64()? as u64,
        ),
        kind: match kind {
            "syn_seen" => KeyEventKind::SynSeen,
            "packet_observed" => KeyEventKind::PacketObserved,
            "udp_datagram_seen" => KeyEventKind::UdpDatagramSeen,
            "process_identified" => KeyEventKind::ProcessIdentified,
            "retrans_suspected" => KeyEventKind::RetransSuspected,
            "route_changed" => KeyEventKind::RouteChanged,
            "fin_or_rst" => KeyEventKind::FinOrRst,
            "state_change" => KeyEventKind::StateChange {
                old: object
                    .get("old")
                    .ok_or_else(|| ExportError::InvalidShape("reason.key_event.old".into()))?
                    .as_i64()? as u8,
                new: object
                    .get("new")
                    .ok_or_else(|| ExportError::InvalidShape("reason.key_event.new".into()))?
                    .as_i64()? as u8,
            },
            other => {
                return Err(ExportError::InvalidValue(format!(
                    "unknown key event kind '{other}'"
                )));
            }
        },
    })
}

fn parse_reason_predicate_scope(
    object: &BTreeMap<String, JsonValue>,
) -> Result<ObservationScope, ExportError> {
    Ok(ObservationScope {
        dir: parse_optional_packet_dir(object.get("dir").unwrap_or(&JsonValue::Null))?,
        local_port: parse_optional_u16(object.get("local_port").unwrap_or(&JsonValue::Null))?,
        remote_port: parse_optional_u16(object.get("remote_port").unwrap_or(&JsonValue::Null))?,
    })
}

fn parse_reason_predicate_payload_matchers(
    object: &BTreeMap<String, JsonValue>,
) -> Result<(Vec<PayloadByteMatch>, Vec<PayloadByteSequenceMatch>), ExportError> {
    let byte_matches = match object.get("byte_matches").unwrap_or(&JsonValue::Null) {
        JsonValue::Null => Vec::new(),
        value => parse_payload_byte_matches(value)?,
    };
    let byte_sequences = match object.get("byte_sequences").unwrap_or(&JsonValue::Null) {
        JsonValue::Null => Vec::new(),
        value => parse_payload_byte_sequence_matches(value)?,
    };
    Ok((byte_matches, byte_sequences))
}

fn parse_reason_transport_predicate_fields(
    object: &BTreeMap<String, JsonValue>,
    require_l4_proto: bool,
) -> Result<ReasonTransportPredicateFields, ExportError> {
    let (byte_matches, byte_sequences) = parse_reason_predicate_payload_matchers(object)?;
    let l4_proto = parse_optional_u8(object.get("l4_proto").unwrap_or(&JsonValue::Null))?;
    if require_l4_proto && l4_proto.is_none() {
        return Err(ExportError::InvalidShape(
            "reason_predicate.l4_proto".into(),
        ));
    }
    Ok(ReasonTransportPredicateFields {
        scope: parse_reason_predicate_scope(object)?,
        l4_proto,
        byte_matches,
        byte_sequences,
    })
}

fn parse_packet_header_qualifiers(
    object: &BTreeMap<String, JsonValue>,
) -> Result<PacketHeaderQualifiers, ExportError> {
    Ok(PacketHeaderQualifiers {
        first_byte_mask: parse_optional_object_u8(object, "first_byte_mask")?,
        first_byte_value: parse_optional_object_u8(object, "first_byte_value")?,
        prefix4: parse_optional_u32(object.get("prefix4").unwrap_or(&JsonValue::Null))?,
        byte4_mask: parse_optional_object_u8(object, "byte4_mask")?,
        byte4_value: parse_optional_object_u8(object, "byte4_value")?,
        byte13_mask: parse_optional_object_u8(object, "byte13_mask")?,
        byte13_value: parse_optional_object_u8(object, "byte13_value")?,
    })
}

fn parse_datagram_header_qualifiers(
    object: &BTreeMap<String, JsonValue>,
) -> Result<DatagramHeaderQualifiers, ExportError> {
    Ok(DatagramHeaderQualifiers {
        min_len: parse_optional_u32(object.get("min_len").unwrap_or(&JsonValue::Null))?,
        first_byte_mask: parse_optional_object_u8(object, "first_byte_mask")?,
        first_byte_value: parse_optional_object_u8(object, "first_byte_value")?,
        prefix2: parse_optional_u16(object.get("prefix2").unwrap_or(&JsonValue::Null))?,
        prefix4: parse_optional_u32(object.get("prefix4").unwrap_or(&JsonValue::Null))?,
        byte13_mask: parse_optional_object_u8(object, "byte13_mask")?,
        byte13_value: parse_optional_object_u8(object, "byte13_value")?,
    })
}

fn parse_quic_packet_header_qualifiers(
    object: &BTreeMap<String, JsonValue>,
) -> Result<QuicPacketHeaderQualifiers, ExportError> {
    Ok(QuicPacketHeaderQualifiers {
        min_len: parse_optional_u32(object.get("min_len").unwrap_or(&JsonValue::Null))?,
        long_header: match object.get("long_header").unwrap_or(&JsonValue::Null) {
            JsonValue::Null => None,
            JsonValue::Bool(value) => Some(*value),
            _ => {
                return Err(ExportError::InvalidShape(
                    "reason_predicate.long_header".into(),
                ));
            }
        },
        packet_type: parse_optional_object_quic_packet_type(object, "packet_type")?,
    })
}

fn parse_quic_frame_header_qualifiers(
    object: &BTreeMap<String, JsonValue>,
) -> Result<QuicFrameHeaderQualifiers, ExportError> {
    Ok(QuicFrameHeaderQualifiers {
        packet_type: parse_optional_object_quic_packet_type(object, "packet_type")?,
        frame_type: parse_required_object_quic_frame_type(object, "frame_type")?,
    })
}

fn parse_optional_packet_dir(value: &JsonValue) -> Result<Option<PacketDir>, ExportError> {
    match value {
        JsonValue::Null => Ok(None),
        JsonValue::String(value) => PacketDir::from_str(value)
            .map(Some)
            .ok_or_else(|| ExportError::InvalidValue(format!("unknown packet dir '{value}'"))),
        _ => Err(ExportError::InvalidShape("reason_predicate.dir".into())),
    }
}

pub(crate) fn parse_quic_packet_type(value: &str) -> Result<QuicPacketType, ExportError> {
    match value {
        "initial" => Ok(QuicPacketType::Initial),
        "0rtt" => Ok(QuicPacketType::ZeroRtt),
        "handshake" => Ok(QuicPacketType::Handshake),
        "retry" => Ok(QuicPacketType::Retry),
        other => Err(ExportError::InvalidValue(format!(
            "unknown quic packet type '{other}'"
        ))),
    }
}

pub(crate) fn parse_quic_frame_type(value: &str) -> Result<QuicFrameType, ExportError> {
    match value {
        "crypto" => Ok(QuicFrameType::Crypto),
        "ack" => Ok(QuicFrameType::Ack),
        "stream" => Ok(QuicFrameType::Stream),
        "datagram" => Ok(QuicFrameType::Datagram),
        "connection_close" => Ok(QuicFrameType::ConnectionClose),
        other => Err(ExportError::InvalidValue(format!(
            "unknown quic frame type '{other}'"
        ))),
    }
}

fn parse_optional_object_u8(
    object: &BTreeMap<String, JsonValue>,
    key: &str,
) -> Result<Option<u8>, ExportError> {
    parse_optional_u8(object.get(key).unwrap_or(&JsonValue::Null))
}

fn parse_optional_object_quic_packet_type(
    object: &BTreeMap<String, JsonValue>,
    key: &str,
) -> Result<Option<QuicPacketType>, ExportError> {
    match object.get(key).unwrap_or(&JsonValue::Null) {
        JsonValue::Null => Ok(None),
        JsonValue::String(value) => parse_quic_packet_type(value).map(Some),
        _ => Err(ExportError::InvalidShape(format!("reason_predicate.{key}"))),
    }
}

fn parse_required_object_quic_frame_type(
    object: &BTreeMap<String, JsonValue>,
    key: &str,
) -> Result<QuicFrameType, ExportError> {
    let value = object
        .get(key)
        .ok_or_else(|| ExportError::InvalidShape(format!("reason_predicate.{key}")))?;
    parse_quic_frame_type(value.as_str()?)
}
