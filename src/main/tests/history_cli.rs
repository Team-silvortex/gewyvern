use super::*;
use crate::history_view::render_history_index;
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

struct EnvGuard {
    key: &'static str,
    previous: Option<String>,
}

impl EnvGuard {
    fn set(key: &'static str, value: impl Into<String>) -> Self {
        let previous = std::env::var(key).ok();
        unsafe {
            std::env::set_var(key, value.into());
        }
        Self { key, previous }
    }

    fn remove(key: &'static str) -> Self {
        let previous = std::env::var(key).ok();
        unsafe {
            std::env::remove_var(key);
        }
        Self { key, previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        match &self.previous {
            Some(value) => unsafe {
                std::env::set_var(self.key, value);
            },
            None => unsafe {
                std::env::remove_var(self.key);
            },
        }
    }
}

fn temp_dir(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "gewyvern-history-cli-{label}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

#[test]
fn cli_accepts_list_history_mode() {
    let cli = Cli::from_args(["--list-history".to_string(), "--json".to_string()]).unwrap();
    assert!(cli.list_history);
    assert!(cli.json);
}

#[test]
fn list_history_json_renders_empty_index_when_history_is_missing() {
    let _lock = env_lock().lock().unwrap();
    let root = temp_dir("empty-json");
    let _state = EnvGuard::set("GEWY_STATE_HOME", root.to_string_lossy());
    let _retention = EnvGuard::set("GEWY_HISTORY_RETENTION", "7");

    let rendered = render_history_index(true).unwrap();

    assert!(rendered.contains("\"schema_version\":2"));
    assert!(rendered.contains("\"minor_line\":\"v0.15.x\""));
    assert!(rendered.contains("\"history_retention\":7"));
    assert!(rendered.contains("\"entries\":[]"));
}

#[test]
fn list_history_text_reports_existing_snapshots() {
    let _lock = env_lock().lock().unwrap();
    let root = temp_dir("text");
    let history_root = root.join("history").join("api").join("v1");
    let _state = EnvGuard::set("GEWY_STATE_HOME", root.to_string_lossy());
    let _retention = EnvGuard::remove("GEWY_HISTORY_RETENTION");
    fs::create_dir_all(history_root.join("1001")).unwrap();
    fs::create_dir_all(history_root.join("1003")).unwrap();

    let rendered = render_history_index(false).unwrap();

    assert!(rendered.contains("History Shelf"));
    assert!(rendered.contains("minor line: v0.15.x"));
    assert!(rendered.contains("retention: 32"));
    assert!(rendered.contains("entries: 2"));
    assert!(rendered.contains("latest: 1003"));
    assert!(rendered.contains("oldest: 1001"));
    assert!(rendered.contains("- 1003 line=v0.15.x"));
    assert!(rendered.contains("- 1001 line=v0.15.x"));

    fs::remove_dir_all(&root).unwrap();
}
