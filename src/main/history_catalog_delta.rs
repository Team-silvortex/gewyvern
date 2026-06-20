use crate::render_utils::{append_json_string, append_string_list_json};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct ProtocolCatalogDelta {
    pub(crate) current_updated_unix_ms: u128,
    pub(crate) previous_updated_unix_ms: u128,
    pub(crate) added_protocols: Vec<String>,
    pub(crate) removed_protocols: Vec<String>,
    pub(crate) changed_protocol_summaries: Vec<String>,
    pub(crate) added_entries: Vec<String>,
    pub(crate) removed_entries: Vec<String>,
    pub(crate) changed_entry_surfaces: Vec<String>,
}

pub(crate) fn latest_protocol_catalog_delta(
    entries: &[(u128, PathBuf)],
) -> Result<Option<ProtocolCatalogDelta>, String> {
    if entries.len() < 2 {
        return Ok(None);
    }
    protocol_catalog_delta_between(
        entries[0].0,
        &entries[0].1,
        entries[1].0,
        &entries[1].1,
    )
    .map(Some)
}

pub(crate) fn protocol_catalog_delta_json(delta: Option<&ProtocolCatalogDelta>) -> String {
    match delta {
        Some(delta) => delta.to_json(),
        None => "null".into(),
    }
}

#[derive(Default)]
struct ProtocolCatalogSnapshot {
    protocols: BTreeMap<String, ProtocolCatalogProtocol>,
}

#[derive(Default)]
struct ProtocolCatalogProtocol {
    summary_json: String,
    entries: BTreeMap<String, String>,
}

fn protocol_catalog_delta_between(
    current_updated_unix_ms: u128,
    current_root: &Path,
    previous_updated_unix_ms: u128,
    previous_root: &Path,
) -> Result<ProtocolCatalogDelta, String> {
    let current = load_protocol_catalog_snapshot(current_root)?;
    let previous = load_protocol_catalog_snapshot(previous_root)?;
    let current_protocols = current.protocols.keys().cloned().collect::<Vec<_>>();
    let previous_protocols = previous.protocols.keys().cloned().collect::<Vec<_>>();
    let mut delta = ProtocolCatalogDelta {
        current_updated_unix_ms,
        previous_updated_unix_ms,
        added_protocols: difference(&current_protocols, &previous_protocols),
        removed_protocols: difference(&previous_protocols, &current_protocols),
        ..ProtocolCatalogDelta::default()
    };
    for protocol_name in current_protocols {
        let Some(current_protocol) = current.protocols.get(&protocol_name) else {
            continue;
        };
        let Some(previous_protocol) = previous.protocols.get(&protocol_name) else {
            continue;
        };
        if current_protocol.summary_json != previous_protocol.summary_json {
            delta.changed_protocol_summaries.push(protocol_name.clone());
        }
        let current_entries = current_protocol.entries.keys().cloned().collect::<Vec<_>>();
        let previous_entries = previous_protocol.entries.keys().cloned().collect::<Vec<_>>();
        delta
            .added_entries
            .extend(difference(&current_entries, &previous_entries).into_iter().map(|entry| {
                format!("{protocol_name}:{entry}")
            }));
        delta
            .removed_entries
            .extend(difference(&previous_entries, &current_entries).into_iter().map(|entry| {
                format!("{protocol_name}:{entry}")
            }));
        for entry_name in current_entries {
            let Some(previous_body) = previous_protocol.entries.get(&entry_name) else {
                continue;
            };
            let Some(current_body) = current_protocol.entries.get(&entry_name) else {
                continue;
            };
            if current_body != previous_body {
                delta
                    .changed_entry_surfaces
                    .push(format!("{protocol_name}:{entry_name}"));
            }
        }
    }
    Ok(delta)
}

fn load_protocol_catalog_snapshot(root: &Path) -> Result<ProtocolCatalogSnapshot, String> {
    let protocols_root = root.join("protocols");
    if !protocols_root.exists() {
        return Ok(ProtocolCatalogSnapshot::default());
    }
    let mut snapshot = ProtocolCatalogSnapshot::default();
    for protocol_entry in read_sorted_dirs(&protocols_root)? {
        let protocol_name = dir_name(&protocol_entry)?;
        let summary_json = read_optional_file(&protocol_entry.join("summary.json"))?;
        let mut protocol = ProtocolCatalogProtocol {
            summary_json,
            ..ProtocolCatalogProtocol::default()
        };
        let entries_root = protocol_entry.join("entries");
        if entries_root.exists() {
            for entry_dir in read_sorted_dirs(&entries_root)? {
                let entry_name = dir_name(&entry_dir)?;
                let surface_json = read_optional_file(&entry_dir.join("surface.json"))?;
                protocol.entries.insert(entry_name, surface_json);
            }
        }
        snapshot.protocols.insert(protocol_name, protocol);
    }
    Ok(snapshot)
}

fn read_sorted_dirs(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut dirs = fs::read_dir(root)
        .map_err(|err| format!("failed to inspect protocol catalog root '{}': {err}", root.display()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();
    dirs.sort();
    Ok(dirs)
}

fn dir_name(path: &Path) -> Result<String, String> {
    path.file_name()
        .and_then(|value| value.to_str())
        .map(str::to_string)
        .ok_or_else(|| format!("invalid protocol catalog path '{}'", path.display()))
}

fn read_optional_file(path: &Path) -> Result<String, String> {
    if !path.exists() {
        return Ok(String::new());
    }
    fs::read_to_string(path)
        .map_err(|err| format!("failed to read protocol catalog file '{}': {err}", path.display()))
}

fn difference(left: &[String], right: &[String]) -> Vec<String> {
    left.iter()
        .filter(|item| !right.contains(item))
        .cloned()
        .collect()
}

impl ProtocolCatalogDelta {
    pub(crate) fn is_empty(&self) -> bool {
        self.added_protocols.is_empty()
            && self.removed_protocols.is_empty()
            && self.changed_protocol_summaries.is_empty()
            && self.added_entries.is_empty()
            && self.removed_entries.is_empty()
            && self.changed_entry_surfaces.is_empty()
    }

    pub(crate) fn to_text_block(&self) -> String {
        let mut text = format!(
            "latest protocol catalog delta: current={} previous={}\n",
            self.current_updated_unix_ms, self.previous_updated_unix_ms
        );
        if self.is_empty() {
            text.push_str("  no protocol catalog changes detected\n");
            return text;
        }
        append_text_line(&mut text, "added protocols", &self.added_protocols);
        append_text_line(&mut text, "removed protocols", &self.removed_protocols);
        append_text_line(
            &mut text,
            "changed protocol summaries",
            &self.changed_protocol_summaries,
        );
        append_text_line(&mut text, "added entries", &self.added_entries);
        append_text_line(&mut text, "removed entries", &self.removed_entries);
        append_text_line(
            &mut text,
            "changed entry surfaces",
            &self.changed_entry_surfaces,
        );
        text
    }

    pub(crate) fn to_json(&self) -> String {
        let mut json = String::from("{\"current_updated_unix_ms\":");
        json.push_str(&self.current_updated_unix_ms.to_string());
        json.push_str(",\"previous_updated_unix_ms\":");
        json.push_str(&self.previous_updated_unix_ms.to_string());
        json.push_str(",\"added_protocols\":");
        append_string_list_json(&mut json, &self.added_protocols);
        json.push_str(",\"removed_protocols\":");
        append_string_list_json(&mut json, &self.removed_protocols);
        json.push_str(",\"changed_protocol_summaries\":");
        append_string_list_json(&mut json, &self.changed_protocol_summaries);
        json.push_str(",\"added_entries\":");
        append_string_list_json(&mut json, &self.added_entries);
        json.push_str(",\"removed_entries\":");
        append_string_list_json(&mut json, &self.removed_entries);
        json.push_str(",\"changed_entry_surfaces\":");
        append_string_list_json(&mut json, &self.changed_entry_surfaces);
        json.push_str(",\"status\":");
        append_json_string(
            &mut json,
            if self.is_empty() {
                "unchanged"
            } else {
                "changed"
            },
        );
        json.push('}');
        json
    }
}

fn append_text_line(target: &mut String, label: &str, items: &[String]) {
    if items.is_empty() {
        return;
    }
    target.push_str(&format!("  {label}: {}\n", items.join(", ")));
}

