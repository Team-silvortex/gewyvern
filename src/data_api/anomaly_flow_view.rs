use crate::render_utils::append_json_string;

use super::ApiTargetSnapshot;
use super::anomaly_phase_hints::phase_hint;

pub(super) fn api_target_anomaly_flow_json(
    target_name: &str,
    target: &ApiTargetSnapshot,
) -> Option<String> {
    if target.analysis_json.is_empty() {
        return None;
    }
    let analysis = AnalysisView::parse(&target.analysis_json)?;
    let focus_flow = analysis
        .protocol_flows
        .iter()
        .find(|flow| flow.status == "attention")
        .or_else(|| analysis.protocol_flows.first());
    let protocol = protocol_hint_key(target_name, target);
    let mut json = String::with_capacity(4096);
    json.push_str("{\"surface\":\"anomaly_flow_view\",\"target\":");
    append_json_string(&mut json, target_name);
    json.push_str(",\"status\":");
    append_json_string(&mut json, &analysis.target_status);
    json.push_str(",\"protocol_surface\":");
    append_protocol_surface_json(&mut json, target);
    json.push_str(",\"summary\":{");
    json.push_str("\"primary_module_family\":");
    append_json_string(&mut json, &analysis.primary_module_family);
    json.push_str(",\"primary_failure_stage\":");
    append_json_string(&mut json, &analysis.primary_failure_stage);
    json.push_str(",\"primary_failure_mode\":");
    append_json_string(&mut json, &analysis.primary_failure_mode);
    json.push_str(",\"primary_failure_detail\":");
    append_json_string(&mut json, &analysis.primary_failure_detail);
    json.push_str(",\"evidence_posture\":");
    append_json_string(&mut json, &analysis.evidence_posture);
    json.push_str(",\"automation_outcome\":");
    append_json_string(&mut json, &analysis.automation_outcome);
    json.push_str(",\"operator_guidance_action\":");
    append_json_string(&mut json, &analysis.operator_guidance_action);
    json.push_str(",\"operator_guidance_reason\":");
    append_json_string(&mut json, &analysis.operator_guidance_reason);
    json.push_str(",\"operator_guidance_summary\":");
    append_json_string(&mut json, &analysis.operator_guidance_summary);
    json.push_str(",\"missing_transitions\":");
    append_string_array_json(&mut json, &analysis.missing_transitions);
    json.push_str(",\"suspect_areas\":");
    append_string_array_json(&mut json, &analysis.suspect_areas);
    json.push_str(",\"suspect_modules\":");
    append_string_array_json(&mut json, &analysis.suspect_modules);
    json.push_str("},\"flow_counts\":{");
    json.push_str("\"total\":");
    json.push_str(&analysis.protocol_flows.len().to_string());
    json.push_str(",\"attention\":");
    json.push_str(
        &analysis
            .protocol_flows
            .iter()
            .filter(|flow| flow.status == "attention")
            .count()
            .to_string(),
    );
    json.push_str(",\"healthy\":");
    json.push_str(
        &analysis
            .protocol_flows
            .iter()
            .filter(|flow| flow.status == "healthy")
            .count()
            .to_string(),
    );
    json.push_str("},\"focus\":");
    append_focus_json(&mut json, focus_flow, &analysis, protocol.as_deref());
    json.push_str(",\"attention_flows\":[");
    let mut first = true;
    for flow in &analysis.protocol_flows {
        if flow.status != "attention" {
            continue;
        }
        if !first {
            json.push(',');
        }
        first = false;
        append_flow_json(&mut json, flow, protocol.as_deref());
    }
    json.push_str("],\"all_flows\":[");
    for (index, flow) in analysis.protocol_flows.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        append_flow_json(&mut json, flow, protocol.as_deref());
    }
    json.push_str("]}");
    Some(json)
}

fn protocol_hint_key(target_name: &str, target: &ApiTargetSnapshot) -> Option<String> {
    let canonical = target
        .protocol_surface
        .as_ref()
        .map(|surface| surface.protocol.as_str());
    let raw = target_name.split(':').nth(1)?;
    match (raw, canonical) {
        ("dot", Some("dns")) | ("doh", Some("http")) => Some(raw.to_string()),
        (_, Some(protocol)) => Some(protocol.to_string()),
        _ => Some(raw.to_string()),
    }
}

struct AnalysisView {
    target_status: String,
    primary_module_family: String,
    primary_failure_stage: String,
    primary_failure_mode: String,
    primary_failure_detail: String,
    evidence_posture: String,
    automation_outcome: String,
    operator_guidance_action: String,
    operator_guidance_reason: String,
    operator_guidance_summary: String,
    missing_transitions: Vec<String>,
    suspect_areas: Vec<String>,
    suspect_modules: Vec<String>,
    protocol_flows: Vec<FlowView>,
}

struct FlowView {
    operation: String,
    network_module_kind: String,
    status: String,
    failure_mode: String,
    failure_detail: String,
    failure_confidence: String,
    failure_basis: String,
    phases: Vec<String>,
    last_phase: Option<String>,
    missing_transitions: Vec<String>,
    suspect_areas: Vec<String>,
}

impl AnalysisView {
    fn parse(input: &str) -> Option<Self> {
        Some(Self {
            target_status: extract_json_string_field(input, "target_status")?,
            primary_module_family: extract_json_string_field(input, "primary_module_family")?,
            primary_failure_stage: extract_json_string_field(input, "primary_failure_stage")?,
            primary_failure_mode: extract_json_string_field(input, "primary_failure_mode")?,
            primary_failure_detail: extract_json_string_field(input, "primary_failure_detail")?,
            evidence_posture: extract_json_string_field(input, "evidence_posture")?,
            automation_outcome: extract_json_string_field(input, "automation_outcome")?,
            operator_guidance_action: extract_json_string_field(input, "operator_guidance_action")
                .unwrap_or_else(|| "manual_review".into()),
            operator_guidance_reason: extract_json_string_field(input, "operator_guidance_reason")
                .unwrap_or_else(|| "heuristic_summary".into()),
            operator_guidance_summary: extract_json_string_field(
                input,
                "operator_guidance_summary",
            )
            .unwrap_or_default(),
            missing_transitions: extract_json_string_array(input, "missing_transitions")
                .unwrap_or_default(),
            suspect_areas: extract_json_string_array(input, "suspect_areas").unwrap_or_default(),
            suspect_modules: extract_json_string_array(input, "suspect_modules")
                .unwrap_or_default(),
            protocol_flows: extract_protocol_flows(input).unwrap_or_default(),
        })
    }
}

fn append_protocol_surface_json(target_json: &mut String, target: &ApiTargetSnapshot) {
    if let Some(surface) = target.protocol_surface.as_ref() {
        target_json.push('{');
        target_json.push_str("\"protocol\":");
        append_json_string(target_json, &surface.protocol);
        target_json.push_str(",\"entry\":");
        append_json_string(target_json, &surface.entry);
        target_json.push_str(",\"default_entry\":");
        append_json_string(target_json, &surface.default_entry);
        target_json.push_str(",\"cluster_key\":");
        if let Some(cluster) = surface.cluster_hint.as_ref() {
            append_json_string(target_json, &cluster.key);
        } else {
            target_json.push_str("null");
        }
        target_json.push('}');
    } else {
        target_json.push_str("null");
    }
}

fn append_focus_json(
    target: &mut String,
    flow: Option<&FlowView>,
    analysis: &AnalysisView,
    protocol: Option<&str>,
) {
    let breakpoint_transition = flow
        .and_then(|flow| flow.missing_transitions.first().cloned())
        .or_else(|| analysis.missing_transitions.first().cloned());
    let breakpoint_stage = flow
        .and_then(|flow| flow.last_phase.clone())
        .unwrap_or_else(|| analysis.primary_failure_stage.clone());
    let suspect_area = flow
        .and_then(|flow| flow.suspect_areas.first().cloned())
        .or_else(|| analysis.suspect_areas.first().cloned());
    target.push('{');
    target.push_str("\"breakpoint_stage\":");
    append_json_string(target, &breakpoint_stage);
    target.push_str(",\"breakpoint_transition\":");
    append_optional_string_json(target, breakpoint_transition.as_deref());
    target.push_str(",\"suspect_area\":");
    append_optional_string_json(target, suspect_area.as_deref());
    target.push_str(",\"next_debug_step\":");
    append_json_string(
        target,
        &next_debug_step(
            breakpoint_transition.as_deref(),
            suspect_area.as_deref(),
            Some(&breakpoint_stage),
            &analysis.primary_failure_mode,
        ),
    );
    target.push_str(",\"breakpoint_hint\":");
    append_json_string(target, &phase_hint(protocol, &breakpoint_stage));
    if let Some(flow) = flow {
        target.push_str(",\"focus_operation\":");
        append_json_string(target, &flow.operation);
        target.push_str(",\"focus_failure_mode\":");
        append_json_string(target, &flow.failure_mode);
    }
    target.push('}');
}

fn next_debug_step(
    breakpoint_transition: Option<&str>,
    suspect_area: Option<&str>,
    stage: Option<&str>,
    failure_mode: &str,
) -> String {
    if let Some(transition) = breakpoint_transition {
        format!("inspect evidence around missing transition {transition}")
    } else if let Some(area) = suspect_area {
        format!("inspect suspect area {area}")
    } else if let Some(stage) = stage {
        format!("inspect runtime evidence around phase {stage}")
    } else {
        format!("inspect primary failure mode {failure_mode}")
    }
}

fn append_flow_json(target: &mut String, flow: &FlowView, protocol: Option<&str>) {
    target.push('{');
    target.push_str("\"operation\":");
    append_json_string(target, &flow.operation);
    target.push_str(",\"network_module_kind\":");
    append_json_string(target, &flow.network_module_kind);
    target.push_str(",\"status\":");
    append_json_string(target, &flow.status);
    target.push_str(",\"failure_mode\":");
    append_json_string(target, &flow.failure_mode);
    target.push_str(",\"failure_detail\":");
    append_json_string(target, &flow.failure_detail);
    target.push_str(",\"failure_confidence\":");
    append_json_string(target, &flow.failure_confidence);
    target.push_str(",\"failure_basis\":");
    append_json_string(target, &flow.failure_basis);
    target.push_str(",\"last_phase\":");
    append_optional_string_json(target, flow.last_phase.as_deref());
    target.push_str(",\"last_phase_hint\":");
    if let Some(phase) = flow.last_phase.as_deref() {
        append_json_string(target, &phase_hint(protocol, phase));
    } else {
        target.push_str("null");
    }
    target.push_str(",\"phases\":");
    append_string_array_json(target, &flow.phases);
    target.push_str(",\"phase_hints\":");
    append_phase_hints_json(target, &flow.phases, protocol);
    target.push_str(",\"missing_transitions\":");
    append_string_array_json(target, &flow.missing_transitions);
    target.push_str(",\"suspect_areas\":");
    append_string_array_json(target, &flow.suspect_areas);
    target.push('}');
}

fn append_string_array_json(target: &mut String, values: &[String]) {
    target.push('[');
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            target.push(',');
        }
        append_json_string(target, value);
    }
    target.push(']');
}

fn append_optional_string_json(target: &mut String, value: Option<&str>) {
    if let Some(value) = value {
        append_json_string(target, value);
    } else {
        target.push_str("null");
    }
}

fn append_phase_hints_json(target: &mut String, phases: &[String], protocol: Option<&str>) {
    target.push('[');
    for (index, phase) in phases.iter().enumerate() {
        if index > 0 {
            target.push(',');
        }
        target.push('{');
        target.push_str("\"phase\":");
        append_json_string(target, phase);
        target.push_str(",\"hint\":");
        append_json_string(target, &phase_hint(protocol, phase));
        target.push('}');
    }
    target.push(']');
}

fn extract_protocol_flows(input: &str) -> Result<Vec<FlowView>, String> {
    let Some(inner) = extract_optional_json_array_contents(input, "protocol_flows")? else {
        return Ok(Vec::new());
    };
    let mut flows = Vec::new();
    for item in split_top_level_json_objects(inner)? {
        flows.push(FlowView {
            operation: extract_json_string_field(item, "operation")
                .ok_or_else(|| "flow missing operation".to_string())?,
            network_module_kind: extract_json_string_field(item, "network_module_kind")
                .ok_or_else(|| "flow missing network_module_kind".to_string())?,
            status: extract_json_string_field(item, "status")
                .ok_or_else(|| "flow missing status".to_string())?,
            failure_mode: extract_json_string_field(item, "failure_mode")
                .ok_or_else(|| "flow missing failure_mode".to_string())?,
            failure_detail: extract_json_string_field(item, "failure_detail")
                .ok_or_else(|| "flow missing failure_detail".to_string())?,
            failure_confidence: extract_json_string_field(item, "failure_confidence")
                .ok_or_else(|| "flow missing failure_confidence".to_string())?,
            failure_basis: extract_json_string_field(item, "failure_basis")
                .ok_or_else(|| "flow missing failure_basis".to_string())?,
            phases: extract_json_string_array(item, "phases").unwrap_or_default(),
            last_phase: extract_json_string_field(item, "last_phase"),
            missing_transitions: extract_json_string_array(item, "missing_transitions")
                .unwrap_or_default(),
            suspect_areas: extract_json_string_array(item, "suspect_areas").unwrap_or_default(),
        });
    }
    Ok(flows)
}

fn extract_json_string_field(input: &str, key: &str) -> Option<String> {
    let needle = format!("\"{}\":", key);
    let start = input.find(&needle)? + needle.len();
    let rest = input[start..].trim_start();
    let mut chars = rest.chars();
    if chars.next()? != '"' {
        return None;
    }
    let mut value = String::new();
    let mut escape = false;
    let mut unicode_remaining = 0usize;
    let mut unicode_buf = String::new();
    for ch in chars {
        if unicode_remaining > 0 {
            unicode_buf.push(ch);
            unicode_remaining -= 1;
            if unicode_remaining == 0 {
                if let Ok(codepoint) = u32::from_str_radix(&unicode_buf, 16)
                    && let Some(decoded) = char::from_u32(codepoint)
                {
                    value.push(decoded);
                }
                unicode_buf.clear();
            }
            continue;
        }
        if escape {
            match ch {
                '"' => value.push('"'),
                '\\' => value.push('\\'),
                '/' => value.push('/'),
                'b' => value.push('\u{08}'),
                'f' => value.push('\u{0C}'),
                'n' => value.push('\n'),
                'r' => value.push('\r'),
                't' => value.push('\t'),
                'u' => unicode_remaining = 4,
                other => value.push(other),
            }
            escape = false;
            continue;
        }
        match ch {
            '\\' => escape = true,
            '"' => return Some(value),
            other => value.push(other),
        }
    }
    None
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

fn split_top_level_json_strings(input: &str) -> Result<Vec<String>, String> {
    let mut values = Vec::new();
    let mut current = String::new();
    let mut in_string = false;
    let mut escape = false;
    for ch in input.chars() {
        if !in_string && (ch == ',' || ch.is_ascii_whitespace()) {
            if ch == ',' && !current.trim().is_empty() {
                values.push(
                    extract_json_string_field(
                        &format!("{{\"value\":{}}}", current.trim()),
                        "value",
                    )
                    .ok_or_else(|| "invalid string array entry".to_string())?,
                );
                current.clear();
            }
            continue;
        }
        current.push(ch);
        if in_string {
            if escape {
                escape = false;
            } else if ch == '\\' {
                escape = true;
            } else if ch == '"' {
                in_string = false;
            }
        } else if ch == '"' {
            in_string = true;
        }
    }
    if !current.trim().is_empty() {
        values.push(
            extract_json_string_field(&format!("{{\"value\":{}}}", current.trim()), "value")
                .ok_or_else(|| "invalid trailing string array entry".to_string())?,
        );
    }
    Ok(values)
}

fn split_top_level_json_objects(input: &str) -> Result<Vec<&str>, String> {
    let bytes = input.as_bytes();
    let mut values = Vec::new();
    let mut in_string = false;
    let mut escape = false;
    let mut depth = 0usize;
    let mut start = None;
    let mut index = 0usize;
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
                '{' => {
                    if depth == 0 {
                        start = Some(index);
                    }
                    depth += 1;
                }
                '}' => {
                    if depth == 0 {
                        return Err("unexpected object terminator".into());
                    }
                    depth -= 1;
                    if depth == 0 {
                        let object_start =
                            start.ok_or_else(|| "missing object start".to_string())?;
                        values.push(&input[object_start..=index]);
                        start = None;
                    }
                }
                _ => {}
            }
        }
        index += 1;
    }
    if depth != 0 {
        return Err("unterminated object list".into());
    }
    Ok(values)
}
