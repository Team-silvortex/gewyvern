pub(crate) fn first_or_none(items: &[String]) -> String {
    items.first().cloned().unwrap_or_else(|| "none".into())
}

pub(crate) fn first_non_none(items: &[String]) -> Option<String> {
    items.iter().find(|item| item.as_str() != "none").cloned()
}

pub(crate) fn module_family_label(module_kind: &str) -> &'static str {
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

pub(crate) fn stage_family_label(stage: &str) -> &'static str {
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
        || lowered.contains("discover")
        || lowered.contains("nak")
        || lowered.contains("offer")
        || lowered.contains("probe")
        || lowered.contains("report")
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

pub(crate) fn failure_mode_label(
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
        || stage.contains("authorization_failure")
        || stage.contains("unauthorized")
        || stage.contains("nak")
    {
        return "server_denied";
    }
    if stage.contains("report_pdu")
        || stage.contains("constraint")
        || stage.contains("error")
        || module.contains("error")
    {
        return "semantic_error";
    }
    if stage.contains("close") {
        return "peer_closed";
    }
    if let Some((left, right)) = stage.split_once("->") {
        if left.starts_with("send")
            && (left.contains("request")
                || left.contains("query")
                || left.contains("discover")
                || left.contains("publish")
                || left.contains("notification")
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
                || left.contains("greeting")
                || left.contains("probe"))
            && (right.starts_with("receive")
                || right.contains("response")
                || right.contains("result")
                || right.contains("report")
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

pub(crate) fn failure_mode_family_label(mode: &str) -> &'static str {
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

pub(crate) fn failure_detail_label(
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
    if stage.contains("denied")
        || stage.contains("authorization_failure")
        || stage.contains("unauthorized")
    {
        return "access_denied";
    }
    if stage.contains("nak") {
        return "request_rejected";
    }
    if stage.contains("report_pdu") || stage.contains("error") || module.contains("error") {
        return "protocol_error";
    }
    if stage.contains("close") {
        return "peer_closed";
    }
    if let Some((left, right)) = stage.split_once("->") {
        if left.starts_with("send")
            && (left.contains("request")
                || left.contains("query")
                || left.contains("discover")
                || left.contains("publish")
                || left.contains("notification")
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
                || left.contains("greeting")
                || left.contains("probe"))
            && (right.starts_with("receive")
                || right.contains("response")
                || right.contains("result")
                || right.contains("report")
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

pub(crate) fn failure_detail_family_label(detail: &str) -> &'static str {
    match detail {
        "dns_unresolved" => "dns",
        "route_or_connect_blocked" => "connect",
        "handshake_incomplete" => "handshake",
        "request_sent_no_reply" => "timeout",
        "request_not_sent" | "followup_not_sent" => "blocked",
        "protocol_error" | "protocol_constraint_violation" => "semantic",
        "access_denied" | "auth_required" | "request_rejected" => "denied",
        "peer_closed" => "peer",
        "none" => "none",
        _ => "general",
    }
}

pub(crate) fn reduce_confidence_level(level: &str) -> &'static str {
    match level {
        "high" => "medium",
        "medium" => "low",
        "low" => "low",
        _ => "none",
    }
}

pub(crate) fn failure_basis_label(
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
        || stage.contains("authorization_failure")
        || stage.contains("unauthorized")
        || stage.contains("nak")
        || stage.contains("report_pdu")
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

pub(crate) fn failure_confidence_label(
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
