use crate::flow::FlowSnapshot;
use crate::ledger::{FactEnvelope, PacketDir};
pub use crate::ledger::{QuicFrameType, QuicPacketType};

mod matching;
mod narrative;
mod predicates;
#[cfg(test)]
mod tests;

pub use self::narrative::{phase_kind, render_narrative_template, render_phase_transition_kind};
pub use self::predicates::flow_predicate_satisfied_in_flow;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PayloadByteMatch {
    pub offset: u16,
    pub mask: u8,
    pub value: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PayloadByteSequenceMatch {
    pub offset: u16,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PayloadMatcherSetRef<'a> {
    pub byte_matches: &'a [PayloadByteMatch],
    pub byte_sequences: &'a [PayloadByteSequenceMatch],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObservationScope {
    pub dir: Option<PacketDir>,
    pub local_port: Option<u16>,
    pub remote_port: Option<u16>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FlowPredicate {
    ProcessBound,
    SocketStateObserved {
        local_port: Option<u16>,
        remote_port: Option<u16>,
        min_new_state: Option<u8>,
    },
    PacketObserved {
        l4_proto: u8,
        dir: Option<PacketDir>,
        local_port: Option<u16>,
        remote_port: Option<u16>,
        first_byte_mask: Option<u8>,
        first_byte_value: Option<u8>,
        prefix4: Option<u32>,
        byte4_mask: Option<u8>,
        byte4_value: Option<u8>,
        byte13_mask: Option<u8>,
        byte13_value: Option<u8>,
        byte_matches: Vec<PayloadByteMatch>,
        byte_sequences: Vec<PayloadByteSequenceMatch>,
    },
    DatagramObserved {
        l4_proto: u8,
        dir: Option<PacketDir>,
        local_port: Option<u16>,
        remote_port: Option<u16>,
        min_len: Option<u32>,
        first_byte_mask: Option<u8>,
        first_byte_value: Option<u8>,
        prefix2: Option<u16>,
        prefix4: Option<u32>,
        byte13_mask: Option<u8>,
        byte13_value: Option<u8>,
        byte_matches: Vec<PayloadByteMatch>,
        byte_sequences: Vec<PayloadByteSequenceMatch>,
    },
    QuicPacketObserved {
        dir: Option<PacketDir>,
        local_port: Option<u16>,
        remote_port: Option<u16>,
        min_len: Option<u32>,
        long_header: Option<bool>,
        packet_type: Option<QuicPacketType>,
    },
    QuicFrameObserved {
        dir: Option<PacketDir>,
        local_port: Option<u16>,
        remote_port: Option<u16>,
        packet_type: Option<QuicPacketType>,
        frame_type: QuicFrameType,
        byte_matches: Vec<PayloadByteMatch>,
        byte_sequences: Vec<PayloadByteSequenceMatch>,
    },
    RouteResolved,
    All(Vec<FlowPredicate>),
    Any(Vec<FlowPredicate>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NarrativeSurface {
    Program,
    Reason,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SignalKind {
    ProcessBound,
    SocketStateTransition,
    PacketObserved,
    DatagramObserved,
    RouteResolved,
    SynSeen,
    UdpDatagramSeen,
    ProcessIdentified,
    StateChange,
    RouteChanged,
    FinOrRst,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NarrativeTemplate {
    None,
    Static(String),
    ProcessBound,
    PacketObserved,
    TransportPayloadSent,
    TransportPayloadReceived,
    TcpStateTransition,
    RouteChanged,
    UdpDatagramObserved,
    UdpDatagramSent,
    UdpDatagramReceived,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuleTemplate {
    pub predicate: FlowPredicate,
    pub signal: Option<SignalKind>,
    pub narrative: NarrativeTemplate,
    pub dedupe: bool,
    pub module: Option<String>,
    pub phase: Option<String>,
}

pub fn matches_flow_predicate(
    predicate: &FlowPredicate,
    flow: &FlowSnapshot,
    fact: &FactEnvelope,
    facts: &[FactEnvelope],
) -> bool {
    matching::matches_flow_predicate(predicate, flow, fact, facts)
}
