use crate::certificate_inventory::{
    CertificateAssetKind, CertificateInventory, CertificateItem, runtime_certificate_inventory,
};
use crate::runtime_layout::{RuntimeLayout, runtime_layout};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const ROTATION_RECORDS_FILE: &str = "rotation-records.tsv";
const REVOCATION_RECORDS_FILE: &str = "revocation-records.tsv";
const AUTO_ROTATION_NOTE: &str = "auto:validity-sync";
const ROTATION_DUE_WINDOW_MS: i128 = 30_i128 * 24 * 60 * 60 * 1000;

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CertificateRotationSyncReport {
    pub updated_record_count: usize,
    pub active_count: usize,
    pub due_count: usize,
    pub overdue_count: usize,
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

pub fn sync_rotation_records_from_inventory() -> Result<CertificateRotationSyncReport, String> {
    sync_rotation_records_from_inventory_at(runtime_current_unix_ms())
}

pub fn sync_rotation_records_from_inventory_at(
    now_unix_ms: i128,
) -> Result<CertificateRotationSyncReport, String> {
    let layout = runtime_layout();
    fs::create_dir_all(&layout.certificate_state_root).map_err(|err| {
        format!(
            "failed to prepare certificate state root '{}': {err}",
            layout.certificate_state_root.display()
        )
    })?;
    let inventory = runtime_certificate_inventory();
    let mut state = runtime_certificate_state_from_layout(layout);
    let generated = generated_rotation_records(&inventory, now_unix_ms);
    let generated_paths = generated
        .iter()
        .map(|record| record.relative_path.clone())
        .collect::<Vec<_>>();
    state.rotation_records.retain(|record| {
        let managed = record
            .note
            .as_deref()
            .is_some_and(|note| note.starts_with(AUTO_ROTATION_NOTE));
        if managed {
            return false;
        }
        !generated_paths.contains(&record.relative_path)
    });
    state.rotation_records.extend(generated.clone());
    state
        .rotation_records
        .sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    write_rotation_records_file(&state.rotation_records_path, &state.rotation_records)?;
    Ok(CertificateRotationSyncReport {
        updated_record_count: generated.len(),
        active_count: generated
            .iter()
            .filter(|record| record.status == CertificateRotationStatus::Active)
            .count(),
        due_count: generated
            .iter()
            .filter(|record| record.status == CertificateRotationStatus::Due)
            .count(),
        overdue_count: generated
            .iter()
            .filter(|record| record.status == CertificateRotationStatus::Overdue)
            .count(),
    })
}

pub fn write_rotation_record(
    relative_path: &str,
    status: CertificateRotationStatus,
    due_unix_ms: Option<i128>,
    last_rotated_unix_ms: Option<i128>,
    updated_unix_ms: Option<i128>,
    note: Option<&str>,
) -> Result<(), String> {
    let layout = runtime_layout();
    fs::create_dir_all(&layout.certificate_state_root).map_err(|err| {
        format!(
            "failed to prepare certificate state root '{}': {err}",
            layout.certificate_state_root.display()
        )
    })?;
    let mut state = runtime_certificate_state_from_layout(layout);
    upsert_rotation_record(
        &mut state.rotation_records,
        CertificateRotationRecord {
            relative_path: relative_path.trim().to_string(),
            status,
            due_unix_ms,
            last_rotated_unix_ms,
            updated_unix_ms,
            note: note.map(|value| value.trim().to_string()),
        },
    );
    write_rotation_records_file(&state.rotation_records_path, &state.rotation_records)
}

pub fn remove_rotation_record(relative_path: &str) -> Result<bool, String> {
    let layout = runtime_layout();
    let mut state = runtime_certificate_state_from_layout(layout);
    let before = state.rotation_records.len();
    state
        .rotation_records
        .retain(|record| record.relative_path != relative_path.trim());
    let changed = state.rotation_records.len() != before;
    if changed {
        write_rotation_records_file(&state.rotation_records_path, &state.rotation_records)?;
    }
    Ok(changed)
}

pub fn write_revocation_record(
    relative_path: &str,
    scope: CertificateMaterialScope,
    status: CertificateRevocationStatus,
    effective_unix_ms: Option<i128>,
    updated_unix_ms: Option<i128>,
    note: Option<&str>,
) -> Result<(), String> {
    let layout = runtime_layout();
    fs::create_dir_all(&layout.certificate_state_root).map_err(|err| {
        format!(
            "failed to prepare certificate state root '{}': {err}",
            layout.certificate_state_root.display()
        )
    })?;
    let mut state = runtime_certificate_state_from_layout(layout);
    upsert_revocation_record(
        &mut state.revocation_records,
        CertificateRevocationRecord {
            relative_path: relative_path.trim().to_string(),
            scope,
            status,
            effective_unix_ms,
            updated_unix_ms,
            note: note.map(|value| value.trim().to_string()),
        },
    );
    write_revocation_records_file(&state.revocation_records_path, &state.revocation_records)
}

pub fn remove_revocation_record(relative_path: &str) -> Result<bool, String> {
    let layout = runtime_layout();
    let mut state = runtime_certificate_state_from_layout(layout);
    let before = state.revocation_records.len();
    state
        .revocation_records
        .retain(|record| record.relative_path != relative_path.trim());
    let changed = state.revocation_records.len() != before;
    if changed {
        write_revocation_records_file(&state.revocation_records_path, &state.revocation_records)?;
    }
    Ok(changed)
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

fn generated_rotation_records(
    inventory: &CertificateInventory,
    now_unix_ms: i128,
) -> Vec<CertificateRotationRecord> {
    let mut records = Vec::new();
    append_generated_rotation_records(&mut records, "trust", &inventory.trust_items, now_unix_ms);
    append_generated_rotation_records(
        &mut records,
        "authority",
        &inventory.authority_items,
        now_unix_ms,
    );
    append_generated_rotation_records(
        &mut records,
        "identity",
        &inventory.identity_items,
        now_unix_ms,
    );
    records.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    records
}

fn append_generated_rotation_records(
    target: &mut Vec<CertificateRotationRecord>,
    shelf: &str,
    items: &[CertificateItem],
    now_unix_ms: i128,
) {
    for item in items {
        if !matches!(
            item.asset_kind,
            CertificateAssetKind::CertificatePem
                | CertificateAssetKind::ChainPem
                | CertificateAssetKind::BundlePem
                | CertificateAssetKind::UnknownPem
        ) {
            continue;
        }
        let Some(validity) = item.validity.as_ref() else {
            continue;
        };
        let Some(not_after_unix_ms) = validity.earliest_not_after_unix_ms else {
            continue;
        };
        let status = if not_after_unix_ms <= now_unix_ms {
            CertificateRotationStatus::Overdue
        } else if not_after_unix_ms <= now_unix_ms + ROTATION_DUE_WINDOW_MS {
            CertificateRotationStatus::Due
        } else {
            CertificateRotationStatus::Active
        };
        target.push(CertificateRotationRecord {
            relative_path: format!("{shelf}/{}", item.relative_path),
            status,
            due_unix_ms: Some(not_after_unix_ms - ROTATION_DUE_WINDOW_MS),
            last_rotated_unix_ms: None,
            updated_unix_ms: Some(now_unix_ms),
            note: Some(AUTO_ROTATION_NOTE.into()),
        });
    }
}

fn upsert_rotation_record(
    records: &mut Vec<CertificateRotationRecord>,
    record: CertificateRotationRecord,
) {
    if let Some(existing) = records
        .iter_mut()
        .find(|existing| existing.relative_path == record.relative_path)
    {
        *existing = record;
    } else {
        records.push(record);
        records.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    }
}

fn upsert_revocation_record(
    records: &mut Vec<CertificateRevocationRecord>,
    record: CertificateRevocationRecord,
) {
    if let Some(existing) = records
        .iter_mut()
        .find(|existing| existing.relative_path == record.relative_path)
    {
        *existing = record;
    } else {
        records.push(record);
        records.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    }
}

fn write_rotation_records_file(
    path: &Path,
    records: &[CertificateRotationRecord],
) -> Result<(), String> {
    let mut rendered = String::new();
    for record in records {
        rendered.push_str(&record.relative_path);
        rendered.push('\t');
        rendered.push_str(rotation_status_label(record.status));
        rendered.push('\t');
        rendered.push_str(&optional_i128_text(record.due_unix_ms));
        rendered.push('\t');
        rendered.push_str(&optional_i128_text(record.last_rotated_unix_ms));
        rendered.push('\t');
        rendered.push_str(&optional_i128_text(record.updated_unix_ms));
        rendered.push('\t');
        rendered.push_str(record.note.as_deref().unwrap_or(""));
        rendered.push('\n');
    }
    fs::write(path, rendered).map_err(|err| {
        format!(
            "failed to write rotation records '{}': {err}",
            path.display()
        )
    })
}

fn write_revocation_records_file(
    path: &Path,
    records: &[CertificateRevocationRecord],
) -> Result<(), String> {
    let mut rendered = String::new();
    for record in records {
        rendered.push_str(&record.relative_path);
        rendered.push('\t');
        rendered.push_str(material_scope_label(record.scope));
        rendered.push('\t');
        rendered.push_str(revocation_status_label(record.status));
        rendered.push('\t');
        rendered.push_str(&optional_i128_text(record.effective_unix_ms));
        rendered.push('\t');
        rendered.push_str(&optional_i128_text(record.updated_unix_ms));
        rendered.push('\t');
        rendered.push_str(record.note.as_deref().unwrap_or(""));
        rendered.push('\n');
    }
    fs::write(path, rendered).map_err(|err| {
        format!(
            "failed to write revocation records '{}': {err}",
            path.display()
        )
    })
}

fn optional_i128_text(value: Option<i128>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "-".into())
}

fn runtime_current_unix_ms() -> i128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|value| value.as_millis() as i128)
        .unwrap_or_default()
}

pub fn rotation_status_label(status: CertificateRotationStatus) -> &'static str {
    match status {
        CertificateRotationStatus::Active => "active",
        CertificateRotationStatus::Due => "due",
        CertificateRotationStatus::Overdue => "overdue",
        CertificateRotationStatus::Error => "error",
    }
}

pub fn revocation_status_label(status: CertificateRevocationStatus) -> &'static str {
    match status {
        CertificateRevocationStatus::Revoked => "revoked",
        CertificateRevocationStatus::Distrusted => "distrusted",
        CertificateRevocationStatus::Cleared => "cleared",
    }
}

pub fn material_scope_label(scope: CertificateMaterialScope) -> &'static str {
    match scope {
        CertificateMaterialScope::Trust => "trust",
        CertificateMaterialScope::Authority => "authority",
        CertificateMaterialScope::Identity => "identity",
        CertificateMaterialScope::Other => "other",
    }
}

#[cfg(test)]
#[path = "certificate_state/tests.rs"]
mod tests;
