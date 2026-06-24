use super::{
    FrontendExpansionPreview, FrontendGraphEdge, FrontendGraphEdgeKind, FrontendGraphNode,
    FrontendGraphNodeKind, FrontendUseEdge,
};
use crate::dsl::{PipelineCall, PipelineModule, parse_pipeline_single_arg};

pub(super) fn pipeline_use_edges(module: &PipelineModule) -> Vec<FrontendUseEdge> {
    let mut edges = Vec::new();
    append_use_edges("entry", &module.body, &mut edges);
    for (function_name, function) in &module.functions {
        append_use_edges(function_name, &function.body, &mut edges);
    }
    edges
}

pub(super) fn pipeline_graph_nodes(module: &PipelineModule) -> Vec<FrontendGraphNode> {
    let mut nodes = Vec::new();
    nodes.push(FrontendGraphNode {
        id: "entry".to_string(),
        kind: FrontendGraphNodeKind::Entry,
        label: "entry".to_string(),
        package_scope: module.package_scope.clone(),
        step_count: Some(module.body.len()),
    });
    for source in &module.include_sources {
        nodes.push(FrontendGraphNode {
            id: format!("file:{}", source.resolved_path),
            kind: FrontendGraphNodeKind::File,
            label: frontend_include_label(&source.resolved_path),
            package_scope: source.package_scope.clone(),
            step_count: None,
        });
    }
    for (name, function) in &module.functions {
        nodes.push(FrontendGraphNode {
            id: format!("fn:{name}"),
            kind: FrontendGraphNodeKind::Function,
            label: name.clone(),
            package_scope: function.package_scope.clone(),
            step_count: Some(function.body.len()),
        });
    }
    nodes
}

pub(super) fn pipeline_graph_edges(module: &PipelineModule) -> Vec<FrontendGraphEdge> {
    let mut edges = module.include_edges.clone();
    for edge in &module.use_edges {
        edges.push(FrontendGraphEdge {
            from: scope_graph_id(&edge.from),
            to: format!("fn:{}", edge.to),
            kind: FrontendGraphEdgeKind::Use,
            line: edge.line,
        });
    }
    edges.sort_by(|left, right| {
        left.from
            .cmp(&right.from)
            .then(left.to.cmp(&right.to))
            .then(
                frontend_graph_edge_kind_rank(left.kind)
                    .cmp(&frontend_graph_edge_kind_rank(right.kind)),
            )
    });
    edges
}

pub(super) fn pipeline_expansion_previews(module: &PipelineModule) -> Vec<FrontendExpansionPreview> {
    let mut previews = vec![FrontendExpansionPreview {
        scope: "entry".to_string(),
        local_bindings: Vec::new(),
        steps: module.body.iter().map(pipeline_call_preview).collect(),
        use_targets: module
            .body
            .iter()
            .filter(|call| call.name == "use")
            .filter_map(|call| parse_pipeline_single_arg(&call.args, "use").ok())
            .collect(),
    }];
    previews.extend(module.functions.iter().map(|(name, function)| {
        FrontendExpansionPreview {
            scope: name.clone(),
            local_bindings: function
                .local_bindings
                .iter()
                .map(|binding| binding.name.clone())
                .collect(),
            steps: function.body.iter().map(pipeline_call_preview).collect(),
            use_targets: function
                .body
                .iter()
                .filter(|call| call.name == "use")
                .filter_map(|call| parse_pipeline_single_arg(&call.args, "use").ok())
                .collect(),
        }
    }));
    previews
}

fn append_use_edges(scope: &str, calls: &[PipelineCall], output: &mut Vec<FrontendUseEdge>) {
    for call in calls {
        if call.name == "use" {
            if let Ok(target) = parse_pipeline_single_arg(&call.args, "use") {
                output.push(FrontendUseEdge {
                    from: scope.to_string(),
                    to: target,
                    line: call.line_no,
                });
            }
        }
    }
}

fn pipeline_call_preview(call: &PipelineCall) -> String {
    if call.args.is_empty() {
        call.name.clone()
    } else {
        format!("{}({})", call.name, call.args.join(", "))
    }
}

fn scope_graph_id(scope: &str) -> String {
    if scope == "entry" {
        "entry".to_string()
    } else {
        format!("fn:{scope}")
    }
}

fn frontend_graph_edge_kind_rank(kind: FrontendGraphEdgeKind) -> u8 {
    match kind {
        FrontendGraphEdgeKind::Include => 0,
        FrontendGraphEdgeKind::Use => 1,
    }
}

fn frontend_include_label(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(path)
        .to_string()
}
