use super::ProtocolEntrySemanticsSummary;

pub(super) fn built_in_protocol_entry_semantics(
    protocol: &str,
    entry: &str,
) -> Option<ProtocolEntrySemanticsSummary> {
    match protocol {
        "ftp" => ftp_entry_semantics(entry),
        "http" => http_entry_semantics(entry),
        "imap" => imap_entry_semantics(entry),
        "kerberos" => kerberos_entry_semantics(entry),
        "ldap" => ldap_entry_semantics(entry),
        "pop3" => pop3_entry_semantics(entry),
        "redis" => redis_entry_semantics(entry),
        "snmp" => snmp_entry_semantics(entry),
        "socks5" => socks5_entry_semantics(entry),
        "smtp" => smtp_entry_semantics(entry),
        "stun" => stun_entry_semantics(entry),
        "ssh" => ssh_entry_semantics(entry),
        _ => None,
    }
}

fn ftp_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    let (operator_focus, typical_signal, failure_mode, failure_detail, failure_basis) = match entry
    {
        "denied" => (
            "ftp login rejection after USER/PASS credential exchange",
            Some("530"),
            Some("server_denied"),
            Some("access_denied"),
            Some("direct_protocol_signal"),
        ),
        _ => return None,
    };
    Some(ProtocolEntrySemanticsSummary {
        category: "failure-path".into(),
        operator_focus: operator_focus.into(),
        typical_signal: typical_signal.map(str::to_string),
        primary_failure_mode: failure_mode.map(str::to_string),
        primary_failure_detail: failure_detail.map(str::to_string),
        primary_failure_basis: failure_basis.map(str::to_string),
    })
}

fn imap_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    let (operator_focus, typical_signal, failure_mode, failure_detail, failure_basis) = match entry
    {
        "auth-denied" => (
            "mailbox login rejection after LOGIN credential exchange",
            Some("NO"),
            Some("server_denied"),
            Some("access_denied"),
            Some("direct_protocol_signal"),
        ),
        _ => return None,
    };
    Some(ProtocolEntrySemanticsSummary {
        category: "failure-path".into(),
        operator_focus: operator_focus.into(),
        typical_signal: typical_signal.map(str::to_string),
        primary_failure_mode: failure_mode.map(str::to_string),
        primary_failure_detail: failure_detail.map(str::to_string),
        primary_failure_basis: failure_basis.map(str::to_string),
    })
}

fn http_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    let (operator_focus, typical_signal, failure_mode, failure_detail, failure_basis) = match entry
    {
        "denied" => (
            "proxy tunnel refusal after CONNECT policy evaluation",
            Some("403"),
            Some("server_denied"),
            Some("access_denied"),
            Some("direct_protocol_signal"),
        ),
        _ => return None,
    };
    Some(ProtocolEntrySemanticsSummary {
        category: "failure-path".into(),
        operator_focus: operator_focus.into(),
        typical_signal: typical_signal.map(str::to_string),
        primary_failure_mode: failure_mode.map(str::to_string),
        primary_failure_detail: failure_detail.map(str::to_string),
        primary_failure_basis: failure_basis.map(str::to_string),
    })
}

fn ldap_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    let (operator_focus, failure_mode, failure_detail, failure_basis) = match entry {
        "bind-denied" => (
            "directory credential or bind-policy rejection during auth establishment",
            Some("server_denied"),
            Some("access_denied"),
            Some("direct_protocol_signal"),
        ),
        _ => return None,
    };
    Some(ProtocolEntrySemanticsSummary {
        category: "failure-path".into(),
        operator_focus: operator_focus.into(),
        typical_signal: None,
        primary_failure_mode: failure_mode.map(str::to_string),
        primary_failure_detail: failure_detail.map(str::to_string),
        primary_failure_basis: failure_basis.map(str::to_string),
    })
}

fn kerberos_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    let (operator_focus, typical_signal, failure_mode, failure_detail, failure_basis) = match entry
    {
        "as-error" => (
            "initial Kerberos authentication exchange failed with explicit KRB-ERROR",
            Some("KRB-ERROR"),
            Some("semantic_error"),
            Some("protocol_error"),
            Some("direct_protocol_signal"),
        ),
        _ => return None,
    };
    Some(ProtocolEntrySemanticsSummary {
        category: "failure-path".into(),
        operator_focus: operator_focus.into(),
        typical_signal: typical_signal.map(str::to_string),
        primary_failure_mode: failure_mode.map(str::to_string),
        primary_failure_detail: failure_detail.map(str::to_string),
        primary_failure_basis: failure_basis.map(str::to_string),
    })
}

fn pop3_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    let (operator_focus, typical_signal, failure_mode, failure_detail, failure_basis) = match entry
    {
        "auth-denied" => (
            "mailbox password rejection after USER/PASS credential exchange",
            Some("-ERR"),
            Some("server_denied"),
            Some("access_denied"),
            Some("direct_protocol_signal"),
        ),
        _ => return None,
    };
    Some(ProtocolEntrySemanticsSummary {
        category: "failure-path".into(),
        operator_focus: operator_focus.into(),
        typical_signal: typical_signal.map(str::to_string),
        primary_failure_mode: failure_mode.map(str::to_string),
        primary_failure_detail: failure_detail.map(str::to_string),
        primary_failure_basis: failure_basis.map(str::to_string),
    })
}

fn snmp_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    let (operator_focus, typical_signal, failure_mode, failure_detail, failure_basis) = match entry
    {
        "unauthorized" => (
            "authorization failure report after SNMPv3 access evaluation",
            None,
            Some("server_denied"),
            Some("access_denied"),
            Some("direct_protocol_signal"),
        ),
        _ => return None,
    };
    Some(ProtocolEntrySemanticsSummary {
        category: "failure-path".into(),
        operator_focus: operator_focus.into(),
        typical_signal: typical_signal.map(str::to_string),
        primary_failure_mode: failure_mode.map(str::to_string),
        primary_failure_detail: failure_detail.map(str::to_string),
        primary_failure_basis: failure_basis.map(str::to_string),
    })
}

fn smtp_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    let (operator_focus, typical_signal, failure_mode, failure_detail, failure_basis) = match entry
    {
        "auth-denied" => (
            "smtp authentication rejection after explicit AUTH exchange",
            Some("535"),
            Some("server_denied"),
            Some("access_denied"),
            Some("direct_protocol_signal"),
        ),
        "rcpt-denied" => (
            "recipient rejection during SMTP envelope construction",
            Some("550"),
            Some("server_denied"),
            Some("access_denied"),
            Some("direct_protocol_signal"),
        ),
        "data-denied" => (
            "message rejection after SMTP body handoff",
            Some("550"),
            Some("server_denied"),
            Some("access_denied"),
            Some("direct_protocol_signal"),
        ),
        _ => return None,
    };
    Some(ProtocolEntrySemanticsSummary {
        category: "failure-path".into(),
        operator_focus: operator_focus.into(),
        typical_signal: typical_signal.map(str::to_string),
        primary_failure_mode: failure_mode.map(str::to_string),
        primary_failure_detail: failure_detail.map(str::to_string),
        primary_failure_basis: failure_basis.map(str::to_string),
    })
}

fn socks5_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    let (operator_focus, failure_mode, failure_detail, failure_basis) = match entry {
        "denied" => (
            "upstream connect refusal after no-auth method selection",
            Some("server_denied"),
            Some("access_denied"),
            Some("direct_protocol_signal"),
        ),
        "auth-denied" => (
            "username/password rejection during proxy auth exchange",
            Some("server_denied"),
            Some("access_denied"),
            Some("direct_protocol_signal"),
        ),
        "auth-connect-denied" => (
            "upstream connect refusal after authenticated proxy setup",
            Some("server_denied"),
            Some("access_denied"),
            Some("direct_protocol_signal"),
        ),
        _ => return None,
    };
    Some(ProtocolEntrySemanticsSummary {
        category: "failure-path".into(),
        operator_focus: operator_focus.into(),
        typical_signal: None,
        primary_failure_mode: failure_mode.map(str::to_string),
        primary_failure_detail: failure_detail.map(str::to_string),
        primary_failure_basis: failure_basis.map(str::to_string),
    })
}

fn stun_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    let (operator_focus, failure_mode, failure_detail, failure_basis) = match entry {
        "binding-error" => (
            "explicit binding failure response instead of successful reachability confirmation",
            Some("server_denied"),
            Some("access_denied"),
            Some("direct_protocol_signal"),
        ),
        _ => return None,
    };
    Some(ProtocolEntrySemanticsSummary {
        category: "failure-path".into(),
        operator_focus: operator_focus.into(),
        typical_signal: None,
        primary_failure_mode: failure_mode.map(str::to_string),
        primary_failure_detail: failure_detail.map(str::to_string),
        primary_failure_basis: failure_basis.map(str::to_string),
    })
}

fn ssh_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    let (operator_focus, typical_signal, failure_mode, failure_detail, failure_basis) = match entry
    {
        "auth-denied" => (
            "ssh authentication rejection after explicit auth request",
            None,
            Some("server_denied"),
            Some("access_denied"),
            Some("direct_protocol_signal"),
        ),
        _ => return None,
    };
    Some(ProtocolEntrySemanticsSummary {
        category: "failure-path".into(),
        operator_focus: operator_focus.into(),
        typical_signal: typical_signal.map(str::to_string),
        primary_failure_mode: failure_mode.map(str::to_string),
        primary_failure_detail: failure_detail.map(str::to_string),
        primary_failure_basis: failure_basis.map(str::to_string),
    })
}

fn redis_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    let (operator_focus, typical_signal, failure_mode, failure_detail, failure_basis) = match entry
    {
        "auth-required" => (
            "authentication gate before command execution",
            Some("-NOAUTH"),
            Some("server_denied"),
            Some("auth_required"),
            Some("direct_protocol_signal"),
        ),
        "auth-denied" => (
            "credential rejection after AUTH",
            Some("-WRONGPASS"),
            Some("server_denied"),
            Some("access_denied"),
            Some("direct_protocol_signal"),
        ),
        "error" => (
            "generic command or request semantic failure",
            Some("-ERR"),
            Some("semantic_error"),
            Some("protocol_error"),
            Some("direct_protocol_signal"),
        ),
        "wrongtype" => (
            "key-type mismatch against requested command",
            Some("-WRONGTYPE"),
            Some("semantic_error"),
            Some("protocol_constraint_violation"),
            Some("direct_protocol_signal"),
        ),
        "busygroup" => (
            "stream consumer-group creation conflict",
            Some("-BUSYGROUP"),
            Some("semantic_error"),
            Some("protocol_error"),
            Some("direct_protocol_signal"),
        ),
        "readonly" => (
            "replica write refusal or readonly placement mismatch",
            Some("-READONLY"),
            Some("server_denied"),
            Some("access_denied"),
            Some("direct_protocol_signal"),
        ),
        "noscript" => (
            "server script cache miss before EVALSHA-style reuse",
            Some("-NOSCRIPT"),
            Some("semantic_error"),
            Some("protocol_error"),
            Some("direct_protocol_signal"),
        ),
        "moved" => (
            "cluster slot redirect that requires target remap",
            Some("-MOVED"),
            Some("semantic_error"),
            Some("protocol_error"),
            Some("direct_protocol_signal"),
        ),
        "ask" => (
            "temporary cluster redirect that expects ASKING on retry",
            Some("-ASK"),
            Some("semantic_error"),
            Some("protocol_error"),
            Some("direct_protocol_signal"),
        ),
        "tryagain" => (
            "retry-needed transient cluster or script window",
            Some("-TRYAGAIN"),
            Some("semantic_error"),
            Some("protocol_error"),
            Some("direct_protocol_signal"),
        ),
        "loading" => (
            "server warmup window before command acceptance",
            Some("-LOADING"),
            Some("semantic_error"),
            Some("protocol_error"),
            Some("direct_protocol_signal"),
        ),
        "crossslot" => (
            "multi-key cluster slot mismatch",
            Some("-CROSSSLOT"),
            Some("semantic_error"),
            Some("protocol_error"),
            Some("direct_protocol_signal"),
        ),
        "clusterdown" => (
            "cluster topology unavailable for routing",
            Some("-CLUSTERDOWN"),
            Some("semantic_error"),
            Some("protocol_error"),
            Some("direct_protocol_signal"),
        ),
        "masterdown" => (
            "primary unavailable during failover window",
            Some("-MASTERDOWN"),
            Some("semantic_error"),
            Some("protocol_error"),
            Some("direct_protocol_signal"),
        ),
        "oom" => (
            "memory policy refusal for write amplification",
            Some("-OOM"),
            Some("semantic_error"),
            Some("protocol_error"),
            Some("direct_protocol_signal"),
        ),
        "busy" => (
            "Lua or long-running server-side script contention",
            Some("-BUSY"),
            Some("semantic_error"),
            Some("protocol_error"),
            Some("direct_protocol_signal"),
        ),
        "execabort" => (
            "transaction abort before EXEC completion",
            Some("-EXECABORT"),
            Some("semantic_error"),
            Some("protocol_error"),
            Some("direct_protocol_signal"),
        ),
        "misconf" => (
            "persistence or write-guard policy rejection",
            Some("-MISCONF"),
            Some("semantic_error"),
            Some("protocol_error"),
            Some("direct_protocol_signal"),
        ),
        _ => return None,
    };
    Some(ProtocolEntrySemanticsSummary {
        category: "failure-path".into(),
        operator_focus: operator_focus.into(),
        typical_signal: typical_signal.map(str::to_string),
        primary_failure_mode: failure_mode.map(str::to_string),
        primary_failure_detail: failure_detail.map(str::to_string),
        primary_failure_basis: failure_basis.map(str::to_string),
    })
}
