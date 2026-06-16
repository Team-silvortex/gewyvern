mod aliases;
mod profiles;
mod registry;
mod shelves;
mod summary;
mod surface;

use aliases::{resolve_protocol_entry_alias, split_protocol_alias};
use profiles::{PROTOCOL_PROFILES, find_protocol_profile};
use registry::{
    default_protocol_scan_set_from_registry, resolve_built_in_dsl_path, resolve_registry_alias,
    resolve_registry_entry_alias, scan_protocol_registry, scan_protocol_registry_in,
};
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
    pub entries: Vec<ProtocolEntrySummary>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtocolShelfSummary {
    pub key: String,
    pub label: String,
    pub page: String,
    pub entries: Vec<String>,
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
    pub shelf: Option<ProtocolShelfSummary>,
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

pub fn protocol_dsl_path(protocol: &str, entry: Option<&str>) -> Option<String> {
    resolve_protocol_profile(protocol, entry).map(|profile| profile.dsl_path)
}

pub fn protocol_summaries() -> Vec<ProtocolSummary> {
    if let Some(registry) = scan_protocol_registry() {
        return protocol_summaries_from_registry(registry);
    }
    built_in_protocol_summaries()
}

pub fn protocol_summary(protocol: &str) -> Option<ProtocolSummary> {
    if let Some(registry) = scan_protocol_registry() {
        return protocol_summary_from_registry(registry, protocol);
    }
    built_in_protocol_summary(protocol)
}

pub fn protocol_surface(protocol: &str, entry: &str) -> Option<ProtocolSurfaceSummary> {
    let summary = protocol_summary(protocol)?;
    let selected_entry = summary
        .entries
        .iter()
        .find(|item| item.mode == entry || item.aliases.iter().any(|alias| alias == entry))?
        .mode
        .clone();
    Some(built_in_protocol_surface(summary, selected_entry))
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
    if let Some(registry) = scan_protocol_registry() {
        if entry.is_none() {
            if let Some(manifest) = registry
                .iter()
                .find(|manifest| manifest.aliases.iter().any(|alias| alias == protocol))
            {
                return Some(ResolvedProtocolProfile {
                    protocol: manifest.protocol.clone(),
                    entry: manifest.entry.clone(),
                    dsl_path: manifest.dsl_path.clone(),
                });
            }
        }
        let canonical =
            resolve_registry_alias(&registry, protocol).unwrap_or_else(|| protocol.to_string());
        let mut matches = registry
            .into_iter()
            .filter(|manifest| manifest.protocol == canonical)
            .collect::<Vec<_>>();
        matches.sort_by(|left, right| left.entry.cmp(&right.entry));
        if let Some(selected) = if let Some(entry) = entry {
            let resolved_entry = resolve_registry_entry_alias(&matches, &canonical, entry)
                .map(str::to_string)
                .unwrap_or_else(|| entry.to_string());
            matches
                .into_iter()
                .find(|manifest| manifest.entry == resolved_entry)
        } else {
            matches
                .iter()
                .find(|manifest| manifest.default)
                .cloned()
                .or_else(|| matches.into_iter().next())
        } {
            return Some(ResolvedProtocolProfile {
                protocol: selected.protocol,
                entry: selected.entry,
                dsl_path: selected.dsl_path,
            });
        }
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
            dsl_path: resolve_built_in_dsl_path(item.dsl_path),
        })
}

pub fn default_protocol_scan_set() -> Vec<ResolvedProtocolProfile> {
    if let Some(registry) = scan_protocol_registry() {
        return default_protocol_scan_set_from_registry(registry);
    }
    PROTOCOL_PROFILES
        .iter()
        .flat_map(|profile| {
            profile
                .entries
                .iter()
                .filter_map(|entry| resolve_protocol_profile(profile.name, Some(entry.mode)))
                .collect::<Vec<_>>()
        })
        .collect()
}

pub fn default_protocol_scan_set_from_dir(dir: &str) -> Option<Vec<ResolvedProtocolProfile>> {
    let registry = scan_protocol_registry_in(std::path::Path::new(dir))?;
    Some(default_protocol_scan_set_from_registry(registry))
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_alias_policy;
#[cfg(test)]
mod tests_coap;
#[cfg(test)]
mod tests_dhcp;
#[cfg(test)]
mod tests_docs;
#[cfg(test)]
mod tests_fallback;
#[cfg(test)]
mod tests_layout;
#[cfg(test)]
mod tests_manifest_parity;
#[cfg(test)]
mod tests_ntp;
#[cfg(test)]
mod tests_snmp;
#[cfg(test)]
mod tests_stun;
#[cfg(test)]
mod tests_surface;
