use super::frontend_focus_text;
use super::*;

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
        "{{\"language_contract\":{},\"summary\":{{\"kind\":\"{}\",\"module_doc\":{},\"template_doc\":{},\"function_count\":{},\"merged_step_count\":{},\"focus\":{}}},\"focused_report\":{},\"report\":{}}}",
        gewylang_contract_json(GewyLangStage::ExpandedAst),
        json_escape_string(&report.kind),
        report
            .module_doc
            .as_ref()
            .map(|doc| json_string(doc))
            .unwrap_or_else(|| "null".into()),
        report
            .template_doc
            .as_ref()
            .map(|doc| json_string(doc))
            .unwrap_or_else(|| "null".into()),
        report.function_count,
        report.merged_step_count,
        focus_json,
        focused_report_json,
        frontend_json(Some(report)),
    )
}

pub(super) fn frontend_focus_json(report: &FrontendReport, focus: FrontendFocus) -> String {
    match focus {
        FrontendFocus::Functions => format!(
            "{{\"kind\":\"functions\",\"function_nodes\":[{}]}}",
            report
                .function_nodes
                .iter()
                .map(|node| format!(
                    "{{\"name\":{},\"signature\":{},\"doc\":{},\"step_count\":{},\"source_id\":{},\"package_scope\":{},\"params\":[{}]}}",
                    json_string(&node.name),
                    json_string(&node.signature),
                    node.doc
                        .as_ref()
                        .map(|doc| json_string(doc))
                        .unwrap_or_else(|| "null".into()),
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
            "{{\"language_contract\":{},\"kind\":\"{}\",\"status\":{{\"present\":true}},\"authoring\":{{\"module_doc\":{},\"template_doc\":{},\"documented_functions\":[{}]}},\"counts\":{{\"functions\":{},\"merged_steps\":{},\"includes\":{},\"use_edges\":{},\"graph_nodes\":{},\"graph_edges\":{},\"expansion_previews\":{}}},\"module_doc\":{},\"template_doc\":{},\"function_count\":{},\"function_nodes\":[{}],\"merged_step_count\":{},\"include_sources\":[{}],\"use_edges\":[{}],\"graph_nodes\":[{}],\"graph_edges\":[{}],\"expansion_previews\":[{}]}}",
            gewylang_contract_json(GewyLangStage::ExpandedAst),
            json_escape_string(&frontend.kind),
            frontend
                .module_doc
                .as_ref()
                .map(|doc| json_string(doc))
                .unwrap_or_else(|| "null".into()),
            frontend
                .template_doc
                .as_ref()
                .map(|doc| json_string(doc))
                .unwrap_or_else(|| "null".into()),
            frontend
                .function_nodes
                .iter()
                .filter_map(|node| node.doc.as_ref().map(|_| json_string(&node.name)))
                .collect::<Vec<_>>()
                .join(","),
            frontend.function_count,
            frontend.merged_step_count,
            frontend.include_sources.len(),
            frontend.use_edges.len(),
            frontend.graph_nodes.len(),
            frontend.graph_edges.len(),
            frontend.expansion_previews.len(),
            frontend
                .module_doc
                .as_ref()
                .map(|doc| json_string(doc))
                .unwrap_or_else(|| "null".into()),
            frontend
                .template_doc
                .as_ref()
                .map(|doc| json_string(doc))
                .unwrap_or_else(|| "null".into()),
            frontend.function_count,
            frontend
                .function_nodes
                .iter()
                .map(|node| format!(
                    "{{\"name\":{},\"signature\":{},\"doc\":{},\"step_count\":{},\"source_id\":{},\"package_scope\":{},\"params\":[{}]}}",
                    json_string(&node.name),
                    json_string(&node.signature),
                    node.doc
                        .as_ref()
                        .map(|doc| json_string(doc))
                        .unwrap_or_else(|| "null".into()),
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

pub(super) fn frontend_expansion_preview_json(preview: &FrontendExpansionPreviewReport) -> String {
    format!(
        "{{\"scope\":{},\"local_bindings\":[{}],\"steps\":[{}],\"use_targets\":[{}]}}",
        json_string(&preview.scope),
        string_json_list(&preview.local_bindings),
        string_json_list(&preview.steps),
        string_json_list(&preview.use_targets),
    )
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
