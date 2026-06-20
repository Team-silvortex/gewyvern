use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

use gewyvern::protocol_profiles::{ProtocolSummary, protocol_summaries, protocol_surface};

const ALIAS_BLOCK_START: &str = "<!-- gewyvern:entry-aliases:start -->";
const ALIAS_BLOCK_END: &str = "<!-- gewyvern:entry-aliases:end -->";
const FAMILY_SHELVES_BLOCK_START: &str = "<!-- gewyvern:family-shelves:start -->";
const FAMILY_SHELVES_BLOCK_END: &str = "<!-- gewyvern:family-shelves:end -->";
const PROTOCOL_GROUPS_BLOCK_START: &str = "<!-- gewyvern:protocol-groups:start -->";
const PROTOCOL_GROUPS_BLOCK_END: &str = "<!-- gewyvern:protocol-groups:end -->";
const PROTOCOL_SURFACE_OVERVIEW_BLOCK_START: &str =
    "<!-- gewyvern:protocol-surface-overview:start -->";
const PROTOCOL_SURFACE_OVERVIEW_BLOCK_END: &str = "<!-- gewyvern:protocol-surface-overview:end -->";
const PROTOCOL_SURFACE_PAGE: &str = "docs/book/reference-protocol-surface.md";
const PROTOCOL_FAMILY_SHELVES_PAGE: &str = "docs/book/reference-protocol-family-shelves.md";
const PROTOCOL_GROUPS_PAGE: &str = "docs/book/reference-protocol-groups.md";
const PROTOCOL_GROUPS: &[ProtocolGroup] = &[
    ProtocolGroup {
        title: "Web, Proxy, And Request/Response",
        families: &["http", "http3", "socks5"],
        fallback_links: &[],
        note: None,
    },
    ProtocolGroup {
        title: "Messaging, Queue, And Cache",
        families: &["redis", "memcached", "mqtt", "amqp"],
        fallback_links: &[],
        note: None,
    },
    ProtocolGroup {
        title: "Database And Query",
        families: &["postgres", "mysql"],
        fallback_links: &[],
        note: None,
    },
    ProtocolGroup {
        title: "Mail And Mailbox",
        families: &["smtp", "imap", "pop3"],
        fallback_links: &[],
        note: None,
    },
    ProtocolGroup {
        title: "Identity, Directory, And Access",
        families: &["ldap", "ssh"],
        fallback_links: &[("Kerberos", PROTOCOL_SURFACE_PAGE)],
        note: Some(
            "- Kerberos currently routes through the general protocol surface and family contract pages rather than a dedicated hub page in this book.\n",
        ),
    },
    ProtocolGroup {
        title: "Transport, Media, And Session Control",
        families: &["quic", "dns", "rtsp", "sip", "ftp"],
        fallback_links: &[],
        note: None,
    },
];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let summaries = protocol_summaries();
    fs::write(
        root.join("docs/book/reference-protocol-alias-index.md"),
        render_protocol_alias_index(&summaries),
    )?;
    sync_generated_block(
        &root.join(PROTOCOL_SURFACE_PAGE),
        PROTOCOL_SURFACE_OVERVIEW_BLOCK_START,
        PROTOCOL_SURFACE_OVERVIEW_BLOCK_END,
        &render_protocol_surface_overview(&summaries),
    )?;
    sync_generated_block(
        &root.join(PROTOCOL_FAMILY_SHELVES_PAGE),
        FAMILY_SHELVES_BLOCK_START,
        FAMILY_SHELVES_BLOCK_END,
        &render_family_shelves_directory(&summaries),
    )?;
    sync_generated_block(
        &root.join(PROTOCOL_GROUPS_PAGE),
        PROTOCOL_GROUPS_BLOCK_START,
        PROTOCOL_GROUPS_BLOCK_END,
        &render_protocol_groups_directory(&summaries),
    )?;
    for (page, aliases) in current_custom_subpages(&summaries) {
        if aliases.is_empty() {
            continue;
        }
        sync_generated_block(
            &root.join(page),
            ALIAS_BLOCK_START,
            ALIAS_BLOCK_END,
            &render_alias_block(&aliases),
        )?;
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

fn render_family_shelves_directory(summaries: &[ProtocolSummary]) -> String {
    let mut out = String::new();
    out.push_str(FAMILY_SHELVES_BLOCK_START);
    out.push_str("\n## Current Family Shelves\n\n");
    for item in family_shelf_sections(summaries) {
        out.push_str("### ");
        out.push_str(item.label);
        out.push_str("\n\n- Hub:\n  ");
        out.push_str(&markdown_link(&item.hub));
        out.push_str("\n- Subpages:\n");
        for subpage in item.subpages {
            out.push_str("  - ");
            out.push_str(&markdown_link(&subpage));
            out.push('\n');
        }
        out.push('\n');
    }
    out.push_str(FAMILY_SHELVES_BLOCK_END);
    out
}

fn render_protocol_surface_overview(summaries: &[ProtocolSummary]) -> String {
    let family_count = summaries.len();
    let entry_count = summaries
        .iter()
        .map(|summary| summary.entries.len())
        .sum::<usize>();
    let family_pages = current_family_hub_pages(summaries);
    let mut out = String::new();
    out.push_str(PROTOCOL_SURFACE_OVERVIEW_BLOCK_START);
    out.push_str("\n## Current Surface Snapshot\n\n");
    out.push_str(&format!(
        "- Built-in families: `{family_count}`\n- Built-in canonical entries: `{entry_count}`\n"
    ));
    out.push_str("- Family/default map:\n");
    for summary in summaries {
        out.push_str("  - `");
        out.push_str(&summary.protocol);
        out.push_str("` -> default `");
        out.push_str(&summary.default_entry);
        out.push('`');
        if let Some(hint) = summary.cluster_hint.as_ref() {
            out.push_str(" in cluster `");
            out.push_str(&hint.key);
            out.push_str("`");
        }
        if let Some(page) = family_pages.get(&summary.protocol) {
            out.push_str(" via ");
            out.push_str(&markdown_link(page));
        }
        out.push('\n');
    }
    out.push('\n');
    out.push_str(PROTOCOL_SURFACE_OVERVIEW_BLOCK_END);
    out
}

fn render_protocol_groups_directory(summaries: &[ProtocolSummary]) -> String {
    let hubs = current_family_hub_pages(summaries);
    let summary_by_protocol = summaries
        .iter()
        .map(|summary| (summary.protocol.as_str(), summary))
        .collect::<BTreeMap<_, _>>();
    let mut out = String::new();
    out.push_str(PROTOCOL_GROUPS_BLOCK_START);
    for group in PROTOCOL_GROUPS {
        out.push_str("\n## ");
        out.push_str(group.title);
        out.push_str("\n\nFamilies:\n\n");
        for protocol in group.families {
            if let Some(page) = hubs.get(*protocol) {
                out.push_str("- ");
                out.push_str(&markdown_link(page));
                out.push('\n');
            }
        }
        if let Some(first_protocol) = group.families.first() {
            if let Some(summary) = summary_by_protocol.get(first_protocol) {
                if let Some(hint) = summary.cluster_hint.as_ref() {
                    out.push_str("\nCluster hint:\n\n");
                    out.push_str("- key: `");
                    out.push_str(&hint.key);
                    out.push_str("`\n- operator hint: ");
                    out.push_str(&hint.operator_hint);
                    out.push_str("\n- sibling protocols: ");
                    out.push_str(
                        &hint
                            .sibling_protocols
                            .iter()
                            .map(|item| format!("`{item}`"))
                            .collect::<Vec<_>>()
                            .join(", "),
                    );
                    out.push('\n');
                }
            }
        }
        for (label, page) in group.fallback_links {
            out.push_str("- ");
            out.push_str(label);
            out.push_str(": ");
            out.push_str(&markdown_link(page));
            out.push('\n');
        }
        if let Some(note) = group.note {
            out.push_str("\nNote:\n\n");
            out.push_str(note);
        }
    }
    out.push('\n');
    out.push_str(PROTOCOL_GROUPS_BLOCK_END);
    out
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
                if let Some(entry_summary) =
                    summary.entries.iter().find(|it| it.mode == *shelf_entry)
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

fn current_family_hub_pages(summaries: &[ProtocolSummary]) -> BTreeMap<String, String> {
    summaries
        .iter()
        .map(|summary| (summary.protocol.clone(), family_hub_page(&summary.protocol)))
        .filter(|(_, page)| PathBuf::from(page).exists())
        .collect()
}

fn family_shelf_sections(summaries: &[ProtocolSummary]) -> Vec<FamilyShelfSection> {
    let mut sections = BTreeMap::<String, BTreeSet<String>>::new();
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
            sections
                .entry(summary.protocol.clone())
                .or_default()
                .insert(shelf.page);
        }
    }

    let order = [
        "redis",
        "ftp",
        "smtp",
        "mqtt",
        "ldap",
        "postgres",
        "http",
        "socks5",
        "mysql",
        "amqp",
        "ssh",
        "rtsp",
        "quic",
        "dns",
        "http3",
        "imap",
        "sip",
        "pop3",
        "memcached",
    ];

    order
        .into_iter()
        .filter_map(|protocol| {
            let subpages = sections.remove(protocol)?;
            Some(FamilyShelfSection {
                label: family_label(protocol),
                hub: family_hub_page(protocol),
                subpages: subpages.into_iter().collect(),
            })
        })
        .collect()
}

fn family_label(protocol: &str) -> &'static str {
    match protocol {
        "redis" => "Redis",
        "http" => "HTTP",
        "http3" => "HTTP/3",
        "ftp" => "FTP",
        "smtp" => "SMTP",
        "mqtt" => "MQTT",
        "ldap" => "LDAP",
        "postgres" => "PostgreSQL",
        "mysql" => "MySQL",
        "amqp" => "AMQP",
        "ssh" => "SSH",
        "rtsp" => "RTSP",
        "dns" => "DNS",
        "imap" => "IMAP",
        "sip" => "SIP",
        "pop3" => "POP3",
        "memcached" => "Memcached",
        _ => Box::leak(protocol.to_uppercase().into_boxed_str()),
    }
}

fn sync_generated_block(
    path: &PathBuf,
    start_marker: &str,
    end_marker: &str,
    block: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut content = fs::read_to_string(path)?;
    if let (Some(start), Some(end)) = (content.find(start_marker), content.find(end_marker)) {
        let end = end + end_marker.len();
        content.replace_range(start..end, block);
    } else {
        if !content.ends_with('\n') {
            content.push('\n');
        }
        content.push('\n');
        content.push_str(block);
        content.push('\n');
    }
    fs::write(path, content)?;
    Ok(())
}

fn markdown_link(page: &str) -> String {
    format!("[{page}]({})", absolute_doc(page))
}

fn absolute_doc(page: &str) -> String {
    format!("{}/{}", env!("CARGO_MANIFEST_DIR"), page)
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

struct FamilyShelfSection {
    label: &'static str,
    hub: String,
    subpages: Vec<String>,
}

struct ProtocolGroup {
    title: &'static str,
    families: &'static [&'static str],
    fallback_links: &'static [(&'static str, &'static str)],
    note: Option<&'static str>,
}
