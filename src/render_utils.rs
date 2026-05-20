use gewyvern::flow::{ModuleSeverity, ProcessView, ProgramFindingCause, ProgramOperation};
use gewyvern::http::{HttpComponentKind, HttpSuspectSide, HttpTransactionVerdict};

pub(crate) fn html_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
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
            target.push_str(",\"comm\":\"");
            target.push_str(&process.comm);
            target.push_str("\"}");
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
        target.push('"');
        target.push_str(item);
        target.push('"');
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
