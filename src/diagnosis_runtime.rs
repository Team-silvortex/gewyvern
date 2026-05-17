use gewyvern::export::ExportBundle;
use gewyvern::flow::ProgramFlowId;
use std::collections::HashMap;

use super::*;
use crate::render_utils::*;

#[derive(Default)]
pub(super) struct ProtocolFlowFindingSummary {
    pub(super) has_findings: bool,
    pub(super) missing_transitions: Vec<String>,
    pub(super) network_module_kinds: Vec<String>,
    pub(super) suspect_areas: Vec<String>,
}

#[derive(Clone, Default)]
pub(super) struct ProcessNetworkProfileSummary {
    pub(super) pid: u32,
    pub(super) comm: String,
    pub(super) status: String,
    pub(super) primary_module_kind: String,
    pub(super) primary_module_family: String,
    pub(super) primary_failure_stage: String,
    pub(super) primary_stage_family: String,
    pub(super) primary_failure_mode: String,
    pub(super) primary_failure_detail: String,
    pub(super) primary_failure_confidence: String,
    pub(super) primary_failure_basis: String,
    pub(super) ambiguous: bool,
    pub(super) competing_hypotheses: Vec<String>,
    pub(super) operations: Vec<String>,
    pub(super) module_kinds: Vec<String>,
    pub(super) phases: Vec<String>,
    pub(super) missing_transitions: Vec<String>,
    pub(super) suspect_areas: Vec<String>,
    pub(super) suspect_modules: Vec<String>,
    pub(super) healthy_flows: usize,
    pub(super) attention_flows: usize,
}

pub(super) fn first_or_none(items: &[String]) -> String {
    items.first().cloned().unwrap_or_else(|| "none".into())
}

pub(super) fn first_non_none(items: &[String]) -> Option<String> {
    items.iter().find(|item| item.as_str() != "none").cloned()
}

pub(super) fn bump_score(
    score_map: &mut HashMap<(u32, String), HashMap<String, u32>>,
    key: &(u32, String),
    value: &str,
    weight: u32,
) {
    if value == "none" {
        return;
    }
    *score_map
        .entry(key.clone())
        .or_default()
        .entry(value.to_string())
        .or_default() += weight;
}

pub(super) fn best_scored_value(
    score_map: &HashMap<(u32, String), HashMap<String, u32>>,
    key: &(u32, String),
) -> Option<String> {
    score_map.get(key).and_then(|scores| {
        scores
            .iter()
            .max_by(|(left_value, left_score), (right_value, right_score)| {
                left_score
                    .cmp(right_score)
                    .then_with(|| right_value.cmp(left_value))
            })
            .map(|(value, _)| value.clone())
    })
}

pub(super) fn module_family_label(module_kind: &str) -> &'static str {
    let lowered = module_kind.to_ascii_lowercase();
    if lowered.contains("dns") || lowered.contains("name_resolution") {
        "dns"
    } else if lowered.contains("route") {
        "route"
    } else if lowered.contains("connect") {
        "connect"
    } else if lowered.contains("tls")
        || lowered.contains("quic_handshake")
        || lowered.contains("handshake")
    {
        "handshake"
    } else if lowered.contains("http")
        || lowered.contains("request_response")
        || lowered.contains("signaling")
    {
        "request-response"
    } else if lowered.contains("database") {
        "database"
    } else if lowered.contains("proxy_authentication") {
        "auth"
    } else if lowered.contains("directory") {
        "directory"
    } else if lowered.contains("mail")
        || lowered.contains("messaging")
        || lowered.contains("publish")
    {
        "messaging"
    } else if lowered.contains("relay") || lowered.contains("tunnel") {
        "relay"
    } else if lowered == "authentication_exchange"
        || lowered == "remote_access_authentication"
        || lowered == "ticket_granting"
    {
        "auth"
    } else if lowered.contains("service") || lowered.contains("discovery") {
        "service"
    } else {
        "general"
    }
}

pub(super) fn stage_family_label(stage: &str) -> &'static str {
    let lowered = stage.to_ascii_lowercase();
    if lowered.contains("dns") || lowered.contains("resolve") {
        "dns"
    } else if lowered.contains("connect") || lowered.contains("establish") {
        "connect"
    } else if lowered.contains("tls")
        || lowered.contains("hello")
        || lowered.contains("crypto")
        || lowered.contains("handshake")
        || lowered.contains("banner")
        || lowered.contains("key_exchange")
        || lowered.contains("kex")
    {
        "handshake"
    } else if lowered.contains("request")
        || lowered.contains("response")
        || lowered.contains("query")
        || lowered.contains("publish")
        || lowered.contains("relay")
        || lowered.contains("stream")
        || lowered.contains("channel")
        || lowered.contains("options")
        || lowered.contains("describe")
        || lowered.contains("setup")
        || lowered.contains("select")
        || lowered.contains("list")
        || lowered.contains("mail")
        || lowered.contains("rcpt")
        || lowered.contains("data")
        || lowered.contains("message")
    {
        "request-response"
    } else if lowered.contains("auth") || lowered.contains("password") || lowered.contains("user") {
        "auth"
    } else if lowered == "none" {
        "none"
    } else {
        "general"
    }
}

pub(super) fn failure_mode_label(
    status: &str,
    module_kind: &str,
    primary_stage: &str,
    suspect_areas: &[String],
) -> &'static str {
    if status != "attention" {
        return "none";
    }

    let stage = primary_stage.to_ascii_lowercase();
    let module = module_kind.to_ascii_lowercase();

    if stage.contains("denied") || stage.contains("auth_required") {
        return "server_denied";
    }
    if stage.contains("constraint") || stage.contains("error") || module.contains("error") {
        return "semantic_error";
    }
    if stage.contains("close") {
        return "peer_closed";
    }
    if let Some((left, right)) = stage.split_once("->") {
        if left.starts_with("send")
            && (left.contains("request")
                || left.contains("query")
                || left.contains("publish")
                || left.contains("auth")
                || left.contains("password")
                || left.contains("options")
                || left.contains("describe")
                || left.contains("setup")
                || left.contains("select")
                || left.contains("port")
                || left.contains("pasv")
                || left.contains("list")
                || left.contains("mail")
                || left.contains("rcpt")
                || left.contains("data")
                || left.contains("message")
                || left.contains("relay")
                || left.contains("stream")
                || left.contains("channel")
                || left.contains("greeting"))
            && (right.starts_with("receive")
                || right.contains("response")
                || right.contains("result")
                || right.contains("ack")
                || right.contains("accept")
                || right.contains("confirmation")
                || right.contains("offer")
                || right.contains("ready")
                || right.contains("selected")
                || right.contains("mailbox")
                || right.contains("transfer")
                || right.contains("ok")
                || right.contains("success")
                || right.contains("established"))
        {
            return "no_response";
        }
        if left.starts_with("receive")
            && (right.starts_with("send")
                || right.contains("request")
                || right.contains("query")
                || right.contains("publish")
                || right.contains("auth")
                || right.contains("password")
                || right.contains("options")
                || right.contains("describe")
                || right.contains("setup")
                || right.contains("select")
                || right.contains("port")
                || right.contains("pasv")
                || right.contains("list")
                || right.contains("mail")
                || right.contains("rcpt")
                || right.contains("data")
                || right.contains("message")
                || right.contains("relay")
                || right.contains("stream")
                || right.contains("channel"))
        {
            return "not_sent";
        }
        if left.starts_with("send")
            && (left.contains("banner") || left.contains("hello"))
            && (right.starts_with("send")
                || right.contains("key_exchange")
                || right.contains("kex"))
        {
            return "not_sent";
        }
    }
    if stage.contains("resolve")
        || stage.contains("dns")
        || stage.contains("connect")
        || stage.contains("establish")
        || stage.contains("handshake")
        || stage.contains("crypto")
        || stage.contains("hello")
        || stage.contains("banner")
        || stage.contains("key_exchange")
        || stage.contains("kex")
    {
        return "setup_incomplete";
    }
    if stage.starts_with("send_")
        || stage.contains("request")
        || stage.contains("query")
        || stage.contains("options")
        || stage.contains("describe")
        || stage.contains("setup")
        || stage.contains("select")
        || stage.contains("publish")
        || stage.contains("port")
        || stage.contains("list")
        || stage.contains("mail")
        || stage.contains("rcpt")
        || stage.contains("data")
        || stage.contains("message")
        || stage.contains("pasv")
        || stage.contains("relay")
        || stage.contains("stream")
        || stage.contains("channel")
    {
        return "not_sent";
    }
    if stage.starts_with("receive_")
        || stage.contains("response")
        || stage.contains("result")
        || stage.contains("confirmation")
        || stage.contains("selected")
        || stage.contains("mailbox")
        || stage.contains("transfer")
        || stage.contains("ack")
        || stage.contains("ready")
        || stage.contains("ok")
    {
        return "no_response";
    }
    if suspect_areas
        .iter()
        .any(|area| area == "route_io" || area == "transport_io")
    {
        return "no_response";
    }
    "attention"
}

pub(super) fn failure_mode_family_label(mode: &str) -> &'static str {
    match mode {
        "not_sent" => "blocked",
        "no_response" => "timeout",
        "setup_incomplete" => "setup",
        "semantic_error" => "semantic",
        "server_denied" => "denied",
        "peer_closed" => "peer",
        "none" => "none",
        _ => "general",
    }
}

pub(super) fn failure_detail_label(
    status: &str,
    module_kind: &str,
    primary_stage: &str,
    suspect_areas: &[String],
) -> &'static str {
    if status != "attention" {
        return "none";
    }

    let stage = primary_stage.to_ascii_lowercase();
    let module = module_kind.to_ascii_lowercase();

    if stage.contains("constraint") {
        return "protocol_constraint_violation";
    }
    if stage.contains("auth_required") {
        return "auth_required";
    }
    if stage.contains("denied") {
        return "access_denied";
    }
    if stage.contains("error") || module.contains("error") {
        return "protocol_error";
    }
    if stage.contains("close") {
        return "peer_closed";
    }
    if let Some((left, right)) = stage.split_once("->") {
        if left.starts_with("send")
            && (left.contains("request")
                || left.contains("query")
                || left.contains("publish")
                || left.contains("auth")
                || left.contains("password")
                || left.contains("options")
                || left.contains("describe")
                || left.contains("setup")
                || left.contains("select")
                || left.contains("port")
                || left.contains("pasv")
                || left.contains("list")
                || left.contains("mail")
                || left.contains("rcpt")
                || left.contains("data")
                || left.contains("message")
                || left.contains("relay")
                || left.contains("stream")
                || left.contains("channel")
                || left.contains("greeting"))
            && (right.starts_with("receive")
                || right.contains("response")
                || right.contains("result")
                || right.contains("ack")
                || right.contains("accept")
                || right.contains("confirmation")
                || right.contains("offer")
                || right.contains("ready")
                || right.contains("selected")
                || right.contains("mailbox")
                || right.contains("transfer")
                || right.contains("ok")
                || right.contains("success")
                || right.contains("established"))
        {
            return "request_sent_no_reply";
        }
        if left.starts_with("receive")
            && (right.starts_with("send")
                || right.contains("request")
                || right.contains("query")
                || right.contains("publish")
                || right.contains("auth")
                || right.contains("password")
                || right.contains("options")
                || right.contains("describe")
                || right.contains("setup")
                || right.contains("select")
                || right.contains("port")
                || right.contains("pasv")
                || right.contains("list")
                || right.contains("mail")
                || right.contains("rcpt")
                || right.contains("data")
                || right.contains("message")
                || right.contains("relay")
                || right.contains("stream")
                || right.contains("channel"))
        {
            return "followup_not_sent";
        }
        if left.starts_with("send")
            && (left.contains("banner") || left.contains("hello"))
            && (right.starts_with("send")
                || right.contains("key_exchange")
                || right.contains("kex"))
        {
            return "followup_not_sent";
        }
    }
    if stage.contains("resolve") || stage.contains("dns") {
        return "dns_unresolved";
    }
    if stage.contains("tls")
        || stage.contains("hello")
        || stage.contains("crypto")
        || stage.contains("handshake")
        || stage.contains("banner")
        || stage.contains("key_exchange")
        || stage.contains("kex")
    {
        return "handshake_incomplete";
    }
    if stage.contains("connect")
        || stage.contains("establish")
        || suspect_areas.iter().any(|area| area == "route_io")
    {
        return "route_or_connect_blocked";
    }
    if stage.starts_with("send_")
        || stage.contains("request")
        || stage.contains("query")
        || stage.contains("options")
        || stage.contains("describe")
        || stage.contains("setup")
        || stage.contains("select")
        || stage.contains("publish")
        || stage.contains("port")
        || stage.contains("list")
        || stage.contains("mail")
        || stage.contains("rcpt")
        || stage.contains("data")
        || stage.contains("message")
        || stage.contains("pasv")
        || stage.contains("relay")
        || stage.contains("stream")
        || stage.contains("channel")
    {
        return "request_not_sent";
    }
    if stage.starts_with("receive_")
        || stage.contains("response")
        || stage.contains("result")
        || stage.contains("confirmation")
        || stage.contains("selected")
        || stage.contains("mailbox")
        || stage.contains("transfer")
        || stage.contains("ack")
        || stage.contains("ready")
        || stage.contains("ok")
        || suspect_areas.iter().any(|area| area == "transport_io")
    {
        return "request_sent_no_reply";
    }
    "attention"
}

pub(super) fn failure_detail_family_label(detail: &str) -> &'static str {
    match detail {
        "dns_unresolved" => "dns",
        "route_or_connect_blocked" => "connect",
        "handshake_incomplete" => "handshake",
        "request_sent_no_reply" => "timeout",
        "request_not_sent" | "followup_not_sent" => "blocked",
        "protocol_error" | "protocol_constraint_violation" => "semantic",
        "access_denied" | "auth_required" => "denied",
        "peer_closed" => "peer",
        "none" => "none",
        _ => "general",
    }
}

pub(super) fn reduce_confidence_level(level: &str) -> &'static str {
    match level {
        "high" => "medium",
        "medium" => "low",
        "low" => "low",
        _ => "none",
    }
}

pub(super) fn failure_basis_label(
    status: &str,
    module_kind: &str,
    primary_stage: &str,
    suspect_areas: &[String],
) -> &'static str {
    if status != "attention" {
        return "none";
    }

    let stage = primary_stage.to_ascii_lowercase();
    let module = module_kind.to_ascii_lowercase();

    if stage.contains("denied")
        || stage.contains("auth_required")
        || stage.contains("constraint")
        || stage.contains("error")
        || stage.contains("close")
        || module.contains("error")
    {
        return "direct_protocol_signal";
    }
    if stage.contains("->") {
        return "missing_transition";
    }
    if stage.contains("resolve")
        || stage.contains("dns")
        || stage.contains("connect")
        || stage.contains("establish")
        || stage.contains("handshake")
        || stage.contains("crypto")
        || stage.contains("hello")
        || stage.contains("banner")
        || stage.contains("key_exchange")
        || stage.contains("kex")
        || suspect_areas
            .iter()
            .any(|area| area == "route_io" || area == "transport_io" || area == "socket_state")
    {
        return "phase_inference";
    }
    "heuristic_summary"
}

pub(super) fn failure_confidence_label(
    status: &str,
    module_kind: &str,
    primary_stage: &str,
    suspect_areas: &[String],
) -> &'static str {
    match failure_basis_label(status, module_kind, primary_stage, suspect_areas) {
        "direct_protocol_signal" => "high",
        "missing_transition" => "medium",
        "phase_inference" | "heuristic_summary" => "low",
        _ => "none",
    }
}

#[derive(Clone, Copy)]
pub(super) enum ScanTargetStatus {
    Idle,
    Healthy,
    Attention,
}

impl ScanTargetStatus {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Healthy => "healthy",
            Self::Attention => "attention",
        }
    }
}

pub(super) fn protocol_flow_phases(flow: &gewyvern::flow::ProgramFlow) -> Vec<String> {
    let mut phases = Vec::new();
    for phase in flow.stages.iter().filter_map(|stage| stage.phase.as_ref()) {
        if phases.last() != Some(phase) {
            phases.push(phase.clone());
        }
    }
    phases
}

pub(super) fn protocol_flow_last_phase(flow: &gewyvern::flow::ProgramFlow) -> Option<String> {
    flow.stages
        .iter()
        .rev()
        .find_map(|stage| stage.phase.clone())
}

fn terminal_failure_phase(phase: &str) -> bool {
    let phase = phase.to_ascii_lowercase();
    phase.contains("denied")
        || phase.contains("auth_required")
        || phase.contains("constraint")
        || phase.contains("error")
}

pub(super) fn protocol_flow_has_terminal_failure(flow: &gewyvern::flow::ProgramFlow) -> bool {
    protocol_flow_last_phase(flow)
        .as_deref()
        .is_some_and(terminal_failure_phase)
}

pub(super) fn protocol_flow_finding_summaries(
    export: &ExportBundle,
) -> HashMap<ProgramFlowId, ProtocolFlowFindingSummary> {
    let mut summaries = HashMap::<ProgramFlowId, ProtocolFlowFindingSummary>::new();
    for finding in &export.program_findings {
        let entry = summaries.entry(finding.program_flow).or_default();
        entry.has_findings = true;
        if let Some(transition) = &finding.phase_transition {
            if !entry.missing_transitions.contains(transition) {
                entry.missing_transitions.push(transition.clone());
            }
        }
        if !entry
            .network_module_kinds
            .contains(&finding.network_module_kind)
        {
            entry
                .network_module_kinds
                .push(finding.network_module_kind.clone());
        }
        if !entry.suspect_areas.contains(&finding.suspect_area) {
            entry.suspect_areas.push(finding.suspect_area.clone());
        }
    }
    summaries
}

fn protocol_flow_status(
    flow: &gewyvern::flow::ProgramFlow,
    finding_summary: Option<&ProtocolFlowFindingSummary>,
) -> &'static str {
    if finding_summary.is_some_and(|summary| summary.has_findings)
        || protocol_flow_has_terminal_failure(flow)
    {
        "attention"
    } else {
        "healthy"
    }
}

pub(super) fn protocol_flow_failure_mode(
    flow: &gewyvern::flow::ProgramFlow,
    finding_summary: Option<&ProtocolFlowFindingSummary>,
) -> String {
    let status = protocol_flow_status(flow, finding_summary);
    let last_phase = protocol_flow_last_phase(flow).unwrap_or_else(|| "none".into());
    let module_kind = gewyvern::flow::infer_network_module_kind(
        &flow.operation,
        Some(&last_phase),
        None,
        "network_module",
    );
    let primary_stage = finding_summary
        .and_then(|summary| summary.missing_transitions.first().cloned())
        .unwrap_or(last_phase);
    let suspect_areas = finding_summary
        .map(|summary| summary.suspect_areas.as_slice())
        .unwrap_or(&[]);
    failure_mode_label(status, module_kind, &primary_stage, suspect_areas).to_string()
}

pub(super) fn protocol_flow_failure_detail(
    flow: &gewyvern::flow::ProgramFlow,
    finding_summary: Option<&ProtocolFlowFindingSummary>,
) -> String {
    let status = protocol_flow_status(flow, finding_summary);
    let last_phase = protocol_flow_last_phase(flow).unwrap_or_else(|| "none".into());
    let module_kind = gewyvern::flow::infer_network_module_kind(
        &flow.operation,
        Some(&last_phase),
        None,
        "network_module",
    );
    let primary_stage = finding_summary
        .and_then(|summary| summary.missing_transitions.first().cloned())
        .unwrap_or(last_phase);
    let suspect_areas = finding_summary
        .map(|summary| summary.suspect_areas.as_slice())
        .unwrap_or(&[]);
    failure_detail_label(status, module_kind, &primary_stage, suspect_areas).to_string()
}

pub(super) fn protocol_flow_failure_confidence(
    flow: &gewyvern::flow::ProgramFlow,
    finding_summary: Option<&ProtocolFlowFindingSummary>,
) -> String {
    let status = protocol_flow_status(flow, finding_summary);
    let last_phase = protocol_flow_last_phase(flow).unwrap_or_else(|| "none".into());
    let module_kind = gewyvern::flow::infer_network_module_kind(
        &flow.operation,
        Some(&last_phase),
        None,
        "network_module",
    );
    let primary_stage = finding_summary
        .and_then(|summary| summary.missing_transitions.first().cloned())
        .unwrap_or(last_phase);
    let suspect_areas = finding_summary
        .map(|summary| summary.suspect_areas.as_slice())
        .unwrap_or(&[]);
    failure_confidence_label(status, module_kind, &primary_stage, suspect_areas).to_string()
}

pub(super) fn protocol_flow_failure_basis(
    flow: &gewyvern::flow::ProgramFlow,
    finding_summary: Option<&ProtocolFlowFindingSummary>,
) -> String {
    let status = protocol_flow_status(flow, finding_summary);
    let last_phase = protocol_flow_last_phase(flow).unwrap_or_else(|| "none".into());
    let module_kind = gewyvern::flow::infer_network_module_kind(
        &flow.operation,
        Some(&last_phase),
        None,
        "network_module",
    );
    let primary_stage = finding_summary
        .and_then(|summary| summary.missing_transitions.first().cloned())
        .unwrap_or(last_phase);
    let suspect_areas = finding_summary
        .map(|summary| summary.suspect_areas.as_slice())
        .unwrap_or(&[]);
    failure_basis_label(status, module_kind, &primary_stage, suspect_areas).to_string()
}

fn protocol_flow_summary_item_json(
    flow: &gewyvern::flow::ProgramFlow,
    finding_summary: Option<&ProtocolFlowFindingSummary>,
) -> String {
    let phases = protocol_flow_phases(flow);
    let network_module_kind = gewyvern::flow::infer_network_module_kind(
        &flow.operation,
        protocol_flow_last_phase(flow).as_deref(),
        None,
        "network_module",
    );
    let missing_transitions = finding_summary
        .map(|summary| summary.missing_transitions.as_slice())
        .unwrap_or(&[]);
    let network_module_kinds = finding_summary
        .map(|summary| summary.network_module_kinds.as_slice())
        .unwrap_or(&[]);
    let suspect_areas = finding_summary
        .map(|summary| summary.suspect_areas.as_slice())
        .unwrap_or(&[]);
    let failure_mode = protocol_flow_failure_mode(flow, finding_summary);
    let failure_detail = protocol_flow_failure_detail(flow, finding_summary);
    let failure_confidence = protocol_flow_failure_confidence(flow, finding_summary);
    let failure_basis = protocol_flow_failure_basis(flow, finding_summary);
    format!(
        "{{\"program_flow\":{},\"process\":{},\"operation\":\"{}\",\"network_module_kind\":\"{}\",\"network_module_kinds\":{},\"status\":\"{}\",\"failure_mode\":\"{}\",\"failure_mode_family\":\"{}\",\"failure_detail\":\"{}\",\"failure_detail_family\":\"{}\",\"failure_confidence\":\"{}\",\"failure_basis\":\"{}\",\"phases\":{},\"last_phase\":{},\"missing_transitions\":{},\"suspect_areas\":{}}}",
        flow.id.0,
        process_json(flow.process.as_ref()),
        operation_label(&flow.operation),
        network_module_kind,
        if network_module_kinds.is_empty() {
            format!("[\"{network_module_kind}\"]")
        } else {
            string_list_json(network_module_kinds)
        },
        protocol_flow_status(flow, finding_summary),
        failure_mode,
        failure_mode_family_label(&failure_mode),
        failure_detail,
        failure_detail_family_label(&failure_detail),
        failure_confidence,
        failure_basis,
        string_list_json(&phases),
        protocol_flow_last_phase(flow)
            .map(|phase| format!("\"{}\"", phase))
            .unwrap_or_else(|| "null".into()),
        string_list_json(missing_transitions),
        string_list_json(suspect_areas),
    )
}

pub(super) fn protocol_flow_summaries_json(export: &ExportBundle) -> String {
    let finding_summaries = protocol_flow_finding_summaries(export);
    format!(
        "[{}]",
        export
            .program_flows
            .iter()
            .map(|flow| protocol_flow_summary_item_json(flow, finding_summaries.get(&flow.id)))
            .collect::<Vec<_>>()
            .join(",")
    )
}

pub(super) fn protocol_flow_summaries_text(export: &ExportBundle) -> String {
    let locale = UiLocale::detect();
    if export.program_flows.is_empty() {
        return locale.none().to_string();
    }
    let finding_summaries = protocol_flow_finding_summaries(export);
    export
        .program_flows
        .iter()
        .map(|flow| {
            let phases = protocol_flow_phases(flow);
            let finding_summary = finding_summaries.get(&flow.id);
            let network_module_kind = gewyvern::flow::infer_network_module_kind(
                &flow.operation,
                protocol_flow_last_phase(flow).as_deref(),
                None,
                "network_module",
            );
            let missing_transitions = finding_summary
                .map(|summary| summary.missing_transitions.as_slice())
                .unwrap_or(&[]);
            let phase_text = if phases.is_empty() {
                locale.none().to_string()
            } else {
                phases.join(">")
            };
            let missing_text = if missing_transitions.is_empty() {
                String::new()
            } else {
                format!(" missing={}", missing_transitions.join("|"))
            };
            let failure_mode = protocol_flow_failure_mode(flow, finding_summary);
            let failure_detail = protocol_flow_failure_detail(flow, finding_summary);
            let failure_confidence = protocol_flow_failure_confidence(flow, finding_summary);
            let failure_basis = protocol_flow_failure_basis(flow, finding_summary);
            format!(
                "{}[kind={} status={} failure_mode={} failure_detail={} confidence={} basis={} phases={}{}]",
                operation_label(&flow.operation),
                network_module_kind,
                protocol_flow_status(flow, finding_summary),
                failure_mode,
                failure_detail,
                failure_confidence,
                failure_basis,
                phase_text,
                missing_text
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

pub(super) fn process_network_profile_summaries(
    export: &ExportBundle,
) -> Vec<ProcessNetworkProfileSummary> {
    let finding_summaries = protocol_flow_finding_summaries(export);
    let mut profiles = HashMap::<(u32, String), ProcessNetworkProfileSummary>::new();
    let mut module_scores = HashMap::<(u32, String), HashMap<String, u32>>::new();
    let mut stage_scores = HashMap::<(u32, String), HashMap<String, u32>>::new();
    let mut suspect_module_scores = HashMap::<(u32, String), HashMap<String, u32>>::new();

    for flow in &export.program_flows {
        let Some(process) = flow.process.as_ref() else {
            continue;
        };
        let key = (process.pid, process.comm.clone());
        let entry = profiles
            .entry(key.clone())
            .or_insert_with(|| ProcessNetworkProfileSummary {
                pid: process.pid,
                comm: process.comm.clone(),
                status: "idle".into(),
                primary_module_kind: "none".into(),
                primary_module_family: "general".into(),
                primary_failure_stage: "none".into(),
                primary_stage_family: "none".into(),
                primary_failure_mode: "none".into(),
                primary_failure_detail: "none".into(),
                primary_failure_confidence: "none".into(),
                primary_failure_basis: "none".into(),
                ..Default::default()
            });

        let operation = operation_label(&flow.operation);
        if !entry.operations.contains(&operation) {
            entry.operations.push(operation);
        }

        let inferred_kind = gewyvern::flow::infer_network_module_kind(
            &flow.operation,
            protocol_flow_last_phase(flow).as_deref(),
            None,
            "network_module",
        )
        .to_string();
        let last_phase = protocol_flow_last_phase(flow).unwrap_or_else(|| "none".into());
        if !entry.module_kinds.contains(&inferred_kind) {
            entry.module_kinds.push(inferred_kind.clone());
        }
        bump_score(&mut module_scores, &key, &inferred_kind, 1);
        bump_score(&mut stage_scores, &key, &last_phase, 1);

        for phase in protocol_flow_phases(flow) {
            if !entry.phases.contains(&phase) {
                entry.phases.push(phase);
            }
        }

        match finding_summaries.get(&flow.id) {
            Some(summary) if summary.has_findings => {
                entry.attention_flows += 1;
                entry.status = "attention".into();
                if summary.network_module_kinds.is_empty() {
                    bump_score(&mut module_scores, &key, &inferred_kind, 10);
                } else {
                    for module_kind in &summary.network_module_kinds {
                        bump_score(&mut module_scores, &key, module_kind, 10);
                    }
                }
                if summary.missing_transitions.is_empty() {
                    bump_score(&mut stage_scores, &key, &last_phase, 10);
                } else {
                    for transition in &summary.missing_transitions {
                        bump_score(&mut stage_scores, &key, transition, 10);
                    }
                }
                for module_kind in &summary.network_module_kinds {
                    if !entry.module_kinds.contains(module_kind) {
                        entry.module_kinds.push(module_kind.clone());
                    }
                }
                for transition in &summary.missing_transitions {
                    if !entry.missing_transitions.contains(transition) {
                        entry.missing_transitions.push(transition.clone());
                    }
                }
                for suspect_area in &summary.suspect_areas {
                    if !entry.suspect_areas.contains(suspect_area) {
                        entry.suspect_areas.push(suspect_area.clone());
                    }
                }
            }
            _ if protocol_flow_has_terminal_failure(flow) => {
                entry.attention_flows += 1;
                entry.status = "attention".into();
                bump_score(&mut module_scores, &key, &inferred_kind, 10);
                bump_score(&mut stage_scores, &key, &last_phase, 10);
            }
            _ => {
                entry.healthy_flows += 1;
                if entry.status != "attention" {
                    entry.status = "healthy".into();
                }
            }
        }
    }

    for finding in &export.program_findings {
        let Some(process) = finding.process.as_ref() else {
            continue;
        };
        let key = (process.pid, process.comm.clone());
        let entry = profiles
            .entry(key.clone())
            .or_insert_with(|| ProcessNetworkProfileSummary {
                pid: process.pid,
                comm: process.comm.clone(),
                status: "idle".into(),
                primary_module_kind: "none".into(),
                primary_module_family: "general".into(),
                primary_failure_stage: "none".into(),
                primary_stage_family: "none".into(),
                primary_failure_mode: "none".into(),
                primary_failure_detail: "none".into(),
                primary_failure_confidence: "none".into(),
                primary_failure_basis: "none".into(),
                ..Default::default()
            });
        entry.status = "attention".into();
        if !entry.module_kinds.contains(&finding.network_module_kind) {
            entry.module_kinds.push(finding.network_module_kind.clone());
        }
        if !entry.suspect_areas.contains(&finding.suspect_area) {
            entry.suspect_areas.push(finding.suspect_area.clone());
        }
        if !entry.suspect_modules.contains(&finding.module_label) {
            entry.suspect_modules.push(finding.module_label.clone());
        }
        bump_score(&mut module_scores, &key, &finding.network_module_kind, 20);
        if let Some(phase) = &finding.phase {
            bump_score(&mut stage_scores, &key, phase, 20);
        }
        if let Some(transition) = &finding.phase_transition {
            bump_score(&mut stage_scores, &key, transition, 25);
        }
        bump_score(&mut suspect_module_scores, &key, &finding.module_label, 20);
    }

    let mut profiles = profiles.into_values().collect::<Vec<_>>();
    for profile in &mut profiles {
        let key = (profile.pid, profile.comm.clone());
        profile.operations.sort();
        profile.operations.dedup();
        profile.module_kinds.sort();
        profile.module_kinds.dedup();
        profile.phases.sort();
        profile.phases.dedup();
        profile.missing_transitions.sort();
        profile.missing_transitions.dedup();
        profile.suspect_areas.sort();
        profile.suspect_areas.dedup();
        profile.suspect_modules.sort();
        profile.suspect_modules.dedup();
        profile.primary_module_kind = best_scored_value(&module_scores, &key)
            .or_else(|| first_non_none(&profile.module_kinds))
            .unwrap_or_else(|| "none".into());
        profile.primary_module_family =
            module_family_label(&profile.primary_module_kind).to_string();
        profile.primary_failure_stage =
            if profile.status == "attention" && !profile.missing_transitions.is_empty() {
                best_scored_value(&stage_scores, &key)
                    .filter(|stage| profile.missing_transitions.contains(stage))
                    .or_else(|| first_non_none(&profile.missing_transitions))
                    .unwrap_or_else(|| "none".into())
            } else {
                best_scored_value(&stage_scores, &key)
                    .or_else(|| first_non_none(&profile.missing_transitions))
                    .or_else(|| first_non_none(&profile.phases))
                    .unwrap_or_else(|| "none".into())
            };
        profile.primary_stage_family =
            stage_family_label(&profile.primary_failure_stage).to_string();
        profile.primary_failure_mode = failure_mode_label(
            &profile.status,
            &profile.primary_module_kind,
            &profile.primary_failure_stage,
            &profile.suspect_areas,
        )
        .to_string();
        profile.primary_failure_detail = failure_detail_label(
            &profile.status,
            &profile.primary_module_kind,
            &profile.primary_failure_stage,
            &profile.suspect_areas,
        )
        .to_string();
        let mut confidence = failure_confidence_label(
            &profile.status,
            &profile.primary_module_kind,
            &profile.primary_failure_stage,
            &profile.suspect_areas,
        );
        let basis = failure_basis_label(
            &profile.status,
            &profile.primary_module_kind,
            &profile.primary_failure_stage,
            &profile.suspect_areas,
        );
        let ambiguity_signals = usize::from(profile.module_kinds.len() > 1)
            + usize::from(profile.missing_transitions.len() > 1);
        if ambiguity_signals > 0 {
            confidence = reduce_confidence_level(confidence);
        }
        if ambiguity_signals > 1 {
            confidence = reduce_confidence_level(confidence);
        }
        profile.ambiguous = profile.module_kinds.len() > 1 || profile.missing_transitions.len() > 1;
        let mut competing_hypotheses = Vec::new();
        competing_hypotheses.extend(
            profile
                .module_kinds
                .iter()
                .filter(|kind| kind.as_str() != profile.primary_module_kind)
                .map(|kind| format!("module:{kind}")),
        );
        competing_hypotheses.extend(
            profile
                .missing_transitions
                .iter()
                .filter(|transition| transition.as_str() != profile.primary_failure_stage)
                .map(|transition| format!("transition:{transition}")),
        );
        competing_hypotheses.extend(
            profile
                .suspect_modules
                .iter()
                .skip(1)
                .map(|module| format!("suspect_module:{module}")),
        );
        competing_hypotheses.sort();
        competing_hypotheses.dedup();
        profile.competing_hypotheses = competing_hypotheses;
        profile.primary_failure_confidence = confidence.to_string();
        profile.primary_failure_basis = basis.to_string();
        if let Some(primary_suspect_module) = best_scored_value(&suspect_module_scores, &key) {
            if let Some(index) = profile
                .suspect_modules
                .iter()
                .position(|module| module == &primary_suspect_module)
            {
                let module = profile.suspect_modules.remove(index);
                profile.suspect_modules.insert(0, module);
            }
        }
    }
    profiles.sort_by(|a, b| a.pid.cmp(&b.pid).then_with(|| a.comm.cmp(&b.comm)));
    profiles
}

pub(super) fn process_network_profiles_json(export: &ExportBundle) -> String {
    format!(
        "[{}]",
        process_network_profile_summaries(export)
            .into_iter()
            .map(|profile| format!(
                "{{\"pid\":{},\"comm\":\"{}\",\"status\":\"{}\",\"ambiguous\":{},\"primary_module_kind\":\"{}\",\"primary_module_family\":\"{}\",\"primary_failure_stage\":\"{}\",\"primary_stage_family\":\"{}\",\"primary_failure_mode\":\"{}\",\"primary_failure_mode_family\":\"{}\",\"primary_failure_detail\":\"{}\",\"primary_failure_detail_family\":\"{}\",\"primary_failure_confidence\":\"{}\",\"primary_failure_basis\":\"{}\",\"competing_hypotheses\":{},\"operations\":{},\"module_kinds\":{},\"phases\":{},\"missing_transitions\":{},\"suspect_areas\":{},\"suspect_modules\":{},\"healthy_flows\":{},\"attention_flows\":{}}}",
                profile.pid,
                profile.comm,
                profile.status,
                profile.ambiguous,
                profile.primary_module_kind,
                profile.primary_module_family,
                profile.primary_failure_stage,
                profile.primary_stage_family,
                profile.primary_failure_mode,
                failure_mode_family_label(&profile.primary_failure_mode),
                profile.primary_failure_detail,
                failure_detail_family_label(&profile.primary_failure_detail),
                profile.primary_failure_confidence,
                profile.primary_failure_basis,
                string_list_json(&profile.competing_hypotheses),
                string_list_json(&profile.operations),
                string_list_json(&profile.module_kinds),
                string_list_json(&profile.phases),
                string_list_json(&profile.missing_transitions),
                string_list_json(&profile.suspect_areas),
                string_list_json(&profile.suspect_modules),
                profile.healthy_flows,
                profile.attention_flows,
            ))
            .collect::<Vec<_>>()
            .join(",")
    )
}

pub(super) fn process_network_profiles_text(export: &ExportBundle) -> String {
    let locale = UiLocale::detect();
    let profiles = process_network_profile_summaries(export);
    if profiles.is_empty() {
        return locale.none().to_string();
    }
    profiles
        .into_iter()
        .map(|profile| {
            let kinds = if profile.module_kinds.is_empty() {
                locale.none().to_string()
            } else {
                profile.module_kinds.join("|")
            };
            let phases = if profile.phases.is_empty() {
                locale.none().to_string()
            } else {
                profile.phases.join(">")
            };
            let missing = if profile.missing_transitions.is_empty() {
                String::new()
            } else {
                format!(" missing={}", profile.missing_transitions.join("|"))
            };
            format!(
                "{}(pid={})[status={} ambiguous={} primary_kind={} primary_stage={} failure_mode={} failure_detail={} confidence={} basis={} competing={} kinds={} healthy={} attention={} phases={}{}]",
                profile.comm,
                profile.pid,
                profile.status,
                profile.ambiguous,
                profile.primary_module_kind,
                profile.primary_failure_stage,
                profile.primary_failure_mode,
                profile.primary_failure_detail,
                profile.primary_failure_confidence,
                profile.primary_failure_basis,
                if profile.competing_hypotheses.is_empty() {
                    locale.none().to_string()
                } else {
                    profile.competing_hypotheses.join("|")
                },
                kinds,
                profile.healthy_flows,
                profile.attention_flows,
                phases,
                missing
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

pub(super) fn primary_process_profile_ambiguous_for_export(export: &ExportBundle) -> bool {
    primary_process_profile_for_export(export)
        .map(|profile| profile.ambiguous)
        .unwrap_or(false)
}

pub(super) fn competing_hypotheses_for_export(export: &ExportBundle) -> String {
    primary_process_profile_for_export(export)
        .map(|profile| string_list_json(&profile.competing_hypotheses))
        .unwrap_or_else(|| "[]".into())
}

pub(super) fn primary_process_profile_for_export(
    export: &ExportBundle,
) -> Option<ProcessNetworkProfileSummary> {
    let mut profiles = process_network_profile_summaries(export);
    profiles.sort_by(|left, right| {
        let left_rank = match left.status.as_str() {
            "attention" => 0,
            "healthy" => 1,
            _ => 2,
        };
        let right_rank = match right.status.as_str() {
            "attention" => 0,
            "healthy" => 1,
            _ => 2,
        };
        left_rank
            .cmp(&right_rank)
            .then_with(|| right.attention_flows.cmp(&left.attention_flows))
            .then_with(|| right.healthy_flows.cmp(&left.healthy_flows))
            .then_with(|| left.pid.cmp(&right.pid))
            .then_with(|| left.comm.cmp(&right.comm))
    });
    profiles.into_iter().next()
}

pub(super) fn primary_module_kind_for_export(export: &ExportBundle) -> String {
    if let Some(profile) = primary_process_profile_for_export(export) {
        return profile.primary_module_kind;
    }
    if let Some(finding) = export.program_findings.first() {
        return finding.network_module_kind.clone();
    }
    export
        .program_flows
        .first()
        .map(|flow| {
            gewyvern::flow::infer_network_module_kind(
                &flow.operation,
                protocol_flow_last_phase(flow).as_deref(),
                None,
                "network_module",
            )
            .to_string()
        })
        .unwrap_or_else(|| "none".into())
}

pub(super) fn primary_failure_stage_for_export(export: &ExportBundle) -> String {
    if let Some(profile) = primary_process_profile_for_export(export) {
        return profile.primary_failure_stage;
    }
    if let Some(finding) = export.program_findings.first() {
        if let Some(transition) = &finding.phase_transition {
            return transition.clone();
        }
        if let Some(phase) = &finding.phase {
            return phase.clone();
        }
    }
    export
        .program_flows
        .iter()
        .filter_map(protocol_flow_last_phase)
        .next_back()
        .unwrap_or_else(|| "none".into())
}

pub(super) fn primary_failure_mode_for_export(export: &ExportBundle) -> String {
    if let Some(profile) = primary_process_profile_for_export(export) {
        return profile.primary_failure_mode;
    }
    failure_mode_label(
        scan_target_status(export).label(),
        &primary_module_kind_for_export(export),
        &primary_failure_stage_for_export(export),
        &export
            .program_findings
            .iter()
            .map(|finding| finding.suspect_area.clone())
            .collect::<Vec<_>>(),
    )
    .to_string()
}

pub(super) fn primary_failure_detail_for_export(export: &ExportBundle) -> String {
    if let Some(profile) = primary_process_profile_for_export(export) {
        return profile.primary_failure_detail;
    }
    failure_detail_label(
        scan_target_status(export).label(),
        &primary_module_kind_for_export(export),
        &primary_failure_stage_for_export(export),
        &export
            .program_findings
            .iter()
            .map(|finding| finding.suspect_area.clone())
            .collect::<Vec<_>>(),
    )
    .to_string()
}

pub(super) fn primary_failure_confidence_for_export(export: &ExportBundle) -> String {
    if let Some(profile) = primary_process_profile_for_export(export) {
        return profile.primary_failure_confidence;
    }
    failure_confidence_label(
        scan_target_status(export).label(),
        &primary_module_kind_for_export(export),
        &primary_failure_stage_for_export(export),
        &export
            .program_findings
            .iter()
            .map(|finding| finding.suspect_area.clone())
            .collect::<Vec<_>>(),
    )
    .to_string()
}

pub(super) fn primary_failure_basis_for_export(export: &ExportBundle) -> String {
    if let Some(profile) = primary_process_profile_for_export(export) {
        return profile.primary_failure_basis;
    }
    failure_basis_label(
        scan_target_status(export).label(),
        &primary_module_kind_for_export(export),
        &primary_failure_stage_for_export(export),
        &export
            .program_findings
            .iter()
            .map(|finding| finding.suspect_area.clone())
            .collect::<Vec<_>>(),
    )
    .to_string()
}

pub(super) fn suspect_modules_for_export(export: &ExportBundle) -> String {
    if let Some(profile) = primary_process_profile_for_export(export) {
        if !profile.suspect_modules.is_empty() {
            return profile.suspect_modules.join(" | ");
        }
    }
    if export.program_findings.is_empty() {
        "none".into()
    } else {
        export
            .program_findings
            .iter()
            .map(|finding| finding.module_label.clone())
            .collect::<Vec<_>>()
            .join(" | ")
    }
}

pub(super) fn scan_target_status(export: &ExportBundle) -> ScanTargetStatus {
    if export.program_flows.is_empty() {
        ScanTargetStatus::Idle
    } else if export.program_findings.is_empty()
        && !export
            .program_flows
            .iter()
            .any(protocol_flow_has_terminal_failure)
    {
        ScanTargetStatus::Healthy
    } else {
        ScanTargetStatus::Attention
    }
}
