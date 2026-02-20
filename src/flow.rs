// gewyvern v0.03 - Flow Registry View Layer

use crate::ledger::FactId;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct FlowId(pub u64);

#[derive(Clone, Debug)]
pub struct FlowView {
    pub id: FlowId,
    pub lifecycle: FlowLifecycleView,
    pub path: PathView,
    pub evidence: EvidenceIndex,
    pub confidence: f32,
}

#[derive(Clone, Debug)]
pub struct FlowLifecycleView {
    pub emerged_at: FactId,
    pub last_seen_at: FactId,
    pub tcp_state_now: Option<u8>,
    pub terminated: bool,
    pub termination_fact: Option<FactId>,
}

#[derive(Clone, Debug)]
pub struct PathView {
    pub current_oif: Option<u32>,
    pub current_gw: Option<[u8; 16]>,
    pub segments: Vec<PathSegment>,
}

#[derive(Clone, Debug)]
pub struct PathSegment {
    pub started_at: FactId,
    pub oif: Option<u32>,
    pub gw: Option<[u8; 16]>,
}

#[derive(Clone, Debug, Default)]
pub struct EvidenceIndex {
    pub tcp_state_facts: Vec<FactId>,
    pub packet_facts: Vec<FactId>,
    pub route_facts: Vec<FactId>,
    pub lineage_facts: Vec<FactId>,
}