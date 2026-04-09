use crate::export::ExportBundle;
use crate::flow::{
    EvidenceIndex, FlowId, FlowLifecycleView, FlowSnapshot, ModuleFinding, ModuleSeverity,
    PathSegment, PathView, ProcessView, ProgramFinding, ProgramFindingCause, ProgramFlow,
};
use crate::fragment::{
    builtin_registry, summarize_attach_failures, AttachFailure, AttachPlan, AttachReport,
    BindingDiagnostics, EvidenceTier, FragmentRegistry, RegistryError, RuleTier,
};
use crate::ledger::{FactEnvelope, FactId, FactKind, FactKindTag};
use crate::loader::{
    LinuxProbeLoader, Loader, LoaderError,
};
use crate::program::build_program_flows;
use crate::reason::{build_reason_chains, ReasonChain, ReasonProfile};
use crate::template::{FragmentParamValue, Template, TemplateBinding, TemplateError, WindowProfile};
use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, SystemTime};

#[derive(Clone, Debug)]
pub struct SessionConfig {
    pub template: Template,
    pub registry: FragmentRegistry,
    pub attach_failures: Vec<AttachFailure>,
    pub fragment_params: BTreeMap<String, BTreeMap<String, FragmentParamValue>>,
    pub evidence_overrides: BTreeMap<FactKindTag, EvidenceTier>,
}

#[derive(Clone, Debug)]
pub struct RuntimeSession {
    template: Template,
    window_profile: WindowProfile,
    reason_profile: ReasonProfile,
    attach_plan: AttachPlan,
    attach_report: AttachReport,
    binding_diagnostics: BindingDiagnostics,
    fragment_params: BTreeMap<String, BTreeMap<String, FragmentParamValue>>,
    evidence_overrides: BTreeMap<FactKindTag, EvidenceTier>,
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
    FilteredByFragmentParam,
}

impl RejectedFactReason {
    pub fn label(&self) -> &'static str {
        match self {
            Self::FragmentNotLoaded => "fragment_not_loaded",
            Self::FilteredByFragmentParam => "filtered_by_fragment_param",
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
            fragment_params: BTreeMap::new(),
            evidence_overrides: BTreeMap::new(),
        })
    }

    pub fn for_binding(binding: TemplateBinding) -> Result<Self, RuntimeError> {
        binding.validate().map_err(RuntimeError::InvalidTemplate)?;
        let registry = builtin_registry();
        registry
            .validate_binding(&binding)
            .map_err(RuntimeError::Registry)?;
        Ok(Self {
            template: binding.template,
            registry,
            attach_failures: Vec::new(),
            fragment_params: binding.fragment_params,
            evidence_overrides: binding.evidence_overrides,
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
        let binding_diagnostics = config
            .registry
            .binding_diagnostics(&TemplateBinding {
                template: config.template.clone(),
                fragment_params: config.fragment_params.clone(),
                evidence_overrides: config.evidence_overrides.clone(),
            })
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
            binding_diagnostics,
            fragment_params: config.fragment_params,
            evidence_overrides: config.evidence_overrides,
            facts: Vec::new(),
            rejected_facts: Vec::new(),
            window_end: None,
            frozen_at: None,
        })
    }

    pub fn ingest(&mut self, mut fact: FactEnvelope) {
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
        if fact.fragment_id == "sock_lineage_fragment" && !self.capture_comm_enabled(&fact.fragment_id) {
            if let FactKind::SockLineage(lineage) = &mut fact.kind {
                lineage.comm = [0; 16];
            }
        }
        if self.packet_below_min_len(&fact) {
            self.rejected_facts.push(RejectedFact {
                id: fact.id,
                fragment_id: fact.fragment_id,
                reason: RejectedFactReason::FilteredByFragmentParam,
            });
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
        let program_findings = build_program_findings(
            self.template
                .program_model
                .as_ref()
                .expect("template already validated"),
            &self.binding_diagnostics,
            &self.attach_report,
            &self.rejected_facts,
            &program_flows,
        );
        let module_findings = summarize_module_findings(&program_findings);
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
            binding_diagnostics: self.binding_diagnostics.clone(),
            attach_failure_summary,
            debug_summary: crate::export::DebugSummary {
                fragments_loaded: self.attach_report.fragments_loaded.len() as u64,
                hookpoints_failed: self.attach_report.hookpoints_failed.len() as u64,
                accepted_facts: facts.len() as u64,
                rejected_facts: self.rejected_facts.len() as u64,
                flows: flows.len() as u64,
                program_flows: program_flows.len() as u64,
                program_findings: program_findings.len() as u64,
                module_findings: module_findings.len() as u64,
                reasons: reasons.len() as u64,
                degraded: !self.attach_report.hookpoints_failed.is_empty()
                    || !self.rejected_facts.is_empty(),
            },
            window_profile: self.window_profile.clone(),
            reason_profile_id: self.reason_profile.id().into(),
            reason_profile: self.reason_profile.clone(),
            fragment_params: self.fragment_params.clone(),
            evidence_overrides: self.evidence_overrides.clone(),
            facts,
            rejected_facts: self.rejected_facts.clone(),
            rejected_fact_summary,
            flows,
            program_flows,
            program_findings,
            module_findings,
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

    fn capture_comm_enabled(&self, fragment_id: &str) -> bool {
        self.fragment_params
            .get(fragment_id)
            .and_then(|params| params.get("capture_comm"))
            .map(|value| matches!(value, FragmentParamValue::Bool(true)))
            .unwrap_or(true)
    }

    fn packet_below_min_len(&self, fact: &FactEnvelope) -> bool {
        let FactKind::PacketMeta(packet) = &fact.kind else {
            return false;
        };
        let Some(min_len) = self
            .fragment_params
            .get(&fact.fragment_id)
            .and_then(|params| params.get("min_len"))
            .and_then(|value| match value {
                FragmentParamValue::U64(value) => Some(*value as u32),
                _ => None,
            })
        else {
            return false;
        };
        packet.tot_len < min_len
    }
}

fn build_program_findings(
    model: &crate::program::ProgramModel,
    binding_diagnostics: &BindingDiagnostics,
    attach_report: &AttachReport,
    rejected_facts: &[RejectedFact],
    program_flows: &[ProgramFlow],
) -> Vec<ProgramFinding> {
    let Some(model_diagnostics) = &binding_diagnostics.program_model else {
        return Vec::new();
    };

    let failed_fragments = attach_report
        .hookpoints_failed
        .iter()
        .filter_map(|label| label.split_once('@').map(|(fragment_id, _)| fragment_id))
        .collect::<BTreeSet<_>>();

    program_flows
        .iter()
        .flat_map(|flow| {
            model_diagnostics.rules.iter().filter_map(|rule_diag| {
                if rule_diag.tier != RuleTier::CoreRequirement || !rule_diag.supported {
                    return None;
                }
                let rule = model.rules.get(rule_diag.rule_index)?;
                let signal = rule.signal.as_ref()?;
                if flow.stages.iter().any(|stage| {
                    &stage.kind == signal && stage.phase == rule.phase
                }) {
                    return None;
                }
                if !prior_phase_requirements_satisfied(model, rule_diag.rule_index, flow) {
                    return None;
                }

                let cause = if rule_diag
                    .supporting_fragments
                    .iter()
                    .any(|fragment| failed_fragments.contains(fragment.as_str()))
                {
                    ProgramFindingCause::AttachFailure
                } else if rejected_facts.iter().any(|rejected| {
                    rule_diag
                        .supporting_fragments
                        .iter()
                        .any(|fragment| fragment == &rejected.fragment_id)
                }) {
                    ProgramFindingCause::RejectedEvidence
                } else {
                    ProgramFindingCause::MissingCoreStage
                };

                let suspect_area = suspect_area_for_signal(signal).to_string();
                let phase = rule.phase.clone();
                let phase_transition = phase_transition_for_rule(
                    model,
                    rule_diag.rule_index,
                    flow,
                );
                let module_label = module_label(
                    model.rules.get(rule_diag.rule_index)?.module.as_deref(),
                    &flow.operation,
                    &suspect_area,
                    &rule_diag.supporting_fragments,
                );
                let evidence_trace = build_evidence_trace(
                    signal,
                    attach_report,
                    rejected_facts,
                    flow,
                    &rule_diag.supporting_fragments,
                );
                Some(ProgramFinding {
                    program_flow: flow.id,
                    process: flow.process.clone(),
                    operation: flow.operation.clone(),
                    module_label,
                    phase: phase.clone(),
                    phase_transition: phase_transition.clone(),
                    summary: finding_summary(
                        flow,
                        phase.as_deref(),
                        phase_transition.as_deref(),
                        &suspect_area,
                        &cause,
                    ),
                    suspect_area,
                    cause,
                    supporting_fragments: rule_diag.supporting_fragments.clone(),
                    evidence_trace,
                })
            })
        })
        .collect()
}

fn suspect_area_for_signal(signal: &crate::ir::SignalKind) -> &'static str {
    match signal {
        crate::ir::SignalKind::ProcessBound | crate::ir::SignalKind::ProcessIdentified => {
            "process_binding"
        }
        crate::ir::SignalKind::SocketStateTransition
        | crate::ir::SignalKind::StateChange
        | crate::ir::SignalKind::SynSeen
        | crate::ir::SignalKind::FinOrRst => "socket_state",
        crate::ir::SignalKind::PacketObserved => "transport_io",
        crate::ir::SignalKind::DatagramObserved | crate::ir::SignalKind::UdpDatagramSeen => {
            "datagram_io"
        }
        crate::ir::SignalKind::RouteResolved | crate::ir::SignalKind::RouteChanged => {
            "route_resolution"
        }
    }
}

fn finding_summary(
    flow: &ProgramFlow,
    phase: Option<&str>,
    phase_transition: Option<&str>,
    suspect_area: &str,
    cause: &ProgramFindingCause,
) -> String {
    let scope = flow.process.as_ref().map_or_else(
        || format!("program flow {}", flow.id.0),
        |process| format!("process {} (pid={})", process.comm, process.pid),
    );
    let cause_text = match cause {
        ProgramFindingCause::AttachFailure => "attach failure blocked required evidence",
        ProgramFindingCause::RejectedEvidence => "required evidence was rejected during ingest",
        ProgramFindingCause::MissingCoreStage => "required runtime evidence never materialized",
    };
    let phase_scope = phase
        .map(|phase| format!(" during {phase} phase"))
        .unwrap_or_default();
    let transition_scope = phase_transition
        .map(|transition| format!(" around {transition}"))
        .unwrap_or_default();
    format!(
        "{} may have a {} issue during {:?}{}{}: {}",
        scope, suspect_area, flow.operation, phase_scope, transition_scope, cause_text
    )
}

fn phase_transition_for_rule(
    model: &crate::program::ProgramModel,
    rule_index: usize,
    flow: &ProgramFlow,
) -> Option<String> {
    let current_phase = model.rules.get(rule_index)?.phase.as_ref()?;
    let current_module = model.rules.get(rule_index)?.module.as_deref();
    let previous_phase = model.rules[..rule_index]
        .iter()
        .filter(|rule| rule.phase.is_some() && rule.module.as_deref() == current_module)
        .filter_map(|rule| {
            let signal = rule.signal.as_ref()?;
            flow.stages
                .iter()
                .any(|stage| &stage.kind == signal && stage.phase == rule.phase)
                .then(|| rule.phase.clone())
                .flatten()
        })
        .next_back();

    Some(match previous_phase {
        Some(previous) => format!("{previous}->{current_phase}"),
        None => format!("start->{current_phase}"),
    })
}

fn prior_phase_requirements_satisfied(
    model: &crate::program::ProgramModel,
    rule_index: usize,
    flow: &ProgramFlow,
) -> bool {
    let rule = match model.rules.get(rule_index) {
        Some(rule) => rule,
        None => return false,
    };
    let current_module = rule.module.as_deref();
    let prior_rule = model.rules[..rule_index]
        .iter()
        .rev()
        .find(|candidate| candidate.phase.is_some() && candidate.module.as_deref() == current_module);
    let Some(prior_rule) = prior_rule else {
        return true;
    };
    let Some(signal) = prior_rule.signal.as_ref() else {
        return true;
    };
    flow.stages
        .iter()
        .any(|stage| &stage.kind == signal && stage.phase == prior_rule.phase)
}

fn module_label(
    declared_module: Option<&str>,
    operation: &crate::flow::ProgramOperation,
    suspect_area: &str,
    supporting_fragments: &[String],
) -> String {
    if let Some(module) = declared_module {
        return module.to_string();
    }
    let fragment_scope = if supporting_fragments.is_empty() {
        "unknown_fragment".to_string()
    } else {
        supporting_fragments.join("+")
    };
    format!(
        "{}::{}::{}",
        operation_label(operation),
        suspect_area,
        fragment_scope
    )
}

fn operation_label(operation: &crate::flow::ProgramOperation) -> String {
    match operation {
        crate::flow::ProgramOperation::ConnectFlow => "connect_flow".into(),
        crate::flow::ProgramOperation::DatagramExchange => "datagram_exchange".into(),
        crate::flow::ProgramOperation::Custom(value) => value.clone(),
        crate::flow::ProgramOperation::Unknown => "unknown".into(),
    }
}

fn build_evidence_trace(
    signal: &crate::ir::SignalKind,
    attach_report: &AttachReport,
    rejected_facts: &[RejectedFact],
    flow: &ProgramFlow,
    supporting_fragments: &[String],
) -> Vec<String> {
    let mut trace = vec![format!("missing_signal:{}", signal.id())];

    for stage in &flow.stages {
        trace.push(match &stage.phase {
            Some(phase) => format!("observed_stage:{}:{}@{}", phase, stage.kind.id(), stage.at.0),
            None => format!("observed_stage:{}@{}", stage.kind.id(), stage.at.0),
        });
    }

    for hookpoint in &attach_report.hookpoints_failed {
        if supporting_fragments
            .iter()
            .any(|fragment| hookpoint.starts_with(&format!("{fragment}@")))
        {
            trace.push(format!("failed_hookpoint:{hookpoint}"));
        }
    }

    for rejected in rejected_facts {
        if supporting_fragments
            .iter()
            .any(|fragment| fragment == &rejected.fragment_id)
        {
            trace.push(format!(
                "rejected_fact:{}:{}:{}",
                rejected.id.0,
                rejected.fragment_id,
                rejected.reason.label()
            ));
        }
    }

    trace
}

fn summarize_module_findings(program_findings: &[ProgramFinding]) -> Vec<ModuleFinding> {
    let mut grouped = BTreeMap::<(String, Option<ProcessView>, crate::flow::ProgramOperation), ModuleFinding>::new();

    for finding in program_findings {
        let key = (
            finding.module_label.clone(),
            finding.process.clone(),
            finding.operation.clone(),
        );
        let entry = grouped.entry(key).or_insert_with(|| ModuleFinding {
            module_label: finding.module_label.clone(),
            process: finding.process.clone(),
            operation: finding.operation.clone(),
            severity: ModuleSeverity::Low,
            phases: Vec::new(),
            phase_transitions: Vec::new(),
            suspect_areas: Vec::new(),
            causes: Vec::new(),
            supporting_fragments: Vec::new(),
            program_flows: Vec::new(),
            summaries: Vec::new(),
            evidence_trace: Vec::new(),
        });

        if let Some(phase) = &finding.phase {
            entry.phases.push(phase.clone());
        }
        if let Some(transition) = &finding.phase_transition {
            entry.phase_transitions.push(transition.clone());
        }
        entry.suspect_areas.push(finding.suspect_area.clone());
        entry.causes.push(finding.cause.clone());
        entry.supporting_fragments.extend(finding.supporting_fragments.clone());
        entry.program_flows.push(finding.program_flow);
        entry.summaries.push(finding.summary.clone());
        entry.evidence_trace.extend(finding.evidence_trace.clone());
    }

    let mut findings = grouped
        .into_values()
        .map(|mut finding| {
            finding.suspect_areas.sort();
            finding.suspect_areas.dedup();
            finding.phases.sort();
            finding.phases.dedup();
            finding.phase_transitions.sort();
            finding.phase_transitions.dedup();
            finding.causes.sort_by_key(|cause| match cause {
                ProgramFindingCause::AttachFailure => 0,
                ProgramFindingCause::RejectedEvidence => 1,
                ProgramFindingCause::MissingCoreStage => 2,
            });
            finding.causes.dedup();
            finding.supporting_fragments.sort();
            finding.supporting_fragments.dedup();
            finding.program_flows.sort();
            finding.program_flows.dedup();
            finding.summaries.sort();
            finding.summaries.dedup();
            finding.evidence_trace.sort();
            finding.evidence_trace.dedup();
            finding.severity = module_severity(&finding.causes);
            finding
        })
        .collect::<Vec<_>>();

    findings.sort_by(|a, b| {
        severity_rank(&a.severity)
            .cmp(&severity_rank(&b.severity))
            .then_with(|| a.module_label.cmp(&b.module_label))
    });
    findings
}

fn module_severity(causes: &[ProgramFindingCause]) -> ModuleSeverity {
    if causes.contains(&ProgramFindingCause::AttachFailure) {
        return ModuleSeverity::High;
    }
    if causes.contains(&ProgramFindingCause::RejectedEvidence) {
        return ModuleSeverity::Medium;
    }
    ModuleSeverity::Low
}

fn severity_rank(severity: &ModuleSeverity) -> u8 {
    match severity {
        ModuleSeverity::High => 0,
        ModuleSeverity::Medium => 1,
        ModuleSeverity::Low => 2,
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
                        comm: decode_comm_or_redacted(&lineage.comm),
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

fn decode_comm_or_redacted(comm: &[u8; 16]) -> String {
    let end = comm.iter().position(|byte| *byte == 0).unwrap_or(comm.len());
    if end == 0 {
        "<redacted>".into()
    } else {
        String::from_utf8_lossy(&comm[..end]).to_string()
    }
}
