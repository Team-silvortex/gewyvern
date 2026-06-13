use std::collections::HashMap;

use crate::render_utils::{append_json_string, append_string_list_json};

use super::{ApiSnapshot, ApiTargetSnapshot};

pub(crate) fn api_snapshot_meta_json(snapshot: &ApiSnapshot) -> String {
    let mut json = String::with_capacity(estimate_api_snapshot_meta_capacity(snapshot));
    json.push_str("{\"updated_unix_ms\":");
    json.push_str(&snapshot.updated_unix_ms.to_string());
    json.push_str(", ");
    append_api_snapshot_index_fields_json(&mut json, snapshot);
    json.push_str(", ");
    append_api_snapshot_presence_fields_json(&mut json, snapshot);
    json.push('}');
    json
}

pub(super) fn api_target_list_json(snapshot: &ApiSnapshot) -> String {
    let mut json = String::with_capacity(estimate_api_target_list_capacity(snapshot));
    json.push('{');
    append_api_snapshot_index_fields_json(&mut json, snapshot);
    json.push_str(",\"targets\":");
    append_string_list_json(&mut json, &snapshot.target_names);
    json.push_str(",\"path_segment_encoding\":\"percent-encoding\",\"direct_path_chars\":\"A-Z a-z 0-9 . _ ~ :\"}");
    json
}

pub(super) fn json_string(value: &str) -> String {
    let mut json = String::new();
    append_json_string(&mut json, value);
    json
}

pub(super) fn api_target_path_segment(name: &str) -> String {
    let mut out = String::new();
    for byte in name.bytes() {
        if is_api_target_direct_path_char(byte) {
            out.push(byte as char);
        } else {
            out.push('%');
            out.push_str(&format!("{:02X}", byte));
        }
    }
    out
}

pub(super) fn decode_api_target_path_segment(segment: &str) -> Result<String, &'static str> {
    let mut bytes = Vec::with_capacity(segment.len());
    let raw = segment.as_bytes();
    let mut index = 0;
    while index < raw.len() {
        if raw[index] == b'%' {
            if index + 2 >= raw.len() {
                return Err("target path segment ends with an incomplete percent-encoding");
            }
            let hi = (raw[index + 1] as char)
                .to_digit(16)
                .ok_or("target path segment contains an invalid percent-encoding")?;
            let lo = (raw[index + 2] as char)
                .to_digit(16)
                .ok_or("target path segment contains an invalid percent-encoding")?;
            bytes.push(((hi << 4) | lo) as u8);
            index += 3;
        } else {
            bytes.push(raw[index]);
            index += 1;
        }
    }
    String::from_utf8(bytes).map_err(|_| "target path segment is not valid UTF-8")
}

fn is_api_target_direct_path_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~' | b':')
}

fn append_api_target_refs_json(
    target: &mut String,
    target_names: &[String],
    target_snapshots: &HashMap<String, ApiTargetSnapshot>,
) {
    target.push('[');
    for (index, name) in target_names.iter().enumerate() {
        if index > 0 {
            target.push(',');
        }
        let path_segment = api_target_path_segment(name);
        let (
            has_sidecar_context,
            has_enrichment,
            has_opinion,
            protocol,
            entry,
            default_entry,
            selected_is_default,
            shelf_key,
            shelf_label,
        ) = target_snapshots
            .get(name)
            .map(|target| {
                (
                    target.has_external_sidecar_context,
                    target.has_external_evidence_chain_enrichment,
                    target.has_external_diagnostic_opinion,
                    target
                        .protocol_surface
                        .as_ref()
                        .map(|surface| surface.protocol.as_str()),
                    target
                        .protocol_surface
                        .as_ref()
                        .map(|surface| surface.entry.as_str()),
                    target
                        .protocol_surface
                        .as_ref()
                        .map(|surface| surface.default_entry.as_str()),
                    target
                        .protocol_surface
                        .as_ref()
                        .is_some_and(|surface| surface.selected_is_default),
                    target
                        .protocol_surface
                        .as_ref()
                        .and_then(|surface| surface.shelf.as_ref().map(|shelf| shelf.key.as_str())),
                    target.protocol_surface.as_ref().and_then(|surface| {
                        surface.shelf.as_ref().map(|shelf| shelf.label.as_str())
                    }),
                )
            })
            .unwrap_or((false, false, false, None, None, None, false, None, None));
        target.push_str("{\"name\":");
        append_json_string(target, name);
        target.push_str(",\"path_segment\":");
        append_json_string(target, &path_segment);
        target.push_str(",\"url_path\":\"/v1/latest/targets/");
        target.push_str(&path_segment);
        target.push_str("\",\"has_external_sidecar_context\":");
        target.push_str(if has_sidecar_context { "true" } else { "false" });
        target.push_str(",\"has_external_evidence_chain_enrichment\":");
        target.push_str(if has_enrichment { "true" } else { "false" });
        target.push_str(",\"has_external_diagnostic_opinion\":");
        target.push_str(if has_opinion { "true" } else { "false" });
        target.push_str(",\"has_protocol_surface\":");
        target.push_str(if protocol.is_some() { "true" } else { "false" });
        target.push_str(",\"protocol\":");
        match protocol {
            Some(value) => append_json_string(target, value),
            None => target.push_str("null"),
        }
        target.push_str(",\"entry\":");
        match entry {
            Some(value) => append_json_string(target, value),
            None => target.push_str("null"),
        }
        target.push_str(",\"default_entry\":");
        match default_entry {
            Some(value) => append_json_string(target, value),
            None => target.push_str("null"),
        }
        target.push_str(",\"selected_is_default\":");
        target.push_str(if selected_is_default { "true" } else { "false" });
        target.push_str(",\"shelf_key\":");
        match shelf_key {
            Some(value) => append_json_string(target, value),
            None => target.push_str("null"),
        }
        target.push_str(",\"shelf_label\":");
        match shelf_label {
            Some(value) => append_json_string(target, value),
            None => target.push_str("null"),
        }
        target.push('}');
    }
    target.push(']');
}

fn append_api_snapshot_index_fields_json(target: &mut String, snapshot: &ApiSnapshot) {
    target.push_str("\"kind\":");
    append_json_string(target, &snapshot.kind);
    target.push_str(",\"name\":");
    if let Some(name) = snapshot.name.as_deref() {
        append_json_string(target, name);
    } else {
        target.push_str("null");
    }
    target.push_str(",\"target_count\":");
    if let Some(count) = snapshot.target_count {
        target.push_str(&count.to_string());
    } else {
        target.push_str("null");
    }
    target.push_str(",\"target_names\":");
    append_string_list_json(target, &snapshot.target_names);
    target.push_str(",\"target_refs\":");
    append_api_target_refs_json(target, &snapshot.target_names, &snapshot.target_snapshots);
}

fn append_api_snapshot_presence_fields_json(target: &mut String, snapshot: &ApiSnapshot) {
    target.push_str("\"has_summary_text\":");
    target.push_str(if snapshot.summary_text.is_some() {
        "true"
    } else {
        "false"
    });
    target.push_str(",\"has_summary_json\":");
    target.push_str(if snapshot.summary_json.is_some() {
        "true"
    } else {
        "false"
    });
    target.push_str(",\"has_findings_json\":");
    target.push_str(if snapshot.findings_json.is_some() {
        "true"
    } else {
        "false"
    });
    target.push_str(",\"has_analysis_json\":");
    target.push_str(if snapshot.analysis_json.is_some() {
        "true"
    } else {
        "false"
    });
    target.push_str(",\"has_export_json\":");
    target.push_str(if snapshot.export_json.is_some() {
        "true"
    } else {
        "false"
    });
    target.push_str(",\"has_report_json\":");
    target.push_str(if snapshot.report_json.is_some() {
        "true"
    } else {
        "false"
    });
    target.push_str(",\"has_report_html\":");
    target.push_str(if snapshot.report_html.is_some() {
        "true"
    } else {
        "false"
    });
    target.push_str(",\"has_external_sidecar_context\":");
    target.push_str(if snapshot.has_external_sidecar_context {
        "true"
    } else {
        "false"
    });
    target.push_str(",\"has_external_evidence_chain_enrichment\":");
    target.push_str(if snapshot.has_external_evidence_chain_enrichment {
        "true"
    } else {
        "false"
    });
    target.push_str(",\"has_external_diagnostic_opinion\":");
    target.push_str(if snapshot.has_external_diagnostic_opinion {
        "true"
    } else {
        "false"
    });
}

fn estimate_api_snapshot_meta_capacity(snapshot: &ApiSnapshot) -> usize {
    320 + snapshot.kind.len()
        + snapshot.name.as_ref().map_or(4, String::len)
        + snapshot.target_names.iter().map(String::len).sum::<usize>() * 3
}

fn estimate_api_target_list_capacity(snapshot: &ApiSnapshot) -> usize {
    128 + snapshot.kind.len() + snapshot.target_names.iter().map(String::len).sum::<usize>() * 4
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_api::{ApiSnapshot, ApiTargetSnapshot};
    use gewyvern::protocol_profiles::{ProtocolShelfSummary, ProtocolSurfaceSummary};

    #[test]
    fn api_target_path_segment_percent_encodes_reserved_bytes() {
        assert_eq!(
            api_target_path_segment("scan/http request"),
            "scan%2Fhttp%20request"
        );
    }

    #[test]
    fn decode_api_target_path_segment_rejects_invalid_escape() {
        let err = decode_api_target_path_segment("%ZZ").expect_err("should reject invalid escape");
        assert!(err.contains("invalid percent-encoding"));
    }

    #[test]
    fn api_target_refs_include_protocol_shelf_summary_when_available() {
        let mut snapshot = ApiSnapshot {
            kind: "scan".into(),
            target_count: Some(1),
            target_names: vec!["scan:redis:zadd".into()],
            ..ApiSnapshot::default()
        };
        snapshot.target_snapshots.insert(
            "scan:redis:zadd".into(),
            ApiTargetSnapshot {
                summary_text: String::new(),
                summary_json: String::new(),
                findings_json: String::new(),
                analysis_json: String::new(),
                protocol_surface_json: None,
                protocol_surface: Some(ProtocolSurfaceSummary {
                    protocol: "redis".into(),
                    entry: "zadd".into(),
                    default_entry: "session".into(),
                    selected_is_default: false,
                    protocol_aliases: vec!["redis-session".into()],
                    entry_aliases: vec!["sorted-write".into()],
                    sibling_entries: vec!["zadd".into(), "zrange".into()],
                    shelf: Some(ProtocolShelfSummary {
                        key: "sorted-set".into(),
                        label: "Sorted Set".into(),
                        page: "docs/book/reference-redis-sorted-set-surface.md".into(),
                        entries: vec!["zadd".into(), "zrange".into()],
                    }),
                }),
                has_external_sidecar_context: false,
                has_external_evidence_chain_enrichment: false,
                has_external_diagnostic_opinion: false,
                export_json: String::new(),
                report_json: String::new(),
                report_html: String::new(),
            },
        );
        let body = api_target_list_json(&snapshot);
        assert!(body.contains("\"shelf_key\":\"sorted-set\""));
        assert!(body.contains("\"shelf_label\":\"Sorted Set\""));
    }
}
