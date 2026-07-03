use super::aliases::{protocol_aliases, protocol_entry_aliases};
use std::collections::{BTreeMap, BTreeSet};

fn alias_text(alias: &super::aliases::ProtocolAlias) -> &'static str {
    alias.alias
}

fn alias_protocol(alias: &super::aliases::ProtocolAlias) -> &'static str {
    alias.protocol
}

fn alias_entry(alias: &super::aliases::ProtocolAlias) -> Option<&'static str> {
    alias.entry
}

#[test]
fn alias_tokens_use_stable_lowercase_slug_style() {
    for alias in protocol_aliases().chain(protocol_entry_aliases()) {
        assert!(
            alias_text(alias)
                .chars()
                .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '_'),
            "alias `{}` should stay lowercase kebab/snake style",
            alias_text(alias)
        );
    }
}

#[test]
fn protocol_scope_does_not_reuse_the_same_alias_for_multiple_targets() {
    let mut seen = BTreeMap::<(&str, &str), BTreeSet<Option<&str>>>::new();
    for alias in protocol_aliases().chain(protocol_entry_aliases()) {
        seen.entry((alias_protocol(alias), alias_text(alias)))
            .or_default()
            .insert(alias_entry(alias));
    }

    for ((protocol, alias), targets) in seen {
        assert_eq!(
            targets.len(),
            1,
            "protocol `{protocol}` reuses alias `{alias}` for multiple targets: {targets:?}"
        );
    }
}

#[test]
fn entry_aliases_do_not_shadow_other_canonical_entries_within_the_same_protocol() {
    let mut canonical_entries = BTreeMap::<&str, BTreeSet<&str>>::new();
    for alias in protocol_aliases() {
        if let Some(entry) = alias_entry(alias) {
            canonical_entries
                .entry(alias_protocol(alias))
                .or_default()
                .insert(entry);
        }
    }

    for alias in protocol_entry_aliases() {
        let Some(target_entry) = alias_entry(alias) else {
            continue;
        };
        let protocol = alias_protocol(alias);
        let canonical = canonical_entries.get(protocol).cloned().unwrap_or_default();
        if canonical.contains(alias_text(alias)) {
            assert_eq!(
                alias_text(alias),
                target_entry,
                "entry alias `{}` in protocol `{}` should not shadow canonical entry `{}`",
                alias_text(alias),
                protocol,
                alias_text(alias)
            );
        }
    }
}

#[test]
fn protocol_alias_dash_and_snake_pairs_stay_consistent_for_prefixed_families() {
    let mut aliases_by_protocol = BTreeMap::<&str, BTreeSet<&str>>::new();
    for alias in protocol_aliases() {
        aliases_by_protocol
            .entry(alias_protocol(alias))
            .or_default()
            .insert(alias_text(alias));
    }

    for (protocol, aliases) in aliases_by_protocol {
        let protocol_snake_prefix = format!("{protocol}_");
        let protocol_dash_prefix = format!("{protocol}-");
        let keeps_both_styles = aliases
            .iter()
            .any(|alias| alias.starts_with(protocol_snake_prefix.as_str()));
        if !keeps_both_styles {
            continue;
        }

        for alias in &aliases {
            if !alias.starts_with(protocol_dash_prefix.as_str()) {
                continue;
            }
            let snake = alias.replace('-', "_");
            assert!(
                aliases.contains(snake.as_str()),
                "protocol alias `{alias}` should keep snake-case peer `{snake}`"
            );
        }
    }
}

#[test]
fn protocol_alias_snake_pairs_keep_dash_peers_when_family_uses_both_styles() {
    let mut aliases_by_protocol = BTreeMap::<&str, BTreeSet<&str>>::new();
    for alias in protocol_aliases() {
        aliases_by_protocol
            .entry(alias_protocol(alias))
            .or_default()
            .insert(alias_text(alias));
    }

    for (protocol, aliases) in aliases_by_protocol {
        let protocol_snake_prefix = format!("{protocol}_");
        let protocol_dash_prefix = format!("{protocol}-");
        let keeps_both_styles = aliases
            .iter()
            .any(|alias| alias.starts_with(protocol_dash_prefix.as_str()));
        if !keeps_both_styles {
            continue;
        }

        for alias in &aliases {
            if !alias.starts_with(protocol_snake_prefix.as_str()) {
                continue;
            }
            let dash = alias.replace('_', "-");
            assert!(
                aliases.contains(dash.as_str()),
                "protocol alias `{alias}` should keep dash peer `{dash}`"
            );
        }
    }
}
