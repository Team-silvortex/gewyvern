use gewyvern::certificate_policy::{CertificatePolicyView, runtime_certificate_policy};

use crate::render_utils::append_json_string;

pub(super) fn api_runtime_certificate_policy_json() -> String {
    api_runtime_certificate_policy_json_from_view(&runtime_certificate_policy())
}

pub(super) fn api_runtime_certificate_policy_json_from_view(
    view: &CertificatePolicyView,
) -> String {
    let mut json = String::with_capacity(2048);
    json.push_str("{\"surface\":\"runtime_certificate_policy\",\"status\":");
    append_json_string(&mut json, view.status);
    json.push_str(",\"severity\":");
    append_json_string(&mut json, view.severity);
    json.push_str(",\"summary\":");
    append_json_string(&mut json, &view.summary);
    json.push_str(",\"recommended_actions\":");
    append_string_list_json(&mut json, &view.recommended_actions);
    json.push_str(",\"reason_count\":");
    json.push_str(&view.reasons.len().to_string());
    json.push_str(",\"reasons\":[");
    for (index, reason) in view.reasons.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        json.push_str("{\"code\":");
        append_json_string(&mut json, reason.code);
        json.push_str(",\"severity\":");
        append_json_string(&mut json, reason.severity);
        json.push_str(",\"summary\":");
        append_json_string(&mut json, reason.summary);
        json.push('}');
    }
    json.push_str("]}");
    json
}

fn append_string_list_json(target: &mut String, items: &[&str]) {
    target.push('[');
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            target.push(',');
        }
        append_json_string(target, item);
    }
    target.push(']');
}

#[cfg(test)]
mod tests {
    use super::*;
    use gewyvern::certificate_policy::{CertificatePolicyReason, CertificatePolicyView};

    #[test]
    fn certificate_policy_json_lists_reasons_and_actions() {
        let view = CertificatePolicyView {
            status: "attention",
            severity: "warning",
            summary: "certificate shelf needs operator attention".into(),
            recommended_actions: vec!["add a trust anchor"],
            reasons: vec![CertificatePolicyReason {
                code: "explicit_remote_trust_without_anchors",
                severity: "warning",
                summary: "remote trust is locked down but no trust anchors are present yet",
            }],
        };
        let body = api_runtime_certificate_policy_json_from_view(&view);
        assert!(body.contains("\"surface\":\"runtime_certificate_policy\""));
        assert!(body.contains("\"status\":\"attention\""));
        assert!(body.contains("\"severity\":\"warning\""));
        assert!(body.contains("\"reason_count\":1"));
        assert!(body.contains("\"code\":\"explicit_remote_trust_without_anchors\""));
        assert!(body.contains("\"severity\":\"warning\",\"summary\":\"remote trust is locked down but no trust anchors are present yet\""));
        assert!(body.contains("\"recommended_actions\":[\"add a trust anchor\"]"));
    }
}
