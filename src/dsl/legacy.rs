use crate::flow::{ProgramOperation, ProgramStageKind};
use crate::fragment::EvidenceTier;
use crate::ledger::FactKindTag;
use crate::program::{ProgramModel, ProgramRule};
use crate::reason::{ReasonModel, ReasonProfile, ReasonRule};
use crate::template::{
    FragmentParamValue, Template, TemplateBinding, WindowProfile, default_5s_window,
    default_program_model_for_reason_profile,
};

use super::DslError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CanonicalAssignment {
    pub value: CanonicalAssignmentValue,
    pub line_no: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum CanonicalAssignmentValue {
    Template(String),
    Window(WindowProfile),
    WindowDuration(u64),
    WindowLateness(u64),
    Reason(ReasonProfile),
    ReasonModel(String),
    ReasonRule(ReasonRule),
    Fragment(String),
    ProgramModel(String),
    Operation(ProgramOperation),
    ProgramRule(ProgramRule),
    FragmentParam {
        fragment_id: String,
        key: String,
        value: FragmentParamValue,
    },
    EvidenceOverride {
        fact_kind: FactKindTag,
        tier: EvidenceTier,
    },
}

impl CanonicalAssignment {
    pub fn new(value: CanonicalAssignmentValue, line_no: usize) -> Self {
        Self { value, line_no }
    }
}

pub(super) fn build_binding_from_canonical_assignments(
    assignments: Vec<CanonicalAssignment>,
) -> Result<TemplateBinding, DslError> {
    let mut template_id = None;
    let mut window_profile = None;
    let mut inline_window_duration_ms = None;
    let mut inline_window_lateness_ms = None;
    let mut reason_profile = None;
    let mut reason_model_id = None;
    let mut reason_rules = Vec::new();
    let mut fragment_set = Vec::new();
    let mut program_model_id = None;
    let mut operation = None;
    let mut rules = Vec::new();
    let mut fragment_params = Vec::new();
    let mut evidence_overrides = Vec::new();

    for assignment in assignments {
        match assignment.value {
            CanonicalAssignmentValue::Template(value) => template_id = Some(value),
            CanonicalAssignmentValue::Window(value) => window_profile = Some(value),
            CanonicalAssignmentValue::WindowDuration(value) => {
                inline_window_duration_ms = Some(value)
            }
            CanonicalAssignmentValue::WindowLateness(value) => {
                inline_window_lateness_ms = Some(value)
            }
            CanonicalAssignmentValue::Reason(value) => reason_profile = Some(value),
            CanonicalAssignmentValue::ReasonModel(value) => reason_model_id = Some(value),
            CanonicalAssignmentValue::ReasonRule(value) => reason_rules.push(value),
            CanonicalAssignmentValue::Fragment(value) => fragment_set.push(value),
            CanonicalAssignmentValue::ProgramModel(value) => program_model_id = Some(value),
            CanonicalAssignmentValue::Operation(value) => operation = Some(value),
            CanonicalAssignmentValue::ProgramRule(value) => rules.push(value),
            CanonicalAssignmentValue::FragmentParam {
                fragment_id,
                key,
                value,
            } => fragment_params.push((fragment_id, key, value)),
            CanonicalAssignmentValue::EvidenceOverride { fact_kind, tier } => {
                evidence_overrides.push((fact_kind, tier))
            }
        }
    }

    let template_id = template_id.ok_or(DslError::MissingField("template"))?;
    let window_profile = build_window_profile(
        window_profile,
        inline_window_duration_ms,
        inline_window_lateness_ms,
    )?;
    let reason_profile =
        build_reason_profile(&template_id, reason_profile, reason_model_id, reason_rules)?;
    let program_model = build_program_model(
        &template_id,
        &reason_profile,
        program_model_id,
        operation,
        rules,
    )?;

    let template = Template {
        id: template_id,
        fragment_set,
        window_profile: Some(window_profile),
        reason_profile: Some(reason_profile),
        program_model: Some(program_model),
    };

    let mut binding = template.bind();
    for (fragment_id, key, value) in fragment_params {
        binding = binding.with_fragment_param(fragment_id, key, value);
    }
    for (fact_kind, tier) in evidence_overrides {
        binding = binding.with_evidence_tier(fact_kind, tier);
    }
    Ok(binding)
}

pub(super) fn parse_window_profile(value: &str) -> Result<WindowProfile, DslError> {
    match value {
        "default_5s" => Ok(default_5s_window()),
        other => Err(DslError::InvalidValue(format!(
            "unknown window profile '{other}'"
        ))),
    }
}

fn build_window_profile(
    profile: Option<WindowProfile>,
    duration_ms: Option<u64>,
    lateness_ms: Option<u64>,
) -> Result<WindowProfile, DslError> {
    if let Some(profile) = profile {
        return Ok(profile);
    }
    match (duration_ms, lateness_ms) {
        (Some(duration_ms), Some(lateness_ms)) => Ok(WindowProfile {
            id: "inline".into(),
            duration_ms,
            lateness_ms,
        }),
        (None, None) => Err(DslError::MissingField("window")),
        _ => Err(DslError::MissingField("window")),
    }
}

fn build_program_model(
    template_id: &str,
    reason_profile: &ReasonProfile,
    program_model_id: Option<String>,
    operation: Option<ProgramOperation>,
    rules: Vec<ProgramRule>,
) -> Result<ProgramModel, DslError> {
    match (program_model_id, operation, rules.is_empty()) {
        (None, None, true) => Ok(default_program_model_for_reason_profile(reason_profile)),
        (program_model_id, operation, _) => {
            let operation = operation.ok_or(DslError::MissingField("operation"))?;
            let id = program_model_id.unwrap_or_else(|| format!("{template_id}_dsl_model"));
            Ok(ProgramModel {
                id,
                operation,
                rules,
            })
        }
    }
}

fn build_reason_profile(
    template_id: &str,
    profile: Option<ReasonProfile>,
    reason_model_id: Option<String>,
    reason_rules: Vec<ReasonRule>,
) -> Result<ReasonProfile, DslError> {
    if reason_rules.is_empty() {
        return profile.ok_or(DslError::MissingField("reason"));
    }

    let id = reason_model_id.unwrap_or_else(|| format!("{template_id}_reason_model"));
    Ok(ReasonProfile::Declarative(ReasonModel {
        id,
        rules: reason_rules,
    }))
}

pub(super) fn parse_operation(value: &str) -> ProgramOperation {
    match value {
        "connect_flow" => ProgramOperation::ConnectFlow,
        "datagram_exchange" => ProgramOperation::DatagramExchange,
        "unknown" => ProgramOperation::Unknown,
        other => ProgramOperation::Custom(other.into()),
    }
}

pub(crate) fn parse_stage(value: &str) -> Result<Option<ProgramStageKind>, DslError> {
    Ok(match value {
        "none" => None,
        other => Some(
            crate::ir::SignalKind::from_id(other)
                .ok_or_else(|| DslError::InvalidValue(format!("unknown stage '{other}'")))?,
        ),
    })
}
