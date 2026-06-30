use crate::render_utils::{append_json_string, append_string_list_json};
use gewyvern::protocol_profiles::{
    ProtocolClusterHintSummary, ProtocolEntrySummary, ProtocolOverlaySummary, ProtocolShelfSummary,
    ProtocolSummary, ProtocolSurfaceSummary, protocol_summaries, protocol_summary,
    protocol_surface, protocol_surface_from_summary,
};
use std::collections::BTreeMap;

pub(super) fn api_protocol_catalog_json() -> String {
    let summaries = protocol_summaries();
    api_protocol_catalog_json_from_summaries(&summaries)
}

pub(super) fn api_protocol_catalog_json_from_summaries(summaries: &[ProtocolSummary]) -> String {
    let mut json = String::with_capacity(4096);
    json.push_str("{\"surface\":\"protocol_catalog\",\"count\":");
    json.push_str(&summaries.len().to_string());
    json.push_str(",\"protocols\":[");
    for (index, summary) in summaries.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        append_protocol_summary_json(&mut json, summary);
    }
    json.push_str("]}");
    json
}

pub(super) fn api_protocol_clusters_json() -> String {
    let clusters = protocol_clusters();
    api_protocol_clusters_json_from_clusters(&clusters)
}

pub(super) fn api_protocol_clusters_json_from_summaries(summaries: &[ProtocolSummary]) -> String {
    let clusters = protocol_clusters_from_summaries(summaries);
    api_protocol_clusters_json_from_clusters(&clusters)
}

fn api_protocol_clusters_json_from_clusters(clusters: &[ProtocolClusterCatalogItem]) -> String {
    let mut json = String::with_capacity(4096);
    json.push_str("{\"surface\":\"protocol_cluster_catalog\",\"count\":");
    json.push_str(&clusters.len().to_string());
    json.push_str(",\"clusters\":[");
    for (index, cluster) in clusters.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        append_protocol_cluster_json(&mut json, cluster);
    }
    json.push_str("]}");
    json
}

pub(super) fn api_protocol_cluster_json(key: &str) -> Option<String> {
    let cluster = protocol_clusters()
        .into_iter()
        .find(|cluster| cluster.key == key)?;
    Some(api_protocol_cluster_json_from_item(&cluster))
}

pub(super) fn api_protocol_cluster_json_from_summaries(
    summaries: &[ProtocolSummary],
    key: &str,
) -> Option<String> {
    let cluster = protocol_clusters_from_summaries(summaries)
        .into_iter()
        .find(|cluster| cluster.key == key)?;
    Some(api_protocol_cluster_json_from_item(&cluster))
}

fn api_protocol_cluster_json_from_item(cluster: &ProtocolClusterCatalogItem) -> String {
    let mut json = String::with_capacity(1024);
    append_protocol_cluster_json(&mut json, &cluster);
    json
}

pub(super) fn api_protocol_summary_json(protocol_name: &str) -> Option<String> {
    let summary = protocol_summary(protocol_name)?;
    Some(api_protocol_summary_json_from_summary(&summary))
}

pub(super) fn api_protocol_summary_json_from_summary(summary: &ProtocolSummary) -> String {
    let mut json = String::with_capacity(1024);
    append_protocol_summary_json(&mut json, &summary);
    json
}

pub(super) fn api_protocol_surface_by_name_json(
    protocol_name: &str,
    entry: &str,
) -> Option<String> {
    let surface = protocol_surface(protocol_name, entry)?;
    Some(api_protocol_surface_json(&surface))
}

pub(super) fn api_protocol_surface_from_summary_json(
    summary: &ProtocolSummary,
    entry: &str,
) -> Option<String> {
    let surface = protocol_surface_from_summary(summary.clone(), entry)?;
    Some(api_protocol_surface_json(&surface))
}

pub(super) fn api_protocol_surface_for_target(name: &str) -> Option<ProtocolSurfaceSummary> {
    let mut parts = name.splitn(3, ':');
    if parts.next()? != "scan" {
        return None;
    }
    let protocol_name = parts.next()?;
    let entry = parts.next()?;
    protocol_surface(protocol_name, entry)
}

pub(super) fn api_protocol_reading_for_target_json(name: &str) -> Option<String> {
    let surface = api_protocol_surface_for_target(name)?;
    Some(api_protocol_reading_json(name, &surface))
}

pub(super) fn api_protocol_surface_json(surface: &ProtocolSurfaceSummary) -> String {
    let mut json = String::from("{\"protocol\":");
    append_json_string(&mut json, &surface.protocol);
    json.push_str(",\"entry\":");
    append_json_string(&mut json, &surface.entry);
    json.push_str(",\"default_entry\":");
    append_json_string(&mut json, &surface.default_entry);
    json.push_str(",\"selected_is_default\":");
    json.push_str(if surface.selected_is_default {
        "true"
    } else {
        "false"
    });
    json.push_str(",\"protocol_aliases\":");
    append_string_list_json(&mut json, &surface.protocol_aliases);
    json.push_str(",\"entry_aliases\":");
    append_string_list_json(&mut json, &surface.entry_aliases);
    json.push_str(",\"sibling_entries\":");
    append_string_list_json(&mut json, &surface.sibling_entries);
    json.push_str(",\"cluster_hint\":");
    append_protocol_cluster_hint_json(&mut json, surface.cluster_hint.as_ref());
    json.push_str(",\"shelf\":");
    append_protocol_shelf_json(&mut json, surface.shelf.as_ref());
    json.push_str(",\"entry_semantics\":");
    append_protocol_entry_semantics_json(&mut json, surface);
    json.push_str(",\"selected_overlay\":");
    if let Some(overlay) = surface.selected_overlay.as_ref() {
        append_json_string(&mut json, overlay);
    } else {
        json.push_str("null");
    }
    json.push_str(",\"overlays\":");
    append_protocol_overlays_json(&mut json, &surface.overlays);
    json.push_str(",\"reading_companions\":");
    append_protocol_companions_json(&mut json, &surface.overlays);
    json.push('}');
    json
}

fn api_protocol_reading_json(target_name: &str, surface: &ProtocolSurfaceSummary) -> String {
    let mut json = String::from("{\"surface\":\"target_protocol_reading\",\"target\":");
    append_json_string(&mut json, target_name);
    json.push_str(",\"protocol\":");
    append_json_string(&mut json, &surface.protocol);
    json.push_str(",\"entry\":");
    append_json_string(&mut json, &surface.entry);
    json.push_str(",\"selected_overlay\":");
    if let Some(overlay) = surface.selected_overlay.as_ref() {
        append_json_string(&mut json, overlay);
    } else {
        json.push_str("null");
    }
    json.push_str(",\"primary_surface_path\":");
    append_json_string(&mut json, &target_protocol_surface_path(target_name));
    json.push_str(",\"catalog_surface_path\":");
    append_json_string(
        &mut json,
        &protocol_entry_surface_path(&surface.protocol, &surface.entry),
    );
    json.push_str(",\"cluster_hint\":");
    append_protocol_cluster_hint_json(&mut json, surface.cluster_hint.as_ref());
    json.push_str(",\"shelf\":");
    append_protocol_shelf_json(&mut json, surface.shelf.as_ref());
    json.push_str(",\"sibling_entries\":");
    append_string_list_json(&mut json, &surface.sibling_entries);
    json.push_str(",\"read_next\":");
    append_protocol_read_next_json(&mut json, surface);
    json.push('}');
    json
}

fn append_protocol_summary_json(target: &mut String, summary: &ProtocolSummary) {
    target.push('{');
    target.push_str("\"protocol\":");
    append_json_string(target, &summary.protocol);
    target.push_str(",\"default_entry\":");
    append_json_string(target, &summary.default_entry);
    target.push_str(",\"aliases\":");
    append_string_list_json(target, &summary.aliases);
    target.push_str(",\"cluster_hint\":");
    append_protocol_cluster_hint_json(target, summary.cluster_hint.as_ref());
    target.push_str(",\"entry_count\":");
    target.push_str(&summary.entries.len().to_string());
    target.push_str(",\"entries\":[");
    for (index, entry) in summary.entries.iter().enumerate() {
        if index > 0 {
            target.push(',');
        }
        append_protocol_entry_json(target, entry);
    }
    target.push_str("],\"entry_surface_path_template\":");
    append_json_string(
        target,
        &format!(
            "/v1/protocols/{}/entries/<entry>/surface.json",
            summary.protocol
        ),
    );
    target.push('}');
}

fn append_protocol_entry_json(target: &mut String, entry: &ProtocolEntrySummary) {
    target.push('{');
    target.push_str("\"mode\":");
    append_json_string(target, &entry.mode);
    target.push_str(",\"default\":");
    target.push_str(if entry.default { "true" } else { "false" });
    target.push_str(",\"aliases\":");
    append_string_list_json(target, &entry.aliases);
    target.push('}');
}

fn append_protocol_cluster_hint_json(
    target: &mut String,
    hint: Option<&ProtocolClusterHintSummary>,
) {
    if let Some(hint) = hint {
        target.push('{');
        target.push_str("\"key\":");
        append_json_string(target, &hint.key);
        target.push_str(",\"label\":");
        append_json_string(target, &hint.label);
        target.push_str(",\"operator_hint\":");
        append_json_string(target, &hint.operator_hint);
        target.push_str(",\"sibling_protocols\":");
        append_string_list_json(target, &hint.sibling_protocols);
        target.push('}');
    } else {
        target.push_str("null");
    }
}

fn append_protocol_overlays_json(target: &mut String, overlays: &[ProtocolOverlaySummary]) {
    target.push('[');
    for (index, overlay) in overlays.iter().enumerate() {
        if index > 0 {
            target.push(',');
        }
        target.push('{');
        target.push_str("\"key\":");
        append_json_string(target, &overlay.key);
        target.push_str(",\"label\":");
        append_json_string(target, &overlay.label);
        target.push_str(",\"kind\":");
        append_json_string(target, &overlay.kind);
        target.push_str(",\"operator_hint\":");
        append_json_string(target, &overlay.operator_hint);
        target.push_str(",\"aliases\":");
        append_string_list_json(target, &overlay.aliases);
        target.push_str(",\"companion_protocol\":");
        if let Some(protocol) = overlay.companion_protocol.as_ref() {
            append_json_string(target, protocol);
        } else {
            target.push_str("null");
        }
        target.push_str(",\"companion_entry\":");
        if let Some(entry) = overlay.companion_entry.as_ref() {
            append_json_string(target, entry);
        } else {
            target.push_str("null");
        }
        target.push('}');
    }
    target.push(']');
}

fn append_protocol_companions_json(target: &mut String, overlays: &[ProtocolOverlaySummary]) {
    let companions = protocol_companion_rows(overlays);
    target.push('[');
    for (index, companion) in companions.iter().enumerate() {
        if index > 0 {
            target.push(',');
        }
        target.push('{');
        target.push_str("\"protocol\":");
        append_json_string(target, &companion.protocol);
        target.push_str(",\"entry\":");
        append_json_string(target, &companion.entry);
        target.push_str(",\"via_overlay\":");
        append_json_string(target, &companion.overlay_key);
        target.push_str(",\"via_label\":");
        append_json_string(target, &companion.overlay_label);
        target.push('}');
    }
    target.push(']');
}

fn append_protocol_read_next_json(target: &mut String, surface: &ProtocolSurfaceSummary) {
    target.push('[');
    target.push_str("{\"kind\":\"primary\",\"protocol\":");
    append_json_string(target, &surface.protocol);
    target.push_str(",\"entry\":");
    append_json_string(target, &surface.entry);
    target.push_str(",\"path\":");
    append_json_string(
        target,
        &protocol_entry_surface_path(&surface.protocol, &surface.entry),
    );
    target.push_str(",\"reason\":\"target surface\"}");
    for companion in protocol_companion_rows(&surface.overlays) {
        target.push_str(",{\"kind\":\"companion\",\"protocol\":");
        append_json_string(target, &companion.protocol);
        target.push_str(",\"entry\":");
        append_json_string(target, &companion.entry);
        target.push_str(",\"path\":");
        append_json_string(
            target,
            &protocol_entry_surface_path(&companion.protocol, &companion.entry),
        );
        target.push_str(",\"via_overlay\":");
        append_json_string(target, &companion.overlay_key);
        target.push_str(",\"via_label\":");
        append_json_string(target, &companion.overlay_label);
        target.push('}');
    }
    target.push(']');
}

fn protocol_entry_surface_path(protocol: &str, entry: &str) -> String {
    format!("/v1/protocols/{protocol}/entries/{entry}/surface.json")
}

fn target_protocol_surface_path(target_name: &str) -> String {
    format!("/v1/latest/targets/{target_name}/protocol-surface.json")
}

fn protocol_companion_rows(overlays: &[ProtocolOverlaySummary]) -> Vec<ProtocolCompanionRow> {
    let mut emitted = BTreeMap::<(String, String), (String, String)>::new();
    for overlay in overlays {
        let Some(protocol) = overlay.companion_protocol.clone() else {
            continue;
        };
        let Some(entry) = overlay.companion_entry.clone() else {
            continue;
        };
        emitted
            .entry((protocol, entry))
            .or_insert_with(|| (overlay.key.clone(), overlay.label.clone()));
    }
    emitted
        .into_iter()
        .map(
            |((protocol, entry), (overlay_key, overlay_label))| ProtocolCompanionRow {
                protocol,
                entry,
                overlay_key,
                overlay_label,
            },
        )
        .collect()
}

struct ProtocolCompanionRow {
    protocol: String,
    entry: String,
    overlay_key: String,
    overlay_label: String,
}

#[derive(Clone)]
struct ProtocolClusterCatalogItem {
    key: String,
    label: String,
    operator_hint: String,
    sibling_protocols: Vec<String>,
    protocols: Vec<ProtocolClusterProtocolItem>,
}

#[derive(Clone)]
struct ProtocolClusterProtocolItem {
    protocol: String,
    default_entry: String,
    entry_count: usize,
}

fn protocol_clusters() -> Vec<ProtocolClusterCatalogItem> {
    protocol_clusters_from_summaries(&protocol_summaries())
}

fn protocol_clusters_from_summaries(
    summaries: &[ProtocolSummary],
) -> Vec<ProtocolClusterCatalogItem> {
    let mut grouped = BTreeMap::<String, ProtocolClusterCatalogItem>::new();
    for summary in summaries {
        let Some(hint) = summary.cluster_hint.clone() else {
            continue;
        };
        let cluster =
            grouped
                .entry(hint.key.clone())
                .or_insert_with(|| ProtocolClusterCatalogItem {
                    key: hint.key.clone(),
                    label: hint.label.clone(),
                    operator_hint: hint.operator_hint.clone(),
                    sibling_protocols: hint.sibling_protocols.clone(),
                    protocols: Vec::new(),
                });
        cluster.protocols.push(ProtocolClusterProtocolItem {
            protocol: summary.protocol.clone(),
            default_entry: summary.default_entry.clone(),
            entry_count: summary.entries.len(),
        });
    }
    grouped
        .into_values()
        .map(|mut cluster| {
            cluster
                .protocols
                .sort_by(|left, right| left.protocol.cmp(&right.protocol));
            cluster
        })
        .collect()
}

fn append_protocol_cluster_json(target: &mut String, cluster: &ProtocolClusterCatalogItem) {
    target.push('{');
    target.push_str("\"key\":");
    append_json_string(target, &cluster.key);
    target.push_str(",\"label\":");
    append_json_string(target, &cluster.label);
    target.push_str(",\"operator_hint\":");
    append_json_string(target, &cluster.operator_hint);
    target.push_str(",\"sibling_protocols\":");
    append_string_list_json(target, &cluster.sibling_protocols);
    target.push_str(",\"protocol_count\":");
    target.push_str(&cluster.protocols.len().to_string());
    target.push_str(",\"protocols\":[");
    for (index, protocol) in cluster.protocols.iter().enumerate() {
        if index > 0 {
            target.push(',');
        }
        target.push('{');
        target.push_str("\"protocol\":");
        append_json_string(target, &protocol.protocol);
        target.push_str(",\"default_entry\":");
        append_json_string(target, &protocol.default_entry);
        target.push_str(",\"entry_count\":");
        target.push_str(&protocol.entry_count.to_string());
        target.push('}');
    }
    target.push_str("]}");
}

fn append_protocol_shelf_json(target: &mut String, shelf: Option<&ProtocolShelfSummary>) {
    if let Some(shelf) = shelf {
        target.push('{');
        target.push_str("\"key\":");
        append_json_string(target, &shelf.key);
        target.push_str(",\"label\":");
        append_json_string(target, &shelf.label);
        target.push_str(",\"page\":");
        append_json_string(target, &shelf.page);
        target.push_str(",\"entries\":");
        append_string_list_json(target, &shelf.entries);
        target.push('}');
    } else {
        target.push_str("null");
    }
}

fn append_protocol_entry_semantics_json(target: &mut String, surface: &ProtocolSurfaceSummary) {
    match surface.entry_semantics.as_ref() {
        Some(semantics) => {
            target.push('{');
            target.push_str("\"category\":");
            append_json_string(target, &semantics.category);
            target.push_str(",\"operator_focus\":");
            append_json_string(target, &semantics.operator_focus);
            target.push_str(",\"typical_signal\":");
            append_optional_string_json(target, semantics.typical_signal.as_deref());
            target.push_str(",\"primary_failure_mode\":");
            append_optional_string_json(target, semantics.primary_failure_mode.as_deref());
            target.push_str(",\"primary_failure_detail\":");
            append_optional_string_json(target, semantics.primary_failure_detail.as_deref());
            target.push_str(",\"primary_failure_basis\":");
            append_optional_string_json(target, semantics.primary_failure_basis.as_deref());
            target.push('}');
        }
        None => target.push_str("null"),
    }
}

fn append_optional_string_json(target: &mut String, value: Option<&str>) {
    if let Some(value) = value {
        append_json_string(target, value);
    } else {
        target.push_str("null");
    }
}

#[cfg(test)]
mod tests;
