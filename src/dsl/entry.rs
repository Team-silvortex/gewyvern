use super::{
    DslError, FrontendModuleSummary, PackageContext, PipelineModule, TemplateBinding,
    semantic_host::GewyvernSemanticHost, summarize_pipeline_module, validate_compiled_binding,
};

pub fn parse_file_unvalidated(path: &str) -> Result<TemplateBinding, DslError> {
    gewylang_compiler::compile_binding_file(path, &GewyvernSemanticHost)
}

pub fn parse_file_with_frontend_unvalidated(
    path: &str,
) -> Result<(TemplateBinding, FrontendModuleSummary), DslError> {
    let module = gewylang_syntax::parse_file(path).map_err(DslError::from)?;
    binding_and_frontend_from_module(module)
}

pub fn compile_file(path: &str) -> Result<TemplateBinding, DslError> {
    let binding = parse_file_unvalidated(path)?;
    validate_compiled_binding(&binding).map_err(DslError::Registry)?;
    Ok(binding)
}

pub fn parse_str_unvalidated(input: &str) -> Result<TemplateBinding, DslError> {
    gewylang_compiler::compile_binding_str(input, &GewyvernSemanticHost)
}

pub fn parse_str_with_frontend_unvalidated(
    input: &str,
) -> Result<(TemplateBinding, FrontendModuleSummary), DslError> {
    let module = gewylang_syntax::parse_str(input).map_err(DslError::from)?;
    binding_and_frontend_from_module(module)
}

pub(crate) fn load_file_with_package_context(
    path: &str,
) -> Result<(String, PackageContext), DslError> {
    gewylang_syntax::load_file_with_package_context(path).map_err(DslError::from)
}

pub(crate) fn parse_str_with_frontend_unvalidated_with_package(
    input: &str,
    package: &PackageContext,
) -> Result<(TemplateBinding, FrontendModuleSummary), DslError> {
    let module = gewylang_syntax::parse_str_with_package(input, package).map_err(DslError::from)?;
    binding_and_frontend_from_module(module)
}

pub(crate) fn parse_str_unvalidated_with_package(
    input: &str,
    package: &PackageContext,
) -> Result<TemplateBinding, DslError> {
    let module = gewylang_syntax::parse_str_with_package(input, package).map_err(DslError::from)?;
    binding_from_module(&module)
}

fn binding_from_module(module: &PipelineModule) -> Result<TemplateBinding, DslError> {
    gewylang_compiler::lower_and_materialize_pipeline_module(module, &GewyvernSemanticHost, true)
}

fn binding_and_frontend_from_module(
    module: PipelineModule,
) -> Result<(TemplateBinding, FrontendModuleSummary), DslError> {
    let binding = binding_from_module(&module)?;
    let frontend = summarize_pipeline_module(module);
    Ok((binding, frontend))
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
