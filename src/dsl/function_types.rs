use super::{
    DslError, PipelineCall, PipelineLetBinding, PipelineParam, parse_bool, parse_flow_predicate,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PipelineValueKind {
    Atom,
    Bool,
    U64,
    Predicate,
    Narrative,
}

pub(super) fn pipeline_value_kind_text(kind: PipelineValueKind) -> &'static str {
    match kind {
        PipelineValueKind::Atom => "atom",
        PipelineValueKind::Bool => "bool",
        PipelineValueKind::U64 => "u64",
        PipelineValueKind::Predicate => "predicate",
        PipelineValueKind::Narrative => "narrative",
    }
}

pub(super) fn validate_pipeline_param_value_kind(
    raw_value: &str,
    kind: PipelineValueKind,
    context: &str,
) -> Result<(), DslError> {
    let normalized = normalize_pipeline_value(raw_value);
    match kind {
        PipelineValueKind::Atom => validate_atom_like_value(raw_value, &normalized, context)?,
        PipelineValueKind::Bool => {
            parse_bool(&normalized).map_err(|_| {
                DslError::InvalidValue(format!(
                    "{context} expects bool-compatible value, got '{raw_value}'"
                ))
            })?;
        }
        PipelineValueKind::U64 => {
            normalized.parse::<u64>().map_err(|_| {
                DslError::InvalidValue(format!(
                    "{context} expects u64-compatible value, got '{raw_value}'"
                ))
            })?;
        }
        PipelineValueKind::Predicate => {
            parse_flow_predicate(&normalized).map_err(|_| {
                DslError::InvalidValue(format!(
                    "{context} expects predicate-compatible value, got '{raw_value}'"
                ))
            })?;
        }
        PipelineValueKind::Narrative => {}
    }
    Ok(())
}

fn validate_atom_like_value(
    raw_value: &str,
    normalized: &str,
    context: &str,
) -> Result<(), DslError> {
    if normalized.is_empty() {
        return Err(DslError::InvalidValue(format!(
            "{context} expects atom-like identifier value, got empty input"
        )));
    }
    if normalized
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | '/' | ':'))
    {
        return Ok(());
    }
    Err(DslError::InvalidValue(format!(
        "{context} expects atom-like identifier value, got '{raw_value}'"
    )))
}

pub(super) fn infer_pipeline_param_kinds(
    params: &[PipelineParam],
    local_bindings: &[PipelineLetBinding],
    body: &[PipelineCall],
) -> Result<BTreeMap<String, PipelineValueKind>, DslError> {
    let param_names = params
        .iter()
        .map(|param| param.name.clone())
        .collect::<BTreeSet<_>>();
    let local_binding_names = local_bindings
        .iter()
        .map(|binding| binding.name.clone())
        .collect::<BTreeSet<_>>();
    let mut requirements = BTreeMap::<String, PipelineValueKind>::new();

    for call in body {
        infer_call_placeholder_kinds(call, &mut requirements)?;
    }

    let mut changed = true;
    while changed {
        changed = false;
        for binding in local_bindings {
            let Some(kind) = requirements.get(&binding.name).copied() else {
                continue;
            };
            for placeholder in placeholders_in(&binding.value) {
                if !param_names.contains(&placeholder)
                    && !local_binding_names.contains(&placeholder)
                {
                    continue;
                }
                if note_requirement(&mut requirements, &placeholder, kind)? {
                    changed = true;
                }
            }
        }
    }

    Ok(requirements
        .into_iter()
        .filter(|(name, _)| param_names.contains(name))
        .collect())
}

fn infer_call_placeholder_kinds(
    call: &PipelineCall,
    output: &mut BTreeMap<String, PipelineValueKind>,
) -> Result<(), DslError> {
    match call.name.as_str() {
        "template" | "fragment" | "operation" | "program_model" => {
            if let Some(arg) = call.args.first() {
                note_placeholders(output, arg, PipelineValueKind::Atom)?;
            }
        }
        "window" => {
            if call.args.len() == 1 && !looks_like_keyword_arg(&call.args[0]) {
                note_placeholders(output, &call.args[0], PipelineValueKind::Atom)?;
            } else {
                for arg in &call.args {
                    if let Some((name, value)) = split_keyword_arg(arg) {
                        match name {
                            "duration_ms" | "lateness_ms" => {
                                note_placeholders(output, value, PipelineValueKind::U64)?;
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        "evidence" => {
            if let Some(arg) = call.args.first() {
                note_placeholders(output, arg, PipelineValueKind::Atom)?;
            }
            if let Some(arg) = call.args.get(1) {
                note_placeholders(output, arg, PipelineValueKind::Atom)?;
            }
        }
        "program_rule" | "reason_rule" => {
            for arg in &call.args {
                if let Some((name, value)) = split_keyword_arg(arg) {
                    match name {
                        "predicate" => {
                            note_placeholders(output, value, PipelineValueKind::Predicate)?
                        }
                        "stage" | "key_event" | "module" | "phase" => {
                            note_placeholders(output, value, PipelineValueKind::Atom)?
                        }
                        "narrative" => {
                            note_placeholders(output, value, PipelineValueKind::Narrative)?
                        }
                        "dedupe" => note_placeholders(output, value, PipelineValueKind::Bool)?,
                        _ => {}
                    }
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn note_placeholders(
    output: &mut BTreeMap<String, PipelineValueKind>,
    value: &str,
    kind: PipelineValueKind,
) -> Result<(), DslError> {
    for placeholder in placeholders_in(value) {
        note_requirement(output, &placeholder, kind)?;
    }
    Ok(())
}

fn note_requirement(
    output: &mut BTreeMap<String, PipelineValueKind>,
    name: &str,
    kind: PipelineValueKind,
) -> Result<bool, DslError> {
    match output.get(name).copied() {
        Some(existing) if existing == kind => Ok(false),
        Some(existing) => Err(DslError::InvalidValue(format!(
            "pipeline parameter '{name}' is inferred inconsistently as both {} and {}",
            pipeline_value_kind_text(existing),
            pipeline_value_kind_text(kind)
        ))),
        None => {
            output.insert(name.to_string(), kind);
            Ok(true)
        }
    }
}

fn placeholders_in(value: &str) -> Vec<String> {
    let mut placeholders = Vec::new();
    let mut rest = value;
    while let Some(start) = rest.find("${") {
        let tail = &rest[start + 2..];
        let Some(end_rel) = tail.find('}') else {
            break;
        };
        let name = tail[..end_rel].trim();
        if !name.is_empty() {
            placeholders.push(name.to_string());
        }
        rest = &tail[end_rel + 1..];
    }
    placeholders
}

fn split_keyword_arg(arg: &str) -> Option<(&str, &str)> {
    let trimmed = arg.trim();
    if trimmed.starts_with(':') || trimmed.starts_with('"') {
        return None;
    }
    let (name, value) = trimmed.split_once(':')?;
    let name = name.trim();
    let value = value.trim();
    if name.is_empty() || value.is_empty() {
        return None;
    }
    Some((name, value))
}

fn looks_like_keyword_arg(arg: &str) -> bool {
    split_keyword_arg(arg).is_some()
}

fn normalize_pipeline_value(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2 {
        trimmed[1..trimmed.len() - 1].to_string()
    } else if let Some(atom) = trimmed.strip_prefix(':') {
        atom.trim().to_string()
    } else {
        trimmed.to_string()
    }
}
