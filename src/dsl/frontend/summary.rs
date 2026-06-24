use super::{
    FrontendDslKind, FrontendFunctionNode, FrontendFunctionParam, FrontendModuleSummary,
};
use super::graph::{
    pipeline_expansion_previews, pipeline_graph_edges, pipeline_graph_nodes, pipeline_use_edges,
};
use crate::dsl::entry::looks_like_pipeline_dsl;
use crate::dsl::function_types::{format_pipeline_function_signature, pipeline_value_kind_text};
use crate::dsl::{DslError, PackageContext, parse_pipeline_module};

pub(super) fn summarize_frontend_file(path: &str) -> Result<FrontendModuleSummary, DslError> {
    let package = crate::dsl::package::resolve_package_context(path)?;
    let resolved = package.entry_file.clone();
    let input = crate::dsl::read_file(&resolved)?;
    summarize_frontend_str_with_base(&input, Some(&package))
}

pub(super) fn summarize_frontend_str(input: &str) -> Result<FrontendModuleSummary, DslError> {
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
        let use_edges = pipeline_use_edges(&module);
        let graph_nodes = pipeline_graph_nodes(&module);
        let graph_edges = pipeline_graph_edges(&module);
        let expansion_previews = pipeline_expansion_previews(&module);
        return Ok(FrontendModuleSummary {
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
        });
    }
    Err(DslError::InvalidValue(
        "gewylang now only supports the pipeline stable subset".into(),
    ))
}
