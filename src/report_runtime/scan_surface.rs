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

pub(super) fn estimate_protocol_surface_json_capacity(
    surface: Option<&ProtocolSurfaceSummary>,
) -> usize {
    let Some(surface) = surface else {
        return 24;
    };
    let overlays_capacity = surface.overlays.iter().fold(0usize, |acc, overlay| {
        acc + overlay.key.len()
            + overlay.label.len()
            + overlay.companion_protocol.as_deref().map_or(0, str::len)
            + overlay.companion_entry.as_deref().map_or(0, str::len)
            + 48
    });
    256 + surface.protocol.len()
        + surface.entry.len()
        + surface.default_entry.len()
        + surface
            .protocol_aliases
            .iter()
            .map(String::len)
            .sum::<usize>()
        + surface.entry_aliases.iter().map(String::len).sum::<usize>()
        + surface
            .sibling_entries
            .iter()
            .map(String::len)
            .sum::<usize>()
        + surface.selected_overlay.as_deref().map_or(0, str::len)
        + surface.entry_semantics.as_ref().map_or(0, |semantics| {
            semantics.category.len()
                + semantics.operator_focus.len()
                + semantics.typical_signal.as_deref().map_or(0, str::len)
                + semantics
                    .primary_failure_mode
                    .as_deref()
                    .map_or(0, str::len)
                + semantics
                    .primary_failure_detail
                    .as_deref()
                    .map_or(0, str::len)
                + semantics
                    .primary_failure_basis
                    .as_deref()
                    .map_or(0, str::len)
                + 80
        })
        + surface.cluster_hint.as_ref().map_or(0, |hint| {
            hint.key.len()
                + hint.label.len()
                + hint.operator_hint.len()
                + hint
                    .sibling_protocols
                    .iter()
                    .map(String::len)
                    .sum::<usize>()
                + 64
        })
        + surface.shelf.as_ref().map_or(0, |shelf| {
            shelf.key.len()
                + shelf.label.len()
                + shelf.page.len()
                + shelf.entries.iter().map(String::len).sum::<usize>()
                + 64
        })
        + overlays_capacity
}

pub(super) fn estimate_protocol_surface_text_capacity(
    surface: Option<&ProtocolSurfaceSummary>,
) -> usize {
    let Some(surface) = surface else {
        return 20;
    };
    320 + surface.protocol.len()
        + surface.entry.len()
        + surface.default_entry.len()
        + surface.protocol_aliases.len() * 16
        + surface.entry_aliases.len() * 16
        + surface.sibling_entries.len() * 16
        + surface.overlays.len() * 32
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
        json.push_str(",\"entry_semantics\":");
        append_entry_semantics_json(json, surface);
        json.push_str(",\"reading_companions\":");
        append_companions_json(json, surface);
        json.push_str(",\"shelf\":");
        append_shelf_json(json, surface);
        json.push('}');
    } else {
        json.push_str("null");
    }
}

pub(super) fn append_protocol_surface_text(
    text: &mut String,
    surface: Option<&ProtocolSurfaceSummary>,
) {
    match surface {
        Some(surface) => {
            use std::fmt::Write;

            let _ = write!(
                text,
                "protocol_surface={} entry={} default={} selected_default={}",
                surface.protocol, surface.entry, surface.default_entry, surface.selected_is_default
            );
            text.push_str(" protocol_aliases=");
            append_join_or_none(text, &surface.protocol_aliases, " | ");
            text.push_str(" entry_aliases=");
            append_join_or_none(text, &surface.entry_aliases, " | ");
            text.push_str(" sibling_entries=");
            append_join_or_none(text, &surface.sibling_entries, " | ");
            text.push_str(" selected_overlay=");
            text.push_str(surface.selected_overlay.as_deref().unwrap_or("none"));
            text.push_str(" entry_semantics=");
            append_entry_semantics_text(text, surface);
            text.push_str(" reading_companions=");
            append_companions_text(text, surface);
            text.push_str(" cluster_hint=");
            append_cluster_hint_text(text, surface);
            text.push_str(" shelf=");
            append_shelf_text(text, surface);
        }
        None => text.push_str("protocol_surface=none"),
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
            let entry_semantics = entry_semantics_html(surface);
            format!(
                "<h3>Protocol Surface</h3><ul><li><strong>protocol:</strong> {}</li><li><strong>entry:</strong> {}</li><li><strong>default entry:</strong> {}{}</li><li><strong>protocol aliases:</strong> {}</li><li><strong>entry aliases:</strong> {}</li><li><strong>sibling entries:</strong> {}</li>{}{}{}{}{}</ul>",
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
                entry_semantics,
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

fn append_entry_semantics_json(json: &mut String, surface: &ProtocolSurfaceSummary) {
    match surface.entry_semantics.as_ref() {
        Some(semantics) => {
            json.push('{');
            json.push_str("\"category\":");
            append_json_string(json, &semantics.category);
            json.push_str(",\"operator_focus\":");
            append_json_string(json, &semantics.operator_focus);
            json.push_str(",\"typical_signal\":");
            append_optional_string_json(json, semantics.typical_signal.as_deref());
            json.push_str(",\"primary_failure_mode\":");
            append_optional_string_json(json, semantics.primary_failure_mode.as_deref());
            json.push_str(",\"primary_failure_detail\":");
            append_optional_string_json(json, semantics.primary_failure_detail.as_deref());
            json.push_str(",\"primary_failure_basis\":");
            append_optional_string_json(json, semantics.primary_failure_basis.as_deref());
            json.push('}');
        }
        None => json.push_str("null"),
    }
}

fn append_optional_string_json(json: &mut String, value: Option<&str>) {
    if let Some(value) = value {
        append_json_string(json, value);
    } else {
        json.push_str("null");
    }
}

fn append_companions_json(json: &mut String, surface: &ProtocolSurfaceSummary) {
    json.push('[');
    let mut seen = Vec::<(&str, &str)>::new();
    let mut index = 0usize;
    for overlay in &surface.overlays {
        let Some(protocol) = overlay.companion_protocol.as_deref() else {
            continue;
        };
        let Some(entry) = overlay.companion_entry.as_deref() else {
            continue;
        };
        if seen
            .iter()
            .any(|(seen_protocol, seen_entry)| *seen_protocol == protocol && *seen_entry == entry)
        {
            continue;
        }
        seen.push((protocol, entry));
        if index > 0 {
            json.push(',');
        }
        json.push('{');
        json.push_str("\"protocol\":");
        append_json_string(json, protocol);
        json.push_str(",\"entry\":");
        append_json_string(json, entry);
        json.push_str(",\"via_overlay\":");
        append_json_string(json, &overlay.key);
        json.push_str(",\"via_label\":");
        append_json_string(json, &overlay.label);
        json.push('}');
        index += 1;
    }
    json.push(']');
}

fn append_companions_text(text: &mut String, surface: &ProtocolSurfaceSummary) {
    let mut seen = Vec::<(&str, &str)>::new();
    let mut wrote_any = false;
    for overlay in &surface.overlays {
        let Some(protocol) = overlay.companion_protocol.as_deref() else {
            continue;
        };
        let Some(entry) = overlay.companion_entry.as_deref() else {
            continue;
        };
        if seen
            .iter()
            .any(|(seen_protocol, seen_entry)| *seen_protocol == protocol && *seen_entry == entry)
        {
            continue;
        }
        seen.push((protocol, entry));
        if wrote_any {
            text.push_str(" | ");
        }
        text.push_str(protocol);
        text.push(':');
        text.push_str(entry);
        text.push('@');
        text.push_str(&overlay.key);
        wrote_any = true;
    }
    if !wrote_any {
        text.push_str("none");
    }
}

fn append_entry_semantics_text(text: &mut String, surface: &ProtocolSurfaceSummary) {
    if let Some(semantics) = surface.entry_semantics.as_ref() {
        text.push_str(&semantics.category);
        text.push(':');
        text.push_str(&semantics.operator_focus);
        text.push(':');
        text.push_str(semantics.typical_signal.as_deref().unwrap_or("none"));
        text.push(':');
        text.push_str(semantics.primary_failure_mode.as_deref().unwrap_or("none"));
        text.push(':');
        text.push_str(
            semantics
                .primary_failure_detail
                .as_deref()
                .unwrap_or("none"),
        );
        text.push(':');
        text.push_str(semantics.primary_failure_basis.as_deref().unwrap_or("none"));
    } else {
        text.push_str("none");
    }
}

fn companions_html(surface: &ProtocolSurfaceSummary) -> String {
    let mut rendered = String::new();
    let mut seen = Vec::<(&str, &str)>::new();
    let mut wrote_any = false;
    for overlay in &surface.overlays {
        let Some(protocol) = overlay.companion_protocol.as_deref() else {
            continue;
        };
        let Some(entry) = overlay.companion_entry.as_deref() else {
            continue;
        };
        if seen
            .iter()
            .any(|(seen_protocol, seen_entry)| *seen_protocol == protocol && *seen_entry == entry)
        {
            continue;
        }
        seen.push((protocol, entry));
        if wrote_any {
            rendered.push_str(" | ");
        }
        rendered.push_str(&html_escape(protocol));
        rendered.push(':');
        rendered.push_str(&html_escape(entry));
        rendered.push_str(" via ");
        rendered.push_str(&html_escape(&overlay.key));
        rendered.push_str(" (");
        rendered.push_str(&html_escape(&overlay.label));
        rendered.push(')');
        wrote_any = true;
    }
    if !wrote_any {
        return "<li><strong>reading companions:</strong> none</li>".to_string();
    }
    format!("<li><strong>reading companions:</strong> {}</li>", rendered)
}

fn entry_semantics_html(surface: &ProtocolSurfaceSummary) -> String {
    surface.entry_semantics.as_ref().map_or_else(String::new, |semantics| {
        format!(
            "<li><strong>entry semantics:</strong> {}</li><li><strong>operator focus:</strong> {}</li><li><strong>typical signal:</strong> {}</li><li><strong>primary failure mode:</strong> {}</li><li><strong>primary failure detail:</strong> {}</li><li><strong>primary failure basis:</strong> {}</li>",
            html_escape(&semantics.category),
            html_escape(&semantics.operator_focus),
            html_escape(semantics.typical_signal.as_deref().unwrap_or("none")),
            html_escape(
                semantics
                    .primary_failure_mode
                    .as_deref()
                    .unwrap_or("none"),
            ),
            html_escape(
                semantics
                    .primary_failure_detail
                    .as_deref()
                    .unwrap_or("none"),
            ),
            html_escape(
                semantics
                    .primary_failure_basis
                    .as_deref()
                    .unwrap_or("none"),
            ),
        )
    })
}

fn append_join_or_none(text: &mut String, items: &[String], separator: &str) {
    if items.is_empty() {
        text.push_str("none");
    } else {
        for (index, item) in items.iter().enumerate() {
            if index > 0 {
                text.push_str(separator);
            }
            text.push_str(item);
        }
    }
}

fn append_cluster_hint_text(text: &mut String, surface: &ProtocolSurfaceSummary) {
    if let Some(hint) = surface.cluster_hint.as_ref() {
        text.push_str(&hint.key);
        text.push(':');
        text.push_str(&hint.label);
        text.push(':');
        text.push_str(&hint.operator_hint);
        text.push(':');
        append_join_or_none(text, &hint.sibling_protocols, " | ");
    } else {
        text.push_str("none");
    }
}

fn append_shelf_text(text: &mut String, surface: &ProtocolSurfaceSummary) {
    if let Some(shelf) = surface.shelf.as_ref() {
        text.push_str(&shelf.key);
        text.push(':');
        text.push_str(&shelf.label);
        text.push(':');
        text.push_str(&shelf.page);
        text.push(':');
        append_join_or_none(text, &shelf.entries, " | ");
    } else {
        text.push_str("none");
    }
}
