use crate::render_utils::{append_json_string, append_string_list_json};
use silvortex_bounded_io::read_bounded_utf8_regular_file;
use std::collections::BTreeMap;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

const MAX_CATALOG_DIRECTORY_ENTRIES: usize = 32_768;
const MAX_CATALOG_PROTOCOLS: usize = 2_048;
const MAX_CATALOG_ENTRIES: usize = 16_384;
const MAX_CATALOG_ARTIFACT_BYTES: u64 = 1024 * 1024;
const MAX_CATALOG_TOTAL_BYTES: u64 = 32 * 1024 * 1024;

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
    protocol_catalog_delta_between(entries[0].0, &entries[0].1, entries[1].0, &entries[1].1)
        .map(Some)
}

pub(crate) fn protocol_catalog_delta_between_paths(
    current_updated_unix_ms: u128,
    current_root: &Path,
    previous_updated_unix_ms: u128,
    previous_root: &Path,
) -> Result<ProtocolCatalogDelta, String> {
    protocol_catalog_delta_between(
        current_updated_unix_ms,
        current_root,
        previous_updated_unix_ms,
        previous_root,
    )
}

pub(crate) fn protocol_catalog_delta_json(delta: Option<&ProtocolCatalogDelta>) -> String {
    match delta {
        Some(delta) => delta.to_json(),
        None => "null".into(),
    }
}

pub(crate) fn protocol_catalog_delta_markdown(delta: Option<&ProtocolCatalogDelta>) -> String {
    match delta {
        Some(delta) => delta.to_markdown(),
        None => [
            "# Protocol Evolution",
            "",
            "No prior protocol catalog snapshot exists yet.",
            "",
            "This is the first captured protocol shelf for the current line.",
        ]
        .join("\n"),
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

#[derive(Default)]
struct ProtocolCatalogLoadBudget {
    directory_entries: usize,
    catalog_entries: usize,
    payload_bytes: u64,
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
        let previous_entries = previous_protocol
            .entries
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        delta.added_entries.extend(
            difference(&current_entries, &previous_entries)
                .into_iter()
                .map(|entry| format!("{protocol_name}:{entry}")),
        );
        delta.removed_entries.extend(
            difference(&previous_entries, &current_entries)
                .into_iter()
                .map(|entry| format!("{protocol_name}:{entry}")),
        );
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
    if !require_optional_directory(&protocols_root)? {
        return Ok(ProtocolCatalogSnapshot::default());
    }
    let mut budget = ProtocolCatalogLoadBudget::default();
    let protocol_dirs = read_sorted_dirs(&protocols_root, &mut budget)?;
    if protocol_dirs.len() > MAX_CATALOG_PROTOCOLS {
        return Err(format!(
            "protocol catalog '{}' exceeds the {} protocol limit",
            protocols_root.display(),
            MAX_CATALOG_PROTOCOLS
        ));
    }
    let mut snapshot = ProtocolCatalogSnapshot::default();
    for protocol_entry in protocol_dirs {
        let protocol_name = dir_name(&protocol_entry)?;
        let summary_json = read_optional_file(&protocol_entry.join("summary.json"), &mut budget)?;
        let mut protocol = ProtocolCatalogProtocol {
            summary_json,
            ..ProtocolCatalogProtocol::default()
        };
        let entries_root = protocol_entry.join("entries");
        if require_optional_directory(&entries_root)? {
            let entry_dirs = read_sorted_dirs(&entries_root, &mut budget)?;
            budget.catalog_entries = budget
                .catalog_entries
                .checked_add(entry_dirs.len())
                .ok_or_else(|| "protocol catalog entry counter overflowed".to_string())?;
            if budget.catalog_entries > MAX_CATALOG_ENTRIES {
                return Err(format!(
                    "protocol catalog '{}' exceeds the {} entry limit",
                    protocols_root.display(),
                    MAX_CATALOG_ENTRIES
                ));
            }
            for entry_dir in entry_dirs {
                let entry_name = dir_name(&entry_dir)?;
                let surface_json =
                    read_optional_file(&entry_dir.join("surface.json"), &mut budget)?;
                protocol.entries.insert(entry_name, surface_json);
            }
        }
        snapshot.protocols.insert(protocol_name, protocol);
    }
    Ok(snapshot)
}

fn require_optional_directory(path: &Path) -> Result<bool, String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => Err(format!(
            "protocol catalog path '{}' must be a non-symlink directory",
            path.display()
        )),
        Ok(_) => Ok(true),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!(
            "failed to inspect protocol catalog path '{}': {error}",
            path.display()
        )),
    }
}

fn read_sorted_dirs(
    root: &Path,
    budget: &mut ProtocolCatalogLoadBudget,
) -> Result<Vec<PathBuf>, String> {
    let read_dir = fs::read_dir(root).map_err(|err| {
        format!(
            "failed to inspect protocol catalog root '{}': {err}",
            root.display()
        )
    })?;
    let mut dirs = Vec::new();
    for entry in read_dir {
        budget.directory_entries = budget
            .directory_entries
            .checked_add(1)
            .ok_or_else(|| "protocol catalog directory counter overflowed".to_string())?;
        if budget.directory_entries > MAX_CATALOG_DIRECTORY_ENTRIES {
            return Err(format!(
                "protocol catalog '{}' exceeds the {} directory-entry limit",
                root.display(),
                MAX_CATALOG_DIRECTORY_ENTRIES
            ));
        }
        let entry = entry.map_err(|error| {
            format!(
                "failed to read protocol catalog root '{}': {error}",
                root.display()
            )
        })?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|error| {
            format!(
                "failed to inspect protocol catalog entry '{}': {error}",
                path.display()
            )
        })?;
        if !metadata.file_type().is_symlink() && metadata.is_dir() {
            dirs.push(path);
        }
    }
    dirs.sort();
    Ok(dirs)
}

fn dir_name(path: &Path) -> Result<String, String> {
    path.file_name()
        .and_then(|value| value.to_str())
        .map(str::to_string)
        .ok_or_else(|| format!("invalid protocol catalog path '{}'", path.display()))
}

fn read_optional_file(
    path: &Path,
    budget: &mut ProtocolCatalogLoadBudget,
) -> Result<String, String> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            return Err(format!(
                "protocol catalog artifact '{}' must be a regular non-symlink file",
                path.display()
            ));
        }
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(String::new()),
        Err(error) => {
            return Err(format!(
                "failed to inspect protocol catalog file '{}': {error}",
                path.display()
            ));
        }
    };
    if metadata.len() > MAX_CATALOG_ARTIFACT_BYTES {
        return Err(format!(
            "protocol catalog artifact '{}' exceeds the {} byte limit",
            path.display(),
            MAX_CATALOG_ARTIFACT_BYTES
        ));
    }
    if budget.payload_bytes.saturating_add(metadata.len()) > MAX_CATALOG_TOTAL_BYTES {
        return Err(format!(
            "protocol catalog '{}' exceeds the {} byte cumulative limit",
            path.display(),
            MAX_CATALOG_TOTAL_BYTES
        ));
    }
    let contents =
        read_bounded_utf8_regular_file(path, MAX_CATALOG_ARTIFACT_BYTES).map_err(|err| {
            format!(
                "failed to securely read protocol catalog file '{}': {err}",
                path.display()
            )
        })?;
    let content_len = contents.len() as u64;
    budget.payload_bytes = budget
        .payload_bytes
        .checked_add(content_len)
        .ok_or_else(|| "protocol catalog byte counter overflowed".to_string())?;
    if budget.payload_bytes > MAX_CATALOG_TOTAL_BYTES {
        return Err(format!(
            "protocol catalog '{}' exceeds the {} byte cumulative limit",
            path.display(),
            MAX_CATALOG_TOTAL_BYTES
        ));
    }
    Ok(contents)
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

    pub(crate) fn to_markdown(&self) -> String {
        let mut md = vec![
            "# Protocol Evolution".to_string(),
            String::new(),
            format!("Current snapshot: `{}`  ", self.current_updated_unix_ms),
            format!("Previous snapshot: `{}`", self.previous_updated_unix_ms),
            String::new(),
        ];
        if self.is_empty() {
            md.push("No protocol catalog changes detected between these snapshots.".into());
            return md.join("\n");
        }
        push_markdown_section(&mut md, "Added Protocols", &self.added_protocols);
        push_markdown_section(&mut md, "Removed Protocols", &self.removed_protocols);
        push_markdown_section(
            &mut md,
            "Changed Protocol Summaries",
            &self.changed_protocol_summaries,
        );
        push_markdown_section(&mut md, "Added Entries", &self.added_entries);
        push_markdown_section(&mut md, "Removed Entries", &self.removed_entries);
        push_markdown_section(
            &mut md,
            "Changed Entry Surfaces",
            &self.changed_entry_surfaces,
        );
        md.join("\n")
    }
}

fn append_text_line(target: &mut String, label: &str, items: &[String]) {
    if items.is_empty() {
        return;
    }
    target.push_str(&format!("  {label}: {}\n", items.join(", ")));
}

fn push_markdown_section(target: &mut Vec<String>, title: &str, items: &[String]) {
    if items.is_empty() {
        return;
    }
    target.push(format!("## {title}"));
    target.push(String::new());
    for item in items {
        target.push(format!("- `{item}`"));
    }
    target.push(String::new());
}
