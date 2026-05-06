use std::collections::BTreeMap;
use std::fmt;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct FactId(pub u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct SessionId(pub u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct CpuId(pub u16);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FactEnvelope {
    pub id: FactId,
    pub ts: SystemTime,
    pub cpu: CpuId,
    pub ifindex: Option<u32>,
    pub session: SessionId,
    pub fragment_id: String,
    pub kind: FactKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FactKind {
    TcpState(TcpStateFact),
    PacketMeta(PacketMetaFact),
    RouteDecision(RouteDecisionFact),
    SockLineage(SockLineageFact),
    DropAction(DropActionFact),
    AttachScope(AttachScopeFact),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum FactKindTag {
    TcpState,
    PacketMeta,
    RouteDecision,
    SockLineage,
    DropAction,
    AttachScope,
}

impl FactKind {
    pub fn tag(&self) -> FactKindTag {
        match self {
            Self::TcpState(_) => FactKindTag::TcpState,
            Self::PacketMeta(_) => FactKindTag::PacketMeta,
            Self::RouteDecision(_) => FactKindTag::RouteDecision,
            Self::SockLineage(_) => FactKindTag::SockLineage,
            Self::DropAction(_) => FactKindTag::DropAction,
            Self::AttachScope(_) => FactKindTag::AttachScope,
        }
    }
}

impl fmt::Display for FactKindTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::TcpState => "tcp_state",
            Self::PacketMeta => "packet_meta",
            Self::RouteDecision => "route_decision",
            Self::SockLineage => "sock_lineage",
            Self::DropAction => "drop_action",
            Self::AttachScope => "attach_scope",
        };
        f.write_str(value)
    }
}

impl FactKindTag {
    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "tcp_state" => Some(Self::TcpState),
            "packet_meta" => Some(Self::PacketMeta),
            "route_decision" => Some(Self::RouteDecision),
            "sock_lineage" => Some(Self::SockLineage),
            "drop_action" => Some(Self::DropAction),
            "attach_scope" => Some(Self::AttachScope),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TcpStateFact {
    pub netns: u32,
    pub sk_cookie: u64,
    pub saddr: [u8; 16],
    pub daddr: [u8; 16],
    pub sport: u16,
    pub dport: u16,
    pub family: u8,
    pub old: u8,
    pub new: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PacketMetaFact {
    pub netns: u32,
    pub sk_cookie: Option<u64>,
    pub dir: PacketDir,
    pub local_port: Option<u16>,
    pub remote_port: Option<u16>,
    pub payload_byte0: Option<u8>,
    pub payload_byte1: Option<u8>,
    pub payload_prefix2: Option<u16>,
    pub payload_prefix4: Option<u32>,
    pub payload_byte4: Option<u8>,
    pub payload_byte5: Option<u8>,
    pub payload_byte9: Option<u8>,
    pub payload_byte10: Option<u8>,
    pub payload_byte13: Option<u8>,
    pub payload_bytes: BTreeMap<u16, u8>,
    pub l3_proto: u16,
    pub l4_proto: u8,
    pub tot_len: u32,
    pub tcp_flags: u16,
    pub seq: Option<u32>,
    pub ack: Option<u32>,
    pub window: Option<u16>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PacketDir {
    Ingress,
    Egress,
}

impl PacketDir {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ingress => "ingress",
            Self::Egress => "egress",
        }
    }

    pub fn as_flow_str(&self) -> &'static str {
        match self {
            Self::Ingress => "remote_to_local",
            Self::Egress => "local_to_remote",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "ingress" | "remote_to_local" => Some(Self::Ingress),
            "egress" | "local_to_remote" => Some(Self::Egress),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouteDecisionFact {
    pub netns: u32,
    pub sk_cookie: Option<u64>,
    pub fib_table: Option<u32>,
    pub oif: u32,
    pub gw: Option<[u8; 16]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SockLineageFact {
    pub netns: u32,
    pub sk_cookie: u64,
    pub pid: u32,
    pub tid: u32,
    pub cgroup_id: u64,
    pub comm: [u8; 16],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DropActionFact {
    pub flow: u64,
    pub reason_id: u64,
    pub packet_fact: FactId,
    pub verdict: DropVerdict,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DropVerdict {
    Applied,
    Refused,
}

impl DropVerdict {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Applied => "applied",
            Self::Refused => "refused",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "applied" => Some(Self::Applied),
            "refused" => Some(Self::Refused),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachScopeFact {
    pub scope_hash: u64,
    pub complete: bool,
}

pub fn system_time_to_millis(ts: SystemTime) -> u64 {
    ts.duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_millis() as u64
}

pub fn millis_to_system_time(ms: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_millis(ms)
}
