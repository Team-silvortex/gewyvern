use super::graph::{pipeline_expansion_previews, pipeline_graph_edges, pipeline_graph_nodes};
use super::{FrontendDslKind, FrontendFunctionNode, FrontendFunctionParam, FrontendModuleSummary};
use crate::dsl::entry::{looks_like_pipeline_dsl, resolve_include_entry_alias};
use crate::dsl::function_types::{format_pipeline_function_signature, pipeline_value_kind_text};
use crate::dsl::{DslError, PackageContext, PipelineModule, parse_pipeline_module};

pub(super) fn summarize_frontend_file(path: &str) -> Result<FrontendModuleSummary, DslError> {
    let package = crate::dsl::package::resolve_package_context(path)?;
    let resolved = package.entry_file.clone();
    let input = crate::dsl::read_file(&resolved)?;
    summarize_frontend_str_with_base(&input, Some(&package))
}

pub(super) fn summarize_frontend_str(input: &str) -> Result<FrontendModuleSummary, DslError> {
    summarize_frontend_str_with_base(input, None)
}

pub(crate) fn summarize_frontend_str_with_package(
    input: &str,
    package: &PackageContext,
) -> Result<FrontendModuleSummary, DslError> {
    summarize_frontend_str_with_base(input, Some(package))
}

fn summarize_frontend_str_with_base(
    input: &str,
    package: Option<&PackageContext>,
) -> Result<FrontendModuleSummary, DslError> {
    if looks_like_pipeline_dsl(input) {
        let module = parse_pipeline_module(input, package, true)?;
        return Ok(summarize_pipeline_module(module));
    }
    if let Some((include_input, include_package)) = resolve_include_entry_alias(input, package)? {
        return summarize_frontend_str_with_base(&include_input, Some(&include_package));
    }
    Err(DslError::InvalidValue(
        "gewylang now only supports the pipeline stable subset".into(),
    ))
}

pub(super) fn summarize_pipeline_module(module: PipelineModule) -> FrontendModuleSummary {
    let function_nodes = module
        .functions
        .iter()
        .map(|(name, function)| FrontendFunctionNode {
            name: name.clone(),
            signature: format_pipeline_function_signature(name, &function.params),
            doc: function.doc.clone(),
            step_count: function.body.len(),
            source_id: function.source_id.clone(),
            package_scope: function.package_scope.clone(),
            params: function
                .params
                .iter()
                .map(|param| FrontendFunctionParam {
                    name: param.name.clone(),
                    has_default: param.default_value.is_some(),
                    declared_kind: param
                        .declared_kind
                        .map(pipeline_value_kind_text)
                        .map(str::to_string),
                    effective_kind: param
                        .inferred_kind
                        .map(pipeline_value_kind_text)
                        .map(str::to_string),
                })
                .collect(),
        })
        .collect();
    let merged_step_count = module.body.len()
        + module
            .functions
            .values()
            .map(|function| function.body.len())
            .sum::<usize>();
    let graph_nodes = pipeline_graph_nodes(&module);
    let expansion_previews = pipeline_expansion_previews(&module);
    let graph_edges = pipeline_graph_edges(module.include_edges, &module.use_edges);
    let mut use_edges = module.use_edges;
    use_edges.sort_by(|left, right| {
        (left.from != "entry")
            .cmp(&(right.from != "entry"))
            .then_with(|| left.from.cmp(&right.from))
    });
    FrontendModuleSummary {
        kind: FrontendDslKind::Pipeline,
        module_doc: module.module_doc,
        template_doc: module.template_doc,
        function_count: module.functions.len(),
        function_nodes,
        merged_step_count,
        include_sources: module.include_sources,
        use_edges,
        graph_nodes,
        graph_edges,
        expansion_previews,
    }
}
