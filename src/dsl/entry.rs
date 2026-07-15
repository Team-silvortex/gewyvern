use super::frontend::summarize_pipeline_module;
use super::legacy::parse_legacy_str_unvalidated;
use super::{
    DslError, FrontendModuleSummary, PackageContext, TemplateBinding,
    lower_pipeline_module_to_legacy, package, parse_pipeline_function_head, parse_pipeline_module,
    read_file, strip_comments_preserve_layout, validate_compiled_binding,
};

pub fn parse_file_unvalidated(path: &str) -> Result<TemplateBinding, DslError> {
    let (input, package) = load_file_with_package_context(path)?;
    parse_str_unvalidated_with_base(&input, Some(&package))
}

pub fn parse_file_with_frontend_unvalidated(
    path: &str,
) -> Result<(TemplateBinding, FrontendModuleSummary), DslError> {
    let (input, package) = load_file_with_package_context(path)?;
    parse_str_with_frontend_unvalidated_with_base(&input, Some(&package))
}

pub fn compile_file(path: &str) -> Result<TemplateBinding, DslError> {
    let binding = parse_file_unvalidated(path)?;
    validate_compiled_binding(&binding).map_err(DslError::Registry)?;
    Ok(binding)
}

pub fn parse_str_unvalidated(input: &str) -> Result<TemplateBinding, DslError> {
    parse_str_unvalidated_with_base(input, None)
}

pub fn parse_str_with_frontend_unvalidated(
    input: &str,
) -> Result<(TemplateBinding, FrontendModuleSummary), DslError> {
    parse_str_with_frontend_unvalidated_with_base(input, None)
}

pub(crate) fn load_file_with_package_context(
    path: &str,
) -> Result<(String, PackageContext), DslError> {
    let package = package::resolve_package_context(path)?;
    let resolved = package.entry_file.clone();
    let input = read_file(&resolved)?;
    Ok((input, package))
}

pub(crate) fn parse_str_with_frontend_unvalidated_with_package(
    input: &str,
    package: &PackageContext,
) -> Result<(TemplateBinding, FrontendModuleSummary), DslError> {
    parse_str_with_frontend_unvalidated_with_base(input, Some(package))
}

fn parse_str_unvalidated_with_base(
    input: &str,
    package: Option<&PackageContext>,
) -> Result<TemplateBinding, DslError> {
    let normalized = strip_comments_preserve_layout(input);
    if looks_like_pipeline_dsl(&normalized) {
        let legacy = pipeline_to_legacy(&normalized, package)?;
        return parse_legacy_str_unvalidated(&legacy);
    }
    if let Some((include_input, include_package)) =
        resolve_include_entry_alias(&normalized, package)?
    {
        return parse_str_unvalidated_with_base(&include_input, Some(&include_package));
    }
    Err(DslError::InvalidValue(
        "gewylang now only supports the pipeline stable subset".into(),
    ))
}

fn parse_str_with_frontend_unvalidated_with_base(
    input: &str,
    package: Option<&PackageContext>,
) -> Result<(TemplateBinding, FrontendModuleSummary), DslError> {
    let normalized = strip_comments_preserve_layout(input);
    if looks_like_pipeline_dsl(&normalized) {
        let module = parse_pipeline_module(&normalized, package, true)?;
        let legacy = lower_pipeline_module_to_legacy(&module, true)?;
        let binding = parse_legacy_str_unvalidated(&legacy)?;
        let frontend = summarize_pipeline_module(module);
        return Ok((binding, frontend));
    }
    if let Some((include_input, include_package)) =
        resolve_include_entry_alias(&normalized, package)?
    {
        return parse_str_with_frontend_unvalidated_with_base(
            &include_input,
            Some(&include_package),
        );
    }
    Err(DslError::InvalidValue(
        "gewylang now only supports the pipeline stable subset".into(),
    ))
}

pub(super) fn looks_like_pipeline_dsl(input: &str) -> bool {
    input
        .lines()
        .map(str::trim)
        .filter(|line| {
            !line.is_empty()
                && !line.starts_with('#')
                && !line.starts_with("///")
                && !line.starts_with("//!")
        })
        .next()
        .is_some_and(|line| {
            is_pipeline_template_head(line) || parse_pipeline_function_head(line).is_some()
        })
}

fn is_pipeline_template_head(line: &str) -> bool {
    (line.starts_with("template(") && line.ends_with(')'))
        || line
            .strip_prefix("template ")
            .is_some_and(|value| !value.trim().is_empty())
}

fn pipeline_to_legacy(input: &str, package: Option<&PackageContext>) -> Result<String, DslError> {
    let module = parse_pipeline_module(input, package, true)?;
    lower_pipeline_module_to_legacy(&module, true)
}

pub(super) fn resolve_include_entry_alias(
    input: &str,
    package: Option<&PackageContext>,
) -> Result<Option<(String, PackageContext)>, DslError> {
    let include_target = parse_include_entry_alias_target(input);
    let Some(include_target) = include_target else {
        return Ok(None);
    };
    let Some(package) = package else {
        return Ok(None);
    };
    let include_root = std::path::Path::new(&package.entry_file)
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| package.root_dir.clone());
    let include_path = include_root
        .join(include_target)
        .canonicalize()
        .map_err(|err| DslError::Io(err.to_string()))?;
    let include_input = read_file(&include_path.to_string_lossy())?;
    let include_root = include_path
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| package.root_dir.clone());
    Ok(Some((
        include_input,
        PackageContext {
            package_scope: package.package_scope.clone(),
            root_dir: include_root,
            entry_file: include_path.to_string_lossy().into_owned(),
            dependencies: package.dependencies.clone(),
        },
    )))
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
    use crate::dsl::summarize_frontend_file;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "gewy-entry-{label}-{}-{unique}",
            std::process::id()
        ))
    }

    #[test]
    fn compile_file_accepts_single_include_entry_alias() {
        let root = temp_root("include-alias-compile");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("gewy.pkg"),
            "name=include_alias_pkg\nversion=0.1.0\nentry=main.gewy\n",
        )
        .unwrap();
        fs::write(root.join("main.gewy"), "include \"./module.gewy\"\n").unwrap();
        fs::copy(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("dsl/http_request_path.gewy"),
            root.join("module.gewy"),
        )
        .unwrap();

        let binding = compile_file(root.to_str().unwrap()).unwrap();
        fs::remove_dir_all(&root).unwrap();

        assert_eq!(binding.template.id, "http_request_path");
    }

    #[test]
    fn summarize_frontend_file_accepts_single_include_entry_alias() {
        let root = temp_root("include-alias-frontend");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("gewy.pkg"),
            "name=include_alias_pkg\nversion=0.1.0\nentry=main.gewy\n",
        )
        .unwrap();
        fs::write(root.join("main.gewy"), "include \"./module.gewy\"\n").unwrap();
        fs::write(
            root.join("module.gewy"),
            "fn include_alias_rules() =\n  |> operation(:include_alias)\n\ntemplate(:include_alias_pkg)\n|> window(:default_5s)\n|> use(:include_alias_rules)\n",
        )
        .unwrap();

        let summary = summarize_frontend_file(root.to_str().unwrap()).unwrap();
        fs::remove_dir_all(&root).unwrap();

        assert_eq!(summary.function_count, 1);
        assert!(
            summary
                .function_nodes
                .iter()
                .any(|node| node.name == "include_alias_rules")
        );
    }
}
