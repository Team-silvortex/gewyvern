use crate::fragment::{RegistryError, builtin_registry};
use crate::template::TemplateBinding;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

mod entry;
mod frontend;
mod function_types;
mod legacy;
mod package;
mod pipeline;
mod predicate;

pub use self::entry::{compile_file, parse_file_unvalidated, parse_str_unvalidated};
pub use self::frontend::{
    FrontendDslKind, FrontendExpansionPreview, FrontendFunctionNode, FrontendFunctionParam,
    FrontendGraphEdge, FrontendGraphEdgeKind, FrontendGraphNode, FrontendGraphNodeKind,
    FrontendIncludeSource, FrontendIncludeSourceKind, FrontendModuleSummary, FrontendUseEdge,
    summarize_frontend_file, summarize_frontend_str,
};
use self::function_types::{
    PipelineValueKind, infer_pipeline_param_kinds, resolve_pipeline_param_kind,
};
use self::package::PackageContext;
pub use self::package::build_lockfile;
use self::pipeline::{
    lower_pipeline_module_to_legacy, parse_pipeline_call, parse_pipeline_function_signature,
    parse_pipeline_let_binding, parse_pipeline_single_arg, push_pipeline_function_call,
};
pub(crate) use self::predicate::{parse_flow_predicate, parse_reason_key_event};

pub const PACKAGE_MANIFEST_FILE: &str = "gewy.pkg";

#[derive(Debug, Eq, PartialEq)]
pub enum DslError {
    Located {
        line: usize,
        column: Option<usize>,
        inner: Box<DslError>,
    },
    InvalidLine(String),
    MissingField(&'static str),
    InvalidValue(String),
    Registry(RegistryError),
    Io(String),
}

pub fn read_file(path: &str) -> Result<String, DslError> {
    fs::read_to_string(path).map_err(|err| DslError::Io(err.to_string()))
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PipelineModule {
    package_scope: String,
    template: Option<PipelineCall>,
    body: Vec<PipelineCall>,
    functions: BTreeMap<String, PipelineFunction>,
    include_sources: Vec<FrontendIncludeSource>,
    include_edges: Vec<FrontendGraphEdge>,
    use_edges: Vec<FrontendUseEdge>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PipelineFunction {
    params: Vec<PipelineParam>,
    local_bindings: Vec<PipelineLetBinding>,
    body: Vec<PipelineCall>,
    source_id: String,
    package_scope: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PipelineParam {
    name: String,
    default_value: Option<String>,
    declared_kind: Option<PipelineValueKind>,
    inferred_kind: Option<PipelineValueKind>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PipelineCall {
    line_no: usize,
    column_no: usize,
    name: String,
    args: Vec<String>,
    arg_columns: Vec<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PipelineFunctionBodySyntax {
    Block,
    Expression,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PipelineLetBinding {
    name: String,
    value: String,
}

fn parse_pipeline_module(
    input: &str,
    package: Option<&PackageContext>,
    allow_template_head: bool,
) -> Result<PipelineModule, DslError> {
    let mut module = PipelineModule {
        package_scope: package
            .map(|package| package.package_scope.clone())
            .unwrap_or_else(|| "inline".to_string()),
        template: None,
        body: Vec::new(),
        functions: BTreeMap::new(),
        include_sources: Vec::new(),
        include_edges: Vec::new(),
        use_edges: Vec::new(),
    };
    let mut include_stack = package
        .map(|package| vec![PathBuf::from(&package.entry_file)])
        .unwrap_or_default();
    parse_pipeline_module_into(
        input,
        package,
        allow_template_head,
        &mut module,
        None,
        "entry",
        &mut include_stack,
    )?;

    if allow_template_head && module.template.is_none() {
        return Err(DslError::InvalidValue(
            "pipeline DSL must start with template(...)".into(),
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
    include_stack: &mut Vec<PathBuf>,
) -> Result<(), DslError> {
    let lines = input.lines().collect::<Vec<_>>();
    let mut index = 0usize;

    while index < lines.len() {
        let line_no = index + 1;
        let raw_line = lines[index];
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            index += 1;
            continue;
        }

        if let Some((signature, body_syntax)) = parse_pipeline_function_head(line) {
            let signature_column = raw_line.find("fn ").map(|idx| idx + 4).unwrap_or(1);
            let (name, params) = parse_pipeline_function_signature(signature)
                .map_err(|err| err.reanchor_line_column(line_no, signature_column))?;
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
                        if !body_line.is_empty() && !body_line.starts_with('#') {
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
            let inferred_param_kinds = infer_pipeline_param_kinds(&params, &local_bindings, &body)
                .map_err(|err| err.reanchor_line_column(line_no, signature_column))?;
            let params = params
                .into_iter()
                .map(|mut param| {
                    param.inferred_kind = resolve_pipeline_param_kind(
                        &param.name,
                        param.declared_kind,
                        inferred_param_kinds.get(&param.name).copied(),
                    )?;
                    Ok(param)
                })
                .collect::<Result<Vec<_>, DslError>>()?;
            module.functions.insert(
                name.to_string(),
                PipelineFunction {
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
                if module.template.is_some() || !allow_template_head {
                    return Err(DslError::InvalidValue(
                        "pipeline DSL supports exactly one template() head".into(),
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
                let include = parse_pipeline_single_arg(&args, "include")?;
                let package = package.ok_or_else(|| {
                    DslError::InvalidValue(
                        "pipeline include() requires a filesystem-backed entry file".into(),
                    )
                    .at_line(line_no)
                })?;
                let include_path = package::resolve_include_path(package, &include)
                    .map_err(|err| err.at_line(line_no))?;
                if include_stack.contains(&include_path) {
                    return Err(DslError::InvalidValue(format!(
                        "pipeline include cycle detected at '{}'",
                        include_path.to_string_lossy()
                    ))
                    .at_line(line_no));
                }
                module.include_sources.push(FrontendIncludeSource {
                    request: include.clone(),
                    resolved_path: include_path.to_string_lossy().into_owned(),
                    kind: if include.split_once(':').is_some() {
                        FrontendIncludeSourceKind::Dependency
                    } else {
                        FrontendIncludeSourceKind::Local
                    },
                    dependency: include
                        .split_once(':')
                        .map(|(dependency, _)| dependency.to_string()),
                    package_scope: include
                        .split_once(':')
                        .map(|(dependency, _)| dependency.to_string())
                        .unwrap_or_else(|| package.package_scope.clone()),
                });
                module.include_edges.push(FrontendGraphEdge {
                    from: source_graph_id.to_string(),
                    to: format!("file:{}", include_path.to_string_lossy()),
                    kind: FrontendGraphEdgeKind::Include,
                    line: line_no,
                });
                let include_input = fs::read_to_string(&include_path)
                    .map_err(|err| DslError::Io(err.to_string()).at_line(line_no))?;
                let include_root = include_path
                    .parent()
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| package.root_dir.clone());
                let include_package = PackageContext {
                    package_scope: include
                        .split_once(':')
                        .map(|(dependency, _)| dependency.to_string())
                        .unwrap_or_else(|| package.package_scope.clone()),
                    root_dir: include_root,
                    entry_file: include_path.to_string_lossy().into_owned(),
                    dependencies: package.dependencies.clone(),
                };
                include_stack.push(include_path.clone());
                parse_pipeline_module_into(
                    &include_input,
                    Some(&include_package),
                    false,
                    module,
                    function_name,
                    &format!("file:{}", include_path.to_string_lossy()),
                    include_stack,
                )
                .map_err(|err| err.at_line(line_no))?;
                include_stack.pop();
            }
            other => {
                let target = if let Some(function_name) = function_name {
                    &mut module
                        .functions
                        .get_mut(function_name)
                        .expect("function exists while parsing")
                        .body
                } else {
                    &mut module.body
                };
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
                if other == "use" {
                    if let Ok(target_name) =
                        parse_pipeline_single_arg(&target.last().unwrap().args, "use")
                    {
                        module.use_edges.push(frontend::FrontendUseEdge {
                            from: function_name.unwrap_or("entry").to_string(),
                            to: target_name,
                            line: line_no,
                        });
                    }
                }
            }
        }
        index += 1;
    }

    Ok(())
}

fn parse_pipeline_function_head(line: &str) -> Option<(&str, PipelineFunctionBodySyntax)> {
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

pub fn compile_str(input: &str) -> Result<TemplateBinding, DslError> {
    let binding = parse_str_unvalidated(input)?;
    validate_compiled_binding(&binding).map_err(DslError::Registry)?;
    Ok(binding)
}

pub fn validate_compiled_binding(binding: &TemplateBinding) -> Result<(), RegistryError> {
    builtin_registry().validate_binding(binding)
}

impl DslError {
    pub fn at_line(self, line: usize) -> Self {
        self.at_line_column(line, None)
    }

    pub fn at_line_column(self, line: usize, column: Option<usize>) -> Self {
        match self {
            Self::Located {
                line: existing_line,
                column: existing_column,
                inner,
            } => Self::Located {
                line: if existing_line == 0 {
                    line
                } else {
                    existing_line
                },
                column: existing_column.or(column),
                inner,
            },
            other => Self::Located {
                line,
                column,
                inner: Box::new(other),
            },
        }
    }

    pub fn reanchor_line_column(self, line: usize, column_offset: usize) -> Self {
        match self {
            Self::Located {
                line: existing_line,
                column,
                inner,
            } => Self::Located {
                line: if existing_line == 0 {
                    line
                } else {
                    existing_line
                },
                column: column.map(|value| value + column_offset.saturating_sub(1)),
                inner,
            },
            other => Self::Located {
                line,
                column: Some(column_offset),
                inner: Box::new(other),
            },
        }
    }

    pub fn line(&self) -> Option<usize> {
        match self {
            Self::Located { line, .. } => Some(*line),
            _ => None,
        }
    }

    pub fn column(&self) -> Option<usize> {
        match self {
            Self::Located { column, .. } => *column,
            _ => None,
        }
    }

    pub fn root(&self) -> &DslError {
        match self {
            Self::Located { inner, .. } => inner.root(),
            other => other,
        }
    }
}

fn parse_bool(value: &str) -> Result<bool, DslError> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        other => Err(DslError::InvalidValue(format!("invalid bool '{other}'"))),
    }
}

fn split_top_level_with_columns(
    input: &str,
    delimiter: char,
    base_column: usize,
) -> Vec<(usize, String)> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;

    for (idx, ch) in input.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ if ch == delimiter && depth == 0 => {
                let raw = &input[start..idx];
                let trimmed = raw.trim();
                let leading = raw.find(trimmed).unwrap_or(0);
                parts.push((base_column + start + leading, trimmed.to_string()));
                start = idx + ch.len_utf8();
            }
            _ => {}
        }
    }

    let raw = &input[start..];
    let trimmed = raw.trim();
    let leading = raw.find(trimmed).unwrap_or(0);
    parts.push((base_column + start + leading, trimmed.to_string()));
    parts
}

#[cfg(test)]
mod tests {
    use super::legacy::{parse_reason_rule, parse_rule};

    #[test]
    fn parse_rule_reanchors_invalid_stage_column() {
        let input = "process_bound;not_a_stage;static:test;true";
        let err = parse_rule(input).expect_err("invalid stage");
        assert_eq!(err.column(), Some(input.find("not_a_stage").unwrap() + 1));
    }

    #[test]
    fn parse_rule_reanchors_composite_predicate_child_column() {
        let input = "all(process_bound, packet_observed:tcp:remote:mysql:byte_at:not_u16:255:1);connect_flow;static:test;true";
        let err = parse_rule(input).expect_err("invalid composite predicate child");
        assert_eq!(err.column(), Some(input.find("byte_at").unwrap() + 1));
    }

    #[test]
    fn parse_reason_rule_reanchors_invalid_key_event_column() {
        let input = "process_bound;not_a_reason_event;static:test;true";
        let err = parse_reason_rule(input).expect_err("invalid reason key event");
        assert_eq!(
            err.column(),
            Some(input.find("not_a_reason_event").unwrap() + 1)
        );
    }
}
