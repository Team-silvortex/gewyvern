use super::JsonValue;
use super::{
    DatagramHeaderQualifiers, PacketHeaderQualifiers, QuicFrameHeaderQualifiers,
    QuicPacketHeaderQualifiers, fact_id_array, quic_frame_type_id, quic_packet_type_id,
};
use crate::ir::{NarrativeTemplate, ObservationScope, PayloadMatcherSetRef};
use crate::ledger::{QuicFrameType, QuicPacketType};
use crate::reason::{
    KeyEventKind, ReasonChain, ReasonKeyEvent, ReasonNarrative, ReasonPredicate, ReasonProfile,
    ReasonRule,
};
use std::collections::BTreeMap;

pub(crate) fn reason_json(reason: &ReasonChain) -> JsonValue {
    JsonValue::Object(BTreeMap::from([
        ("id".into(), JsonValue::Number(reason.id.0 as i64)),
        ("flow".into(), JsonValue::Number(reason.flow.0 as i64)),
        ("l0_facts".into(), fact_id_array(&reason.l0_facts)),
        (
            "l1".into(),
            JsonValue::Object(BTreeMap::from([
                (
                    "tcp_state_timeline".into(),
                    fact_id_array(&reason.l1.tcp_state_timeline),
                ),
                (
                    "path_segments".into(),
                    fact_id_array(&reason.l1.path_segments),
                ),
                (
                    "key_events".into(),
                    JsonValue::Array(
                        reason
                            .l1
                            .key_events
                            .iter()
                            .map(|event| {
                                let mut fields = BTreeMap::from([
                                    ("at".into(), JsonValue::Number(event.at.0 as i64)),
                                    (
                                        "kind".into(),
                                        JsonValue::String(
                                            match &event.kind {
                                                KeyEventKind::SynSeen => "syn_seen",
                                                KeyEventKind::PacketObserved => "packet_observed",
                                                KeyEventKind::UdpDatagramSeen => {
                                                    "udp_datagram_seen"
                                                }
                                                KeyEventKind::ProcessIdentified => {
                                                    "process_identified"
                                                }
                                                KeyEventKind::RetransSuspected => {
                                                    "retrans_suspected"
                                                }
                                                KeyEventKind::RouteChanged => "route_changed",
                                                KeyEventKind::FinOrRst => "fin_or_rst",
                                                KeyEventKind::StateChange { .. } => "state_change",
                                            }
                                            .into(),
                                        ),
                                    ),
                                ]);
                                if let KeyEventKind::StateChange { old, new } = event.kind {
                                    fields.insert("old".into(), JsonValue::Number(old as i64));
                                    fields.insert("new".into(), JsonValue::Number(new as i64));
                                }
                                JsonValue::Object(fields)
                            })
                            .collect(),
                    ),
                ),
            ])),
        ),
        (
            "l3".into(),
            JsonValue::Object(BTreeMap::from([(
                "narrative".into(),
                JsonValue::Array(
                    reason
                        .l3
                        .narrative
                        .iter()
                        .map(|line| {
                            JsonValue::Object(BTreeMap::from([
                                ("at".into(), JsonValue::Number(line.at.0 as i64)),
                                ("text".into(), JsonValue::String(line.text.clone())),
                            ]))
                        })
                        .collect(),
                ),
            )])),
        ),
    ]))
}

pub(crate) fn reason_profile_json(profile: &ReasonProfile) -> JsonValue {
    match profile {
        ReasonProfile::HandshakeL1 | ReasonProfile::UdpDatagramL1 => {
            JsonValue::String(profile.id().into())
        }
        ReasonProfile::Declarative(model) => JsonValue::Object(BTreeMap::from([
            ("id".into(), JsonValue::String(model.id.into())),
            ("kind".into(), JsonValue::String("declarative".into())),
            (
                "rules".into(),
                JsonValue::Array(model.rules.iter().map(reason_rule_json).collect()),
            ),
        ])),
    }
}

fn reason_rule_json(rule: &ReasonRule) -> JsonValue {
    JsonValue::Object(BTreeMap::from([
        ("predicate".into(), reason_predicate_json(&rule.predicate)),
        (
            "key_event".into(),
            rule.signal.as_ref().map_or(JsonValue::Null, |event| {
                JsonValue::String(reason_key_event_id(event).into())
            }),
        ),
        ("narrative".into(), reason_narrative_json(&rule.narrative)),
        ("dedupe".into(), JsonValue::Bool(rule.dedupe)),
        (
            "module".into(),
            rule.module
                .as_ref()
                .map_or(JsonValue::Null, |module| JsonValue::String(module.clone())),
        ),
        (
            "phase".into(),
            rule.phase
                .as_ref()
                .map_or(JsonValue::Null, |phase| JsonValue::String(phase.clone())),
        ),
    ]))
}

fn reason_predicate_json(predicate: &ReasonPredicate) -> JsonValue {
    match predicate {
        ReasonPredicate::ProcessBound => JsonValue::String("process_bound".into()),
        ReasonPredicate::SocketStateObserved {
            local_port,
            remote_port,
            min_new_state,
        } => match (local_port, remote_port, min_new_state) {
            (None, None, None) => JsonValue::String("socket_state_observed".into()),
            _ => {
                let mut object = BTreeMap::from([(
                    "kind".into(),
                    JsonValue::String("socket_state_observed".into()),
                )]);
                insert_optional_ports(&mut object, *local_port, *remote_port);
                if let Some(min_new_state) = min_new_state {
                    object.insert(
                        "min_new_state".into(),
                        JsonValue::Number(*min_new_state as i64),
                    );
                }
                JsonValue::Object(object)
            }
        },
        ReasonPredicate::RouteResolved => JsonValue::String("route_resolved".into()),
        ReasonPredicate::QuicPacketObserved {
            dir,
            local_port,
            remote_port,
            min_len,
            long_header,
            packet_type,
        } => {
            let mut object = reason_transport_object(
                "quic_packet_observed",
                ObservationScope {
                    dir: *dir,
                    local_port: *local_port,
                    remote_port: *remote_port,
                },
                None,
            );
            insert_quic_packet_header_qualifiers(
                &mut object,
                QuicPacketHeaderQualifiers {
                    min_len: *min_len,
                    long_header: *long_header,
                    packet_type: *packet_type,
                },
            );
            JsonValue::Object(object)
        }
        ReasonPredicate::QuicFrameObserved {
            dir,
            local_port,
            remote_port,
            packet_type,
            frame_type,
            byte_matches,
            byte_sequences,
        } => {
            let mut object = reason_transport_object_with_matchers(
                "quic_frame_observed",
                ObservationScope {
                    dir: *dir,
                    local_port: *local_port,
                    remote_port: *remote_port,
                },
                None,
                PayloadMatcherSetRef::new(byte_matches, byte_sequences),
            );
            insert_quic_frame_header_qualifiers(
                &mut object,
                QuicFrameHeaderQualifiers {
                    packet_type: *packet_type,
                    frame_type: *frame_type,
                },
            );
            JsonValue::Object(object)
        }
        ReasonPredicate::PacketObserved {
            l4_proto,
            dir,
            local_port,
            remote_port,
            first_byte_mask,
            first_byte_value,
            prefix4,
            byte4_mask,
            byte4_value,
            byte13_mask,
            byte13_value,
            byte_matches,
            byte_sequences,
        } => {
            let mut object = reason_transport_object_with_matchers(
                "packet_observed",
                ObservationScope {
                    dir: *dir,
                    local_port: *local_port,
                    remote_port: *remote_port,
                },
                Some(*l4_proto),
                PayloadMatcherSetRef::new(byte_matches, byte_sequences),
            );
            insert_packet_header_qualifiers(
                &mut object,
                PacketHeaderQualifiers {
                    first_byte_mask: *first_byte_mask,
                    first_byte_value: *first_byte_value,
                    prefix4: *prefix4,
                    byte4_mask: *byte4_mask,
                    byte4_value: *byte4_value,
                    byte13_mask: *byte13_mask,
                    byte13_value: *byte13_value,
                },
            );
            JsonValue::Object(object)
        }
        ReasonPredicate::DatagramObserved {
            l4_proto,
            dir,
            local_port,
            remote_port,
            min_len,
            first_byte_mask,
            first_byte_value,
            prefix2,
            prefix4,
            byte13_mask,
            byte13_value,
            byte_matches,
            byte_sequences,
        } => {
            let mut object = reason_transport_object_with_matchers(
                "datagram_observed",
                ObservationScope {
                    dir: *dir,
                    local_port: *local_port,
                    remote_port: *remote_port,
                },
                Some(*l4_proto),
                PayloadMatcherSetRef::new(byte_matches, byte_sequences),
            );
            insert_datagram_header_qualifiers(
                &mut object,
                DatagramHeaderQualifiers {
                    min_len: *min_len,
                    first_byte_mask: *first_byte_mask,
                    first_byte_value: *first_byte_value,
                    prefix2: *prefix2,
                    prefix4: *prefix4,
                    byte13_mask: *byte13_mask,
                    byte13_value: *byte13_value,
                },
            );
            JsonValue::Object(object)
        }
        ReasonPredicate::All(items) => JsonValue::Object(BTreeMap::from([
            ("kind".into(), JsonValue::String("all".into())),
            (
                "items".into(),
                JsonValue::Array(items.iter().map(reason_predicate_json).collect()),
            ),
        ])),
        ReasonPredicate::Any(items) => JsonValue::Object(BTreeMap::from([
            ("kind".into(), JsonValue::String("any".into())),
            (
                "items".into(),
                JsonValue::Array(items.iter().map(reason_predicate_json).collect()),
            ),
        ])),
    }
}

fn reason_key_event_id(event: &ReasonKeyEvent) -> &'static str {
    event.id()
}

fn payload_byte_match_json(value: &crate::ir::PayloadByteMatch) -> JsonValue {
    JsonValue::Object(BTreeMap::from([
        ("offset".into(), JsonValue::Number(value.offset as i64)),
        ("mask".into(), JsonValue::Number(value.mask as i64)),
        ("value".into(), JsonValue::Number(value.value as i64)),
    ]))
}

fn payload_byte_sequence_match_json(value: &crate::ir::PayloadByteSequenceMatch) -> JsonValue {
    JsonValue::Object(BTreeMap::from([
        ("offset".into(), JsonValue::Number(value.offset as i64)),
        (
            "bytes".into(),
            JsonValue::Array(
                value
                    .bytes
                    .iter()
                    .map(|byte| JsonValue::Number(*byte as i64))
                    .collect(),
            ),
        ),
    ]))
}

fn reason_transport_object(
    kind: &str,
    scope: ObservationScope,
    l4_proto: Option<u8>,
) -> BTreeMap<String, JsonValue> {
    let mut object = BTreeMap::from([("kind".into(), JsonValue::String(kind.into()))]);
    if let Some(l4_proto) = l4_proto {
        object.insert("l4_proto".into(), JsonValue::Number(l4_proto as i64));
    }
    insert_optional_dir_and_ports(&mut object, scope);
    object
}

fn reason_transport_object_with_matchers(
    kind: &str,
    scope: ObservationScope,
    l4_proto: Option<u8>,
    matchers: PayloadMatcherSetRef<'_>,
) -> BTreeMap<String, JsonValue> {
    let mut object = reason_transport_object(kind, scope, l4_proto);
    insert_payload_matchers(&mut object, matchers);
    object
}

fn insert_packet_header_qualifiers(
    object: &mut BTreeMap<String, JsonValue>,
    qualifiers: PacketHeaderQualifiers,
) {
    insert_optional_u8_field(object, "first_byte_mask", qualifiers.first_byte_mask);
    insert_optional_u8_field(object, "first_byte_value", qualifiers.first_byte_value);
    insert_optional_u32_field(object, "prefix4", qualifiers.prefix4);
    insert_optional_u8_field(object, "byte4_mask", qualifiers.byte4_mask);
    insert_optional_u8_field(object, "byte4_value", qualifiers.byte4_value);
    insert_optional_u8_field(object, "byte13_mask", qualifiers.byte13_mask);
    insert_optional_u8_field(object, "byte13_value", qualifiers.byte13_value);
}

fn insert_datagram_header_qualifiers(
    object: &mut BTreeMap<String, JsonValue>,
    qualifiers: DatagramHeaderQualifiers,
) {
    insert_optional_u32_field(object, "min_len", qualifiers.min_len);
    insert_optional_u8_field(object, "first_byte_mask", qualifiers.first_byte_mask);
    insert_optional_u8_field(object, "first_byte_value", qualifiers.first_byte_value);
    insert_optional_u16_field(object, "prefix2", qualifiers.prefix2);
    insert_optional_u32_field(object, "prefix4", qualifiers.prefix4);
    insert_optional_u8_field(object, "byte13_mask", qualifiers.byte13_mask);
    insert_optional_u8_field(object, "byte13_value", qualifiers.byte13_value);
}

fn insert_quic_packet_header_qualifiers(
    object: &mut BTreeMap<String, JsonValue>,
    qualifiers: QuicPacketHeaderQualifiers,
) {
    insert_optional_u32_field(object, "min_len", qualifiers.min_len);
    insert_optional_bool_field(object, "long_header", qualifiers.long_header);
    insert_optional_quic_packet_type(object, qualifiers.packet_type);
}

fn insert_quic_frame_header_qualifiers(
    object: &mut BTreeMap<String, JsonValue>,
    qualifiers: QuicFrameHeaderQualifiers,
) {
    insert_optional_quic_packet_type(object, qualifiers.packet_type);
    insert_required_quic_frame_type(object, qualifiers.frame_type);
}

fn insert_payload_matchers(
    object: &mut BTreeMap<String, JsonValue>,
    matchers: PayloadMatcherSetRef<'_>,
) {
    if !matchers.byte_matches.is_empty() {
        object.insert(
            "byte_matches".into(),
            JsonValue::Array(
                matchers
                    .byte_matches
                    .iter()
                    .map(payload_byte_match_json)
                    .collect(),
            ),
        );
    }
    if !matchers.byte_sequences.is_empty() {
        object.insert(
            "byte_sequences".into(),
            JsonValue::Array(
                matchers
                    .byte_sequences
                    .iter()
                    .map(payload_byte_sequence_match_json)
                    .collect(),
            ),
        );
    }
}

fn insert_optional_dir_and_ports(
    object: &mut BTreeMap<String, JsonValue>,
    scope: ObservationScope,
) {
    if let Some(dir) = scope.dir {
        object.insert("dir".into(), JsonValue::String(dir.as_flow_str().into()));
    }
    insert_optional_ports(object, scope.local_port, scope.remote_port);
}

fn insert_optional_ports(
    object: &mut BTreeMap<String, JsonValue>,
    local_port: Option<u16>,
    remote_port: Option<u16>,
) {
    if let Some(local_port) = local_port {
        object.insert("local_port".into(), JsonValue::Number(local_port as i64));
    }
    if let Some(remote_port) = remote_port {
        object.insert("remote_port".into(), JsonValue::Number(remote_port as i64));
    }
}

fn insert_optional_quic_packet_type(
    object: &mut BTreeMap<String, JsonValue>,
    packet_type: Option<QuicPacketType>,
) {
    if let Some(packet_type) = packet_type {
        object.insert(
            "packet_type".into(),
            JsonValue::String(quic_packet_type_id(&packet_type).into()),
        );
    }
}

fn insert_required_quic_frame_type(
    object: &mut BTreeMap<String, JsonValue>,
    frame_type: QuicFrameType,
) {
    object.insert(
        "frame_type".into(),
        JsonValue::String(quic_frame_type_id(&frame_type).into()),
    );
}

fn insert_optional_u8_field(
    object: &mut BTreeMap<String, JsonValue>,
    key: &str,
    value: Option<u8>,
) {
    if let Some(value) = value {
        object.insert(key.into(), JsonValue::Number(value as i64));
    }
}

fn insert_optional_u16_field(
    object: &mut BTreeMap<String, JsonValue>,
    key: &str,
    value: Option<u16>,
) {
    if let Some(value) = value {
        object.insert(key.into(), JsonValue::Number(value as i64));
    }
}

fn insert_optional_u32_field(
    object: &mut BTreeMap<String, JsonValue>,
    key: &str,
    value: Option<u32>,
) {
    if let Some(value) = value {
        object.insert(key.into(), JsonValue::Number(value as i64));
    }
}

fn insert_optional_bool_field(
    object: &mut BTreeMap<String, JsonValue>,
    key: &str,
    value: Option<bool>,
) {
    if let Some(value) = value {
        object.insert(key.into(), JsonValue::Bool(value));
    }
}

fn reason_narrative_json(narrative: &ReasonNarrative) -> JsonValue {
    narrative_template_json(narrative)
}

fn narrative_template_json(narrative: &NarrativeTemplate) -> JsonValue {
    match narrative {
        NarrativeTemplate::None => JsonValue::String("none".into()),
        NarrativeTemplate::ProcessBound => JsonValue::String("process_bound".into()),
        NarrativeTemplate::PacketObserved => JsonValue::String("packet_observed".into()),
        NarrativeTemplate::TransportPayloadSent => {
            JsonValue::String("transport_payload_sent".into())
        }
        NarrativeTemplate::TransportPayloadReceived => {
            JsonValue::String("transport_payload_received".into())
        }
        NarrativeTemplate::TcpStateTransition => JsonValue::String("tcp_state_transition".into()),
        NarrativeTemplate::RouteChanged => JsonValue::String("route_changed".into()),
        NarrativeTemplate::UdpDatagramObserved => JsonValue::String("udp_datagram_observed".into()),
        NarrativeTemplate::UdpDatagramSent => JsonValue::String("udp_datagram_sent".into()),
        NarrativeTemplate::UdpDatagramReceived => JsonValue::String("udp_datagram_received".into()),
        NarrativeTemplate::Static(text) => JsonValue::Object(BTreeMap::from([
            ("kind".into(), JsonValue::String("static".into())),
            ("text".into(), JsonValue::String((*text).into())),
        ])),
    }
}
