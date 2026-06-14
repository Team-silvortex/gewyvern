use crate::diagnosis_runtime::AnalysisAugmentation;
use crate::render_utils::{append_json_string, extract_json_string_field};

const EXTERNAL_PROTOCOL_FAMILY: &str = "etragon-resident-protocol";
const EXTERNAL_PROTOCOL_MAJOR_VERSION: u32 = 1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ExternalCapabilityProfile {
    pub(super) protocol_family: String,
    pub(super) protocol_version: u32,
    pub(super) safe_automation_hints: Vec<String>,
    pub(super) operator_review_hints: Vec<String>,
    pub(super) handoff_readiness_levels: Vec<String>,
    pub(super) published_contexts: Vec<String>,
    pub(super) forward_compatibility_rules: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SidecarContextKind {
    Enrichment,
    Opinion,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CapabilityAdjustment {
    pub(super) handoff_readiness: String,
    pub(super) merge_hint: String,
    pub(super) capability_status: &'static str,
    pub(super) hint_status: &'static str,
    pub(super) context_status: &'static str,
}

pub(super) fn parse_external_capability_profile(
    input: &str,
) -> Result<ExternalCapabilityProfile, String> {
    let protocol_family = extract_required_json_string(input, "protocol_family")
        .map_err(|_| "missing 'protocol_family' string in capability profile".to_string())?;
    let protocol_version = extract_required_json_u32(input, "protocol_version")
        .map_err(|_| "missing 'protocol_version' number in capability profile".to_string())?;
    let merge_capabilities = extract_required_json_object(input, "merge_capabilities")
        .map_err(|_| "missing 'merge_capabilities' object in capability profile".to_string())?;
    let handoff_capabilities = extract_required_json_object(input, "handoff_capabilities")
        .map_err(|_| "missing 'handoff_capabilities' object in capability profile".to_string())?;
    let context_capabilities = extract_required_json_object(input, "context_capabilities")
        .map_err(|_| "missing 'context_capabilities' object in capability profile".to_string())?;
    let compatibility = extract_required_json_object(input, "compatibility")
        .map_err(|_| "missing 'compatibility' object in capability profile".to_string())?;
    Ok(ExternalCapabilityProfile {
        protocol_family,
        protocol_version,
        safe_automation_hints: extract_json_string_array(
            &merge_capabilities,
            "safe_automation_hints",
        )?,
        operator_review_hints: extract_json_string_array(
            &merge_capabilities,
            "operator_review_hints",
        )?,
        handoff_readiness_levels: extract_json_string_array(
            &handoff_capabilities,
            "readiness_levels",
        )?,
        published_contexts: extract_json_string_array(&context_capabilities, "published_contexts")?,
        forward_compatibility_rules: extract_json_string_array(
            &compatibility,
            "forward_compatibility_rules",
        )?,
    })
}

pub(super) fn adjust_sidecar_metadata(
    capabilities: Option<&ExternalCapabilityProfile>,
    kind: SidecarContextKind,
    raw_handoff_readiness: &str,
    raw_merge_hint: &str,
) -> CapabilityAdjustment {
    let advisory_fallback = fallback_merge_hint(kind, false);
    let review_fallback = fallback_merge_hint(kind, true);
    let Some(profile) = capabilities else {
        return CapabilityAdjustment {
            handoff_readiness: "advisory_only".to_string(),
            merge_hint: advisory_fallback.to_string(),
            capability_status: "unavailable",
            hint_status: "downgraded_unverified_profile",
            context_status: "unavailable",
        };
    };
    if profile.protocol_family != EXTERNAL_PROTOCOL_FAMILY {
        return CapabilityAdjustment {
            handoff_readiness: "advisory_only".to_string(),
            merge_hint: advisory_fallback.to_string(),
            capability_status: "protocol_family_mismatch",
            hint_status: "downgraded_incompatible_profile",
            context_status: "protocol_family_mismatch",
        };
    }
    if profile.protocol_version != EXTERNAL_PROTOCOL_MAJOR_VERSION {
        return CapabilityAdjustment {
            handoff_readiness: "advisory_only".to_string(),
            merge_hint: advisory_fallback.to_string(),
            capability_status: "unsupported_protocol_version",
            hint_status: "downgraded_incompatible_profile",
            context_status: "unsupported_protocol_version",
        };
    }
    let raw_context = sidecar_context_name(kind);
    let context_declared = profile
        .published_contexts
        .iter()
        .any(|item| item == raw_context);
    let readiness_allowed = profile
        .handoff_readiness_levels
        .iter()
        .any(|level| level == raw_handoff_readiness);
    let handoff_readiness = if context_declared && readiness_allowed {
        raw_handoff_readiness.to_string()
    } else {
        "advisory_only".to_string()
    };
    let merge_hint_allowed = profile
        .safe_automation_hints
        .iter()
        .chain(profile.operator_review_hints.iter())
        .any(|hint| hint == raw_merge_hint);
    let merge_hint = if context_declared && merge_hint_allowed {
        raw_merge_hint.to_string()
    } else if profile
        .forward_compatibility_rules
        .iter()
        .any(|rule| rule == "unknown_merge_hints_must_downgrade_to_operator_review")
    {
        review_fallback.to_string()
    } else {
        advisory_fallback.to_string()
    };
    let hint_status = if !context_declared {
        "downgraded_undeclared_context_surface"
    } else if readiness_allowed && merge_hint_allowed {
        "declared"
    } else if merge_hint_allowed {
        "downgraded_unknown_readiness"
    } else if readiness_allowed {
        "downgraded_unknown_hint"
    } else {
        "downgraded_unknown_hint_and_readiness"
    };
    CapabilityAdjustment {
        handoff_readiness,
        merge_hint,
        capability_status: "verified",
        hint_status,
        context_status: if context_declared {
            "declared"
        } else {
            "undeclared_context_surface"
        },
    }
}

pub(super) fn external_capability_note(
    capabilities: Option<&ExternalCapabilityProfile>,
    capability_status: &str,
    hint_status: &str,
    context_status: &str,
) -> Option<AnalysisAugmentation> {
    let summary = match (capability_status, hint_status, context_status) {
        ("verified", "declared", "declared") => {
            "external capability profile verified; sidecar collaboration hints are declared and safe to consume through the current contract"
        }
        ("verified", _, "undeclared_context_surface") => {
            "external capability profile was recognized, but this sidecar context surface was not declared and was downgraded conservatively"
        }
        ("unavailable", _, _) => {
            "external capability profile unavailable; sidecar hints were downgraded to advisory defaults"
        }
        ("protocol_family_mismatch", _, _) => {
            "external capability profile uses a different protocol family; sidecar hints were downgraded to advisory defaults"
        }
        ("unsupported_protocol_version", _, _) => {
            "external capability profile uses an unsupported protocol version; sidecar hints were downgraded to advisory defaults"
        }
        (_, "downgraded_unknown_hint", _) => {
            "external capability profile was recognized, but unknown merge hints were downgraded to operator-review defaults"
        }
        (_, "downgraded_unknown_readiness", _) => {
            "external capability profile was recognized, but unknown readiness labels were downgraded to advisory-only"
        }
        _ => {
            "external capability profile was recognized, but some sidecar collaboration hints were downgraded conservatively"
        }
    };
    Some(AnalysisAugmentation {
        kind: "external-engine".into(),
        name: "external_capability_profile".into(),
        summary: summary.into(),
        confidence: "advisory".into(),
        producer_stage: Some("external".into()),
        producer_pass: Some("external-capability-handshake".into()),
        data_json: Some(format!(
            concat!(
                "{{",
                "\"compatibility_status\":\"{}\",",
                "\"hint_status\":\"{}\",",
                "\"context_status\":\"{}\",",
                "\"protocol_family\":{},",
                "\"protocol_version\":{}",
                "}}"
            ),
            escape_json_string(capability_status),
            escape_json_string(hint_status),
            escape_json_string(context_status),
            optional_json_string(
                capabilities
                    .map(|profile| profile.protocol_family.as_str())
                    .filter(|value| !value.is_empty()),
            ),
            capabilities
                .map(|profile| profile.protocol_version.to_string())
                .unwrap_or_else(|| "null".to_string()),
        )),
    })
}

pub(super) fn object_with_merge_metadata(
    object: &str,
    capabilities: Option<&ExternalCapabilityProfile>,
    adjustment: &CapabilityAdjustment,
) -> String {
    let trimmed = object.trim();
    if !trimmed.ends_with('}') {
        return trimmed.to_string();
    }
    let inner = trimmed.trim_end_matches('}');
    format!(
        concat!(
            "{}{}",
            "\"external_handoff_readiness\":\"{}\",",
            "\"external_merge_hint\":\"{}\",",
            "\"external_capability_status\":\"{}\",",
            "\"external_hint_status\":\"{}\",",
            "\"external_context_status\":\"{}\",",
            "\"external_protocol_family\":{},",
            "\"external_protocol_version\":{}",
            "}}"
        ),
        inner,
        if inner.ends_with('{') { "" } else { "," },
        escape_json_string(&adjustment.handoff_readiness),
        escape_json_string(&adjustment.merge_hint),
        escape_json_string(adjustment.capability_status),
        escape_json_string(adjustment.hint_status),
        escape_json_string(adjustment.context_status),
        optional_json_string(
            capabilities
                .map(|profile| profile.protocol_family.as_str())
                .filter(|value| !value.is_empty()),
        ),
        capabilities
            .map(|profile| profile.protocol_version.to_string())
            .unwrap_or_else(|| "null".to_string()),
    )
}

fn sidecar_context_name(kind: SidecarContextKind) -> &'static str {
    match kind {
        SidecarContextKind::Enrichment => "evidence_chain_enrichment",
        SidecarContextKind::Opinion => "diagnostic_opinion",
    }
}

fn fallback_merge_hint(kind: SidecarContextKind, operator_review: bool) -> &'static str {
    match (kind, operator_review) {
        (SidecarContextKind::Enrichment, false) => "augmentations_only",
        (SidecarContextKind::Enrichment, true) => "augmentations_and_guidance_context",
        (SidecarContextKind::Opinion, false) => "sidecar_only_opinion",
        (SidecarContextKind::Opinion, true) => "sidecar_only_opinion",
    }
}

fn extract_required_json_string(input: &str, key: &str) -> Result<String, String> {
    extract_json_string_field(input, key).ok_or_else(|| format!("missing '{}' string", key))
}

fn extract_required_json_u32(input: &str, key: &str) -> Result<u32, String> {
    extract_optional_json_value(input, key)
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| format!("missing '{}' number", key))
}

fn extract_required_json_object(input: &str, key: &str) -> Result<String, String> {
    let value = extract_optional_json_value(input, key)
        .ok_or_else(|| format!("missing '{}' object", key))?;
    if value.trim_start().starts_with('{') {
        Ok(value)
    } else {
        Err(format!("'{}' is not an object", key))
    }
}

fn extract_json_string_array(input: &str, key: &str) -> Result<Vec<String>, String> {
    let Some(inner) = extract_optional_json_array_contents(input, key)? else {
        return Ok(Vec::new());
    };
    split_top_level_json_strings(inner)
}

fn extract_optional_json_array_contents<'a>(
    input: &'a str,
    key: &str,
) -> Result<Option<&'a str>, String> {
    let needle = format!("\"{}\":[", key);
    let Some(offset) = input.find(&needle) else {
        return Ok(None);
    };
    let start = offset + needle.len();
    let bytes = input.as_bytes();
    let mut depth = 1usize;
    let mut in_string = false;
    let mut escape = false;
    let mut index = start;
    while index < bytes.len() {
        let ch = bytes[index] as char;
        if in_string {
            if escape {
                escape = false;
            } else if ch == '\\' {
                escape = true;
            } else if ch == '"' {
                in_string = false;
            }
        } else {
            match ch {
                '"' => in_string = true,
                '[' => depth += 1,
                ']' => {
                    depth -= 1;
                    if depth == 0 {
                        return Ok(Some(&input[start..index]));
                    }
                }
                _ => {}
            }
        }
        index += 1;
    }
    Err(format!("unterminated '{}' array", key))
}

fn extract_optional_json_value(input: &str, key: &str) -> Option<String> {
    let needle = format!("\"{}\":", key);
    let start = input.find(&needle)? + needle.len();
    let rest = &input[start..];
    let consumed = consume_json_value(rest)?;
    let value = rest[..consumed].trim();
    if value == "null" {
        None
    } else {
        Some(value.to_string())
    }
}

fn consume_json_value(input: &str) -> Option<usize> {
    let bytes = input.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() && (bytes[index] as char).is_ascii_whitespace() {
        index += 1;
    }
    let start = index;
    if start >= bytes.len() {
        return None;
    }
    match bytes[start] as char {
        '"' => {
            index += 1;
            let mut escape = false;
            while index < bytes.len() {
                let ch = bytes[index] as char;
                if escape {
                    escape = false;
                } else if ch == '\\' {
                    escape = true;
                } else if ch == '"' {
                    return Some(index + 1);
                }
                index += 1;
            }
            None
        }
        '{' | '[' => {
            let first = bytes[start] as char;
            let (open, close) = if first == '{' { ('{', '}') } else { ('[', ']') };
            let mut depth = 0usize;
            let mut in_string = false;
            let mut escape = false;
            while index < bytes.len() {
                let ch = bytes[index] as char;
                if in_string {
                    if escape {
                        escape = false;
                    } else if ch == '\\' {
                        escape = true;
                    } else if ch == '"' {
                        in_string = false;
                    }
                } else {
                    match ch {
                        '"' => in_string = true,
                        c if c == open => depth += 1,
                        c if c == close => {
                            depth -= 1;
                            if depth == 0 {
                                return Some(index + 1);
                            }
                        }
                        _ => {}
                    }
                }
                index += 1;
            }
            None
        }
        _ => {
            while index < bytes.len() {
                let ch = bytes[index] as char;
                if ch == ',' || ch == '}' || ch == ']' {
                    break;
                }
                index += 1;
            }
            Some(index.max(start))
        }
    }
}

fn split_top_level_json_strings(input: &str) -> Result<Vec<String>, String> {
    let bytes = input.as_bytes();
    let mut items = Vec::new();
    let mut index = 0usize;
    while index < bytes.len() {
        while index < bytes.len()
            && ((bytes[index] as char).is_ascii_whitespace() || bytes[index] as char == ',')
        {
            index += 1;
        }
        if index >= bytes.len() {
            break;
        }
        if bytes[index] as char != '"' {
            return Err("invalid string array in capability profile".to_string());
        }
        let start = index;
        index += 1;
        let mut escape = false;
        while index < bytes.len() {
            let ch = bytes[index] as char;
            if escape {
                escape = false;
            } else if ch == '\\' {
                escape = true;
            } else if ch == '"' {
                index += 1;
                break;
            }
            index += 1;
        }
        let item = &input[start..index];
        items.push(
            extract_json_string_field(&format!("{{\"value\":{item}}}"), "value")
                .ok_or_else(|| "failed to decode string array item".to_string())?,
        );
    }
    Ok(items)
}

fn escape_json_string(input: &str) -> String {
    let mut escaped = String::new();
    append_json_string(&mut escaped, input);
    escaped
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(escaped.as_str())
        .to_string()
}

fn optional_json_string(value: Option<&str>) -> String {
    value
        .map(|value| {
            let mut json = String::new();
            append_json_string(&mut json, value);
            json
        })
        .unwrap_or_else(|| "null".to_string())
}
