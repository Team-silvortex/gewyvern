use super::{protocol_summaries, protocol_surface};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;

const PROTOCOL_ALIAS_INDEX_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/docs/book/reference-protocol-alias-index.md"
);
const PROTOCOL_FAMILY_SHELVES_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/docs/book/reference-protocol-family-shelves.md"
);
const PROTOCOL_GROUPS_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/docs/book/reference-protocol-groups.md"
);
const PROTOCOL_SURFACE_PAGE: &str = "docs/book/reference-protocol-surface.md";
const IR_LOWERING_PAGE: &str = "docs/book/reference-ir-lowering.md";
const PROTOCOL_GROUPS_PAGE: &str = "docs/book/reference-protocol-groups.md";
const PROTOCOL_ALIAS_INDEX_PAGE: &str = "docs/book/reference-protocol-alias-index.md";
const EXPECTED_GROUP_FAMILY_HUBS: &[&str] = &[
    "http",
    "http3",
    "socks5",
    "redis",
    "memcached",
    "mqtt",
    "amqp",
    "postgres",
    "mysql",
    "smtp",
    "imap",
    "pop3",
    "ldap",
    "ssh",
    "quic",
    "dns",
    "rtsp",
    "sip",
    "ftp",
];

#[test]
fn protocol_surface_front_door_links_core_protocol_navigation_pages() {
    let actual = fs::read_to_string(PROTOCOL_SURFACE_PAGE)
        .expect("protocol surface doc should exist");
    let links = markdown_book_links(&actual);
    let expected = [
        PROTOCOL_GROUPS_PAGE.to_string(),
        "docs/book/reference-protocol-family-shelves.md".to_string(),
        PROTOCOL_ALIAS_INDEX_PAGE.to_string(),
        IR_LOWERING_PAGE.to_string(),
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    for page in expected {
        assert!(
            links.contains(&page),
            "protocol surface front door should link `{page}`"
        );
    }
}

#[test]
fn protocol_surface_front_door_mentions_each_current_family_default_pair() {
    let actual = fs::read_to_string(PROTOCOL_SURFACE_PAGE)
        .expect("protocol surface doc should exist");
    for summary in protocol_summaries() {
        let needle = format!("`{}` -> default `{}`", summary.protocol, summary.default_entry);
        assert!(
            actual.contains(&needle),
            "protocol surface front door should mention current family/default pair {needle}"
        );
    }
}

#[test]
fn protocol_alias_index_doc_matches_current_registry_surface() {
    let expected = render_protocol_alias_index();
    let actual = fs::read_to_string(PROTOCOL_ALIAS_INDEX_PATH)
        .expect("protocol alias index doc should exist");
    if actual != expected {
        let first_diff = actual
            .bytes()
            .zip(expected.bytes())
            .position(|(left, right)| left != right)
            .unwrap_or_else(|| actual.len().min(expected.len()));
        panic!(
            "protocol alias index doc mismatch at byte {} (actual len {}, expected len {})\nactual tail: {:?}\nexpected tail: {:?}",
            first_diff,
            actual.len(),
            expected.len(),
            &actual[first_diff.saturating_sub(40)..actual.len().min(first_diff + 80)],
            &expected[first_diff.saturating_sub(40)..expected.len().min(first_diff + 80)],
        );
    }
}

#[test]
fn protocol_family_shelf_directory_lists_current_custom_hubs_and_subpages() {
    let actual = fs::read_to_string(PROTOCOL_FAMILY_SHELVES_PATH)
        .expect("protocol family shelves doc should exist");
    let actual_links = filtered_surface_links(&actual, &allowed_directory_links());
    let expected_links = expected_family_directory_links();
    assert_eq!(actual_links, expected_links);
}

#[test]
fn protocol_groups_page_only_links_current_family_hubs_or_explicit_fallbacks() {
    let actual = fs::read_to_string(PROTOCOL_GROUPS_PATH)
        .expect("protocol groups doc should exist");
    let allowed = allowed_group_links();
    let actual_links = filtered_surface_links(&actual, &allowed_directory_links());
    assert_eq!(actual_links, allowed);
}

#[test]
fn protocol_groups_expected_family_hubs_match_current_curated_set() {
    let expected = EXPECTED_GROUP_FAMILY_HUBS
        .iter()
        .map(|protocol| family_hub_page(protocol))
        .collect::<BTreeSet<_>>();
    let actual = fs::read_to_string(PROTOCOL_GROUPS_PATH)
        .expect("protocol groups doc should exist");
    let actual_links = filtered_surface_links(&actual, &allowed_directory_links());
    assert!(
        expected.is_subset(&actual_links),
        "protocol groups page should expose the curated family hub set"
    );
}

#[test]
fn every_family_hub_page_links_its_current_custom_subpages() {
    for (protocol, subpages) in expected_hub_subpages() {
        let hub_page = family_hub_page(&protocol);
        let absolute_hub = format!("{}/{}", env!("CARGO_MANIFEST_DIR"), hub_page);
        let actual = fs::read_to_string(&absolute_hub)
            .unwrap_or_else(|_| panic!("family hub page should exist: {hub_page}"));
        let allowed = [
            hub_page.clone(),
            PROTOCOL_SURFACE_PAGE.to_string(),
            IR_LOWERING_PAGE.to_string(),
        ]
        .into_iter()
        .collect::<BTreeSet<_>>();
        let actual_links = filtered_surface_links(&actual, &allowed);
        assert_eq!(actual_links, subpages, "hub page mismatch for {protocol}");
    }
}

#[test]
fn every_custom_subpage_links_back_to_its_family_hub() {
    for (page, shelf) in current_custom_subpages() {
        let absolute_page = format!("{}/{}", env!("CARGO_MANIFEST_DIR"), page);
        let actual = fs::read_to_string(&absolute_page)
            .unwrap_or_else(|_| panic!("custom subpage should exist: {page}"));
        let links = markdown_book_links(&actual);
        assert!(
            links.contains(&family_hub_page(&shelf.protocol)),
            "custom subpage {page} should link back to hub {}",
            family_hub_page(&shelf.protocol)
        );
    }
}

#[test]
fn every_custom_subpage_mentions_each_current_shelf_entry() {
    for (page, shelf) in current_custom_subpages() {
        let absolute_page = format!("{}/{}", env!("CARGO_MANIFEST_DIR"), page);
        let actual = fs::read_to_string(&absolute_page)
            .unwrap_or_else(|_| panic!("custom subpage should exist: {page}"));
        let tokens = markdown_backtick_tokens(&actual);
        for entry in &shelf.entries {
            assert!(
                tokens.contains(entry),
                "custom subpage {page} should mention canonical entry `{entry}`"
            );
        }
    }
}

#[test]
fn every_custom_subpage_mentions_each_current_entry_alias() {
    for (page, shelf) in current_custom_subpages() {
        let absolute_page = format!("{}/{}", env!("CARGO_MANIFEST_DIR"), page);
        let actual = fs::read_to_string(&absolute_page)
            .unwrap_or_else(|_| panic!("custom subpage should exist: {page}"));
        for alias in &shelf.aliases {
            assert!(
                actual.contains(alias),
                "custom subpage {page} should mention entry alias `{alias}`"
            );
        }
    }
}

#[test]
fn every_family_hub_page_mentions_each_current_protocol_alias() {
    for summary in protocol_summaries() {
        if summary.aliases.is_empty() {
            continue;
        }
        let hub_page = family_hub_page(&summary.protocol);
        let absolute_hub = format!("{}/{}", env!("CARGO_MANIFEST_DIR"), hub_page);
        let Ok(actual) = fs::read_to_string(&absolute_hub) else {
            continue;
        };
        for alias in &summary.aliases {
            assert!(
                actual.contains(alias),
                "family hub page {hub_page} should mention protocol alias `{alias}`"
            );
        }
    }
}

fn render_protocol_alias_index() -> String {
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
        out.push_str(&format!("Default entry: `{}`  \n", summary.default_entry));
        if summary.aliases.is_empty() {
            out.push_str("Protocol aliases: none  \n");
        } else {
            out.push_str("Protocol aliases: ");
            out.push_str(&quoted_csv(&summary.aliases));
            out.push_str("  \n");
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

fn quoted_csv(values: &[String]) -> String {
    values
        .iter()
        .map(|value| format!("`{value}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn expected_family_directory_links() -> BTreeSet<String> {
    let mut links = BTreeSet::new();
    for (protocol, subpages) in expected_hub_subpages() {
        links.insert(family_hub_page(&protocol));
        links.extend(subpages);
    }
    links
}

fn expected_hub_subpages() -> BTreeMap<String, BTreeSet<String>> {
    let mut by_protocol = BTreeMap::<String, BTreeSet<String>>::new();
    for summary in protocol_summaries() {
        let hub_page = family_hub_page(&summary.protocol);
        let subpages = summary
            .entries
            .iter()
            .filter_map(|entry| protocol_surface(&summary.protocol, &entry.mode))
            .filter_map(|surface| surface.shelf)
            .map(|shelf| shelf.page)
            .filter(|page| page != PROTOCOL_SURFACE_PAGE && page != &hub_page)
            .collect::<BTreeSet<_>>();
        if !subpages.is_empty() {
            by_protocol.insert(summary.protocol, subpages);
        }
    }
    by_protocol
}

fn current_custom_subpages() -> BTreeMap<String, CustomSubpageSummary> {
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

fn family_hub_page(protocol: &str) -> String {
    format!("docs/book/reference-{protocol}-surface.md")
}

fn allowed_directory_links() -> BTreeSet<String> {
    [
        PROTOCOL_SURFACE_PAGE.to_string(),
        PROTOCOL_GROUPS_PAGE.to_string(),
        PROTOCOL_ALIAS_INDEX_PAGE.to_string(),
        IR_LOWERING_PAGE.to_string(),
    ]
    .into_iter()
    .collect()
}

fn allowed_group_links() -> BTreeSet<String> {
    current_family_hub_pages()
}

fn current_family_hub_pages() -> BTreeSet<String> {
    protocol_summaries()
        .into_iter()
        .map(|summary| family_hub_page(&summary.protocol))
        .filter(|page| fs::metadata(page).is_ok())
        .collect()
}

fn filtered_surface_links(content: &str, allowed: &BTreeSet<String>) -> BTreeSet<String> {
    markdown_book_links(content)
        .into_iter()
        .filter(|link| link.starts_with("docs/book/reference-"))
        .filter(|link| link.ends_with("-surface.md") || link == IR_LOWERING_PAGE)
        .filter(|link| !allowed.contains(link))
        .collect()
}

fn markdown_book_links(content: &str) -> BTreeSet<String> {
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

fn markdown_backtick_tokens(content: &str) -> BTreeSet<String> {
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

#[derive(Clone, Debug, Eq, PartialEq)]
struct CustomSubpageSummary {
    protocol: String,
    entries: BTreeSet<String>,
    aliases: BTreeSet<String>,
}
