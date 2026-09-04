use super::{
    DslError, parse_bool, parse_flow_predicate,
    predicate::{parse_narrative_template, parse_reason_key_event},
    semantic_values::parse_stage,
};
use gewylang_syntax::PipelineValueKind;

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
