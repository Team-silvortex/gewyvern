use crate::render_utils::append_json_string;
use gewyvern::protocol_profiles::protocol_summaries;
use gewyvern::runtime_layout::runtime_layout;
use silvortex_bounded_io::atomic_write_bounded_private_file;
use std::fs;
use std::path::{Path, PathBuf};

use super::anomaly_flow_view::api_target_anomaly_flow_json;
use super::certificate_inventory::api_runtime_certificates_json;
use super::certificate_policy::api_runtime_certificate_policy_json;
use super::certificate_state::api_runtime_certificate_state_json;
use super::json::{api_snapshot_meta_json, api_target_list_json, api_target_path_segment};
use super::protocol_catalog::{
    api_protocol_catalog_json_from_summaries, api_protocol_cluster_json_from_summaries,
    api_protocol_clusters_json_from_summaries, api_protocol_summary_json_from_summary,
    api_protocol_surface_from_summary_json,
};
use super::runtime_capability_digest::api_runtime_capability_digest_json;
use super::runtime_cluster_attention::{
    api_runtime_cluster_attention_json, api_runtime_cluster_attention_reasons_json,
    api_runtime_cluster_attention_summary_json,
};
use super::runtime_cluster_overview::api_runtime_cluster_overview_json;
use super::training_manifest::{
    target_training_dataset_manifest_json, training_dataset_manifest_json,
};
use super::{ApiSnapshot, ApiTargetSnapshot};
use crate::history_catalog_delta::{
    latest_protocol_catalog_delta, protocol_catalog_delta_between_paths,
    protocol_catalog_delta_json, protocol_catalog_delta_markdown,
};
const API_VERSION: &str = env!("CARGO_PKG_VERSION");
const HISTORY_RETENTION_LIMIT: usize = 32;
const MAX_HISTORY_RETENTION_LIMIT: usize = 1024;
const MAX_HISTORY_DIRECTORY_ENTRIES: usize = 4096;
const MAX_TARGET_DIRECTORY_ENTRIES: usize = 4096;
const MAX_PERSISTED_SNAPSHOT_FILE_BYTES: u64 = 64 * 1024 * 1024;
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
    write_protocol_catalog_delta_artifacts(&state_root, &latest_root, snapshot.updated_unix_ms)?;
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

fn write_protocol_catalog_delta_artifacts(
    state_root: &Path,
    latest_root: &Path,
    current_updated_unix_ms: u128,
) -> Result<(), String> {
    let history_root = state_root.join("history").join("api").join("v1");
    let entries = history_snapshot_dirs(&history_root)?;
    let (delta_json, delta_markdown) = if let Some((_, current_root)) = entries
        .iter()
        .find(|(updated_unix_ms, _)| *updated_unix_ms == current_updated_unix_ms)
    {
        let delta = if let Some((previous_updated_unix_ms, previous_root)) = entries
            .iter()
            .filter(|(updated_unix_ms, _)| *updated_unix_ms != current_updated_unix_ms)
            .max_by_key(|(updated_unix_ms, _)| *updated_unix_ms)
        {
            Some(protocol_catalog_delta_between_paths(
                current_updated_unix_ms,
                current_root,
                *previous_updated_unix_ms,
                previous_root,
            )?)
        } else {
            None
        };
        let delta_json = protocol_catalog_delta_json(delta.as_ref());
        let delta_markdown = protocol_catalog_delta_markdown(delta.as_ref());
        write_text_file(&current_root.join("protocol-delta.json"), &delta_json)?;
        write_text_file(&current_root.join("protocol-evolution.md"), &delta_markdown)?;
        (delta_json, delta_markdown)
    } else {
        ("null".into(), protocol_catalog_delta_markdown(None))
    };
    write_text_file(&latest_root.join("protocol-delta.json"), &delta_json)?;
    write_text_file(&latest_root.join("protocol-evolution.md"), &delta_markdown)
}

fn history_retention_limit() -> usize {
    std::env::var(HISTORY_RETENTION_ENV)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| (1..=MAX_HISTORY_RETENTION_LIMIT).contains(value))
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
    write_text_file(
        &root.join("runtime-capability-digest.json"),
        &api_runtime_capability_digest_json(snapshot),
    )?;
    write_text_file(
        &root.join("runtime-cluster-overview.json"),
        &api_runtime_cluster_overview_json(snapshot),
    )?;
    write_text_file(
        &root.join("runtime-cluster-attention.json"),
        &api_runtime_cluster_attention_json(snapshot),
    )?;
    write_text_file(
        &root.join("runtime-cluster-attention-reasons.json"),
        &api_runtime_cluster_attention_reasons_json(),
    )?;
    write_text_file(
        &root.join("runtime-cluster-attention-summary.json"),
        &api_runtime_cluster_attention_summary_json(snapshot),
    )?;
    write_text_file(
        &root.join("runtime-certificates.json"),
        &api_runtime_certificates_json(),
    )?;
    write_text_file(
        &root.join("runtime-certificate-policy.json"),
        &api_runtime_certificate_policy_json(),
    )?;
    write_text_file(
        &root.join("runtime-certificate-state.json"),
        &api_runtime_certificate_state_json(),
    )?;
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
    remove_if_exists(&root.join("protocol-delta.json"))?;
    persist_protocol_catalog(root)?;

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

fn persist_protocol_catalog(root: &Path) -> Result<(), String> {
    let summaries = protocol_summaries();
    write_text_file(
        &root.join("protocols.json"),
        &api_protocol_catalog_json_from_summaries(&summaries),
    )?;
    write_text_file(
        &root.join("protocol-clusters.json"),
        &api_protocol_clusters_json_from_summaries(&summaries),
    )?;
    let protocols_root = root.join("protocols");
    fs::create_dir_all(&protocols_root).map_err(|err| {
        format!(
            "failed to create protocol catalog root '{}': {err}",
            protocols_root.display()
        )
    })?;
    for summary in &summaries {
        let protocol_root = protocols_root.join(&summary.protocol);
        fs::create_dir_all(protocol_root.join("entries")).map_err(|err| {
            format!(
                "failed to create protocol catalog directory '{}': {err}",
                protocol_root.display()
            )
        })?;
        write_text_file(
            &protocol_root.join("summary.json"),
            &api_protocol_summary_json_from_summary(summary),
        )?;
        for entry in &summary.entries {
            if let Some(body) = api_protocol_surface_from_summary_json(summary, &entry.mode) {
                write_text_file(
                    &protocol_root
                        .join("entries")
                        .join(&entry.mode)
                        .join("surface.json"),
                    &body,
                )?;
            }
        }
    }
    let clusters_root = root.join("protocol-clusters");
    fs::create_dir_all(&clusters_root).map_err(|err| {
        format!(
            "failed to create protocol cluster catalog root '{}': {err}",
            clusters_root.display()
        )
    })?;
    let mut written_clusters = std::collections::BTreeSet::new();
    for summary in &summaries {
        let Some(hint) = summary.cluster_hint.as_ref() else {
            continue;
        };
        if !written_clusters.insert(hint.key.clone()) {
            continue;
        }
        if let Some(body) = api_protocol_cluster_json_from_summaries(&summaries, &hint.key) {
            write_text_file(&clusters_root.join(format!("{}.json", hint.key)), &body)?;
        }
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
    write_optional_file(
        target_root.join("anomaly-flow.json"),
        api_target_anomaly_flow_json(name, target).as_deref(),
    )?;
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
    let retention = history_retention_limit();
    let minor_line = current_minor_line();
    let latest_updated_unix_ms = entries.first().map(|(updated_unix_ms, _)| *updated_unix_ms);
    let oldest_updated_unix_ms = entries.last().map(|(updated_unix_ms, _)| *updated_unix_ms);

    let mut json = String::from("{\"schema_version\":2,\"api_version\":");
    append_json_string(&mut json, API_VERSION);
    json.push_str(",\"minor_line\":");
    append_json_string(&mut json, &minor_line);
    json.push_str(",\"history_retention\":");
    json.push_str(&retention.to_string());
    json.push_str(",\"latest_updated_unix_ms\":");
    match latest_updated_unix_ms {
        Some(value) => json.push_str(&value.to_string()),
        None => json.push_str("null"),
    }
    json.push_str(",\"oldest_updated_unix_ms\":");
    match oldest_updated_unix_ms {
        Some(value) => json.push_str(&value.to_string()),
        None => json.push_str("null"),
    }
    json.push_str(",\"catalog_artifacts\":[\"protocols.json\",\"protocol-clusters.json\",\"protocol-clusters/<cluster>.json\",\"protocols/<protocol>/summary.json\",\"protocols/<protocol>/entries/<entry>/surface.json\"]");
    json.push_str(",\"latest_protocol_catalog_delta\":");
    let delta = latest_protocol_catalog_delta(&entries)?;
    json.push_str(&protocol_catalog_delta_json(delta.as_ref()));
    json.push_str(",\"latest_protocol_catalog_delta_path\":");
    match latest_updated_unix_ms {
        Some(value) => append_json_string(
            &mut json,
            &format!("history/api/v1/{value}/protocol-delta.json"),
        ),
        None => json.push_str("null"),
    }
    json.push_str(",\"lines\":[{\"line\":");
    append_json_string(&mut json, &minor_line);
    json.push_str(",\"status\":\"active\",\"entry_count\":");
    json.push_str(&entries.len().to_string());
    json.push_str(",\"latest_updated_unix_ms\":");
    match latest_updated_unix_ms {
        Some(value) => json.push_str(&value.to_string()),
        None => json.push_str("null"),
    }
    json.push_str(",\"oldest_updated_unix_ms\":");
    match oldest_updated_unix_ms {
        Some(value) => json.push_str(&value.to_string()),
        None => json.push_str("null"),
    }
    json.push_str("}],\"entries\":[");
    for (index, (updated_unix_ms, path)) in entries.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        let relative_path = format!("history/api/v1/{updated_unix_ms}");
        json.push_str("{\"updated_unix_ms\":");
        json.push_str(&updated_unix_ms.to_string());
        json.push_str(",\"line\":");
        append_json_string(&mut json, &minor_line);
        json.push_str(",\"api_version\":");
        append_json_string(&mut json, API_VERSION);
        json.push_str(",\"path\":");
        append_json_string(&mut json, &relative_path);
        json.push_str(",\"meta_path\":");
        append_json_string(&mut json, &format!("{relative_path}/meta.json"));
        json.push_str(",\"targets_path\":");
        append_json_string(&mut json, &format!("{relative_path}/targets.json"));
        json.push_str(",\"protocol_catalog_path\":");
        append_json_string(&mut json, &format!("{relative_path}/protocols.json"));
        json.push_str(",\"protocol_root_path\":");
        append_json_string(&mut json, &format!("{relative_path}/protocols"));
        json.push_str(",\"protocol_delta_path\":");
        append_json_string(&mut json, &format!("{relative_path}/protocol-delta.json"));
        json.push_str(",\"protocol_evolution_path\":");
        append_json_string(&mut json, &format!("{relative_path}/protocol-evolution.md"));
        json.push_str(",\"exists\":");
        json.push_str(if path.exists() { "true" } else { "false" });
        json.push('}');
    }
    json.push_str("]}");
    write_text_file(&history_root.join("index.json"), &json)
}

fn current_minor_line() -> String {
    let mut parts = API_VERSION.split('.');
    let major = parts.next().unwrap_or("0");
    let minor = parts.next().unwrap_or("0");
    format!("v{major}.{minor}.x")
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
    let directory = fs::read_dir(history_root).map_err(|err| {
        format!(
            "failed to inspect history snapshot root '{}': {err}",
            history_root.display()
        )
    })?;
    let mut entries = Vec::new();
    for (index, entry) in directory.enumerate() {
        if index >= MAX_HISTORY_DIRECTORY_ENTRIES {
            return Err(format!(
                "history snapshot root '{}' exceeds the entry limit of {MAX_HISTORY_DIRECTORY_ENTRIES}",
                history_root.display()
            ));
        }
        let entry = entry.map_err(|err| {
            format!(
                "failed to read an entry under history snapshot root '{}': {err}",
                history_root.display()
            )
        })?;
        let entry_type = entry.file_type().map_err(|err| {
            format!(
                "failed to inspect history snapshot entry '{}': {err}",
                entry.path().display()
            )
        })?;
        if !entry_type.is_dir() {
            continue;
        }
        let file_name = entry.file_name();
        let Some(updated_unix_ms) = file_name
            .to_str()
            .and_then(|name| name.parse::<u128>().ok())
        else {
            continue;
        };
        entries.push((updated_unix_ms, entry.path()));
    }
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.0));
    Ok(entries)
}

fn remove_stale_target_dirs(targets_root: &Path, snapshot: &ApiSnapshot) -> Result<(), String> {
    let mut active = std::collections::BTreeSet::new();
    for name in &snapshot.target_names {
        active.insert(api_target_path_segment(name));
    }
    for (index, entry) in fs::read_dir(targets_root)
        .map_err(|err| {
            format!(
                "failed to inspect latest snapshot target directories '{}': {err}",
                targets_root.display()
            )
        })?
        .enumerate()
    {
        if index >= MAX_TARGET_DIRECTORY_ENTRIES {
            return Err(format!(
                "latest snapshot target root '{}' exceeds the entry limit of {MAX_TARGET_DIRECTORY_ENTRIES}",
                targets_root.display()
            ));
        }
        let entry = entry.map_err(|err| {
            format!(
                "failed to read an entry under latest snapshot target directories '{}': {err}",
                targets_root.display()
            )
        })?;
        let path = entry.path();
        let entry_type = entry.file_type().map_err(|err| {
            format!(
                "failed to inspect latest snapshot target entry '{}': {err}",
                path.display()
            )
        })?;
        if !entry_type.is_dir() {
            continue;
        }
        let file_name = entry.file_name();
        let Some(name) = file_name.to_str() else {
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
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "failed to create latest snapshot directory '{}': {err}",
                parent.display()
            )
        })?;
    }
    atomic_write_bounded_private_file(path, content.as_bytes(), MAX_PERSISTED_SNAPSHOT_FILE_BYTES)
        .map_err(|err| {
            format!(
                "failed to persist latest snapshot file '{}': {err}",
                path.display()
            )
        })
}

fn remove_if_exists(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {
            return Err(format!(
                "refusing to remove snapshot directory as a file: '{}'",
                path.display()
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(format!(
                "failed to inspect stale latest snapshot file '{}': {error}",
                path.display()
            ));
        }
    }
    fs::remove_file(path).map_err(|err| {
        format!(
            "failed to remove stale latest snapshot file '{}': {err}",
            path.display()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::{history_snapshot_dirs, remove_if_exists, write_text_file};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "gewyvern-persistence-{label}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock must follow the epoch")
                .as_nanos()
        ))
    }

    #[cfg(unix)]
    #[test]
    fn history_snapshot_discovery_does_not_follow_symlinks() {
        use std::os::unix::fs::symlink;

        let root = temp_root("history-symlink");
        let history = root.join("history");
        let outside = root.join("outside");
        fs::create_dir_all(history.join("100")).unwrap();
        fs::create_dir_all(&outside).unwrap();
        symlink(&outside, history.join("200")).unwrap();

        let entries = history_snapshot_dirs(&history).unwrap();

        assert_eq!(entries, vec![(100, history.join("100"))]);
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn snapshot_writes_replace_destination_symlinks_without_touching_targets() {
        use std::os::unix::fs::symlink;

        let root = temp_root("write-symlink");
        fs::create_dir_all(&root).unwrap();
        let outside = root.join("outside.json");
        let snapshot = root.join("snapshot.json");
        fs::write(&outside, "outside").unwrap();
        symlink(&outside, &snapshot).unwrap();

        write_text_file(&snapshot, "snapshot").unwrap();

        assert_eq!(fs::read_to_string(&outside).unwrap(), "outside");
        assert_eq!(fs::read_to_string(&snapshot).unwrap(), "snapshot");
        assert!(
            !fs::symlink_metadata(&snapshot)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn stale_file_removal_handles_broken_symlinks() {
        use std::os::unix::fs::symlink;

        let root = temp_root("remove-broken-symlink");
        fs::create_dir_all(&root).unwrap();
        let link = root.join("stale.json");
        symlink(root.join("missing.json"), &link).unwrap();

        remove_if_exists(&link).unwrap();

        assert!(fs::symlink_metadata(&link).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
