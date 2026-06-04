use crate::flow::FlowSnapshot;
use crate::ledger::{FactEnvelope, FactKind};

use super::{FlowPredicate, PayloadByteSequenceMatch, PayloadMatcherSetRef, QuicPacketType};

impl<'a> PayloadMatcherSetRef<'a> {
    pub fn new(
        byte_matches: &'a [super::PayloadByteMatch],
        byte_sequences: &'a [PayloadByteSequenceMatch],
    ) -> Self {
        Self {
            byte_matches,
            byte_sequences,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.byte_matches.is_empty() && self.byte_sequences.is_empty()
    }

    pub fn required_offsets(&self) -> Vec<u16> {
        let mut offsets = std::collections::BTreeSet::new();
        for matcher in self.byte_matches {
            offsets.insert(matcher.offset);
        }
        for matcher in self.byte_sequences {
            for offset in matcher.offset..matcher.offset + matcher.bytes.len() as u16 {
                offsets.insert(offset);
            }
        }
        offsets.into_iter().collect()
    }

    pub fn matches_packet(&self, packet: &crate::ledger::PacketMetaFact) -> bool {
        self.byte_matches.iter().all(|matcher| {
            packet_payload_byte_at(packet, matcher.offset)
                .is_some_and(|byte| byte & matcher.mask == matcher.value)
        }) && self
            .byte_sequences
            .iter()
            .all(|matcher| packet_payload_sequence_at(packet, matcher))
    }

    pub fn matches_payload_bytes(
        &self,
        payload_bytes: &std::collections::BTreeMap<u16, u8>,
    ) -> bool {
        self.byte_matches.iter().all(|matcher| {
            payload_bytes
                .get(&matcher.offset)
                .is_some_and(|byte| *byte & matcher.mask == matcher.value)
        }) && self.byte_sequences.iter().all(|matcher| {
            matcher.bytes.iter().enumerate().all(|(idx, expected)| {
                payload_bytes
                    .get(&(matcher.offset + idx as u16))
                    .is_some_and(|byte| byte == expected)
            })
        })
    }
}

pub fn matches_flow_predicate(
    predicate: &FlowPredicate,
    flow: &FlowSnapshot,
    fact: &FactEnvelope,
    facts: &[FactEnvelope],
) -> bool {
    match predicate {
        FlowPredicate::ProcessBound => flow.evidence.lineage_facts.contains(&fact.id),
        FlowPredicate::SocketStateObserved {
            local_port,
            remote_port,
            min_new_state,
        } => {
            if !flow.evidence.tcp_state_facts.contains(&fact.id) {
                return false;
            }
            matches!(
                &fact.kind,
                FactKind::TcpState(state)
                    if local_port.as_ref().is_none_or(|expected| state.sport == *expected)
                        && remote_port.as_ref().is_none_or(|expected| state.dport == *expected)
                        && min_new_state.as_ref().is_none_or(|expected| state.new >= *expected)
            )
        }
        FlowPredicate::PacketObserved {
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
            if !flow.evidence.packet_facts.contains(&fact.id) {
                return false;
            }
            matches!(
                &fact.kind,
                FactKind::PacketMeta(packet)
                    if packet.l4_proto == *l4_proto
                        && dir.as_ref().is_none_or(|expected| packet.dir == *expected)
                        && local_port.as_ref().is_none_or(|expected| packet.local_port == Some(*expected))
                        && remote_port.as_ref().is_none_or(|expected| packet.remote_port == Some(*expected))
                        && match (first_byte_mask, first_byte_value) {
                            (Some(mask), Some(value)) => packet.payload_byte0.is_some_and(|byte| byte & *mask == *value),
                            _ => true,
                        }
                        && prefix4.as_ref().is_none_or(|expected| packet.payload_prefix4 == Some(*expected))
                        && match (byte4_mask, byte4_value) {
                            (Some(mask), Some(value)) => packet.payload_byte4.is_some_and(|byte| byte & *mask == *value),
                            _ => true,
                        }
                        && match (byte13_mask, byte13_value) {
                            (Some(mask), Some(value)) => packet.payload_byte13.is_some_and(|byte| byte & *mask == *value),
                            _ => true,
                        }
                        && PayloadMatcherSetRef::new(byte_matches, byte_sequences).matches_packet(packet)
            )
        }
        FlowPredicate::DatagramObserved {
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
            if !flow.evidence.packet_facts.contains(&fact.id) {
                return false;
            }
            matches!(
                &fact.kind,
                FactKind::PacketMeta(packet)
                    if packet.l4_proto == *l4_proto
                        && dir.as_ref().is_none_or(|expected| packet.dir == *expected)
                        && local_port.as_ref().is_none_or(|expected| packet.local_port == Some(*expected))
                        && remote_port.as_ref().is_none_or(|expected| packet.remote_port == Some(*expected))
                        && min_len.as_ref().is_none_or(|expected| packet.tot_len >= *expected)
                        && match (first_byte_mask, first_byte_value) {
                            (Some(mask), Some(value)) => packet.payload_byte0.is_some_and(|byte| byte & *mask == *value),
                            _ => true,
                        }
                        && prefix2.as_ref().is_none_or(|expected| packet.payload_prefix2 == Some(*expected))
                        && prefix4.as_ref().is_none_or(|expected| packet.payload_prefix4 == Some(*expected))
                        && match (byte13_mask, byte13_value) {
                            (Some(mask), Some(value)) => packet.payload_byte13.is_some_and(|byte| byte & *mask == *value),
                            _ => true,
                        }
                        && PayloadMatcherSetRef::new(byte_matches, byte_sequences).matches_packet(packet)
            )
        }
        FlowPredicate::RouteResolved => flow.evidence.route_facts.contains(&fact.id),
        FlowPredicate::QuicPacketObserved {
            dir,
            local_port,
            remote_port,
            min_len,
            long_header,
            packet_type,
        } => {
            if !flow.evidence.packet_facts.contains(&fact.id) {
                return false;
            }
            matches!(
                &fact.kind,
                FactKind::PacketMeta(packet)
                    if packet.l4_proto == 17
                        && dir.as_ref().is_none_or(|expected| packet.dir == *expected)
                        && local_port.as_ref().is_none_or(|expected| packet.local_port == Some(*expected))
                        && remote_port.as_ref().is_none_or(|expected| packet.remote_port == Some(*expected))
                        && min_len.as_ref().is_none_or(|expected| packet.tot_len >= *expected)
                        && long_header.as_ref().is_none_or(|expected| quic_long_header(packet) == Some(*expected))
                        && packet_type.as_ref().is_none_or(|expected| quic_packet_type(packet) == Some(*expected))
            )
        }
        FlowPredicate::QuicFrameObserved {
            dir,
            local_port,
            remote_port,
            packet_type,
            frame_type,
            byte_matches,
            byte_sequences,
        } => {
            if !flow.evidence.quic_facts.contains(&fact.id) {
                return false;
            }
            matches!(
                &fact.kind,
                FactKind::QuicMeta(quic)
                    if dir.as_ref().is_none_or(|expected| quic.dir == *expected)
                        && local_port.as_ref().is_none_or(|expected| quic.local_port == Some(*expected))
                        && remote_port.as_ref().is_none_or(|expected| quic.remote_port == Some(*expected))
                        && packet_type.as_ref().is_none_or(|expected| quic.packet_type == Some(*expected))
                        && quic.frame_types.contains(frame_type)
                        && PayloadMatcherSetRef::new(byte_matches, byte_sequences).matches_payload_bytes(&quic.payload_bytes)
            )
        }
        FlowPredicate::All(predicates) => {
            predicates.iter().all(|predicate| {
                super::predicates::flow_predicate_satisfied_in_flow(predicate, flow, facts)
            }) && predicates
                .iter()
                .any(|predicate| matches_flow_predicate(predicate, flow, fact, facts))
        }
        FlowPredicate::Any(predicates) => predicates
            .iter()
            .any(|predicate| matches_flow_predicate(predicate, flow, fact, facts)),
    }
}

fn packet_payload_byte_at(packet: &crate::ledger::PacketMetaFact, offset: u16) -> Option<u8> {
    match offset {
        0 => packet.payload_byte0,
        1 => packet.payload_byte1,
        4 => packet.payload_byte4,
        5 => packet.payload_byte5,
        9 => packet.payload_byte9,
        10 => packet.payload_byte10,
        13 => packet.payload_byte13,
        _ => packet.payload_bytes.get(&offset).copied(),
    }
}

fn quic_long_header(packet: &crate::ledger::PacketMetaFact) -> Option<bool> {
    packet.payload_byte0.map(|byte| byte & 0x80 != 0)
}

fn quic_packet_type(packet: &crate::ledger::PacketMetaFact) -> Option<QuicPacketType> {
    let byte = packet.payload_byte0?;
    if byte & 0x80 == 0 {
        return None;
    }
    match byte & 0x30 {
        0x00 => Some(QuicPacketType::Initial),
        0x10 => Some(QuicPacketType::ZeroRtt),
        0x20 => Some(QuicPacketType::Handshake),
        0x30 => Some(QuicPacketType::Retry),
        _ => None,
    }
}

fn packet_payload_sequence_at(
    packet: &crate::ledger::PacketMetaFact,
    matcher: &PayloadByteSequenceMatch,
) -> bool {
    matcher.bytes.iter().enumerate().all(|(index, expected)| {
        packet_payload_byte_at(packet, matcher.offset + index as u16) == Some(*expected)
    })
}
