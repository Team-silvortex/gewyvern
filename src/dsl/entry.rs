use super::legacy::parse_legacy_str_unvalidated;
use super::{
    DslError, PackageContext, TemplateBinding, lower_pipeline_module_to_legacy, package,
    parse_pipeline_function_head, parse_pipeline_module, read_file, validate_compiled_binding,
};

pub fn parse_file_unvalidated(path: &str) -> Result<TemplateBinding, DslError> {
    let package = package::resolve_package_context(path)?;
    let resolved = package.entry_file.clone();
    let input = read_file(&resolved)?;
    parse_str_unvalidated_with_base(&input, Some(&package))
}

pub fn compile_file(path: &str) -> Result<TemplateBinding, DslError> {
    let binding = parse_file_unvalidated(path)?;
    validate_compiled_binding(&binding).map_err(DslError::Registry)?;
    Ok(binding)
}

pub fn parse_str_unvalidated(input: &str) -> Result<TemplateBinding, DslError> {
    parse_str_unvalidated_with_base(input, None)
}

fn parse_str_unvalidated_with_base(
    input: &str,
    package: Option<&PackageContext>,
) -> Result<TemplateBinding, DslError> {
    if looks_like_pipeline_dsl(input) {
        let legacy = pipeline_to_legacy(input, package)?;
        return parse_legacy_str_unvalidated(&legacy);
    }
    Err(DslError::InvalidValue(
        "gewylang now only supports the pipeline stable subset".into(),
    ))
}

pub(super) fn looks_like_pipeline_dsl(input: &str) -> bool {
    input
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .next()
        .is_some_and(|line| {
            (line.starts_with("template(") && line.ends_with(')'))
                || parse_pipeline_function_head(line).is_some()
        })
}

fn pipeline_to_legacy(input: &str, package: Option<&PackageContext>) -> Result<String, DslError> {
    let module = parse_pipeline_module(input, package, true)?;
    lower_pipeline_module_to_legacy(&module, true)
}
