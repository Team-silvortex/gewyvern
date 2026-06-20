use crate::render_utils::{append_json_string, append_string_list_json};
use gewyvern::protocol_profiles::{
    ProtocolClusterHintSummary, ProtocolEntrySummary, ProtocolShelfSummary, ProtocolSummary,
    ProtocolSurfaceSummary,
    protocol_summaries, protocol_summary, protocol_surface,
};
use std::collections::BTreeMap;

pub(super) fn api_protocol_catalog_json() -> String {
    let summaries = protocol_summaries();
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
    let cluster = protocol_clusters().into_iter().find(|cluster| cluster.key == key)?;
    let mut json = String::with_capacity(1024);
    append_protocol_cluster_json(&mut json, &cluster);
    Some(json)
}

pub(super) fn api_protocol_summary_json(protocol_name: &str) -> Option<String> {
    let summary = protocol_summary(protocol_name)?;
    let mut json = String::with_capacity(1024);
    append_protocol_summary_json(&mut json, &summary);
    Some(json)
}

pub(super) fn api_protocol_surface_by_name_json(protocol_name: &str, entry: &str) -> Option<String> {
    let surface = protocol_surface(protocol_name, entry)?;
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
        &format!("/v1/protocols/{}/entries/<entry>/surface.json", summary.protocol),
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
    let mut grouped = BTreeMap::<String, ProtocolClusterCatalogItem>::new();
    for summary in protocol_summaries() {
        let Some(hint) = summary.cluster_hint.clone() else {
            continue;
        };
        let cluster = grouped
            .entry(hint.key.clone())
            .or_insert_with(|| ProtocolClusterCatalogItem {
                key: hint.key.clone(),
                label: hint.label.clone(),
                operator_hint: hint.operator_hint.clone(),
                sibling_protocols: hint.sibling_protocols.clone(),
                protocols: Vec::new(),
            });
        cluster.protocols.push(ProtocolClusterProtocolItem {
            protocol: summary.protocol,
            default_entry: summary.default_entry,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_catalog_lists_mysql_summary_and_entry_path_template() {
        let body = api_protocol_catalog_json();
        assert!(body.contains("\"surface\":\"protocol_catalog\""));
        assert!(body.contains("\"protocol\":\"mysql\""));
        assert!(body.contains("\"default_entry\":\"session\""));
        assert!(body.contains("\"cluster_hint\":{"));
        assert!(body.contains("\"entry_surface_path_template\":\"/v1/protocols/mysql/entries/<entry>/surface.json\""));
    }

    #[test]
    fn protocol_surface_by_name_includes_redis_shelf_context() {
        let body = api_protocol_surface_by_name_json("redis", "zadd")
            .expect("redis zadd surface should exist");
        assert!(body.contains("\"protocol\":\"redis\""));
        assert!(body.contains("\"entry\":\"zadd\""));
        assert!(body.contains("\"selected_is_default\":false"));
        assert!(body.contains("\"cluster_hint\":{"));
        assert!(body.contains("\"key\":\"cache-queue-stream\""));
        assert!(body.contains("\"key\":\"sorted-set\""));
    }

    #[test]
    fn protocol_clusters_catalog_groups_cache_queue_families() {
        let body = api_protocol_clusters_json();
        assert!(body.contains("\"surface\":\"protocol_cluster_catalog\""));
        assert!(body.contains("\"key\":\"cache-queue-stream\""));
        assert!(body.contains("\"protocol\":\"redis\""));
        assert!(body.contains("\"protocol\":\"mqtt\""));
    }

    #[test]
    fn protocol_cluster_view_returns_identity_access_cluster() {
        let body = api_protocol_cluster_json("identity-directory-access")
            .expect("identity cluster should exist");
        assert!(body.contains("\"key\":\"identity-directory-access\""));
        assert!(body.contains("\"protocol\":\"ldap\""));
        assert!(body.contains("\"protocol\":\"ssh\""));
    }

    #[test]
    fn scan_target_name_resolves_protocol_surface() {
        let surface = api_protocol_surface_for_target("scan:http:request")
            .expect("scan target should resolve protocol surface");
        assert_eq!(surface.protocol, "http");
        assert_eq!(surface.entry, "request");
    }

    #[test]
    fn protocol_surface_by_alias_resolves_dot_and_doh_targets() {
        let dot = api_protocol_surface_for_target("scan:dot:tcp")
            .expect("dot target alias should resolve");
        assert_eq!(dot.protocol, "dns");
        assert_eq!(dot.entry, "tcp");

        let doh = api_protocol_surface_for_target("scan:doh:request")
            .expect("doh target alias should resolve");
        assert_eq!(doh.protocol, "http");
        assert_eq!(doh.entry, "request");
    }
}
