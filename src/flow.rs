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
    pub route_facts: Vec<FactId>,
    pub lineage_facts: Vec<FactId>,
}
