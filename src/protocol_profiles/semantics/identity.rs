use super::common::{failure, summary};
use crate::protocol_profiles::ProtocolEntrySemanticsSummary;

pub(super) fn amqp_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    match entry {
        "start" => summary(
            "amqp-start-path",
            "AMQP protocol header and connection.start negotiation with the broker",
            Some("AMQP header + connection.start"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        "session" => summary(
            "amqp-session-path",
            "AMQP connection and channel setup before message transfer",
            Some("connection.open + channel.open"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        "publish" => summary(
            "amqp-publish-path",
            "AMQP basic.publish flow acknowledged by the broker",
            Some("basic.publish + basic.ack"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        "consume" => summary(
            "amqp-consume-path",
            "AMQP basic.consume request followed by message delivery",
            Some("basic.consume + basic.deliver"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
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
        "session" => summary(
            "ftp-session-path",
            "FTP control session login with banner, USER, PASS, and success response",
            Some("220/331/230 control replies"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        "denied" => failure(
            "ftp login rejection after USER/PASS credential exchange",
            Some("530"),
            Some("server_denied"),
            Some("access_denied"),
        ),
        "list" => summary(
            "ftp-passive-directory-path",
            "FTP passive-mode directory listing after PASV and LIST transfer setup",
            Some("PASV + LIST + 150/226"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        "retr" => summary(
            "ftp-passive-download-path",
            "FTP passive-mode file retrieval after PASV and RETR transfer setup",
            Some("PASV + RETR + 150/226"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        "stor" => summary(
            "ftp-passive-upload-path",
            "FTP passive-mode file upload after PASV and STOR transfer setup",
            Some("PASV + STOR + 150/226"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        "active-list" => summary(
            "ftp-active-directory-path",
            "FTP active-mode directory listing after PORT and LIST transfer setup",
            Some("PORT + LIST + 150/226"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        "active-retr" => summary(
            "ftp-active-download-path",
            "FTP active-mode file retrieval after PORT and RETR transfer setup",
            Some("PORT + RETR + 150/226"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        "active-stor" => summary(
            "ftp-active-upload-path",
            "FTP active-mode file upload after PORT and STOR transfer setup",
            Some("PORT + STOR + 150/226"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        _ => None,
    }
}

pub(super) fn imap_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    match entry {
        "auth" => summary(
            "mailbox-auth-path",
            "IMAP server greeting followed by LOGIN and tagged OK response",
            Some("A001 LOGIN + A001 OK"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        "auth-denied" => failure(
            "mailbox login rejection after LOGIN credential exchange",
            Some("NO"),
            Some("server_denied"),
            Some("access_denied"),
        ),
        "select" => summary(
            "mailbox-select-path",
            "IMAP authenticated session selecting a mailbox for message access",
            Some("A002 SELECT + A002 OK"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        _ => None,
    }
}

pub(super) fn kerberos_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    match entry {
        "as" => summary(
            "kerberos-as-path",
            "Kerberos AS-REQ and AS-REP exchange issuing an initial ticket-granting ticket",
            Some("AS-REQ + AS-REP"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        "tgs" => summary(
            "kerberos-tgs-path",
            "Kerberos TGS-REQ and TGS-REP exchange issuing a service ticket",
            Some("TGS-REQ + TGS-REP"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
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
        "bind" => summary(
            "directory-bind-path",
            "LDAP bind request accepted by the directory server",
            Some("bindResponse success"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        "search" => summary(
            "directory-search-path",
            "LDAP search request followed by search result entries or completion",
            Some("searchRequest + searchResult"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        "modify" => summary(
            "directory-modify-path",
            "LDAP modify request accepted by the directory server",
            Some("modifyResponse success"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        "session" => summary(
            "directory-session-path",
            "LDAP bind followed by directory search response flow",
            Some("bindResponse + searchResult"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        "write" => summary(
            "directory-write-path",
            "LDAP bind, search, and modify response flow for directory writes",
            Some("modifyResponse success"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        "sync" => summary(
            "directory-sync-path",
            "LDAP bind, search, and modify response flow used as a directory synchronization probe",
            Some("bind/search/modify response chain"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
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
        "auth" => summary(
            "mailbox-auth-path",
            "POP3 greeting followed by USER/PASS and positive mailbox response",
            Some("+OK USER/PASS"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        "auth-denied" => failure(
            "mailbox password rejection after USER/PASS credential exchange",
            Some("-ERR"),
            Some("server_denied"),
            Some("access_denied"),
        ),
        "list" => summary(
            "mailbox-list-path",
            "POP3 authenticated session issuing LIST and receiving mailbox message metadata",
            Some("LIST + +OK message list"),
            None,
            None,
            Some("protocol_entry_signal"),
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
        "get" => summary(
            "snmp-read-path",
            "SNMP GET request and response reading one or more managed object values",
            Some("GetRequest PDU + Response PDU"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        "get-next" => summary(
            "snmp-walk-path",
            "SNMP GETNEXT request and response walking adjacent managed object values",
            Some("GetNextRequest PDU + Response PDU"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        "bulk" => summary(
            "snmp-bulk-read-path",
            "SNMP GETBULK request and response collecting a larger management table slice",
            Some("GetBulkRequest PDU + Response PDU"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        "set" => summary(
            "snmp-write-path",
            "SNMP SET request and response changing a managed object value",
            Some("SetRequest PDU + Response PDU"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        "trap" => summary(
            "snmp-notification-path",
            "one-way SNMP trap notification emitted from an agent toward a manager",
            Some("Trap PDU"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        "inform" => summary(
            "snmp-acknowledged-notification-path",
            "SNMP inform notification followed by manager acknowledgement",
            Some("InformRequest PDU + Response PDU"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        "engine-sync" => summary(
            "snmp-engine-sync-path",
            "SNMPv3 engine identifier and time synchronization before authenticated access",
            Some("SNMPv3 discovery/report exchange"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        "trap-recv" => summary(
            "snmp-notification-receive-path",
            "SNMP trap notification received by a local management listener",
            Some("Trap PDU ingress"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        "report" => summary(
            "snmp-report-path",
            "SNMP report response carrying protocol status or engine discovery data",
            Some("Report PDU"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        "unauthorized" => failure(
            "authorization failure report after SNMPv3 access evaluation",
            None,
            Some("server_denied"),
            Some("access_denied"),
        ),
        "v3-auth" => summary(
            "snmpv3-authenticated-path",
            "SNMPv3 authenticated message exchange with authNoPriv security",
            Some("SNMPv3 msgFlags auth bit"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        "v3-priv" => summary(
            "snmpv3-private-path",
            "SNMPv3 authenticated and encrypted message exchange with authPriv security",
            Some("SNMPv3 msgFlags auth+priv bits"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        _ => None,
    }
}

pub(super) fn smtp_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    match entry {
        "session" => summary(
            "mail-submit-session-path",
            "SMTP server banner and EHLO capability negotiation",
            Some("220 banner + EHLO"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        "auth" => summary(
            "mail-submit-auth-path",
            "SMTP AUTH request accepted after server greeting and EHLO negotiation",
            Some("AUTH + 235"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        "auth-denied" => failure(
            "smtp authentication rejection after explicit AUTH exchange",
            Some("535"),
            Some("server_denied"),
            Some("access_denied"),
        ),
        "mail" => summary(
            "mail-envelope-sender-path",
            "SMTP MAIL FROM accepted after authenticated session setup",
            Some("MAIL FROM + 250 2.1.x"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        "rcpt" => summary(
            "mail-envelope-recipient-path",
            "SMTP RCPT TO accepted for a recipient in the delivery envelope",
            Some("RCPT TO + 250 2.1.5"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        "rcpt-denied" => failure(
            "recipient rejection during SMTP envelope construction",
            Some("550"),
            Some("server_denied"),
            Some("access_denied"),
        ),
        "data" => summary(
            "mail-data-submit-path",
            "SMTP DATA handoff accepted and message body queued by the server",
            Some("DATA + 354 + 250 2.0.0"),
            None,
            None,
            Some("protocol_entry_signal"),
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
        "session" => summary(
            "ssh-session-path",
            "SSH transport banner and key exchange setup for an interactive connection",
            Some("SSH banner + KEXINIT"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        "auth" => summary(
            "ssh-auth-path",
            "SSH user authentication request followed by explicit success",
            Some("SSH_MSG_USERAUTH_SUCCESS"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        "auth-denied" => failure(
            "ssh authentication rejection after explicit auth request",
            None,
            Some("server_denied"),
            Some("access_denied"),
        ),
        "channel" => summary(
            "interactive-channel-path",
            "SSH session channel open after banner, key exchange, and authentication",
            Some("SSH_MSG_CHANNEL_OPEN_CONFIRMATION"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        _ => None,
    }
}

pub(super) fn smb_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    let (category, operator_focus, typical_signal) = match entry {
        "negotiate" => (
            "file-share-negotiate-path",
            "SMB2 dialect negotiation on TCP port 445 before authentication or tree access",
            Some("SMB2 NEGOTIATE"),
        ),
        "session" => (
            "file-share-session-path",
            "SMB2 session setup for user or machine access to a file share endpoint",
            Some("SMB2 SESSION_SETUP"),
        ),
        "tree" => (
            "file-share-tree-path",
            "SMB2 tree connect selecting the concrete share path after session setup",
            Some("SMB2 TREE_CONNECT"),
        ),
        _ => return None,
    };
    summary(category, operator_focus, typical_signal, None, None, None)
}
