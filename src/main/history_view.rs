use crate::history_catalog_delta::latest_protocol_catalog_delta;
use crate::render_utils::append_json_string;
use gewyvern::runtime_layout::runtime_layout;
use silvortex_bounded_io::read_bounded_utf8_regular_file;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

const API_VERSION: &str = env!("CARGO_PKG_VERSION");
const HISTORY_RETENTION_LIMIT: usize = 32;
const HISTORY_RETENTION_ENV: &str = "GEWY_HISTORY_RETENTION";
const MAX_HISTORY_INDEX_BYTES: u64 = 4 * 1024 * 1024;
const MAX_HISTORY_ROOT_ENTRIES: usize = 4096;

pub(crate) fn render_history_index(json: bool) -> Result<String, String> {
    let history_root = history_root();
    if json {
        return render_history_index_json(&history_root);
    }
    render_history_index_text(&history_root)
}

fn history_root() -> PathBuf {
    runtime_layout()
        .state_root
        .join("history")
        .join("api")
        .join("v1")
}

fn render_history_index_json(history_root: &Path) -> Result<String, String> {
    let index_path = history_root.join("index.json");
    match fs::symlink_metadata(&index_path) {
        Ok(_) => {
            return read_bounded_utf8_regular_file(&index_path, MAX_HISTORY_INDEX_BYTES).map_err(
                |err| {
                    format!(
                        "failed to securely read history index '{}': {err}",
                        index_path.display()
                    )
                },
            );
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => {
            return Err(format!(
                "failed to inspect history index '{}': {error}",
                index_path.display()
            ));
        }
    }
    Ok(empty_history_index_json(history_root))
}

fn render_history_index_text(history_root: &Path) -> Result<String, String> {
    let entries = history_snapshot_dirs(history_root)?;
    let retention = history_retention_limit();
    let minor_line = current_minor_line();
    let latest = entries.first().map(|(updated_unix_ms, _)| *updated_unix_ms);
    let oldest = entries.last().map(|(updated_unix_ms, _)| *updated_unix_ms);
    let mut text = format!(
        "History Shelf\nroot: {}\nminor line: {}\nretention: {}\nentries: {}\n",
        history_root.display(),
        minor_line,
        retention,
        entries.len()
    );
    match latest {
        Some(value) => text.push_str(&format!("latest: {value}\n")),
        None => text.push_str("latest: none\n"),
    }
    match oldest {
        Some(value) => text.push_str(&format!("oldest: {value}\n")),
        None => text.push_str("oldest: none\n"),
    }
    if entries.is_empty() {
        text.push_str("snapshots:\n- none yet\n");
        return Ok(text);
    }
    if let Some(delta) = latest_protocol_catalog_delta(&entries)? {
        text.push_str(&delta.to_text_block());
    }
    text.push_str("snapshots:\n");
    for (updated_unix_ms, path) in entries {
        text.push_str(&format!(
            "- {} line={} path={} protocol_catalog={}/protocols.json\n",
            updated_unix_ms,
            minor_line,
            path.display(),
            path.display()
        ));
    }
    Ok(text)
}

fn empty_history_index_json(history_root: &Path) -> String {
    let mut json = format!(
        "{{\"schema_version\":2,\"api_version\":\"{}\",\"minor_line\":\"{}\",\"history_retention\":{},\"latest_updated_unix_ms\":null,\"oldest_updated_unix_ms\":null,\"lines\":[{{\"line\":\"{}\",\"status\":\"active\",\"entry_count\":0,\"latest_updated_unix_ms\":null,\"oldest_updated_unix_ms\":null}}],\"entries\":[],\"root\":",
        API_VERSION,
        current_minor_line(),
        history_retention_limit(),
        current_minor_line(),
    );
    append_json_string(&mut json, &history_root.to_string_lossy());
    json.push_str(
        ",\"catalog_artifacts\":[\"protocols.json\",\"protocol-clusters.json\",\"protocol-clusters/<cluster>.json\",\"protocols/<protocol>/summary.json\",\"protocols/<protocol>/entries/<entry>/surface.json\"],\"latest_protocol_catalog_delta\":null}",
    );
    json
}

fn history_snapshot_dirs(history_root: &Path) -> Result<Vec<(u128, PathBuf)>, String> {
    match fs::symlink_metadata(history_root) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(format!(
                "history snapshot root '{}' must be a non-symlink directory",
                history_root.display()
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(format!(
                "failed to inspect history snapshot root '{}': {error}",
                history_root.display()
            ));
        }
    }
    let read_dir = fs::read_dir(history_root).map_err(|err| {
        format!(
            "failed to inspect history snapshot root '{}': {err}",
            history_root.display()
        )
    })?;
    let mut entries = Vec::new();
    for (index, entry) in read_dir.enumerate() {
        if index >= MAX_HISTORY_ROOT_ENTRIES {
            return Err(format!(
                "history snapshot root '{}' exceeds the {} entry limit",
                history_root.display(),
                MAX_HISTORY_ROOT_ENTRIES
            ));
        }
        let entry = entry.map_err(|error| {
            format!(
                "failed to read history snapshot root '{}': {error}",
                history_root.display()
            )
        })?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|error| {
            format!(
                "failed to inspect history entry '{}': {error}",
                path.display()
            )
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            continue;
        }
        if let Some(updated_unix_ms) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<u128>().ok())
        {
            entries.push((updated_unix_ms, path));
        }
    }
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.0));
    Ok(entries)
}

fn history_retention_limit() -> usize {
    std::env::var(HISTORY_RETENTION_ENV)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(HISTORY_RETENTION_LIMIT)
}

fn current_minor_line() -> String {
    let mut parts = API_VERSION.split('.');
    let major = parts.next().unwrap_or("0");
    let minor = parts.next().unwrap_or("0");
    format!("v{major}.{minor}.x")
}
