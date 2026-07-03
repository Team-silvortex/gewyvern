use std::collections::{BTreeMap, BTreeSet};

use super::aliases::{protocol_aliases, protocol_entry_aliases, split_protocol_alias};
use super::clusters::built_in_protocol_cluster_hint;
use super::profiles::{PROTOCOL_PROFILES, ProtocolProfile, find_protocol_profile};
use super::{ProtocolEntrySummary, ProtocolSummary, RegistryManifest};

pub(super) fn protocol_summaries_from_registry(
    registry: Vec<RegistryManifest>,
) -> Vec<ProtocolSummary> {
    let mut protocols = BTreeMap::<String, RegistryProtocolSummary>::new();
    for manifest in registry {
        let protocol = protocols
            .entry(manifest.protocol.clone())
            .or_insert_with(RegistryProtocolSummary::default);
        protocol.aliases.extend(manifest.aliases.clone());
        let entry = protocol
            .entries
            .entry(manifest.entry)
            .or_insert_with(RegistryEntrySummary::default);
        entry.default |= manifest.default;
        entry.aliases.extend(manifest.aliases.clone());
        entry.aliases.extend(manifest.entry_aliases);
    }
    protocols
        .into_iter()
        .map(|(protocol, summary)| {
            let default_entry = summary.default_entry();
            let entries = summary.entries(&protocol);
            let aliases = summary
                .aliases
                .into_iter()
                .chain(protocol_aliases_for(&protocol))
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            ProtocolSummary {
                cluster_hint: built_in_protocol_cluster_hint(&protocol),
                protocol,
                default_entry,
                aliases,
                entries,
            }
        })
        .collect()
}

pub(super) fn protocol_summary_from_registry(
    registry: Vec<RegistryManifest>,
    protocol: &str,
) -> Option<ProtocolSummary> {
    let canonical = registry
        .iter()
        .find(|manifest| manifest.aliases.iter().any(|alias| alias == protocol))
        .map(|manifest| manifest.protocol.clone())
        .unwrap_or_else(|| protocol.to_string());
    protocol_summaries_from_registry(registry)
        .into_iter()
        .find(|summary| summary.protocol == canonical)
}

pub(super) fn built_in_protocol_summaries() -> Vec<ProtocolSummary> {
    PROTOCOL_PROFILES.iter().map(summary_for_profile).collect()
}

pub(super) fn built_in_protocol_summary(protocol: &str) -> Option<ProtocolSummary> {
    let (canonical, _) = split_protocol_alias(protocol);
    find_protocol_profile(canonical).map(summary_for_profile)
}

#[derive(Default)]
struct RegistryEntrySummary {
    aliases: BTreeSet<String>,
    default: bool,
}

#[derive(Default)]
struct RegistryProtocolSummary {
    aliases: BTreeSet<String>,
    entries: BTreeMap<String, RegistryEntrySummary>,
}

impl RegistryProtocolSummary {
    fn default_entry(&self) -> String {
        self.entries
            .iter()
            .find(|(_, summary)| summary.default)
            .or_else(|| self.entries.iter().next())
            .map(|(entry, _)| entry.clone())
            .unwrap_or_default()
    }

    fn entries(&self, protocol: &str) -> Vec<ProtocolEntrySummary> {
        self.entries
            .iter()
            .map(|(mode, summary)| ProtocolEntrySummary {
                mode: mode.clone(),
                default: summary.default,
                aliases: summary
                    .aliases
                    .iter()
                    .cloned()
                    .chain(entry_aliases_for(protocol, mode))
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect(),
            })
            .collect()
    }
}

fn summary_for_profile(profile: &ProtocolProfile) -> ProtocolSummary {
    ProtocolSummary {
        protocol: profile.name.to_string(),
        default_entry: profile.default_entry.to_string(),
        aliases: protocol_aliases_for(profile.name),
        cluster_hint: built_in_protocol_cluster_hint(profile.name),
        entries: profile
            .entries
            .iter()
            .map(|entry| ProtocolEntrySummary {
                mode: entry.mode.to_string(),
                default: entry.mode == profile.default_entry,
                aliases: entry_aliases_for(profile.name, entry.mode),
            })
            .collect(),
    }
}

fn protocol_aliases_for(protocol: &str) -> Vec<String> {
    protocol_aliases()
        .filter(|alias| alias.protocol == protocol && alias.entry.is_none())
        .map(|alias| alias.alias.to_string())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn entry_aliases_for(protocol: &str, mode: &str) -> Vec<String> {
    protocol_aliases()
        .chain(protocol_entry_aliases())
        .filter(|alias| alias.protocol == protocol && alias.entry == Some(mode))
        .map(|alias| alias.alias.to_string())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}
