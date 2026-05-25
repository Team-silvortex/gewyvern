use gewyvern::flow::{ModuleSeverity, ProcessView, ProgramFindingCause, ProgramOperation};
use gewyvern::http::{HttpComponentKind, HttpSuspectSide, HttpTransactionVerdict};

pub(crate) fn html_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub(crate) fn append_json_string(target: &mut String, value: &str) {
    target.push('"');
    for ch in value.chars() {
        match ch {
            '\\' => target.push_str("\\\\"),
            '"' => target.push_str("\\\""),
            '\n' => target.push_str("\\n"),
            '\r' => target.push_str("\\r"),
            '\t' => target.push_str("\\t"),
            '\u{08}' => target.push_str("\\b"),
            '\u{0C}' => target.push_str("\\f"),
            ch if ch.is_control() => {
                use std::fmt::Write;
                let _ = write!(target, "\\u{:04x}", ch as u32);
            }
            _ => target.push(ch),
        }
    }
    target.push('"');
}

pub(crate) fn extract_json_string_field(input: &str, key: &str) -> Option<String> {
    let needle = format!("\"{}\":", key);
    let start = input.find(&needle)? + needle.len();
    let rest = input[start..].trim_start();
    let mut chars = rest.chars();
    if chars.next()? != '"' {
        return None;
    }
    let mut value = String::new();
    let mut escape = false;
    let mut unicode_remaining = 0usize;
    let mut unicode_buf = String::new();
    for ch in chars {
        if unicode_remaining > 0 {
            unicode_buf.push(ch);
            unicode_remaining -= 1;
            if unicode_remaining == 0 {
                if let Ok(codepoint) = u32::from_str_radix(&unicode_buf, 16) {
                    if let Some(decoded) = char::from_u32(codepoint) {
                        value.push(decoded);
                    }
                }
                unicode_buf.clear();
            }
            continue;
        }
        if escape {
            match ch {
                '"' => value.push('"'),
                '\\' => value.push('\\'),
                '/' => value.push('/'),
                'b' => value.push('\u{08}'),
                'f' => value.push('\u{0C}'),
                'n' => value.push('\n'),
                'r' => value.push('\r'),
                't' => value.push('\t'),
                'u' => unicode_remaining = 4,
                other => value.push(other),
            }
            escape = false;
            continue;
        }
        match ch {
            '\\' => escape = true,
            '"' => return Some(value),
            other => value.push(other),
        }
    }
    None
}

pub(crate) fn http_component_kind_label(kind: &HttpComponentKind) -> &'static str {
    match kind {
        HttpComponentKind::DnsLookup => "dns",
        HttpComponentKind::ClientRequest => "client",
        HttpComponentKind::ServerResponse => "server",
    }
}

pub(crate) fn http_suspect_side_label(side: &HttpSuspectSide) -> &'static str {
    match side {
        HttpSuspectSide::Dns => "dns",
        HttpSuspectSide::Client => "client",
        HttpSuspectSide::Server => "server",
    }
}

pub(crate) fn http_transaction_verdict_label(verdict: &HttpTransactionVerdict) -> &'static str {
    match verdict {
        HttpTransactionVerdict::HealthyRequestResponsePath => "healthy_request_response_path",
        HttpTransactionVerdict::SuspectDnsResolutionGap => "suspect_dns_resolution_gap",
        HttpTransactionVerdict::SuspectClientResponseGap => "suspect_client_response_gap",
        HttpTransactionVerdict::SuspectServerResponseGap => "suspect_server_response_gap",
        HttpTransactionVerdict::SuspectMultiSidedGap => "suspect_multi_sided_gap",
    }
}

pub(crate) fn append_process_json(target: &mut String, process: Option<&ProcessView>) {
    match process {
        Some(process) => {
            target.push_str("{\"pid\":");
            target.push_str(&process.pid.to_string());
            target.push_str(",\"tid\":");
            target.push_str(&process.tid.to_string());
            target.push_str(",\"cgroup_id\":");
            target.push_str(&process.cgroup_id.to_string());
            target.push_str(",\"comm\":");
            append_json_string(target, &process.comm);
            target.push('}');
        }
        None => target.push_str("null"),
    }
}

pub(crate) fn string_list_json(items: &[String]) -> String {
    let mut json = String::new();
    append_string_list_json(&mut json, items);
    json
}

pub(crate) fn append_string_list_json(target: &mut String, items: &[String]) {
    target.push('[');
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            target.push(',');
        }
        append_json_string(target, item);
    }
    target.push(']');
}

pub(crate) fn push_joined_strings(target: &mut String, items: &[String], separator: &str) {
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            target.push_str(separator);
        }
        target.push_str(item);
    }
}

pub(crate) fn operation_label(operation: &ProgramOperation) -> String {
    let mut label = String::new();
    append_operation_label(&mut label, operation);
    label
}

pub(crate) fn append_operation_label(target: &mut String, operation: &ProgramOperation) {
    match operation {
        ProgramOperation::ConnectFlow => target.push_str("connect_flow"),
        ProgramOperation::DatagramExchange => target.push_str("datagram_exchange"),
        ProgramOperation::Custom(value) => target.push_str(value),
        ProgramOperation::Unknown => target.push_str("unknown"),
    }
}

pub(crate) fn finding_cause_label(cause: &ProgramFindingCause) -> &'static str {
    match cause {
        ProgramFindingCause::AttachFailure => "attach_failure",
        ProgramFindingCause::RejectedEvidence => "rejected_evidence",
        ProgramFindingCause::MissingCoreStage => "missing_core_stage",
    }
}

pub(crate) fn module_severity_label(severity: &ModuleSeverity) -> &'static str {
    match severity {
        ModuleSeverity::High => "high",
        ModuleSeverity::Medium => "medium",
        ModuleSeverity::Low => "low",
    }
}
