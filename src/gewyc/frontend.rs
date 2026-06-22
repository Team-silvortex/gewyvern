use super::*;
use crate::dsl::{
    FrontendDslKind, FrontendExpansionPreview, FrontendFunctionNode, FrontendFunctionParam,
    FrontendGraphEdge, FrontendGraphEdgeKind, FrontendGraphNode, FrontendGraphNodeKind,
    FrontendIncludeSource, FrontendIncludeSourceKind, FrontendModuleSummary, FrontendUseEdge,
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
        include_sources: summary
            .include_sources
            .into_iter()
            .map(frontend_include_source_report)
            .collect(),
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

pub(super) fn frontend_include_source_report(
    source: FrontendIncludeSource,
) -> FrontendIncludeSourceReport {
    FrontendIncludeSourceReport {
        request: source.request,
        resolved_path: source.resolved_path,
        kind: frontend_include_source_kind_text(source.kind).to_string(),
        dependency: source.dependency,
        package_scope: source.package_scope,
    }
}

pub(super) fn frontend_function_report(node: FrontendFunctionNode) -> FrontendFunctionReport {
    FrontendFunctionReport {
        name: node.name,
        signature: node.signature,
        step_count: node.step_count,
        source_id: node.source_id,
        package_scope: node.package_scope,
        params: node
            .params
            .into_iter()
            .map(frontend_function_param_report)
            .collect(),
    }
}

pub(super) fn frontend_function_param_report(
    param: FrontendFunctionParam,
) -> FrontendFunctionParamReport {
    FrontendFunctionParamReport {
        name: param.name,
        has_default: param.has_default,
        declared_kind: param.declared_kind,
        effective_kind: param.effective_kind,
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
        label: node.label,
        package_scope: node.package_scope,
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

pub(super) fn frontend_include_source_kind_text(kind: FrontendIncludeSourceKind) -> &'static str {
    match kind {
        FrontendIncludeSourceKind::Local => "local",
        FrontendIncludeSourceKind::Dependency => "dependency",
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
