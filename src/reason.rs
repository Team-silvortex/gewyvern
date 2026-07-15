use crate::flow::{FlowId, FlowSnapshot};
use crate::ir::{
    FlowPredicate, NarrativeSurface, NarrativeTemplate, RuleTemplate, SignalKind,
    matches_flow_predicate, render_narrative_template,
};
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
    PacketObserved,
    UdpDatagramSeen,
    ProcessIdentified,
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
    UdpDatagramL1,
    Declarative(ReasonModel),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReasonModel {
    pub id: String,
    pub rules: Vec<ReasonRule>,
}

pub type ReasonPredicate = FlowPredicate;
pub type ReasonKeyEvent = SignalKind;
pub type ReasonNarrative = NarrativeTemplate;
pub type ReasonRule = RuleTemplate;

impl ReasonProfile {
    pub fn id(&self) -> &str {
        match self {
            Self::HandshakeL1 => "handshake_l1",
            Self::UdpDatagramL1 => "udp_datagram_l1",
            Self::Declarative(model) => model.id.as_str(),
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "handshake_l1" => Some(Self::HandshakeL1),
            "udp_datagram_l1" => Some(Self::UdpDatagramL1),
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
        ReasonProfile::UdpDatagramL1 => flows
            .iter()
            .enumerate()
            .map(|(idx, flow)| build_udp_reason(ReasonId((idx + 1) as u64), flow, facts))
            .collect(),
        ReasonProfile::Declarative(model) => flows
            .iter()
            .enumerate()
            .map(|(idx, flow)| {
                build_declarative_reason(model, ReasonId((idx + 1) as u64), flow, facts)
            })
            .collect(),
    }
}

fn build_handshake_reason(
    id: ReasonId,
    flow: &FlowSnapshot,
    facts: &[FactEnvelope],
) -> ReasonChain {
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

        if flow.evidence.quic_facts.contains(&fact.id) {
            l0_facts.push(fact.id);
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

        if flow.evidence.lineage_facts.contains(&fact.id) {
            l0_facts.push(fact.id);
            key_events.push(KeyEvent {
                at: fact.id,
                kind: KeyEventKind::ProcessIdentified,
            });
            if let Some(process) = &flow.process {
                narrative.push(NarrLine {
                    at: fact.id,
                    text: format!(
                        "flow bound to process {} (pid={})",
                        process.comm, process.pid
                    ),
                });
            }
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

fn build_udp_reason(id: ReasonId, flow: &FlowSnapshot, facts: &[FactEnvelope]) -> ReasonChain {
    let mut l0_facts = Vec::new();
    let timeline = Vec::new();
    let mut path_segments = Vec::new();
    let mut key_events = Vec::new();
    let mut narrative = Vec::new();

    for fact in facts {
        if flow.evidence.packet_facts.contains(&fact.id) {
            l0_facts.push(fact.id);
            if let FactKind::PacketMeta(packet) = &fact.kind {
                if packet.l4_proto == 17 {
                    key_events.push(KeyEvent {
                        at: fact.id,
                        kind: KeyEventKind::UdpDatagramSeen,
                    });
                    narrative.push(NarrLine {
                        at: fact.id,
                        text: "udp datagram observed".into(),
                    });
                }
            }
        }

        if flow.evidence.quic_facts.contains(&fact.id) {
            l0_facts.push(fact.id);
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

        if flow.evidence.lineage_facts.contains(&fact.id) {
            l0_facts.push(fact.id);
            key_events.push(KeyEvent {
                at: fact.id,
                kind: KeyEventKind::ProcessIdentified,
            });
            if let Some(process) = &flow.process {
                narrative.push(NarrLine {
                    at: fact.id,
                    text: format!(
                        "flow bound to process {} (pid={})",
                        process.comm, process.pid
                    ),
                });
            }
        }
    }

    l0_facts.sort_unstable();
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

fn build_declarative_reason(
    model: &ReasonModel,
    id: ReasonId,
    flow: &FlowSnapshot,
    facts: &[FactEnvelope],
) -> ReasonChain {
    let mut l0_facts = Vec::new();
    let mut timeline = Vec::new();
    let mut path_segments = Vec::new();
    let mut key_events = Vec::new();
    let mut narrative = Vec::new();
    let mut seen_predicates = Vec::new();

    for fact in facts {
        if flow.evidence.tcp_state_facts.contains(&fact.id)
            || flow.evidence.packet_facts.contains(&fact.id)
            || flow.evidence.quic_facts.contains(&fact.id)
            || flow.evidence.route_facts.contains(&fact.id)
            || flow.evidence.lineage_facts.contains(&fact.id)
        {
            l0_facts.push(fact.id);
        }

        for rule in &model.rules {
            if rule.dedupe && seen_predicates.contains(&rule.predicate) {
                continue;
            }
            if !matches_flow_predicate(&rule.predicate, flow, fact, facts) {
                continue;
            }

            if let Some(event) = render_key_event(rule.signal.as_ref(), fact) {
                if matches!(event.kind, KeyEventKind::StateChange { .. }) {
                    timeline.push(fact.id);
                }
                if matches!(event.kind, KeyEventKind::RouteChanged) {
                    path_segments.push(fact.id);
                }
                key_events.push(event);
            }

            if let Some(line) = render_narrative(&rule.narrative, flow, fact) {
                narrative.push(NarrLine {
                    at: fact.id,
                    text: line,
                });
            }

            if rule.dedupe {
                seen_predicates.push(rule.predicate.clone());
            }
        }
    }

    l0_facts.sort_unstable();
    l0_facts.dedup();
    timeline.sort_unstable();
    timeline.dedup();
    path_segments.sort_unstable();
    path_segments.dedup();
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

fn render_key_event(kind: Option<&ReasonKeyEvent>, fact: &FactEnvelope) -> Option<KeyEvent> {
    let kind = match kind? {
        ReasonKeyEvent::SynSeen => KeyEventKind::SynSeen,
        ReasonKeyEvent::PacketObserved => KeyEventKind::PacketObserved,
        ReasonKeyEvent::UdpDatagramSeen => KeyEventKind::UdpDatagramSeen,
        ReasonKeyEvent::ProcessIdentified => KeyEventKind::ProcessIdentified,
        ReasonKeyEvent::StateChange => {
            let FactKind::TcpState(state) = &fact.kind else {
                return None;
            };
            KeyEventKind::StateChange {
                old: state.old,
                new: state.new,
            }
        }
        ReasonKeyEvent::RouteChanged => KeyEventKind::RouteChanged,
        ReasonKeyEvent::FinOrRst => KeyEventKind::FinOrRst,
        ReasonKeyEvent::ProcessBound => KeyEventKind::ProcessIdentified,
        ReasonKeyEvent::SocketStateTransition => {
            let FactKind::TcpState(state) = &fact.kind else {
                return None;
            };
            KeyEventKind::StateChange {
                old: state.old,
                new: state.new,
            }
        }
        ReasonKeyEvent::DatagramObserved => KeyEventKind::UdpDatagramSeen,
        ReasonKeyEvent::RouteResolved => KeyEventKind::RouteChanged,
    };

    Some(KeyEvent { at: fact.id, kind })
}

fn render_narrative(
    narrative: &ReasonNarrative,
    flow: &FlowSnapshot,
    fact: &FactEnvelope,
) -> Option<String> {
    render_narrative_template(narrative, NarrativeSurface::Reason, flow, fact)
}
