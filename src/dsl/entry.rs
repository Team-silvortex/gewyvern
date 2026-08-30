use super::frontend::summarize_pipeline_module;
use super::legacy::build_binding_from_canonical_assignments;
use super::source_graph::SourceGraphState;
use super::{
    DslError, FrontendModuleSummary, PackageContext, TemplateBinding,
    lower_pipeline_module_to_assignments, package, parse_pipeline_function_head,
    parse_pipeline_module, read_file, strip_comments_preserve_layout, validate_compiled_binding,
    validate_gewylang_source_size,
};
use std::path::Path;

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

pub(crate) fn parse_str_unvalidated_with_package(
    input: &str,
    package: &PackageContext,
) -> Result<TemplateBinding, DslError> {
    parse_str_unvalidated_with_base(input, Some(package))
}

fn parse_str_unvalidated_with_base(
    input: &str,
    package: Option<&PackageContext>,
) -> Result<TemplateBinding, DslError> {
    let module = parse_expanded_pipeline_module(input, package)?;
    let assignments = lower_pipeline_module_to_assignments(&module, true)?;
    build_binding_from_canonical_assignments(assignments)
}

fn parse_str_with_frontend_unvalidated_with_base(
    input: &str,
    package: Option<&PackageContext>,
) -> Result<(TemplateBinding, FrontendModuleSummary), DslError> {
    let module = parse_expanded_pipeline_module(input, package)?;
    let assignments = lower_pipeline_module_to_assignments(&module, true)?;
    let binding = build_binding_from_canonical_assignments(assignments)?;
    let frontend = summarize_pipeline_module(module);
    Ok((binding, frontend))
}

pub(super) fn parse_expanded_pipeline_module(
    input: &str,
    package: Option<&PackageContext>,
) -> Result<super::PipelineModule, DslError> {
    validate_gewylang_source_size(input)?;
    let entry_path = package.map(|package| Path::new(&package.entry_file));
    let mut source_graph = SourceGraphState::new(entry_path, input.len())?;
    parse_expanded_pipeline_module_with_graph(input, package, &mut source_graph)
}

fn parse_expanded_pipeline_module_with_graph(
    input: &str,
    package: Option<&PackageContext>,
    source_graph: &mut SourceGraphState,
) -> Result<super::PipelineModule, DslError> {
    validate_gewylang_source_size(input)?;
    let normalized = strip_comments_preserve_layout(input)?;
    if looks_like_pipeline_dsl(&normalized) {
        return parse_pipeline_module(&normalized, package, true, source_graph);
    }

    let Some(include_target) = parse_include_entry_alias_target(&normalized) else {
        return Err(DslError::InvalidValue(
            "gewylang now only supports the pipeline stable subset".into(),
        ));
    };
    let Some(package) = package else {
        return Err(DslError::InvalidValue(
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

pub(super) fn looks_like_pipeline_dsl(input: &str) -> bool {
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

    #[test]
    fn entry_alias_rejects_escape_from_package_root() {
        let root = temp_root("include-alias-escape");
        let package = root.join("package");
        fs::create_dir_all(&package).unwrap();
        fs::write(
            package.join("gewy.pkg"),
            "name=include_alias_escape\nversion=0.1.0\nentry=main.gewy\n",
        )
        .unwrap();
        fs::write(package.join("main.gewy"), "include \"../outside.gewy\"\n").unwrap();
        fs::write(
            root.join("outside.gewy"),
            "template :outside\n|> window :default_5s\n",
        )
        .unwrap();

        let err = compile_file(package.to_str().unwrap()).unwrap_err();
        fs::remove_dir_all(root).unwrap();
        assert!(
            format!("{err:?}").contains("escapes package root"),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn entry_alias_cycle_is_rejected_without_recursive_overflow() {
        let root = temp_root("include-alias-cycle");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("gewy.pkg"),
            "name=include_alias_cycle\nversion=0.1.0\nentry=main.gewy\n",
        )
        .unwrap();
        fs::write(root.join("main.gewy"), "include \"./module.gewy\"\n").unwrap();
        fs::write(root.join("module.gewy"), "include \"./main.gewy\"\n").unwrap();

        let err = compile_file(root.to_str().unwrap()).unwrap_err();
        fs::remove_dir_all(root).unwrap();
        assert!(
            format!("{err:?}").contains("include cycle detected"),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn entry_alias_depth_is_bounded() {
        let root = temp_root("include-alias-depth");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("gewy.pkg"),
            "name=include_alias_depth\nversion=0.1.0\nentry=source_0.gewy\n",
        )
        .unwrap();
        for index in 0..=crate::dsl::MAX_GEWYLANG_INCLUDE_DEPTH {
            fs::write(
                root.join(format!("source_{index}.gewy")),
                format!("include \"./source_{}.gewy\"\n", index + 1),
            )
            .unwrap();
        }
        fs::write(
            root.join(format!(
                "source_{}.gewy",
                crate::dsl::MAX_GEWYLANG_INCLUDE_DEPTH + 1
            )),
            "template :depth_limit\n|> window :default_5s\n",
        )
        .unwrap();

        let err = compile_file(root.to_str().unwrap()).unwrap_err();
        fs::remove_dir_all(root).unwrap();
        assert!(
            format!("{err:?}").contains("include depth exceeds 32"),
            "unexpected error: {err:?}"
        );
    }
}
