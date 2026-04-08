use crate::export::ExportBundle;
use crate::flow::{
    EvidenceIndex, FlowId, FlowLifecycleView, FlowSnapshot, PathSegment, PathView, ProcessView,
};
use crate::fragment::{
    builtin_registry, summarize_attach_failures, AttachFailure, AttachPlan, AttachReport,
    FragmentRegistry, RegistryError,
};
use crate::ledger::{FactEnvelope, FactId, FactKind};
use crate::loader::{
    LinuxProbeLoader, Loader, LoaderError,
};
use crate::program::build_program_flows;
use crate::reason::{build_reason_chains, ReasonChain, ReasonProfile};
use crate::template::{Template, TemplateError, WindowProfile};
use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, SystemTime};

#[derive(Clone, Debug)]
pub struct SessionConfig {
    pub template: Template,
    pub registry: FragmentRegistry,
    pub attach_failures: Vec<AttachFailure>,
}

#[derive(Clone, Debug)]
pub struct RuntimeSession {
    template: Template,
    window_profile: WindowProfile,
    reason_profile: ReasonProfile,
    attach_plan: AttachPlan,
    attach_report: AttachReport,
    facts: Vec<FactEnvelope>,
    rejected_facts: Vec<RejectedFact>,
    window_end: Option<SystemTime>,
    frozen_at: Option<SystemTime>,
}

#[derive(Debug, Eq, PartialEq)]
pub enum RuntimeError {
    InvalidTemplate(TemplateError),
    Registry(RegistryError),
    Loader(LoaderError),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RejectedFact {
    pub id: FactId,
    pub fragment_id: String,
    pub reason: RejectedFactReason,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RejectedFactReason {
    FragmentNotLoaded,
}

impl RejectedFactReason {
    pub fn label(&self) -> &'static str {
        match self {
            Self::FragmentNotLoaded => "fragment_not_loaded",
        }
    }
}

pub fn summarize_rejected_facts(rejected_facts: &[RejectedFact]) -> Vec<crate::export::RejectedFactSummaryItem> {
    let mut counts = BTreeMap::<(String, &'static str), u64>::new();

    for rejected in rejected_facts {
        *counts
            .entry((rejected.fragment_id.clone(), rejected.reason.label()))
            .or_default() += 1;
    }

    counts
        .into_iter()
        .map(|((fragment_id, reason), count)| crate::export::RejectedFactSummaryItem {
            fragment_id,
            reason: reason.into(),
            count,
        })
        .collect()
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
    process: Option<ProcessView>,
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
            attach_failures: Vec::new(),
        })
    }
}

impl RuntimeSession {
    pub fn start_with_loader<L: Loader>(
        config: SessionConfig,
        loader: &L,
    ) -> Result<Self, RuntimeError> {
        let attach_plan = config
            .registry
            .plan(config.template.fragment_set.iter().copied())
            .map_err(RuntimeError::Registry)?;

        let mut config = config;
        config.attach_failures = loader
            .collect_failures(&attach_plan)
            .map_err(RuntimeError::Loader)?;
        Self::start(config)
    }

    pub fn start_with_linux_kernel_probes(config: SessionConfig) -> Result<Self, RuntimeError> {
        Self::start_with_loader(config, &LinuxProbeLoader::kernel())
    }

    pub fn start_with_linux_tracepoint_probes(config: SessionConfig) -> Result<Self, RuntimeError> {
        Self::start_with_loader(config, &LinuxProbeLoader::tracepoints_only())
    }

    pub fn start_with_linux_tracepoint_smoke(
        config: SessionConfig,
        hookpoint_name: &'static str,
    ) -> Result<Self, RuntimeError> {
        Self::start_with_loader(config, &LinuxProbeLoader::single_tracepoint_smoke(hookpoint_name))
    }

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
        let attach_report = config
            .registry
            .attach_report_with_failure_records(&attach_plan, config.attach_failures.clone());

        Ok(Self {
            template: config.template,
            window_profile,
            reason_profile,
            attach_plan,
            attach_report,
            facts: Vec::new(),
            rejected_facts: Vec::new(),
            window_end: None,
            frozen_at: None,
        })
    }

    pub fn ingest(&mut self, fact: FactEnvelope) {
        if !self
            .attach_report
            .fragments_loaded
            .iter()
            .any(|fragment_id| fragment_id == &fact.fragment_id)
        {
            self.rejected_facts.push(RejectedFact {
                id: fact.id,
                fragment_id: fact.fragment_id,
                reason: RejectedFactReason::FragmentNotLoaded,
            });
            return;
        }
        if self
            .frozen_at
            .is_some_and(|freeze_at| fact.ts > freeze_at)
        {
            return;
        }
        self.facts.push(fact);
        self.facts.sort_by_key(|fact| fact.id);
    }

    pub fn freeze(&mut self, end: SystemTime) {
        let freeze_at = end + Duration::from_millis(self.window_profile.lateness_ms);
        self.window_end = Some(end);
        self.frozen_at = Some(freeze_at);
    }

    pub fn flow_snapshots(&self) -> Vec<FlowSnapshot> {
        let facts = self.materialized_facts();
        build_flow_snapshots(&facts)
    }

    pub fn reasons(&self) -> Vec<ReasonChain> {
        let facts = self.materialized_facts();
        let flows = build_flow_snapshots(&facts);
        build_reason_chains(
            &self.reason_profile,
            &flows,
            &facts,
        )
    }

    pub fn export_bundle(&self) -> ExportBundle {
        let facts = self.materialized_facts();
        let flows = build_flow_snapshots(&facts);
        let program_model = self
            .template
            .program_model
            .as_ref()
            .expect("template already validated");
        let program_flows = build_program_flows(program_model, &flows, &facts);
        let reasons = build_reason_chains(&self.reason_profile, &flows, &facts);
        let attach_failure_summary = summarize_attach_failures(&self.attach_report);
        let rejected_fact_summary = summarize_rejected_facts(&self.rejected_facts);

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
            attach_failure_summary,
            debug_summary: crate::export::DebugSummary {
                fragments_loaded: self.attach_report.fragments_loaded.len() as u64,
                hookpoints_failed: self.attach_report.hookpoints_failed.len() as u64,
                accepted_facts: facts.len() as u64,
                rejected_facts: self.rejected_facts.len() as u64,
                flows: flows.len() as u64,
                program_flows: program_flows.len() as u64,
                reasons: reasons.len() as u64,
                degraded: !self.attach_report.hookpoints_failed.is_empty()
                    || !self.rejected_facts.is_empty(),
            },
            window_profile: self.window_profile.clone(),
            reason_profile_id: self.reason_profile.id().into(),
            facts,
            rejected_facts: self.rejected_facts.clone(),
            rejected_fact_summary,
            flows,
            program_flows,
            reasons,
        }
    }

    pub fn seed_rejected_facts(&mut self, rejected_facts: Vec<RejectedFact>) {
        self.rejected_facts = rejected_facts;
    }

    fn materialized_facts(&self) -> Vec<FactEnvelope> {
        match (self.window_end, self.frozen_at) {
            (Some(window_end), Some(freeze_at)) => {
                let window_start = window_end
                    .checked_sub(Duration::from_millis(self.window_profile.duration_ms))
                    .unwrap_or(SystemTime::UNIX_EPOCH);
                self.facts
                    .iter()
                    .filter(|fact| fact.ts >= window_start && fact.ts <= freeze_at)
                    .cloned()
                    .collect()
            }
            (_, Some(freeze_at)) => self
                .facts
                .iter()
                .filter(|fact| fact.ts <= freeze_at)
                .cloned()
                .collect(),
            (_, None) => self.facts.clone(),
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
                if let FactKind::SockLineage(lineage) = &fact.kind {
                    acc.process = Some(ProcessView {
                        pid: lineage.pid,
                        tid: lineage.tid,
                        cgroup_id: lineage.cgroup_id,
                        comm: decode_comm(&lineage.comm),
                    });
                }
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

    fn route_facts(&self) -> &[crate::ledger::FactId] {
        &self.route_facts
    }

    fn lineage_facts(&self) -> &[crate::ledger::FactId] {
        &self.lineage_facts
    }
}

fn decode_comm(comm: &[u8; 16]) -> String {
    let end = comm.iter().position(|byte| *byte == 0).unwrap_or(comm.len());
    String::from_utf8_lossy(&comm[..end]).to_string()
}
