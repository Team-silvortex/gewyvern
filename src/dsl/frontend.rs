use super::{
    DslError, PackageContext, PipelineCall, PipelineModule, looks_like_pipeline_dsl,
    parse_pipeline_module, parse_pipeline_single_arg,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrontendDslKind {
    Pipeline,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrontendModuleSummary {
    pub kind: FrontendDslKind,
    pub function_count: usize,
    pub function_nodes: Vec<FrontendFunctionNode>,
    pub merged_step_count: usize,
    pub include_sources: Vec<String>,
    pub use_edges: Vec<FrontendUseEdge>,
    pub graph_nodes: Vec<FrontendGraphNode>,
    pub graph_edges: Vec<FrontendGraphEdge>,
    pub expansion_previews: Vec<FrontendExpansionPreview>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrontendFunctionNode {
    pub name: String,
    pub step_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrontendUseEdge {
    pub from: String,
    pub to: String,
    pub line: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrontendGraphNodeKind {
    Entry,
    File,
    Function,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrontendGraphNode {
    pub id: String,
    pub kind: FrontendGraphNodeKind,
    pub step_count: Option<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrontendGraphEdgeKind {
    Include,
    Use,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrontendGraphEdge {
    pub from: String,
    pub to: String,
    pub kind: FrontendGraphEdgeKind,
    pub line: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrontendExpansionPreview {
    pub scope: String,
    pub local_bindings: Vec<String>,
    pub steps: Vec<String>,
    pub use_targets: Vec<String>,
}

pub fn summarize_frontend_file(path: &str) -> Result<FrontendModuleSummary, DslError> {
    let package = super::package::resolve_package_context(path)?;
    let resolved = package.entry_file.clone();
    let input = super::read_file(&resolved)?;
    summarize_frontend_str_with_base(&input, Some(&package))
}

pub fn summarize_frontend_str(input: &str) -> Result<FrontendModuleSummary, DslError> {
    summarize_frontend_str_with_base(input, None)
}

fn summarize_frontend_str_with_base(
    input: &str,
    package: Option<&PackageContext>,
) -> Result<FrontendModuleSummary, DslError> {
    if looks_like_pipeline_dsl(input) {
        let module = parse_pipeline_module(input, package, true)?;
        let function_nodes = module
            .functions
            .iter()
            .map(|(name, function)| FrontendFunctionNode {
                name: name.clone(),
                step_count: function.body.len(),
            })
            .collect();
        let merged_step_count = module.body.len()
            + module
                .functions
                .values()
                .map(|function| function.body.len())
                .sum::<usize>();
        let use_edges = pipeline_use_edges(&module);
        let graph_nodes = pipeline_graph_nodes(&module);
        let graph_edges = pipeline_graph_edges(&module);
        let expansion_previews = pipeline_expansion_previews(&module);
        return Ok(FrontendModuleSummary {
            kind: FrontendDslKind::Pipeline,
            function_count: module.functions.len(),
            function_nodes,
            merged_step_count,
            include_sources: module.include_sources,
            use_edges,
            graph_nodes,
            graph_edges,
            expansion_previews,
        });
    }
    Err(DslError::InvalidValue(
        "gewylang now only supports the pipeline stable subset".into(),
    ))
}

fn pipeline_use_edges(module: &PipelineModule) -> Vec<FrontendUseEdge> {
    let mut edges = Vec::new();
    append_use_edges("entry", &module.body, &mut edges);
    for (function_name, function) in &module.functions {
        append_use_edges(function_name, &function.body, &mut edges);
    }
    edges
}

pub(super) fn append_use_edges(
    scope: &str,
    calls: &[PipelineCall],
    output: &mut Vec<FrontendUseEdge>,
) {
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

fn pipeline_graph_nodes(module: &PipelineModule) -> Vec<FrontendGraphNode> {
    let mut nodes = Vec::new();
    nodes.push(FrontendGraphNode {
        id: "entry".to_string(),
        kind: FrontendGraphNodeKind::Entry,
        step_count: Some(module.body.len()),
    });
    for source in &module.include_sources {
        nodes.push(FrontendGraphNode {
            id: format!("file:{source}"),
            kind: FrontendGraphNodeKind::File,
            step_count: None,
        });
    }
    for (name, function) in &module.functions {
        nodes.push(FrontendGraphNode {
            id: format!("fn:{name}"),
            kind: FrontendGraphNodeKind::Function,
            step_count: Some(function.body.len()),
        });
    }
    nodes
}

fn pipeline_graph_edges(module: &PipelineModule) -> Vec<FrontendGraphEdge> {
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

fn pipeline_expansion_previews(module: &PipelineModule) -> Vec<FrontendExpansionPreview> {
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

fn pipeline_call_preview(call: &PipelineCall) -> String {
    if call.args.is_empty() {
        call.name.clone()
    } else {
        format!("{}({})", call.name, call.args.join(", "))
    }
}

pub(super) fn scope_graph_id(scope: &str) -> String {
    if scope == "entry" {
        "entry".to_string()
    } else {
        format!("fn:{scope}")
    }
}

pub(super) fn frontend_graph_edge_kind_rank(kind: FrontendGraphEdgeKind) -> u8 {
    match kind {
        FrontendGraphEdgeKind::Include => 0,
        FrontendGraphEdgeKind::Use => 1,
    }
}
