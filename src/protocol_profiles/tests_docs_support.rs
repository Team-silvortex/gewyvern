use super::{protocol_summaries, protocol_surface};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;

pub(super) const PROTOCOL_SURFACE_PAGE: &str = "docs/book/reference-protocol-surface.md";
pub(super) const IR_LOWERING_PAGE: &str = "docs/book/reference-ir-lowering.md";
pub(super) const PROTOCOL_GROUPS_PAGE: &str = "docs/book/reference-protocol-groups.md";
pub(super) const PROTOCOL_READING_PATHS_PAGE: &str =
    "docs/book/reference-protocol-reading-paths.md";
pub(super) const PROTOCOL_ALIAS_INDEX_PAGE: &str = "docs/book/reference-protocol-alias-index.md";

pub(super) fn render_protocol_alias_index() -> String {
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

    let summaries = protocol_summaries();
    let last_index = summaries.len().saturating_sub(1);
    for (index, summary) in summaries.into_iter().enumerate() {
        out.push_str(&format!("## `{}`\n\n", summary.protocol));
        out.push_str(&format!("Default entry: `{}`\n", summary.default_entry));
        if summary.aliases.is_empty() {
            out.push_str("Protocol aliases: none\n");
        } else {
            out.push_str("Protocol aliases: ");
            out.push_str(&quoted_csv(&summary.aliases));
            out.push('\n');
        }
        out.push_str("Entry aliases:\n");

        let mut entries = summary.entries;
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
        if index != last_index {
            out.push('\n');
        }
    }

    out
}

pub(super) fn expected_family_directory_links() -> BTreeSet<String> {
    let mut links = current_family_hub_pages();
    for (protocol, subpages) in expected_hub_subpages() {
        links.insert(family_hub_page(&protocol));
        links.extend(subpages);
    }
    links
}

pub(super) fn expected_hub_subpages() -> BTreeMap<String, BTreeSet<String>> {
    let mut by_protocol = BTreeMap::<String, BTreeSet<String>>::new();
    for summary in protocol_summaries() {
        let hub_page = family_hub_page(&summary.protocol);
        let subpages = summary
            .entries
            .iter()
            .filter_map(|entry| protocol_surface(&summary.protocol, &entry.mode))
            .filter_map(|surface| surface.shelf)
            .map(|shelf| shelf.page)
            .filter(|page| page.ends_with("-surface.md"))
            .filter(|page| page != PROTOCOL_SURFACE_PAGE && page != &hub_page)
            .collect::<BTreeSet<_>>();
        if !subpages.is_empty() {
            by_protocol.insert(summary.protocol, subpages);
        }
    }
    by_protocol
}

pub(super) fn current_custom_subpages() -> BTreeMap<String, CustomSubpageSummary> {
    let mut pages = BTreeMap::<String, CustomSubpageSummary>::new();
    for summary in protocol_summaries() {
        let hub_page = family_hub_page(&summary.protocol);
        for entry in &summary.entries {
            let Some(surface) = protocol_surface(&summary.protocol, &entry.mode) else {
                continue;
            };
            let Some(shelf) = surface.shelf else {
                continue;
            };
            if !shelf.page.ends_with("-surface.md") {
                continue;
            }
            if shelf.page == PROTOCOL_SURFACE_PAGE || shelf.page == hub_page {
                continue;
            }
            let page = shelf.page.clone();
            let item = pages.entry(page).or_insert_with(|| CustomSubpageSummary {
                protocol: summary.protocol.clone(),
                entries: BTreeSet::new(),
                aliases: BTreeSet::new(),
            });
            item.entries.extend(shelf.entries.iter().cloned());
            for shelf_entry in &shelf.entries {
                if let Some(entry_summary) = summary
                    .entries
                    .iter()
                    .find(|item| &item.mode == shelf_entry)
                {
                    item.aliases.extend(entry_summary.aliases.iter().cloned());
                }
            }
        }
    }
    pages
}

pub(super) fn family_hub_page(protocol: &str) -> String {
    format!("docs/book/reference-{protocol}-surface.md")
}

pub(super) fn allowed_directory_links() -> BTreeSet<String> {
    [
        PROTOCOL_SURFACE_PAGE.to_string(),
        PROTOCOL_GROUPS_PAGE.to_string(),
        PROTOCOL_ALIAS_INDEX_PAGE.to_string(),
        IR_LOWERING_PAGE.to_string(),
    ]
    .into_iter()
    .collect()
}

pub(super) fn allowed_group_links() -> BTreeSet<String> {
    current_family_hub_pages()
}

pub(super) fn filtered_surface_links(
    content: &str,
    allowed: &BTreeSet<String>,
) -> BTreeSet<String> {
    markdown_book_links(content)
        .into_iter()
        .filter(|link| link.starts_with("docs/book/reference-"))
        .filter(|link| link.ends_with("-surface.md") || link == IR_LOWERING_PAGE)
        .filter(|link| !allowed.contains(link))
        .collect()
}

pub(super) fn markdown_book_links(content: &str) -> BTreeSet<String> {
    let mut links = BTreeSet::new();
    let prefix = concat!(env!("CARGO_MANIFEST_DIR"), "/");
    let mut rest = content;
    while let Some(start) = rest.find("](") {
        let candidate = &rest[start + 2..];
        let Some(end) = candidate.find(')') else {
            break;
        };
        let link = &candidate[..end];
        if let Some(relative) = link.strip_prefix(prefix) {
            links.insert(relative.to_string());
        }
        rest = &candidate[end + 1..];
    }
    links
}

pub(super) fn markdown_backtick_tokens(content: &str) -> BTreeSet<String> {
    let mut tokens = BTreeSet::new();
    let mut in_token = false;
    let mut current = String::new();
    for ch in content.chars() {
        if ch == '`' {
            if in_token {
                tokens.insert(current.clone());
                current.clear();
            }
            in_token = !in_token;
            continue;
        }
        if in_token {
            current.push(ch);
        }
    }
    tokens
}

fn current_family_hub_pages() -> BTreeSet<String> {
    protocol_summaries()
        .into_iter()
        .map(|summary| family_hub_page(&summary.protocol))
        .filter(|page| fs::metadata(page).is_ok())
        .collect()
}

fn quoted_csv(values: &[String]) -> String {
    values
        .iter()
        .map(|value| format!("`{value}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CustomSubpageSummary {
    pub(super) protocol: String,
    pub(super) entries: BTreeSet<String>,
    pub(super) aliases: BTreeSet<String>,
}
