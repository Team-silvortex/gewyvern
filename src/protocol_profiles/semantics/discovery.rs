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

pub(super) fn dns_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    match entry {
        "udp" => summary(
            "name-resolution-path",
            "UDP DNS lookup request and response posture on the resolver path",
            Some("UDP DNS query and response"),
            None,
            None,
            None,
        ),
        "tcp" => summary(
            "name-resolution-path",
            "TCP-carried DNS query and response posture for stream-based resolver paths",
            Some("TCP DNS query with two-byte length prefix"),
            None,
            None,
            None,
        ),
        "error" => summary(
            "name-resolution-error",
            "UDP DNS resolver response carrying FORMERR, SERVFAIL, NXDOMAIN, or REFUSED",
            Some("DNS QR response with non-zero rcode"),
            Some("name_resolution_failed"),
            Some("dns_error_rcode"),
            Some("direct_protocol_signal"),
        ),
        "tcp-error" => summary(
            "name-resolution-error",
            "TCP-carried DNS resolver response carrying FORMERR, SERVFAIL, NXDOMAIN, or REFUSED",
            Some("TCP DNS QR response with non-zero rcode"),
            Some("name_resolution_failed"),
            Some("dns_tcp_error_rcode"),
            Some("direct_protocol_signal"),
        ),
        _ => None,
    }
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

pub(super) fn rip_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    let (category, operator_focus, typical_signal) = match entry {
        "request" => (
            "distance-vector-route-request",
            "RIP route table request asking a neighbor for distance-vector routes",
            Some("RIP command 1 on UDP/520"),
        ),
        "response" => (
            "distance-vector-route-update",
            "RIP route update response advertising distance-vector reachability",
            Some("RIP command 2 on UDP/520"),
        ),
        "unreachable" => (
            "distance-vector-route-withdrawal",
            "RIP route update advertising an unreachable metric for one or more routes",
            Some("RIP command 2 with metric 16"),
        ),
        _ => return None,
    };
    summary(
        category,
        operator_focus,
        typical_signal,
        None,
        None,
        Some("protocol_entry_signal"),
    )
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

pub(super) fn llmnr_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    let (category, operator_focus, typical_signal) = match entry {
        "query" => (
            "local-name-query-path",
            "LLMNR local-link name query from a host toward nearby responders",
            Some("QR=0 query on UDP/5355"),
        ),
        "response" => (
            "local-name-response-path",
            "LLMNR local-link name answer received from a responder",
            Some("QR=1 response on UDP/5355"),
        ),
        "error" => (
            "local-name-error-path",
            "LLMNR response carrying a local resolver error code",
            Some("QR=1 response with non-zero rcode"),
        ),
        _ => return None,
    };
    summary(
        category,
        operator_focus,
        typical_signal,
        None,
        None,
        Some("protocol_entry_signal"),
    )
}

pub(super) fn nbns_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    let (category, operator_focus, typical_signal) = match entry {
        "query" => (
            "legacy-local-name-query-path",
            "NetBIOS name query from a host toward nearby Windows-style name responders",
            Some("NBNS query on UDP/137"),
        ),
        "response" => (
            "legacy-local-name-response-path",
            "NetBIOS name answer received from a local name responder",
            Some("NBNS response on UDP/137"),
        ),
        "negative" => (
            "legacy-local-name-negative-path",
            "NetBIOS name response indicating lookup failure or refusal",
            Some("NBNS response with non-zero rcode"),
        ),
        _ => return None,
    };
    summary(
        category,
        operator_focus,
        typical_signal,
        None,
        None,
        Some("protocol_entry_signal"),
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

pub(super) fn gtpu_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    if entry != "echo" {
        return None;
    }
    summary(
        "tunnel-liveness-path",
        "GTP-U echo request and echo response validating user-plane tunnel reachability",
        Some("GTP-U Echo Request 0x01 + Echo Response 0x02"),
        None,
        None,
        Some("protocol_entry_signal"),
    )
}

pub(super) fn coap_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    let (category, operator_focus, typical_signal) = match entry {
        "get" => (
            "constrained-resource-read-path",
            "CoAP GET request and content response for constrained resource reads",
            Some("CoAP GET + 2.05 Content"),
        ),
        "post" => (
            "constrained-resource-create-path",
            "CoAP POST request and created response for constrained resource writes",
            Some("CoAP POST + 2.01 Created"),
        ),
        "put" => (
            "constrained-resource-update-path",
            "CoAP PUT request and changed response for constrained resource updates",
            Some("CoAP PUT + 2.04 Changed"),
        ),
        "delete" => (
            "constrained-resource-delete-path",
            "CoAP DELETE request and deleted response for constrained resource removal",
            Some("CoAP DELETE + 2.02 Deleted"),
        ),
        _ => return None,
    };
    summary(
        category,
        operator_focus,
        typical_signal,
        None,
        None,
        Some("protocol_entry_signal"),
    )
}

pub(super) fn dhcp_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    let (category, operator_focus, typical_signal) = match entry {
        "client" => (
            "lease-client-path",
            "DHCP client request and server reply posture on UDP ports 67/68",
            Some("DHCP client/server datagrams"),
        ),
        "discover" => (
            "lease-discovery-path",
            "DHCP discover and offer exchange before a lease is requested",
            Some("DHCPDISCOVER + DHCPOFFER"),
        ),
        "request" => (
            "lease-request-path",
            "DHCP request and acknowledgement exchange for lease acquisition or renewal",
            Some("DHCPREQUEST + DHCPACK"),
        ),
        "nak" => (
            "lease-denied-path",
            "DHCP server rejected a requested lease with a negative acknowledgement",
            Some("DHCPNAK"),
        ),
        _ => return None,
    };
    summary(
        category,
        operator_focus,
        typical_signal,
        None,
        None,
        Some("protocol_entry_signal"),
    )
}

pub(super) fn dhcpv6_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    let (category, operator_focus, typical_signal) = match entry {
        "solicit" => (
            "ipv6-lease-discovery-path",
            "DHCPv6 Solicit and Advertise exchange on UDP ports 546/547",
            Some("SOLICIT + ADVERTISE"),
        ),
        "request" => (
            "ipv6-lease-request-path",
            "DHCPv6 Request and Reply exchange for IPv6 lease acquisition or renewal",
            Some("REQUEST + REPLY"),
        ),
        "release" => (
            "ipv6-lease-release-path",
            "DHCPv6 client releases an IPv6 lease back to the server",
            Some("RELEASE"),
        ),
        _ => return None,
    };
    summary(
        category,
        operator_focus,
        typical_signal,
        None,
        None,
        Some("protocol_entry_signal"),
    )
}

pub(super) fn ntp_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    let (category, operator_focus, typical_signal) = match entry {
        "client" => (
            "time-client-path",
            "NTP client exchange against a time server on UDP/123",
            Some("client mode request + server mode response"),
        ),
        "query" => (
            "time-query-path",
            "NTP query probe and server response used for time reachability diagnostics",
            Some("mode 3 request + mode 4 response"),
        ),
        "sync" => (
            "time-sync-path",
            "NTP synchronization-oriented request and response for clock discipline",
            Some("synchronization request + response"),
        ),
        _ => return None,
    };
    summary(
        category,
        operator_focus,
        typical_signal,
        None,
        None,
        Some("protocol_entry_signal"),
    )
}
