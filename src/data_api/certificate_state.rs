use gewyvern::certificate_state::{
    CertificateMaterialScope, CertificateRevocationStatus, CertificateRotationStatus,
    CertificateRuntimeState, runtime_certificate_state,
};

use crate::render_utils::append_json_string;

pub(super) fn api_runtime_certificate_state_json() -> String {
    api_runtime_certificate_state_json_from_state(&runtime_certificate_state())
}

pub(super) fn api_runtime_certificate_state_json_from_state(
    state: &CertificateRuntimeState,
) -> String {
    let mut json = String::with_capacity(4096);
    json.push_str("{\"surface\":\"runtime_certificate_state\",\"root\":");
    append_json_string(&mut json, &state.root.to_string_lossy());
    json.push_str(",\"rotation_records_path\":");
    append_json_string(&mut json, &state.rotation_records_path.to_string_lossy());
    json.push_str(",\"revocation_records_path\":");
    append_json_string(&mut json, &state.revocation_records_path.to_string_lossy());
    json.push_str(",\"rotation_records_exist\":");
    json.push_str(if state.rotation_records_exist {
        "true"
    } else {
        "false"
    });
    json.push_str(",\"revocation_records_exist\":");
    json.push_str(if state.revocation_records_exist {
        "true"
    } else {
        "false"
    });
    json.push_str(",\"rotation_records_valid\":");
    json.push_str(if state.rotation_records_valid {
        "true"
    } else {
        "false"
    });
    json.push_str(",\"revocation_records_valid\":");
    json.push_str(if state.revocation_records_valid {
        "true"
    } else {
        "false"
    });
    json.push_str(",\"summary\":{");
    json.push_str("\"state_valid\":");
    json.push_str(
        if state.rotation_records_valid && state.revocation_records_valid {
            "true"
        } else {
            "false"
        },
    );
    json.push(',');
    append_count_field(&mut json, "rotation_records", state.rotation_records.len());
    json.push(',');
    append_count_field(
        &mut json,
        "overdue_rotations",
        state
            .rotation_records
            .iter()
            .filter(|record| {
                matches!(
                    record.status,
                    CertificateRotationStatus::Overdue | CertificateRotationStatus::Error
                )
            })
            .count(),
    );
    json.push(',');
    append_count_field(
        &mut json,
        "revocation_records",
        state.revocation_records.len(),
    );
    json.push(',');
    append_count_field(
        &mut json,
        "active_revocations",
        state
            .revocation_records
            .iter()
            .filter(|record| {
                matches!(
                    record.status,
                    CertificateRevocationStatus::Revoked | CertificateRevocationStatus::Distrusted
                )
            })
            .count(),
    );
    json.push_str("},\"rotation_records\":");
    append_rotation_records_json(&mut json, state);
    json.push_str(",\"revocation_records\":");
    append_revocation_records_json(&mut json, state);
    json.push('}');
    json
}

pub(super) fn append_state_summary_json(target: &mut String, state: &CertificateRuntimeState) {
    target.push('{');
    target.push_str("\"state_valid\":");
    target.push_str(
        if state.rotation_records_valid && state.revocation_records_valid {
            "true"
        } else {
            "false"
        },
    );
    target.push(',');
    append_count_field(target, "rotation_records", state.rotation_records.len());
    target.push(',');
    append_count_field(
        target,
        "overdue_rotations",
        state
            .rotation_records
            .iter()
            .filter(|record| {
                matches!(
                    record.status,
                    CertificateRotationStatus::Overdue | CertificateRotationStatus::Error
                )
            })
            .count(),
    );
    target.push(',');
    append_count_field(target, "revocation_records", state.revocation_records.len());
    target.push(',');
    append_count_field(
        target,
        "active_revocations",
        state
            .revocation_records
            .iter()
            .filter(|record| {
                matches!(
                    record.status,
                    CertificateRevocationStatus::Revoked | CertificateRevocationStatus::Distrusted
                )
            })
            .count(),
    );
    target.push('}');
}

fn append_rotation_records_json(target: &mut String, state: &CertificateRuntimeState) {
    target.push('[');
    for (index, record) in state.rotation_records.iter().enumerate() {
        if index > 0 {
            target.push(',');
        }
        target.push('{');
        target.push_str("\"relative_path\":");
        append_json_string(target, &record.relative_path);
        target.push_str(",\"status\":");
        append_json_string(target, rotation_status_label(record.status));
        target.push_str(",\"due_unix_ms\":");
        append_optional_i128_json(target, record.due_unix_ms);
        target.push_str(",\"last_rotated_unix_ms\":");
        append_optional_i128_json(target, record.last_rotated_unix_ms);
        target.push_str(",\"updated_unix_ms\":");
        append_optional_i128_json(target, record.updated_unix_ms);
        target.push_str(",\"note\":");
        append_optional_string_json(target, record.note.as_deref());
        target.push('}');
    }
    target.push(']');
}

fn append_revocation_records_json(target: &mut String, state: &CertificateRuntimeState) {
    target.push('[');
    for (index, record) in state.revocation_records.iter().enumerate() {
        if index > 0 {
            target.push(',');
        }
        target.push('{');
        target.push_str("\"relative_path\":");
        append_json_string(target, &record.relative_path);
        target.push_str(",\"scope\":");
        append_json_string(target, material_scope_label(record.scope));
        target.push_str(",\"status\":");
        append_json_string(target, revocation_status_label(record.status));
        target.push_str(",\"effective_unix_ms\":");
        append_optional_i128_json(target, record.effective_unix_ms);
        target.push_str(",\"updated_unix_ms\":");
        append_optional_i128_json(target, record.updated_unix_ms);
        target.push_str(",\"note\":");
        append_optional_string_json(target, record.note.as_deref());
        target.push('}');
    }
    target.push(']');
}

fn append_count_field(target: &mut String, key: &str, value: usize) {
    append_json_string(target, key);
    target.push(':');
    target.push_str(&value.to_string());
}

fn append_optional_i128_json(target: &mut String, value: Option<i128>) {
    match value {
        Some(value) => target.push_str(&value.to_string()),
        None => target.push_str("null"),
    }
}

fn append_optional_string_json(target: &mut String, value: Option<&str>) {
    match value {
        Some(value) => append_json_string(target, value),
        None => target.push_str("null"),
    }
}

fn rotation_status_label(status: CertificateRotationStatus) -> &'static str {
    match status {
        CertificateRotationStatus::Active => "active",
        CertificateRotationStatus::Due => "due",
        CertificateRotationStatus::Overdue => "overdue",
        CertificateRotationStatus::Error => "error",
    }
}

fn revocation_status_label(status: CertificateRevocationStatus) -> &'static str {
    match status {
        CertificateRevocationStatus::Revoked => "revoked",
        CertificateRevocationStatus::Distrusted => "distrusted",
        CertificateRevocationStatus::Cleared => "cleared",
    }
}

fn material_scope_label(scope: CertificateMaterialScope) -> &'static str {
    match scope {
        CertificateMaterialScope::Trust => "trust",
        CertificateMaterialScope::Authority => "authority",
        CertificateMaterialScope::Identity => "identity",
        CertificateMaterialScope::Other => "other",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gewyvern::certificate_state::{CertificateRevocationRecord, CertificateRotationRecord};
    use std::path::PathBuf;

    #[test]
    fn certificate_state_json_lists_rotation_and_revocation_records() {
        let state = CertificateRuntimeState {
            root: PathBuf::from("/srv/gewyvern/state/certificates"),
            rotation_records_path: PathBuf::from(
                "/srv/gewyvern/state/certificates/rotation-records.tsv",
            ),
            revocation_records_path: PathBuf::from(
                "/srv/gewyvern/state/certificates/revocation-records.tsv",
            ),
            rotation_records_exist: true,
            revocation_records_exist: true,
            rotation_records_valid: true,
            revocation_records_valid: true,
            rotation_records: vec![CertificateRotationRecord {
                relative_path: "identities/prod/runtime.pem".into(),
                status: CertificateRotationStatus::Overdue,
                due_unix_ms: Some(10),
                last_rotated_unix_ms: Some(5),
                updated_unix_ms: Some(12),
                note: Some("rotate now".into()),
            }],
            revocation_records: vec![CertificateRevocationRecord {
                relative_path: "trust/anchors/root-ca.pem".into(),
                scope: CertificateMaterialScope::Trust,
                status: CertificateRevocationStatus::Distrusted,
                effective_unix_ms: Some(20),
                updated_unix_ms: Some(21),
                note: Some("legacy".into()),
            }],
        };
        let body = api_runtime_certificate_state_json_from_state(&state);
        assert!(body.contains("\"surface\":\"runtime_certificate_state\""));
        assert!(body.contains("\"rotation_records_valid\":true"));
        assert!(body.contains("\"revocation_records_valid\":true"));
        assert!(body.contains("\"state_valid\":true"));
        assert!(body.contains("\"overdue_rotations\":1"));
        assert!(body.contains("\"active_revocations\":1"));
        assert!(body.contains("\"status\":\"overdue\""));
        assert!(body.contains("\"status\":\"distrusted\""));
    }
}
