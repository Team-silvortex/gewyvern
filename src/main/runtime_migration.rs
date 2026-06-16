use gewyvern::runtime_layout::{RuntimeLayout, runtime_layout};
use std::fs;
use std::path::{Path, PathBuf};

const DEFAULT_CONFIG_NAME: &str = "gewyvern.toml";
const LEGACY_CONFIG_NAME: &str = "config.toml";

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct RuntimeMigrationReport {
    pub(crate) created_roots: Vec<PathBuf>,
    pub(crate) copied_config_to: Option<PathBuf>,
    pub(crate) copied_protocol_entries: usize,
    pub(crate) copied_dsl_entries: usize,
}

pub(crate) fn prepare_runtime_layout() -> Result<RuntimeMigrationReport, String> {
    let layout = runtime_layout();
    let mut report = RuntimeMigrationReport::default();
    ensure_standard_roots(&layout, &mut report)?;
    migrate_legacy_config(&layout, &mut report)?;
    report.copied_protocol_entries = migrate_legacy_tree(
        legacy_subdir(&layout, "protocols"),
        layout.data_root.join("protocols"),
    )?;
    report.copied_dsl_entries =
        migrate_legacy_tree(legacy_subdir(&layout, "dsl"), layout.data_root.join("dsl"))?;
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
    fs::copy(&source, &standard).map_err(|err| {
        format!(
            "failed to migrate legacy runtime config '{}' to '{}': {err}",
            source.display(),
            standard.display()
        )
    })?;
    report.copied_config_to = Some(standard);
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

fn migrate_legacy_tree(source: Option<PathBuf>, target: PathBuf) -> Result<usize, String> {
    let Some(source) = source else {
        return Ok(0);
    };
    copy_missing_tree(&source, &target)
}

fn copy_missing_tree(source: &Path, target: &Path) -> Result<usize, String> {
    let metadata = fs::metadata(source).map_err(|err| {
        format!(
            "failed to inspect legacy runtime tree '{}': {err}",
            source.display()
        )
    })?;
    if metadata.is_file() {
        return copy_missing_file(source, target).map(|copied| usize::from(copied));
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
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        let entry_type = entry.file_type().map_err(|err| {
            format!(
                "failed to inspect an entry under legacy runtime tree '{}': {err}",
                source_path.display()
            )
        })?;
        if entry_type.is_dir() {
            copied += copy_missing_tree(&source_path, &target_path)?;
            continue;
        }
        if entry_type.is_file() && copy_missing_file(&source_path, &target_path)? {
            copied += 1;
        }
    }
    Ok(copied)
}

fn copy_missing_file(source: &Path, target: &Path) -> Result<bool, String> {
    if target.exists() {
        return Ok(false);
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "failed to create migrated runtime directory '{}': {err}",
                parent.display()
            )
        })?;
    }
    fs::copy(source, target).map_err(|err| {
        format!(
            "failed to copy legacy runtime file '{}' to '{}': {err}",
            source.display(),
            target.display()
        )
    })?;
    Ok(true)
}
