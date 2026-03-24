use crate::flow::{FlowId, FlowSnapshot};
use crate::ledger::{FactEnvelope, FactId, FactKind, PacketDir};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct ReasonId(pub u64);

#[derive(Clone, Debug, PartialEq)]
pub struct ReasonChain {
    pub id: ReasonId,
    pub flow: FlowId,
    pub l0_facts: Vec<FactId>,
    pub l1: ReasonL1,
    pub l3: ReasonL3,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReasonL1 {
    pub tcp_state_timeline: Vec<FactId>,
    pub path_segments: Vec<FactId>,
    pub key_events: Vec<KeyEvent>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct KeyEvent {
    pub at: FactId,
    pub kind: KeyEventKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum KeyEventKind {
    SynSeen,
    StateChange { old: u8, new: u8 },
    RetransSuspected,
    RouteChanged,
    FinOrRst,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReasonL3 {
    pub narrative: Vec<NarrLine>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NarrLine {
    pub at: FactId,
    pub text: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReasonProfile {
    HandshakeL1,
}

impl ReasonProfile {
    pub fn id(&self) -> &'static str {
        match self {
            Self::HandshakeL1 => "handshake_l1",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "handshake_l1" => Some(Self::HandshakeL1),
            _ => None,
        }
    }
}

pub fn build_reason_chains(
    profile: &ReasonProfile,
    flows: &[FlowSnapshot],
    facts: &[FactEnvelope],
) -> Vec<ReasonChain> {
    match profile {
        ReasonProfile::HandshakeL1 => flows
            .iter()
            .enumerate()
            .map(|(idx, flow)| build_handshake_reason(ReasonId((idx + 1) as u64), flow, facts))
            .collect(),
    }
}

fn build_handshake_reason(id: ReasonId, flow: &FlowSnapshot, facts: &[FactEnvelope]) -> ReasonChain {
    let mut l0_facts = Vec::new();
    let mut timeline = Vec::new();
    let mut path_segments = Vec::new();
    let mut key_events = Vec::new();
    let mut narrative = Vec::new();

    for fact in facts {
        if flow.evidence.tcp_state_facts.contains(&fact.id) {
            l0_facts.push(fact.id);
            timeline.push(fact.id);
            if let FactKind::TcpState(state) = &fact.kind {
                key_events.push(KeyEvent {
                    at: fact.id,
                    kind: KeyEventKind::StateChange {
                        old: state.old,
                        new: state.new,
                    },
                });
                if state.new == 2 {
                    key_events.push(KeyEvent {
                        at: fact.id,
                        kind: KeyEventKind::SynSeen,
                    });
                }
                if state.new >= 7 {
                    key_events.push(KeyEvent {
                        at: fact.id,
                        kind: KeyEventKind::FinOrRst,
                    });
                }
                narrative.push(NarrLine {
                    at: fact.id,
                    text: format!("tcp state {} -> {}", state.old, state.new),
                });
            }
        }

        if flow.evidence.packet_facts.contains(&fact.id) {
            l0_facts.push(fact.id);
            if let FactKind::PacketMeta(packet) = &fact.kind {
                if packet.tcp_flags & 0x02 != 0 && packet.dir == PacketDir::Egress {
                    key_events.push(KeyEvent {
                        at: fact.id,
                        kind: KeyEventKind::SynSeen,
                    });
                }
                if packet.tcp_flags & 0x04 != 0 {
                    key_events.push(KeyEvent {
                        at: fact.id,
                        kind: KeyEventKind::FinOrRst,
                    });
                }
            }
        }

        if flow.evidence.route_facts.contains(&fact.id) {
            l0_facts.push(fact.id);
            path_segments.push(fact.id);
            key_events.push(KeyEvent {
                at: fact.id,
                kind: KeyEventKind::RouteChanged,
            });
            narrative.push(NarrLine {
                at: fact.id,
                text: "route fingerprint updated".into(),
            });
        }
    }

    l0_facts.sort_unstable();
    timeline.sort_unstable();
    path_segments.sort_unstable();
    key_events.sort_by_key(|event| event.at);
    narrative.sort_by_key(|line| line.at);

    ReasonChain {
        id,
        flow: flow.id,
        l0_facts,
        l1: ReasonL1 {
            tcp_state_timeline: timeline,
            path_segments,
            key_events,
        },
        l3: ReasonL3 { narrative },
    }
}
