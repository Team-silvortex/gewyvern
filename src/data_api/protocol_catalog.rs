use crate::render_utils::{append_json_string, append_string_list_json};
use gewyvern::protocol_profiles::{
    ProtocolClusterHintSummary, ProtocolEntrySummary, ProtocolOverlaySummary, ProtocolShelfSummary,
    ProtocolSummary, ProtocolSurfaceSummary, protocol_summaries, protocol_summary,
    protocol_surface,
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
    let cluster = protocol_clusters()
        .into_iter()
        .find(|cluster| cluster.key == key)?;
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

pub(super) fn api_protocol_surface_by_name_json(
    protocol_name: &str,
    entry: &str,
) -> Option<String> {
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
    target.push('[');
    for (index, ((protocol, entry), (overlay_key, overlay_label))) in
        emitted.into_iter().enumerate()
    {
        if index > 0 {
            target.push(',');
        }
        target.push('{');
        target.push_str("\"protocol\":");
        append_json_string(target, &protocol);
        target.push_str(",\"entry\":");
        append_json_string(target, &entry);
        target.push_str(",\"via_overlay\":");
        append_json_string(target, &overlay_key);
        target.push_str(",\"via_label\":");
        append_json_string(target, &overlay_label);
        target.push('}');
    }
    target.push(']');
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
mod tests {
    use super::*;

    #[test]
    fn protocol_catalog_lists_mysql_summary_and_entry_path_template() {
        let body = api_protocol_catalog_json();
        assert!(body.contains("\"surface\":\"protocol_catalog\""));
        assert!(body.contains("\"protocol\":\"mysql\""));
        assert!(body.contains("\"default_entry\":\"session\""));
        assert!(body.contains("\"cluster_hint\":{"));
        assert!(body.contains(
            "\"entry_surface_path_template\":\"/v1/protocols/mysql/entries/<entry>/surface.json\""
        ));
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
        assert!(body.contains("\"entry_semantics\":null"));
    }

    #[test]
    fn protocol_surface_by_name_includes_redis_failure_entry_semantics() {
        let body = api_protocol_surface_by_name_json("redis", "clusterdown")
            .expect("redis clusterdown surface should exist");
        assert!(body.contains("\"entry\":\"clusterdown\""));
        assert!(body.contains("\"entry_semantics\":{"));
        assert!(body.contains("\"category\":\"failure-path\""));
        assert!(body.contains("\"typical_signal\":\"-CLUSTERDOWN\""));
        assert!(body.contains("\"primary_failure_mode\":\"semantic_error\""));
        assert!(body.contains("\"primary_failure_detail\":\"protocol_error\""));
        assert!(body.contains("\"primary_failure_basis\":\"direct_protocol_signal\""));
    }

    #[test]
    fn protocol_surface_by_name_includes_socks5_denied_entry_semantics() {
        let body = api_protocol_surface_by_name_json("socks5", "auth-connect-denied")
            .expect("socks5 auth-connect-denied surface should exist");
        assert!(body.contains("\"entry\":\"auth-connect-denied\""));
        assert!(body.contains("\"entry_semantics\":{"));
        assert!(body.contains("\"category\":\"failure-path\""));
        assert!(body.contains(
            "\"operator_focus\":\"upstream connect refusal after authenticated proxy setup\""
        ));
        assert!(body.contains("\"typical_signal\":null"));
        assert!(body.contains("\"primary_failure_mode\":\"server_denied\""));
        assert!(body.contains("\"primary_failure_detail\":\"access_denied\""));
        assert!(body.contains("\"primary_failure_basis\":\"direct_protocol_signal\""));
    }

    #[test]
    fn protocol_surface_by_name_includes_http_connect_denied_entry_semantics() {
        let body = api_protocol_surface_by_name_json("http", "denied")
            .expect("http denied surface should exist");
        assert!(body.contains("\"entry\":\"denied\""));
        assert!(body.contains("\"entry_semantics\":{"));
        assert!(body.contains("\"category\":\"failure-path\""));
        assert!(
            body.contains("\"operator_focus\":\"proxy tunnel refusal after CONNECT policy evaluation\"")
        );
        assert!(body.contains("\"typical_signal\":\"403\""));
        assert!(body.contains("\"primary_failure_mode\":\"server_denied\""));
        assert!(body.contains("\"primary_failure_detail\":\"access_denied\""));
        assert!(body.contains("\"primary_failure_basis\":\"direct_protocol_signal\""));
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
        assert_eq!(dot.selected_overlay.as_deref(), Some("dot"));
        assert!(dot.overlays.iter().any(|overlay| overlay.key == "dot"));

        let doh = api_protocol_surface_for_target("scan:doh:request")
            .expect("doh target alias should resolve");
        assert_eq!(doh.protocol, "http");
        assert_eq!(doh.entry, "request");
        assert_eq!(doh.selected_overlay.as_deref(), Some("doh"));
        assert!(doh.overlays.iter().any(|overlay| overlay.key == "doh"));
    }

    #[test]
    fn protocol_surface_json_emits_overlay_metadata_for_dot_and_doh() {
        let dot = api_protocol_surface_by_name_json("dot", "tcp")
            .expect("dot overlay should render as dns/tcp surface");
        assert!(dot.contains("\"protocol\":\"dns\""));
        assert!(dot.contains("\"entry\":\"tcp\""));
        assert!(dot.contains("\"selected_overlay\":\"dot\""));
        assert!(dot.contains("\"key\":\"dot\""));
        assert!(dot.contains("\"kind\":\"encrypted_resolver_overlay\""));

        let doh = api_protocol_surface_by_name_json("doh", "request")
            .expect("doh overlay should render as http/request surface");
        assert!(doh.contains("\"protocol\":\"http\""));
        assert!(doh.contains("\"entry\":\"request\""));
        assert!(doh.contains("\"selected_overlay\":\"doh\""));
        assert!(doh.contains("\"key\":\"doh\""));
        assert!(doh.contains("\"kind\":\"resolver_payload_overlay\""));
    }

    #[test]
    fn protocol_surface_json_emits_http_connect_overlay_metadata() {
        let connect = api_protocol_surface_by_name_json("http-connect", "connect")
            .expect("http connect alias should render as http/connect surface");
        assert!(connect.contains("\"protocol\":\"http\""));
        assert!(connect.contains("\"entry\":\"connect\""));
        assert!(connect.contains("\"selected_overlay\":\"http-connect\""));
        assert!(connect.contains("\"key\":\"http-connect\""));
        assert!(connect.contains("\"kind\":\"proxy_tunnel_overlay\""));

        let denied = api_protocol_surface_by_name_json("http-connect-denied", "denied")
            .expect("http connect denied alias should render as denied tunnel surface");
        assert!(denied.contains("\"protocol\":\"http\""));
        assert!(denied.contains("\"entry\":\"denied\""));
        assert!(denied.contains("\"selected_overlay\":\"http-connect\""));
        assert!(denied.contains("\"key\":\"http-connect\""));
    }

    #[test]
    fn protocol_surface_json_emits_starttls_overlay_metadata_on_mail_auth_surfaces() {
        let smtp = api_protocol_surface_by_name_json("smtp", "auth")
            .expect("smtp auth surface should exist");
        assert!(smtp.contains("\"protocol\":\"smtp\""));
        assert!(smtp.contains("\"entry\":\"auth\""));
        assert!(smtp.contains("\"selected_overlay\":null"));
        assert!(smtp.contains("\"key\":\"starttls\""));
        assert!(smtp.contains("\"kind\":\"tls_upgrade_overlay\""));

        let smtp_denied = api_protocol_surface_by_name_json("smtp", "auth-denied")
            .expect("smtp denied auth surface should exist");
        assert!(smtp_denied.contains("\"protocol\":\"smtp\""));
        assert!(smtp_denied.contains("\"entry\":\"auth-denied\""));
        assert!(smtp_denied.contains("\"key\":\"starttls\""));

        let imap = api_protocol_surface_by_name_json("imap", "auth-denied")
            .expect("imap denied auth surface should exist");
        assert!(imap.contains("\"protocol\":\"imap\""));
        assert!(imap.contains("\"entry\":\"auth-denied\""));
        assert!(imap.contains("\"key\":\"starttls\""));

        let pop3 = api_protocol_surface_by_name_json("pop3", "auth")
            .expect("pop3 auth surface should exist");
        assert!(pop3.contains("\"protocol\":\"pop3\""));
        assert!(pop3.contains("\"entry\":\"auth\""));
        assert!(pop3.contains("\"key\":\"starttls\""));
    }

    #[test]
    fn protocol_surface_json_emits_https_and_tls_companion_overlay_metadata() {
        let https = api_protocol_surface_by_name_json("https", "connect")
            .expect("https connect surface should exist");
        assert!(https.contains("\"protocol\":\"https\""));
        assert!(https.contains("\"entry\":\"connect\""));
        assert!(https.contains("\"key\":\"https\""));
        assert!(https.contains("\"kind\":\"tls_application_overlay\""));
        assert!(https.contains("\"companion_protocol\":\"tls\""));
        assert!(https.contains("\"companion_entry\":\"client\""));
        assert!(https.contains("\"reading_companions\":[{\"protocol\":\"tls\",\"entry\":\"client\",\"via_overlay\":\"https\""));

        let tls = api_protocol_surface_by_name_json("tls", "client")
            .expect("tls client surface should exist");
        assert!(tls.contains("\"protocol\":\"tls\""));
        assert!(tls.contains("\"entry\":\"client\""));
        assert!(tls.contains("\"key\":\"https\""));
        assert!(tls.contains("\"companion_protocol\":\"https\""));
        assert!(tls.contains("\"companion_entry\":\"connect\""));
        assert!(tls.contains("\"key\":\"dot\""));
        assert!(tls.contains("\"reading_companions\":["));
    }

    #[test]
    fn protocol_surface_json_emits_http3_and_quic_companion_overlay_metadata() {
        let http3 = api_protocol_surface_by_name_json("http3", "request")
            .expect("http3 request surface should exist");
        assert!(http3.contains("\"protocol\":\"http3\""));
        assert!(http3.contains("\"entry\":\"request\""));
        assert!(http3.contains("\"key\":\"http3\""));
        assert!(http3.contains("\"kind\":\"quic_application_overlay\""));
        assert!(http3.contains("\"companion_protocol\":\"quic\""));
        assert!(http3.contains("\"companion_entry\":\"initial\""));
        assert!(http3.contains("\"reading_companions\":[{\"protocol\":\"quic\",\"entry\":\"initial\",\"via_overlay\":\"http3\""));

        let quic = api_protocol_surface_by_name_json("quic", "crypto")
            .expect("quic crypto surface should exist");
        assert!(quic.contains("\"protocol\":\"quic\""));
        assert!(quic.contains("\"entry\":\"crypto\""));
        assert!(quic.contains("\"key\":\"http3\""));
        assert!(quic.contains("\"companion_protocol\":\"http3\""));
        assert!(quic.contains("\"companion_entry\":\"request\""));
        assert!(quic.contains("\"reading_companions\":[{\"protocol\":\"http3\",\"entry\":\"request\",\"via_overlay\":\"http3\""));
    }
}
