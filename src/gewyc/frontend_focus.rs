use super::*;

pub(super) fn frontend_text(frontend: Option<&FrontendReport>) -> String {
    match frontend {
        Some(frontend) => format!(
            "kind={} functions={} function_nodes={} merged_steps={} include_sources={} use_edges={} expansions={} graph_nodes={} graph_edges={}",
            frontend.kind,
            frontend.function_count,
            frontend
                .function_nodes
                .iter()
                .map(|node| format!(
                    "{}:{}@{}#{}",
                    node.name, node.step_count, node.source_id, node.package_scope
                ))
                .collect::<Vec<_>>()
                .join(","),
            frontend.merged_step_count,
            frontend
                .include_sources
                .iter()
                .map(frontend_include_source_summary)
                .collect::<Vec<_>>()
                .join(","),
            frontend
                .use_edges
                .iter()
                .map(|edge| format!("{}->{}@{}", edge.from, edge.to, edge.line))
                .collect::<Vec<_>>()
                .join(","),
            frontend
                .expansion_previews
                .iter()
                .map(|preview| format!("{}:{}", preview.scope, preview.steps.join(" -> ")))
                .collect::<Vec<_>>()
                .join(","),
            frontend
                .graph_nodes
                .iter()
                .map(|node| match node.step_count {
                    Some(step_count) => {
                        format!(
                            "{}:{}:{}:{}:{}",
                            node.id, node.kind, node.label, node.package_scope, step_count
                        )
                    }
                    None => format!(
                        "{}:{}:{}:{}",
                        node.id, node.kind, node.label, node.package_scope
                    ),
                })
                .collect::<Vec<_>>()
                .join(","),
            frontend
                .graph_edges
                .iter()
                .map(|edge| format!("{}-{}->{}@{}", edge.from, edge.kind, edge.to, edge.line))
                .collect::<Vec<_>>()
                .join(",")
        ),
        None => "none".into(),
    }
}

pub(super) fn frontend_report_text(
    report: &FrontendReport,
    focus: Option<FrontendFocus>,
) -> String {
    let mut lines = vec![
        format!("kind={}", report.kind),
        format!("function_count={}", report.function_count),
        format!("merged_step_count={}", report.merged_step_count),
    ];

    if let Some(focus) = focus {
        lines.push(format!("focus={}", frontend_focus_text(focus)));
        lines.extend(frontend_focus_text_lines(report, focus));
        return lines.join("\n");
    }

    if report.include_sources.is_empty() {
        lines.push("include_sources=none".into());
    } else {
        lines.push("include_sources:".into());
        lines.extend(
            report
                .include_sources
                .iter()
                .map(frontend_include_source_text),
        );
    }

    if report.function_nodes.is_empty() {
        lines.push("function_nodes=none".into());
    } else {
        lines.push("function_nodes:".into());
        lines.extend(report.function_nodes.iter().map(|node| {
            format!(
                "- {} (steps={}, source={}, package={})",
                node.name, node.step_count, node.source_id, node.package_scope
            )
        }));
    }

    if report.use_edges.is_empty() {
        lines.push("use_edges=none".into());
    } else {
        lines.push("use_edges:".into());
        lines.extend(
            report
                .use_edges
                .iter()
                .map(|edge| format!("- {} -> {} @ line {}", edge.from, edge.to, edge.line)),
        );
    }

    if report.expansion_previews.is_empty() {
        lines.push("expansion_previews=none".into());
    } else {
        lines.push("expansion_previews:".into());
        lines.extend(
            report
                .expansion_previews
                .iter()
                .map(frontend_expansion_preview_text),
        );
    }

    if report.graph_nodes.is_empty() {
        lines.push("graph_nodes=none".into());
    } else {
        lines.push("graph_nodes:".into());
        lines.extend(report.graph_nodes.iter().map(|node| match node.step_count {
            Some(step_count) => format!(
                "- {} [{}] label={} package={} steps={}",
                node.id, node.kind, node.label, node.package_scope, step_count
            ),
            None => format!(
                "- {} [{}] label={} package={}",
                node.id, node.kind, node.label, node.package_scope
            ),
        }));
    }

    if report.graph_edges.is_empty() {
        lines.push("graph_edges=none".into());
    } else {
        lines.push("graph_edges:".into());
        lines.extend(report.graph_edges.iter().map(|edge| {
            format!(
                "- {} -{}-> {} @ line {}",
                edge.from, edge.kind, edge.to, edge.line
            )
        }));
    }

    lines.join("\n")
}

pub(super) fn frontend_report_text_compact(
    report: &FrontendReport,
    focus: Option<FrontendFocus>,
) -> String {
    let mut lines = vec![format!(
        "kind={} function_count={} merged_step_count={}",
        report.kind, report.function_count, report.merged_step_count
    )];
    if let Some(focus) = focus {
        lines.push(format!("focus={}", frontend_focus_text(focus)));
        match focus {
            FrontendFocus::Functions => lines.push(format!(
                "functions={}",
                if report.function_nodes.is_empty() {
                    "none".into()
                } else {
                    report
                        .function_nodes
                        .iter()
                        .map(|node| {
                            let notes = frontend_function_param_summary(&node.params);
                            format!(
                                "{}:{}@{}#{}{}",
                                node.signature,
                                node.step_count,
                                node.source_id,
                                node.package_scope,
                                notes
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(",")
                }
            )),
            FrontendFocus::Includes => lines.push(format!(
                "include_sources={}",
                if report.include_sources.is_empty() {
                    "none".into()
                } else {
                    report
                        .include_sources
                        .iter()
                        .map(frontend_include_source_summary)
                        .collect::<Vec<_>>()
                        .join(",")
                }
            )),
            FrontendFocus::Graph => lines.push(format!(
                "graph_nodes={} graph_edges={}",
                report.graph_nodes.len(),
                report.graph_edges.len()
            )),
            FrontendFocus::Expansion => lines.push(format!(
                "expansion_previews={}",
                if report.expansion_previews.is_empty() {
                    "none".into()
                } else {
                    report
                        .expansion_previews
                        .iter()
                        .map(|preview| format!("{}:{}", preview.scope, preview.steps.len()))
                        .collect::<Vec<_>>()
                        .join(",")
                }
            )),
        }
        return lines.join("\n");
    }
    lines.push(format!(
        "includes={} use_edges={} expansion_previews={} graph_nodes={} graph_edges={}",
        report.include_sources.len(),
        report.use_edges.len(),
        report.expansion_previews.len(),
        report.graph_nodes.len(),
        report.graph_edges.len()
    ));
    lines.join("\n")
}

pub(super) fn frontend_report_json(
    report: &FrontendReport,
    focus: Option<FrontendFocus>,
) -> String {
    let focus_json = focus
        .map(|focus| json_string(frontend_focus_text(focus)))
        .unwrap_or_else(|| "null".into());
    let focused_report_json = focus
        .map(|focus| frontend_focus_json(report, focus))
        .unwrap_or_else(|| "null".into());
    format!(
        "{{\"summary\":{{\"kind\":\"{}\",\"function_count\":{},\"merged_step_count\":{},\"focus\":{}}},\"focused_report\":{},\"report\":{}}}",
        json_escape_string(&report.kind),
        report.function_count,
        report.merged_step_count,
        focus_json,
        focused_report_json,
        frontend_json(Some(report)),
    )
}

pub(super) fn frontend_focus_text(focus: FrontendFocus) -> &'static str {
    match focus {
        FrontendFocus::Functions => "functions",
        FrontendFocus::Includes => "includes",
        FrontendFocus::Graph => "graph",
        FrontendFocus::Expansion => "expansion",
    }
}

pub(super) fn frontend_focus_text_lines(
    report: &FrontendReport,
    focus: FrontendFocus,
) -> Vec<String> {
    match focus {
        FrontendFocus::Functions => {
            let mut lines = vec!["function_nodes:".into()];
            if report.function_nodes.is_empty() {
                lines.push("- none".into());
            } else {
                lines.extend(report.function_nodes.iter().map(|node| {
                    let notes = frontend_function_param_text(&node.params);
                    format!(
                        "- {} (steps={}, source={}, package={}{}{})",
                        node.signature,
                        node.step_count,
                        node.source_id,
                        node.package_scope,
                        if notes.is_empty() { "" } else { ", " },
                        notes
                    )
                }));
            }
            lines
        }
        FrontendFocus::Includes => {
            let mut lines = vec!["include_sources:".into()];
            if report.include_sources.is_empty() {
                lines.push("- none".into());
            } else {
                lines.extend(
                    report
                        .include_sources
                        .iter()
                        .map(frontend_include_source_text),
                );
            }
            lines
        }
        FrontendFocus::Graph => {
            let mut lines = vec!["graph_nodes:".into()];
            if report.graph_nodes.is_empty() {
                lines.push("- none".into());
            } else {
                lines.extend(report.graph_nodes.iter().map(|node| match node.step_count {
                    Some(step_count) => {
                        format!(
                            "- {} [{}] label={} package={} steps={}",
                            node.id, node.kind, node.label, node.package_scope, step_count
                        )
                    }
                    None => format!(
                        "- {} [{}] label={} package={}",
                        node.id, node.kind, node.label, node.package_scope
                    ),
                }));
            }
            lines.push("graph_edges:".into());
            if report.graph_edges.is_empty() {
                lines.push("- none".into());
            } else {
                lines.extend(report.graph_edges.iter().map(|edge| {
                    format!(
                        "- {} -{}-> {} @ line {}",
                        edge.from, edge.kind, edge.to, edge.line
                    )
                }));
            }
            lines
        }
        FrontendFocus::Expansion => {
            let mut lines = vec!["expansion_previews:".into()];
            if report.expansion_previews.is_empty() {
                lines.push("- none".into());
            } else {
                lines.extend(
                    report
                        .expansion_previews
                        .iter()
                        .map(frontend_expansion_preview_text),
                );
            }
            lines
        }
    }
}

pub(super) fn frontend_focus_json(report: &FrontendReport, focus: FrontendFocus) -> String {
    match focus {
        FrontendFocus::Functions => format!(
            "{{\"kind\":\"functions\",\"function_nodes\":[{}]}}",
            report
                .function_nodes
                .iter()
                .map(|node| format!(
                    "{{\"name\":{},\"signature\":{},\"step_count\":{},\"source_id\":{},\"package_scope\":{},\"params\":[{}]}}",
                    json_string(&node.name),
                    json_string(&node.signature),
                    node.step_count,
                    json_string(&node.source_id),
                    json_string(&node.package_scope),
                    frontend_function_params_json(&node.params)
                ))
                .collect::<Vec<_>>()
                .join(",")
        ),
        FrontendFocus::Includes => format!(
            "{{\"kind\":\"includes\",\"include_sources\":[{}]}}",
            report
                .include_sources
                .iter()
                .map(frontend_include_source_json)
                .collect::<Vec<_>>()
                .join(",")
        ),
        FrontendFocus::Graph => format!(
            "{{\"kind\":\"graph\",\"graph_nodes\":[{}],\"graph_edges\":[{}]}}",
            report
                .graph_nodes
                .iter()
                .map(frontend_graph_node_json)
                .collect::<Vec<_>>()
                .join(","),
            report
                .graph_edges
                .iter()
                .map(|edge| format!(
                    "{{\"from\":{},\"to\":{},\"kind\":{},\"line\":{}}}",
                    json_string(&edge.from),
                    json_string(&edge.to),
                    json_string(&edge.kind),
                    edge.line
                ))
                .collect::<Vec<_>>()
                .join(",")
        ),
        FrontendFocus::Expansion => format!(
            "{{\"kind\":\"expansion\",\"expansion_previews\":[{}]}}",
            report
                .expansion_previews
                .iter()
                .map(frontend_expansion_preview_json)
                .collect::<Vec<_>>()
                .join(",")
        ),
    }
}

pub(super) fn frontend_json(frontend: Option<&FrontendReport>) -> String {
    match frontend {
        Some(frontend) => format!(
            "{{\"kind\":\"{}\",\"function_count\":{},\"function_nodes\":[{}],\"merged_step_count\":{},\"include_sources\":[{}],\"use_edges\":[{}],\"graph_nodes\":[{}],\"graph_edges\":[{}],\"expansion_previews\":[{}]}}",
            json_escape_string(&frontend.kind),
            frontend.function_count,
            frontend
                .function_nodes
                .iter()
                .map(|node| format!(
                    "{{\"name\":{},\"signature\":{},\"step_count\":{},\"source_id\":{},\"package_scope\":{},\"params\":[{}]}}",
                    json_string(&node.name),
                    json_string(&node.signature),
                    node.step_count,
                    json_string(&node.source_id),
                    json_string(&node.package_scope),
                    frontend_function_params_json(&node.params)
                ))
                .collect::<Vec<_>>()
                .join(","),
            frontend.merged_step_count,
            frontend
                .include_sources
                .iter()
                .map(frontend_include_source_json)
                .collect::<Vec<_>>()
                .join(","),
            frontend
                .use_edges
                .iter()
                .map(|edge| format!(
                    "{{\"from\":{},\"to\":{},\"line\":{}}}",
                    json_string(&edge.from),
                    json_string(&edge.to),
                    edge.line
                ))
                .collect::<Vec<_>>()
                .join(","),
            frontend
                .graph_nodes
                .iter()
                .map(frontend_graph_node_json)
                .collect::<Vec<_>>()
                .join(","),
            frontend
                .graph_edges
                .iter()
                .map(|edge| format!(
                    "{{\"from\":{},\"to\":{},\"kind\":{},\"line\":{}}}",
                    json_string(&edge.from),
                    json_string(&edge.to),
                    json_string(&edge.kind),
                    edge.line
                ))
                .collect::<Vec<_>>()
                .join(","),
            frontend
                .expansion_previews
                .iter()
                .map(frontend_expansion_preview_json)
                .collect::<Vec<_>>()
                .join(",")
        ),
        None => "null".into(),
    }
}

pub(super) fn frontend_expansion_preview_text(preview: &FrontendExpansionPreviewReport) -> String {
    let bindings = if preview.local_bindings.is_empty() {
        "none".to_string()
    } else {
        preview.local_bindings.join(", ")
    };
    let uses = if preview.use_targets.is_empty() {
        "none".to_string()
    } else {
        preview.use_targets.join(", ")
    };
    let steps = if preview.steps.is_empty() {
        "none".to_string()
    } else {
        preview.steps.join(" -> ")
    };
    format!(
        "- {} bindings=[{}] uses=[{}] steps=[{}]",
        preview.scope, bindings, uses, steps
    )
}

pub(super) fn frontend_expansion_preview_json(preview: &FrontendExpansionPreviewReport) -> String {
    format!(
        "{{\"scope\":{},\"local_bindings\":[{}],\"steps\":[{}],\"use_targets\":[{}]}}",
        json_string(&preview.scope),
        string_json_list(&preview.local_bindings),
        string_json_list(&preview.steps),
        string_json_list(&preview.use_targets),
    )
}

fn frontend_function_param_summary(params: &[FrontendFunctionParamReport]) -> String {
    let notes = params
        .iter()
        .filter_map(frontend_function_param_note)
        .collect::<Vec<_>>();
    if notes.is_empty() {
        return String::new();
    }
    format!(" {{notes: {}}}", notes.join(", "))
}

fn frontend_function_param_text(params: &[FrontendFunctionParamReport]) -> String {
    let notes = params
        .iter()
        .filter_map(frontend_function_param_note)
        .collect::<Vec<_>>();
    if notes.is_empty() {
        return String::new();
    }
    format!("param_notes: {}", notes.join(", "))
}

fn frontend_function_param_note(param: &FrontendFunctionParamReport) -> Option<String> {
    let mut details = Vec::new();
    match (&param.declared_kind, &param.effective_kind) {
        (Some(declared), Some(effective)) if declared != effective => {
            details.push(format!("declared {declared} -> {effective}"));
        }
        (None, Some(effective)) => {
            details.push(format!("inferred {effective}"));
        }
        _ => {}
    }
    if details.is_empty() {
        None
    } else {
        Some(format!("{} <{}>", param.name, details.join(", ")))
    }
}

fn frontend_function_params_json(params: &[FrontendFunctionParamReport]) -> String {
    params
        .iter()
        .map(|param| {
            format!(
                "{{\"name\":{},\"has_default\":{},\"declared_kind\":{},\"effective_kind\":{}}}",
                json_string(&param.name),
                if param.has_default { "true" } else { "false" },
                param
                    .declared_kind
                    .as_ref()
                    .map(|kind| json_string(kind))
                    .unwrap_or_else(|| "null".to_string()),
                param
                    .effective_kind
                    .as_ref()
                    .map(|kind| json_string(kind))
                    .unwrap_or_else(|| "null".to_string())
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn frontend_include_source_summary(source: &FrontendIncludeSourceReport) -> String {
    match &source.dependency {
        Some(dependency) => format!(
            "{}:{}=>{}#{}",
            dependency, source.request, source.resolved_path, source.package_scope
        ),
        None => format!(
            "{}=>{}#{}",
            source.request, source.resolved_path, source.package_scope
        ),
    }
}

fn frontend_include_source_text(source: &FrontendIncludeSourceReport) -> String {
    match &source.dependency {
        Some(dependency) => format!(
            "- {} [{} dependency={} package={}] -> {}",
            source.request, source.kind, dependency, source.package_scope, source.resolved_path
        ),
        None => format!(
            "- {} [{} package={}] -> {}",
            source.request, source.kind, source.package_scope, source.resolved_path
        ),
    }
}

fn frontend_include_source_json(source: &FrontendIncludeSourceReport) -> String {
    format!(
        "{{\"request\":{},\"resolved_path\":{},\"kind\":{},\"dependency\":{},\"package_scope\":{}}}",
        json_string(&source.request),
        json_string(&source.resolved_path),
        json_string(&source.kind),
        source
            .dependency
            .as_ref()
            .map(|dependency| json_string(dependency))
            .unwrap_or_else(|| "null".to_string()),
        json_string(&source.package_scope)
    )
}

fn frontend_graph_node_json(node: &FrontendGraphNodeReport) -> String {
    match node.step_count {
        Some(step_count) => format!(
            "{{\"id\":{},\"kind\":{},\"label\":{},\"package_scope\":{},\"step_count\":{}}}",
            json_string(&node.id),
            json_string(&node.kind),
            json_string(&node.label),
            json_string(&node.package_scope),
            step_count
        ),
        None => format!(
            "{{\"id\":{},\"kind\":{},\"label\":{},\"package_scope\":{},\"step_count\":null}}",
            json_string(&node.id),
            json_string(&node.kind),
            json_string(&node.label),
            json_string(&node.package_scope)
        ),
    }
}
