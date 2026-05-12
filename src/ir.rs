use crate::flow::FlowSnapshot;
use crate::ledger::{FactEnvelope, FactKind, PacketDir};
pub use crate::ledger::{QuicFrameType, QuicPacketType};

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
            "send_request"
            | "send_response"
            | "send_connect_request"
            | "send_client_hello"
            | "send_ping"
            | "send_query"
            | "send_connect"
            | "send_ehlo"
            | "send_mail_from"
            | "send_rcpt_to"
            | "send_data"
            | "send_message_body"
            | "send_client_banner"
            | "send_key_exchange_init"
            | "send_channel_open"
            | "send_method_greeting"
            | "send_auth_user"
            | "send_auth_pass"
            | "send_auth_request"
            | "send_pasv"
            | "send_list"
            | "send_retr"
            | "send_stor"
            | "send_port"
            | "send_bind"
            | "send_search"
            | "send_modify"
            | "send_password"
            | "send_get"
            | "send_set"
            | "send_protocol_header"
            | "send_start_ok"
            | "send_publish"
            | "send_crypto"
            | "send_stream"
            | "send_request_stream"
            | "send_response_stream"
            | "send_auth_request_stream"
            | "send_auth_ok_stream"
            | "send_tcp_request_stream"
            | "send_close" => Some("emit_payload"),
            "receive_request"
            | "receive_connect_established"
            | "receive_connect_denied"
            | "receive_auth_required"
            | "receive_ok"
            | "receive_error"
            | "receive_response"
            | "receive_auth"
            | "receive_ehlo_ok"
            | "receive_mail_ok"
            | "receive_rcpt_denied"
            | "receive_rcpt_ok"
            | "receive_data_ready"
            | "receive_message_denied"
            | "receive_message_queued"
            | "receive_ready"
            | "receive_pong"
            | "receive_connack"
            | "receive_banner"
            | "receive_server_banner"
            | "receive_password_required"
            | "receive_auth_denied"
            | "receive_auth_success"
            | "receive_channel_open_confirmation"
            | "receive_auth_ok"
            | "receive_port_ready"
            | "receive_pasv_ready"
            | "receive_transfer_open"
            | "receive_transfer_complete"
            | "receive_method_selection"
            | "receive_connect_success"
            | "receive_bind_denied"
            | "receive_bind_response"
            | "receive_search_result"
            | "receive_modify_response"
            | "receive_modify_denied"
            | "receive_modify_constraint_violation"
            | "receive_value"
            | "receive_stored"
            | "receive_start"
            | "receive_crypto"
            | "receive_close"
            | "receive_response_stream"
            | "receive_request_stream"
            | "receive_auth_ok_stream"
            | "receive_tcp_response_stream" => Some("receive_payload"),
            "receive_ack" => Some("receive_payload"),
            "send_udp_relay_datagram" => Some("emit_datagram"),
            "receive_udp_relay_datagram" => Some("receive_datagram"),
            _ => None,
        },
        SignalKind::DatagramObserved | SignalKind::UdpDatagramSeen => match phase {
            "send_request"
            | "send_initial"
            | "send_handshake"
            | "send_discover"
            | "send_initiation"
            | "send_echo_request"
            | "send_query"
            | "send_search"
            | "send_access_request"
            | "send_get_request"
            | "send_register" => Some("emit_datagram"),
            "receive_reply"
            | "receive_handshake"
            | "receive_initial"
            | "receive_echo_response"
            | "receive_response"
            | "receive_offer"
            | "receive_access_accept"
            | "receive_get_response"
            | "receive_ok" => Some("receive_datagram"),
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
                        && local_port
                            .as_ref()
                            .is_none_or(|expected| packet.local_port == Some(*expected))
                        && remote_port
                            .as_ref()
                            .is_none_or(|expected| packet.remote_port == Some(*expected))
                        && match (first_byte_mask, first_byte_value) {
                            (Some(mask), Some(value)) => packet
                                .payload_byte0
                                .is_some_and(|byte| byte & *mask == *value),
                            _ => true,
                        }
                        && prefix4
                            .as_ref()
                            .is_none_or(|expected| packet.payload_prefix4 == Some(*expected))
                        && match (byte4_mask, byte4_value) {
                            (Some(mask), Some(value)) => packet
                                .payload_byte4
                                .is_some_and(|byte| byte & *mask == *value),
                            _ => true,
                        }
                        && match (byte13_mask, byte13_value) {
                            (Some(mask), Some(value)) => packet
                                .payload_byte13
                                .is_some_and(|byte| byte & *mask == *value),
                            _ => true,
                        }
                        && byte_matches.iter().all(|matcher| {
                            packet_payload_byte_at(packet, matcher.offset)
                                .is_some_and(|byte| byte & matcher.mask == matcher.value)
                        })
                        && byte_sequences
                            .iter()
                            .all(|matcher| packet_payload_sequence_at(packet, matcher))
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
                        && local_port
                            .as_ref()
                            .is_none_or(|expected| packet.local_port == Some(*expected))
                        && remote_port
                            .as_ref()
                            .is_none_or(|expected| packet.remote_port == Some(*expected))
                        && min_len
                            .as_ref()
                            .is_none_or(|expected| packet.tot_len >= *expected)
                        && match (first_byte_mask, first_byte_value) {
                            (Some(mask), Some(value)) => packet
                                .payload_byte0
                                .is_some_and(|byte| byte & *mask == *value),
                            _ => true,
                        }
                        && prefix2
                            .as_ref()
                            .is_none_or(|expected| packet.payload_prefix2 == Some(*expected))
                        && prefix4
                            .as_ref()
                            .is_none_or(|expected| packet.payload_prefix4 == Some(*expected))
                        && match (byte13_mask, byte13_value) {
                            (Some(mask), Some(value)) => packet
                                .payload_byte13
                                .is_some_and(|byte| byte & *mask == *value),
                            _ => true,
                        }
                        && byte_matches.iter().all(|matcher| {
                            packet_payload_byte_at(packet, matcher.offset)
                                .is_some_and(|byte| byte & matcher.mask == matcher.value)
                        })
                        && byte_sequences
                            .iter()
                            .all(|matcher| packet_payload_sequence_at(packet, matcher))
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
                        && local_port
                            .as_ref()
                            .is_none_or(|expected| packet.local_port == Some(*expected))
                        && remote_port
                            .as_ref()
                            .is_none_or(|expected| packet.remote_port == Some(*expected))
                        && min_len
                            .as_ref()
                            .is_none_or(|expected| packet.tot_len >= *expected)
                        && long_header
                            .as_ref()
                            .is_none_or(|expected| quic_long_header(packet) == Some(*expected))
                        && packet_type
                            .as_ref()
                            .is_none_or(|expected| quic_packet_type(packet) == Some(*expected))
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
                        && local_port
                            .as_ref()
                            .is_none_or(|expected| quic.local_port == Some(*expected))
                        && remote_port
                            .as_ref()
                            .is_none_or(|expected| quic.remote_port == Some(*expected))
                        && packet_type
                            .as_ref()
                            .is_none_or(|expected| quic.packet_type == Some(*expected))
                        && quic.frame_types.contains(frame_type)
                        && byte_matches.iter().all(|matcher| {
                            quic.payload_bytes
                                .get(&matcher.offset)
                                .is_some_and(|byte| *byte & matcher.mask == matcher.value)
                        })
                        && byte_sequences.iter().all(|matcher| {
                            matcher.bytes.iter().enumerate().all(|(idx, expected)| {
                                quic.payload_bytes
                                    .get(&(matcher.offset + idx as u16))
                                    .is_some_and(|byte| byte == expected)
                            })
                        })
            )
        }
        FlowPredicate::All(predicates) => {
            predicates
                .iter()
                .all(|predicate| flow_predicate_satisfied_in_flow(predicate, flow, facts))
                && predicates
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
                format!(
                    "process {} (pid={}) bound this network flow",
                    process.comm, process.pid
                )
            }
            NarrativeSurface::Reason => {
                format!(
                    "flow bound to process {} (pid={})",
                    process.comm, process.pid
                )
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
