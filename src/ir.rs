use crate::flow::FlowSnapshot;
use crate::ledger::{FactEnvelope, FactKind, PacketDir};

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
    },
    DatagramObserved { l4_proto: u8, dir: Option<PacketDir> },
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
    Static(&'static str),
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

impl SignalKind {
    pub fn id(&self) -> &'static str {
        match self {
            SignalKind::ProcessBound => "process_bound",
            SignalKind::SocketStateTransition => "socket_state_transition",
            SignalKind::PacketObserved => "packet_observed",
            SignalKind::DatagramObserved => "datagram_observed",
            SignalKind::RouteResolved => "route_resolved",
            SignalKind::SynSeen => "syn_seen",
            SignalKind::UdpDatagramSeen => "udp_datagram_seen",
            SignalKind::ProcessIdentified => "process_identified",
            SignalKind::StateChange => "state_change",
            SignalKind::RouteChanged => "route_changed",
            SignalKind::FinOrRst => "fin_or_rst",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "process_bound" => Some(Self::ProcessBound),
            "socket_state_transition" => Some(Self::SocketStateTransition),
            "packet_observed" => Some(Self::PacketObserved),
            "datagram_observed" => Some(Self::DatagramObserved),
            "route_resolved" => Some(Self::RouteResolved),
            "syn_seen" => Some(Self::SynSeen),
            "udp_datagram_seen" => Some(Self::UdpDatagramSeen),
            "process_identified" => Some(Self::ProcessIdentified),
            "state_change" => Some(Self::StateChange),
            "route_changed" => Some(Self::RouteChanged),
            "fin_or_rst" => Some(Self::FinOrRst),
            _ => None,
        }
    }
}

pub fn phase_kind(signal: &SignalKind, phase: Option<&str>) -> Option<&'static str> {
    let phase = phase?;
    match signal {
        SignalKind::ProcessBound | SignalKind::ProcessIdentified => Some("bind_process"),
        SignalKind::RouteResolved | SignalKind::RouteChanged => Some("resolve_route"),
        SignalKind::SocketStateTransition | SignalKind::StateChange => match phase {
            "bind" => Some("bind_socket"),
            "connect" => Some("initiate_connection"),
            "establish" => Some("establish_connection"),
            "accept" => Some("accept_connection"),
            _ => None,
        },
        SignalKind::PacketObserved => match phase {
            "send_request" | "send_response" | "send_client_hello" => Some("emit_payload"),
            "receive_request" | "receive_response" => Some("receive_payload"),
            _ => None,
        },
        SignalKind::DatagramObserved | SignalKind::UdpDatagramSeen => match phase {
            "send_request" | "send_initial" => Some("emit_datagram"),
            "receive_reply" | "receive_handshake" => Some("receive_datagram"),
            _ => None,
        },
        SignalKind::SynSeen => Some("initiate_connection"),
        SignalKind::FinOrRst => Some("terminate_connection"),
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
                    if local_port
                        .as_ref()
                        .is_none_or(|expected| state.sport == *expected)
                        && remote_port
                            .as_ref()
                            .is_none_or(|expected| state.dport == *expected)
                        && min_new_state
                            .as_ref()
                            .is_none_or(|expected| state.new >= *expected)
            )
        }
        FlowPredicate::PacketObserved { l4_proto, dir } => {
            if !flow.evidence.packet_facts.contains(&fact.id) {
                return false;
            }
            matches!(
                &fact.kind,
                FactKind::PacketMeta(packet)
                    if packet.l4_proto == *l4_proto
                        && dir.as_ref().is_none_or(|expected| packet.dir == *expected)
            )
        }
        FlowPredicate::DatagramObserved { l4_proto, dir } => {
            if !flow.evidence.packet_facts.contains(&fact.id) {
                return false;
            }
            matches!(
                &fact.kind,
                FactKind::PacketMeta(packet)
                    if packet.l4_proto == *l4_proto
                        && dir.as_ref().is_none_or(|expected| packet.dir == *expected)
            )
        }
        FlowPredicate::RouteResolved => flow.evidence.route_facts.contains(&fact.id),
        FlowPredicate::All(predicates) => predicates
            .iter()
            .all(|predicate| flow_predicate_satisfied_in_flow(predicate, flow, facts))
            && predicates
                .iter()
                .any(|predicate| matches_flow_predicate(predicate, flow, fact, facts)),
        FlowPredicate::Any(predicates) => predicates
            .iter()
            .any(|predicate| matches_flow_predicate(predicate, flow, fact, facts)),
    }
}

pub fn flow_predicate_satisfied_in_flow(
    predicate: &FlowPredicate,
    flow: &FlowSnapshot,
    facts: &[FactEnvelope],
) -> bool {
    facts
        .iter()
        .any(|fact| matches_flow_predicate(predicate, flow, fact, facts))
}

pub fn render_narrative_template(
    template: &NarrativeTemplate,
    surface: NarrativeSurface,
    flow: &FlowSnapshot,
    fact: &FactEnvelope,
) -> Option<String> {
    match template {
        NarrativeTemplate::None => None,
        NarrativeTemplate::Static(line) => Some((*line).into()),
        NarrativeTemplate::ProcessBound => flow.process.as_ref().map(|process| match surface {
            NarrativeSurface::Program => {
                format!("process {} (pid={}) bound this network flow", process.comm, process.pid)
            }
            NarrativeSurface::Reason => {
                format!("flow bound to process {} (pid={})", process.comm, process.pid)
            }
        }),
        NarrativeTemplate::PacketObserved => Some(match surface {
            NarrativeSurface::Program => "program observed a transport packet for this flow".into(),
            NarrativeSurface::Reason => "transport packet observed".into(),
        }),
        NarrativeTemplate::TransportPayloadSent => Some(match surface {
            NarrativeSurface::Program => {
                "program sent transport payload on this network flow".into()
            }
            NarrativeSurface::Reason => "transport payload sent".into(),
        }),
        NarrativeTemplate::TransportPayloadReceived => Some(match surface {
            NarrativeSurface::Program => {
                "program received transport payload on this network flow".into()
            }
            NarrativeSurface::Reason => "transport payload received".into(),
        }),
        NarrativeTemplate::TcpStateTransition => {
            let FactKind::TcpState(state) = &fact.kind else {
                return None;
            };
            Some(match surface {
                NarrativeSurface::Program => {
                    format!("program observed tcp state {} -> {}", state.old, state.new)
                }
                NarrativeSurface::Reason => format!("tcp state {} -> {}", state.old, state.new),
            })
        }
        NarrativeTemplate::RouteChanged => Some(match surface {
            NarrativeSurface::Program => "program resolved a route for this network flow".into(),
            NarrativeSurface::Reason => "route fingerprint updated".into(),
        }),
        NarrativeTemplate::UdpDatagramObserved => Some(match surface {
            NarrativeSurface::Program => "program emitted or received a UDP datagram".into(),
            NarrativeSurface::Reason => "udp datagram observed".into(),
        }),
        NarrativeTemplate::UdpDatagramSent => Some(match surface {
            NarrativeSurface::Program => "program emitted a UDP datagram".into(),
            NarrativeSurface::Reason => "udp datagram sent".into(),
        }),
        NarrativeTemplate::UdpDatagramReceived => Some(match surface {
            NarrativeSurface::Program => "program received a UDP datagram".into(),
            NarrativeSurface::Reason => "udp datagram received".into(),
        }),
    }
}
