use super::common::{failure, summary};
use crate::protocol_profiles::ProtocolEntrySemanticsSummary;

pub(super) fn rtsp_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    match entry {
        "options" => summary(
            "rtsp-options-path",
            "bounded RTSP 200/Public response candidate, not request correlation",
            Some("200 + CSeq + Public methods"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        "describe" => summary(
            "rtsp-describe-path",
            "bounded RTSP SDP response candidate, not media delivery",
            Some("200 + CSeq + application/sdp + positive Content-Length"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        "setup" => summary(
            "rtsp-setup-path",
            "bounded RTSP transport/session response candidate, not generic Session presence",
            Some("200 + CSeq + Session + Transport"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        "play" => summary(
            "rtsp-play-path",
            "bounded RTSP RTP-Info/session response candidate, not generic keepalive success",
            Some("200 + CSeq + Session + RTP-Info"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        _ => None,
    }
}

pub(super) fn sip_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    match entry {
        "register" => summary(
            "sip-register-path",
            "SIP REGISTER request and success response for endpoint registration",
            Some("REGISTER + SIP/2.0 response"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        "invite" => summary(
            "sip-invite-path",
            "SIP INVITE request and response for call/session setup",
            Some("INVITE + SIP/2.0 response"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        "bye" => summary(
            "sip-bye-path",
            "SIP BYE request and response for session teardown",
            Some("BYE + SIP/2.0 response"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        "response" => summary(
            "response-path",
            "SIP response datagram received after a session-control request",
            Some("SIP/2.0"),
            None,
            None,
            None,
        ),
        "denied" => failure(
            "SIP request rejected or failed by a 4xx, 5xx, or 6xx response",
            Some("SIP/2.0 4xx/5xx/6xx"),
            Some("server_denied"),
            Some("session_control_rejected"),
        ),
        _ => None,
    }
}
