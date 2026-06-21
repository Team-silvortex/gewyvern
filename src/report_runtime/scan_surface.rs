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
        json.push_str(",\"selected_overlay\":");
        if let Some(overlay) = surface.selected_overlay.as_ref() {
            append_json_string(json, overlay);
        } else {
            json.push_str("null");
        }
        json.push_str(",\"reading_companions\":");
        append_companions_json(json, surface);
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
            let companions = companions_text(surface);
            format!(
                "protocol_surface={} entry={} default={} selected_default={} protocol_aliases={} entry_aliases={} sibling_entries={} selected_overlay={} reading_companions={} cluster_hint={} shelf={}",
                surface.protocol,
                surface.entry,
                surface.default_entry,
                surface.selected_is_default,
                join_or_none(&surface.protocol_aliases),
                join_or_none(&surface.entry_aliases),
                join_or_none(&surface.sibling_entries),
                surface.selected_overlay.as_deref().unwrap_or("none"),
                companions,
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
            let selected_overlay = surface.selected_overlay.as_ref().map_or_else(
                || "<li><strong>selected overlay:</strong> none</li>".to_string(),
                |overlay| {
                    format!(
                        "<li><strong>selected overlay:</strong> {}</li>",
                        html_escape(overlay)
                    )
                },
            );
            let companions = companions_html(surface);
            format!(
                "<h3>Protocol Surface</h3><ul><li><strong>protocol:</strong> {}</li><li><strong>entry:</strong> {}</li><li><strong>default entry:</strong> {}{}</li><li><strong>protocol aliases:</strong> {}</li><li><strong>entry aliases:</strong> {}</li><li><strong>sibling entries:</strong> {}</li>{}{}{}{}</ul>",
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
                selected_overlay,
                companions,
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

fn append_companions_json(json: &mut String, surface: &ProtocolSurfaceSummary) {
    let companions = reading_companions(surface);
    json.push('[');
    for (index, (protocol, entry, overlay_key, overlay_label)) in companions.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        json.push('{');
        json.push_str("\"protocol\":");
        append_json_string(json, protocol);
        json.push_str(",\"entry\":");
        append_json_string(json, entry);
        json.push_str(",\"via_overlay\":");
        append_json_string(json, overlay_key);
        json.push_str(",\"via_label\":");
        append_json_string(json, overlay_label);
        json.push('}');
    }
    json.push(']');
}

fn companions_text(surface: &ProtocolSurfaceSummary) -> String {
    let companions = reading_companions(surface);
    if companions.is_empty() {
        return "none".to_string();
    }
    companions
        .into_iter()
        .map(|(protocol, entry, overlay_key, _)| format!("{protocol}:{entry}@{overlay_key}"))
        .collect::<Vec<_>>()
        .join(" | ")
}

fn companions_html(surface: &ProtocolSurfaceSummary) -> String {
    let companions = reading_companions(surface);
    if companions.is_empty() {
        return "<li><strong>reading companions:</strong> none</li>".to_string();
    }
    let rendered = companions
        .into_iter()
        .map(|(protocol, entry, overlay_key, overlay_label)| {
            format!(
                "{}:{} via {} ({})",
                html_escape(&protocol),
                html_escape(&entry),
                html_escape(&overlay_key),
                html_escape(&overlay_label),
            )
        })
        .collect::<Vec<_>>()
        .join(" | ");
    format!("<li><strong>reading companions:</strong> {}</li>", rendered)
}

fn reading_companions(surface: &ProtocolSurfaceSummary) -> Vec<(String, String, String, String)> {
    let mut companions = Vec::new();
    for overlay in &surface.overlays {
        let Some(protocol) = overlay.companion_protocol.clone() else {
            continue;
        };
        let Some(entry) = overlay.companion_entry.clone() else {
            continue;
        };
        if companions
            .iter()
            .any(|item: &(String, String, String, String)| item.0 == protocol && item.1 == entry)
        {
            continue;
        }
        companions.push((protocol, entry, overlay.key.clone(), overlay.label.clone()));
    }
    companions
}
