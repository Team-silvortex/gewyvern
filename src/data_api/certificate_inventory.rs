use gewyvern::certificate_inventory::{
    CertificateAssetKind, CertificateInventory, runtime_certificate_inventory_scan,
};
use gewyvern::certificate_policy::certificate_policy_for_inventory_with_scan_status;
use gewyvern::certificate_state::runtime_certificate_state;

use super::certificate_state::append_state_summary_json;
use crate::render_utils::append_json_string;

pub(super) fn api_runtime_certificates_json() -> String {
    let scan = runtime_certificate_inventory_scan();
    api_runtime_certificates_json_with_scan_status(&scan.inventory, scan.truncated)
}

#[cfg(test)]
pub(super) fn api_runtime_certificates_json_from_inventory(
    inventory: &CertificateInventory,
) -> String {
    api_runtime_certificates_json_with_scan_status(inventory, false)
}

fn api_runtime_certificates_json_with_scan_status(
    inventory: &CertificateInventory,
    scan_truncated: bool,
) -> String {
    let mut json = String::with_capacity(4096);
    json.push_str("{\"surface\":\"runtime_certificates\",\"root\":");
    append_json_string(&mut json, &inventory.root.to_string_lossy());
    json.push_str(",\"trust_root\":");
    append_json_string(&mut json, &inventory.trust_root.to_string_lossy());
    json.push_str(",\"authority_root\":");
    append_json_string(&mut json, &inventory.authority_root.to_string_lossy());
    json.push_str(",\"identity_root\":");
    append_json_string(&mut json, &inventory.identity_root.to_string_lossy());
    json.push_str(",\"state_root\":");
    append_json_string(&mut json, &inventory.state_root.to_string_lossy());
    json.push_str(",\"roots\":{");
    append_bool_field(&mut json, "root_exists", inventory.root_exists);
    json.push(',');
    append_bool_field(&mut json, "trust_root_exists", inventory.trust_root_exists);
    json.push(',');
    append_bool_field(
        &mut json,
        "authority_root_exists",
        inventory.authority_root_exists,
    );
    json.push(',');
    append_bool_field(
        &mut json,
        "identity_root_exists",
        inventory.identity_root_exists,
    );
    json.push(',');
    append_bool_field(&mut json, "state_root_exists", inventory.state_root_exists);
    json.push('}');
    json.push_str(",\"require_explicit_remote_trust\":");
    json.push_str(if inventory.require_explicit_remote_trust {
        "true"
    } else {
        "false"
    });
    json.push_str(",\"scan_truncated\":");
    json.push_str(if scan_truncated { "true" } else { "false" });
    json.push_str(",\"summary\":{");
    append_count_field(&mut json, "trust_items", inventory.trust_items.len());
    json.push(',');
    append_count_field(
        &mut json,
        "authority_items",
        inventory.authority_items.len(),
    );
    json.push(',');
    append_count_field(&mut json, "identity_items", inventory.identity_items.len());
    json.push_str("},\"policy\":");
    append_policy_summary_json(&mut json, inventory, scan_truncated);
    json.push_str(",\"state\":");
    append_state_summary_json(&mut json, &runtime_certificate_state());
    json.push_str(",\"trust_items\":");
    append_items_json(&mut json, &inventory.trust_items);
    json.push_str(",\"authority_items\":");
    append_items_json(&mut json, &inventory.authority_items);
    json.push_str(",\"identity_items\":");
    append_items_json(&mut json, &inventory.identity_items);
    json.push('}');
    json
}

fn append_policy_summary_json(
    target: &mut String,
    inventory: &CertificateInventory,
    scan_truncated: bool,
) {
    let policy = certificate_policy_for_inventory_with_scan_status(inventory, scan_truncated);
    target.push('{');
    target.push_str("\"status\":");
    append_json_string(target, policy.status);
    target.push_str(",\"severity\":");
    append_json_string(target, policy.severity);
    target.push_str(",\"summary\":");
    append_json_string(target, &policy.summary);
    target.push_str(",\"reason_count\":");
    target.push_str(&policy.reasons.len().to_string());
    target.push_str(",\"reason_codes\":[");
    for (index, reason) in policy.reasons.iter().enumerate() {
        if index > 0 {
            target.push(',');
        }
        append_json_string(target, reason.code);
    }
    target.push_str("]}");
}

fn append_count_field(target: &mut String, key: &str, value: usize) {
    append_json_string(target, key);
    target.push(':');
    target.push_str(&value.to_string());
}

fn append_bool_field(target: &mut String, key: &str, value: bool) {
    append_json_string(target, key);
    target.push(':');
    target.push_str(if value { "true" } else { "false" });
}

fn append_items_json(
    target: &mut String,
    items: &[gewyvern::certificate_inventory::CertificateItem],
) {
    target.push('[');
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            target.push(',');
        }
        target.push('{');
        target.push_str("\"relative_path\":");
        append_json_string(target, &item.relative_path);
        target.push_str(",\"asset_kind\":");
        append_json_string(target, certificate_asset_kind_label(item.asset_kind));
        target.push_str(",\"bytes\":");
        target.push_str(&item.bytes.to_string());
        target.push_str(",\"modified_unix_ms\":");
        match item.modified_unix_ms {
            Some(value) => {
                let rendered = value.to_string();
                target.push_str(rendered.as_str());
            }
            None => target.push_str("null"),
        }
        target.push_str(",\"validity\":");
        append_validity_json(target, item.validity.as_ref());
        target.push('}');
    }
    target.push(']');
}

fn append_validity_json(
    target: &mut String,
    validity: Option<&gewyvern::certificate_validity::CertificateValidityWindow>,
) {
    let Some(validity) = validity else {
        target.push_str("null");
        return;
    };
    target.push('{');
    target.push_str("\"certificate_count\":");
    target.push_str(&validity.certificate_count.to_string());
    target.push_str(",\"earliest_not_before_unix_ms\":");
    append_optional_i128_json(target, validity.earliest_not_before_unix_ms);
    target.push_str(",\"earliest_not_after_unix_ms\":");
    append_optional_i128_json(target, validity.earliest_not_after_unix_ms);
    target.push_str(",\"latest_not_after_unix_ms\":");
    append_optional_i128_json(target, validity.latest_not_after_unix_ms);
    target.push('}');
}

fn append_optional_i128_json(target: &mut String, value: Option<i128>) {
    match value {
        Some(value) => target.push_str(&value.to_string()),
        None => target.push_str("null"),
    }
}

fn certificate_asset_kind_label(kind: CertificateAssetKind) -> &'static str {
    match kind {
        CertificateAssetKind::CertificatePem => "certificate_pem",
        CertificateAssetKind::PrivateKeyPem => "private_key_pem",
        CertificateAssetKind::ChainPem => "chain_pem",
        CertificateAssetKind::BundlePem => "bundle_pem",
        CertificateAssetKind::UnknownPem => "unknown_pem",
        CertificateAssetKind::Other => "other",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gewyvern::certificate_inventory::{
        CertificateItem, runtime_certificate_inventory_from_layout,
    };
    use gewyvern::runtime_layout::RuntimeLayout;
    use std::path::PathBuf;

    #[test]
    fn certificates_json_lists_operator_roots_and_items() {
        let inventory = CertificateInventory {
            root: PathBuf::from("/srv/gewyvern/certificates"),
            trust_root: PathBuf::from("/srv/gewyvern/certificates/trust"),
            authority_root: PathBuf::from("/srv/gewyvern/certificates/authorities"),
            identity_root: PathBuf::from("/srv/gewyvern/certificates/identities"),
            state_root: PathBuf::from("/srv/gewyvern/state/certificates"),
            root_exists: true,
            trust_root_exists: true,
            authority_root_exists: true,
            identity_root_exists: true,
            state_root_exists: true,
            require_explicit_remote_trust: true,
            trust_items: vec![CertificateItem {
                relative_path: "anchors/root-ca.pem".into(),
                asset_kind: CertificateAssetKind::BundlePem,
                bytes: 2048,
                modified_unix_ms: Some(1000),
                validity: Some(gewyvern::certificate_validity::CertificateValidityWindow {
                    certificate_count: 1,
                    earliest_not_before_unix_ms: Some(1_750_000_000_000),
                    earliest_not_after_unix_ms: Some(2_060_000_000_000),
                    latest_not_after_unix_ms: Some(2_060_000_000_000),
                }),
            }],
            authority_items: vec![],
            identity_items: vec![CertificateItem {
                relative_path: "prod/runtime.key".into(),
                asset_kind: CertificateAssetKind::PrivateKeyPem,
                bytes: 1024,
                modified_unix_ms: None,
                validity: None,
            }],
        };

        let body = api_runtime_certificates_json_from_inventory(&inventory);
        assert!(body.contains("\"surface\":\"runtime_certificates\""));
        assert!(body.contains("\"trust_items\":1"));
        assert!(body.contains("\"identity_items\":1"));
        assert!(body.contains("\"asset_kind\":\"bundle_pem\""));
        assert!(body.contains("\"asset_kind\":\"private_key_pem\""));
        assert!(body.contains("\"roots\":{\"root_exists\":true"));
        assert!(body.contains("\"policy\":{"));
        assert!(body.contains("\"reason_codes\":["));
        assert!(body.contains("\"require_explicit_remote_trust\":true"));
        assert!(body.contains("\"scan_truncated\":false"));
        assert!(body.contains("\"validity\":{\"certificate_count\":1"));
    }

    #[test]
    fn runtime_inventory_from_layout_can_render_empty_roots() {
        let body = api_runtime_certificates_json_from_inventory(
            &runtime_certificate_inventory_from_layout(
                RuntimeLayout {
                    config_root: PathBuf::from("/tmp/config"),
                    data_root: PathBuf::from("/tmp/data"),
                    state_root: PathBuf::from("/tmp/state"),
                    cache_root: PathBuf::from("/tmp/cache"),
                    certificate_root: PathBuf::from("/tmp/config/certificates"),
                    trust_root: PathBuf::from("/tmp/config/certificates/trust"),
                    authority_root: PathBuf::from("/tmp/config/certificates/authorities"),
                    identity_root: PathBuf::from("/tmp/config/certificates/identities"),
                    certificate_state_root: PathBuf::from("/tmp/state/certificates"),
                    legacy_root: None,
                },
                false,
            ),
        );
        assert!(body.contains(
            "\"summary\":{\"trust_items\":0,\"authority_items\":0,\"identity_items\":0}"
        ));
    }

    #[test]
    fn certificates_json_keeps_policy_and_state_machine_summary_fields() {
        let inventory = CertificateInventory {
            root: PathBuf::from("/srv/gewyvern/certificates"),
            trust_root: PathBuf::from("/srv/gewyvern/certificates/trust"),
            authority_root: PathBuf::from("/srv/gewyvern/certificates/authorities"),
            identity_root: PathBuf::from("/srv/gewyvern/certificates/identities"),
            state_root: PathBuf::from("/srv/gewyvern/state/certificates"),
            root_exists: false,
            trust_root_exists: false,
            authority_root_exists: true,
            identity_root_exists: true,
            state_root_exists: false,
            require_explicit_remote_trust: true,
            trust_items: vec![],
            authority_items: vec![],
            identity_items: vec![],
        };

        let body = api_runtime_certificates_json_with_scan_status(&inventory, true);
        assert!(body.contains("\"root\":\"/srv/gewyvern/certificates\""));
        assert!(body.contains("\"trust_root\":\"/srv/gewyvern/certificates/trust\""));
        assert!(body.contains("\"roots\":{\"root_exists\":false"));
        assert!(body.contains("\"trust_root_exists\":false"));
        assert!(body.contains("\"authority_root_exists\":true"));
        assert!(body.contains("\"identity_root_exists\":true"));
        assert!(body.contains("\"state_root_exists\":false"));
        assert!(body.contains("\"require_explicit_remote_trust\":true"));
        assert!(body.contains("\"scan_truncated\":true"));
        assert!(body.contains("\"policy\":{\"status\":"));
        assert!(body.contains("\"severity\":"));
        assert!(body.contains("\"summary\":"));
        assert!(body.contains("\"reason_count\":"));
        assert!(body.contains("\"reason_codes\":["));
        let document: serde_json::Value =
            serde_json::from_str(&body).expect("certificate inventory must be valid JSON");
        let state = document
            .get("state")
            .and_then(serde_json::Value::as_object)
            .expect("certificate inventory must carry a state summary");
        assert!(
            state
                .get("state_valid")
                .and_then(serde_json::Value::as_bool)
                .is_some()
        );
        for field in [
            "rotation_records",
            "overdue_rotations",
            "revocation_records",
            "active_revocations",
        ] {
            assert!(
                state
                    .get(field)
                    .and_then(serde_json::Value::as_u64)
                    .is_some(),
                "certificate state summary field {field} must be an unsigned count"
            );
        }
    }
}
