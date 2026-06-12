use super::*;

pub(super) fn list_or_none(items: &[String]) -> String {
    if items.is_empty() {
        "none".into()
    } else {
        items.join(",")
    }
}

pub(super) fn unique_strings(mut items: Vec<String>) -> Vec<String> {
    items.sort();
    items.dedup();
    items
}

pub(super) fn narrative_summary(narrative: &NarrativeTemplate) -> String {
    match narrative {
        NarrativeTemplate::None => "none".into(),
        NarrativeTemplate::Static(line) => format!("static:{line}"),
        NarrativeTemplate::ProcessBound => "process_bound".into(),
        NarrativeTemplate::PacketObserved => "packet_observed".into(),
        NarrativeTemplate::TransportPayloadSent => "transport_payload_sent".into(),
        NarrativeTemplate::TransportPayloadReceived => "transport_payload_received".into(),
        NarrativeTemplate::TcpStateTransition => "tcp_state_transition".into(),
        NarrativeTemplate::RouteChanged => "route_changed".into(),
        NarrativeTemplate::UdpDatagramObserved => "udp_datagram_observed".into(),
        NarrativeTemplate::UdpDatagramSent => "udp_datagram_sent".into(),
        NarrativeTemplate::UdpDatagramReceived => "udp_datagram_received".into(),
    }
}

pub(super) fn predicate_summary(predicate: &FlowPredicate) -> String {
    match predicate {
        FlowPredicate::ProcessBound => "process_bound".into(),
        FlowPredicate::SocketStateObserved {
            local_port,
            remote_port,
            min_new_state,
        } => format!(
            "socket_state_observed(local_port={},remote_port={},min_new_state={})",
            optional_u16(*local_port),
            optional_u16(*remote_port),
            optional_u8(*min_new_state),
        ),
        FlowPredicate::PacketObserved {
            l4_proto,
            dir,
            local_port,
            remote_port,
            ..
        } => format!(
            "packet_observed(l4_proto={},dir={},local_port={},remote_port={},payload_offsets={:?})",
            l4_proto,
            optional_dir(*dir),
            optional_u16(*local_port),
            optional_u16(*remote_port),
            predicate.required_payload_offsets(),
        ),
        FlowPredicate::DatagramObserved {
            l4_proto,
            dir,
            local_port,
            remote_port,
            min_len,
            ..
        } => format!(
            "datagram_observed(l4_proto={},dir={},local_port={},remote_port={},min_len={},payload_offsets={:?})",
            l4_proto,
            optional_dir(*dir),
            optional_u16(*local_port),
            optional_u16(*remote_port),
            optional_u32(*min_len),
            predicate.required_payload_offsets(),
        ),
        FlowPredicate::QuicPacketObserved {
            dir,
            local_port,
            remote_port,
            min_len,
            long_header,
            packet_type,
        } => format!(
            "quic_packet_observed(dir={},local_port={},remote_port={},min_len={},long_header={},packet_type={})",
            optional_dir(*dir),
            optional_u16(*local_port),
            optional_u16(*remote_port),
            optional_u32(*min_len),
            optional_bool(*long_header),
            packet_type
                .map(|item| format!("{item:?}"))
                .unwrap_or_else(|| "none".into()),
        ),
        FlowPredicate::QuicFrameObserved {
            dir,
            local_port,
            remote_port,
            packet_type,
            frame_type,
            ..
        } => format!(
            "quic_frame_observed(dir={},local_port={},remote_port={},packet_type={},frame_type={frame_type:?},payload_offsets={:?})",
            optional_dir(*dir),
            optional_u16(*local_port),
            optional_u16(*remote_port),
            packet_type
                .map(|item| format!("{item:?}"))
                .unwrap_or_else(|| "none".into()),
            predicate.required_payload_offsets(),
        ),
        FlowPredicate::RouteResolved => "route_resolved".into(),
        FlowPredicate::All(predicates) => format!(
            "all({})",
            predicates
                .iter()
                .map(predicate_summary)
                .collect::<Vec<_>>()
                .join(" && ")
        ),
        FlowPredicate::Any(predicates) => format!(
            "any({})",
            predicates
                .iter()
                .map(predicate_summary)
                .collect::<Vec<_>>()
                .join(" || ")
        ),
    }
}

fn optional_bool(value: Option<bool>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "none".into())
}

fn optional_dir(value: Option<crate::ledger::PacketDir>) -> String {
    value
        .map(|value| format!("{value:?}").to_lowercase())
        .unwrap_or_else(|| "none".into())
}

fn optional_u8(value: Option<u8>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "none".into())
}

fn optional_u16(value: Option<u16>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "none".into())
}

fn optional_u32(value: Option<u32>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "none".into())
}
