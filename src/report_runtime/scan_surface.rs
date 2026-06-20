use gewyvern::protocol_profiles::{ProtocolSurfaceSummary, protocol_surface};

use crate::render_utils::append_json_string;

use super::*;

pub(super) fn protocol_surface_for_target(target: &str) -> Option<ProtocolSurfaceSummary> {
    let mut parts = target.splitn(3, ':');
    if parts.next()? != "scan" {
        return None;
    }
    let protocol = parts.next()?;
    let entry = parts.next()?;
    protocol_surface(protocol, entry)
}

pub(super) fn append_protocol_surface_json(
    json: &mut String,
    surface: Option<&ProtocolSurfaceSummary>,
) {
    json.push_str(",\"protocol_surface\":");
    if let Some(surface) = surface {
        json.push('{');
        json.push_str("\"protocol\":");
        append_json_string(json, &surface.protocol);
        json.push_str(",\"entry\":");
        append_json_string(json, &surface.entry);
        json.push_str(",\"default_entry\":");
        append_json_string(json, &surface.default_entry);
        json.push_str(",\"selected_is_default\":");
        json.push_str(if surface.selected_is_default {
            "true"
        } else {
            "false"
        });
        json.push_str(",\"protocol_aliases\":");
        append_string_array_json(json, &surface.protocol_aliases);
        json.push_str(",\"entry_aliases\":");
        append_string_array_json(json, &surface.entry_aliases);
        json.push_str(",\"sibling_entries\":");
        append_string_array_json(json, &surface.sibling_entries);
        json.push_str(",\"shelf\":");
        append_shelf_json(json, surface);
        json.push('}');
    } else {
        json.push_str("null");
    }
}

pub(super) fn protocol_surface_text(surface: Option<&ProtocolSurfaceSummary>) -> String {
    match surface {
        Some(surface) => {
            let shelf = surface.shelf.as_ref().map_or_else(
                || "none".to_string(),
                |shelf| {
                    format!(
                        "{}:{}:{}:{}",
                        shelf.key,
                        shelf.label,
                        shelf.page,
                        join_or_none(&shelf.entries)
                    )
                },
            );
            let cluster_hint = surface.cluster_hint.as_ref().map_or_else(
                || "none".to_string(),
                |hint| {
                    format!(
                        "{}:{}:{}:{}",
                        hint.key,
                        hint.label,
                        hint.operator_hint,
                        join_or_none(&hint.sibling_protocols)
                    )
                },
            );
            format!(
                "protocol_surface={} entry={} default={} selected_default={} protocol_aliases={} entry_aliases={} sibling_entries={} cluster_hint={} shelf={}",
                surface.protocol,
                surface.entry,
                surface.default_entry,
                surface.selected_is_default,
                join_or_none(&surface.protocol_aliases),
                join_or_none(&surface.entry_aliases),
                join_or_none(&surface.sibling_entries),
                cluster_hint,
                shelf,
            )
        }
        None => "protocol_surface=none".to_string(),
    }
}

pub(super) fn protocol_surface_html(surface: Option<&ProtocolSurfaceSummary>) -> String {
    match surface {
        Some(surface) => {
            let shelf = surface.shelf.as_ref().map_or_else(
                || "<li><strong>shelf:</strong> none</li>".to_string(),
                |shelf| {
                    format!(
                        "<li><strong>shelf:</strong> {} ({})</li><li><strong>shelf page:</strong> {}</li><li><strong>shelf entries:</strong> {}</li>",
                        html_escape(&shelf.label),
                        html_escape(&shelf.key),
                        html_escape(&shelf.page),
                        html_escape(&join_or_none(&shelf.entries)),
                    )
                },
            );
            let cluster_hint = surface.cluster_hint.as_ref().map_or_else(
                || "<li><strong>cluster hint:</strong> none</li>".to_string(),
                |hint| {
                    format!(
                        "<li><strong>cluster hint:</strong> {} ({})</li><li><strong>operator hint:</strong> {}</li><li><strong>cluster siblings:</strong> {}</li>",
                        html_escape(&hint.label),
                        html_escape(&hint.key),
                        html_escape(&hint.operator_hint),
                        html_escape(&join_or_none(&hint.sibling_protocols)),
                    )
                },
            );
            format!(
                "<h3>Protocol Surface</h3><ul><li><strong>protocol:</strong> {}</li><li><strong>entry:</strong> {}</li><li><strong>default entry:</strong> {}{}</li><li><strong>protocol aliases:</strong> {}</li><li><strong>entry aliases:</strong> {}</li><li><strong>sibling entries:</strong> {}</li>{}{}</ul>",
                html_escape(&surface.protocol),
                html_escape(&surface.entry),
                html_escape(&surface.default_entry),
                if surface.selected_is_default {
                    " (selected)"
                } else {
                    ""
                },
                html_escape(&join_or_none(&surface.protocol_aliases)),
                html_escape(&join_or_none(&surface.entry_aliases)),
                html_escape(&join_or_none(&surface.sibling_entries)),
                cluster_hint,
                shelf,
            )
        }
        None => "<h3>Protocol Surface</h3><ul><li>none</li></ul>".to_string(),
    }
}

fn join_or_none(items: &[String]) -> String {
    if items.is_empty() {
        "none".to_string()
    } else {
        items.join(" | ")
    }
}

fn append_string_array_json(json: &mut String, items: &[String]) {
    json.push('[');
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        append_json_string(json, item);
    }
    json.push(']');
}

fn append_shelf_json(json: &mut String, surface: &ProtocolSurfaceSummary) {
    match surface.shelf.as_ref() {
        Some(shelf) => {
            json.push('{');
            json.push_str("\"key\":");
            append_json_string(json, &shelf.key);
            json.push_str(",\"label\":");
            append_json_string(json, &shelf.label);
            json.push_str(",\"page\":");
            append_json_string(json, &shelf.page);
            json.push_str(",\"entries\":");
            append_string_array_json(json, &shelf.entries);
            json.push('}');
        }
        None => json.push_str("null"),
    }
}
