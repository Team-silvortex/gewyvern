use super::{EvidenceTier, FactKindTag, FragmentDescriptor, RegistryError};
use crate::ir::{FlowPredicate, NarrativeTemplate, SignalKind};
use crate::reason::ReasonRule;
use crate::template::{FragmentParamValue, TemplateBinding};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BindingDiagnostics {
    pub program_model: Option<ModelDiagnostics>,
    pub reason_model: Option<ModelDiagnostics>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelDiagnostics {
    pub model: String,
    pub rules: Vec<RuleDiagnostics>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuleDiagnostics {
    pub rule_index: usize,
    pub tier: RuleTier,
    pub required_facts: Vec<FactKindTag>,
    pub supporting_fragments: Vec<String>,
    pub missing_facts: Vec<FactKindTag>,
    pub unsupported_payload_offsets: Vec<u16>,
    pub supported: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PayloadOffsetSupportSummary {
    pub sampled_offsets: Vec<u16>,
    pub required_offsets: Vec<u16>,
    pub unsupported_offsets: Vec<u16>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuleTier {
    CoreRequirement,
    OptionalEnhancement,
    Unsupported,
}

pub(super) fn validate_model_diagnostics(
    model_name: &str,
    diagnostics: &ModelDiagnostics,
) -> Result<(), RegistryError> {
    if diagnostics.rules.is_empty() {
        return Ok(());
    }

    if diagnostics.rules.iter().any(|rule| rule.supported) {
        return Ok(());
    }

    let first = diagnostics.rules.first().expect("checked non-empty rules");
    if !first.unsupported_payload_offsets.is_empty() && first.missing_facts.is_empty() {
        return Err(RegistryError::UnsupportedRulePayloadOffsets {
            model: model_name.into(),
            rule_index: first.rule_index,
            offsets: first.unsupported_payload_offsets.clone(),
        });
    }
    Err(RegistryError::MissingRuleEvidence {
        model: model_name.into(),
        rule_index: first.rule_index,
        missing: first.missing_facts.clone(),
    })
}

pub(super) fn build_rule_diagnostics(
    binding: &TemplateBinding,
    rule_index: usize,
    rule: &ReasonRule,
    fact_producers: &BTreeMap<FactKindTag, Vec<String>>,
    fact_tiers: &BTreeMap<FactKindTag, EvidenceTier>,
    descriptors: &[FragmentDescriptor],
) -> RuleDiagnostics {
    let mut required_facts = predicate_required_facts(&rule.predicate);
    required_facts.extend(signal_required_facts(rule.signal.as_ref()));
    required_facts.extend(narrative_required_facts(&rule.narrative));
    required_facts.sort();
    required_facts.dedup();

    let mut supporting_fragments = Vec::new();
    let mut missing_facts = Vec::new();
    for fact in &required_facts {
        match fact_producers.get(fact) {
            Some(producers) if !producers.is_empty() => {
                supporting_fragments.extend(producers.iter().cloned());
            }
            _ => missing_facts.push(*fact),
        }
    }
    supporting_fragments.sort();
    supporting_fragments.dedup();
    let unsupported_payload_offsets =
        unsupported_payload_offsets(binding, &rule.predicate, descriptors, &required_facts);

    RuleDiagnostics {
        rule_index,
        tier: classify_rule_tier(
            &required_facts,
            &missing_facts,
            &unsupported_payload_offsets,
            fact_tiers,
        ),
        required_facts,
        supporting_fragments,
        missing_facts: missing_facts.clone(),
        unsupported_payload_offsets: unsupported_payload_offsets.clone(),
        supported: missing_facts.is_empty() && unsupported_payload_offsets.is_empty(),
    }
}

fn unsupported_payload_offsets(
    binding: &TemplateBinding,
    predicate: &FlowPredicate,
    descriptors: &[FragmentDescriptor],
    required_facts: &[FactKindTag],
) -> Vec<u16> {
    if !required_facts.contains(&FactKindTag::PacketMeta) {
        return Vec::new();
    }
    let required_offsets = predicate_payload_offsets(predicate);
    if required_offsets.is_empty() {
        return Vec::new();
    }
    let available_offsets = descriptors
        .iter()
        .filter(|descriptor| descriptor.emits.contains(&FactKindTag::PacketMeta))
        .flat_map(|descriptor| {
            descriptor_sampled_payload_offsets(binding, descriptor)
                .into_iter()
                .collect::<Vec<_>>()
        })
        .collect::<BTreeSet<_>>();
    required_offsets
        .into_iter()
        .filter(|offset| !available_offsets.contains(offset))
        .collect()
}

pub(super) fn descriptor_sampled_payload_offsets(
    binding: &TemplateBinding,
    descriptor: &FragmentDescriptor,
) -> BTreeSet<u16> {
    let mut offsets = descriptor
        .sampled_payload_offsets
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if let Some(params) = binding.fragment_params.get(&descriptor.id) {
        if let Some(value) = params.get("sample_payload_offsets") {
            match value {
                FragmentParamValue::String(extra) => {
                    offsets.extend(parse_sample_payload_offsets(extra));
                }
                FragmentParamValue::U64(offset) => {
                    if let Ok(offset) = u16::try_from(*offset) {
                        offsets.insert(offset);
                    }
                }
                FragmentParamValue::Bool(_) => {}
            }
        }
    }
    offsets
}

fn parse_sample_payload_offsets(value: &str) -> Vec<u16> {
    value
        .split(',')
        .filter_map(|item| item.trim().parse::<u16>().ok())
        .collect()
}

pub(super) fn predicate_payload_offsets(predicate: &FlowPredicate) -> Vec<u16> {
    predicate.required_payload_offsets()
}

fn classify_rule_tier(
    required_facts: &[FactKindTag],
    missing_facts: &[FactKindTag],
    unsupported_payload_offsets: &[u16],
    fact_tiers: &BTreeMap<FactKindTag, EvidenceTier>,
) -> RuleTier {
    if !missing_facts.is_empty() || !unsupported_payload_offsets.is_empty() {
        return RuleTier::Unsupported;
    }
    if required_facts
        .iter()
        .any(|fact| fact_tiers.get(fact) == Some(&EvidenceTier::OptionalEnhancement))
    {
        return RuleTier::OptionalEnhancement;
    }
    RuleTier::CoreRequirement
}

fn predicate_required_facts(predicate: &FlowPredicate) -> Vec<FactKindTag> {
    predicate.required_fact_kinds()
}

fn signal_required_facts(signal: Option<&SignalKind>) -> Vec<FactKindTag> {
    signal.map_or_else(Vec::new, SignalKind::required_fact_kinds)
}

fn narrative_required_facts(narrative: &NarrativeTemplate) -> Vec<FactKindTag> {
    narrative.required_fact_kinds()
}
