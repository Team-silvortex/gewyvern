use super::protocol_summaries;
use super::tests_docs_support::{
    IR_LOWERING_PAGE, PROTOCOL_ALIAS_INDEX_PAGE, PROTOCOL_GROUPS_PAGE, PROTOCOL_READING_PATHS_PAGE,
    PROTOCOL_SURFACE_PAGE, allowed_directory_links, allowed_group_links, current_custom_subpages,
    expected_family_directory_links, expected_hub_subpages, family_hub_page,
    filtered_surface_links, markdown_backtick_tokens, markdown_book_links,
    render_protocol_alias_index,
};
use std::collections::BTreeSet;
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
const PROTOCOL_READING_PATHS_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/docs/book/reference-protocol-reading-paths.md"
);
const EXPECTED_GROUP_FAMILY_HUBS: &[&str] = &[
    "http",
    "https",
    "http3",
    "grpc",
    "websocket",
    "graphql",
    "hy2",
    "socks5",
    "redis",
    "memcached",
    "mqtt",
    "amqp",
    "postgres",
    "mysql",
    "mongodb",
    "cassandra",
    "mssql",
    "smtp",
    "imap",
    "pop3",
    "ldap",
    "ssh",
    "kerberos",
    "stun",
    "coap",
    "dhcp",
    "arp",
    "icmp",
    "icmpv6",
    "ndp",
    "bgp",
    "ospf",
    "gre",
    "ntp",
    "snmp",
    "radius",
    "mdns",
    "ssdp",
    "gtpu",
    "wireguard",
    "ipsec",
    "tls",
    "quic",
    "dns",
    "rtsp",
    "sip",
    "ftp",
];
const HIGH_FREQUENCY_RUNTIME_HUBS: &[&str] = &[
    "http", "https", "tls", "dns", "ssh", "socks5", "postgres", "mysql", "quic", "http3",
];

#[test]
fn protocol_surface_front_door_links_core_protocol_navigation_pages() {
    let _lock = super::tests_env::lock();
    let actual =
        fs::read_to_string(PROTOCOL_SURFACE_PAGE).expect("protocol surface doc should exist");
    let links = markdown_book_links(&actual);
    let expected = [
        PROTOCOL_GROUPS_PAGE.to_string(),
        "docs/book/reference-protocol-family-shelves.md".to_string(),
        PROTOCOL_READING_PATHS_PAGE.to_string(),
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
    let _lock = super::tests_env::lock();
    let actual =
        fs::read_to_string(PROTOCOL_SURFACE_PAGE).expect("protocol surface doc should exist");
    for summary in protocol_summaries() {
        let needle = format!(
            "`{}` -> default `{}`",
            summary.protocol, summary.default_entry
        );
        assert!(
            actual.contains(&needle),
            "protocol surface front door should mention current family/default pair {needle}"
        );
    }
}

#[test]
fn protocol_alias_index_doc_matches_current_registry_surface() {
    let _lock = super::tests_env::lock();
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
    let _lock = super::tests_env::lock();
    let actual = fs::read_to_string(PROTOCOL_FAMILY_SHELVES_PATH)
        .expect("protocol family shelves doc should exist");
    let actual_links = filtered_surface_links(&actual, &allowed_directory_links());
    let expected_links = expected_family_directory_links();
    assert_eq!(actual_links, expected_links);
}

#[test]
fn protocol_groups_page_only_links_current_family_hubs_or_explicit_fallbacks() {
    let _lock = super::tests_env::lock();
    let actual =
        fs::read_to_string(PROTOCOL_GROUPS_PATH).expect("protocol groups doc should exist");
    let allowed = allowed_group_links();
    let actual_links = filtered_surface_links(&actual, &allowed_directory_links());
    assert_eq!(actual_links, allowed);
}

#[test]
fn protocol_groups_expected_family_hubs_match_current_curated_set() {
    let _lock = super::tests_env::lock();
    let expected = EXPECTED_GROUP_FAMILY_HUBS
        .iter()
        .map(|protocol| family_hub_page(protocol))
        .collect::<BTreeSet<_>>();
    let actual =
        fs::read_to_string(PROTOCOL_GROUPS_PATH).expect("protocol groups doc should exist");
    let actual_links = filtered_surface_links(&actual, &allowed_directory_links());
    assert!(
        expected.is_subset(&actual_links),
        "protocol groups page should expose the curated family hub set"
    );
}

#[test]
fn protocol_reading_paths_page_links_expected_reference_and_guidance_spine() {
    let _lock = super::tests_env::lock();
    let actual = fs::read_to_string(PROTOCOL_READING_PATHS_PATH)
        .expect("protocol reading paths doc should exist");
    let links = markdown_book_links(&actual);
    let expected = [
        PROTOCOL_SURFACE_PAGE.to_string(),
        PROTOCOL_GROUPS_PAGE.to_string(),
        "docs/book/reference-protocol-family-shelves.md".to_string(),
        IR_LOWERING_PAGE.to_string(),
        "docs/book/explanation-protocol-package-spine.md".to_string(),
        "docs/book/explanation-gewylang-to-ir.md".to_string(),
        "docs/book/explanation-gewy-to-runtime.md".to_string(),
        "docs/architecture-walkthrough-http-request.md".to_string(),
        "docs/book/how-to-add-or-debug-protocol-package.md".to_string(),
        "docs/book/how-to-validate-runtime-surface.md".to_string(),
        "docs/book/reference-diagnosis-spine.md".to_string(),
        "docs/book/reference-runtime-config.md".to_string(),
        "docs/book/reference-runtime-layout.md".to_string(),
        "docs/book/reference-gewylang-package.md".to_string(),
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    for page in expected {
        assert!(
            links.contains(&page),
            "protocol reading paths page should link `{page}`"
        );
    }
}

#[test]
fn protocol_surface_and_family_directory_link_protocol_reading_paths() {
    let _lock = super::tests_env::lock();
    let surface =
        fs::read_to_string(PROTOCOL_SURFACE_PAGE).expect("protocol surface doc should exist");
    let shelves = fs::read_to_string(PROTOCOL_FAMILY_SHELVES_PATH)
        .expect("protocol family shelves doc should exist");
    let surface_links = markdown_book_links(&surface);
    let shelves_links = markdown_book_links(&shelves);
    assert!(
        surface_links.contains(PROTOCOL_READING_PATHS_PAGE),
        "protocol surface should link protocol reading paths"
    );
    assert!(
        shelves_links.contains(PROTOCOL_READING_PATHS_PAGE),
        "protocol family shelves should link protocol reading paths"
    );
}

#[test]
fn every_family_hub_page_links_its_current_custom_subpages() {
    let _lock = super::tests_env::lock();
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
    let _lock = super::tests_env::lock();
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
    let _lock = super::tests_env::lock();
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
    let _lock = super::tests_env::lock();
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
    let _lock = super::tests_env::lock();
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

#[test]
fn every_family_hub_page_links_protocol_surface_and_ir_lowering() {
    let _lock = super::tests_env::lock();
    for summary in protocol_summaries() {
        let hub_page = family_hub_page(&summary.protocol);
        let absolute_hub = format!("{}/{}", env!("CARGO_MANIFEST_DIR"), hub_page);
        let Ok(actual) = fs::read_to_string(&absolute_hub) else {
            continue;
        };
        let links = markdown_book_links(&actual);
        assert!(
            links.contains(PROTOCOL_SURFACE_PAGE),
            "family hub page {hub_page} should link protocol surface"
        );
        assert!(
            links.contains(IR_LOWERING_PAGE),
            "family hub page {hub_page} should link IR lowering"
        );
    }
}

#[test]
fn every_family_hub_page_mentions_the_current_default_entry() {
    let _lock = super::tests_env::lock();
    for summary in protocol_summaries() {
        let hub_page = family_hub_page(&summary.protocol);
        let absolute_hub = format!("{}/{}", env!("CARGO_MANIFEST_DIR"), hub_page);
        let Ok(actual) = fs::read_to_string(&absolute_hub) else {
            continue;
        };
        let needle = format!("Default entry: `{}`", summary.default_entry);
        assert!(
            actual.contains(&needle),
            "family hub page {hub_page} should mention current default entry `{}`",
            summary.default_entry
        );
    }
}

#[test]
fn high_frequency_family_hubs_link_runtime_validation_and_diagnosis_spine() {
    let _lock = super::tests_env::lock();
    for protocol in HIGH_FREQUENCY_RUNTIME_HUBS {
        let hub_page = family_hub_page(protocol);
        let absolute_hub = format!("{}/{}", env!("CARGO_MANIFEST_DIR"), hub_page);
        let actual = fs::read_to_string(&absolute_hub)
            .unwrap_or_else(|_| panic!("family hub page should exist: {hub_page}"));
        let links = markdown_book_links(&actual);
        assert!(
            links.contains("docs/book/how-to-validate-runtime-surface.md"),
            "high-frequency family hub {hub_page} should link runtime validation guidance"
        );
        assert!(
            links.contains("docs/book/reference-diagnosis-spine.md"),
            "high-frequency family hub {hub_page} should link diagnosis spine"
        );
    }
}

#[test]
fn dot_and_doh_overlay_pages_exist_and_link_back_into_current_spines() {
    let _lock = super::tests_env::lock();
    let overlay_pairs = [
        (
            "docs/book/reference-dot-overlay.md",
            "docs/book/reference-dns-surface.md",
        ),
        (
            "docs/book/reference-doh-overlay.md",
            "docs/book/reference-http-surface.md",
        ),
    ];
    for (overlay, hub) in overlay_pairs {
        let actual = fs::read_to_string(overlay)
            .unwrap_or_else(|_| panic!("overlay page should exist: {overlay}"));
        let links = markdown_book_links(&actual);
        assert!(
            links.contains(hub),
            "overlay {overlay} should link hub {hub}"
        );
        assert!(
            links.contains(PROTOCOL_READING_PATHS_PAGE),
            "overlay {overlay} should link protocol reading paths"
        );
    }
}
