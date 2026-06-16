use gewyvern::runtime_layout::runtime_layout;
use std::fs;
use std::path::{Path, PathBuf};

const API_VERSION: &str = env!("CARGO_PKG_VERSION");
const HISTORY_RETENTION_LIMIT: usize = 32;
const HISTORY_RETENTION_ENV: &str = "GEWY_HISTORY_RETENTION";

pub(crate) fn render_history_index(json: bool) -> Result<String, String> {
    let history_root = history_root();
    if json {
        return render_history_index_json(&history_root);
    }
    render_history_index_text(&history_root)
}

fn history_root() -> PathBuf {
    runtime_layout().state_root.join("history").join("api").join("v1")
}

fn render_history_index_json(history_root: &Path) -> Result<String, String> {
    let index_path = history_root.join("index.json");
    if index_path.exists() {
        return fs::read_to_string(&index_path)
            .map_err(|err| format!("failed to read history index '{}': {err}", index_path.display()));
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
    text.push_str("snapshots:\n");
    for (updated_unix_ms, path) in entries {
        text.push_str(&format!(
            "- {} line={} path={}\n",
            updated_unix_ms,
            minor_line,
            path.display()
        ));
    }
    Ok(text)
}

fn empty_history_index_json(history_root: &Path) -> String {
    format!(
        "{{\"schema_version\":2,\"api_version\":\"{}\",\"minor_line\":\"{}\",\"history_retention\":{},\"latest_updated_unix_ms\":null,\"oldest_updated_unix_ms\":null,\"lines\":[{{\"line\":\"{}\",\"status\":\"active\",\"entry_count\":0,\"latest_updated_unix_ms\":null,\"oldest_updated_unix_ms\":null}}],\"entries\":[],\"root\":\"{}\"}}",
        API_VERSION,
        current_minor_line(),
        history_retention_limit(),
        current_minor_line(),
        history_root.display()
    )
}

fn history_snapshot_dirs(history_root: &Path) -> Result<Vec<(u128, PathBuf)>, String> {
    if !history_root.exists() {
        return Ok(Vec::new());
    }
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
