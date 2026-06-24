use crate::flow::{ProgramOperation, ProgramStageKind};
use crate::fragment::EvidenceTier;
use crate::ledger::FactKindTag;
use crate::program::{ProgramModel, ProgramNarrative, ProgramRule};
use crate::reason::{ReasonModel, ReasonProfile, ReasonRule};
use crate::template::{
    FragmentParamValue, Template, TemplateBinding, WindowProfile, default_5s_window,
    default_program_model_for_reason_profile,
};

use super::predicate::{
    parse_flow_predicate, parse_narrative_template, parse_reason_key_event, parse_reason_narrative,
};
use super::{DslError, parse_bool, split_top_level_with_columns, strip_comments_preserve_layout};

pub(super) fn parse_legacy_str_unvalidated(input: &str) -> Result<TemplateBinding, DslError> {
    let normalized = strip_comments_preserve_layout(input);
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

    for (line_no, raw_line) in normalized.lines().enumerate() {
        let line_no = line_no + 1;
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| DslError::InvalidLine(line.into()).at_line(line_no))?;
        let key = key.trim();
        let value = value.trim();

        match key {
            "template" => template_id = Some(value.to_string()),
            "window" => {
                window_profile =
                    Some(parse_window_profile(value).map_err(|err| err.at_line(line_no))?)
            }
            "window.duration_ms" => {
                inline_window_duration_ms =
                    Some(parse_u64(value, key).map_err(|err| err.at_line(line_no))?);
            }
            "window.lateness_ms" => {
                inline_window_lateness_ms =
                    Some(parse_u64(value, key).map_err(|err| err.at_line(line_no))?);
            }
            "reason" => {
                reason_profile = Some(ReasonProfile::from_id(value).ok_or_else(|| {
                    DslError::InvalidValue(format!("unknown reason profile '{value}'"))
                        .at_line(line_no)
                })?)
            }
            "reason_model" => reason_model_id = Some(value.to_string()),
            "reason.rule" => {
                reason_rules.push(parse_reason_rule(value).map_err(|err| err.at_line(line_no))?)
            }
            "fragment" => fragment_set.push(value.to_string()),
            "program_model" => program_model_id = Some(value.to_string()),
            "operation" => operation = Some(parse_operation(value)),
            "rule" => rules.push(parse_rule(value).map_err(|err| err.at_line(line_no))?),
            "param" => {
                fragment_params.push(parse_param_entry(value).map_err(|err| err.at_line(line_no))?)
            }
            "evidence" => evidence_overrides
                .push(parse_evidence_override(value).map_err(|err| err.at_line(line_no))?),
            other => {
                return Err(
                    DslError::InvalidValue(format!("unknown DSL key '{other}'")).at_line(line_no)
                );
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
        id: Box::leak(template_id.into_boxed_str()),
        fragment_set: fragment_set
            .into_iter()
            .map(|item| Box::leak(item.into_boxed_str()) as &'static str)
            .collect(),
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
            id: "inline",
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
                id: Box::leak(id.into_boxed_str()),
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
        id: Box::leak(id.into_boxed_str()),
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

pub(crate) fn parse_rule(value: &str) -> Result<ProgramRule, DslError> {
    let parts = split_top_level_with_columns(value, ';', 1);
    if !(4..=6).contains(&parts.len()) {
        return Err(DslError::InvalidValue(format!("invalid rule '{value}'"))
            .at_line_column(0, Some(value.len() + 1)));
    }

    Ok(ProgramRule {
        predicate: parse_flow_predicate(&parts[0].1)
            .map_err(|err| err.reanchor_line_column(0, parts[0].0))?,
        signal: parse_stage(&parts[1].1).map_err(|err| err.reanchor_line_column(0, parts[1].0))?,
        narrative: parse_narrative(&parts[2].1),
        dedupe: parse_bool(&parts[3].1).map_err(|err| err.reanchor_line_column(0, parts[3].0))?,
        module: parts.get(4).map(|(_, value)| value.clone()),
        phase: parts.get(5).map(|(_, value)| value.clone()),
    })
}

pub(crate) fn parse_reason_rule(value: &str) -> Result<ReasonRule, DslError> {
    let parts = split_top_level_with_columns(value, ';', 1);
    if !(4..=6).contains(&parts.len()) {
        return Err(
            DslError::InvalidValue(format!("invalid reason rule '{value}'"))
                .at_line_column(0, Some(value.len() + 1)),
        );
    }

    Ok(ReasonRule {
        predicate: parse_flow_predicate(&parts[0].1)
            .map_err(|err| err.reanchor_line_column(0, parts[0].0))?,
        signal: parse_reason_key_event(&parts[1].1)
            .map_err(|err| err.reanchor_line_column(0, parts[1].0))?,
        narrative: parse_reason_narrative(&parts[2].1),
        dedupe: parse_bool(&parts[3].1).map_err(|err| err.reanchor_line_column(0, parts[3].0))?,
        module: parts.get(4).map(|(_, value)| value.clone()),
        phase: parts.get(5).map(|(_, value)| value.clone()),
    })
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

fn parse_narrative(value: &str) -> ProgramNarrative {
    parse_narrative_template(value)
}

fn parse_param_entry(
    value: &str,
) -> Result<(&'static str, &'static str, FragmentParamValue), DslError> {
    let (lhs, rhs) = value
        .split_once('=')
        .ok_or_else(|| DslError::InvalidValue(format!("invalid param '{value}'")))?;
    let (fragment_id, key) = lhs
        .split_once('.')
        .ok_or_else(|| DslError::InvalidValue(format!("invalid param target '{lhs}'")))?;

    Ok((
        Box::leak(fragment_id.trim().to_string().into_boxed_str()),
        Box::leak(key.trim().to_string().into_boxed_str()),
        parse_param_value(rhs.trim())?,
    ))
}

fn parse_evidence_override(value: &str) -> Result<(FactKindTag, EvidenceTier), DslError> {
    let (fact_kind, tier) = value
        .split_once(':')
        .ok_or_else(|| DslError::InvalidValue(format!("invalid evidence override '{value}'")))?;
    let fact_kind = FactKindTag::from_str(fact_kind.trim()).ok_or_else(|| {
        DslError::InvalidValue(format!("unknown evidence fact kind '{}'", fact_kind.trim()))
    })?;
    let tier = match tier.trim() {
        "core_requirement" => EvidenceTier::CoreRequirement,
        "optional_enhancement" => EvidenceTier::OptionalEnhancement,
        other => {
            return Err(DslError::InvalidValue(format!(
                "unknown evidence tier '{other}'"
            )));
        }
    };
    Ok((fact_kind, tier))
}

fn parse_param_value(value: &str) -> Result<FragmentParamValue, DslError> {
    if matches!(value, "true" | "false") {
        return Ok(FragmentParamValue::Bool(parse_bool(value)?));
    }
    if let Ok(value) = value.parse::<u64>() {
        return Ok(FragmentParamValue::U64(value));
    }
    Ok(FragmentParamValue::String(
        value.trim_matches('"').to_string(),
    ))
}

fn parse_u64(value: &str, key: &str) -> Result<u64, DslError> {
    value
        .parse::<u64>()
        .map_err(|_| DslError::InvalidValue(format!("invalid u64 for '{key}': '{value}'")))
}
