use super::*;
use crate::dsl::{
    FrontendDslKind, FrontendExpansionPreview, FrontendFunctionNode, FrontendGraphEdge,
    FrontendGraphEdgeKind, FrontendGraphNode, FrontendGraphNodeKind, FrontendModuleSummary,
    FrontendUseEdge,
};

pub(super) fn frontend_report(summary: FrontendModuleSummary) -> FrontendReport {
    FrontendReport {
        kind: frontend_kind_text(summary.kind).to_string(),
        function_count: summary.function_count,
        function_nodes: summary
            .function_nodes
            .into_iter()
            .map(frontend_function_report)
            .collect(),
        merged_step_count: summary.merged_step_count,
        include_sources: summary.include_sources,
        use_edges: summary
            .use_edges
            .into_iter()
            .map(frontend_use_edge_report)
            .collect(),
        graph_nodes: summary
            .graph_nodes
            .into_iter()
            .map(frontend_graph_node_report)
            .collect(),
        graph_edges: summary
            .graph_edges
            .into_iter()
            .map(frontend_graph_edge_report)
            .collect(),
        expansion_previews: summary
            .expansion_previews
            .into_iter()
            .map(frontend_expansion_preview_report)
            .collect(),
    }
}

pub(super) fn frontend_function_report(node: FrontendFunctionNode) -> FrontendFunctionReport {
    FrontendFunctionReport {
        name: node.name,
        step_count: node.step_count,
    }
}

pub(super) fn frontend_use_edge_report(edge: FrontendUseEdge) -> FrontendUseEdgeReport {
    FrontendUseEdgeReport {
        from: edge.from,
        to: edge.to,
        line: edge.line,
    }
}

pub(super) fn frontend_graph_node_report(node: FrontendGraphNode) -> FrontendGraphNodeReport {
    FrontendGraphNodeReport {
        id: node.id,
        kind: frontend_graph_node_kind_text(node.kind).to_string(),
        step_count: node.step_count,
    }
}

pub(super) fn frontend_graph_edge_report(edge: FrontendGraphEdge) -> FrontendGraphEdgeReport {
    FrontendGraphEdgeReport {
        from: edge.from,
        to: edge.to,
        kind: frontend_graph_edge_kind_text(edge.kind).to_string(),
        line: edge.line,
    }
}

pub(super) fn frontend_expansion_preview_report(
    preview: FrontendExpansionPreview,
) -> FrontendExpansionPreviewReport {
    FrontendExpansionPreviewReport {
        scope: preview.scope,
        local_bindings: preview.local_bindings,
        steps: preview.steps,
        use_targets: preview.use_targets,
    }
}

pub(super) fn frontend_kind_text(kind: FrontendDslKind) -> &'static str {
    match kind {
        FrontendDslKind::Pipeline => "pipeline",
    }
}

pub(super) fn frontend_graph_node_kind_text(kind: FrontendGraphNodeKind) -> &'static str {
    match kind {
        FrontendGraphNodeKind::Entry => "entry",
        FrontendGraphNodeKind::File => "file",
        FrontendGraphNodeKind::Function => "function",
    }
}

pub(super) fn frontend_graph_edge_kind_text(kind: FrontendGraphEdgeKind) -> &'static str {
    match kind {
        FrontendGraphEdgeKind::Include => "include",
        FrontendGraphEdgeKind::Use => "use",
    }
}

pub(super) fn frontend_text(frontend: Option<&FrontendReport>) -> String {
    match frontend {
        Some(frontend) => format!(
            "kind={} functions={} function_nodes={} merged_steps={} include_sources={} use_edges={} expansions={} graph_nodes={} graph_edges={}",
            frontend.kind,
            frontend.function_count,
            frontend
                .function_nodes
                .iter()
                .map(|node| format!("{}:{}", node.name, node.step_count))
                .collect::<Vec<_>>()
                .join(","),
            frontend.merged_step_count,
            frontend.include_sources.join(","),
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
                    Some(step_count) => format!("{}:{}:{}", node.id, node.kind, step_count),
                    None => format!("{}:{}", node.id, node.kind),
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
                .map(|source| format!("- {source}")),
        );
    }

    if report.function_nodes.is_empty() {
        lines.push("function_nodes=none".into());
    } else {
        lines.push("function_nodes:".into());
        lines.extend(
            report
                .function_nodes
                .iter()
                .map(|node| format!("- {} (steps={})", node.name, node.step_count)),
        );
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
            Some(step_count) => format!("- {} [{}] steps={}", node.id, node.kind, step_count),
            None => format!("- {} [{}]", node.id, node.kind),
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
                        .map(|node| format!("{}:{}", node.name, node.step_count))
                        .collect::<Vec<_>>()
                        .join(",")
                }
            )),
            FrontendFocus::Includes => lines.push(format!(
                "include_sources={}",
                if report.include_sources.is_empty() {
                    "none".into()
                } else {
                    report.include_sources.join(",")
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
                lines.extend(
                    report
                        .function_nodes
                        .iter()
                        .map(|node| format!("- {} (steps={})", node.name, node.step_count)),
                );
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
                        .map(|source| format!("- {source}")),
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
                        format!("- {} [{}] steps={}", node.id, node.kind, step_count)
                    }
                    None => format!("- {} [{}]", node.id, node.kind),
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
                    "{{\"name\":{},\"step_count\":{}}}",
                    json_string(&node.name),
                    node.step_count
                ))
                .collect::<Vec<_>>()
                .join(",")
        ),
        FrontendFocus::Includes => format!(
            "{{\"kind\":\"includes\",\"include_sources\":[{}]}}",
            string_json_list(&report.include_sources)
        ),
        FrontendFocus::Graph => format!(
            "{{\"kind\":\"graph\",\"graph_nodes\":[{}],\"graph_edges\":[{}]}}",
            report
                .graph_nodes
                .iter()
                .map(|node| match node.step_count {
                    Some(step_count) => format!(
                        "{{\"id\":{},\"kind\":{},\"step_count\":{}}}",
                        json_string(&node.id),
                        json_string(&node.kind),
                        step_count
                    ),
                    None => format!(
                        "{{\"id\":{},\"kind\":{},\"step_count\":null}}",
                        json_string(&node.id),
                        json_string(&node.kind)
                    ),
                })
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
                    "{{\"name\":{},\"step_count\":{}}}",
                    json_string(&node.name),
                    node.step_count
                ))
                .collect::<Vec<_>>()
                .join(","),
            frontend.merged_step_count,
            string_json_list(&frontend.include_sources),
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
                .map(|node| match node.step_count {
                    Some(step_count) => format!(
                        "{{\"id\":{},\"kind\":{},\"step_count\":{}}}",
                        json_string(&node.id),
                        json_string(&node.kind),
                        step_count
                    ),
                    None => format!(
                        "{{\"id\":{},\"kind\":{},\"step_count\":null}}",
                        json_string(&node.id),
                        json_string(&node.kind)
                    ),
                })
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
