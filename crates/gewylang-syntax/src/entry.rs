use std::path::Path;

use crate::package;
use crate::parser::{parse_pipeline_function_head, parse_pipeline_module};
use crate::source_graph::SourceGraphState;
use crate::{
    PackageContext, PipelineModule, SyntaxError, read_file, strip_comments_preserve_layout,
    validate_source_size,
};

pub fn parse_file(path: &str) -> Result<PipelineModule, SyntaxError> {
    let (input, package) = load_file_with_package_context(path)?;
    parse_str_with_package(&input, &package)
}

pub fn parse_str(input: &str) -> Result<PipelineModule, SyntaxError> {
    parse_expanded_pipeline_module(input, None)
}

pub fn parse_str_with_package(
    input: &str,
    package: &PackageContext,
) -> Result<PipelineModule, SyntaxError> {
    parse_expanded_pipeline_module(input, Some(package))
}

pub fn load_file_with_package_context(path: &str) -> Result<(String, PackageContext), SyntaxError> {
    let package = package::resolve_package_context(path)?;
    let input = read_file(&package.entry_file)?;
    Ok((input, package))
}

pub(crate) fn parse_expanded_pipeline_module(
    input: &str,
    package: Option<&PackageContext>,
) -> Result<PipelineModule, SyntaxError> {
    validate_source_size(input)?;
    let entry_path = package.map(|package| Path::new(&package.entry_file));
    let mut source_graph = SourceGraphState::new(entry_path, input.len())?;
    parse_expanded_pipeline_module_with_graph(input, package, &mut source_graph)
}

fn parse_expanded_pipeline_module_with_graph(
    input: &str,
    package: Option<&PackageContext>,
    source_graph: &mut SourceGraphState,
) -> Result<PipelineModule, SyntaxError> {
    validate_source_size(input)?;
    let normalized = strip_comments_preserve_layout(input)?;
    if looks_like_pipeline_dsl(&normalized) {
        return parse_pipeline_module(&normalized, package, true, source_graph);
    }

    let Some(include_target) = parse_include_entry_alias_target(&normalized) else {
        return Err(SyntaxError::InvalidValue(
            "gewylang now only supports the pipeline stable subset".into(),
        ));
    };
    let Some(package) = package else {
        return Err(SyntaxError::InvalidValue(
            "pipeline include() requires a filesystem-backed entry file".into(),
        ));
    };
    let include = package::resolve_include(package, &include_target)?;
    let include_input = source_graph.load_include(&include.path)?;
    let include_package = package.for_include(&include);
    let result = parse_expanded_pipeline_module_with_graph(
        &include_input,
        Some(&include_package),
        source_graph,
    );
    source_graph.leave_include(&include.path);
    result
}

pub fn looks_like_pipeline_dsl(input: &str) -> bool {
    input
        .lines()
        .map(str::trim)
        .find(|line| {
            !line.is_empty()
                && !line.starts_with('#')
                && !line.starts_with("///")
                && !line.starts_with("//!")
        })
        .is_some_and(|line| {
            is_pipeline_template_head(line)
                || parse_pipeline_function_head(line).is_some()
                || line == "fn"
                || line.starts_with("fn ")
                || line == "template"
                || line.starts_with("template(")
        })
}

fn is_pipeline_template_head(line: &str) -> bool {
    (line.starts_with("template(") && line.ends_with(')'))
        || line
            .strip_prefix("template ")
            .is_some_and(|value| !value.trim().is_empty())
}

fn parse_include_entry_alias_target(input: &str) -> Option<String> {
    let mut substantive_lines = input.lines().map(str::trim).filter(|line| {
        !line.is_empty()
            && !line.starts_with('#')
            && !line.starts_with("///")
            && !line.starts_with("//!")
    });
    let line = substantive_lines.next()?;
    if substantive_lines.next().is_some() {
        return None;
    }
    if let Some(target) = line.strip_prefix("include ") {
        return parse_quoted_include_target(target.trim());
    }
    if let Some(target) = line
        .strip_prefix("include(")
        .and_then(|rest| rest.strip_suffix(')'))
    {
        return parse_quoted_include_target(target.trim());
    }
    None
}

fn parse_quoted_include_target(input: &str) -> Option<String> {
    input
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_frontend_parses_without_a_product_runtime() {
        let module = parse_str("template :standalone\n|> window :default_5s\n").unwrap();
        assert_eq!(module.package_scope, "inline");
        assert_eq!(module.template.unwrap().args, [":standalone"]);
        assert_eq!(module.body.len(), 1);
    }
}
