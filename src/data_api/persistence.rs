use crate::render_utils::append_json_string;
use gewyvern::runtime_layout::runtime_layout;
use std::fs;
use std::path::{Path, PathBuf};

use super::json::{api_snapshot_meta_json, api_target_list_json, api_target_path_segment};
use super::training_manifest::{
    target_training_dataset_manifest_json, training_dataset_manifest_json,
};
use super::{ApiSnapshot, ApiTargetSnapshot};

const HISTORY_RETENTION_LIMIT: usize = 32;
const HISTORY_RETENTION_ENV: &str = "GEWY_HISTORY_RETENTION";

pub(super) fn persist_latest_snapshot(snapshot: &ApiSnapshot) -> Result<(), String> {
    let state_root = runtime_layout().state_root;
    let latest_root = state_root
        .join("latest")
        .join("api")
        .join("v1")
        .join("latest");
    persist_snapshot_tree(&latest_root, snapshot, true)?;
    persist_history_snapshot(&state_root, snapshot)?;
    Ok(())
}

fn persist_history_snapshot(state_root: &Path, snapshot: &ApiSnapshot) -> Result<(), String> {
    if snapshot.updated_unix_ms == 0 || snapshot.kind.is_empty() {
        return Ok(());
    }
    let history_root = state_root.join("history").join("api").join("v1");
    let snapshot_root = history_root.join(snapshot.updated_unix_ms.to_string());
    persist_snapshot_tree(&snapshot_root, snapshot, false)?;
    prune_history_snapshots(&history_root, history_retention_limit())?;
    write_history_index(&history_root)
}

fn history_retention_limit() -> usize {
    std::env::var(HISTORY_RETENTION_ENV)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(HISTORY_RETENTION_LIMIT)
}

fn persist_snapshot_tree(
    root: &Path,
    snapshot: &ApiSnapshot,
    remove_stale_targets: bool,
) -> Result<(), String> {
    fs::create_dir_all(root).map_err(|err| {
        format!(
            "failed to create snapshot state root '{}': {err}",
            root.display()
        )
    })?;
    write_text_file(&root.join("meta.json"), &api_snapshot_meta_json(snapshot))?;
    write_text_file(&root.join("targets.json"), &api_target_list_json(snapshot))?;
    write_optional_file(root.join("summary.txt"), snapshot.summary_text.as_deref())?;
    write_optional_file(root.join("summary.json"), snapshot.summary_json.as_deref())?;
    write_optional_file(
        root.join("findings.json"),
        snapshot.findings_json.as_deref(),
    )?;
    write_optional_file(
        root.join("analysis.json"),
        snapshot.analysis_json.as_deref(),
    )?;
    write_optional_file(
        root.join("training-example.json"),
        snapshot.training_example_json.as_deref(),
    )?;
    let training_dataset_manifest = snapshot
        .training_example_json
        .as_ref()
        .map(|_| training_dataset_manifest_json(snapshot));
    write_optional_file(
        root.join("training-dataset.json"),
        training_dataset_manifest.as_deref(),
    )?;
    write_optional_file(root.join("export.json"), snapshot.export_json.as_deref())?;
    write_optional_file(root.join("report.json"), snapshot.report_json.as_deref())?;
    write_optional_file(root.join("report.html"), snapshot.report_html.as_deref())?;

    let targets_root = root.join("targets");
    fs::create_dir_all(&targets_root).map_err(|err| {
        format!(
            "failed to create snapshot target root '{}': {err}",
            targets_root.display()
        )
    })?;
    if remove_stale_targets {
        remove_stale_target_dirs(&targets_root, snapshot)?;
    }
    for name in &snapshot.target_names {
        let Some(target) = snapshot.target_snapshots.get(name) else {
            continue;
        };
        persist_target_snapshot(&targets_root, name, target)?;
    }
    Ok(())
}

fn persist_target_snapshot(
    targets_root: &Path,
    name: &str,
    target: &ApiTargetSnapshot,
) -> Result<(), String> {
    let target_root = targets_root.join(api_target_path_segment(name));
    fs::create_dir_all(&target_root).map_err(|err| {
        format!(
            "failed to create latest snapshot target directory '{}': {err}",
            target_root.display()
        )
    })?;
    write_text_file(&target_root.join("summary.txt"), &target.summary_text)?;
    write_text_file(&target_root.join("summary.json"), &target.summary_json)?;
    write_text_file(&target_root.join("findings.json"), &target.findings_json)?;
    write_text_file(&target_root.join("analysis.json"), &target.analysis_json)?;
    write_text_file(
        &target_root.join("training-example.json"),
        &target.training_example_json,
    )?;
    write_text_file(
        &target_root.join("training-dataset.json"),
        &target_training_dataset_manifest_json(name, target),
    )?;
    write_text_file(&target_root.join("export.json"), &target.export_json)?;
    write_text_file(&target_root.join("report.json"), &target.report_json)?;
    write_text_file(&target_root.join("report.html"), &target.report_html)?;
    write_optional_file(
        target_root.join("protocol-surface.json"),
        target.protocol_surface_json.as_deref(),
    )?;
    Ok(())
}

fn write_history_index(history_root: &Path) -> Result<(), String> {
    let entries = history_snapshot_dirs(history_root)?;

    let mut json = String::from("{\"schema_version\":1,\"entries\":[");
    for (index, (updated_unix_ms, path)) in entries.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        let relative_path = format!("history/api/v1/{updated_unix_ms}");
        json.push_str("{\"updated_unix_ms\":");
        json.push_str(&updated_unix_ms.to_string());
        json.push_str(",\"path\":");
        append_json_string(&mut json, &relative_path);
        json.push_str(",\"meta_path\":");
        append_json_string(&mut json, &format!("{relative_path}/meta.json"));
        json.push_str(",\"targets_path\":");
        append_json_string(&mut json, &format!("{relative_path}/targets.json"));
        json.push_str(",\"exists\":");
        json.push_str(if path.exists() { "true" } else { "false" });
        json.push('}');
    }
    json.push_str("]}");
    write_text_file(&history_root.join("index.json"), &json)
}

fn prune_history_snapshots(history_root: &Path, keep: usize) -> Result<(), String> {
    let entries = history_snapshot_dirs(history_root)?;
    for (_, path) in entries.into_iter().skip(keep) {
        fs::remove_dir_all(&path).map_err(|err| {
            format!(
                "failed to prune stale history snapshot directory '{}': {err}",
                path.display()
            )
        })?;
    }
    Ok(())
}

fn history_snapshot_dirs(history_root: &Path) -> Result<Vec<(u128, PathBuf)>, String> {
    let mut entries = fs::read_dir(history_root)
        .map_err(|err| {
            format!(
                "failed to inspect history snapshot root '{}': {err}",
                history_root.display()
            )
        })?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| {
            entry
                .file_name()
                .to_str()
                .and_then(|name| name.parse::<u128>().ok())
                .map(|updated_unix_ms| (updated_unix_ms, entry.path()))
        })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| right.0.cmp(&left.0));
    Ok(entries)
}

fn remove_stale_target_dirs(targets_root: &Path, snapshot: &ApiSnapshot) -> Result<(), String> {
    let mut active = std::collections::BTreeSet::new();
    for name in &snapshot.target_names {
        active.insert(api_target_path_segment(name));
    }
    for entry in fs::read_dir(targets_root).map_err(|err| {
        format!(
            "failed to inspect latest snapshot target directories '{}': {err}",
            targets_root.display()
        )
    })? {
        let entry = entry.map_err(|err| {
            format!(
                "failed to read an entry under latest snapshot target directories '{}': {err}",
                targets_root.display()
            )
        })?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if !active.contains(name) {
            fs::remove_dir_all(&path).map_err(|err| {
                format!(
                    "failed to remove stale latest snapshot target directory '{}': {err}",
                    path.display()
                )
            })?;
        }
    }
    Ok(())
}

fn write_optional_file(path: PathBuf, content: Option<&str>) -> Result<(), String> {
    match content {
        Some(value) => write_text_file(&path, value),
        None => remove_if_exists(&path),
    }
}

fn write_text_file(path: &Path, content: &str) -> Result<(), String> {
    let temp_path = temp_path(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "failed to create latest snapshot directory '{}': {err}",
                parent.display()
            )
        })?;
    }
    fs::write(&temp_path, content).map_err(|err| {
        format!(
            "failed to write latest snapshot file '{}': {err}",
            temp_path.display()
        )
    })?;
    fs::rename(&temp_path, path).map_err(|err| {
        format!(
            "failed to finalize latest snapshot file '{}' as '{}': {err}",
            temp_path.display(),
            path.display()
        )
    })?;
    Ok(())
}

fn remove_if_exists(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    fs::remove_file(path).map_err(|err| {
        format!(
            "failed to remove stale latest snapshot file '{}': {err}",
            path.display()
        )
    })
}

fn temp_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("snapshot");
    path.with_file_name(format!("{file_name}.tmp"))
}
