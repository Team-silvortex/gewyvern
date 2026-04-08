use crate::flow::{
    FlowSnapshot, ProgramFlow, ProgramFlowId, ProgramOperation, ProgramStage, ProgramStageKind,
};
use crate::ledger::{FactEnvelope, FactKind};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgramModel {
    pub id: &'static str,
    pub operation: ProgramOperation,
    pub rules: Vec<ProgramRule>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgramRule {
    pub predicate: ProgramPredicate,
    pub stage: Option<ProgramStageKind>,
    pub narrative: ProgramNarrative,
    pub dedupe: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProgramPredicate {
    ProcessBound,
    SocketStateObserved,
    DatagramObserved { l4_proto: u8 },
    RouteResolved,
    All(Vec<ProgramPredicate>),
    Any(Vec<ProgramPredicate>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProgramNarrative {
    None,
    Static(&'static str),
    ProcessBound,
}

pub fn build_program_flows(
    model: &ProgramModel,
    transport_flows: &[FlowSnapshot],
    facts: &[FactEnvelope],
) -> Vec<ProgramFlow> {
    transport_flows
        .iter()
        .enumerate()
        .map(|(idx, flow)| build_program_flow(model, (idx + 1) as u64, flow, facts))
        .collect()
}

fn build_program_flow(
    model: &ProgramModel,
    id: u64,
    flow: &FlowSnapshot,
    facts: &[FactEnvelope],
) -> ProgramFlow {
    let mut stages = Vec::new();
    let mut narrative = Vec::new();
    let mut seen_predicates = Vec::new();

    for fact in facts {
        for rule in &model.rules {
            if rule.dedupe && seen_predicates.contains(&rule.predicate) {
                continue;
            }
            if !matches_predicate(&rule.predicate, flow, fact, facts) {
                continue;
            }

            if let Some(kind) = &rule.stage {
                stages.push(ProgramStage {
                    at: fact.id,
                    kind: kind.clone(),
                });
            }

            if let Some(line) = render_narrative(&rule.narrative, flow, fact) {
                narrative.push(line);
            }

            if rule.dedupe {
                seen_predicates.push(rule.predicate.clone());
            }
        }
    }

    stages.sort_by_key(|stage| stage.at);

    ProgramFlow {
        id: ProgramFlowId(id),
        process: flow.process.clone(),
        operation: model.operation.clone(),
        transport_flows: vec![flow.id],
        stages,
        narrative,
    }
}

fn matches_predicate(
    predicate: &ProgramPredicate,
    flow: &FlowSnapshot,
    fact: &FactEnvelope,
    facts: &[FactEnvelope],
) -> bool {
    match predicate {
        ProgramPredicate::ProcessBound => flow.evidence.lineage_facts.contains(&fact.id),
        ProgramPredicate::SocketStateObserved => flow.evidence.tcp_state_facts.contains(&fact.id),
        ProgramPredicate::DatagramObserved { l4_proto } => {
            if !flow.evidence.packet_facts.contains(&fact.id) {
                return false;
            }
            matches!(&fact.kind, FactKind::PacketMeta(packet) if packet.l4_proto == *l4_proto)
        }
        ProgramPredicate::RouteResolved => flow.evidence.route_facts.contains(&fact.id),
        ProgramPredicate::All(predicates) => predicates
            .iter()
            .all(|predicate| predicate_satisfied_in_flow(predicate, flow, facts))
            && predicates
                .iter()
                .any(|predicate| matches_predicate(predicate, flow, fact, facts)),
        ProgramPredicate::Any(predicates) => predicates
            .iter()
            .any(|predicate| matches_predicate(predicate, flow, fact, facts)),
    }
}

fn predicate_satisfied_in_flow(
    predicate: &ProgramPredicate,
    flow: &FlowSnapshot,
    facts: &[FactEnvelope],
) -> bool {
    facts.iter()
        .any(|fact| matches_predicate(predicate, flow, fact, facts))
}

fn render_narrative(
    narrative: &ProgramNarrative,
    flow: &FlowSnapshot,
    _fact: &FactEnvelope,
) -> Option<String> {
    match narrative {
        ProgramNarrative::None => None,
        ProgramNarrative::Static(line) => Some((*line).into()),
        ProgramNarrative::ProcessBound => {
            if let Some(process) = &flow.process {
                Some(format!(
                    "process {} (pid={}) bound this network flow",
                    process.comm,
                    process.pid
                ))
            } else {
                None
            }
        }
    }
}
