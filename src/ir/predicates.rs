use std::collections::BTreeSet;

use crate::ledger::FactKindTag;

use super::{
    FlowPredicate, ObservationScope, PayloadByteMatch, PayloadByteSequenceMatch,
    PayloadMatcherSetRef, QuicFrameType, QuicPacketType,
};

impl FlowPredicate {
    pub fn socket_state_observed(
        local_port: Option<u16>,
        remote_port: Option<u16>,
        min_new_state: Option<u8>,
    ) -> Self {
        Self::SocketStateObserved {
            local_port,
            remote_port,
            min_new_state,
        }
    }

    // The explicit fields mirror the stable packet predicate contract.
    #[allow(clippy::too_many_arguments)]
    pub fn packet_observed(
        l4_proto: u8,
        scope: ObservationScope,
        first_byte_mask: Option<u8>,
        first_byte_value: Option<u8>,
        prefix4: Option<u32>,
        byte4_mask: Option<u8>,
        byte4_value: Option<u8>,
        byte13_mask: Option<u8>,
        byte13_value: Option<u8>,
        byte_matches: Vec<PayloadByteMatch>,
        byte_sequences: Vec<PayloadByteSequenceMatch>,
    ) -> Self {
        Self::PacketObserved {
            l4_proto,
            dir: scope.dir,
            local_port: scope.local_port,
            remote_port: scope.remote_port,
            first_byte_mask,
            first_byte_value,
            prefix4,
            byte4_mask,
            byte4_value,
            byte13_mask,
            byte13_value,
            byte_matches,
            byte_sequences,
        }
    }

    // The explicit fields mirror the stable datagram predicate contract.
    #[allow(clippy::too_many_arguments)]
    pub fn datagram_observed(
        l4_proto: u8,
        scope: ObservationScope,
        min_len: Option<u32>,
        first_byte_mask: Option<u8>,
        first_byte_value: Option<u8>,
        prefix2: Option<u16>,
        prefix4: Option<u32>,
        byte13_mask: Option<u8>,
        byte13_value: Option<u8>,
        byte_matches: Vec<PayloadByteMatch>,
        byte_sequences: Vec<PayloadByteSequenceMatch>,
    ) -> Self {
        Self::DatagramObserved {
            l4_proto,
            dir: scope.dir,
            local_port: scope.local_port,
            remote_port: scope.remote_port,
            min_len,
            first_byte_mask,
            first_byte_value,
            prefix2,
            prefix4,
            byte13_mask,
            byte13_value,
            byte_matches,
            byte_sequences,
        }
    }

    pub fn quic_packet_observed(
        scope: ObservationScope,
        min_len: Option<u32>,
        long_header: Option<bool>,
        packet_type: Option<QuicPacketType>,
    ) -> Self {
        Self::QuicPacketObserved {
            dir: scope.dir,
            local_port: scope.local_port,
            remote_port: scope.remote_port,
            min_len,
            long_header,
            packet_type,
        }
    }

    pub fn quic_frame_observed(
        scope: ObservationScope,
        packet_type: Option<QuicPacketType>,
        frame_type: QuicFrameType,
        byte_matches: Vec<PayloadByteMatch>,
        byte_sequences: Vec<PayloadByteSequenceMatch>,
    ) -> Self {
        Self::QuicFrameObserved {
            dir: scope.dir,
            local_port: scope.local_port,
            remote_port: scope.remote_port,
            packet_type,
            frame_type,
            byte_matches,
            byte_sequences,
        }
    }

    pub fn observation_scope(&self) -> Option<ObservationScope> {
        match self {
            FlowPredicate::PacketObserved {
                dir,
                local_port,
                remote_port,
                ..
            }
            | FlowPredicate::DatagramObserved {
                dir,
                local_port,
                remote_port,
                ..
            }
            | FlowPredicate::QuicPacketObserved {
                dir,
                local_port,
                remote_port,
                ..
            }
            | FlowPredicate::QuicFrameObserved {
                dir,
                local_port,
                remote_port,
                ..
            } => Some(ObservationScope {
                dir: *dir,
                local_port: *local_port,
                remote_port: *remote_port,
            }),
            _ => None,
        }
    }

    pub fn payload_matchers(&self) -> Option<PayloadMatcherSetRef<'_>> {
        match self {
            FlowPredicate::PacketObserved {
                byte_matches,
                byte_sequences,
                ..
            }
            | FlowPredicate::DatagramObserved {
                byte_matches,
                byte_sequences,
                ..
            }
            | FlowPredicate::QuicFrameObserved {
                byte_matches,
                byte_sequences,
                ..
            } => Some(PayloadMatcherSetRef::new(byte_matches, byte_sequences)),
            _ => None,
        }
    }

    pub fn required_payload_offsets(&self) -> Vec<u16> {
        let mut offsets = BTreeSet::new();
        match self {
            FlowPredicate::PacketObserved {
                byte4_mask,
                byte13_mask,
                ..
            } => {
                if byte4_mask.is_some() {
                    offsets.insert(4);
                }
                if byte13_mask.is_some() {
                    offsets.insert(13);
                }
                if let Some(matchers) = self.payload_matchers() {
                    offsets.extend(matchers.required_offsets());
                }
            }
            FlowPredicate::DatagramObserved { byte13_mask, .. } => {
                if byte13_mask.is_some() {
                    offsets.insert(13);
                }
                if let Some(matchers) = self.payload_matchers() {
                    offsets.extend(matchers.required_offsets());
                }
            }
            FlowPredicate::QuicFrameObserved { .. } => {
                if let Some(matchers) = self.payload_matchers() {
                    offsets.extend(matchers.required_offsets());
                }
            }
            FlowPredicate::All(predicates) | FlowPredicate::Any(predicates) => {
                for predicate in predicates {
                    offsets.extend(predicate.required_payload_offsets());
                }
            }
            _ => {}
        }
        offsets.into_iter().collect()
    }

    pub fn required_fact_kinds(&self) -> Vec<FactKindTag> {
        match self {
            FlowPredicate::ProcessBound => vec![FactKindTag::SockLineage],
            FlowPredicate::SocketStateObserved { .. } => vec![FactKindTag::TcpState],
            FlowPredicate::PacketObserved { .. } => vec![FactKindTag::PacketMeta],
            FlowPredicate::DatagramObserved { .. } => vec![FactKindTag::PacketMeta],
            FlowPredicate::RouteResolved => vec![FactKindTag::RouteDecision],
            FlowPredicate::QuicPacketObserved { .. } => vec![FactKindTag::PacketMeta],
            FlowPredicate::QuicFrameObserved { .. } => vec![FactKindTag::QuicMeta],
            FlowPredicate::All(predicates) | FlowPredicate::Any(predicates) => predicates
                .iter()
                .flat_map(|predicate| predicate.required_fact_kinds())
                .collect(),
        }
    }
}

pub fn flow_predicate_satisfied_in_flow(
    predicate: &FlowPredicate,
    flow: &crate::flow::FlowSnapshot,
    facts: &[crate::ledger::FactEnvelope],
) -> bool {
    facts
        .iter()
        .any(|fact| super::matching::matches_flow_predicate(predicate, flow, fact, facts))
}
