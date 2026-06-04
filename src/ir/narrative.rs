use crate::flow::FlowSnapshot;
use crate::ledger::{FactEnvelope, FactKind, FactKindTag};

use super::{NarrativeSurface, NarrativeTemplate, SignalKind};

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

    pub fn required_fact_kinds(&self) -> Vec<FactKindTag> {
        match self {
            SignalKind::ProcessBound | SignalKind::ProcessIdentified => {
                vec![FactKindTag::SockLineage]
            }
            SignalKind::SocketStateTransition
            | SignalKind::StateChange
            | SignalKind::SynSeen
            | SignalKind::FinOrRst => vec![FactKindTag::TcpState],
            SignalKind::PacketObserved => vec![FactKindTag::PacketMeta],
            SignalKind::DatagramObserved | SignalKind::UdpDatagramSeen => {
                vec![FactKindTag::PacketMeta]
            }
            SignalKind::RouteResolved | SignalKind::RouteChanged => {
                vec![FactKindTag::RouteDecision]
            }
        }
    }

    pub fn phase_kind(&self, phase: Option<&str>) -> Option<&'static str> {
        let phase = phase?;
        match self {
            SignalKind::ProcessBound | SignalKind::ProcessIdentified => Some("bind_process"),
            SignalKind::RouteResolved | SignalKind::RouteChanged => Some("resolve_route"),
            SignalKind::SocketStateTransition | SignalKind::StateChange => {
                socket_state_phase_kind(phase)
            }
            SignalKind::PacketObserved => packet_phase_kind(phase),
            SignalKind::DatagramObserved | SignalKind::UdpDatagramSeen => {
                datagram_phase_kind(phase)
            }
            SignalKind::SynSeen => Some("initiate_connection"),
            SignalKind::FinOrRst => Some("terminate_connection"),
        }
    }

    pub fn suspect_area(&self) -> &'static str {
        match self {
            SignalKind::ProcessBound | SignalKind::ProcessIdentified => "process_binding",
            SignalKind::SocketStateTransition
            | SignalKind::StateChange
            | SignalKind::SynSeen
            | SignalKind::FinOrRst => "socket_state",
            SignalKind::PacketObserved => "transport_io",
            SignalKind::DatagramObserved | SignalKind::UdpDatagramSeen => "datagram_io",
            SignalKind::RouteResolved | SignalKind::RouteChanged => "route_resolution",
        }
    }
}

impl NarrativeTemplate {
    pub fn required_fact_kinds(&self) -> Vec<FactKindTag> {
        match self {
            NarrativeTemplate::None | NarrativeTemplate::Static(_) => Vec::new(),
            NarrativeTemplate::ProcessBound => vec![FactKindTag::SockLineage],
            NarrativeTemplate::PacketObserved
            | NarrativeTemplate::TransportPayloadSent
            | NarrativeTemplate::TransportPayloadReceived => vec![FactKindTag::PacketMeta],
            NarrativeTemplate::TcpStateTransition => vec![FactKindTag::TcpState],
            NarrativeTemplate::RouteChanged => vec![FactKindTag::RouteDecision],
            NarrativeTemplate::UdpDatagramObserved
            | NarrativeTemplate::UdpDatagramSent
            | NarrativeTemplate::UdpDatagramReceived => vec![FactKindTag::PacketMeta],
        }
    }
}

pub fn phase_kind(signal: &SignalKind, phase: Option<&str>) -> Option<&'static str> {
    signal.phase_kind(phase)
}

pub fn render_phase_transition_kind(
    previous: Option<(&SignalKind, Option<&str>)>,
    current: (&SignalKind, Option<&str>),
) -> String {
    let current_kind = current.0.phase_kind(current.1).unwrap_or("unknown");
    match previous {
        Some((previous_signal, previous_phase)) => {
            let previous_kind = previous_signal
                .phase_kind(previous_phase)
                .unwrap_or("start");
            format!("{previous_kind}->{current_kind}")
        }
        None => format!("start->{current_kind}"),
    }
}

fn socket_state_phase_kind(phase: &str) -> Option<&'static str> {
    match phase {
        "bind" => Some("bind_socket"),
        "connect" => Some("initiate_connection"),
        "establish" => Some("establish_connection"),
        "accept" => Some("accept_connection"),
        _ => None,
    }
}

fn packet_phase_kind(phase: &str) -> Option<&'static str> {
    match phase {
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
        | "send_login"
        | "send_user"
        | "send_pass"
        | "send_options"
        | "send_describe"
        | "send_setup"
        | "send_pasv"
        | "send_list"
        | "send_retr"
        | "send_stor"
        | "send_port"
        | "send_select"
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
        | "receive_login_ok"
        | "receive_user_ok"
        | "receive_options_ok"
        | "receive_describe_ok"
        | "receive_setup_ok"
        | "receive_port_ready"
        | "receive_pasv_ready"
        | "receive_transfer_open"
        | "receive_transfer_complete"
        | "receive_mailbox_selected"
        | "receive_list_ready"
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
        | "receive_tcp_response_stream"
        | "receive_ack" => Some("receive_payload"),
        "send_udp_relay_datagram" => Some("emit_datagram"),
        "receive_udp_relay_datagram" => Some("receive_datagram"),
        _ => None,
    }
}

fn datagram_phase_kind(phase: &str) -> Option<&'static str> {
    match phase {
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
        | "send_as_request"
        | "send_tgs_request"
        | "send_register" => Some("emit_datagram"),
        "receive_reply"
        | "receive_handshake"
        | "receive_initial"
        | "receive_echo_response"
        | "receive_response"
        | "receive_offer"
        | "receive_access_accept"
        | "receive_get_response"
        | "receive_as_reply"
        | "receive_tgs_reply"
        | "receive_error"
        | "receive_ok" => Some("receive_datagram"),
        _ => None,
    }
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
            NarrativeSurface::Program => format!(
                "process {} (pid={}) bound this network flow",
                process.comm, process.pid
            ),
            NarrativeSurface::Reason => format!(
                "flow bound to process {} (pid={})",
                process.comm, process.pid
            ),
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
