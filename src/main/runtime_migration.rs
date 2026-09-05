use gewyvern::runtime_layout::{RuntimeLayout, runtime_layout};
use silvortex_bounded_io::open_bounded_regular_file_allow_empty;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const DEFAULT_CONFIG_NAME: &str = "gewyvern.toml";
const LEGACY_CONFIG_NAME: &str = "config.toml";
pub(crate) const MAX_LEGACY_MIGRATION_DEPTH: usize = 32;
pub(crate) const MAX_LEGACY_MIGRATION_FILE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_LEGACY_MIGRATION_ENTRIES: usize = 10_000;
const MAX_LEGACY_MIGRATION_TOTAL_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Debug, Default)]
struct MigrationBudget {
    entries: usize,
    copied_bytes: u64,
}

impl MigrationBudget {
    fn observe_entry(&mut self) -> Result<(), String> {
        self.entries = self
            .entries
            .checked_add(1)
            .ok_or_else(|| "legacy runtime migration entry count overflowed".to_string())?;
        if self.entries > MAX_LEGACY_MIGRATION_ENTRIES {
            return Err(format!(
                "legacy runtime migration exceeds the entry limit of {MAX_LEGACY_MIGRATION_ENTRIES}"
            ));
        }
        Ok(())
    }

    fn remaining_bytes(&self) -> Result<u64, String> {
        let remaining = MAX_LEGACY_MIGRATION_TOTAL_BYTES.saturating_sub(self.copied_bytes);
        if remaining == 0 {
            return Err(format!(
                "legacy runtime migration exceeds the byte limit of {MAX_LEGACY_MIGRATION_TOTAL_BYTES}"
            ));
        }
        Ok(remaining)
    }

    fn record_copied_bytes(&mut self, copied: u64) -> Result<(), String> {
        self.copied_bytes = self
            .copied_bytes
            .checked_add(copied)
            .ok_or_else(|| "legacy runtime migration byte count overflowed".to_string())?;
        if self.copied_bytes > MAX_LEGACY_MIGRATION_TOTAL_BYTES {
            return Err(format!(
                "legacy runtime migration exceeds the byte limit of {MAX_LEGACY_MIGRATION_TOTAL_BYTES}"
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct RuntimeMigrationReport {
    pub(crate) created_roots: Vec<PathBuf>,
    pub(crate) copied_config_to: Option<PathBuf>,
    pub(crate) copied_protocol_entries: usize,
    pub(crate) copied_dsl_entries: usize,
    pub(crate) copied_certificate_entries: usize,
    pub(crate) copied_certificate_state_entries: usize,
}

pub(crate) fn prepare_runtime_layout() -> Result<RuntimeMigrationReport, String> {
    let layout = runtime_layout();
    let mut report = RuntimeMigrationReport::default();
    let mut budget = MigrationBudget::default();
    ensure_standard_roots(&layout, &mut report)?;
    migrate_legacy_config(&layout, &mut report, &mut budget)?;
    report.copied_protocol_entries = migrate_legacy_tree(
        legacy_subdir(&layout, "protocols"),
        layout.data_root.join("protocols"),
        &mut budget,
    )?;
    report.copied_dsl_entries = migrate_legacy_tree(
        legacy_subdir(&layout, "dsl"),
        layout.data_root.join("dsl"),
        &mut budget,
    )?;
    report.copied_certificate_entries = migrate_legacy_certificate_tree(
        legacy_certificate_root(&layout),
        layout.certificate_root.clone(),
        &mut budget,
    )?;
    report.copied_certificate_state_entries = migrate_legacy_tree(
        legacy_certificate_state_root(&layout),
        layout.certificate_state_root.clone(),
        &mut budget,
    )?;
    Ok(report)
}

fn ensure_standard_roots(
    layout: &RuntimeLayout,
    report: &mut RuntimeMigrationReport,
) -> Result<(), String> {
    for root in [
        layout.config_root.clone(),
        layout.data_root.clone(),
        layout.state_root.clone(),
        layout.cache_root.clone(),
        layout.certificate_root.clone(),
        layout.trust_root.clone(),
        layout.authority_root.clone(),
        layout.identity_root.clone(),
        layout.certificate_state_root.clone(),
    ] {
        if !root.exists() {
            fs::create_dir_all(&root).map_err(|err| {
                format!("failed to create runtime root '{}': {err}", root.display())
            })?;
            report.created_roots.push(root);
        }
    }
    Ok(())
}

fn migrate_legacy_config(
    layout: &RuntimeLayout,
    report: &mut RuntimeMigrationReport,
    budget: &mut MigrationBudget,
) -> Result<(), String> {
    let standard = layout.config_root.join(DEFAULT_CONFIG_NAME);
    if standard.exists() {
        return Ok(());
    }
    let Some(source) = legacy_config_source(layout) else {
        return Ok(());
    };
    if let Some(parent) = standard.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "failed to create runtime config directory '{}': {err}",
                parent.display()
            )
        })?;
    }
    budget.observe_entry()?;
    if copy_missing_file(&source, &standard, budget)? {
        report.copied_config_to = Some(standard);
    }
    Ok(())
}

fn legacy_config_source(layout: &RuntimeLayout) -> Option<PathBuf> {
    let legacy_root = layout.legacy_root.as_ref()?;
    let primary = legacy_root.join(LEGACY_CONFIG_NAME);
    if primary.exists() {
        return Some(primary);
    }
    let named = legacy_root.join(DEFAULT_CONFIG_NAME);
    if named.exists() {
        return Some(named);
    }
    None
}

fn legacy_subdir(layout: &RuntimeLayout, name: &str) -> Option<PathBuf> {
    let root = layout.legacy_root.as_ref()?;
    let candidate = root.join(name);
    candidate.exists().then_some(candidate)
}

fn legacy_certificate_root(layout: &RuntimeLayout) -> Option<PathBuf> {
    legacy_subdir(layout, "certificates")
}

fn legacy_certificate_state_root(layout: &RuntimeLayout) -> Option<PathBuf> {
    let root = layout.legacy_root.as_ref()?;
    let candidate = root.join("state").join("certificates");
    candidate.exists().then_some(candidate)
}

fn migrate_legacy_tree(
    source: Option<PathBuf>,
    target: PathBuf,
    budget: &mut MigrationBudget,
) -> Result<usize, String> {
    let Some(source) = source else {
        return Ok(0);
    };
    copy_missing_tree(&source, &target, budget, 0)
}

fn migrate_legacy_certificate_tree(
    source: Option<PathBuf>,
    target: PathBuf,
    budget: &mut MigrationBudget,
) -> Result<usize, String> {
    let Some(source) = source else {
        return Ok(0);
    };
    copy_missing_tree(&source, &target, budget, 0)
}

fn copy_missing_tree(
    source: &Path,
    target: &Path,
    budget: &mut MigrationBudget,
    depth: usize,
) -> Result<usize, String> {
    if depth > MAX_LEGACY_MIGRATION_DEPTH {
        return Err(format!(
            "legacy runtime migration exceeds the directory depth limit of {MAX_LEGACY_MIGRATION_DEPTH} at '{}'",
            source.display()
        ));
    }
    let metadata = fs::symlink_metadata(source).map_err(|err| {
        format!(
            "failed to inspect legacy runtime tree '{}': {err}",
            source.display()
        )
    })?;
    if metadata.file_type().is_symlink() {
        return Ok(0);
    }

    if metadata.is_file() {
        return copy_missing_file(source, target, budget).map(usize::from);
    }
    if !metadata.is_dir() {
        return Ok(0);
    }
    fs::create_dir_all(target).map_err(|err| {
        format!(
            "failed to create migrated runtime tree '{}': {err}",
            target.display()
        )
    })?;
    let mut copied = 0usize;
    for entry in fs::read_dir(source).map_err(|err| {
        format!(
            "failed to read legacy runtime tree '{}': {err}",
            source.display()
        )
    })? {
        let entry = entry.map_err(|err| {
            format!(
                "failed to read an entry under legacy runtime tree '{}': {err}",
                source.display()
            )
        })?;
        budget.observe_entry()?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        let entry_type = entry.file_type().map_err(|err| {
            format!(
                "failed to inspect an entry under legacy runtime tree '{}': {err}",
                source_path.display()
            )
        })?;
        if entry_type.is_dir() {
            copied += copy_missing_tree(&source_path, &target_path, budget, depth + 1)?;
            continue;
        }
        if entry_type.is_file() && copy_missing_file(&source_path, &target_path, budget)? {
            copied += 1;
        }
    }
    Ok(copied)
}

fn copy_missing_file(
    source: &Path,
    target: &Path,
    budget: &mut MigrationBudget,
) -> Result<bool, String> {
    match fs::symlink_metadata(target) {
        Ok(_) => return Ok(false),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(format!(
                "failed to inspect migrated runtime file '{}': {error}",
                target.display()
            ));
        }
    }
    let max_bytes = MAX_LEGACY_MIGRATION_FILE_BYTES.min(budget.remaining_bytes()?);
    let mut input = open_bounded_regular_file_allow_empty(source, max_bytes).map_err(|err| {
        format!(
            "failed to securely open legacy runtime file '{}': {err}",
            source.display()
        )
    })?;
    let source_permissions = input
        .metadata()
        .map_err(|err| {
            format!(
                "failed to inspect opened legacy runtime file '{}': {err}",
                source.display()
            )
        })?
        .permissions();
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "failed to create migrated runtime directory '{}': {err}",
                parent.display()
            )
        })?;
    }
    let mut output = match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(target)
    {
        Ok(output) => output,
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => return Ok(false),
        Err(error) => {
            return Err(format!(
                "failed to create migrated runtime file '{}': {error}",
                target.display()
            ));
        }
    };
    let result = (|| {
        output.set_permissions(source_permissions).map_err(|err| {
            format!(
                "failed to preserve permissions on migrated runtime file '{}': {err}",
                target.display()
            )
        })?;
        let copied = io::copy(&mut input, &mut output).map_err(|err| {
            format!(
                "failed to copy legacy runtime file '{}' to '{}': {err}",
                source.display(),
                target.display()
            )
        })?;
        output.sync_all().map_err(|err| {
            format!(
                "failed to sync migrated runtime file '{}': {err}",
                target.display()
            )
        })?;
        budget.record_copied_bytes(copied)
    })();
    if result.is_err() {
        drop(output);
        let _ = fs::remove_file(target);
        return result.map(|()| true);
    }
    Ok(true)
}
