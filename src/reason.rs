// gewyvern v0.03 - Reason Chain Structures

use crate::ledger::FactId;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct ReasonId(pub u64);

#[derive(Clone, Debug)]
pub struct ReasonChain {
    pub id: ReasonId,
    pub flow: u64,
    pub l0_facts: Vec<FactId>,
    pub l1: ReasonL1,
    pub l3: ReasonL3,
}

#[derive(Clone, Debug)]
pub struct ReasonL1 {
    pub tcp_state_timeline: Vec<FactId>,
    pub path_segments: Vec<FactId>,
    pub key_events: Vec<KeyEvent>,
}

#[derive(Clone, Debug)]
pub struct KeyEvent {
    pub at: FactId,
    pub kind: KeyEventKind,
}

#[derive(Clone, Debug)]
pub enum KeyEventKind {
    SynSeen,
    StateChange { old: u8, new: u8 },
    RetransSuspected,
    RouteChanged,
    FinOrRst,
}

#[derive(Clone, Debug)]
pub struct ReasonL3 {
    pub narrative: Vec<NarrLine>,
}

#[derive(Clone, Debug)]
pub struct NarrLine {
    pub at: FactId,
    pub text: String,
}