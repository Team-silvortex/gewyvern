use crate::flow::{FlowSnapshot, ProgramFlow, ProgramFlowId, ProgramOperation, ProgramStage};
use crate::ir::{
    FlowPredicate, NarrativeSurface, NarrativeTemplate, RuleTemplate, matches_flow_predicate_refs,
    render_narrative_template,
};
use crate::ledger::{FactEnvelope, FactIndex};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgramModel {
    pub id: String,
    pub operation: ProgramOperation,
    pub rules: Vec<ProgramRule>,
}

pub type ProgramPredicate = FlowPredicate;
pub type ProgramNarrative = NarrativeTemplate;
pub type ProgramRule = RuleTemplate;

pub fn build_program_flows(
    model: &ProgramModel,
    transport_flows: &[FlowSnapshot],
    facts: &[FactEnvelope],
) -> Vec<ProgramFlow> {
    let fact_index = FactIndex::new(facts);
    build_program_flows_indexed(model, transport_flows, &fact_index)
}

pub(crate) fn build_program_flows_indexed(
    model: &ProgramModel,
    transport_flows: &[FlowSnapshot],
    fact_index: &FactIndex<'_>,
) -> Vec<ProgramFlow> {
    transport_flows
        .iter()
        .enumerate()
        .map(|(idx, flow)| {
            let facts = fact_index.facts_for_ids(flow.evidence.fact_ids());
            build_program_flow(model, (idx + 1) as u64, flow, &facts)
        })
        .collect()
}

fn build_program_flow(
    model: &ProgramModel,
    id: u64,
    flow: &FlowSnapshot,
    facts: &[&FactEnvelope],
) -> ProgramFlow {
    let mut stages = Vec::new();
    let mut narrative = Vec::new();
    let mut seen_predicates = Vec::new();

    for &fact in facts {
        for rule in &model.rules {
            if rule.dedupe && seen_predicates.contains(&rule.predicate) {
                continue;
            }
            if !matches_flow_predicate_refs(&rule.predicate, flow, fact, facts) {
                continue;
            }

            if let Some(kind) = &rule.signal {
                stages.push(ProgramStage::from_signal(
                    fact.id,
                    kind.clone(),
                    rule.phase.clone(),
                ));
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

fn render_narrative(
    narrative: &ProgramNarrative,
    flow: &FlowSnapshot,
    fact: &FactEnvelope,
) -> Option<String> {
    render_narrative_template(narrative, NarrativeSurface::Program, flow, fact)
}
