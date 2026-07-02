use super::common::{failure, summary};
use crate::protocol_profiles::ProtocolEntrySemanticsSummary;

pub(super) fn rtsp_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    match entry {
        "options" => summary(
            "rtsp-options-path",
            "RTSP OPTIONS probe and 200 response carrying supported methods",
            Some("OPTIONS + Public header"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        "describe" => summary(
            "rtsp-describe-path",
            "RTSP DESCRIBE request and media description response",
            Some("DESCRIBE + Content-Type"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        "setup" => summary(
            "rtsp-setup-path",
            "RTSP OPTIONS, DESCRIBE, and SETUP choreography before media playback",
            Some("SETUP + Session header"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        "play" => summary(
            "rtsp-play-path",
            "RTSP OPTIONS, DESCRIBE, SETUP, and PLAY choreography for media start",
            Some("PLAY + Range header"),
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
