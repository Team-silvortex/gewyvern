use crate::certificate_inventory::{
    CertificateAssetKind, CertificateInventory, CertificateInventoryScan, CertificateItem,
    runtime_certificate_inventory_scan,
};
use crate::certificate_state::{
    CertificateMaterialScope, CertificateRevocationStatus, CertificateRotationStatus,
    runtime_certificate_state,
};
use std::time::{SystemTime, UNIX_EPOCH};

pub const REASON_EXPLICIT_REMOTE_TRUST_WITHOUT_ANCHORS: &str =
    "explicit_remote_trust_without_anchors";
pub const REASON_PRIVATE_KEYS_PRESENT_IN_TRUST_ROOT: &str = "private_keys_present_in_trust_root";
pub const REASON_IDENTITY_KEYS_WITHOUT_CERTIFICATES: &str = "identity_keys_without_certificates";
pub const REASON_IDENTITY_CERTIFICATES_WITHOUT_KEYS: &str = "identity_certificates_without_keys";
pub const REASON_EMPTY_AUTHORITY_ROOT: &str = "empty_authority_root";
pub const REASON_CERTIFICATE_STATE_ROOT_MISSING: &str = "certificate_state_root_missing";
pub const REASON_CERTIFICATE_SHELF_BOOTSTRAP_EMPTY: &str = "certificate_shelf_bootstrap_empty";
pub const REASON_EXPIRED_CERTIFICATE_MATERIAL: &str = "expired_certificate_material";
pub const REASON_EXPIRING_CERTIFICATE_MATERIAL: &str = "expiring_certificate_material";
pub const REASON_OVERDUE_CERTIFICATE_ROTATION: &str = "overdue_certificate_rotation";
pub const REASON_REVOKED_CERTIFICATE_MATERIAL: &str = "revoked_certificate_material";
pub const REASON_DISTRUSTED_TRUST_ANCHOR_MATERIAL: &str = "distrusted_trust_anchor_material";
pub const REASON_CERTIFICATE_INVENTORY_TRUNCATED: &str = "certificate_inventory_truncated";

const EXPIRING_SOON_WINDOW_MS: i128 = 30_i128 * 24 * 60 * 60 * 1000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CertificatePolicyView {
    pub status: &'static str,
    pub severity: &'static str,
    pub summary: String,
    pub recommended_actions: Vec<&'static str>,
    pub reasons: Vec<CertificatePolicyReason>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CertificatePolicyReason {
    pub code: &'static str,
    pub severity: &'static str,
    pub summary: &'static str,
}

pub fn runtime_certificate_policy() -> CertificatePolicyView {
    let scan = runtime_certificate_inventory_scan();
    certificate_policy_for_inventory_and_state_with_truncation(
        &scan.inventory,
        &runtime_certificate_state(),
        scan.truncated,
    )
}

pub fn certificate_policy_for_inventory(inventory: &CertificateInventory) -> CertificatePolicyView {
    certificate_policy_for_inventory_and_state(inventory, &empty_certificate_state(inventory))
}

pub fn certificate_policy_for_inventory_scan(
    scan: &CertificateInventoryScan,
) -> CertificatePolicyView {
    certificate_policy_for_inventory_with_scan_status(&scan.inventory, scan.truncated)
}

pub fn certificate_policy_for_inventory_with_scan_status(
    inventory: &CertificateInventory,
    scan_truncated: bool,
) -> CertificatePolicyView {
    certificate_policy_for_inventory_and_state_with_truncation(
        inventory,
        &empty_certificate_state(inventory),
        scan_truncated,
    )
}

pub fn certificate_policy_for_inventory_and_state(
    inventory: &CertificateInventory,
    state: &crate::certificate_state::CertificateRuntimeState,
) -> CertificatePolicyView {
    certificate_policy_for_inventory_and_state_with_truncation(inventory, state, false)
}

fn certificate_policy_for_inventory_and_state_with_truncation(
    inventory: &CertificateInventory,
    state: &crate::certificate_state::CertificateRuntimeState,
    scan_truncated: bool,
) -> CertificatePolicyView {
    let mut reasons = Vec::new();
    let trust_private_keys =
        count_kind(&inventory.trust_items, CertificateAssetKind::PrivateKeyPem);
    let trust_certs = count_cert_like(&inventory.trust_items);
    let identity_private_keys = count_kind(
        &inventory.identity_items,
        CertificateAssetKind::PrivateKeyPem,
    );
    let identity_certs = count_kind(
        &inventory.identity_items,
        CertificateAssetKind::CertificatePem,
    ) + count_kind(&inventory.identity_items, CertificateAssetKind::ChainPem);
    let authority_material = count_cert_like(&inventory.authority_items)
        + count_kind(
            &inventory.authority_items,
            CertificateAssetKind::PrivateKeyPem,
        );
    let now_unix_ms = current_unix_ms();
    let expired_certificate_count = count_expired_certificate_items(inventory, now_unix_ms);
    let expiring_certificate_count =
        count_expiring_certificate_items(inventory, now_unix_ms, EXPIRING_SOON_WINDOW_MS);
    let overdue_rotation_count = state
        .rotation_records
        .iter()
        .filter(|record| {
            matches!(
                record.status,
                CertificateRotationStatus::Overdue | CertificateRotationStatus::Error
            )
        })
        .count();
    let revoked_material_count = state
        .revocation_records
        .iter()
        .filter(|record| record.status == CertificateRevocationStatus::Revoked)
        .count();
    let distrusted_trust_anchor_count = state
        .revocation_records
        .iter()
        .filter(|record| {
            record.status == CertificateRevocationStatus::Distrusted
                && record.scope == CertificateMaterialScope::Trust
        })
        .count();

    if scan_truncated {
        reasons.push(reason(
            REASON_CERTIFICATE_INVENTORY_TRUNCATED,
            "warning",
            "certificate inventory exceeded a safe scan boundary and is incomplete",
        ));
    }
    if inventory.require_explicit_remote_trust && trust_certs == 0 {
        reasons.push(reason(
            REASON_EXPLICIT_REMOTE_TRUST_WITHOUT_ANCHORS,
            "warning",
            "remote trust is locked down but no trust anchors are present yet",
        ));
    }
    if trust_private_keys > 0 {
        reasons.push(reason(
            REASON_PRIVATE_KEYS_PRESENT_IN_TRUST_ROOT,
            "warning",
            "private keys were found in the trust root, which should normally hold anchors only",
        ));
    }
    if identity_private_keys > 0 && identity_certs == 0 {
        reasons.push(reason(
            REASON_IDENTITY_KEYS_WITHOUT_CERTIFICATES,
            "warning",
            "identity private keys exist without matching certificate material",
        ));
    }
    if identity_certs > 0 && identity_private_keys == 0 {
        reasons.push(reason(
            REASON_IDENTITY_CERTIFICATES_WITHOUT_KEYS,
            "observe",
            "identity certificates are present without private keys in the identity shelf",
        ));
    }
    if inventory.authority_root_exists && authority_material == 0 {
        reasons.push(reason(
            REASON_EMPTY_AUTHORITY_ROOT,
            "observe",
            "the authority shelf exists but does not contain local authority material yet",
        ));
    }
    if !inventory.state_root_exists {
        reasons.push(reason(
            REASON_CERTIFICATE_STATE_ROOT_MISSING,
            "observe",
            "the certificate state root has not been prepared yet",
        ));
    }
    if expired_certificate_count > 0 {
        reasons.push(reason(
            REASON_EXPIRED_CERTIFICATE_MATERIAL,
            "warning",
            "certificate material has already expired and should not be trusted for remote or identity workflows",
        ));
    } else if expiring_certificate_count > 0 {
        reasons.push(reason(
            REASON_EXPIRING_CERTIFICATE_MATERIAL,
            "observe",
            "certificate material is approaching expiry and should be rotated soon",
        ));
    }
    if overdue_rotation_count > 0 {
        reasons.push(reason(
            REASON_OVERDUE_CERTIFICATE_ROTATION,
            "warning",
            "certificate rotation records show overdue or failed rotations that need operator attention",
        ));
    }
    if revoked_material_count > 0 {
        reasons.push(reason(
            REASON_REVOKED_CERTIFICATE_MATERIAL,
            "warning",
            "revocation records mark certificate material as revoked and no longer safe for runtime use",
        ));
    }
    if distrusted_trust_anchor_count > 0 {
        reasons.push(reason(
            REASON_DISTRUSTED_TRUST_ANCHOR_MATERIAL,
            "warning",
            "trust-anchor material has been explicitly distrusted and should not be used as a remote trust root",
        ));
    }
    if reasons.is_empty() && trust_certs == 0 && identity_certs == 0 && identity_private_keys == 0 {
        reasons.push(reason(
            REASON_CERTIFICATE_SHELF_BOOTSTRAP_EMPTY,
            "observe",
            "the certificate shelf is empty and still in bootstrap posture",
        ));
    }

    let (status, severity) = summarize_policy_status(&reasons);
    CertificatePolicyView {
        status,
        severity,
        summary: summarize_policy_text(&reasons, status),
        recommended_actions: recommended_actions(&reasons),
        reasons,
    }
}

fn empty_certificate_state(
    inventory: &CertificateInventory,
) -> crate::certificate_state::CertificateRuntimeState {
    crate::certificate_state::CertificateRuntimeState {
        root: inventory.state_root.clone(),
        rotation_records_path: inventory.state_root.join("rotation-records.tsv"),
        revocation_records_path: inventory.state_root.join("revocation-records.tsv"),
        rotation_records_exist: false,
        revocation_records_exist: false,
        rotation_records: Vec::new(),
        revocation_records: Vec::new(),
    }
}

fn reason(
    code: &'static str,
    severity: &'static str,
    summary: &'static str,
) -> CertificatePolicyReason {
    CertificatePolicyReason {
        code,
        severity,
        summary,
    }
}

fn count_kind(items: &[CertificateItem], kind: CertificateAssetKind) -> usize {
    items.iter().filter(|item| item.asset_kind == kind).count()
}

fn count_cert_like(items: &[CertificateItem]) -> usize {
    items
        .iter()
        .filter(|item| {
            matches!(
                item.asset_kind,
                CertificateAssetKind::CertificatePem
                    | CertificateAssetKind::ChainPem
                    | CertificateAssetKind::BundlePem
            )
        })
        .count()
}

fn count_expired_certificate_items(inventory: &CertificateInventory, now_unix_ms: i128) -> usize {
    certificate_items(inventory)
        .filter(|item| {
            item.validity
                .as_ref()
                .and_then(|validity| validity.earliest_not_after_unix_ms)
                .is_some_and(|not_after| not_after < now_unix_ms)
        })
        .count()
}

fn count_expiring_certificate_items(
    inventory: &CertificateInventory,
    now_unix_ms: i128,
    expiring_window_ms: i128,
) -> usize {
    certificate_items(inventory)
        .filter(|item| {
            item.validity
                .as_ref()
                .and_then(|validity| validity.earliest_not_after_unix_ms)
                .is_some_and(|not_after| {
                    not_after >= now_unix_ms && not_after <= now_unix_ms + expiring_window_ms
                })
        })
        .count()
}

fn certificate_items(inventory: &CertificateInventory) -> impl Iterator<Item = &CertificateItem> {
    inventory
        .trust_items
        .iter()
        .chain(inventory.authority_items.iter())
        .chain(inventory.identity_items.iter())
}

fn current_unix_ms() -> i128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|value| value.as_millis() as i128)
        .unwrap_or_default()
}

fn summarize_policy_status(reasons: &[CertificatePolicyReason]) -> (&'static str, &'static str) {
    if reasons.iter().any(|reason| reason.severity == "warning") {
        ("attention", "warning")
    } else if reasons.iter().any(|reason| reason.severity == "observe") {
        ("observe", "observe")
    } else {
        ("healthy", "ok")
    }
}

fn summarize_policy_text(reasons: &[CertificatePolicyReason], status: &str) -> String {
    match status {
        "attention" => {
            "certificate shelf needs operator attention before remote trust or identity use".into()
        }
        "observe" => {
            "certificate shelf is safe to inspect but still incomplete or in bootstrap posture"
                .into()
        }
        _ if reasons.is_empty() => {
            "certificate shelf posture is healthy and consistent with current policy".into()
        }
        _ => "certificate shelf posture is healthy".into(),
    }
}

fn recommended_actions(reasons: &[CertificatePolicyReason]) -> Vec<&'static str> {
    let mut actions = Vec::new();
    if reasons
        .iter()
        .any(|reason| reason.code == REASON_CERTIFICATE_INVENTORY_TRUNCATED)
    {
        actions.push(
            "reduce or partition the certificate shelves before relying on inventory policy results",
        );
    }
    if reasons
        .iter()
        .any(|reason| reason.code == REASON_EXPLICIT_REMOTE_TRUST_WITHOUT_ANCHORS)
    {
        actions.push("add at least one trust anchor before relying on protected remote endpoints");
    }
    if reasons
        .iter()
        .any(|reason| reason.code == REASON_PRIVATE_KEYS_PRESENT_IN_TRUST_ROOT)
    {
        actions.push("move private keys out of the trust root and keep that shelf anchor-only");
    }
    if reasons
        .iter()
        .any(|reason| reason.code == REASON_IDENTITY_KEYS_WITHOUT_CERTIFICATES)
    {
        actions.push("pair runtime identity keys with matching certificates before enabling identity-based transport");
    }
    if reasons
        .iter()
        .any(|reason| reason.code == REASON_CERTIFICATE_STATE_ROOT_MISSING)
    {
        actions.push("prepare the certificate state root before relying on runtime-managed certificate workflows");
    }
    if reasons
        .iter()
        .any(|reason| reason.code == REASON_EXPIRED_CERTIFICATE_MATERIAL)
    {
        actions.push("replace expired certificate material before using remote trust, authority, or runtime identity features");
    }
    if reasons
        .iter()
        .any(|reason| reason.code == REASON_EXPIRING_CERTIFICATE_MATERIAL)
    {
        actions.push(
            "schedule a certificate rotation before the current material reaches its expiry window",
        );
    }
    if reasons
        .iter()
        .any(|reason| reason.code == REASON_OVERDUE_CERTIFICATE_ROTATION)
    {
        actions.push("resolve overdue or failed certificate rotations recorded under the certificate state shelf");
    }
    if reasons
        .iter()
        .any(|reason| reason.code == REASON_REVOKED_CERTIFICATE_MATERIAL)
    {
        actions.push("remove or replace revoked certificate material before using the affected runtime workflows");
    }
    if reasons
        .iter()
        .any(|reason| reason.code == REASON_DISTRUSTED_TRUST_ANCHOR_MATERIAL)
    {
        actions.push("remove distrusted trust anchors from active use and replace them with approved trust material");
    }
    if actions.is_empty() {
        actions.push("no operator action required");
    }
    actions
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::certificate_inventory::CertificateInventory;
    use crate::certificate_state::CertificateRuntimeState;
    use std::path::PathBuf;

    fn inventory() -> CertificateInventory {
        CertificateInventory {
            root: PathBuf::from("/tmp/certificates"),
            trust_root: PathBuf::from("/tmp/certificates/trust"),
            authority_root: PathBuf::from("/tmp/certificates/authorities"),
            identity_root: PathBuf::from("/tmp/certificates/identities"),
            state_root: PathBuf::from("/tmp/state/certificates"),
            root_exists: true,
            trust_root_exists: true,
            authority_root_exists: true,
            identity_root_exists: true,
            state_root_exists: true,
            require_explicit_remote_trust: true,
            trust_items: Vec::new(),
            authority_items: Vec::new(),
            identity_items: Vec::new(),
        }
    }

    fn empty_state() -> CertificateRuntimeState {
        CertificateRuntimeState {
            root: PathBuf::from("/tmp/state/certificates"),
            rotation_records_path: PathBuf::from("/tmp/state/certificates/rotation-records.tsv"),
            revocation_records_path: PathBuf::from(
                "/tmp/state/certificates/revocation-records.tsv",
            ),
            rotation_records_exist: false,
            revocation_records_exist: false,
            rotation_records: Vec::new(),
            revocation_records: Vec::new(),
        }
    }

    #[test]
    fn policy_marks_missing_trust_anchors_as_attention() {
        let view = certificate_policy_for_inventory(&inventory());
        assert_eq!(view.status, "attention");
        assert!(
            view.reasons
                .iter()
                .any(|reason| reason.code == REASON_EXPLICIT_REMOTE_TRUST_WITHOUT_ANCHORS)
        );
    }

    #[test]
    fn policy_flags_private_keys_in_trust_root() {
        let mut inventory = inventory();
        inventory.trust_items.push(CertificateItem {
            relative_path: "anchors/root.key".into(),
            asset_kind: CertificateAssetKind::PrivateKeyPem,
            bytes: 128,
            modified_unix_ms: None,
            validity: None,
        });
        let view = certificate_policy_for_inventory(&inventory);
        assert!(
            view.reasons
                .iter()
                .any(|reason| reason.code == REASON_PRIVATE_KEYS_PRESENT_IN_TRUST_ROOT)
        );
    }

    #[test]
    fn policy_fails_conservatively_when_inventory_is_truncated() {
        let mut inventory = inventory();
        inventory.require_explicit_remote_trust = false;
        inventory.authority_root_exists = false;
        let view = certificate_policy_for_inventory_scan(&CertificateInventoryScan {
            inventory,
            truncated: true,
        });

        assert_eq!(view.status, "attention");
        assert!(
            view.reasons
                .iter()
                .any(|reason| reason.code == REASON_CERTIFICATE_INVENTORY_TRUNCATED)
        );
        assert!(
            view.recommended_actions
                .iter()
                .any(|action| { action.contains("reduce or partition the certificate shelves") })
        );
    }

    #[test]
    fn policy_is_healthy_with_anchor_identity_and_state_root() {
        let mut inventory = inventory();
        inventory.require_explicit_remote_trust = false;
        inventory.authority_root_exists = false;
        inventory.trust_items.push(CertificateItem {
            relative_path: "anchors/root-ca.pem".into(),
            asset_kind: CertificateAssetKind::BundlePem,
            bytes: 128,
            modified_unix_ms: None,
            validity: Some(crate::certificate_validity::CertificateValidityWindow {
                certificate_count: 1,
                earliest_not_before_unix_ms: Some(1_750_000_000_000),
                earliest_not_after_unix_ms: Some(2_060_000_000_000),
                latest_not_after_unix_ms: Some(2_060_000_000_000),
            }),
        });
        inventory.identity_items.push(CertificateItem {
            relative_path: "prod/runtime.pem".into(),
            asset_kind: CertificateAssetKind::CertificatePem,
            bytes: 256,
            modified_unix_ms: None,
            validity: Some(crate::certificate_validity::CertificateValidityWindow {
                certificate_count: 1,
                earliest_not_before_unix_ms: Some(1_750_000_000_000),
                earliest_not_after_unix_ms: Some(2_060_000_000_000),
                latest_not_after_unix_ms: Some(2_060_000_000_000),
            }),
        });
        inventory.identity_items.push(CertificateItem {
            relative_path: "prod/runtime.key".into(),
            asset_kind: CertificateAssetKind::PrivateKeyPem,
            bytes: 128,
            modified_unix_ms: None,
            validity: None,
        });
        let view = certificate_policy_for_inventory(&inventory);
        assert_eq!(view.status, "healthy");
        assert_eq!(view.severity, "ok");
        assert!(view.reasons.is_empty());
    }

    #[test]
    fn policy_marks_expired_certificate_material_as_attention() {
        let mut inventory = inventory();
        inventory.require_explicit_remote_trust = false;
        inventory.authority_root_exists = false;
        inventory.trust_items.push(CertificateItem {
            relative_path: "anchors/root-ca.pem".into(),
            asset_kind: CertificateAssetKind::BundlePem,
            bytes: 128,
            modified_unix_ms: None,
            validity: Some(crate::certificate_validity::CertificateValidityWindow {
                certificate_count: 1,
                earliest_not_before_unix_ms: Some(1_600_000_000_000),
                earliest_not_after_unix_ms: Some(1),
                latest_not_after_unix_ms: Some(1),
            }),
        });
        let view = certificate_policy_for_inventory(&inventory);
        assert_eq!(view.status, "attention");
        assert!(
            view.reasons
                .iter()
                .any(|reason| reason.code == REASON_EXPIRED_CERTIFICATE_MATERIAL)
        );
    }

    #[test]
    fn policy_marks_expiring_certificate_material_as_observe() {
        let mut inventory = inventory();
        inventory.require_explicit_remote_trust = false;
        inventory.authority_root_exists = false;
        let now_unix_ms = current_unix_ms();
        inventory.identity_items.push(CertificateItem {
            relative_path: "prod/runtime.pem".into(),
            asset_kind: CertificateAssetKind::CertificatePem,
            bytes: 256,
            modified_unix_ms: None,
            validity: Some(crate::certificate_validity::CertificateValidityWindow {
                certificate_count: 1,
                earliest_not_before_unix_ms: Some(now_unix_ms - 1_000),
                earliest_not_after_unix_ms: Some(now_unix_ms + 7 * 24 * 60 * 60 * 1000),
                latest_not_after_unix_ms: Some(now_unix_ms + 7 * 24 * 60 * 60 * 1000),
            }),
        });
        let view = certificate_policy_for_inventory(&inventory);
        assert_eq!(view.status, "observe");
        assert!(
            view.reasons
                .iter()
                .any(|reason| reason.code == REASON_EXPIRING_CERTIFICATE_MATERIAL)
        );
    }

    #[test]
    fn policy_marks_overdue_rotation_and_revocation_state() {
        let mut inventory = inventory();
        inventory.require_explicit_remote_trust = false;
        inventory.authority_root_exists = false;
        let mut state = empty_state();
        state
            .rotation_records
            .push(crate::certificate_state::CertificateRotationRecord {
                relative_path: "identities/prod/runtime.pem".into(),
                status: CertificateRotationStatus::Overdue,
                due_unix_ms: Some(10),
                last_rotated_unix_ms: Some(5),
                updated_unix_ms: Some(12),
                note: Some("stuck".into()),
            });
        state
            .revocation_records
            .push(crate::certificate_state::CertificateRevocationRecord {
                relative_path: "trust/anchors/root-ca.pem".into(),
                scope: CertificateMaterialScope::Trust,
                status: CertificateRevocationStatus::Distrusted,
                effective_unix_ms: Some(20),
                updated_unix_ms: Some(21),
                note: Some("legacy anchor".into()),
            });
        let view = certificate_policy_for_inventory_and_state(&inventory, &state);
        assert!(
            view.reasons
                .iter()
                .any(|reason| reason.code == REASON_OVERDUE_CERTIFICATE_ROTATION)
        );
        assert!(
            view.reasons
                .iter()
                .any(|reason| reason.code == REASON_DISTRUSTED_TRUST_ANCHOR_MATERIAL)
        );
    }
}
