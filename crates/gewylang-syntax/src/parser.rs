use super::function_types::{
    format_pipeline_function_signature, infer_pipeline_param_kinds, resolve_pipeline_param_kind,
};
use super::parsing::{
    parse_pipeline_call, parse_pipeline_function_signature, parse_pipeline_let_binding,
    parse_pipeline_single_arg, push_pipeline_function_call,
};
use super::source_graph::SourceGraphState;
use super::{
    FrontendGraphEdge, FrontendGraphEdgeKind, FrontendIncludeSource, FrontendIncludeSourceKind,
    FrontendUseEdge, PackageContext, PipelineCall, PipelineFunction, PipelineFunctionBodySyntax,
    PipelineLetBinding, PipelineModule, PipelineParam, SyntaxError as DslError, package,
    strip_comments_preserve_layout,
};
use std::collections::BTreeMap;

pub(super) fn parse_pipeline_module(
    input: &str,
    package: Option<&PackageContext>,
    allow_template_head: bool,
    source_graph: &mut SourceGraphState,
) -> Result<PipelineModule, DslError> {
    let mut module = PipelineModule {
        package_scope: package
            .map(|package| package.package_scope.clone())
            .unwrap_or_else(|| "inline".to_string()),
        module_doc: None,
        template_doc: None,
        template: None,
        body: Vec::new(),
        functions: BTreeMap::new(),
        include_sources: Vec::new(),
        include_edges: Vec::new(),
        use_edges: Vec::new(),
    };
    parse_pipeline_module_into(
        input,
        package,
        allow_template_head,
        &mut module,
        None,
        "entry",
        source_graph,
    )?;

    if allow_template_head && module.template.is_none() {
        return Err(DslError::InvalidValue(
            "pipeline DSL must start with template(...) or template :name".into(),
        ));
    }

    Ok(module)
}

fn parse_pipeline_module_into(
    input: &str,
    package: Option<&PackageContext>,
    allow_template_head: bool,
    module: &mut PipelineModule,
    function_name: Option<&str>,
    source_graph_id: &str,
    source_graph: &mut SourceGraphState,
) -> Result<(), DslError> {
    let lines = input.lines().collect::<Vec<_>>();
    let mut index = 0usize;
    let mut pending_docs = Vec::new();

    while index < lines.len() {
        let line_no = index + 1;
        let raw_line = lines[index];
        let line = raw_line.trim();
        if line.is_empty() {
            index += 1;
            continue;
        }
        if let Some(doc) = line.strip_prefix("//!") {
            push_pipeline_doc_line(&mut module.module_doc, doc);
            pending_docs.clear();
            index += 1;
            continue;
        }
        if let Some(doc) = line.strip_prefix("///") {
            pending_docs.push(doc.trim().to_string());
            index += 1;
            continue;
        }
        if line.starts_with('#') {
            pending_docs.clear();
            index += 1;
            continue;
        }

        if let Some((signature, body_syntax)) = parse_pipeline_function_head(line) {
            let signature_column = raw_line.find("fn ").map(|idx| idx + 4).unwrap_or(1);
            let (name, params) = parse_pipeline_function_signature(signature)
                .map_err(|err| err.reanchor_line_column(line_no, signature_column))?;
            if module.functions.contains_key(&name) {
                return Err(DslError::InvalidValue(format!(
                    "duplicate pipeline function '{name}'"
                ))
                .at_line_column(line_no, Some(signature_column)));
            }
            let function_doc = take_pipeline_pending_docs(&mut pending_docs);
            let mut local_bindings = Vec::new();
            let mut body = Vec::new();
            index += 1;
            match body_syntax {
                PipelineFunctionBodySyntax::Block => {
                    while index < lines.len() {
                        let raw_body_line = lines[index];
                        let body_line = raw_body_line.trim();
                        if body_line == "}" {
                            break;
                        }
                        if !body_line.is_empty()
                            && !body_line.starts_with('#')
                            && !body_line.starts_with("///")
                            && !body_line.starts_with("//!")
                        {
                            parse_pipeline_function_body_line(
                                module,
                                &name,
                                raw_body_line,
                                body_line,
                                index + 1,
                                &params,
                                &mut local_bindings,
                                &mut body,
                            )?;
                        }
                        index += 1;
                    }
                    if index == lines.len() {
                        return Err(DslError::InvalidValue(
                            "unclosed pipeline function block".into(),
                        )
                        .at_line(line_no));
                    }
                    index += 1;
                }
                PipelineFunctionBodySyntax::Expression => {
                    while index < lines.len() {
                        let raw_body_line = lines[index];
                        let body_line = raw_body_line.trim();
                        if body_line.is_empty() || body_line.starts_with('#') {
                            index += 1;
                            continue;
                        }
                        if body_line.starts_with("///") || body_line.starts_with("//!") {
                            break;
                        }
                        if !body_line.starts_with("|>") && !body_line.starts_with("let ") {
                            break;
                        }
                        parse_pipeline_function_body_line(
                            module,
                            &name,
                            raw_body_line,
                            body_line,
                            index + 1,
                            &params,
                            &mut local_bindings,
                            &mut body,
                        )?;
                        index += 1;
                    }
                    if body.is_empty() {
                        return Err(DslError::InvalidValue(
                            "pipeline function expressions must contain '|>' steps".into(),
                        )
                        .at_line(line_no));
                    }
                }
            }
            let function_signature = format_pipeline_function_signature(&name, &params);
            let inferred_param_kinds =
                infer_pipeline_param_kinds(&function_signature, &params, &local_bindings, &body)
                    .map_err(|err| err.reanchor_line_column(line_no, signature_column))?;
            let params = params
                .into_iter()
                .map(|mut param| {
                    param.inferred_kind = resolve_pipeline_param_kind(
                        &function_signature,
                        &param.name,
                        param.declared_kind,
                        inferred_param_kinds.get(&param.name).copied(),
                    )
                    .map_err(|err| err.reanchor_line_column(line_no, signature_column))?;
                    Ok(param)
                })
                .collect::<Result<Vec<_>, DslError>>()?;
            module.functions.insert(
                name.to_string(),
                PipelineFunction {
                    doc: function_doc,
                    params,
                    local_bindings,
                    body,
                    source_id: source_graph_id.to_string(),
                    package_scope: package
                        .map(|package| package.package_scope.clone())
                        .unwrap_or_else(|| "inline".to_string()),
                },
            );
            continue;
        }

        if line == "fn" || line.starts_with("fn ") {
            return Err(
                DslError::InvalidValue(format!("invalid function signature '{line}'"))
                    .at_line_column(line_no, Some(line.len() + 1)),
            );
        }

        let call = if module.template.is_some() || !allow_template_head {
            line.strip_prefix("|>")
                .ok_or_else(|| {
                    DslError::InvalidValue(
                        "pipeline DSL steps after template must start with '|>'".into(),
                    )
                    .at_line(line_no)
                })?
                .trim()
        } else {
            line
        };

        let call_column = raw_line.find(call).map(|idx| idx + 1).unwrap_or(1);
        let (name, args, arg_columns) = parse_pipeline_call(call)
            .map_err(|err| err.reanchor_line_column(line_no, call_column))?;
        match name.as_str() {
            "template" => {
                if module.template_doc.is_none() {
                    module.template_doc = take_pipeline_pending_docs(&mut pending_docs);
                } else {
                    pending_docs.clear();
                }
                if module.template.is_some() || !allow_template_head {
                    return Err(DslError::InvalidValue(
                        "pipeline DSL supports exactly one template head".into(),
                    )
                    .at_line(line_no));
                }
                module.template = Some(PipelineCall {
                    line_no,
                    column_no: call_column,
                    name,
                    args,
                    arg_columns: arg_columns
                        .into_iter()
                        .map(|column| call_column + column.saturating_sub(1))
                        .collect(),
                });
            }
            "include" => {
                pending_docs.clear();
                let include = parse_pipeline_single_arg(&args, "include")?;
                let package = package.ok_or_else(|| {
                    DslError::InvalidValue(
                        "pipeline include() requires a filesystem-backed entry file".into(),
                    )
                    .at_line(line_no)
                })?;
                let resolved = package::resolve_include(package, &include)
                    .map_err(|err| err.at_line(line_no))?;
                let resolved_path = resolved.path.to_string_lossy().into_owned();
                module.include_sources.push(FrontendIncludeSource {
                    request: include.clone(),
                    resolved_path: resolved_path.clone(),
                    kind: if resolved.dependency.is_some() {
                        FrontendIncludeSourceKind::Dependency
                    } else {
                        FrontendIncludeSourceKind::Local
                    },
                    dependency: resolved.dependency.clone(),
                    package_scope: resolved.package_scope.clone(),
                });
                let include_graph_id = format!("file:{resolved_path}");
                module.include_edges.push(FrontendGraphEdge {
                    from: source_graph_id.to_string(),
                    to: include_graph_id.clone(),
                    kind: FrontendGraphEdgeKind::Include,
                    line: line_no,
                });
                let include_input = source_graph
                    .load_include(&resolved.path)
                    .map_err(|err| err.at_line(line_no))?;
                let include_package = package.for_include(&resolved);
                let result =
                    strip_comments_preserve_layout(&include_input).and_then(|normalized| {
                        parse_pipeline_module_into(
                            &normalized,
                            Some(&include_package),
                            false,
                            module,
                            function_name,
                            &include_graph_id,
                            source_graph,
                        )
                    });
                source_graph
                    .leave_include(&resolved.path)
                    .map_err(|err| err.at_line(line_no))?;
                result.map_err(|err| err.at_line(line_no))?;
            }
            other => {
                pending_docs.clear();
                let target = if let Some(function_name) = function_name {
                    &mut module
                        .functions
                        .get_mut(function_name)
                        .ok_or_else(|| {
                            DslError::InvalidValue(format!(
                                "pipeline function '{function_name}' is unavailable while parsing"
                            ))
                            .at_line(line_no)
                        })?
                        .body
                } else {
                    &mut module.body
                };
                let use_target = (other == "use")
                    .then(|| parse_pipeline_single_arg(&args, "use").ok())
                    .flatten();
                target.push(PipelineCall {
                    line_no,
                    column_no: call_column,
                    name: other.to_string(),
                    args,
                    arg_columns: arg_columns
                        .into_iter()
                        .map(|column| call_column + column.saturating_sub(1))
                        .collect(),
                });
                if let Some(target_name) = use_target {
                    module.use_edges.push(FrontendUseEdge {
                        from: function_name.unwrap_or("entry").to_string(),
                        to: target_name,
                        line: line_no,
                    });
                }
            }
        }
        index += 1;
    }

    Ok(())
}

pub fn parse_pipeline_function_head(line: &str) -> Option<(&str, PipelineFunctionBodySyntax)> {
    if let Some(header) = line.strip_suffix('{') {
        let header = header.trim();
        let signature = header.strip_prefix("fn ")?;
        return Some((signature, PipelineFunctionBodySyntax::Block));
    }
    if let Some(header) = line.strip_suffix("=>") {
        let header = header.trim();
        let signature = header.strip_prefix("fn ")?;
        return Some((signature, PipelineFunctionBodySyntax::Expression));
    }
    if let Some(header) = line.strip_suffix('=') {
        let header = header.trim();
        let signature = header.strip_prefix("fn ")?;
        return Some((signature, PipelineFunctionBodySyntax::Expression));
    }
    None
}

// Parser state is intentionally explicit so diagnostics retain exact source anchors.
#[allow(clippy::too_many_arguments)]
fn parse_pipeline_function_body_line(
    module: &mut PipelineModule,
    function_name: &str,
    raw_body_line: &str,
    body_line: &str,
    line_no: usize,
    params: &[PipelineParam],
    local_bindings: &mut Vec<PipelineLetBinding>,
    output: &mut Vec<PipelineCall>,
) -> Result<(), DslError> {
    let body_column = raw_body_line
        .find(body_line)
        .map(|idx| idx + 1)
        .unwrap_or(1);
    if let Some(binding) = parse_pipeline_let_binding(body_line)
        .map_err(|err| err.reanchor_line_column(line_no, body_column))?
    {
        if params.iter().any(|param| param.name == binding.name)
            || local_bindings
                .iter()
                .any(|existing| existing.name == binding.name)
        {
            return Err(DslError::InvalidValue(format!(
                "duplicate pipeline local binding '{}'",
                binding.name
            ))
            .at_line(line_no));
        }
        local_bindings.push(binding);
        return Ok(());
    }
    push_pipeline_function_call(
        module,
        function_name,
        raw_body_line,
        body_line,
        line_no,
        output,
    )
}

fn take_pipeline_pending_docs(lines: &mut Vec<String>) -> Option<String> {
    if lines.is_empty() {
        None
    } else {
        Some(std::mem::take(lines).join("\n"))
    }
}

fn push_pipeline_doc_line(slot: &mut Option<String>, line: &str) {
    let line = line.trim();
    if line.is_empty() {
        return;
    }
    match slot {
        Some(existing) => {
            existing.push('\n');
            existing.push_str(line);
        }
        None => *slot = Some(line.to_string()),
    }
}
