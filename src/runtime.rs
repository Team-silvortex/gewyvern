use crate::export::ExportBundle;
use crate::flow::FlowSnapshot;
use crate::fragment::{
    AttachFailure, AttachPlan, AttachReport, BindingDiagnostics, EvidenceTier, FragmentRegistry,
    RegistryError, builtin_registry, summarize_attach_failures,
};
use crate::ledger::{FactEnvelope, FactId, FactKind, FactKindTag};
use crate::loader::{LinuxProbeLoader, Loader, LoaderError};
use crate::program::build_program_flows;
use crate::reason::{ReasonChain, ReasonProfile, build_reason_chains};
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
        {
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
        let facts = self.materialized_facts();
        build_flow_snapshots(&facts)
    }

    pub fn reasons(&self) -> Vec<ReasonChain> {
        let facts = self.materialized_facts();
        let flows = build_flow_snapshots(&facts);
        build_reason_chains(&self.reason_profile, &flows, &facts)
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
        let protocol_ir = crate::export::infer_protocol_ir(&program_flows);

        ExportBundle {
            template_id: self.template.id.clone(),
            ingest_trust_mode: "unspecified".into(),
            fragment_inventory: self
                .attach_plan
                .fragments
                .iter()
                .map(|fragment| crate::export::FragmentInventoryItem {
                    id: fragment.id.clone(),
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
            protocol_ir,
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
