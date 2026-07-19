use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use super::profiles::{PACKAGED_SHARE_ROOT, PROTOCOL_REGISTRY_ROOT};
use super::{RegistryManifest, ResolvedProtocolProfile};
use crate::runtime_layout::{
    packaged_share_roots as discovered_share_roots,
    protocol_registry_roots as discovered_registry_roots,
};

const MAX_REGISTRY_DIRECTORIES: usize = 4096;
const MAX_REGISTRY_MANIFESTS: usize = 2048;
const MAX_REGISTRY_MANIFEST_BYTES: u64 = 64 * 1024;

pub(super) fn scan_protocol_registry() -> Option<Vec<RegistryManifest>> {
    for root in protocol_registry_roots() {
        if let Some(registry) = scan_protocol_registry_in(&root) {
            return Some(registry);
        }
    }
    None
}

pub(super) fn scan_protocol_registry_in(root: &Path) -> Option<Vec<RegistryManifest>> {
    scan_protocol_registry_in_strict(root).ok()
}

pub(super) fn scan_protocol_registry_in_strict(
    root: &Path,
) -> Result<Vec<RegistryManifest>, String> {
    let mut manifests = Vec::new();
    let mut state = RegistryScanState::default();
    collect_registry_manifests(root, &mut manifests, &mut state)?;
    if manifests.is_empty() {
        return Err(format!(
            "protocol registry '{}' contains no gewy.pkg manifests",
            root.display()
        ));
    }
    Ok(manifests)
}

fn protocol_registry_roots() -> Vec<PathBuf> {
    discovered_registry_roots(
        Path::new(PROTOCOL_REGISTRY_ROOT),
        Path::new(PACKAGED_SHARE_ROOT),
    )
}

fn packaged_share_roots() -> Vec<PathBuf> {
    let mut roots = discovered_share_roots(Path::new(PACKAGED_SHARE_ROOT));
    roots.push(Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf());
    roots
}

pub(super) fn resolve_built_in_dsl_path(raw: &str) -> String {
    if Path::new(raw).exists() {
        return raw.to_string();
    }
    let relative_path = raw
        .strip_prefix("dsl/")
        .map(PathBuf::from)
        .or_else(|| raw.strip_prefix("protocols/").map(|_| PathBuf::from(raw)))
        .or_else(|| {
            raw.split("/dsl/")
                .nth(1)
                .map(|path| Path::new("dsl").join(path))
        })
        .or_else(|| {
            raw.split("/protocols/")
                .nth(1)
                .map(|path| Path::new("protocols").join(path))
        });
    if let Some(relative) = relative_path {
        for share_root in packaged_share_roots() {
            let candidate = share_root.join(&relative);
            if candidate.exists() {
                return candidate.to_string_lossy().into_owned();
            }
        }
    }
    let file_name = raw
        .split("/dsl/")
        .nth(1)
        .or_else(|| Path::new(raw).file_name().and_then(|name| name.to_str()));
    if let Some(relative) = file_name {
        for share_root in packaged_share_roots() {
            let candidate = share_root.join("dsl").join(relative);
            if candidate.exists() {
                return candidate.to_string_lossy().into_owned();
            }
        }
    }
    raw.to_string()
}

pub(super) fn default_protocol_scan_set_from_registry(
    registry: Vec<RegistryManifest>,
) -> Vec<ResolvedProtocolProfile> {
    let mut seen = BTreeSet::<(String, String)>::new();
    let mut resolved = registry
        .into_iter()
        .filter(|manifest| seen.insert((manifest.protocol.clone(), manifest.entry.clone())))
        .map(|manifest| ResolvedProtocolProfile {
            protocol: manifest.protocol,
            entry: manifest.entry,
            dsl_path: manifest.dsl_path,
        })
        .collect::<Vec<_>>();
    resolved.sort_by(|left, right| {
        left.protocol
            .cmp(&right.protocol)
            .then_with(|| left.entry.cmp(&right.entry))
    });
    resolved
}

fn collect_registry_manifests(
    dir: &Path,
    manifests: &mut Vec<RegistryManifest>,
    state: &mut RegistryScanState,
) -> Result<(), String> {
    if !dir.exists() {
        return Ok(());
    }
    let dir_metadata = fs::symlink_metadata(dir).map_err(|err| err.to_string())?;
    if dir_metadata.file_type().is_symlink() {
        return Ok(());
    }
    let canonical_dir = fs::canonicalize(dir).map_err(|err| err.to_string())?;
    if !state.visited_dirs.insert(canonical_dir) {
        return Ok(());
    }
    state.directories_scanned += 1;
    if state.directories_scanned > MAX_REGISTRY_DIRECTORIES {
        return Err(format!(
            "protocol registry exceeded directory budget of {}",
            MAX_REGISTRY_DIRECTORIES
        ));
    }
    for entry in fs::read_dir(dir).map_err(|err| err.to_string())? {
        let entry = entry.map_err(|err| err.to_string())?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|err| err.to_string())?;
        if metadata.file_type().is_symlink() {
            continue;
        }
        if path.is_dir() {
            collect_registry_manifests(&path, manifests, state)?;
            continue;
        }
        if path.file_name().and_then(|name| name.to_str()) != Some("gewy.pkg") {
            continue;
        }
        state.manifests_loaded += 1;
        if state.manifests_loaded > MAX_REGISTRY_MANIFESTS {
            return Err(format!(
                "protocol registry exceeded manifest budget of {}",
                MAX_REGISTRY_MANIFESTS
            ));
        }
        manifests.push(read_registry_manifest(&path)?);
    }
    Ok(())
}

fn read_registry_manifest(path: &Path) -> Result<RegistryManifest, String> {
    let manifest_metadata = fs::metadata(path).map_err(|err| err.to_string())?;
    if manifest_metadata.len() > MAX_REGISTRY_MANIFEST_BYTES {
        return Err(format!(
            "manifest '{}' exceeded size budget of {} bytes",
            path.display(),
            MAX_REGISTRY_MANIFEST_BYTES
        ));
    }
    let input = fs::read_to_string(path).map_err(|err| err.to_string())?;
    let root = path
        .parent()
        .ok_or_else(|| format!("manifest '{}' has no parent", path.display()))?;
    let mut entry = None;
    let mut protocol = None;
    let mut protocol_entry = None;
    let mut default = false;
    let mut aliases = Vec::new();
    let mut entry_aliases = Vec::new();

    for raw_line in input.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| format!("invalid manifest line '{}'", line))?;
        let key = key.trim();
        let value = value.trim().trim_matches('"');
        match key {
            "entry" => entry = Some(value.to_string()),
            "register.protocol" => protocol = Some(value.to_string()),
            "register.entry" => protocol_entry = Some(value.to_string()),
            "register.default" => default = matches!(value, "true" | "1" | "yes"),
            "register.aliases" => {
                aliases = value
                    .split(',')
                    .map(str::trim)
                    .filter(|item| !item.is_empty())
                    .map(str::to_string)
                    .collect();
            }
            "register.entry_aliases" => {
                entry_aliases = value
                    .split(',')
                    .map(str::trim)
                    .filter(|item| !item.is_empty())
                    .map(str::to_string)
                    .collect();
            }
            _ => {}
        }
    }

    let entry = entry.ok_or_else(|| format!("manifest '{}' missing entry", path.display()))?;
    let protocol = protocol
        .ok_or_else(|| format!("manifest '{}' missing register.protocol", path.display()))?;
    let protocol_entry = protocol_entry
        .ok_or_else(|| format!("manifest '{}' missing register.entry", path.display()))?;
    validate_package_entry(root, &entry, path)?;
    let dsl_path = fs::canonicalize(root)
        .map_err(|err| format!("failed to resolve package root '{}': {err}", root.display()))?;

    Ok(RegistryManifest {
        protocol,
        entry: protocol_entry,
        default,
        aliases,
        entry_aliases,
        dsl_path: dsl_path.to_string_lossy().into_owned(),
    })
}

fn validate_package_entry(root: &Path, entry: &str, manifest: &Path) -> Result<(), String> {
    let relative = Path::new(entry);
    if relative.as_os_str().is_empty()
        || relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err(format!(
            "manifest '{}' entry must be a normalized relative path",
            manifest.display()
        ));
    }

    let canonical_root = fs::canonicalize(root)
        .map_err(|err| format!("failed to resolve package root '{}': {err}", root.display()))?;
    let mut candidate = root.to_path_buf();
    for component in relative.components() {
        let std::path::Component::Normal(component) = component else {
            unreachable!("entry components were validated above");
        };
        candidate.push(component);
        let metadata = fs::symlink_metadata(&candidate)
            .map_err(|err| format!("failed to inspect '{}': {err}", candidate.display()))?;
        if metadata.file_type().is_symlink() {
            return Err(format!(
                "manifest '{}' entry must not traverse symlinks",
                manifest.display()
            ));
        }
    }
    let metadata = fs::symlink_metadata(&candidate)
        .map_err(|err| format!("failed to inspect '{}': {err}", candidate.display()))?;
    if !metadata.is_file() {
        return Err(format!(
            "manifest '{}' entry must resolve to a regular file",
            manifest.display()
        ));
    }
    let canonical_entry = fs::canonicalize(&candidate)
        .map_err(|err| format!("failed to resolve '{}': {err}", candidate.display()))?;
    if !canonical_entry.starts_with(&canonical_root) {
        return Err(format!(
            "manifest '{}' entry escapes its package root",
            manifest.display()
        ));
    }
    Ok(())
}

pub(super) fn resolve_registry_alias(
    registry: &[RegistryManifest],
    protocol: &str,
) -> Option<String> {
    registry
        .iter()
        .find(|manifest| manifest.aliases.iter().any(|alias| alias == protocol))
        .map(|manifest| manifest.protocol.clone())
}

pub(super) fn resolve_registry_entry_alias<'a>(
    registry: &'a [RegistryManifest],
    protocol: &str,
    entry: &str,
) -> Option<&'a str> {
    registry
        .iter()
        .find(|manifest| {
            manifest.protocol == protocol
                && (manifest.entry_aliases.iter().any(|alias| alias == entry)
                    || manifest.aliases.iter().any(|alias| alias == entry))
        })
        .map(|manifest| manifest.entry.as_str())
}

#[derive(Default)]
struct RegistryScanState {
    visited_dirs: HashSet<PathBuf>,
    directories_scanned: usize,
    manifests_loaded: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_registry_root() -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("gewyvern-registry-test-{unique}"))
    }

    #[test]
    fn oversized_manifest_is_rejected() {
        let root = temp_registry_root();
        let package_dir = root.join("http").join("request");
        fs::create_dir_all(&package_dir).expect("package dir should be creatable");
        fs::write(package_dir.join("session.gewy"), "fragment packet_meta {}")
            .expect("entry file should be writable");
        let oversized = "x".repeat((MAX_REGISTRY_MANIFEST_BYTES as usize) + 8);
        fs::write(package_dir.join("gewy.pkg"), oversized).expect("manifest should be writable");

        let result = scan_protocol_registry_in(&root);
        let error = scan_protocol_registry_in_strict(&root).unwrap_err();
        let _ = fs::remove_dir_all(&root);
        assert!(result.is_none());
        assert!(error.contains("exceeded size budget"));
    }

    #[test]
    fn package_entry_cannot_escape_its_manifest_directory() {
        let root = temp_registry_root();
        let package_dir = root.join("http").join("request");
        fs::create_dir_all(&package_dir).expect("package dir should be creatable");
        fs::write(root.join("outside.gewy"), "fragment packet_meta {}")
            .expect("outside entry should be writable");
        fs::write(
            package_dir.join("gewy.pkg"),
            "entry=../../outside.gewy\nregister.protocol=http\nregister.entry=request\n",
        )
        .expect("manifest should be writable");

        let error = scan_protocol_registry_in_strict(&root).unwrap_err();
        let _ = fs::remove_dir_all(&root);
        assert!(error.contains("entry must be a normalized relative path"));
    }

    #[cfg(unix)]
    #[test]
    fn package_entry_cannot_traverse_symlinks() {
        use std::os::unix::fs::symlink;

        let root = temp_registry_root();
        let package_dir = root.join("http").join("request");
        fs::create_dir_all(&package_dir).expect("package dir should be creatable");
        fs::write(root.join("outside.gewy"), "fragment packet_meta {}")
            .expect("outside entry should be writable");
        symlink(root.join("outside.gewy"), package_dir.join("main.gewy"))
            .expect("entry symlink should be creatable");
        fs::write(
            package_dir.join("gewy.pkg"),
            "entry=main.gewy\nregister.protocol=http\nregister.entry=request\n",
        )
        .expect("manifest should be writable");

        let error = scan_protocol_registry_in_strict(&root).unwrap_err();
        let _ = fs::remove_dir_all(&root);
        assert!(error.contains("entry must not traverse symlinks"));
    }
}
