mod graph;
mod summary;

use super::{PipelineModule, SyntaxError as DslError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrontendDslKind {
    Pipeline,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrontendModuleSummary {
    pub kind: FrontendDslKind,
    pub module_doc: Option<String>,
    pub template_doc: Option<String>,
    pub function_count: usize,
    pub function_nodes: Vec<FrontendFunctionNode>,
    pub merged_step_count: usize,
    pub include_sources: Vec<FrontendIncludeSource>,
    pub use_edges: Vec<FrontendUseEdge>,
    pub graph_nodes: Vec<FrontendGraphNode>,
    pub graph_edges: Vec<FrontendGraphEdge>,
    pub expansion_previews: Vec<FrontendExpansionPreview>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrontendIncludeSourceKind {
    Local,
    Dependency,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrontendIncludeSource {
    pub request: String,
    pub resolved_path: String,
    pub kind: FrontendIncludeSourceKind,
    pub dependency: Option<String>,
    pub package_scope: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrontendFunctionNode {
    pub name: String,
    pub signature: String,
    pub doc: Option<String>,
    pub step_count: usize,
    pub source_id: String,
    pub package_scope: String,
    pub params: Vec<FrontendFunctionParam>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrontendFunctionParam {
    pub name: String,
    pub has_default: bool,
    pub declared_kind: Option<String>,
    pub effective_kind: Option<String>,
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
    pub label: String,
    pub package_scope: String,
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
    summary::summarize_frontend_file(path)
}

pub fn summarize_frontend_str(input: &str) -> Result<FrontendModuleSummary, DslError> {
    summary::summarize_frontend_str(input)
}

pub fn summarize_frontend_str_with_package(
    input: &str,
    package: &super::PackageContext,
) -> Result<FrontendModuleSummary, DslError> {
    summary::summarize_frontend_str_with_package(input, package)
}

pub fn summarize_pipeline_module(module: PipelineModule) -> FrontendModuleSummary {
    summary::summarize_pipeline_module(module)
}
