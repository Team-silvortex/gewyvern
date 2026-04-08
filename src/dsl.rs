use crate::fragment::{builtin_registry, RegistryError};
use crate::flow::{ProgramOperation, ProgramStageKind};
use crate::program::{ProgramModel, ProgramNarrative, ProgramPredicate, ProgramRule};
use crate::reason::ReasonProfile;
use crate::template::{
    default_5s_window, FragmentParamValue, Template, TemplateBinding, WindowProfile,
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
    let mut reason_profile = None;
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
            "reason" => {
                reason_profile = Some(
                    ReasonProfile::from_id(value)
                        .ok_or_else(|| DslError::InvalidValue(format!("unknown reason profile '{value}'")))?,
                )
            }
            "fragment" => fragment_set.push(value.to_string()),
            "program_model" => program_model_id = Some(value.to_string()),
            "operation" => operation = Some(parse_operation(value)),
            "rule" => rules.push(parse_rule(value)?),
            "param" => {
                if binding.is_none() {
                    binding = Some(Template {
                        id: Box::leak(
                            template_id
                                .clone()
                                .ok_or(DslError::MissingField("template"))?
                                .into_boxed_str(),
                        ),
                        fragment_set: fragment_set
                            .iter()
                            .map(|item| Box::leak(item.clone().into_boxed_str()) as &'static str)
                            .collect(),
                        window_profile: Some(
                            window_profile
                                .clone()
                                .ok_or(DslError::MissingField("window"))?,
                        ),
                        reason_profile: Some(
                            reason_profile
                                .clone()
                                .ok_or(DslError::MissingField("reason"))?,
                        ),
                        program_model: Some(ProgramModel {
                            id: Box::leak(
                                program_model_id
                                    .clone()
                                    .ok_or(DslError::MissingField("program_model"))?
                                    .into_boxed_str(),
                            ),
                            operation: operation
                                .clone()
                                .ok_or(DslError::MissingField("operation"))?,
                            rules: rules.clone(),
                        }),
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

    let template = Template {
        id: Box::leak(
            template_id
                .ok_or(DslError::MissingField("template"))?
                .into_boxed_str(),
        ),
        fragment_set: fragment_set
            .into_iter()
            .map(|item| Box::leak(item.into_boxed_str()) as &'static str)
            .collect(),
        window_profile: Some(window_profile.ok_or(DslError::MissingField("window"))?),
        reason_profile: Some(reason_profile.ok_or(DslError::MissingField("reason"))?),
        program_model: Some(ProgramModel {
            id: Box::leak(
                program_model_id
                    .ok_or(DslError::MissingField("program_model"))?
                    .into_boxed_str(),
            ),
            operation: operation.ok_or(DslError::MissingField("operation"))?,
            rules,
        }),
    };

    let binding = binding.unwrap_or_else(|| template.bind());
    builtin_registry()
        .validate_binding_params(&binding)
        .map_err(DslError::Registry)?;
    Ok(binding)
}

fn parse_window_profile(value: &str) -> Result<WindowProfile, DslError> {
    match value {
        "default_5s" => Ok(default_5s_window()),
        other => Err(DslError::InvalidValue(format!("unknown window profile '{other}'"))),
    }
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
        predicate: parse_predicate(parts[0].trim())?,
        stage: parse_stage(parts[1].trim())?,
        narrative: parse_narrative(parts[2].trim()),
        dedupe: parse_bool(parts[3].trim())?,
    })
}

fn parse_stage(value: &str) -> Result<Option<ProgramStageKind>, DslError> {
    Ok(match value {
        "none" => None,
        "process_bound" => Some(ProgramStageKind::ProcessBound),
        "socket_state_transition" => Some(ProgramStageKind::SocketStateTransition),
        "datagram_observed" => Some(ProgramStageKind::DatagramObserved),
        "route_resolved" => Some(ProgramStageKind::RouteResolved),
        other => return Err(DslError::InvalidValue(format!("unknown stage '{other}'"))),
    })
}

fn parse_narrative(value: &str) -> ProgramNarrative {
    match value {
        "none" => ProgramNarrative::None,
        "process_bound" => ProgramNarrative::ProcessBound,
        other if other.starts_with("static:") => {
            ProgramNarrative::Static(Box::leak(other[7..].to_string().into_boxed_str()))
        }
        other => ProgramNarrative::Static(Box::leak(other.to_string().into_boxed_str())),
    }
}

fn parse_predicate(value: &str) -> Result<ProgramPredicate, DslError> {
    if let Some(inner) = value
        .strip_prefix("all(")
        .and_then(|rest| rest.strip_suffix(')'))
    {
        return Ok(ProgramPredicate::All(
            split_top_level(inner, ',')
                .into_iter()
                .map(|part| parse_predicate(part.trim()))
                .collect::<Result<Vec<_>, _>>()?,
        ));
    }
    if let Some(inner) = value
        .strip_prefix("any(")
        .and_then(|rest| rest.strip_suffix(')'))
    {
        return Ok(ProgramPredicate::Any(
            split_top_level(inner, ',')
                .into_iter()
                .map(|part| parse_predicate(part.trim()))
                .collect::<Result<Vec<_>, _>>()?,
        ));
    }

    match value {
        "process_bound" => Ok(ProgramPredicate::ProcessBound),
        "socket_state_observed" => Ok(ProgramPredicate::SocketStateObserved),
        "route_resolved" => Ok(ProgramPredicate::RouteResolved),
        other if other.starts_with("datagram_observed:") => {
            let proto = &other["datagram_observed:".len()..];
            let l4_proto = match proto {
                "udp" => 17,
                "tcp" => 6,
                _ => proto
                    .parse::<u8>()
                    .map_err(|_| DslError::InvalidValue(format!("unknown datagram proto '{proto}'")))?,
            };
            Ok(ProgramPredicate::DatagramObserved { l4_proto })
        }
        other => Err(DslError::InvalidValue(format!("unknown predicate '{other}'"))),
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
