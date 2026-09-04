use crate::certificate_validity::{CertificateValidityWindow, inspect_certificate_validity};
use crate::runtime_layout::{RuntimeLayout, runtime_layout};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

const MAX_CERTIFICATE_SCAN_DEPTH: usize = 32;
const MAX_CERTIFICATE_SCAN_ENTRIES_PER_ROOT: usize = 1024;
const MAX_CERTIFICATE_ITEMS_PER_ROOT: usize = 256;
const MAX_CERTIFICATE_VALIDITY_BYTES_PER_ROOT: u64 = 8 * 1024 * 1024;

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
pub struct CertificateInventoryScan {
    pub inventory: CertificateInventory,
    pub truncated: bool,
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
    runtime_certificate_inventory_scan().inventory
}

pub fn runtime_certificate_inventory_scan() -> CertificateInventoryScan {
    runtime_certificate_inventory_scan_from_layout(
        runtime_layout(),
        require_explicit_remote_trust(),
    )
}

pub fn runtime_certificate_inventory_from_layout(
    layout: RuntimeLayout,
    require_explicit_remote_trust: bool,
) -> CertificateInventory {
    runtime_certificate_inventory_scan_from_layout(layout, require_explicit_remote_trust).inventory
}

pub fn runtime_certificate_inventory_scan_from_layout(
    layout: RuntimeLayout,
    require_explicit_remote_trust: bool,
) -> CertificateInventoryScan {
    let root_exists = layout.certificate_root.exists();
    let trust_root_exists = layout.trust_root.exists();
    let authority_root_exists = layout.authority_root.exists();
    let identity_root_exists = layout.identity_root.exists();
    let state_root_exists = layout.certificate_state_root.exists();
    let trust_scan = scan_certificate_dir(&layout.trust_root);
    let authority_scan = scan_certificate_dir(&layout.authority_root);
    let identity_scan = scan_certificate_dir(&layout.identity_root);
    let scan_truncated =
        trust_scan.truncated || authority_scan.truncated || identity_scan.truncated;
    let inventory = CertificateInventory {
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
        trust_items: trust_scan.items,
        authority_items: authority_scan.items,
        identity_items: identity_scan.items,
    };
    CertificateInventoryScan {
        inventory,
        truncated: scan_truncated,
    }
}

pub fn require_explicit_remote_trust() -> bool {
    match std::env::var("GEWY_REQUIRE_EXPLICIT_REMOTE_TRUST") {
        Ok(value) => !matches!(value.trim(), "false" | "0" | "no"),
        Err(_) => true,
    }
}

#[derive(Default)]
struct CertificateScan {
    items: Vec<CertificateItem>,
    truncated: bool,
}

fn scan_certificate_dir(root: &Path) -> CertificateScan {
    let root_metadata = match fs::symlink_metadata(root) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return CertificateScan::default();
        }
        Err(_) => {
            return CertificateScan {
                truncated: true,
                ..CertificateScan::default()
            };
        }
    };
    if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
        return CertificateScan {
            truncated: true,
            ..CertificateScan::default()
        };
    }

    let mut items = Vec::new();
    let mut pending = vec![(root.to_path_buf(), 0_usize)];
    let mut observed_entries = 0_usize;
    let mut validity_bytes_remaining = MAX_CERTIFICATE_VALIDITY_BYTES_PER_ROOT;
    let mut truncated = false;

    while let Some((dir, depth)) = pending.pop() {
        let metadata = match fs::symlink_metadata(&dir) {
            Ok(metadata) => metadata,
            Err(_) => {
                truncated = true;
                continue;
            }
        };
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            truncated = true;
            continue;
        }
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => {
                truncated = true;
                continue;
            }
        };
        for entry in entries {
            if observed_entries >= MAX_CERTIFICATE_SCAN_ENTRIES_PER_ROOT {
                truncated = true;
                pending.clear();
                break;
            }
            observed_entries += 1;
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => {
                    truncated = true;
                    continue;
                }
            };
            let path = entry.path();
            let file_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(_) => {
                    truncated = true;
                    continue;
                }
            };
            if file_type.is_dir() {
                if depth >= MAX_CERTIFICATE_SCAN_DEPTH {
                    truncated = true;
                } else {
                    pending.push((path, depth + 1));
                }
                continue;
            }
            if !file_type.is_file() {
                continue;
            }
            if items.len() >= MAX_CERTIFICATE_ITEMS_PER_ROOT {
                truncated = true;
                pending.clear();
                break;
            }
            let metadata = match entry.metadata() {
                Ok(metadata) => metadata,
                Err(_) => {
                    truncated = true;
                    continue;
                }
            };
            let relative_path = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            let asset_kind = classify_certificate_asset(&relative_path);
            let validity = if certificate_kind_has_validity(asset_kind) {
                if metadata.len() > validity_bytes_remaining {
                    truncated = true;
                    None
                } else {
                    validity_bytes_remaining -= metadata.len();
                    inspect_certificate_validity(&path, asset_kind)
                }
            } else {
                None
            };
            items.push(CertificateItem {
                relative_path,
                asset_kind,
                bytes: metadata.len(),
                modified_unix_ms: metadata
                    .modified()
                    .ok()
                    .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
                    .map(|value| value.as_millis()),
                validity,
            });
        }
    }
    items.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    CertificateScan { items, truncated }
}

fn certificate_kind_has_validity(kind: CertificateAssetKind) -> bool {
    matches!(
        kind,
        CertificateAssetKind::CertificatePem
            | CertificateAssetKind::ChainPem
            | CertificateAssetKind::BundlePem
            | CertificateAssetKind::UnknownPem
    )
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
        assert!(
            inventory
                .identity_items
                .iter()
                .filter(|item| item.asset_kind == CertificateAssetKind::PrivateKeyPem)
                .all(|item| item.validity.is_none())
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn inventory_scan_is_iterative_bounded_and_reports_truncation() {
        let item_root = temp_root("item-limit");
        fs::create_dir_all(&item_root).unwrap();
        for index in 0..=MAX_CERTIFICATE_ITEMS_PER_ROOT {
            fs::write(
                item_root.join(format!("certificate-{index:04}.pem")),
                "invalid",
            )
            .unwrap();
        }

        let item_scan = scan_certificate_dir(&item_root);
        assert_eq!(item_scan.items.len(), MAX_CERTIFICATE_ITEMS_PER_ROOT);
        assert!(item_scan.truncated);
        fs::remove_dir_all(item_root).unwrap();

        let depth_root = temp_root("depth-limit");
        fs::create_dir_all(&depth_root).unwrap();
        let mut directory = depth_root.clone();
        for index in 0..=MAX_CERTIFICATE_SCAN_DEPTH {
            directory = directory.join(format!("level-{index:02}"));
            fs::create_dir(&directory).unwrap();
        }

        let depth_scan = scan_certificate_dir(&depth_root);
        assert!(depth_scan.items.is_empty());
        assert!(depth_scan.truncated);
        fs::remove_dir_all(depth_root).unwrap();
    }
}
