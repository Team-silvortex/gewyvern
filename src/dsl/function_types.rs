use super::{
    DslError, PipelineCall, PipelineLetBinding, PipelineParam,
    diagnostics::{
        pipeline_declared_kind_conflict_message, pipeline_inferred_kind_conflict_message,
    },
    legacy::parse_stage,
    parse_bool, parse_flow_predicate,
    predicate::parse_narrative_template,
    predicate::parse_reason_key_event,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PipelineValueKind {
    Atom,
    Bool,
    U64,
    Predicate,
    Narrative,
    Stage,
    KeyEvent,
    Phase,
}

pub(super) fn pipeline_value_kind_text(kind: PipelineValueKind) -> &'static str {
    match kind {
        PipelineValueKind::Atom => "atom",
        PipelineValueKind::Bool => "bool",
        PipelineValueKind::U64 => "u64",
        PipelineValueKind::Predicate => "predicate",
        PipelineValueKind::Narrative => "narrative",
        PipelineValueKind::Stage => "stage",
        PipelineValueKind::KeyEvent => "key_event",
        PipelineValueKind::Phase => "phase",
    }
}

pub(super) fn format_pipeline_param_signature(param: &PipelineParam) -> String {
    let mut rendered = param.name.clone();
    if let Some(kind) = param.declared_kind {
        rendered.push_str(": ");
        rendered.push_str(pipeline_value_kind_text(kind));
    }
    if let Some(default) = &param.default_value {
        rendered.push_str(" = ");
        rendered.push_str(default);
    }
    rendered
}

pub(super) fn format_pipeline_function_signature(
    function_name: &str,
    params: &[PipelineParam],
) -> String {
    let params = params
        .iter()
        .map(format_pipeline_param_signature)
        .collect::<Vec<_>>()
        .join(", ");
    format!("{function_name}({params})")
}

pub(super) fn parse_pipeline_value_kind_name(value: &str) -> Result<PipelineValueKind, DslError> {
    match value.trim() {
        "atom" => Ok(PipelineValueKind::Atom),
        "bool" => Ok(PipelineValueKind::Bool),
        "u64" => Ok(PipelineValueKind::U64),
        "predicate" => Ok(PipelineValueKind::Predicate),
        "narrative" => Ok(PipelineValueKind::Narrative),
        "stage" => Ok(PipelineValueKind::Stage),
        "key_event" => Ok(PipelineValueKind::KeyEvent),
        "phase" => Ok(PipelineValueKind::Phase),
        other => Err(DslError::InvalidValue(format!(
            "unknown pipeline parameter kind '{other}'. Expected one of: atom, bool, u64, predicate, narrative, stage, key_event, phase"
        ))),
    }
}

pub(super) fn resolve_pipeline_param_kind(
    function_signature: &str,
    param_name: &str,
    declared_kind: Option<PipelineValueKind>,
    inferred_kind: Option<PipelineValueKind>,
) -> Result<Option<PipelineValueKind>, DslError> {
    match (declared_kind, inferred_kind) {
        (Some(declared), Some(inferred)) if declared != inferred => Err(DslError::InvalidValue(
            pipeline_declared_kind_conflict_message(
                function_signature,
                param_name,
                pipeline_value_kind_text(declared),
                pipeline_value_kind_text(inferred),
            ),
        )),
        (Some(declared), _) => Ok(Some(declared)),
        (None, inferred) => Ok(inferred),
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
        PipelineValueKind::Narrative => {
            validate_narrative_value(raw_value, &normalized, context)?;
        }
        PipelineValueKind::Stage => {
            parse_stage(&normalized).map_err(|_| {
                DslError::InvalidValue(format!(
                    "{context} expects stage-compatible value, got '{raw_value}'"
                ))
            })?;
        }
        PipelineValueKind::KeyEvent => {
            parse_reason_key_event(&normalized).map_err(|_| {
                DslError::InvalidValue(format!(
                    "{context} expects key_event-compatible value, got '{raw_value}'"
                ))
            })?;
        }
        PipelineValueKind::Phase => {
            validate_phase_value(raw_value, &normalized, context)?;
        }
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

fn validate_narrative_value(
    raw_value: &str,
    normalized: &str,
    context: &str,
) -> Result<(), DslError> {
    if normalized.is_empty() {
        return Err(DslError::InvalidValue(format!(
            "{context} expects narrative-compatible value, got empty input"
        )));
    }
    if is_known_narrative_template(normalized) || normalized.starts_with("static:") {
        let _ = parse_narrative_template(normalized);
        return Ok(());
    }
    Err(DslError::InvalidValue(format!(
        "{context} expects narrative-compatible value, got '{raw_value}'. Use a built-in narrative template or explicit static:... text"
    )))
}

fn is_known_narrative_template(value: &str) -> bool {
    matches!(
        value,
        "none"
            | "process_bound"
            | "packet_observed"
            | "transport_payload_sent"
            | "transport_payload_received"
            | "tcp_state_transition"
            | "route_changed"
            | "udp_datagram_observed"
            | "udp_datagram_sent"
            | "udp_datagram_received"
    )
}

fn validate_phase_value(raw_value: &str, normalized: &str, context: &str) -> Result<(), DslError> {
    if normalized.is_empty() {
        return Err(DslError::InvalidValue(format!(
            "{context} expects phase-compatible value, got empty input"
        )));
    }
    let mut chars = normalized.chars();
    let Some(first) = chars.next() else {
        return Err(DslError::InvalidValue(format!(
            "{context} expects phase-compatible value, got empty input"
        )));
    };
    if !first.is_ascii_lowercase() {
        return Err(DslError::InvalidValue(format!(
            "{context} expects phase-compatible value, got '{raw_value}'. Use lowercase snake_case phase names"
        )));
    }
    if chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
        && !normalized.ends_with('_')
        && !normalized.contains("__")
    {
        return Ok(());
    }
    Err(DslError::InvalidValue(format!(
        "{context} expects phase-compatible value, got '{raw_value}'. Use lowercase snake_case phase names"
    )))
}

pub(super) fn infer_pipeline_param_kinds(
    function_signature: &str,
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
        infer_call_placeholder_kinds(function_signature, call, &mut requirements)?;
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
                if note_requirement(function_signature, &mut requirements, &placeholder, kind)? {
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
    function_signature: &str,
    call: &PipelineCall,
    output: &mut BTreeMap<String, PipelineValueKind>,
) -> Result<(), DslError> {
    match call.name.as_str() {
        "template" | "fragment" | "operation" | "program_model" => {
            if let Some(arg) = call.args.first() {
                note_placeholders(function_signature, output, arg, PipelineValueKind::Atom)?;
            }
        }
        "window" => {
            if call.args.len() == 1 && !looks_like_keyword_arg(&call.args[0]) {
                note_placeholders(
                    function_signature,
                    output,
                    &call.args[0],
                    PipelineValueKind::Atom,
                )?;
            } else {
                for arg in &call.args {
                    if let Some((name, value)) = split_keyword_arg(arg) {
                        match name {
                            "duration_ms" | "lateness_ms" => {
                                note_placeholders(
                                    function_signature,
                                    output,
                                    value,
                                    PipelineValueKind::U64,
                                )?;
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        "evidence" => {
            if let Some(arg) = call.args.first() {
                note_placeholders(function_signature, output, arg, PipelineValueKind::Atom)?;
            }
            if let Some(arg) = call.args.get(1) {
                note_placeholders(function_signature, output, arg, PipelineValueKind::Atom)?;
            }
        }
        "program_rule" | "reason_rule" => {
            for arg in &call.args {
                if let Some((name, value)) = split_keyword_arg(arg) {
                    match name {
                        "predicate" => note_placeholders(
                            function_signature,
                            output,
                            value,
                            PipelineValueKind::Predicate,
                        )?,
                        "stage" => note_placeholders(
                            function_signature,
                            output,
                            value,
                            PipelineValueKind::Stage,
                        )?,
                        "key_event" => note_placeholders(
                            function_signature,
                            output,
                            value,
                            PipelineValueKind::KeyEvent,
                        )?,
                        "module" => note_placeholders(
                            function_signature,
                            output,
                            value,
                            PipelineValueKind::Atom,
                        )?,
                        "phase" => note_placeholders(
                            function_signature,
                            output,
                            value,
                            PipelineValueKind::Phase,
                        )?,
                        "narrative" => note_placeholders(
                            function_signature,
                            output,
                            value,
                            PipelineValueKind::Narrative,
                        )?,
                        "dedupe" => note_placeholders(
                            function_signature,
                            output,
                            value,
                            PipelineValueKind::Bool,
                        )?,
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
    function_signature: &str,
    output: &mut BTreeMap<String, PipelineValueKind>,
    value: &str,
    kind: PipelineValueKind,
) -> Result<(), DslError> {
    for placeholder in placeholders_in(value) {
        note_requirement(function_signature, output, &placeholder, kind)?;
    }
    Ok(())
}

fn note_requirement(
    function_signature: &str,
    output: &mut BTreeMap<String, PipelineValueKind>,
    name: &str,
    kind: PipelineValueKind,
) -> Result<bool, DslError> {
    match output.get(name).copied() {
        Some(existing) if existing == kind => Ok(false),
        Some(existing) => Err(DslError::InvalidValue(
            pipeline_inferred_kind_conflict_message(
                function_signature,
                name,
                pipeline_value_kind_text(existing),
                pipeline_value_kind_text(kind),
            ),
        )),
        None => {
            output.insert(name.to_string(), kind);
            Ok(true)
        }
    }
}

fn placeholders_in(value: &str) -> Vec<String> {
    let mut placeholders = Vec::new();
    let chars = value.char_indices().collect::<Vec<_>>();
    let mut index = 0usize;

    while index < chars.len() {
        if chars[index].1 != '$' {
            index += 1;
            continue;
        }

        let Some((_, next_ch)) = chars.get(index + 1).copied() else {
            break;
        };

        if next_ch == '{' {
            let mut end_index = index + 2;
            while end_index < chars.len() && chars[end_index].1 != '}' {
                end_index += 1;
            }
            if end_index == chars.len() {
                break;
            }
            let name_start = chars[index + 2].0;
            let name_end = chars[end_index].0;
            let name = value[name_start..name_end].trim();
            if !name.is_empty() {
                placeholders.push(name.to_string());
            }
            index = end_index + 1;
            continue;
        }

        if is_pipeline_placeholder_char(next_ch) {
            let start_index = index + 1;
            let mut end_index = start_index + 1;
            while end_index < chars.len() && is_pipeline_placeholder_char(chars[end_index].1) {
                end_index += 1;
            }
            let name_start = chars[start_index].0;
            let name_end = chars
                .get(end_index)
                .map(|(pos, _)| *pos)
                .unwrap_or(value.len());
            placeholders.push(value[name_start..name_end].to_string());
            index = end_index;
            continue;
        }

        index += 1;
    }
    placeholders
}

fn is_pipeline_placeholder_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-')
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
