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
        let (has_sidecar_context, has_enrichment, has_opinion) = target_snapshots
            .get(name)
            .map(|target| {
                (
                    target.has_external_sidecar_context,
                    target.has_external_evidence_chain_enrichment,
                    target.has_external_diagnostic_opinion,
                )
            })
            .unwrap_or((false, false, false));
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
}
