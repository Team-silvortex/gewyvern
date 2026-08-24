use super::{protocol_summaries, protocol_surface, protocol_surface_from_summaries};

#[test]
fn shared_protocol_summaries_preserve_surface_resolution() {
    let summaries = protocol_summaries();
    for (protocol, entry) in [("http", "request"), ("https", "request"), ("redis", "auth")] {
        assert_eq!(
            protocol_surface_from_summaries(&summaries, protocol, entry),
            protocol_surface(protocol, entry),
        );
    }
    assert!(protocol_surface_from_summaries(&summaries, "missing", "entry").is_none());
}

#[test]
fn every_built_in_protocol_family_has_a_family_hub_page() {
    for summary in protocol_summaries() {
        let hub_page = format!("docs/book/reference-{}-surface.md", summary.protocol);
        assert!(
            std::fs::metadata(&hub_page).is_ok(),
            "family hub page missing for protocol {} at {}",
            summary.protocol,
            hub_page
        );
    }
}

#[test]
fn every_built_in_protocol_entry_exposes_a_surface_and_shelf() {
    for summary in protocol_summaries() {
        let cluster_hint = summary
            .cluster_hint
            .clone()
            .unwrap_or_else(|| panic!("cluster hint missing for protocol {}", summary.protocol));
        assert!(
            cluster_hint.sibling_protocols.contains(&summary.protocol),
            "cluster hint for protocol {} should list itself",
            summary.protocol
        );
        assert!(
            summary
                .entries
                .iter()
                .any(|entry| entry.mode == summary.default_entry),
            "default entry {} missing from protocol {}",
            summary.default_entry,
            summary.protocol
        );

        for entry in &summary.entries {
            let surface = protocol_surface(&summary.protocol, &entry.mode).unwrap_or_else(|| {
                panic!(
                    "surface missing for protocol {} entry {}",
                    summary.protocol, entry.mode
                )
            });
            let shelf = surface.shelf.unwrap_or_else(|| {
                panic!(
                    "shelf missing for protocol {} entry {}",
                    summary.protocol, entry.mode
                )
            });

            assert_eq!(surface.protocol, summary.protocol);
            assert_eq!(surface.entry, entry.mode);
            assert_eq!(surface.cluster_hint, summary.cluster_hint);
            assert!(
                surface.sibling_entries.contains(&summary.default_entry),
                "default entry {} missing from sibling entries for protocol {}",
                summary.default_entry,
                summary.protocol
            );
            assert!(
                shelf.entries.contains(&entry.mode),
                "entry {} missing from shelf {} for protocol {}",
                entry.mode,
                shelf.key,
                summary.protocol
            );
            assert!(
                !shelf.page.is_empty(),
                "shelf page missing for protocol {} entry {}",
                summary.protocol,
                entry.mode
            );
            assert_ne!(
                shelf.page, "docs/book/reference-protocol-surface.md",
                "protocol {} entry {} should not fall back to the generic protocol surface",
                summary.protocol, entry.mode
            );
        }
    }
}

#[test]
fn every_surface_shelf_only_references_known_sibling_entries() {
    for summary in protocol_summaries() {
        let canonical_entries = summary
            .entries
            .iter()
            .map(|entry| entry.mode.as_str())
            .collect::<Vec<_>>();

        for entry in &summary.entries {
            let shelf = protocol_surface(&summary.protocol, &entry.mode)
                .expect("surface should exist")
                .shelf
                .expect("shelf should exist");

            for shelf_entry in &shelf.entries {
                assert!(
                    canonical_entries.contains(&shelf_entry.as_str()),
                    "shelf {} for protocol {} references unknown entry {}",
                    shelf.key,
                    summary.protocol,
                    shelf_entry
                );
            }
        }
    }
}
