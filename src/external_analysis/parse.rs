use crate::diagnosis_runtime::AnalysisAugmentation;
use crate::render_utils::{append_json_string, extract_json_string_field};

use super::MAX_EXTERNAL_AUGMENTATIONS;
use super::capabilities::{
    ExternalCapabilityProfile, SidecarContextKind, adjust_sidecar_metadata,
    external_capability_note, object_with_merge_metadata,
};

pub(super) fn parse_external_augmentations(
    input: &str,
    capabilities: Option<&ExternalCapabilityProfile>,
) -> Result<Vec<AnalysisAugmentation>, String> {
    let mut items = Vec::new();
    if let Some(inner) = extract_optional_json_array_contents(input, "augmentations")? {
        let objects = split_top_level_json_objects(inner)?;
        for object in objects {
            if items.len() >= MAX_EXTERNAL_AUGMENTATIONS {
                return Err(format!(
                    "external output contains more than {} augmentations",
                    MAX_EXTERNAL_AUGMENTATIONS
                ));
            }
            items.push(AnalysisAugmentation {
                kind: extract_required_json_string(object, "kind")?,
                name: extract_required_json_string(object, "name")?,
                summary: extract_required_json_string(object, "summary")?,
                confidence: extract_required_json_string(object, "confidence")?,
                producer_stage: extract_optional_json_string(object, "producer_stage"),
                producer_pass: extract_optional_json_string(object, "producer_pass"),
                data_json: extract_optional_json_value(object, "data"),
            });
        }
    }
    append_external_merge_hint_augmentations(&mut items, input, capabilities)?;
    if items.is_empty() {
        return Err("missing 'augmentations' array in external output".to_string());
    }
    Ok(items)
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
    Err(format!("unterminated '{}' array in external output", key))
}

fn append_external_merge_hint_augmentations(
    items: &mut Vec<AnalysisAugmentation>,
    input: &str,
    capabilities: Option<&ExternalCapabilityProfile>,
) -> Result<(), String> {
    let mut capability_note = None;
    if let Some(object) = extract_optional_json_value(input, "evidence_chain_enrichment") {
        if items.len() >= MAX_EXTERNAL_AUGMENTATIONS {
            return Err(format!(
                "external output contains more than {} augmentations",
                MAX_EXTERNAL_AUGMENTATIONS
            ));
        }
        let summary = extract_required_json_string(&object, "summary")?;
        let raw_handoff_readiness = extract_optional_json_string(&object, "handoff_readiness")
            .unwrap_or_else(|| "advisory_only".to_string());
        let raw_merge_hint = extract_optional_json_string(&object, "gewyvern_merge_hint")
            .unwrap_or_else(|| "augmentations_only".to_string());
        let adjustment = adjust_sidecar_metadata(
            capabilities,
            SidecarContextKind::Enrichment,
            &raw_handoff_readiness,
            &raw_merge_hint,
        );
        capability_note = capability_note.or_else(|| {
            external_capability_note(
                capabilities,
                adjustment.capability_status,
                adjustment.hint_status,
                adjustment.context_status,
            )
        });
        items.push(AnalysisAugmentation {
            kind: "external-enrichment".into(),
            name: "external_evidence_chain_enrichment".into(),
            summary,
            confidence: external_hint_confidence(&adjustment.handoff_readiness).into(),
            producer_stage: Some("external".into()),
            producer_pass: Some("external-engine-merge-prototype".into()),
            data_json: Some(object_with_merge_metadata(
                &object,
                capabilities,
                &adjustment,
            )),
        });
    }
    if let Some(object) = extract_optional_json_value(input, "diagnostic_opinion")
        && object != "null"
    {
        if items.len() >= MAX_EXTERNAL_AUGMENTATIONS {
            return Err(format!(
                "external output contains more than {} augmentations",
                MAX_EXTERNAL_AUGMENTATIONS
            ));
        }
        let summary = extract_required_json_string(&object, "summary")?;
        let raw_handoff_readiness = extract_optional_json_string(&object, "handoff_readiness")
            .unwrap_or_else(|| "mergeable".to_string());
        let raw_merge_hint = extract_optional_json_string(&object, "gewyvern_merge_hint")
            .unwrap_or_else(|| "sidecar_only_opinion".to_string());
        let adjustment = adjust_sidecar_metadata(
            capabilities,
            SidecarContextKind::Opinion,
            &raw_handoff_readiness,
            &raw_merge_hint,
        );
        capability_note = capability_note.or_else(|| {
            external_capability_note(
                capabilities,
                adjustment.capability_status,
                adjustment.hint_status,
                adjustment.context_status,
            )
        });
        items.push(AnalysisAugmentation {
            kind: "external-opinion".into(),
            name: "external_diagnostic_opinion".into(),
            summary,
            confidence: external_hint_confidence(&adjustment.handoff_readiness).into(),
            producer_stage: Some("external".into()),
            producer_pass: Some("external-engine-merge-prototype".into()),
            data_json: Some(object_with_merge_metadata(
                &object,
                capabilities,
                &adjustment,
            )),
        });
    }
    if let Some(note) = capability_note {
        if items.len() >= MAX_EXTERNAL_AUGMENTATIONS {
            return Err(format!(
                "external output contains more than {} augmentations",
                MAX_EXTERNAL_AUGMENTATIONS
            ));
        }
        items.push(note);
    }
    Ok(())
}

fn external_hint_confidence(handoff_readiness: &str) -> &'static str {
    match handoff_readiness {
        "automation_worthy" => "candidate",
        "mergeable" => "advisory",
        _ => "advisory",
    }
}

fn split_top_level_json_objects(input: &str) -> Result<Vec<&str>, String> {
    let bytes = input.as_bytes();
    let mut objects = Vec::new();
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escape = false;
    let mut start = None;
    for (index, byte) in bytes.iter().enumerate() {
        let ch = *byte as char;
        if in_string {
            if escape {
                escape = false;
            } else if ch == '\\' {
                escape = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
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
                    return Err("invalid external augmentation payload".into());
                }
                depth -= 1;
                if depth == 0 {
                    let object_start = start.ok_or_else(|| {
                        "invalid external augmentation payload: missing object start".to_string()
                    })?;
                    objects.push(&input[object_start..=index]);
                    start = None;
                }
            }
            _ => {}
        }
    }
    if depth != 0 {
        return Err("unterminated external augmentation object".into());
    }
    Ok(objects)
}

fn extract_required_json_string(input: &str, key: &str) -> Result<String, String> {
    extract_optional_json_string(input, key)
        .ok_or_else(|| format!("missing '{}' string in external augmentation", key))
}

fn extract_optional_json_string(input: &str, key: &str) -> Option<String> {
    extract_json_string_field(input, key)
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
    let first = *bytes.get(index)? as char;
    match first {
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

pub(super) fn single_json_string_field(key: &str, value: &str) -> String {
    let mut json = String::new();
    json.push('{');
    append_json_string(&mut json, key);
    json.push(':');
    append_json_string(&mut json, value);
    json.push('}');
    json
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::external_analysis::capabilities::parse_external_capability_profile;

    #[test]
    fn parse_external_augmentations_rejects_too_many_items() {
        let mut payload = String::from("{\"augmentations\":[");
        for index in 0..=MAX_EXTERNAL_AUGMENTATIONS {
            if index > 0 {
                payload.push(',');
            }
            payload.push_str(
                "{\"kind\":\"candidate\",\"name\":\"x\",\"summary\":\"y\",\"confidence\":\"candidate\"}",
            );
        }
        payload.push_str("]}");
        let err = match parse_external_augmentations(&payload, None) {
            Ok(_) => panic!("should reject oversized list"),
            Err(err) => err,
        };
        assert!(err.contains("more than"));
    }

    #[test]
    fn parse_external_augmentations_includes_merge_hint_contexts() {
        let payload = "{\"augmentations\":[{\"kind\":\"ml-candidate\",\"name\":\"ml_candidate_targeted_escalation\",\"summary\":\"external engine suggests targeted escalation\",\"confidence\":\"candidate\"}],\"evidence_chain_enrichment\":{\"status\":\"reinforced\",\"primary_label\":\"targeted_escalation\",\"summary\":\"reinforced evidence chain\",\"handoff_readiness\":\"automation_worthy\",\"gewyvern_merge_hint\":\"augmentations_with_operator_guidance_support\"},\"diagnostic_opinion\":{\"status\":\"ready\",\"diagnosis_kind\":\"direct_protocol_failure\",\"label\":\"targeted_escalation\",\"summary\":\"direct protocol failure is now the most direct opinion\",\"handoff_readiness\":\"automation_worthy\",\"gewyvern_merge_hint\":\"operator_guidance_candidate\"}}";
        let capabilities = parse_external_capability_profile(
            "{\"protocol_family\":\"etragon-resident-protocol\",\"protocol_version\":1,\"merge_capabilities\":{\"safe_automation_hints\":[\"augmentations_only\",\"augmentations_and_guidance_context\"],\"operator_review_hints\":[\"augmentations_with_operator_guidance_support\",\"sidecar_only_opinion\",\"operator_guidance_candidate\"]},\"handoff_capabilities\":{\"readiness_levels\":[\"advisory_only\",\"mergeable\",\"automation_worthy\"]},\"context_capabilities\":{\"published_contexts\":[\"evidence_chain_enrichment\",\"diagnostic_opinion\"]},\"compatibility\":{\"forward_compatibility_rules\":[\"unknown_merge_hints_must_downgrade_to_operator_review\"]}}",
        )
        .expect("capability profile should parse");
        let items = parse_external_augmentations(payload, Some(&capabilities))
            .expect("payload should parse");
        assert_eq!(items.len(), 4);
        assert!(
            items
                .iter()
                .any(|item| item.name == "external_evidence_chain_enrichment"
                    && item.summary == "reinforced evidence chain")
        );
        assert!(
            items
                .iter()
                .any(|item| item.name == "external_diagnostic_opinion"
                    && item.summary == "direct protocol failure is now the most direct opinion")
        );
        let evidence = items
            .iter()
            .find(|item| item.name == "external_evidence_chain_enrichment")
            .expect("synthetic evidence enrichment should exist");
        assert_eq!(evidence.confidence, "candidate");
        assert!(
            evidence.data_json.as_deref().unwrap_or_default().contains(
                "\"external_merge_hint\":\"augmentations_with_operator_guidance_support\""
            )
        );
        let opinion = items
            .iter()
            .find(|item| item.name == "external_diagnostic_opinion")
            .expect("synthetic diagnostic opinion should exist");
        assert_eq!(opinion.confidence, "candidate");
        assert!(
            opinion
                .data_json
                .as_deref()
                .unwrap_or_default()
                .contains("\"external_merge_hint\":\"operator_guidance_candidate\"")
        );
        assert!(
            opinion
                .data_json
                .as_deref()
                .unwrap_or_default()
                .contains("\"external_capability_status\":\"verified\"")
        );
        assert!(
            items
                .iter()
                .any(|item| item.name == "external_capability_profile")
        );
    }

    #[test]
    fn parse_external_augmentations_accepts_merge_only_payload() {
        let payload = "{\"evidence_chain_enrichment\":{\"status\":\"emerging\",\"primary_label\":\"network_observe_longer\",\"summary\":\"still maturing\",\"handoff_readiness\":\"advisory_only\",\"gewyvern_merge_hint\":\"augmentations_only\"}}";
        let items =
            parse_external_augmentations(payload, None).expect("merge-only payload should parse");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].name, "external_evidence_chain_enrichment");
        assert_eq!(items[0].confidence, "advisory");
        assert_eq!(items[1].name, "external_capability_profile");
    }

    #[test]
    fn parse_external_augmentations_decodes_escaped_strings() {
        let payload = r#"{"augmentations":[{"kind":"ml-candidate","name":"quoted","summary":"sidecar said \"wait more\"","confidence":"candidate","producer_stage":"candidate","producer_pass":"worker\\runner"}]}"#;
        let items =
            parse_external_augmentations(payload, None).expect("escaped strings should parse");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].summary, "sidecar said \"wait more\"");
        assert_eq!(items[0].producer_pass.as_deref(), Some("worker\\runner"));
    }

    #[test]
    fn parse_external_capability_profile_extracts_nested_arrays() {
        let profile = parse_external_capability_profile(
            "{\"protocol_family\":\"etragon-resident-protocol\",\"protocol_version\":1,\"merge_capabilities\":{\"safe_automation_hints\":[\"augmentations_only\"],\"operator_review_hints\":[\"sidecar_only_opinion\"]},\"handoff_capabilities\":{\"readiness_levels\":[\"advisory_only\",\"mergeable\"]},\"context_capabilities\":{\"published_contexts\":[\"evidence_chain_enrichment\",\"diagnostic_opinion\"]},\"compatibility\":{\"forward_compatibility_rules\":[\"unknown_merge_hints_must_downgrade_to_operator_review\"]}}",
        )
        .expect("capability profile should parse");
        assert_eq!(profile.protocol_family, "etragon-resident-protocol");
        assert_eq!(profile.protocol_version, 1);
        assert_eq!(profile.safe_automation_hints, vec!["augmentations_only"]);
        assert_eq!(profile.operator_review_hints, vec!["sidecar_only_opinion"]);
        assert_eq!(
            profile.handoff_readiness_levels,
            vec!["advisory_only", "mergeable"]
        );
        assert_eq!(
            profile.published_contexts,
            vec!["evidence_chain_enrichment", "diagnostic_opinion"]
        );
    }

    #[test]
    fn parse_external_augmentations_downgrades_unknown_hints_to_review_defaults() {
        let payload = "{\"diagnostic_opinion\":{\"summary\":\"needs richer synthesis\",\"handoff_readiness\":\"automation_worthy\",\"gewyvern_merge_hint\":\"future_direct_merge\"}}";
        let capabilities = parse_external_capability_profile(
            "{\"protocol_family\":\"etragon-resident-protocol\",\"protocol_version\":1,\"merge_capabilities\":{\"safe_automation_hints\":[\"augmentations_only\"],\"operator_review_hints\":[\"sidecar_only_opinion\"]},\"handoff_capabilities\":{\"readiness_levels\":[\"advisory_only\",\"mergeable\",\"automation_worthy\"]},\"context_capabilities\":{\"published_contexts\":[\"diagnostic_opinion\"]},\"compatibility\":{\"forward_compatibility_rules\":[\"unknown_merge_hints_must_downgrade_to_operator_review\"]}}",
        )
        .expect("capability profile should parse");
        let items = parse_external_augmentations(payload, Some(&capabilities))
            .expect("payload should parse");
        let opinion = items
            .iter()
            .find(|item| item.name == "external_diagnostic_opinion")
            .expect("synthetic opinion should exist");
        let data = opinion.data_json.as_deref().unwrap_or_default();
        assert!(data.contains("\"external_merge_hint\":\"sidecar_only_opinion\""));
        assert!(data.contains("\"external_hint_status\":\"downgraded_unknown_hint\""));
        assert!(
            items
                .iter()
                .any(|item| item.name == "external_capability_profile")
        );
    }

    #[test]
    fn parse_external_augmentations_downgrades_undeclared_context_surface() {
        let payload = "{\"evidence_chain_enrichment\":{\"summary\":\"needs cautious merge\",\"handoff_readiness\":\"mergeable\",\"gewyvern_merge_hint\":\"augmentations_and_guidance_context\"}}";
        let capabilities = parse_external_capability_profile(
            "{\"protocol_family\":\"etragon-resident-protocol\",\"protocol_version\":1,\"merge_capabilities\":{\"safe_automation_hints\":[\"augmentations_only\",\"augmentations_and_guidance_context\"],\"operator_review_hints\":[\"sidecar_only_opinion\"]},\"handoff_capabilities\":{\"readiness_levels\":[\"advisory_only\",\"mergeable\",\"automation_worthy\"]},\"context_capabilities\":{\"published_contexts\":[\"diagnostic_opinion\"]},\"compatibility\":{\"forward_compatibility_rules\":[\"unknown_merge_hints_must_downgrade_to_operator_review\"]}}",
        )
        .expect("capability profile should parse");
        let items = parse_external_augmentations(payload, Some(&capabilities))
            .expect("payload should parse");
        let enrichment = items
            .iter()
            .find(|item| item.name == "external_evidence_chain_enrichment")
            .expect("synthetic enrichment should exist");
        let data = enrichment.data_json.as_deref().unwrap_or_default();
        assert!(data.contains("\"external_handoff_readiness\":\"advisory_only\""));
        assert!(data.contains("\"external_context_status\":\"undeclared_context_surface\""));
        let note = items
            .iter()
            .find(|item| item.name == "external_capability_profile")
            .expect("capability note should exist");
        let note_data = note.data_json.as_deref().unwrap_or_default();
        assert!(note_data.contains("\"context_status\":\"undeclared_context_surface\""));
        assert!(note_data.contains("\"hint_status\":\"downgraded_undeclared_context_surface\""));
    }

    #[test]
    fn single_json_string_field_escapes_quotes() {
        let json = single_json_string_field("message", "sidecar said \"wait\"");
        assert_eq!(json, "{\"message\":\"sidecar said \\\"wait\\\"\"}");
    }
}
