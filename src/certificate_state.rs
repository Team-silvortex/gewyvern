use crate::runtime_layout::{runtime_layout, RuntimeLayout};
use std::fs;
use std::path::{Path, PathBuf};

const ROTATION_RECORDS_FILE: &str = "rotation-records.tsv";
const REVOCATION_RECORDS_FILE: &str = "revocation-records.tsv";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CertificateRuntimeState {
    pub root: PathBuf,
    pub rotation_records_path: PathBuf,
    pub revocation_records_path: PathBuf,
    pub rotation_records_exist: bool,
    pub revocation_records_exist: bool,
    pub rotation_records: Vec<CertificateRotationRecord>,
    pub revocation_records: Vec<CertificateRevocationRecord>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CertificateRotationRecord {
    pub relative_path: String,
    pub status: CertificateRotationStatus,
    pub due_unix_ms: Option<i128>,
    pub last_rotated_unix_ms: Option<i128>,
    pub updated_unix_ms: Option<i128>,
    pub note: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CertificateRevocationRecord {
    pub relative_path: String,
    pub scope: CertificateMaterialScope,
    pub status: CertificateRevocationStatus,
    pub effective_unix_ms: Option<i128>,
    pub updated_unix_ms: Option<i128>,
    pub note: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CertificateRotationStatus {
    Active,
    Due,
    Overdue,
    Error,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CertificateRevocationStatus {
    Revoked,
    Distrusted,
    Cleared,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CertificateMaterialScope {
    Trust,
    Authority,
    Identity,
    Other,
}

pub fn runtime_certificate_state() -> CertificateRuntimeState {
    runtime_certificate_state_from_layout(runtime_layout())
}

pub fn runtime_certificate_state_from_layout(layout: RuntimeLayout) -> CertificateRuntimeState {
    let root = layout.certificate_state_root;
    let rotation_records_path = root.join(ROTATION_RECORDS_FILE);
    let revocation_records_path = root.join(REVOCATION_RECORDS_FILE);
    CertificateRuntimeState {
        root,
        rotation_records_exist: rotation_records_path.exists(),
        revocation_records_exist: revocation_records_path.exists(),
        rotation_records: read_rotation_records(&rotation_records_path),
        revocation_records: read_revocation_records(&revocation_records_path),
        rotation_records_path,
        revocation_records_path,
    }
}

fn read_rotation_records(path: &Path) -> Vec<CertificateRotationRecord> {
    let Ok(contents) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut records = Vec::new();
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields = line.split('\t').collect::<Vec<_>>();
        if fields.len() < 5 {
            continue;
        }
        records.push(CertificateRotationRecord {
            relative_path: fields[0].trim().to_string(),
            status: parse_rotation_status(fields[1]),
            due_unix_ms: parse_optional_i128(fields[2]),
            last_rotated_unix_ms: parse_optional_i128(fields[3]),
            updated_unix_ms: parse_optional_i128(fields[4]),
            note: fields.get(5).map(|value| value.trim().to_string()),
        });
    }
    records.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    records
}

fn read_revocation_records(path: &Path) -> Vec<CertificateRevocationRecord> {
    let Ok(contents) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut records = Vec::new();
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields = line.split('\t').collect::<Vec<_>>();
        if fields.len() < 5 {
            continue;
        }
        records.push(CertificateRevocationRecord {
            relative_path: fields[0].trim().to_string(),
            scope: parse_material_scope(fields[1]),
            status: parse_revocation_status(fields[2]),
            effective_unix_ms: parse_optional_i128(fields[3]),
            updated_unix_ms: parse_optional_i128(fields[4]),
            note: fields.get(5).map(|value| value.trim().to_string()),
        });
    }
    records.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    records
}

fn parse_rotation_status(value: &str) -> CertificateRotationStatus {
    match value.trim() {
        "due" => CertificateRotationStatus::Due,
        "overdue" => CertificateRotationStatus::Overdue,
        "error" => CertificateRotationStatus::Error,
        _ => CertificateRotationStatus::Active,
    }
}

fn parse_revocation_status(value: &str) -> CertificateRevocationStatus {
    match value.trim() {
        "distrusted" => CertificateRevocationStatus::Distrusted,
        "cleared" => CertificateRevocationStatus::Cleared,
        _ => CertificateRevocationStatus::Revoked,
    }
}

fn parse_material_scope(value: &str) -> CertificateMaterialScope {
    match value.trim() {
        "trust" => CertificateMaterialScope::Trust,
        "authority" => CertificateMaterialScope::Authority,
        "identity" => CertificateMaterialScope::Identity,
        _ => CertificateMaterialScope::Other,
    }
}

fn parse_optional_i128(value: &str) -> Option<i128> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed == "-" || trimmed.eq_ignore_ascii_case("null") {
        None
    } else {
        trimmed.parse::<i128>().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "gewyvern-certificate-state-{label}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn state_reads_rotation_and_revocation_records() {
        let root = temp_root("scan");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join(ROTATION_RECORDS_FILE),
            "identities/prod/runtime.pem\toverdue\t200\t100\t150\trotate now\n",
        )
        .unwrap();
        fs::write(
            root.join(REVOCATION_RECORDS_FILE),
            "trust/anchors/root-ca.pem\ttrust\tdistrusted\t300\t350\tlegacy anchor\n",
        )
        .unwrap();

        let state = runtime_certificate_state_from_layout(RuntimeLayout {
            config_root: PathBuf::from("/tmp/config"),
            data_root: PathBuf::from("/tmp/data"),
            state_root: PathBuf::from("/tmp/state"),
            cache_root: PathBuf::from("/tmp/cache"),
            certificate_root: PathBuf::from("/tmp/config/certificates"),
            trust_root: PathBuf::from("/tmp/config/certificates/trust"),
            authority_root: PathBuf::from("/tmp/config/certificates/authorities"),
            identity_root: PathBuf::from("/tmp/config/certificates/identities"),
            certificate_state_root: root.clone(),
            legacy_root: None,
        });

        assert!(state.rotation_records_exist);
        assert!(state.revocation_records_exist);
        assert_eq!(state.rotation_records.len(), 1);
        assert_eq!(
            state.rotation_records[0].status,
            CertificateRotationStatus::Overdue
        );
        assert_eq!(state.revocation_records.len(), 1);
        assert_eq!(
            state.revocation_records[0].status,
            CertificateRevocationStatus::Distrusted
        );

        fs::remove_dir_all(root).unwrap();
    }
}
