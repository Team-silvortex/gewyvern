use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

use gewyvern::protocol_profiles::{ProtocolSummary, protocol_summaries, protocol_surface};

const ALIAS_BLOCK_START: &str = "<!-- gewyvern:entry-aliases:start -->";
const ALIAS_BLOCK_END: &str = "<!-- gewyvern:entry-aliases:end -->";
const PROTOCOL_SURFACE_PAGE: &str = "docs/book/reference-protocol-surface.md";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let summaries = protocol_summaries();
    fs::write(
        root.join("docs/book/reference-protocol-alias-index.md"),
        render_protocol_alias_index(&summaries),
    )?;
    for (page, aliases) in current_custom_subpages(&summaries) {
        if aliases.is_empty() {
            continue;
        }
        sync_alias_block(&root.join(page), &aliases)?;
    }
    Ok(())
}

fn render_protocol_alias_index(summaries: &[ProtocolSummary]) -> String {
    let mut out = String::new();
    out.push_str("# Reference: Protocol Alias Index\n\n");
    out.push_str(
        "Use this page when you want the current built-in alias map without scanning every protocol package manifest by hand.\n\n",
    );
    out.push_str(
        "This index is intended to stay synchronized with the registry-backed protocol surface in the current tree.\n\n",
    );
    out.push_str("## Format\n\n");
    out.push_str(
        "- `Default entry` is the canonical entry chosen when a family is selected without an explicit entry.\n",
    );
    out.push_str(
        "- `Protocol aliases` are accepted family-level spellings that resolve before entry selection.\n",
    );
    out.push_str(
        "- `Entry aliases` are accepted per-entry spellings that resolve to a canonical entry name.\n\n",
    );

    for (index, summary) in summaries.iter().enumerate() {
        out.push_str(&format!("## `{}`\n\n", summary.protocol));
        out.push_str(&format!("Default entry: `{}`  \n", summary.default_entry));
        if summary.aliases.is_empty() {
            out.push_str("Protocol aliases: none  \n");
        } else {
            out.push_str("Protocol aliases: ");
            out.push_str(&quoted_csv(&summary.aliases));
            out.push_str("  \n");
        }
        out.push_str("Entry aliases:\n");

        let mut entries = summary.entries.clone();
        entries.sort_by(|left, right| {
            (!left.default)
                .cmp(&!right.default)
                .then_with(|| left.mode.cmp(&right.mode))
        });

        for entry in entries {
            out.push_str("- `");
            out.push_str(&entry.mode);
            out.push('`');
            if entry.default {
                out.push_str(" (default)");
            }
            out.push_str(": ");
            if entry.aliases.is_empty() {
                out.push_str("none\n");
            } else {
                out.push_str(&quoted_csv(&entry.aliases));
                out.push('\n');
            }
        }
        if index + 1 != summaries.len() {
            out.push('\n');
        }
    }

    out
}

fn quoted_csv(values: &[String]) -> String {
    values
        .iter()
        .map(|value| format!("`{value}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn current_custom_subpages(summaries: &[ProtocolSummary]) -> BTreeMap<String, BTreeSet<String>> {
    let mut pages = BTreeMap::<String, BTreeSet<String>>::new();
    for summary in summaries {
        let hub_page = family_hub_page(&summary.protocol);
        for entry in &summary.entries {
            let Some(surface) = protocol_surface(&summary.protocol, &entry.mode) else {
                continue;
            };
            let Some(shelf) = surface.shelf else {
                continue;
            };
            if shelf.page == PROTOCOL_SURFACE_PAGE || shelf.page == hub_page {
                continue;
            }
            let aliases = pages.entry(shelf.page).or_default();
            for shelf_entry in &shelf.entries {
                if let Some(entry_summary) = summary.entries.iter().find(|it| it.mode == *shelf_entry)
                {
                    aliases.extend(entry_summary.aliases.iter().cloned());
                }
            }
        }
    }
    pages
}

fn family_hub_page(protocol: &str) -> String {
    format!("docs/book/reference-{protocol}-surface.md")
}

fn sync_alias_block(path: &PathBuf, aliases: &BTreeSet<String>) -> Result<(), Box<dyn std::error::Error>> {
    let mut content = fs::read_to_string(path)?;
    let block = render_alias_block(aliases);
    if let (Some(start), Some(end)) = (content.find(ALIAS_BLOCK_START), content.find(ALIAS_BLOCK_END))
    {
        let end = end + ALIAS_BLOCK_END.len();
        content.replace_range(start..end, &block);
    } else {
        if !content.ends_with('\n') {
            content.push('\n');
        }
        content.push('\n');
        content.push_str(&block);
        content.push('\n');
    }
    fs::write(path, content)?;
    Ok(())
}

fn render_alias_block(aliases: &BTreeSet<String>) -> String {
    let mut out = String::new();
    out.push_str(ALIAS_BLOCK_START);
    out.push_str("\n## Current Entry Aliases\n\n");
    out.push_str("This generated block tracks the aliases that currently resolve into this custom surface.\n\n");
    for alias in aliases {
        out.push_str("- `");
        out.push_str(alias);
        out.push_str("`\n");
    }
    out.push_str("\n");
    out.push_str(ALIAS_BLOCK_END);
    out
}
