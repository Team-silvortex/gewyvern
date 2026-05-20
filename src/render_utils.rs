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

pub(crate) fn process_json(process: Option<&ProcessView>) -> String {
    match process {
        Some(process) => format!(
            "{{\"pid\":{},\"tid\":{},\"cgroup_id\":{},\"comm\":\"{}\"}}",
            process.pid, process.tid, process.cgroup_id, process.comm
        ),
        None => "null".into(),
    }
}

pub(crate) fn string_list_json(items: &[String]) -> String {
    let mut json = String::from("[");
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        json.push('"');
        json.push_str(item);
        json.push('"');
    }
    json.push(']');
    json
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
    match operation {
        ProgramOperation::ConnectFlow => "connect_flow".into(),
        ProgramOperation::DatagramExchange => "datagram_exchange".into(),
        ProgramOperation::Custom(value) => value.clone(),
        ProgramOperation::Unknown => "unknown".into(),
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
