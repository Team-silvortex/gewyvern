use crate::certificate_validity::{CertificateValidityWindow, inspect_certificate_validity};
use crate::runtime_layout::{RuntimeLayout, runtime_layout};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CertificateInventory {
    pub root: PathBuf,
    pub trust_root: PathBuf,
    pub authority_root: PathBuf,
    pub identity_root: PathBuf,
    pub state_root: PathBuf,
    pub root_exists: bool,
    pub trust_root_exists: bool,
    pub authority_root_exists: bool,
    pub identity_root_exists: bool,
    pub state_root_exists: bool,
    pub require_explicit_remote_trust: bool,
    pub trust_items: Vec<CertificateItem>,
    pub authority_items: Vec<CertificateItem>,
    pub identity_items: Vec<CertificateItem>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CertificateItem {
    pub relative_path: String,
    pub asset_kind: CertificateAssetKind,
    pub bytes: u64,
    pub modified_unix_ms: Option<u128>,
    pub validity: Option<CertificateValidityWindow>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CertificateAssetKind {
    CertificatePem,
    PrivateKeyPem,
    ChainPem,
    BundlePem,
    UnknownPem,
    Other,
}

pub fn runtime_certificate_inventory() -> CertificateInventory {
    runtime_certificate_inventory_from_layout(runtime_layout(), require_explicit_remote_trust())
}

pub fn runtime_certificate_inventory_from_layout(
    layout: RuntimeLayout,
    require_explicit_remote_trust: bool,
) -> CertificateInventory {
    let root_exists = layout.certificate_root.exists();
    let trust_root_exists = layout.trust_root.exists();
    let authority_root_exists = layout.authority_root.exists();
    let identity_root_exists = layout.identity_root.exists();
    let state_root_exists = layout.certificate_state_root.exists();
    CertificateInventory {
        root: layout.certificate_root,
        trust_root: layout.trust_root.clone(),
        authority_root: layout.authority_root.clone(),
        identity_root: layout.identity_root.clone(),
        state_root: layout.certificate_state_root,
        root_exists,
        trust_root_exists,
        authority_root_exists,
        identity_root_exists,
        state_root_exists,
        require_explicit_remote_trust,
        trust_items: scan_certificate_dir(&layout.trust_root),
        authority_items: scan_certificate_dir(&layout.authority_root),
        identity_items: scan_certificate_dir(&layout.identity_root),
    }
}

pub fn require_explicit_remote_trust() -> bool {
    match std::env::var("GEWY_REQUIRE_EXPLICIT_REMOTE_TRUST") {
        Ok(value) => !matches!(value.trim(), "false" | "0" | "no"),
        Err(_) => true,
    }
}

fn scan_certificate_dir(root: &Path) -> Vec<CertificateItem> {
    let mut items = Vec::new();
    visit_certificate_dir(root, root, &mut items);
    items.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    items
}

fn visit_certificate_dir(root: &Path, dir: &Path, items: &mut Vec<CertificateItem>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            visit_certificate_dir(root, &path, items);
            continue;
        }
        if !file_type.is_file() {
            continue;
        }
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        let relative_path = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        let asset_kind = classify_certificate_asset(&relative_path);
        items.push(CertificateItem {
            relative_path: relative_path.clone(),
            asset_kind,
            bytes: metadata.len(),
            modified_unix_ms: metadata
                .modified()
                .ok()
                .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
                .map(|value| value.as_millis()),
            validity: inspect_certificate_validity(&path, asset_kind),
        });
    }
}

fn classify_certificate_asset(relative_path: &str) -> CertificateAssetKind {
    let lower = relative_path.to_ascii_lowercase();
    if lower.ends_with(".pem") || lower.ends_with(".crt") || lower.ends_with(".cer") {
        if lower.contains("key") || lower.ends_with(".key.pem") {
            return CertificateAssetKind::PrivateKeyPem;
        }
        if lower.contains("chain") {
            return CertificateAssetKind::ChainPem;
        }
        if lower.contains("bundle") || lower.contains("trust") || lower.contains("ca") {
            return CertificateAssetKind::BundlePem;
        }
        return CertificateAssetKind::CertificatePem;
    }
    if lower.ends_with(".key") {
        return CertificateAssetKind::PrivateKeyPem;
    }
    if lower.ends_with(".p7b") || lower.ends_with(".p12") || lower.ends_with(".pfx") {
        return CertificateAssetKind::Other;
    }
    if lower.contains("pem") {
        return CertificateAssetKind::UnknownPem;
    }
    CertificateAssetKind::Other
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "gewyvern-certificate-inventory-{label}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn inventory_scans_certificate_roots_and_classifies_assets() {
        let root = temp_root("scan");
        let trust_root = root.join("trust");
        let authority_root = root.join("authorities");
        let identity_root = root.join("identities");
        fs::create_dir_all(trust_root.join("anchors")).unwrap();
        fs::create_dir_all(&authority_root).unwrap();
        fs::create_dir_all(identity_root.join("prod")).unwrap();
        fs::write(trust_root.join("anchors/root-ca.pem"), "pem").unwrap();
        fs::write(authority_root.join("issuing-chain.pem"), "chain").unwrap();
        fs::write(identity_root.join("prod/runtime.key"), "key").unwrap();
        fs::write(identity_root.join("prod/runtime-cert.pem"), "cert").unwrap();

        let inventory = runtime_certificate_inventory_from_layout(
            RuntimeLayout {
                config_root: PathBuf::from("/tmp/config"),
                data_root: PathBuf::from("/tmp/data"),
                state_root: PathBuf::from("/tmp/state"),
                cache_root: PathBuf::from("/tmp/cache"),
                certificate_root: root.clone(),
                trust_root: trust_root.clone(),
                authority_root: authority_root.clone(),
                identity_root: identity_root.clone(),
                certificate_state_root: PathBuf::from("/tmp/state/certificates"),
                legacy_root: None,
            },
            true,
        );

        assert_eq!(inventory.trust_items.len(), 1);
        assert_eq!(inventory.authority_items.len(), 1);
        assert_eq!(inventory.identity_items.len(), 2);
        assert!(inventory.root_exists);
        assert!(inventory.trust_root_exists);
        assert!(inventory.authority_root_exists);
        assert!(inventory.identity_root_exists);
        assert_eq!(
            inventory.trust_items[0].asset_kind,
            CertificateAssetKind::BundlePem
        );
        assert_eq!(
            inventory.authority_items[0].asset_kind,
            CertificateAssetKind::ChainPem
        );
        assert!(inventory.identity_items.iter().any(|item| {
            item.relative_path == "prod/runtime.key"
                && item.asset_kind == CertificateAssetKind::PrivateKeyPem
        }));
        assert!(inventory
            .identity_items
            .iter()
            .filter(|item| item.asset_kind == CertificateAssetKind::PrivateKeyPem)
            .all(|item| item.validity.is_none()));

        fs::remove_dir_all(root).unwrap();
    }
}
