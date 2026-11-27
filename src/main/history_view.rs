use crate::history_catalog_delta::latest_protocol_catalog_delta;
use crate::render_utils::append_json_string;
use gewyvern::runtime_layout::runtime_layout;
use silvortex_bounded_io::read_bounded_utf8_regular_file;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

const API_VERSION: &str = env!("CARGO_PKG_VERSION");
const HISTORY_RETENTION_LIMIT: usize = 32;
const MAX_HISTORY_RETENTION_LIMIT: usize = 1024;
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
    let configured = history_retention_limit();
    let (entries, retention) = match read_committed_index(history_root)? {
        Some(index) => (index.entries, index.retention.unwrap_or(configured)),
        None => (history_snapshot_dirs(history_root)?, configured),
    };
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
    if configured != retention {
        text.push_str(&format!("local configured retention: {configured}\n"));
    }
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

pub(crate) struct CommittedIndex {
    pub(crate) entries: Vec<(u128, PathBuf)>,
    retention: Option<usize>,
}

impl CommittedIndex {
    pub(crate) fn backfill_retention(&self) -> Option<usize> {
        self.retention
            .filter(|cap| (self.entries.len().max(1)..=MAX_HISTORY_RETENTION_LIMIT).contains(cap))
    }
}

/// Do not trust path strings from disk or include an unindexed numeric directory.
pub(crate) fn read_committed_index(root: &Path) -> Result<Option<CommittedIndex>, String> {
    if fs::symlink_metadata(root).is_ok_and(|m| !m.is_dir()) {
        return Err(format!(
            "history root must be a real directory: {}",
            root.display()
        ));
    }
    #[derive(serde::Deserialize)]
    struct Index {
        entries: Vec<Row>,
        history_retention: Option<usize>,
    }
    #[derive(serde::Deserialize)]
    struct Row {
        updated_unix_ms: u128,
    }
    let path = root.join("index.json");
    if let Err(err) = fs::symlink_metadata(&path) {
        if err.kind() == ErrorKind::NotFound {
            return Ok(None);
        }
        return Err(format!("inspect history index: {err}"));
    }
    let data = read_bounded_utf8_regular_file(&path, MAX_HISTORY_INDEX_BYTES)
        .map_err(|err| format!("read history index: {err}"))?;
    let index: Index =
        serde_json::from_str(&data).map_err(|err| format!("decode history index: {err}"))?;
    if index.entries.len() > MAX_HISTORY_ROOT_ENTRIES
        || index
            .entries
            .windows(2)
            .any(|p| p[0].updated_unix_ms <= p[1].updated_unix_ms)
    {
        return Err("invalid history index size or timestamp order".into());
    }
    let mut entries = Vec::with_capacity(index.entries.len());
    for row in index.entries {
        let path = root.join(row.updated_unix_ms.to_string());
        if !fs::symlink_metadata(&path).is_ok_and(|m| m.is_dir()) {
            return Err(format!(
                "indexed history entry is not a real directory: {}",
                path.display()
            ));
        }
        entries.push((row.updated_unix_ms, path));
    }
    Ok(Some(CommittedIndex {
        entries,
        retention: index.history_retention,
    }))
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

pub(crate) fn history_retention_limit() -> usize {
    std::env::var(HISTORY_RETENTION_ENV)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| (1..=MAX_HISTORY_RETENTION_LIMIT).contains(value))
        .unwrap_or(HISTORY_RETENTION_LIMIT)
}

fn current_minor_line() -> String {
    let mut parts = API_VERSION.split('.');
    let major = parts.next().unwrap_or("0");
    let minor = parts.next().unwrap_or("0");
    format!("v{major}.{minor}.x")
}
