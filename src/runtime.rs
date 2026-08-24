use crate::export::ExportBundle;
use crate::flow::FlowSnapshot;
use crate::fragment::{
    AttachFailure, AttachPlan, AttachReport, BindingDiagnostics, EvidenceTier, FragmentRegistry,
    RegistryError, builtin_registry, summarize_attach_failures,
};
use crate::ledger::{FactEnvelope, FactId, FactIndex, FactKind, FactKindTag};
use crate::loader::{LinuxProbeLoader, Loader, LoaderError};
use crate::program::build_program_flows_indexed;
use crate::reason::{ReasonChain, ReasonProfile, build_reason_chains_indexed};
use crate::template::{
    FragmentParamValue, Template, TemplateBinding, TemplateError, WindowProfile,
};
use std::collections::BTreeMap;
use std::time::{Duration, SystemTime};

mod analysis;
mod flows;

use self::analysis::{build_program_findings, summarize_module_findings};
pub use self::flows::build_flow_snapshots;

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

struct ExportState {
    template_id: String,
    attach_plan: AttachPlan,
    attach_report: AttachReport,
    binding_diagnostics: BindingDiagnostics,
    window_profile: WindowProfile,
    reason_profile: ReasonProfile,
    fragment_params: BTreeMap<String, BTreeMap<String, FragmentParamValue>>,
    evidence_overrides: BTreeMap<FactKindTag, EvidenceTier>,
    facts: Vec<FactEnvelope>,
    rejected_facts: Vec<RejectedFact>,
}

struct ExportAnalysis {
    attach_failure_summary: Vec<crate::export::AttachFailureSummaryItem>,
    rejected_fact_summary: Vec<crate::export::RejectedFactSummaryItem>,
    flows: Vec<FlowSnapshot>,
    program_flows: Vec<crate::flow::ProgramFlow>,
    protocol_ir: Vec<crate::export::ProtocolIr>,
    program_findings: Vec<crate::flow::ProgramFinding>,
    module_findings: Vec<crate::flow::ModuleFinding>,
    reasons: Vec<ReasonChain>,
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
    BeforeWindowStart,
    AfterLatenessCutoff,
}

impl RejectedFactReason {
    pub fn label(&self) -> &'static str {
        match self {
            Self::FragmentNotLoaded => "fragment_not_loaded",
            Self::FilteredByFragmentParam => "filtered_by_fragment_param",
            Self::BeforeWindowStart => "before_window_start",
            Self::AfterLatenessCutoff => "after_lateness_cutoff",
        }
    }
}

pub fn summarize_rejected_facts(
    rejected_facts: &[RejectedFact],
) -> Vec<crate::export::RejectedFactSummaryItem> {
    let mut counts = BTreeMap::<(String, &'static str), u64>::new();

    for rejected in rejected_facts {
        *counts
            .entry((rejected.fragment_id.clone(), rejected.reason.label()))
            .or_default() += 1;
    }

    counts
        .into_iter()
        .map(
            |((fragment_id, reason), count)| crate::export::RejectedFactSummaryItem {
                fragment_id,
                reason: reason.into(),
                count,
            },
        )
        .collect()
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
            .plan(config.template.fragment_set.iter().map(String::as_str))
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
        Self::start_with_loader(
            config,
            &LinuxProbeLoader::single_tracepoint_smoke(hookpoint_name),
        )
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
            .plan(config.template.fragment_set.iter().map(String::as_str))
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
        if let Some(window_start) = self.window_start()
            && fact.ts < window_start
        {
            self.reject_fact(fact, RejectedFactReason::BeforeWindowStart);
            return;
        }
        if self.frozen_at.is_some_and(|freeze_at| fact.ts > freeze_at) {
            self.reject_fact(fact, RejectedFactReason::AfterLatenessCutoff);
            return;
        }
        if fact.fragment_id == "sock_lineage_fragment"
            && !self.capture_comm_enabled(&fact.fragment_id)
            && let FactKind::SockLineage(lineage) = &mut fact.kind
        {
            lineage.comm = [0; 16];
        }
        if self.packet_below_min_len(&fact) {
            self.rejected_facts.push(RejectedFact {
                id: fact.id,
                fragment_id: fact.fragment_id,
                reason: RejectedFactReason::FilteredByFragmentParam,
            });
            return;
        }
        self.insert_fact_in_id_order(fact);
    }

    pub fn freeze(&mut self, end: SystemTime) {
        let freeze_at = end + Duration::from_millis(self.window_profile.lateness_ms);
        self.window_end = Some(end);
        self.frozen_at = Some(freeze_at);

        let window_start = self
            .window_start()
            .expect("window start exists after freezing");
        let mut retained = Vec::with_capacity(self.facts.len());
        for fact in std::mem::take(&mut self.facts) {
            if fact.ts < window_start {
                self.reject_fact(fact, RejectedFactReason::BeforeWindowStart);
            } else if fact.ts > freeze_at {
                self.reject_fact(fact, RejectedFactReason::AfterLatenessCutoff);
            } else {
                retained.push(fact);
            }
        }
        self.facts = retained;
    }

    pub fn flow_snapshots(&self) -> Vec<FlowSnapshot> {
        build_flow_snapshots(&self.facts)
    }

    pub fn reasons(&self) -> Vec<ReasonChain> {
        let flows = build_flow_snapshots(&self.facts);
        let fact_index = FactIndex::new(&self.facts);
        build_reason_chains_indexed(&self.reason_profile, &flows, &fact_index)
    }

    pub fn export_bundle(&self) -> ExportBundle {
        let analysis = self.export_analysis();
        build_export_bundle(self.cloned_export_state(), analysis)
    }

    pub fn into_export_bundle(self) -> ExportBundle {
        let analysis = self.export_analysis();
        build_export_bundle(self.into_export_state(), analysis)
    }

    fn export_analysis(&self) -> ExportAnalysis {
        let flows = build_flow_snapshots(&self.facts);
        let fact_index = FactIndex::new(&self.facts);
        let program_model = self
            .template
            .program_model
            .as_ref()
            .expect("template already validated");
        let program_flows = build_program_flows_indexed(program_model, &flows, &fact_index);
        let reasons = build_reason_chains_indexed(&self.reason_profile, &flows, &fact_index);
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
        let protocol_ir = crate::export::infer_protocol_ir(&program_flows);

        ExportAnalysis {
            attach_failure_summary,
            rejected_fact_summary,
            flows,
            program_flows,
            protocol_ir,
            program_findings,
            module_findings,
            reasons,
        }
    }

    fn cloned_export_state(&self) -> ExportState {
        ExportState {
            template_id: self.template.id.clone(),
            attach_plan: self.attach_plan.clone(),
            attach_report: self.attach_report.clone(),
            binding_diagnostics: self.binding_diagnostics.clone(),
            window_profile: self.window_profile.clone(),
            reason_profile: self.reason_profile.clone(),
            fragment_params: self.fragment_params.clone(),
            evidence_overrides: self.evidence_overrides.clone(),
            facts: self.facts.clone(),
            rejected_facts: self.rejected_facts.clone(),
        }
    }

    fn into_export_state(self) -> ExportState {
        ExportState {
            template_id: self.template.id,
            attach_plan: self.attach_plan,
            attach_report: self.attach_report,
            binding_diagnostics: self.binding_diagnostics,
            window_profile: self.window_profile,
            reason_profile: self.reason_profile,
            fragment_params: self.fragment_params,
            evidence_overrides: self.evidence_overrides,
            facts: self.facts,
            rejected_facts: self.rejected_facts,
        }
    }

    pub fn seed_rejected_facts(&mut self, rejected_facts: Vec<RejectedFact>) {
        self.rejected_facts = rejected_facts;
    }

    fn window_start(&self) -> Option<SystemTime> {
        self.window_end.map(|window_end| {
            window_end
                .checked_sub(Duration::from_millis(self.window_profile.duration_ms))
                .unwrap_or(SystemTime::UNIX_EPOCH)
        })
    }

    fn reject_fact(&mut self, fact: FactEnvelope, reason: RejectedFactReason) {
        self.rejected_facts.push(RejectedFact {
            id: fact.id,
            fragment_id: fact.fragment_id,
            reason,
        });
    }

    fn insert_fact_in_id_order(&mut self, fact: FactEnvelope) {
        if self.facts.last().is_none_or(|last| last.id <= fact.id) {
            self.facts.push(fact);
            return;
        }

        // Insert after equal IDs to retain the stable arrival order of the old full sort.
        let index = self
            .facts
            .partition_point(|existing| existing.id <= fact.id);
        self.facts.insert(index, fact);
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

fn build_export_bundle(state: ExportState, analysis: ExportAnalysis) -> ExportBundle {
    let fragment_inventory = state
        .attach_plan
        .fragments
        .iter()
        .map(|fragment| crate::export::FragmentInventoryItem {
            id: fragment.id.clone(),
            version: fragment.version,
        })
        .collect();
    let debug_summary = crate::export::DebugSummary {
        fragments_loaded: state.attach_report.fragments_loaded.len() as u64,
        hookpoints_failed: state.attach_report.hookpoints_failed.len() as u64,
        accepted_facts: state.facts.len() as u64,
        rejected_facts: state.rejected_facts.len() as u64,
        flows: analysis.flows.len() as u64,
        program_flows: analysis.program_flows.len() as u64,
        program_findings: analysis.program_findings.len() as u64,
        module_findings: analysis.module_findings.len() as u64,
        reasons: analysis.reasons.len() as u64,
        degraded: !state.attach_report.hookpoints_failed.is_empty()
            || !state.rejected_facts.is_empty(),
    };
    let reason_profile_id = state.reason_profile.id().into();

    ExportBundle {
        template_id: state.template_id,
        ingest_trust_mode: "unspecified".into(),
        fragment_inventory,
        attach_plan: state.attach_plan,
        attach_report: state.attach_report,
        binding_diagnostics: state.binding_diagnostics,
        attach_failure_summary: analysis.attach_failure_summary,
        debug_summary,
        window_profile: state.window_profile,
        reason_profile_id,
        reason_profile: state.reason_profile,
        fragment_params: state.fragment_params,
        evidence_overrides: state.evidence_overrides,
        facts: state.facts,
        rejected_facts: state.rejected_facts,
        rejected_fact_summary: analysis.rejected_fact_summary,
        flows: analysis.flows,
        program_flows: analysis.program_flows,
        protocol_ir: analysis.protocol_ir,
        program_findings: analysis.program_findings,
        module_findings: analysis.module_findings,
        reasons: analysis.reasons,
    }
}
