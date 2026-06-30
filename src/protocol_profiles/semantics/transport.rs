use super::common::{failure, summary};
use crate::protocol_profiles::ProtocolEntrySemanticsSummary;

pub(super) fn http_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    match entry {
        "denied" => failure(
            "proxy tunnel refusal after CONNECT policy evaluation",
            Some("403"),
            Some("server_denied"),
            Some("access_denied"),
        ),
        _ => None,
    }
}

pub(super) fn http3_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    match entry {
        "close" => failure(
            "application-layer HTTP/3 request path terminated by peer connection close before steady response completion",
            Some("CONNECTION_CLOSE"),
            Some("peer_closed"),
            Some("transport_terminated"),
        ),
        "server-close" => failure(
            "HTTP/3 server response path ended with a locally emitted connection close after request handling and response delivery had already started",
            Some("CONNECTION_CLOSE"),
            Some("local_closed"),
            Some("server_terminated_session"),
        ),
        _ => None,
    }
}

pub(super) fn grpc_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    match entry {
        "call" => summary(
            "rpc-call-path",
            "gRPC unary call over HTTP/2 with :path, content-type, and DATA frames",
            Some("content-type: application/grpc"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        "status" => summary(
            "rpc-status-path",
            "gRPC response trailer carrying grpc-status and optional grpc-message",
            Some("grpc-status trailer"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        "stream" => summary(
            "rpc-stream-path",
            "gRPC streaming RPC with repeated HTTP/2 DATA frames",
            Some("HTTP/2 DATA continuation"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        _ => None,
    }
}

pub(super) fn websocket_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    match entry {
        "upgrade" => summary(
            "websocket-upgrade-path",
            "HTTP Upgrade request followed by an HTTP 101 Switching Protocols response",
            Some("GET + 101 Switching Protocols"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        "frame" => summary(
            "websocket-frame-path",
            "WebSocket text or binary frame traffic after the upgrade path",
            Some("opcode text/binary"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        "close" => failure(
            "WebSocket session termination observed through a close control frame",
            Some("opcode close"),
            Some("peer_or_local_closed"),
            Some("session_terminated"),
        ),
        _ => None,
    }
}

pub(super) fn tls_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    match entry {
        "client" => summary(
            "tls-client-path",
            "client-side TLS handshake posture and outbound secure transport setup",
            Some("ClientHello-oriented TCP stream"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        "server" => summary(
            "tls-server-path",
            "server-side TLS accept path and inbound secure transport setup",
            Some("ServerHello-oriented TCP stream"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        "alert" => failure(
            "TLS alert record observed during secure transport negotiation or shutdown",
            Some("TLS record content type 0x15"),
            Some("peer_or_local_alert"),
            Some("tls_alert"),
        ),
        "certificate" => summary(
            "tls-certificate-path",
            "plaintext TLS certificate handshake message carrying peer identity material",
            Some("TLS handshake message type 0x0b"),
            Some("certificate_not_visible"),
            Some("encrypted_tls13_or_fragmented_record"),
            Some("protocol_entry_signal"),
        ),
        _ => None,
    }
}

pub(super) fn graphql_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    match entry {
        "query" => summary(
            "graphql-query-path",
            "GraphQL read-style operation transported over HTTP GET or POST",
            Some("HTTP GraphQL query request"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        "mutation" => summary(
            "graphql-mutation-path",
            "GraphQL write-style operation transported over HTTP POST",
            Some("HTTP POST GraphQL mutation candidate"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        "subscription" => summary(
            "graphql-subscription-path",
            "GraphQL subscription setup over HTTP upgrade or streaming WebSocket traffic",
            Some("HTTP upgrade + WebSocket frame"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        _ => None,
    }
}

pub(super) fn socks5_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    match entry {
        "denied" => failure(
            "upstream connect refusal after no-auth method selection",
            None,
            Some("server_denied"),
            Some("access_denied"),
        ),
        "auth-denied" => failure(
            "username/password rejection during proxy auth exchange",
            None,
            Some("server_denied"),
            Some("access_denied"),
        ),
        "auth-connect-denied" => failure(
            "upstream connect refusal after authenticated proxy setup",
            None,
            Some("server_denied"),
            Some("access_denied"),
        ),
        _ => None,
    }
}

pub(super) fn rdp_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    let (category, operator_focus, typical_signal) = match entry {
        "connect" => (
            "remote-desktop-connect-path",
            "RDP TPKT/X.224 connection establishment on TCP port 3389",
            Some("X.224 connection request"),
        ),
        "channel" => (
            "remote-desktop-channel-path",
            "RDP data TPDU channel traffic after desktop session setup",
            Some("X.224 data TPDU"),
        ),
        "denied" => (
            "remote-desktop-denied-path",
            "RDP connection attempt ended by X.224 disconnect or negotiation failure",
            Some("X.224 disconnect or negotiation failure"),
        ),
        _ => return None,
    };
    if entry == "denied" {
        return summary(
            category,
            operator_focus,
            typical_signal,
            Some("server_denied"),
            Some("rdp_negotiation_failed"),
            Some("direct_protocol_signal"),
        );
    }
    summary(category, operator_focus, typical_signal, None, None, None)
}

pub(super) fn stun_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    match entry {
        "binding-error" => failure(
            "explicit binding failure response instead of successful reachability confirmation",
            None,
            Some("server_denied"),
            Some("access_denied"),
        ),
        _ => None,
    }
}

pub(super) fn icmp_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    match entry {
        "echo" => summary(
            "reachability-path",
            "ICMP echo probe and echo reply reachability check",
            Some("type 8 request / type 0 reply"),
            None,
            None,
            None,
        ),
        "unreachable" => failure(
            "network, host, or port reachability failed with an ICMP unreachable response",
            Some("type 3 unreachable"),
            Some("network_unreachable"),
            Some("remote_or_path_rejected"),
        ),
        _ => None,
    }
}

pub(super) fn icmpv6_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    match entry {
        "echo" => summary(
            "reachability-path",
            "ICMPv6 echo probe and echo reply reachability check",
            Some("type 128 request / type 129 reply"),
            None,
            None,
            None,
        ),
        "unreachable" => failure(
            "IPv6 reachability failed with an ICMPv6 destination unreachable response",
            Some("type 1 destination unreachable"),
            Some("network_unreachable"),
            Some("remote_or_path_rejected"),
        ),
        _ => None,
    }
}

pub(super) fn quic_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    match entry {
        "retry" => summary(
            "continuation-path",
            "peer address-validation continuation during QUIC Retry evaluation",
            Some("Retry"),
            None,
            None,
            None,
        ),
        "close" => failure(
            "peer transport termination during QUIC connection close evaluation",
            Some("CONNECTION_CLOSE"),
            Some("peer_closed"),
            Some("transport_terminated"),
        ),
        "local-close" => failure(
            "local transport termination during QUIC connection close evaluation",
            Some("CONNECTION_CLOSE"),
            Some("local_closed"),
            Some("transport_terminated"),
        ),
        _ => None,
    }
}

pub(super) fn wireguard_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    match entry {
        "cookie" => summary(
            "continuation-path",
            "peer anti-abuse continuation during WireGuard cookie reply evaluation",
            Some("Cookie Reply"),
            None,
            None,
            None,
        ),
        _ => None,
    }
}

pub(super) fn ipsec_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    let (category, operator_focus, typical_signal) = match entry {
        "esp" => (
            "secure-encapsulation-path",
            "IPsec Encapsulating Security Payload traffic on IP protocol 50",
            Some("IP protocol 50 ESP packet"),
        ),
        "ah" => (
            "secure-authentication-header-path",
            "IPsec Authentication Header traffic on IP protocol 51",
            Some("IP protocol 51 AH packet"),
        ),
        _ => return None,
    };
    summary(category, operator_focus, typical_signal, None, None, None)
}

pub(super) fn vxlan_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    let (category, operator_focus, typical_signal) = match entry {
        "encap" => (
            "overlay-encapsulation-path",
            "VXLAN overlay traffic on UDP port 4789 carrying an inner Ethernet frame",
            Some("UDP/4789 VXLAN packet"),
        ),
        "vni" => (
            "overlay-tenant-path",
            "VXLAN packet with the VNI-present flag set for tenant overlay analysis",
            Some("VXLAN flags byte with I flag set"),
        ),
        _ => return None,
    };
    summary(category, operator_focus, typical_signal, None, None, None)
}

pub(super) fn geneve_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    let (category, operator_focus, typical_signal) = match entry {
        "encap" => (
            "overlay-encapsulation-path",
            "GENEVE overlay traffic on UDP port 6081 carrying a virtual network payload",
            Some("UDP/6081 GENEVE packet"),
        ),
        "options" => (
            "overlay-option-path",
            "GENEVE packet carrying option metadata for extensible overlay debugging",
            Some("GENEVE option length bits set"),
        ),
        _ => return None,
    };
    summary(category, operator_focus, typical_signal, None, None, None)
}

pub(super) fn l2tp_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    let (category, operator_focus, typical_signal) = match entry {
        "control" => (
            "tunnel-control-path",
            "L2TP control traffic on UDP port 1701 establishing or maintaining a tunnel",
            Some("UDP/1701 packet with L2TP control flags"),
        ),
        "session" => (
            "tunnel-session-path",
            "L2TP data session traffic on UDP port 1701 carrying tunneled payloads",
            Some("UDP/1701 packet without the control flag"),
        ),
        _ => return None,
    };
    summary(category, operator_focus, typical_signal, None, None, None)
}

pub(super) fn pptp_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    let (category, operator_focus, typical_signal) = match entry {
        "control" => (
            "tunnel-control-path",
            "PPTP TCP control channel traffic on port 1723 with the PPTP magic cookie",
            Some("TCP/1723 PPTP control message"),
        ),
        "data" => (
            "tunnel-data-path",
            "PPTP data traffic carried over GRE after control-channel setup",
            Some("IP protocol 47 GRE data packet"),
        ),
        _ => return None,
    };
    summary(category, operator_focus, typical_signal, None, None, None)
}

pub(super) fn hy2_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    match entry {
        "close" => failure(
            "authenticated Hysteria2 session terminated by peer connection close before relay continuity could be maintained",
            Some("CONNECTION_CLOSE"),
            Some("peer_closed"),
            Some("secure_session_terminated"),
        ),
        "tcp-close" => failure(
            "authenticated Hysteria2 TCP relay terminated by peer connection close after relay request and response activity had already started",
            Some("CONNECTION_CLOSE"),
            Some("peer_closed"),
            Some("tcp_relay_terminated"),
        ),
        "udp-close" => failure(
            "authenticated Hysteria2 UDP relay terminated by peer connection close after relay datagram exchange had already started",
            Some("CONNECTION_CLOSE"),
            Some("peer_closed"),
            Some("udp_relay_terminated"),
        ),
        _ => None,
    }
}
