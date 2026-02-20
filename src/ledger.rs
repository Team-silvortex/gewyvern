// gewyvern v0.03 - Ledger Layer
// Append-only physical fact storage

use std::time::SystemTime;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct FactId(pub u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct SessionId(pub u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct CpuId(pub u16);

#[derive(Clone, Debug)]
pub struct FactEnvelope {
    pub id: FactId,
    pub ts: SystemTime,
    pub cpu: CpuId,
    pub ifindex: Option<u32>,
    pub session: SessionId,
    pub kind: FactKind,
}

#[derive(Clone, Debug)]
pub enum FactKind {
    TcpState(TcpStateFact),
    PacketMeta(PacketMetaFact),
    RouteDecision(RouteDecisionFact),
    SockLineage(SockLineageFact),
    DropAction(DropActionFact),
    AttachScope(AttachScopeFact),
}

#[derive(Clone, Debug)]
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

#[derive(Clone, Debug)]
pub struct PacketMetaFact {
    pub netns: u32,
    pub sk_cookie: Option<u64>,
    pub dir: PacketDir,
    pub l3_proto: u16,
    pub l4_proto: u8,
    pub tot_len: u32,
    pub tcp_flags: u16,
    pub seq: Option<u32>,
    pub ack: Option<u32>,
    pub window: Option<u16>,
}

#[derive(Clone, Debug)]
pub enum PacketDir {
    Ingress,
    Egress,
}

#[derive(Clone, Debug)]
pub struct RouteDecisionFact {
    pub netns: u32,
    pub sk_cookie: Option<u64>,
    pub fib_table: Option<u32>,
    pub oif: u32,
    pub gw: Option<[u8; 16]>,
}

#[derive(Clone, Debug)]
pub struct SockLineageFact {
    pub netns: u32,
    pub sk_cookie: u64,
    pub pid: u32,
    pub tid: u32,
    pub cgroup_id: u64,
    pub comm: [u8; 16],
}

#[derive(Clone, Debug)]
pub struct DropActionFact {
    pub flow: u64,
    pub reason_id: u64,
    pub packet_fact: FactId,
    pub verdict: DropVerdict,
}

#[derive(Clone, Debug)]
pub enum DropVerdict {
    Applied,
    Refused,
}

#[derive(Clone, Debug)]
pub struct AttachScopeFact {
    pub scope_hash: u64,
    pub complete: bool,
}