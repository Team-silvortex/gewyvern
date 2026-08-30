mod aliases;
mod clusters;
mod overlays;
mod profiles;
mod registry;
mod semantics;
mod shelves;
mod summary;
mod surface;

use aliases::{protocol_entry_aliases, resolve_protocol_entry_alias, split_protocol_alias};
use overlays::selected_overlay_for_alias;
use profiles::{PROTOCOL_PROFILES, find_protocol_profile};
use registry::{
    default_protocol_scan_set_from_registry,
    resolve_built_in_dsl_path as resolve_built_in_dsl_path_inner, resolve_registry_alias,
    resolve_registry_entry_alias, scan_protocol_registry, scan_protocol_registry_in,
    scan_protocol_registry_in_strict,
};
use std::fs;
use std::path::Path;
use summary::{
    built_in_protocol_summaries, built_in_protocol_summary, protocol_summaries_from_registry,
    protocol_summary_from_registry,
};
use surface::built_in_protocol_surface;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedProtocolProfile {
    pub protocol: String,
    pub entry: String,
    pub dsl_path: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtocolEntrySummary {
    pub mode: String,
    pub default: bool,
    pub aliases: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtocolSummary {
    pub protocol: String,
    pub default_entry: String,
    pub aliases: Vec<String>,
    pub cluster_hint: Option<ProtocolClusterHintSummary>,
    pub entries: Vec<ProtocolEntrySummary>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtocolClusterHintSummary {
    pub key: String,
    pub label: String,
    pub operator_hint: String,
    pub sibling_protocols: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtocolShelfSummary {
    pub key: String,
    pub label: String,
    pub page: String,
    pub entries: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtocolOverlaySummary {
    pub key: String,
    pub label: String,
    pub kind: String,
    pub operator_hint: String,
    pub aliases: Vec<String>,
    pub companion_protocol: Option<String>,
    pub companion_entry: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtocolEntrySemanticsSummary {
    pub category: String,
    pub operator_focus: String,
    pub typical_signal: Option<String>,
    pub primary_failure_mode: Option<String>,
    pub primary_failure_detail: Option<String>,
    pub primary_failure_basis: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtocolSurfaceSummary {
    pub protocol: String,
    pub entry: String,
    pub default_entry: String,
    pub selected_is_default: bool,
    pub protocol_aliases: Vec<String>,
    pub entry_aliases: Vec<String>,
    pub sibling_entries: Vec<String>,
    pub cluster_hint: Option<ProtocolClusterHintSummary>,
    pub shelf: Option<ProtocolShelfSummary>,
    pub entry_semantics: Option<ProtocolEntrySemanticsSummary>,
    pub overlays: Vec<ProtocolOverlaySummary>,
    pub selected_overlay: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RegistryManifest {
    protocol: String,
    entry: String,
    default: bool,
    aliases: Vec<String>,
    entry_aliases: Vec<String>,
    dsl_path: String,
}

#[derive(Debug)]
/// An immutable, request-local view of the discovered protocol registry.
pub struct ProtocolCatalogSnapshot {
    registry: Option<Vec<RegistryManifest>>,
}

impl ProtocolCatalogSnapshot {
    /// Discovers the current registry without installing a process-wide cache.
    pub fn discover() -> Self {
        Self {
            registry: scan_protocol_registry(),
        }
    }

    #[cfg(test)]
    fn discover_in(dir: &Path) -> Option<Self> {
        Some(Self {
            registry: Some(scan_protocol_registry_in(dir)?),
        })
    }

    pub fn protocol_summaries(&self) -> Vec<ProtocolSummary> {
        self.registry
            .as_deref()
            .map(protocol_summaries_from_registry)
            .unwrap_or_else(built_in_protocol_summaries)
    }

    pub fn protocol_summary(&self, protocol: &str) -> Option<ProtocolSummary> {
        let (protocol, _) = split_protocol_alias(protocol);
        if let Some(registry) = self.registry.as_deref() {
            return protocol_summary_from_registry(registry, protocol);
        }
        built_in_protocol_summary(protocol)
    }

    pub fn resolve_protocol_profile(
        &self,
        protocol: &str,
        entry: Option<&str>,
    ) -> Option<ResolvedProtocolProfile> {
        resolve_protocol_profile_with_registry(self.registry.as_deref(), protocol, entry)
    }

    pub fn default_protocol_scan_set(&self) -> Vec<ResolvedProtocolProfile> {
        if let Some(registry) = self.registry.as_deref() {
            return default_protocol_scan_set_from_registry(registry);
        }
        PROTOCOL_PROFILES
            .iter()
            .flat_map(|profile| {
                profile.entries.iter().filter_map(|entry| {
                    resolve_protocol_profile_with_registry(None, profile.name, Some(entry.mode))
                })
            })
            .collect()
    }
}

pub fn protocol_dsl_path(protocol: &str, entry: Option<&str>) -> Option<String> {
    resolve_protocol_profile(protocol, entry).map(|profile| profile.dsl_path)
}

pub fn resolve_built_in_dsl_path(raw: &str) -> String {
    resolve_built_in_dsl_path_inner(raw)
}

pub fn protocol_summaries() -> Vec<ProtocolSummary> {
    ProtocolCatalogSnapshot::discover().protocol_summaries()
}

pub fn protocol_summary(protocol: &str) -> Option<ProtocolSummary> {
    ProtocolCatalogSnapshot::discover().protocol_summary(protocol)
}

pub fn protocol_surface(protocol: &str, entry: &str) -> Option<ProtocolSurfaceSummary> {
    protocol_surface_with_summary_lookup(protocol, entry, protocol_summary)
}

pub fn protocol_surface_from_summaries(
    summaries: &[ProtocolSummary],
    protocol: &str,
    entry: &str,
) -> Option<ProtocolSurfaceSummary> {
    protocol_surface_with_summary_lookup(protocol, entry, |protocol| {
        summaries
            .iter()
            .find(|summary| {
                summary.protocol == protocol
                    || summary.aliases.iter().any(|alias| alias == protocol)
            })
            .cloned()
    })
}

fn protocol_surface_with_summary_lookup(
    protocol: &str,
    entry: &str,
    mut summary_for: impl FnMut(&str) -> Option<ProtocolSummary>,
) -> Option<ProtocolSurfaceSummary> {
    let raw_protocol = protocol;
    let (protocol, _) = split_protocol_alias(protocol);
    let selected_overlay = selected_overlay_for_alias(raw_protocol).map(str::to_string);
    let (summary, selected_entry) = if let Some(summary) = summary_for(protocol) {
        let selected_entry = summary
            .entries
            .iter()
            .find(|item| item.mode == entry || item.aliases.iter().any(|alias| alias == entry))?
            .mode
            .clone();
        (summary, selected_entry)
    } else {
        let alias = protocol_entry_aliases().find(|alias| alias.alias == protocol)?;
        let summary = summary_for(alias.protocol)?;
        let selected_entry = alias.entry?;
        if selected_entry != entry {
            return None;
        }
        (summary, selected_entry.to_string())
    };
    Some(built_in_protocol_surface(
        summary,
        selected_entry,
        selected_overlay,
    ))
}

pub fn protocol_surface_from_summary(
    summary: ProtocolSummary,
    entry: &str,
) -> Option<ProtocolSurfaceSummary> {
    let selected_entry = summary
        .entries
        .iter()
        .find(|item| item.mode == entry || item.aliases.iter().any(|alias| alias == entry))?
        .mode
        .clone();
    Some(built_in_protocol_surface(summary, selected_entry, None))
}

pub fn protocol_names() -> Vec<String> {
    protocol_summaries()
        .into_iter()
        .map(|summary| summary.protocol)
        .collect()
}

pub fn protocol_default_entry(protocol: &str) -> Option<String> {
    protocol_summary(protocol).map(|summary| summary.default_entry)
}

pub fn protocol_entries(protocol: &str) -> Option<Vec<String>> {
    protocol_summary(protocol).map(|summary| {
        summary
            .entries
            .into_iter()
            .map(|entry| entry.mode)
            .collect::<Vec<_>>()
    })
}

pub fn resolve_protocol_profile(
    protocol: &str,
    entry: Option<&str>,
) -> Option<ResolvedProtocolProfile> {
    ProtocolCatalogSnapshot::discover().resolve_protocol_profile(protocol, entry)
}

fn resolve_protocol_profile_with_registry(
    registry: Option<&[RegistryManifest]>,
    protocol: &str,
    entry: Option<&str>,
) -> Option<ResolvedProtocolProfile> {
    if let Some(registry) = registry
        && let Some(profile) = resolve_protocol_profile_from_registry(registry, protocol, entry)
    {
        return Some(profile);
    }
    let (protocol_name, alias_entry) = split_protocol_alias(protocol);
    let profile = find_protocol_profile(protocol_name)?;
    let resolved_entry = entry
        .and_then(|item| resolve_protocol_entry_alias(protocol_name, item).or(Some(item)))
        .or(alias_entry)
        .unwrap_or(profile.default_entry);
    profile
        .entries
        .iter()
        .find(|item| item.mode == resolved_entry)
        .map(|item| ResolvedProtocolProfile {
            protocol: profile.name.to_string(),
            entry: item.mode.to_string(),
            dsl_path: resolve_built_in_dsl_path_inner(item.dsl_path),
        })
}

fn resolve_protocol_profile_from_registry(
    registry: &[RegistryManifest],
    protocol: &str,
    entry: Option<&str>,
) -> Option<ResolvedProtocolProfile> {
    if entry.is_none()
        && let Some(manifest) = registry
            .iter()
            .find(|manifest| manifest.aliases.iter().any(|alias| alias == protocol))
    {
        return Some(ResolvedProtocolProfile {
            protocol: manifest.protocol.clone(),
            entry: manifest.entry.clone(),
            dsl_path: manifest.dsl_path.clone(),
        });
    }
    let canonical =
        resolve_registry_alias(registry, protocol).unwrap_or_else(|| protocol.to_string());
    let mut matches = registry
        .iter()
        .filter(|manifest| manifest.protocol == canonical)
        .collect::<Vec<_>>();
    matches.sort_by(|left, right| left.entry.cmp(&right.entry));
    let selected = if let Some(entry) = entry {
        let resolved_entry = resolve_registry_entry_alias(registry, &canonical, entry)
            .map(str::to_string)
            .unwrap_or_else(|| entry.to_string());
        matches
            .into_iter()
            .find(|manifest| manifest.entry == resolved_entry)
    } else {
        matches
            .iter()
            .find(|manifest| manifest.default)
            .copied()
            .or_else(|| matches.into_iter().next())
    }?;
    Some(ResolvedProtocolProfile {
        protocol: selected.protocol.clone(),
        entry: selected.entry.clone(),
        dsl_path: selected.dsl_path.clone(),
    })
}

#[cfg(test)]
fn resolve_protocol_profile_from_dir(
    dir: &Path,
    protocol: &str,
    entry: Option<&str>,
) -> Option<ResolvedProtocolProfile> {
    let registry = scan_protocol_registry_in(dir)?;
    resolve_protocol_profile_from_registry(&registry, protocol, entry)
}

pub fn default_protocol_scan_set() -> Vec<ResolvedProtocolProfile> {
    ProtocolCatalogSnapshot::discover().default_protocol_scan_set()
}

pub fn default_protocol_scan_set_from_dir(dir: &str) -> Option<Vec<ResolvedProtocolProfile>> {
    let registry = scan_protocol_registry_in(std::path::Path::new(dir))?;
    Some(default_protocol_scan_set_from_registry(&registry))
}

pub fn validate_protocol_registry_dir(dir: &str) -> Result<Vec<ResolvedProtocolProfile>, String> {
    let registry = scan_protocol_registry_in_strict(std::path::Path::new(dir))?;
    Ok(default_protocol_scan_set_from_registry(&registry))
}

pub fn protocol_target_name_for_template_id(template_id: &str) -> Option<String> {
    if template_id.trim().is_empty() {
        return None;
    }
    protocol_target_name_from_scan_set(default_protocol_scan_set(), template_id)
}

fn protocol_target_name_from_scan_set(
    profiles: impl IntoIterator<Item = ResolvedProtocolProfile>,
    template_id: &str,
) -> Option<String> {
    profiles.into_iter().find_map(|profile| {
        protocol_profile_matches_template_id(&profile, template_id)
            .then(|| format!("scan:{}:{}", profile.protocol, profile.entry))
    })
}

#[cfg(test)]
fn protocol_target_name_for_template_id_from_dir(dir: &Path, template_id: &str) -> Option<String> {
    if template_id.trim().is_empty() {
        return None;
    }
    let registry = scan_protocol_registry_in(dir)?;
    let profiles = default_protocol_scan_set_from_registry(&registry);
    protocol_target_name_from_scan_set(profiles, template_id)
}

fn protocol_profile_matches_template_id(
    profile: &ResolvedProtocolProfile,
    template_id: &str,
) -> bool {
    protocol_template_candidates(&profile.dsl_path)
        .into_iter()
        .any(|candidate| candidate == template_id)
}

fn protocol_template_candidates(dsl_path: &str) -> Vec<String> {
    let path = Path::new(dsl_path);
    let mut candidates = Vec::new();
    if path.is_file() {
        push_protocol_template_candidates(&mut candidates, path);
        return candidates;
    }
    if path.is_dir() {
        let main = path.join("main.gewy");
        if main.exists() {
            push_protocol_template_candidates(&mut candidates, &main);
        }
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                let entry_path = entry.path();
                if entry_path == main
                    || entry_path.extension().and_then(|ext| ext.to_str()) != Some("gewy")
                {
                    continue;
                }
                push_protocol_template_candidates(&mut candidates, &entry_path);
            }
        }
    }
    candidates
}

fn push_protocol_template_candidates(candidates: &mut Vec<String>, path: &Path) {
    if let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) {
        candidates.push(stem.to_string());
    }
    if let Some(template_id) = read_template_id_from_dsl(path)
        && !candidates.iter().any(|candidate| candidate == &template_id)
    {
        candidates.push(template_id);
    }
}

fn read_template_id_from_dsl(path: &Path) -> Option<String> {
    let input = fs::read_to_string(path).ok()?;
    extract_template_id(&input)
}

fn extract_template_id(input: &str) -> Option<String> {
    input.lines().find_map(|line| {
        let line = line.trim();
        if let Some(tail) = line.strip_prefix("template(:") {
            let end = tail.find(')')?;
            let template_id = tail[..end].trim();
            return (!template_id.is_empty()).then(|| template_id.to_string());
        }
        line.strip_prefix("template :")
            .or_else(|| line.strip_prefix("template "))
            .map(str::trim)
            .filter(|template_id| !template_id.is_empty())
            .map(ToString::to_string)
    })
}

#[cfg(test)]
mod tests_env {
    use std::sync::{Mutex, MutexGuard, OnceLock};

    pub(crate) struct EnvGuard {
        key: &'static str,
        previous: Option<String>,
    }

    impl EnvGuard {
        pub(crate) fn set(key: &'static str, value: impl Into<String>) -> Self {
            let previous = std::env::var(key).ok();
            unsafe {
                std::env::set_var(key, value.into());
            }
            Self { key, previous }
        }

        pub(crate) fn remove(key: &'static str) -> Self {
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

    pub(crate) fn lock() -> MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[cfg(test)]
fn protocol_fixture_path(relative: &str) -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("protocols")
        .join(relative)
        .to_string_lossy()
        .into_owned()
}
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_alias_policy;
#[cfg(test)]
mod tests_coap;
#[cfg(test)]
mod tests_database_error_surfaces;
#[cfg(test)]
mod tests_dhcp;
#[cfg(test)]
mod tests_docs;
#[cfg(test)]
mod tests_docs_support;
#[cfg(test)]
mod tests_fallback;
#[cfg(test)]
mod tests_http3_hy2;
#[cfg(test)]
mod tests_layout;
#[cfg(test)]
mod tests_ldap_write_failures;
#[cfg(test)]
mod tests_manifest_parity;
#[cfg(test)]
mod tests_mysql_auth;
#[cfg(test)]
mod tests_ntp;
#[cfg(test)]
mod tests_path_resolution;
#[cfg(test)]
mod tests_postgres_auth_denied;
#[cfg(test)]
mod tests_quic;
#[cfg(test)]
mod tests_radius;
#[cfg(test)]
mod tests_semantics;
#[cfg(test)]
mod tests_snmp;
#[cfg(test)]
mod tests_stun;
#[cfg(test)]
mod tests_surface;
#[cfg(test)]
mod tests_target_names;
#[cfg(test)]
mod tests_tls;
#[cfg(test)]
mod tests_validation_paths;
#[cfg(test)]
mod tests_wireguard;
