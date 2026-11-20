use crate::flow::FlowSnapshot;
use crate::ledger::{FactEnvelope, PacketDir};
pub use crate::ledger::{QuicFrameType, QuicPacketType};

mod matching;
mod narrative;
mod ntp;
mod predicates;
mod rtsp;
mod snmp;
mod stun;
#[cfg(test)]
mod tests;

pub(crate) use self::matching::matches_flow_predicate_refs;
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RtspResponseKind {
    Options,
    Describe,
    Setup,
    Play,
}

impl RtspResponseKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Options => "options_ok",
            Self::Describe => "describe_ok",
            Self::Setup => "setup_ok",
            Self::Play => "play_ok",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SnmpPduKind {
    Get,
    GetNext,
    GetBulk,
    Response,
}

impl SnmpPduKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Get => "get",
            Self::GetNext => "get_next",
            Self::GetBulk => "get_bulk",
            Self::Response => "response",
        }
    }

    pub fn from_label(label: &str) -> Option<Self> {
        match label {
            "get" => Some(Self::Get),
            "get_next" => Some(Self::GetNext),
            "get_bulk" => Some(Self::GetBulk),
            "response" => Some(Self::Response),
            _ => None,
        }
    }

    pub fn direction(self) -> PacketDir {
        if self == Self::Response {
            PacketDir::Ingress
        } else {
            PacketDir::Egress
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StunBindingKind {
    Request,
    Response,
    Error,
}

impl StunBindingKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Request => "request",
            Self::Response => "response",
            Self::Error => "error",
        }
    }

    pub fn from_label(label: &str) -> Option<Self> {
        match label {
            "request" => Some(Self::Request),
            "response" => Some(Self::Response),
            "error" => Some(Self::Error),
            _ => None,
        }
    }

    pub fn direction(self) -> PacketDir {
        if self == Self::Request {
            PacketDir::Egress
        } else {
            PacketDir::Ingress
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NtpMessageKind {
    Request,
    Response,
    TimeReply,
}

impl NtpMessageKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Request => "request",
            Self::Response => "response",
            Self::TimeReply => "time_reply",
        }
    }

    pub fn from_label(label: &str) -> Option<Self> {
        match label {
            "request" => Some(Self::Request),
            "response" => Some(Self::Response),
            "time_reply" => Some(Self::TimeReply),
            _ => None,
        }
    }

    pub fn direction(self) -> PacketDir {
        if self == Self::Request {
            PacketDir::Egress
        } else {
            PacketDir::Ingress
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FlowPredicate {
    ProcessBound,
    StunBindingObserved {
        remote_port: u16,
        kind: StunBindingKind,
    },
    NtpMessageObserved {
        remote_port: u16,
        kind: NtpMessageKind,
    },
    SnmpPduObserved {
        remote_port: u16,
        kind: SnmpPduKind,
    },
    RtspResponseObserved {
        remote_port: u16,
        kind: RtspResponseKind,
    },
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
