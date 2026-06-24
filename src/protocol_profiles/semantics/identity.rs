use super::common::{failure, summary};
use crate::protocol_profiles::ProtocolEntrySemanticsSummary;

pub(super) fn amqp_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    match entry {
        "auth-denied" => failure(
            "broker connection close after AMQP start-ok credential or mechanism negotiation",
            Some("connection.close"),
            Some("server_denied"),
            Some("access_denied"),
        ),
        _ => None,
    }
}

pub(super) fn ftp_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    match entry {
        "denied" => failure(
            "ftp login rejection after USER/PASS credential exchange",
            Some("530"),
            Some("server_denied"),
            Some("access_denied"),
        ),
        _ => None,
    }
}

pub(super) fn imap_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    match entry {
        "auth-denied" => failure(
            "mailbox login rejection after LOGIN credential exchange",
            Some("NO"),
            Some("server_denied"),
            Some("access_denied"),
        ),
        _ => None,
    }
}

pub(super) fn kerberos_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    match entry {
        "as-error" => failure(
            "initial Kerberos authentication exchange failed with explicit KRB-ERROR",
            Some("KRB-ERROR"),
            Some("semantic_error"),
            Some("protocol_error"),
        ),
        _ => None,
    }
}

pub(super) fn ldap_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    match entry {
        "bind-denied" => failure(
            "directory credential or bind-policy rejection during auth establishment",
            None,
            Some("server_denied"),
            Some("access_denied"),
        ),
        "denied" => failure(
            "directory write refusal during LDAP modify result evaluation",
            Some("modifyResponse"),
            Some("server_denied"),
            Some("access_denied"),
        ),
        "constraint" => failure(
            "directory constraint violation during LDAP modify result evaluation",
            Some("modifyResponse"),
            Some("semantic_error"),
            Some("protocol_constraint_violation"),
        ),
        _ => None,
    }
}

pub(super) fn pop3_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    match entry {
        "auth-denied" => failure(
            "mailbox password rejection after USER/PASS credential exchange",
            Some("-ERR"),
            Some("server_denied"),
            Some("access_denied"),
        ),
        _ => None,
    }
}

pub(super) fn radius_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    match entry {
        "challenge" => summary(
            "continuation-path",
            "identity challenge continuation during RADIUS Access-Challenge evaluation",
            Some("Access-Challenge"),
            None,
            None,
            None,
        ),
        "denied" => failure(
            "identity access rejection during RADIUS Access-Reject evaluation",
            Some("Access-Reject"),
            Some("server_denied"),
            Some("access_denied"),
        ),
        _ => None,
    }
}

pub(super) fn snmp_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    match entry {
        "unauthorized" => failure(
            "authorization failure report after SNMPv3 access evaluation",
            None,
            Some("server_denied"),
            Some("access_denied"),
        ),
        _ => None,
    }
}

pub(super) fn smtp_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    match entry {
        "auth-denied" => failure(
            "smtp authentication rejection after explicit AUTH exchange",
            Some("535"),
            Some("server_denied"),
            Some("access_denied"),
        ),
        "rcpt-denied" => failure(
            "recipient rejection during SMTP envelope construction",
            Some("550"),
            Some("server_denied"),
            Some("access_denied"),
        ),
        "data-denied" => failure(
            "message rejection after SMTP body handoff",
            Some("550"),
            Some("server_denied"),
            Some("access_denied"),
        ),
        _ => None,
    }
}

pub(super) fn ssh_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    match entry {
        "auth-denied" => failure(
            "ssh authentication rejection after explicit auth request",
            None,
            Some("server_denied"),
            Some("access_denied"),
        ),
        _ => None,
    }
}
