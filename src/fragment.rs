use crate::ledger::FactKindTag;
use crate::reason::ReasonProfile;
use crate::template::TemplateBinding;
use std::collections::{BTreeMap, BTreeSet};

mod builtin;
mod diagnostics;
mod params;
pub use self::builtin::builtin_registry;
pub use self::diagnostics::{
    BindingDiagnostics, ModelDiagnostics, PayloadOffsetSupportSummary, RuleDiagnostics, RuleTier,
};
use self::diagnostics::{
    build_rule_diagnostics, descriptor_sampled_payload_offsets, predicate_payload_offsets,
    validate_model_diagnostics,
};
use self::params::fragment_param_type_matches;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FragmentDescriptor {
    pub id: String,
    pub version: u32,
    pub hookpoints: Vec<HookPoint>,
    pub emits: Vec<FactKindTag>,
    pub evidence_classes: Vec<EvidenceClassSpec>,
    pub requires: Vec<FactKindTag>,
    pub maps: Vec<MapSpec>,
    pub capabilities: Vec<CapabilityFlag>,
    pub sampled_payload_offsets: Vec<u16>,
    pub params: Vec<FragmentParamSpec>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvidenceClassSpec {
    pub fact_kind: FactKindTag,
    pub tier: EvidenceTier,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EvidenceTier {
    CoreRequirement,
    OptionalEnhancement,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FragmentParamSpec {
    pub key: String,
    pub value_type: FragmentParamType,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FragmentParamType {
    Bool,
    U64,
    String,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum HookPoint {
    TracePoint(String),
    KProbe(String),
    TCIngress,
    TCEgress,
}

impl HookPoint {
    pub fn label(&self) -> String {
        match self {
            Self::TracePoint(name) => format!("tracepoint:{name}"),
            Self::KProbe(name) => format!("kprobe:{name}"),
            Self::TCIngress => "tc:ingress".into(),
            Self::TCEgress => "tc:egress".into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MapSpec {
    pub name: String,
    pub kind: MapKind,
    pub max_entries: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MapKind {
    RingBuf,
    Hash,
    LruHash,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum CapabilityFlag {
    TcpState,
    PacketMeta,
    RouteMeta,
    SockLineage,
}

#[derive(Clone, Debug, Default)]
pub struct FragmentRegistry {
    descriptors: BTreeMap<String, FragmentDescriptor>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachPlan {
    pub fragments: Vec<FragmentDescriptor>,
    pub hook_graph: Vec<HookBinding>,
    pub fact_graph: Vec<FactBinding>,
    pub dependency_graph: Vec<DependencyEdge>,
    pub coverage: CoverageReport,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HookBinding {
    pub fragment_id: String,
    pub hookpoint: HookPoint,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachFailure {
    pub fragment_id: String,
    pub hookpoint: HookPoint,
    pub error: String,
}

impl AttachFailure {
    pub fn label(&self) -> String {
        format!("{}@{}", self.fragment_id, self.hookpoint.label())
    }
}

pub fn summarize_attach_failures(
    report: &AttachReport,
) -> Vec<crate::export::AttachFailureSummaryItem> {
    let mut counts = BTreeMap::<&'static str, u64>::new();

    for label in &report.hookpoints_failed {
        let hookpoint_kind = match label
            .split_once('@')
            .and_then(|(_, hookpoint)| hookpoint.split_once(':'))
        {
            Some(("tc", "ingress")) => "tc_ingress",
            Some(("tc", "egress")) => "tc_egress",
            Some(("tracepoint", _)) => "tracepoint",
            Some(("kprobe", _)) => "kprobe",
            _ => "unknown",
        };
        *counts.entry(hookpoint_kind).or_default() += 1;
    }

    counts
        .into_iter()
        .map(
            |(hookpoint_kind, count)| crate::export::AttachFailureSummaryItem {
                hookpoint_kind: hookpoint_kind.into(),
                count,
            },
        )
        .collect()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FactBinding {
    pub fragment_id: String,
    pub emits: Vec<FactKindTag>,
    pub requires: Vec<FactKindTag>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependencyEdge {
    pub fragment_id: String,
    pub depends_on: String,
    pub fact_kind: FactKindTag,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoverageReport {
    pub required: Vec<FactKindTag>,
    pub covered: Vec<FactKindTag>,
    pub missing: Vec<FactKindTag>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachReport {
    pub fragments_loaded: Vec<String>,
    pub hookpoints_attached: Vec<String>,
    pub hookpoints_failed: Vec<String>,
    pub required_fact_kinds_coverage: CoverageReport,
    pub ringbuf_stats: RingBufStats,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RingBufStats {
    pub maps: usize,
    pub total_max_entries: u32,
}

#[derive(Debug, Eq, PartialEq)]
pub enum RegistryError {
    DuplicateFragmentId(String),
    MissingFragment(String),
    HookConflict(String),
    FactConflict(String),
    MissingCoverage(Vec<FactKindTag>),
    UnknownFragmentParam {
        fragment_id: String,
        key: String,
    },
    InvalidFragmentParamType {
        fragment_id: String,
        key: String,
        expected: &'static str,
    },
    MissingRuleEvidence {
        model: String,
        rule_index: usize,
        missing: Vec<FactKindTag>,
    },
    UnsupportedRulePayloadOffsets {
        model: String,
        rule_index: usize,
        offsets: Vec<u16>,
    },
}

impl FragmentRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, descriptor: FragmentDescriptor) -> Result<(), RegistryError> {
        if self.descriptors.contains_key(&descriptor.id) {
            return Err(RegistryError::DuplicateFragmentId(descriptor.id));
        }
        self.descriptors.insert(descriptor.id.clone(), descriptor);
        Ok(())
    }

    pub fn descriptor(&self, id: &str) -> Option<&FragmentDescriptor> {
        self.descriptors.get(id)
    }

    pub fn plan<'a, I>(&self, fragment_ids: I) -> Result<AttachPlan, RegistryError>
    where
        I: IntoIterator<Item = &'a str>,
    {
        let mut fragments = Vec::new();
        for id in fragment_ids {
            let descriptor = self
                .descriptor(id)
                .cloned()
                .ok_or_else(|| RegistryError::MissingFragment(id.into()))?;
            fragments.push(descriptor);
        }

        let mut hook_owners: BTreeMap<HookPoint, String> = BTreeMap::new();
        let mut fact_owners: BTreeMap<FactKindTag, String> = BTreeMap::new();
        let mut hook_graph = Vec::new();
        let mut fact_graph = Vec::new();

        for fragment in &fragments {
            for hookpoint in &fragment.hookpoints {
                if let Some(existing) = hook_owners.insert(hookpoint.clone(), fragment.id.clone()) {
                    return Err(RegistryError::HookConflict(format!(
                        "{} already owned by {}",
                        hookpoint.label(),
                        existing
                    )));
                }
                hook_graph.push(HookBinding {
                    fragment_id: fragment.id.clone(),
                    hookpoint: hookpoint.clone(),
                });
            }

            for emitted in &fragment.emits {
                if let Some(existing) = fact_owners.insert(*emitted, fragment.id.clone()) {
                    return Err(RegistryError::FactConflict(format!(
                        "{} emitted by both {} and {}",
                        emitted, existing, fragment.id
                    )));
                }
            }

            fact_graph.push(FactBinding {
                fragment_id: fragment.id.clone(),
                emits: fragment.emits.clone(),
                requires: fragment.requires.clone(),
            });
        }

        let required: BTreeSet<_> = fragments
            .iter()
            .flat_map(|fragment| fragment.requires.iter().copied())
            .collect();
        let covered: BTreeSet<_> = fragments
            .iter()
            .flat_map(|fragment| fragment.emits.iter().copied())
            .collect();
        let missing: Vec<_> = required.difference(&covered).copied().collect();
        if !missing.is_empty() {
            return Err(RegistryError::MissingCoverage(missing));
        }

        let mut dependency_graph = Vec::new();
        for fragment in &fragments {
            for requirement in &fragment.requires {
                if let Some((producer, _)) = fragments.iter().find_map(|candidate| {
                    candidate
                        .emits
                        .contains(requirement)
                        .then_some((candidate.id.clone(), requirement))
                }) {
                    dependency_graph.push(DependencyEdge {
                        fragment_id: fragment.id.clone(),
                        depends_on: producer,
                        fact_kind: *requirement,
                    });
                }
            }
        }

        Ok(AttachPlan {
            fragments,
            hook_graph,
            fact_graph,
            dependency_graph,
            coverage: CoverageReport {
                required: required.iter().copied().collect(),
                covered: covered.iter().copied().collect(),
                missing,
            },
        })
    }

    pub fn validate_binding_params(&self, binding: &TemplateBinding) -> Result<(), RegistryError> {
        for (fragment_id, params) in &binding.fragment_params {
            let descriptor = self
                .descriptor(fragment_id)
                .ok_or_else(|| RegistryError::MissingFragment(fragment_id.clone()))?;
            for (key, value) in params {
                let spec = descriptor
                    .params
                    .iter()
                    .find(|spec| &spec.key == key)
                    .ok_or_else(|| RegistryError::UnknownFragmentParam {
                        fragment_id: fragment_id.clone(),
                        key: key.clone(),
                    })?;
                let type_matches = fragment_param_type_matches(spec, value);
                if !type_matches {
                    return Err(RegistryError::InvalidFragmentParamType {
                        fragment_id: fragment_id.clone(),
                        key: key.clone(),
                        expected: spec.value_type.label(),
                    });
                }
            }
        }
        Ok(())
    }

    pub fn validate_binding(&self, binding: &TemplateBinding) -> Result<(), RegistryError> {
        self.validate_binding_params(binding)?;
        self.validate_binding_rule_coverage(binding)
    }

    pub fn binding_diagnostics(
        &self,
        binding: &TemplateBinding,
    ) -> Result<BindingDiagnostics, RegistryError> {
        let descriptors = binding
            .template
            .fragment_set
            .iter()
            .map(|fragment_id| {
                self.descriptor(fragment_id)
                    .cloned()
                    .ok_or_else(|| RegistryError::MissingFragment(fragment_id.clone()))
            })
            .collect::<Result<Vec<_>, _>>()?;

        let mut fact_producers = BTreeMap::<FactKindTag, Vec<String>>::new();
        let mut fact_tiers = BTreeMap::<FactKindTag, EvidenceTier>::new();
        for descriptor in &descriptors {
            for fact in &descriptor.emits {
                fact_producers
                    .entry(*fact)
                    .or_default()
                    .push(descriptor.id.clone());
                let tier = descriptor
                    .evidence_classes
                    .iter()
                    .find(|spec| spec.fact_kind == *fact)
                    .map(|spec| spec.tier.clone())
                    .unwrap_or(EvidenceTier::CoreRequirement);
                fact_tiers.entry(*fact).or_insert(tier);
            }
        }
        for (fact_kind, tier) in &binding.evidence_overrides {
            fact_tiers.insert(*fact_kind, tier.clone());
        }

        Ok(BindingDiagnostics {
            program_model: binding
                .template
                .program_model
                .as_ref()
                .map(|model| ModelDiagnostics {
                    model: model.id.clone(),
                    rules: model
                        .rules
                        .iter()
                        .enumerate()
                        .map(|(rule_index, rule)| {
                            build_rule_diagnostics(
                                binding,
                                rule_index,
                                rule,
                                &fact_producers,
                                &fact_tiers,
                                &descriptors,
                            )
                        })
                        .collect(),
                }),
            reason_model: match &binding.template.reason_profile {
                Some(ReasonProfile::Declarative(model)) => Some(ModelDiagnostics {
                    model: model.id.clone(),
                    rules: model
                        .rules
                        .iter()
                        .enumerate()
                        .map(|(rule_index, rule)| {
                            build_rule_diagnostics(
                                binding,
                                rule_index,
                                rule,
                                &fact_producers,
                                &fact_tiers,
                                &descriptors,
                            )
                        })
                        .collect(),
                }),
                _ => None,
            },
        })
    }

    pub fn payload_offset_support_summary(
        &self,
        binding: &TemplateBinding,
        diagnostics: &BindingDiagnostics,
    ) -> PayloadOffsetSupportSummary {
        let sampled_offsets = binding
            .template
            .fragment_set
            .iter()
            .filter_map(|fragment_id| self.descriptor(fragment_id))
            .flat_map(|descriptor| {
                descriptor_sampled_payload_offsets(binding, descriptor)
                    .into_iter()
                    .collect::<Vec<_>>()
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let mut required_offsets = BTreeSet::new();
        if let Some(model) = &binding.template.program_model {
            for rule in &model.rules {
                required_offsets.extend(predicate_payload_offsets(&rule.predicate));
            }
        }
        if let Some(ReasonProfile::Declarative(model)) = &binding.template.reason_profile {
            for rule in &model.rules {
                required_offsets.extend(predicate_payload_offsets(&rule.predicate));
            }
        }
        let unsupported_offsets = diagnostics
            .program_model
            .iter()
            .chain(diagnostics.reason_model.iter())
            .flat_map(|model| model.rules.iter())
            .flat_map(|rule| rule.unsupported_payload_offsets.iter().copied())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();

        PayloadOffsetSupportSummary {
            sampled_offsets,
            required_offsets: required_offsets.into_iter().collect(),
            unsupported_offsets,
        }
    }

    pub fn attach_report(&self, plan: &AttachPlan) -> AttachReport {
        self.attach_report_with_failures(plan, std::iter::empty::<String>())
    }

    pub fn attach_report_with_failure_records<I>(
        &self,
        plan: &AttachPlan,
        failures: I,
    ) -> AttachReport
    where
        I: IntoIterator<Item = AttachFailure>,
    {
        self.attach_report_with_failures(plan, failures.into_iter().map(|failure| failure.label()))
    }

    pub fn attach_report_with_failures<I>(
        &self,
        plan: &AttachPlan,
        failed_hookpoints: I,
    ) -> AttachReport
    where
        I: IntoIterator<Item = String>,
    {
        let failed: BTreeSet<_> = failed_hookpoints.into_iter().collect();
        let mut attached = Vec::new();
        let mut failed_report = Vec::new();
        let mut loaded_fragments = BTreeSet::new();
        let mut matched_failures = BTreeSet::new();

        for binding in &plan.hook_graph {
            let label = format!("{}@{}", binding.fragment_id, binding.hookpoint.label());
            if failed.contains(&label) {
                matched_failures.insert(label.clone());
                failed_report.push(label);
            } else {
                loaded_fragments.insert(&binding.fragment_id);
                attached.push(label);
            }
        }

        for failure in failed.difference(&matched_failures) {
            failed_report.push(failure.clone());
        }

        let maps = plan
            .fragments
            .iter()
            .filter(|fragment| loaded_fragments.contains(&fragment.id))
            .flat_map(|fragment| fragment.maps.iter())
            .filter(|spec| spec.kind == MapKind::RingBuf)
            .collect::<Vec<_>>();

        AttachReport {
            fragments_loaded: plan
                .fragments
                .iter()
                .filter(|fragment| loaded_fragments.contains(&fragment.id))
                .map(|fragment| fragment.id.clone())
                .collect(),
            hookpoints_attached: attached,
            hookpoints_failed: failed_report,
            required_fact_kinds_coverage: plan.coverage.clone(),
            ringbuf_stats: RingBufStats {
                maps: maps.len(),
                total_max_entries: maps.iter().map(|map| map.max_entries).sum(),
            },
        }
    }
}
impl FragmentRegistry {
    fn validate_binding_rule_coverage(
        &self,
        binding: &TemplateBinding,
    ) -> Result<(), RegistryError> {
        let diagnostics = self.binding_diagnostics(binding)?;
        if let Some(model) = diagnostics.program_model {
            validate_model_diagnostics("program_model", &model)?;
        }
        if let Some(model) = diagnostics.reason_model {
            validate_model_diagnostics("reason_model", &model)?;
        }
        Ok(())
    }
}
