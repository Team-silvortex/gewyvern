use super::common::summary;
use crate::protocol_profiles::ProtocolEntrySemanticsSummary;

pub(super) fn arp_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    let (operator_focus, typical_signal) = match entry {
        "request" => (
            "local-link address resolution request asking who owns an IPv4 address",
            Some("ARP opcode 1 who-has"),
        ),
        "reply" => (
            "local-link address resolution answer mapping an IPv4 address to a MAC address",
            Some("ARP opcode 2 is-at"),
        ),
        _ => return None,
    };
    summary(
        "neighbor-resolution-path",
        operator_focus,
        typical_signal,
        None,
        None,
        None,
    )
}

pub(super) fn ndp_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    let (operator_focus, typical_signal) = match entry {
        "solicit" => (
            "IPv6 neighbor solicitation asking who owns a target IPv6 address",
            Some("ICMPv6 type 135 neighbor solicitation"),
        ),
        "advertise" => (
            "IPv6 neighbor advertisement returning link-layer reachability",
            Some("ICMPv6 type 136 neighbor advertisement"),
        ),
        _ => return None,
    };
    summary(
        "neighbor-resolution-path",
        operator_focus,
        typical_signal,
        None,
        None,
        None,
    )
}

pub(super) fn bgp_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    let (operator_focus, typical_signal) = match entry {
        "open" => (
            "BGP peer session establishment exchanging OPEN messages on TCP/179",
            Some("BGP message type 1 OPEN"),
        ),
        "keepalive" => (
            "BGP peer liveness confirmation after a session is established",
            Some("BGP message type 4 KEEPALIVE"),
        ),
        _ => return None,
    };
    summary(
        "routing-control-session",
        operator_focus,
        typical_signal,
        None,
        None,
        None,
    )
}

pub(super) fn ospf_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    let (category, operator_focus, typical_signal) = match entry {
        "hello" => (
            "link-state-neighbor-discovery",
            "OSPF neighbor discovery and liveness on IP protocol 89",
            Some("OSPF packet type 1 Hello"),
        ),
        "dbdesc" => (
            "link-state-database-sync",
            "OSPF database description exchange during adjacency formation",
            Some("OSPF packet type 2 Database Description"),
        ),
        _ => return None,
    };
    summary(category, operator_focus, typical_signal, None, None, None)
}

pub(super) fn gre_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    let (category, operator_focus, typical_signal) = match entry {
        "encap" => (
            "tunnel-encapsulation-path",
            "GRE tunnel encapsulation on IP protocol 47 carrying an inner payload",
            Some("IP protocol 47 GRE packet"),
        ),
        "keepalive" => (
            "tunnel-liveness-path",
            "minimal GRE keepalive-style liveness probe on a tunnel path",
            Some("GRE flags/version prefix 0x0000"),
        ),
        _ => return None,
    };
    summary(category, operator_focus, typical_signal, None, None, None)
}

pub(super) fn mdns_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    let (operator_focus, typical_signal) = match entry {
        "query" => (
            "local-link multicast name lookup from a host toward nearby responders",
            Some("query flags 0x0000"),
        ),
        "response" => (
            "local-link multicast answer or announcement received from a responder",
            Some("response flags 0x8400"),
        ),
        "probe" => (
            "local-link name conflict probing before claiming or advertising a name",
            Some("query flags 0x0000"),
        ),
        _ => return None,
    };
    summary(
        "discovery-path",
        operator_focus,
        typical_signal,
        None,
        None,
        None,
    )
}

pub(super) fn ssdp_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    let (operator_focus, typical_signal) = match entry {
        "discovery" => (
            "active SSDP M-SEARCH discovery and HTTP-style response traffic",
            Some("M-SEARCH / HTTP response"),
        ),
        "notify" => (
            "passive SSDP NOTIFY advertisement, alive, or byebye message",
            Some("NOTIFY"),
        ),
        _ => return None,
    };
    summary(
        "discovery-path",
        operator_focus,
        typical_signal,
        None,
        None,
        None,
    )
}
