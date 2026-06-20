use crate::render_utils::{append_json_string, append_string_list_json};
use gewyvern::protocol_profiles::{
    ProtocolEntrySummary, ProtocolShelfSummary, ProtocolSummary, ProtocolSurfaceSummary,
    protocol_summaries, protocol_summary, protocol_surface,
};

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
        assert!(body.contains("\"entry_surface_path_template\":\"/v1/protocols/mysql/entries/<entry>/surface.json\""));
    }

    #[test]
    fn protocol_surface_by_name_includes_redis_shelf_context() {
        let body = api_protocol_surface_by_name_json("redis", "zadd")
            .expect("redis zadd surface should exist");
        assert!(body.contains("\"protocol\":\"redis\""));
        assert!(body.contains("\"entry\":\"zadd\""));
        assert!(body.contains("\"selected_is_default\":false"));
        assert!(body.contains("\"key\":\"sorted-set\""));
    }

    #[test]
    fn scan_target_name_resolves_protocol_surface() {
        let surface = api_protocol_surface_for_target("scan:http:request")
            .expect("scan target should resolve protocol surface");
        assert_eq!(surface.protocol, "http");
        assert_eq!(surface.entry, "request");
    }
}
