use crate::ir::SignalKind;
use crate::ledger::FactId;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct FlowId(pub u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct ProgramFlowId(pub u64);

#[derive(Clone, Debug, PartialEq)]
pub struct FlowSnapshot {
    pub id: FlowId,
    pub lifecycle: FlowLifecycleView,
    pub path: PathView,
    pub process: Option<ProcessView>,
    pub evidence: EvidenceIndex,
    pub confidence: f32,
    pub fragment_sources: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgramFlow {
    pub id: ProgramFlowId,
    pub process: Option<ProcessView>,
    pub operation: ProgramOperation,
    pub transport_flows: Vec<FlowId>,
    pub stages: Vec<ProgramStage>,
    pub narrative: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgramFinding {
    pub program_flow: ProgramFlowId,
    pub process: Option<ProcessView>,
    pub operation: ProgramOperation,
    pub module_label: String,
    pub network_module_kind: String,
    pub phase: Option<String>,
    pub phase_kind: Option<String>,
    pub phase_transition: Option<String>,
    pub phase_transition_kind: Option<String>,
    pub suspect_area: String,
    pub cause: ProgramFindingCause,
    pub summary: String,
    pub supporting_fragments: Vec<String>,
    pub evidence_trace: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleFinding {
    pub module_label: String,
    pub process: Option<ProcessView>,
    pub operation: ProgramOperation,
    pub severity: ModuleSeverity,
    pub network_module_kinds: Vec<String>,
    pub phases: Vec<String>,
    pub phase_kinds: Vec<String>,
    pub phase_transitions: Vec<String>,
    pub phase_transition_kinds: Vec<String>,
    pub suspect_areas: Vec<String>,
    pub causes: Vec<ProgramFindingCause>,
    pub supporting_fragments: Vec<String>,
    pub program_flows: Vec<ProgramFlowId>,
    pub summaries: Vec<String>,
    pub evidence_trace: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum ModuleSeverity {
    High,
    Medium,
    Low,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProgramFindingCause {
    AttachFailure,
    RejectedEvidence,
    MissingCoreStage,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum ProgramOperation {
    ConnectFlow,
    DatagramExchange,
    Custom(String),
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgramStage {
    pub at: FactId,
    pub kind: ProgramStageKind,
    pub phase: Option<String>,
    pub phase_kind: Option<String>,
}

pub type ProgramStageKind = SignalKind;

pub fn infer_network_module_kind(
    operation: &ProgramOperation,
    phase: Option<&str>,
    phase_transition: Option<&str>,
    suspect_area: &str,
) -> &'static str {
    let operation_id = match operation {
        ProgramOperation::ConnectFlow => "connect_flow",
        ProgramOperation::DatagramExchange => "datagram_exchange",
        ProgramOperation::Custom(value) => value.as_str(),
        ProgramOperation::Unknown => "unknown",
    };

    if matches!(operation_id, "dns_lookup" | "dns_tcp_query" | "mdns_query") {
        return "name_resolution";
    }
    if operation_id.starts_with("http3_") {
        return "http3_request_response";
    }
    if operation_id.starts_with("http_connect_") {
        return "proxy_tunnel_establishment";
    }
    if operation_id.starts_with("http_") {
        return "http_request_response";
    }
    if operation_id.starts_with("tls_") {
        return "tls_handshake";
    }
    if operation_id.starts_with("quic_") {
        if operation_id.contains("stream") || operation_id.contains("bidi") {
            return "quic_stream_session";
        }
        return "quic_handshake";
    }
    if operation_id.starts_with("hy2_") {
        if operation_id.contains("udp") {
            return "proxy_udp_relay";
        }
        if operation_id.contains("tcp") {
            return "proxy_tcp_relay";
        }
        return "proxy_authentication";
    }
    if operation_id.starts_with("mysql_") || operation_id.starts_with("postgres_") {
        if operation_id.contains("auth")
            || phase.is_some_and(|value| value.contains("auth") || value.contains("password"))
        {
            return "database_authentication";
        }
        if operation_id.contains("connect") {
            return "database_connectivity";
        }
        if operation_id.contains("error") {
            return "database_error_handling";
        }
        return "database_query";
    }
    if operation_id.starts_with("ldap_") {
        if operation_id.contains("sync") {
            return "directory_sync";
        }
        if operation_id.contains("modify") || operation_id.contains("write") {
            return "directory_write";
        }
        if operation_id.contains("search") {
            return "directory_search";
        }
        return "directory_bind";
    }
    if operation_id.starts_with("amqp_") {
        if operation_id.contains("publish") {
            return "message_publish";
        }
        return "message_session";
    }
    if operation_id.starts_with("mqtt_") {
        return "message_session";
    }
    if operation_id.starts_with("redis_") {
        return "cache_access";
    }
    if operation_id.starts_with("memcached_") {
        return "cache_access";
    }
    if operation_id.starts_with("smtp_") {
        return "mail_session";
    }
    if operation_id.starts_with("ftp_") {
        if operation_id.contains("list")
            || operation_id.contains("retr")
            || operation_id.contains("stor")
        {
            return "file_transfer_session";
        }
        return "authentication_exchange";
    }
    if operation_id.starts_with("ssh_") {
        return "remote_access_session";
    }
    if operation_id.starts_with("socks5_") {
        return "proxy_negotiation";
    }
    if operation_id.starts_with("radius_") {
        return "authentication_exchange";
    }
    if operation_id.starts_with("snmp_") {
        return "management_query";
    }
    if operation_id.starts_with("dhcp_") {
        return "address_configuration";
    }
    if operation_id.starts_with("ntp_") {
        return "time_synchronization";
    }
    if operation_id.starts_with("sip_") {
        return "signaling_session";
    }
    if operation_id.starts_with("gtpu_") {
        return "tunnel_control";
    }
    if operation_id.starts_with("wireguard_") {
        return "tunnel_handshake";
    }
    if operation_id.starts_with("coap_") {
        return "iot_request_response";
    }
    if operation_id == "connect_flow"
        || phase_transition.is_some_and(|value| value.contains("connect->establish"))
    {
        return "connection_establishment";
    }
    if operation_id == "datagram_exchange" {
        return "datagram_exchange";
    }

    match suspect_area {
        "process_binding" => "process_binding",
        "socket_state" => "connection_establishment",
        "route_resolution" => "route_resolution",
        "transport_io" => "transport_session",
        "datagram_io" => "datagram_exchange",
        _ => "network_module",
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct ProcessView {
    pub pid: u32,
    pub tid: u32,
    pub cgroup_id: u64,
    pub comm: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FlowLifecycleView {
    pub emerged_at: FactId,
    pub last_seen_at: FactId,
    pub tcp_state_now: Option<u8>,
    pub terminated: bool,
    pub termination_fact: Option<FactId>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PathView {
    pub current_oif: Option<u32>,
    pub current_gw: Option<[u8; 16]>,
    pub segments: Vec<PathSegment>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PathSegment {
    pub started_at: FactId,
    pub oif: Option<u32>,
    pub gw: Option<[u8; 16]>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct EvidenceIndex {
    pub tcp_state_facts: Vec<FactId>,
    pub packet_facts: Vec<FactId>,
    pub quic_facts: Vec<FactId>,
    pub route_facts: Vec<FactId>,
    pub lineage_facts: Vec<FactId>,
}
