use crate::export::ExportBundle;
use crate::flow::{
    EvidenceIndex, FlowId, FlowLifecycleView, FlowSnapshot, PathSegment, PathView,
};
use crate::fragment::{builtin_registry, AttachPlan, AttachReport, FragmentRegistry, RegistryError};
use crate::ledger::{FactEnvelope, FactId, FactKind};
use crate::reason::{build_reason_chains, ReasonChain, ReasonProfile};
use crate::template::{Template, TemplateError, WindowProfile};
use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, SystemTime};

#[derive(Clone, Debug)]
pub struct SessionConfig {
    pub template: Template,
    pub registry: FragmentRegistry,
}

#[derive(Clone, Debug)]
pub struct RuntimeSession {
    template: Template,
    window_profile: WindowProfile,
    reason_profile: ReasonProfile,
    attach_plan: AttachPlan,
    attach_report: AttachReport,
    facts: Vec<FactEnvelope>,
    frozen_at: Option<SystemTime>,
}

#[derive(Debug, Eq, PartialEq)]
pub enum RuntimeError {
    InvalidTemplate(TemplateError),
    Registry(RegistryError),
}

#[derive(Default)]
struct FlowAccumulator {
    emerged_at: Option<FactId>,
    last_seen_at: Option<FactId>,
    tcp_state_now: Option<u8>,
    terminated: bool,
    termination_fact: Option<FactId>,
    current_oif: Option<u32>,
    current_gw: Option<[u8; 16]>,
    segments: Vec<PathSegment>,
    evidence: EvidenceIndex,
    fragment_sources: BTreeSet<String>,
}

impl SessionConfig {
    pub fn for_template(template: Template) -> Result<Self, RuntimeError> {
        template.validate().map_err(RuntimeError::InvalidTemplate)?;
        Ok(Self {
            template,
            registry: builtin_registry(),
        })
    }
}

impl RuntimeSession {
    pub fn start(config: SessionConfig) -> Result<Self, RuntimeError> {
        let window_profile = config
            .template
            .window_profile
            .clone()
            .expect("template already validated");
        let reason_profile = config
            .template
            .reason_profile
            .clone()
            .expect("template already validated");
        let attach_plan = config
            .registry
            .plan(config.template.fragment_set.iter().copied())
            .map_err(RuntimeError::Registry)?;
        let attach_report = config.registry.attach_report(&attach_plan);

        Ok(Self {
            template: config.template,
            window_profile,
            reason_profile,
            attach_plan,
            attach_report,
            facts: Vec::new(),
            frozen_at: None,
        })
    }

    pub fn ingest(&mut self, fact: FactEnvelope) {
        self.facts.push(fact);
        self.facts.sort_by_key(|fact| fact.id);
    }

    pub fn freeze(&mut self, end: SystemTime) {
        let freeze_at =
            end + Duration::from_millis(self.window_profile.lateness_ms);
        self.frozen_at = Some(freeze_at);
    }

    pub fn flow_snapshots(&self) -> Vec<FlowSnapshot> {
        build_flow_snapshots(&self.facts)
    }

    pub fn reasons(&self) -> Vec<ReasonChain> {
        build_reason_chains(&self.reason_profile, &self.flow_snapshots(), &self.facts)
    }

    pub fn export_bundle(&self) -> ExportBundle {
        ExportBundle {
            template_id: self.template.id.into(),
            fragment_inventory: self
                .attach_plan
                .fragments
                .iter()
                .map(|fragment| crate::export::FragmentInventoryItem {
                    id: fragment.id.into(),
                    version: fragment.version,
                })
                .collect(),
            attach_plan: self.attach_plan.clone(),
            attach_report: self.attach_report.clone(),
            window_profile: self.window_profile.clone(),
            reason_profile_id: self.reason_profile.id().into(),
            facts: self.facts.clone(),
            flows: self.flow_snapshots(),
            reasons: self.reasons(),
        }
    }
}

pub fn build_flow_snapshots(facts: &[FactEnvelope]) -> Vec<FlowSnapshot> {
    let mut by_cookie: BTreeMap<u64, Vec<FlowAccumulator>> = BTreeMap::new();

    for fact in facts {
        let cookie = match &fact.kind {
            FactKind::TcpState(state) => state.sk_cookie,
            FactKind::PacketMeta(packet) => packet.sk_cookie.unwrap_or(0),
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
        acc.fragment_sources.insert(fact.fragment_id.clone());
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
            FactKind::SockLineage(_) => {
                acc.evidence.lineage_facts.push(fact.id);
            }
            FactKind::DropAction(_) | FactKind::AttachScope(_) => {}
        }
    }

    by_cookie
        .into_iter()
        .flat_map(|(_, flows)| flows.into_iter())
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
    if !acc.route_facts().is_empty() {
        score += 0.3;
    }
    score
}

trait FlowAccumulatorView {
    fn tcp_state_facts(&self) -> &[crate::ledger::FactId];
    fn packet_facts(&self) -> &[crate::ledger::FactId];
    fn route_facts(&self) -> &[crate::ledger::FactId];
}

impl FlowAccumulatorView for EvidenceIndex {
    fn tcp_state_facts(&self) -> &[crate::ledger::FactId] {
        &self.tcp_state_facts
    }

    fn packet_facts(&self) -> &[crate::ledger::FactId] {
        &self.packet_facts
    }

    fn route_facts(&self) -> &[crate::ledger::FactId] {
        &self.route_facts
    }
}
