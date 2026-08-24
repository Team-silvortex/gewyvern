use std::collections::{BTreeMap, BTreeSet};

use crate::flow::{
    EvidenceIndex, FlowId, FlowLifecycleView, FlowSnapshot, PathSegment, PathView, ProcessView,
};
use crate::ledger::{FactEnvelope, FactId, FactKind};

#[derive(Default)]
struct FlowAccumulator {
    emerged_at: Option<FactId>,
    last_seen_at: Option<FactId>,
    tcp_state_now: Option<u8>,
    terminated: bool,
    termination_fact: Option<FactId>,
    current_oif: Option<u32>,
    current_gw: Option<[u8; 16]>,
    process: Option<ProcessView>,
    segments: Vec<PathSegment>,
    evidence: EvidenceIndex,
    fragment_sources: BTreeSet<String>,
}

pub fn build_flow_snapshots(facts: &[FactEnvelope]) -> Vec<FlowSnapshot> {
    let mut by_cookie: BTreeMap<u64, Vec<FlowAccumulator>> = BTreeMap::new();

    for fact in facts {
        let cookie = match &fact.kind {
            FactKind::TcpState(state) => state.sk_cookie,
            FactKind::PacketMeta(packet) => packet.sk_cookie.unwrap_or(0),
            FactKind::QuicMeta(quic) => quic.sk_cookie.unwrap_or(0),
            FactKind::RouteDecision(route) => route.sk_cookie.unwrap_or(0),
            FactKind::SockLineage(lineage) => lineage.sk_cookie,
            FactKind::DropAction(drop) => drop.flow,
            FactKind::AttachScope(_) => 0,
        };

        let flows = by_cookie.entry(cookie).or_default();
        if flows.is_empty() {
            flows.push(FlowAccumulator::default());
        }

        let should_rotate = match (&fact.kind, flows.last()) {
            (FactKind::RouteDecision(route), Some(current)) => {
                current.current_oif.is_some() && current.current_oif != Some(route.oif)
            }
            _ => false,
        };
        if should_rotate {
            flows.push(FlowAccumulator::default());
        }

        let acc = flows.last_mut().expect("flow accumulator should exist");
        if !acc.fragment_sources.contains(fact.fragment_id.as_str()) {
            acc.fragment_sources.insert(fact.fragment_id.clone());
        }
        acc.last_seen_at = Some(fact.id);
        acc.emerged_at.get_or_insert(fact.id);

        match &fact.kind {
            FactKind::TcpState(state) => {
                acc.evidence.tcp_state_facts.push(fact.id);
                acc.tcp_state_now = Some(state.new);
                if state.new >= 7 {
                    acc.terminated = true;
                    acc.termination_fact = Some(fact.id);
                }
            }
            FactKind::PacketMeta(_) => {
                acc.evidence.packet_facts.push(fact.id);
            }
            FactKind::QuicMeta(_) => {
                acc.evidence.quic_facts.push(fact.id);
            }
            FactKind::RouteDecision(route) => {
                acc.evidence.route_facts.push(fact.id);
                let changed = acc.current_oif != Some(route.oif) || acc.current_gw != route.gw;
                if changed {
                    acc.current_oif = Some(route.oif);
                    acc.current_gw = route.gw;
                    acc.segments.push(PathSegment {
                        started_at: fact.id,
                        oif: Some(route.oif),
                        gw: route.gw,
                    });
                }
            }
            FactKind::SockLineage(lineage) => {
                acc.evidence.lineage_facts.push(fact.id);
                acc.process = Some(ProcessView {
                    pid: lineage.pid,
                    tid: lineage.tid,
                    cgroup_id: lineage.cgroup_id,
                    comm: decode_comm_or_redacted(&lineage.comm),
                });
            }
            FactKind::DropAction(_) | FactKind::AttachScope(_) => {}
        }
    }

    by_cookie
        .into_values()
        .flatten()
        .filter(|acc| acc.emerged_at.is_some())
        .enumerate()
        .map(|(idx, acc)| build_flow_snapshot((idx + 1) as u64, acc))
        .collect()
}

fn build_flow_snapshot(id: u64, acc: FlowAccumulator) -> FlowSnapshot {
    let confidence = confidence_for_flow(&acc.evidence);
    FlowSnapshot {
        id: FlowId(id),
        lifecycle: FlowLifecycleView {
            emerged_at: acc.emerged_at.expect("flow should have a first fact"),
            last_seen_at: acc.last_seen_at.expect("flow should have a last fact"),
            tcp_state_now: acc.tcp_state_now,
            terminated: acc.terminated,
            termination_fact: acc.termination_fact,
        },
        path: PathView {
            current_oif: acc.current_oif,
            current_gw: acc.current_gw,
            segments: acc.segments,
        },
        process: acc.process,
        evidence: acc.evidence,
        confidence,
        fragment_sources: acc.fragment_sources.into_iter().collect(),
    }
}

fn confidence_for_flow(acc: &impl FlowAccumulatorView) -> f32 {
    let mut score = 0.0f32;
    if !acc.tcp_state_facts().is_empty() {
        score += 0.4;
    }
    if !acc.packet_facts().is_empty() {
        score += 0.3;
    }
    if !acc.quic_facts().is_empty() {
        score += 0.1;
    }
    if !acc.route_facts().is_empty() {
        score += 0.3;
    }
    if !acc.lineage_facts().is_empty() {
        score += 0.2;
    }
    score.min(1.0)
}

trait FlowAccumulatorView {
    fn tcp_state_facts(&self) -> &[crate::ledger::FactId];
    fn packet_facts(&self) -> &[crate::ledger::FactId];
    fn quic_facts(&self) -> &[crate::ledger::FactId];
    fn route_facts(&self) -> &[crate::ledger::FactId];
    fn lineage_facts(&self) -> &[crate::ledger::FactId];
}

impl FlowAccumulatorView for EvidenceIndex {
    fn tcp_state_facts(&self) -> &[crate::ledger::FactId] {
        &self.tcp_state_facts
    }

    fn packet_facts(&self) -> &[crate::ledger::FactId] {
        &self.packet_facts
    }

    fn quic_facts(&self) -> &[crate::ledger::FactId] {
        &self.quic_facts
    }

    fn route_facts(&self) -> &[crate::ledger::FactId] {
        &self.route_facts
    }

    fn lineage_facts(&self) -> &[crate::ledger::FactId] {
        &self.lineage_facts
    }
}

fn decode_comm_or_redacted(comm: &[u8; 16]) -> String {
    let end = comm
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(comm.len());
    if end == 0 {
        "<redacted>".into()
    } else {
        String::from_utf8_lossy(&comm[..end]).to_string()
    }
}
