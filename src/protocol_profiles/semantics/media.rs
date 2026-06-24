use super::common::{failure, summary};
use crate::protocol_profiles::ProtocolEntrySemanticsSummary;

pub(super) fn sip_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    match entry {
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
