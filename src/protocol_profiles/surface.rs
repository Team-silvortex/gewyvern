use super::overlays::overlays_for_surface;
use super::shelves::built_in_protocol_shelf;
use super::{ProtocolSummary, ProtocolSurfaceSummary};

pub(super) fn built_in_protocol_surface(
    summary: ProtocolSummary,
    selected_entry: String,
    selected_overlay: Option<String>,
) -> ProtocolSurfaceSummary {
    let selected = summary
        .entries
        .iter()
        .find(|entry| entry.mode == selected_entry)
        .expect("selected entry should exist in protocol summary");
    let sibling_entries = summary
        .entries
        .iter()
        .map(|entry| entry.mode.clone())
        .collect::<Vec<_>>();
    ProtocolSurfaceSummary {
        protocol: summary.protocol.clone(),
        entry: selected.mode.clone(),
        default_entry: summary.default_entry.clone(),
        selected_is_default: selected.default,
        protocol_aliases: summary.aliases.clone(),
        entry_aliases: selected.aliases.clone(),
        sibling_entries,
        cluster_hint: summary.cluster_hint.clone(),
        shelf: built_in_protocol_shelf(&summary.protocol, &selected.mode),
        overlays: overlays_for_surface(&summary.protocol, &selected.mode),
        selected_overlay,
    }
}
