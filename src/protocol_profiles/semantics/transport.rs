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
