use crate::fragment::{builtin_registry, RegistryError};
use crate::flow::{ProgramOperation, ProgramStageKind};
use crate::ir::{FlowPredicate, NarrativeTemplate, SignalKind};
use crate::program::{ProgramModel, ProgramNarrative, ProgramRule};
use crate::reason::{
    ReasonKeyEvent, ReasonModel, ReasonNarrative, ReasonProfile, ReasonRule,
};
use crate::template::{
    default_5s_window, default_program_model_for_reason_profile, FragmentParamValue, Template,
    TemplateBinding, WindowProfile,
};
use std::fs;

#[derive(Debug, Eq, PartialEq)]
pub enum DslError {
    InvalidLine(String),
    MissingField(&'static str),
    InvalidValue(String),
    Registry(RegistryError),
    Io(String),
}

pub fn compile_file(path: &str) -> Result<TemplateBinding, DslError> {
    let input = fs::read_to_string(path).map_err(|err| DslError::Io(err.to_string()))?;
    compile_str(&input)
}

pub fn compile_str(input: &str) -> Result<TemplateBinding, DslError> {
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
    let mut binding: Option<TemplateBinding> = None;

    for raw_line in input.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| DslError::InvalidLine(line.into()))?;
        let key = key.trim();
        let value = value.trim();

        match key {
            "template" => template_id = Some(value.to_string()),
            "window" => window_profile = Some(parse_window_profile(value)?),
            "window.duration_ms" => {
                inline_window_duration_ms = Some(parse_u64(value, key)?);
            }
            "window.lateness_ms" => {
                inline_window_lateness_ms = Some(parse_u64(value, key)?);
            }
            "reason" => {
                reason_profile = Some(
                    ReasonProfile::from_id(value)
                        .ok_or_else(|| DslError::InvalidValue(format!("unknown reason profile '{value}'")))?,
                )
            }
            "reason_model" => reason_model_id = Some(value.to_string()),
            "reason.rule" => reason_rules.push(parse_reason_rule(value)?),
            "fragment" => fragment_set.push(value.to_string()),
            "program_model" => program_model_id = Some(value.to_string()),
            "operation" => operation = Some(parse_operation(value)),
            "rule" => rules.push(parse_rule(value)?),
            "param" => {
                if binding.is_none() {
                    let template_id = template_id
                        .clone()
                        .ok_or(DslError::MissingField("template"))?;
                    let window_profile = build_window_profile(
                        window_profile.clone(),
                        inline_window_duration_ms,
                        inline_window_lateness_ms,
                    )?;
                    let reason_profile = build_reason_profile(
                        &template_id,
                        reason_profile.clone(),
                        reason_model_id.clone(),
                        reason_rules.clone(),
                    )?;
                    binding = Some(Template {
                        id: Box::leak(
                            template_id.clone().into_boxed_str(),
                        ),
                        fragment_set: fragment_set
                            .iter()
                            .map(|item| Box::leak(item.clone().into_boxed_str()) as &'static str)
                            .collect(),
                        window_profile: Some(window_profile),
                        reason_profile: Some(reason_profile.clone()),
                        program_model: Some(build_program_model(
                            &template_id,
                            &reason_profile,
                            program_model_id.clone(),
                            operation.clone(),
                            rules.clone(),
                        )?),
                    }
                    .bind());
                }
                binding = Some(parse_param(
                    binding.take().expect("binding initialized"),
                    value,
                )?);
            }
            other => return Err(DslError::InvalidValue(format!("unknown DSL key '{other}'"))),
        }
    }

    let template_id = template_id.ok_or(DslError::MissingField("template"))?;
    let window_profile = build_window_profile(
        window_profile,
        inline_window_duration_ms,
        inline_window_lateness_ms,
    )?;
    let reason_profile = build_reason_profile(
        &template_id,
        reason_profile,
        reason_model_id,
        reason_rules,
    )?;
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

    let binding = binding.unwrap_or_else(|| template.bind());
    builtin_registry()
        .validate_binding(&binding)
        .map_err(DslError::Registry)?;
    Ok(binding)
}

fn parse_window_profile(value: &str) -> Result<WindowProfile, DslError> {
    match value {
        "default_5s" => Ok(default_5s_window()),
        other => Err(DslError::InvalidValue(format!("unknown window profile '{other}'"))),
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

fn parse_operation(value: &str) -> ProgramOperation {
    match value {
        "connect_flow" => ProgramOperation::ConnectFlow,
        "datagram_exchange" => ProgramOperation::DatagramExchange,
        "unknown" => ProgramOperation::Unknown,
        other => ProgramOperation::Custom(other.into()),
    }
}

fn parse_rule(value: &str) -> Result<ProgramRule, DslError> {
    let parts = split_top_level(value, ';');
    if parts.len() != 4 {
        return Err(DslError::InvalidValue(format!("invalid rule '{value}'")));
    }

    Ok(ProgramRule {
        predicate: parse_flow_predicate(parts[0].trim())?,
        signal: parse_stage(parts[1].trim())?,
        narrative: parse_narrative(parts[2].trim()),
        dedupe: parse_bool(parts[3].trim())?,
    })
}

fn parse_reason_rule(value: &str) -> Result<ReasonRule, DslError> {
    let parts = split_top_level(value, ';');
    if parts.len() != 4 {
        return Err(DslError::InvalidValue(format!("invalid reason rule '{value}'")));
    }

    Ok(ReasonRule {
        predicate: parse_flow_predicate(parts[0].trim())?,
        signal: parse_reason_key_event(parts[1].trim())?,
        narrative: parse_reason_narrative(parts[2].trim()),
        dedupe: parse_bool(parts[3].trim())?,
    })
}

fn parse_stage(value: &str) -> Result<Option<ProgramStageKind>, DslError> {
    Ok(match value {
        "none" => None,
        other => Some(
            SignalKind::from_id(other)
                .ok_or_else(|| DslError::InvalidValue(format!("unknown stage '{other}'")))?,
        ),
    })
}

fn parse_narrative(value: &str) -> ProgramNarrative {
    parse_narrative_template(value)
}

fn parse_flow_predicate(value: &str) -> Result<FlowPredicate, DslError> {
    if let Some(inner) = value
        .strip_prefix("all(")
        .and_then(|rest| rest.strip_suffix(')'))
    {
        return Ok(FlowPredicate::All(
            split_top_level(inner, ',')
                .into_iter()
                .map(|part| parse_flow_predicate(part.trim()))
                .collect::<Result<Vec<_>, _>>()?,
        ));
    }
    if let Some(inner) = value
        .strip_prefix("any(")
        .and_then(|rest| rest.strip_suffix(')'))
    {
        return Ok(FlowPredicate::Any(
            split_top_level(inner, ',')
                .into_iter()
                .map(|part| parse_flow_predicate(part.trim()))
                .collect::<Result<Vec<_>, _>>()?,
        ));
    }

    match value {
        "process_bound" => Ok(FlowPredicate::ProcessBound),
        "socket_state_observed" => Ok(FlowPredicate::SocketStateObserved),
        "route_resolved" => Ok(FlowPredicate::RouteResolved),
        other if other.starts_with("datagram_observed:") => {
            let proto = &other["datagram_observed:".len()..];
            let l4_proto = match proto {
                "udp" => 17,
                "tcp" => 6,
                _ => proto
                    .parse::<u8>()
                    .map_err(|_| DslError::InvalidValue(format!("unknown datagram proto '{proto}'")))?,
            };
            Ok(FlowPredicate::DatagramObserved { l4_proto })
        }
        other => Err(DslError::InvalidValue(format!("unknown predicate '{other}'"))),
    }
}

fn parse_reason_key_event(value: &str) -> Result<Option<ReasonKeyEvent>, DslError> {
    Ok(match value {
        "none" => None,
        other => Some(
            SignalKind::from_id(other).ok_or_else(|| {
                DslError::InvalidValue(format!("unknown reason key event '{other}'"))
            })?,
        ),
    })
}

fn parse_reason_narrative(value: &str) -> ReasonNarrative {
    parse_narrative_template(value)
}

fn parse_narrative_template(value: &str) -> NarrativeTemplate {
    match value {
        "none" => NarrativeTemplate::None,
        "process_bound" => NarrativeTemplate::ProcessBound,
        "tcp_state_transition" => NarrativeTemplate::TcpStateTransition,
        "route_changed" => NarrativeTemplate::RouteChanged,
        "udp_datagram_observed" => NarrativeTemplate::UdpDatagramObserved,
        other if other.starts_with("static:") => {
            NarrativeTemplate::Static(Box::leak(other[7..].to_string().into_boxed_str()))
        }
        other => NarrativeTemplate::Static(Box::leak(other.to_string().into_boxed_str())),
    }
}

fn parse_param(binding: TemplateBinding, value: &str) -> Result<TemplateBinding, DslError> {
    let (lhs, rhs) = value
        .split_once('=')
        .ok_or_else(|| DslError::InvalidValue(format!("invalid param '{value}'")))?;
    let (fragment_id, key) = lhs
        .split_once('.')
        .ok_or_else(|| DslError::InvalidValue(format!("invalid param target '{lhs}'")))?;

    Ok(binding.with_fragment_param(
        Box::leak(fragment_id.trim().to_string().into_boxed_str()),
        Box::leak(key.trim().to_string().into_boxed_str()),
        parse_param_value(rhs.trim())?,
    ))
}

fn parse_param_value(value: &str) -> Result<FragmentParamValue, DslError> {
    if matches!(value, "true" | "false") {
        return Ok(FragmentParamValue::Bool(parse_bool(value)?));
    }
    if let Ok(value) = value.parse::<u64>() {
        return Ok(FragmentParamValue::U64(value));
    }
    Ok(FragmentParamValue::String(value.trim_matches('"').to_string()))
}

fn parse_bool(value: &str) -> Result<bool, DslError> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        other => Err(DslError::InvalidValue(format!("invalid bool '{other}'"))),
    }
}

fn parse_u64(value: &str, key: &str) -> Result<u64, DslError> {
    value
        .parse::<u64>()
        .map_err(|_| DslError::InvalidValue(format!("invalid u64 for '{key}': '{value}'")))
}

fn split_top_level(input: &str, delimiter: char) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;

    for (idx, ch) in input.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ if ch == delimiter && depth == 0 => {
                parts.push(input[start..idx].trim());
                start = idx + ch.len_utf8();
            }
            _ => {}
        }
    }

    parts.push(input[start..].trim());
    parts
}
