use crate::certificate_inventory::{
    CertificateAssetKind, CertificateInventory, CertificateItem, runtime_certificate_inventory,
};
use crate::runtime_layout::{RuntimeLayout, runtime_layout};
use silvortex_bounded_io::read_bounded_utf8_regular_file;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const ROTATION_RECORDS_FILE: &str = "rotation-records.tsv";
const REVOCATION_RECORDS_FILE: &str = "revocation-records.tsv";
const AUTO_ROTATION_NOTE: &str = "auto:validity-sync";
const ROTATION_DUE_WINDOW_MS: i128 = 30_i128 * 24 * 60 * 60 * 1000;
pub const MAX_CERTIFICATE_STATE_FILE_BYTES: u64 = 1024 * 1024;
pub const MAX_CERTIFICATE_STATE_RECORDS: usize = 4096;
const MAX_CERTIFICATE_STATE_PATH_BYTES: usize = 1024;
const MAX_CERTIFICATE_STATE_NOTE_BYTES: usize = 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CertificateRuntimeState {
    pub root: PathBuf,
    pub rotation_records_path: PathBuf,
    pub revocation_records_path: PathBuf,
    pub rotation_records_exist: bool,
    pub revocation_records_exist: bool,
    pub rotation_records_valid: bool,
    pub revocation_records_valid: bool,
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
    let rotation_records_path = layout.certificate_state_root.join(ROTATION_RECORDS_FILE);
    let mut rotation_records = read_rotation_records(&rotation_records_path)?;
    let generated = generated_rotation_records(&inventory, now_unix_ms);
    let generated_paths = generated
        .iter()
        .map(|record| record.relative_path.clone())
        .collect::<Vec<_>>();
    rotation_records.retain(|record| {
        let managed = record
            .note
            .as_deref()
            .is_some_and(|note| note.starts_with(AUTO_ROTATION_NOTE));
        if managed {
            return false;
        }
        !generated_paths.contains(&record.relative_path)
    });
    rotation_records.extend(generated.clone());
    rotation_records.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    write_rotation_records_file(&rotation_records_path, &rotation_records)?;
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
    let relative_path = validate_record_relative_path(relative_path)?;
    let note = sanitize_record_note(note)?;
    let layout = runtime_layout();
    fs::create_dir_all(&layout.certificate_state_root).map_err(|err| {
        format!(
            "failed to prepare certificate state root '{}': {err}",
            layout.certificate_state_root.display()
        )
    })?;
    let rotation_records_path = layout.certificate_state_root.join(ROTATION_RECORDS_FILE);
    let mut rotation_records = read_rotation_records(&rotation_records_path)?;
    upsert_rotation_record(
        &mut rotation_records,
        CertificateRotationRecord {
            relative_path,
            status,
            due_unix_ms,
            last_rotated_unix_ms,
            updated_unix_ms,
            note,
        },
    );
    write_rotation_records_file(&rotation_records_path, &rotation_records)
}

pub fn remove_rotation_record(relative_path: &str) -> Result<bool, String> {
    let relative_path = validate_record_relative_path(relative_path)?;
    let layout = runtime_layout();
    let rotation_records_path = layout.certificate_state_root.join(ROTATION_RECORDS_FILE);
    let mut rotation_records = read_rotation_records(&rotation_records_path)?;
    let before = rotation_records.len();
    rotation_records.retain(|record| record.relative_path != relative_path);
    let changed = rotation_records.len() != before;
    if changed {
        write_rotation_records_file(&rotation_records_path, &rotation_records)?;
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
    let relative_path = validate_record_relative_path(relative_path)?;
    let note = sanitize_record_note(note)?;
    let layout = runtime_layout();
    fs::create_dir_all(&layout.certificate_state_root).map_err(|err| {
        format!(
            "failed to prepare certificate state root '{}': {err}",
            layout.certificate_state_root.display()
        )
    })?;
    let revocation_records_path = layout.certificate_state_root.join(REVOCATION_RECORDS_FILE);
    let mut revocation_records = read_revocation_records(&revocation_records_path)?;
    upsert_revocation_record(
        &mut revocation_records,
        CertificateRevocationRecord {
            relative_path,
            scope,
            status,
            effective_unix_ms,
            updated_unix_ms,
            note,
        },
    );
    write_revocation_records_file(&revocation_records_path, &revocation_records)
}

pub fn remove_revocation_record(relative_path: &str) -> Result<bool, String> {
    let relative_path = validate_record_relative_path(relative_path)?;
    let layout = runtime_layout();
    let revocation_records_path = layout.certificate_state_root.join(REVOCATION_RECORDS_FILE);
    let mut revocation_records = read_revocation_records(&revocation_records_path)?;
    let before = revocation_records.len();
    revocation_records.retain(|record| record.relative_path != relative_path);
    let changed = revocation_records.len() != before;
    if changed {
        write_revocation_records_file(&revocation_records_path, &revocation_records)?;
    }
    Ok(changed)
}

pub fn runtime_certificate_state_from_layout(layout: RuntimeLayout) -> CertificateRuntimeState {
    let root = layout.certificate_state_root;
    let rotation_records_path = root.join(ROTATION_RECORDS_FILE);
    let revocation_records_path = root.join(REVOCATION_RECORDS_FILE);
    let rotation_records_exist = state_file_exists(&rotation_records_path);
    let revocation_records_exist = state_file_exists(&revocation_records_path);
    let (rotation_records, rotation_records_valid) =
        match read_rotation_records(&rotation_records_path) {
            Ok(records) => (records, true),
            Err(_) => (Vec::new(), false),
        };
    let (revocation_records, revocation_records_valid) =
        match read_revocation_records(&revocation_records_path) {
            Ok(records) => (records, true),
            Err(_) => (Vec::new(), false),
        };
    CertificateRuntimeState {
        root,
        rotation_records_exist,
        revocation_records_exist,
        rotation_records_valid,
        revocation_records_valid,
        rotation_records,
        revocation_records,
        rotation_records_path,
        revocation_records_path,
    }
}

fn state_file_exists(path: &Path) -> bool {
    match fs::symlink_metadata(path) {
        Ok(_) => true,
        Err(error) => error.kind() != std::io::ErrorKind::NotFound,
    }
}

fn read_state_file(path: &Path, kind: &str) -> Result<Option<String>, String> {
    match read_bounded_utf8_regular_file(path, MAX_CERTIFICATE_STATE_FILE_BYTES) {
        Ok(contents) => Ok(Some(contents)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!(
            "failed to read {kind} records '{}': {error}",
            path.display()
        )),
    }
}

fn read_rotation_records(path: &Path) -> Result<Vec<CertificateRotationRecord>, String> {
    let Some(contents) = read_state_file(path, "rotation")? else {
        return Ok(Vec::new());
    };
    let mut records = Vec::new();
    for (line_index, line) in contents.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields = line.split('\t').collect::<Vec<_>>();
        let line_number = line_index + 1;
        if !matches!(fields.len(), 5 | 6) {
            return Err(invalid_record(
                "rotation",
                line_number,
                "expected five fields and an optional note",
            ));
        }
        if records.len() >= MAX_CERTIFICATE_STATE_RECORDS {
            return Err(format!(
                "rotation records exceed the limit of {MAX_CERTIFICATE_STATE_RECORDS}"
            ));
        }
        let relative_path = validate_record_relative_path(fields[0])
            .map_err(|error| invalid_record("rotation", line_number, &error))?;
        let status = parse_rotation_status(fields[1]).ok_or_else(|| {
            invalid_record("rotation", line_number, "unsupported rotation status")
        })?;
        let note = fields
            .get(5)
            .map(|value| sanitize_record_note(Some(value)))
            .transpose()
            .map_err(|error| invalid_record("rotation", line_number, &error))?
            .flatten();
        records.push(CertificateRotationRecord {
            relative_path,
            status,
            due_unix_ms: parse_optional_i128(fields[2], "due_unix_ms")
                .map_err(|error| invalid_record("rotation", line_number, &error))?,
            last_rotated_unix_ms: parse_optional_i128(fields[3], "last_rotated_unix_ms")
                .map_err(|error| invalid_record("rotation", line_number, &error))?,
            updated_unix_ms: parse_optional_i128(fields[4], "updated_unix_ms")
                .map_err(|error| invalid_record("rotation", line_number, &error))?,
            note,
        });
    }
    records.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    reject_duplicate_paths("rotation", &records)?;
    Ok(records)
}

fn read_revocation_records(path: &Path) -> Result<Vec<CertificateRevocationRecord>, String> {
    let Some(contents) = read_state_file(path, "revocation")? else {
        return Ok(Vec::new());
    };
    let mut records = Vec::new();
    for (line_index, line) in contents.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields = line.split('\t').collect::<Vec<_>>();
        let line_number = line_index + 1;
        if !matches!(fields.len(), 5 | 6) {
            return Err(invalid_record(
                "revocation",
                line_number,
                "expected five fields and an optional note",
            ));
        }
        if records.len() >= MAX_CERTIFICATE_STATE_RECORDS {
            return Err(format!(
                "revocation records exceed the limit of {MAX_CERTIFICATE_STATE_RECORDS}"
            ));
        }
        let relative_path = validate_record_relative_path(fields[0])
            .map_err(|error| invalid_record("revocation", line_number, &error))?;
        let scope = parse_material_scope(fields[1]).ok_or_else(|| {
            invalid_record(
                "revocation",
                line_number,
                "unsupported certificate material scope",
            )
        })?;
        let status = parse_revocation_status(fields[2]).ok_or_else(|| {
            invalid_record("revocation", line_number, "unsupported revocation status")
        })?;
        let note = fields
            .get(5)
            .map(|value| sanitize_record_note(Some(value)))
            .transpose()
            .map_err(|error| invalid_record("revocation", line_number, &error))?
            .flatten();
        records.push(CertificateRevocationRecord {
            relative_path,
            scope,
            status,
            effective_unix_ms: parse_optional_i128(fields[3], "effective_unix_ms")
                .map_err(|error| invalid_record("revocation", line_number, &error))?,
            updated_unix_ms: parse_optional_i128(fields[4], "updated_unix_ms")
                .map_err(|error| invalid_record("revocation", line_number, &error))?,
            note,
        });
    }
    records.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    reject_duplicate_paths("revocation", &records)?;
    Ok(records)
}

fn invalid_record(kind: &str, line_number: usize, detail: &str) -> String {
    format!("invalid {kind} record at line {line_number}: {detail}")
}

fn reject_duplicate_paths<T>(kind: &str, records: &[T]) -> Result<(), String>
where
    T: CertificateRecordPath,
{
    if let Some(pair) = records
        .windows(2)
        .find(|pair| pair[0].relative_path() == pair[1].relative_path())
    {
        return Err(format!(
            "duplicate {kind} record path '{}'",
            pair[0].relative_path()
        ));
    }
    Ok(())
}

trait CertificateRecordPath {
    fn relative_path(&self) -> &str;
}

impl CertificateRecordPath for CertificateRotationRecord {
    fn relative_path(&self) -> &str {
        &self.relative_path
    }
}

impl CertificateRecordPath for CertificateRevocationRecord {
    fn relative_path(&self) -> &str {
        &self.relative_path
    }
}

fn parse_rotation_status(value: &str) -> Option<CertificateRotationStatus> {
    match value.trim() {
        "active" => Some(CertificateRotationStatus::Active),
        "due" => Some(CertificateRotationStatus::Due),
        "overdue" => Some(CertificateRotationStatus::Overdue),
        "error" => Some(CertificateRotationStatus::Error),
        _ => None,
    }
}

fn parse_revocation_status(value: &str) -> Option<CertificateRevocationStatus> {
    match value.trim() {
        "revoked" => Some(CertificateRevocationStatus::Revoked),
        "distrusted" => Some(CertificateRevocationStatus::Distrusted),
        "cleared" => Some(CertificateRevocationStatus::Cleared),
        _ => None,
    }
}

fn parse_material_scope(value: &str) -> Option<CertificateMaterialScope> {
    match value.trim() {
        "trust" => Some(CertificateMaterialScope::Trust),
        "authority" => Some(CertificateMaterialScope::Authority),
        "identity" => Some(CertificateMaterialScope::Identity),
        "other" => Some(CertificateMaterialScope::Other),
        _ => None,
    }
}

fn validate_record_relative_path(value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("relative path must not be empty".into());
    }
    if trimmed.len() > MAX_CERTIFICATE_STATE_PATH_BYTES {
        return Err(format!(
            "relative path exceeds {MAX_CERTIFICATE_STATE_PATH_BYTES} bytes"
        ));
    }
    if trimmed.starts_with('/') {
        return Err("relative path must be relative".into());
    }
    if trimmed.contains('\\') {
        return Err("relative path must not contain backslash".into());
    }
    if trimmed.chars().any(|ch| ch.is_control()) {
        return Err("relative path must not contain control characters".into());
    }
    for segment in trimmed.split('/') {
        if segment.is_empty() || segment == "." || segment == ".." {
            return Err("relative path contains invalid segment".into());
        }
    }
    Ok(trimmed.to_string())
}

fn sanitize_record_note(note: Option<&str>) -> Result<Option<String>, String> {
    note.map(|value| {
        let trimmed = value.trim();
        if trimmed.len() > MAX_CERTIFICATE_STATE_NOTE_BYTES {
            return Err(format!(
                "note exceeds {MAX_CERTIFICATE_STATE_NOTE_BYTES} bytes"
            ));
        }
        if trimmed.chars().any(|ch| ch.is_control()) {
            Err("note contains control characters".into())
        } else {
            Ok(trimmed.to_string())
        }
    })
    .transpose()
}

fn parse_optional_i128(value: &str, field: &str) -> Result<Option<i128>, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed == "-" || trimmed.eq_ignore_ascii_case("null") {
        Ok(None)
    } else {
        trimmed
            .parse::<i128>()
            .map(Some)
            .map_err(|_| format!("{field} must be an integer or '-'"))
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
    if records.len() > MAX_CERTIFICATE_STATE_RECORDS {
        return Err(format!(
            "rotation records exceed the limit of {MAX_CERTIFICATE_STATE_RECORDS}"
        ));
    }
    let mut rendered = String::new();
    for record in records {
        validate_record_relative_path(&record.relative_path)?;
        sanitize_record_note(record.note.as_deref())?;
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
        if rendered.len() as u64 > MAX_CERTIFICATE_STATE_FILE_BYTES {
            return Err(format!(
                "rotation records exceed {MAX_CERTIFICATE_STATE_FILE_BYTES} bytes"
            ));
        }
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
    if records.len() > MAX_CERTIFICATE_STATE_RECORDS {
        return Err(format!(
            "revocation records exceed the limit of {MAX_CERTIFICATE_STATE_RECORDS}"
        ));
    }
    let mut rendered = String::new();
    for record in records {
        validate_record_relative_path(&record.relative_path)?;
        sanitize_record_note(record.note.as_deref())?;
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
        if rendered.len() as u64 > MAX_CERTIFICATE_STATE_FILE_BYTES {
            return Err(format!(
                "revocation records exceed {MAX_CERTIFICATE_STATE_FILE_BYTES} bytes"
            ));
        }
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
