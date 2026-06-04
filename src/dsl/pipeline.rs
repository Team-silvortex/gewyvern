use super::{
    DslError, PipelineCall, PipelineFunction, PipelineLetBinding, PipelineModule, PipelineParam,
    frontend, function_types::validate_pipeline_param_value_kind, legacy,
};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Eq, PartialEq)]
struct PipelineProvidedArg {
    raw: String,
    value: String,
}

pub(super) struct PipelineUseCall {
    function_name: String,
    positional_args: Vec<PipelineProvidedArg>,
    named_args: BTreeMap<String, PipelineProvidedArg>,
}

pub(super) fn push_pipeline_function_call(
    module: &mut PipelineModule,
    function_name: &str,
    raw_body_line: &str,
    body_line: &str,
    line_no: usize,
    output: &mut Vec<PipelineCall>,
) -> Result<(), DslError> {
    let nested_call = body_line.strip_prefix("|>").ok_or_else(|| {
        DslError::InvalidValue(
            "pipeline function bodies may only contain 'let' bindings or '|>' steps".into(),
        )
        .at_line(line_no)
    })?;
    let nested_call = nested_call.trim();
    let nested_call_column = raw_body_line
        .find(nested_call)
        .map(|idx| idx + 1)
        .unwrap_or(1);
    let (nested_name, nested_args, nested_arg_columns) = parse_pipeline_call(nested_call)
        .map_err(|err| err.reanchor_line_column(line_no, nested_call_column))?;
    if nested_name == "use" {
        if let Ok(target_name) = parse_pipeline_single_arg(&nested_args, "use") {
            module.use_edges.push(frontend::FrontendUseEdge {
                from: function_name.trim().to_string(),
                to: target_name,
                line: line_no,
            });
        }
    }
    output.push(PipelineCall {
        line_no,
        column_no: nested_call_column,
        name: nested_name,
        args: nested_args,
        arg_columns: nested_arg_columns
            .into_iter()
            .map(|column| nested_call_column + column.saturating_sub(1))
            .collect(),
    });
    Ok(())
}

pub(super) fn parse_pipeline_let_binding(
    line: &str,
) -> Result<Option<PipelineLetBinding>, DslError> {
    let Some(remainder) = line.strip_prefix("let ") else {
        return Ok(None);
    };
    let (name, value) = remainder.split_once('=').ok_or_else(|| {
        DslError::InvalidValue(format!("invalid let binding '{line}'"))
            .at_line_column(0, Some(line.len() + 1))
    })?;
    let name = parse_pipeline_param_name(name)?;
    let value = value.trim();
    if value.is_empty() {
        return Err(DslError::InvalidValue(format!(
            "pipeline let binding '{name}' requires a value"
        ))
        .at_line_column(0, Some(line.len() + 1)));
    }
    Ok(Some(PipelineLetBinding {
        name,
        value: value.to_string(),
    }))
}

pub(super) fn lower_pipeline_module_to_legacy(
    module: &PipelineModule,
    allow_template_head: bool,
) -> Result<String, DslError> {
    let mut output = Vec::<String>::new();
    if let Some(template) = &module.template {
        output.push(format!(
            "template={}",
            parse_pipeline_single_arg(&template.args, "template")
                .map_err(|err| err.at_line(template.line_no))?
        ));
    } else if allow_template_head {
        return Err(DslError::InvalidValue(
            "pipeline DSL must start with template(...)".into(),
        ));
    }

    lower_pipeline_calls(
        &module.body,
        module,
        &mut output,
        allow_template_head,
        &mut Vec::new(),
        &BTreeMap::new(),
    )?;
    Ok(output.join("\n"))
}

fn lower_pipeline_calls(
    calls: &[PipelineCall],
    module: &PipelineModule,
    output: &mut Vec<String>,
    allow_template_head: bool,
    use_stack: &mut Vec<String>,
    bindings: &BTreeMap<String, String>,
) -> Result<(), DslError> {
    for call in calls {
        lower_pipeline_call(
            call,
            module,
            output,
            allow_template_head,
            use_stack,
            bindings,
        )?;
    }
    Ok(())
}

fn lower_pipeline_call(
    call: &PipelineCall,
    module: &PipelineModule,
    output: &mut Vec<String>,
    allow_template_head: bool,
    use_stack: &mut Vec<String>,
    bindings: &BTreeMap<String, String>,
) -> Result<(), DslError> {
    let line_no = call.line_no;
    let column_no = call.column_no;
    let resolved_args = call
        .args
        .iter()
        .map(|arg| substitute_pipeline_arg(arg, bindings))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| err.reanchor_line_column(line_no, column_no))?;
    match call.name.as_str() {
        "template" => {
            if !allow_template_head {
                return Err(DslError::InvalidValue(
                    "pipeline DSL supports exactly one template() head".into(),
                )
                .at_line(line_no));
            }
            output.push(format!(
                "template={}",
                parse_pipeline_single_arg(&resolved_args, "template")
                    .map_err(|err| err.reanchor_line_column(line_no, column_no))?
            ));
        }
        "use" => {
            let use_call = parse_pipeline_use_call(&resolved_args)
                .map_err(|err| err.reanchor_line_column(line_no, column_no))?;
            let function_name = use_call.function_name.clone();
            if use_stack.contains(&function_name) {
                return Err(DslError::InvalidValue(format!(
                    "pipeline use cycle detected at function '{function_name}'"
                ))
                .at_line(line_no));
            }
            let function = module.functions.get(&function_name).ok_or_else(|| {
                DslError::InvalidValue(format!("unknown pipeline function '{function_name}'"))
                    .at_line(line_no)
            })?;
            let mut function_bindings = build_pipeline_function_bindings(function, &use_call)
                .map_err(|err| err.reanchor_line_column(line_no, column_no))?;
            for binding in &function.local_bindings {
                let resolved = substitute_pipeline_arg(&binding.value, &function_bindings)
                    .map_err(|err| err.reanchor_line_column(line_no, column_no))?;
                function_bindings.insert(binding.name.clone(), parse_pipeline_literal(&resolved));
            }
            use_stack.push(function_name.clone());
            lower_pipeline_calls(
                &function.body,
                module,
                output,
                false,
                use_stack,
                &function_bindings,
            )?;
            use_stack.pop();
        }
        "include" => {
            return Err(DslError::InvalidValue(
                "pipeline include() should be resolved before lowering".into(),
            )
            .at_line(line_no));
        }
        "window" => lower_pipeline_window(&resolved_args, &call.arg_columns, column_no, output)
            .map_err(|err| err.at_line(line_no))?,
        "reason" => output.push(format!(
            "reason={}",
            parse_pipeline_single_arg(&resolved_args, "reason")
                .map_err(|err| err.reanchor_line_column(line_no, column_no))?
        )),
        "reason_model" => output.push(format!(
            "reason_model={}",
            parse_pipeline_single_arg(&resolved_args, "reason_model")
                .map_err(|err| err.reanchor_line_column(line_no, column_no))?
        )),
        "fragment" => output.push(format!(
            "fragment={}",
            parse_pipeline_single_arg(&resolved_args, "fragment")
                .map_err(|err| err.reanchor_line_column(line_no, column_no))?
        )),
        "program_model" => output.push(format!(
            "program_model={}",
            parse_pipeline_single_arg(&resolved_args, "program_model")
                .map_err(|err| err.reanchor_line_column(line_no, column_no))?
        )),
        "operation" => output.push(format!(
            "operation={}",
            parse_pipeline_single_arg(&resolved_args, "operation")
                .map_err(|err| err.reanchor_line_column(line_no, column_no))?
        )),
        "param" => output.push(format!(
            "param={}",
            lower_pipeline_param(&resolved_args)
                .map_err(|err| err.reanchor_line_column(line_no, column_no))?
        )),
        "evidence" => output.push(format!(
            "evidence={}",
            lower_pipeline_evidence(&resolved_args)
                .map_err(|err| err.reanchor_line_column(line_no, column_no))?
        )),
        "program_rule" => output.push(
            lower_pipeline_rule(&resolved_args, &call.arg_columns, column_no, false)
                .map_err(|err| err.at_line(line_no))?,
        ),
        "reason_rule" => output.push(
            lower_pipeline_rule(&resolved_args, &call.arg_columns, column_no, true)
                .map_err(|err| err.at_line(line_no))?,
        ),
        other => {
            return Err(
                DslError::InvalidValue(format!("unknown pipeline DSL step '{other}'"))
                    .at_line(line_no),
            );
        }
    }
    Ok(())
}

pub(super) fn parse_pipeline_call(
    line: &str,
) -> Result<(String, Vec<String>, Vec<usize>), DslError> {
    let open = line.find('(').ok_or_else(|| {
        DslError::InvalidValue(format!("invalid pipeline call '{line}'"))
            .at_line_column(0, Some(line.len() + 1))
    })?;
    let name = line[..open].trim();
    let inner = line[open + 1..].strip_suffix(')').ok_or_else(|| {
        DslError::InvalidValue(format!("invalid pipeline call '{line}'"))
            .at_line_column(0, Some(line.len() + 1))
    })?;
    if name.is_empty() {
        return Err(
            DslError::InvalidValue(format!("invalid pipeline call '{line}'"))
                .at_line_column(0, Some(1)),
        );
    }
    let args_with_columns = split_pipeline_args_with_columns(inner, open + 2);
    Ok((
        name.to_string(),
        args_with_columns
            .iter()
            .map(|(_, value)| value.clone())
            .collect(),
        args_with_columns
            .into_iter()
            .map(|(column, _)| column)
            .collect(),
    ))
}

pub(super) fn parse_pipeline_function_signature(
    signature: &str,
) -> Result<(String, Vec<PipelineParam>), DslError> {
    let open = signature.find('(').ok_or_else(|| {
        DslError::InvalidValue(format!("invalid function signature '{signature}'"))
            .at_line_column(0, Some(signature.len() + 1))
    })?;
    let close = signature.rfind(')').ok_or_else(|| {
        DslError::InvalidValue(format!("invalid function signature '{signature}'"))
            .at_line_column(0, Some(signature.len() + 1))
    })?;
    if close < open {
        return Err(
            DslError::InvalidValue(format!("invalid function signature '{signature}'"))
                .at_line_column(0, Some(close + 1)),
        );
    }
    let name = signature[..open].trim();
    if name.is_empty() {
        return Err(
            DslError::InvalidValue(format!("invalid function signature '{signature}'"))
                .at_line_column(0, Some(1)),
        );
    }
    let params_src = &signature[open + 1..close];
    let params = if params_src.trim().is_empty() {
        Vec::new()
    } else {
        split_pipeline_args(params_src)
            .into_iter()
            .map(|param| parse_pipeline_param(&param))
            .collect::<Result<Vec<_>, _>>()?
    };
    if let Some(non_trailing_required) =
        params.windows(2).find_map(
            |pair| match (&pair[0].default_value, &pair[1].default_value) {
                (Some(_), None) => Some(pair[1].name.clone()),
                _ => None,
            },
        )
    {
        return Err(DslError::InvalidValue(format!(
            "pipeline function required parameter '{non_trailing_required}' cannot follow a defaulted parameter"
        ))
        .at_line_column(0, Some(open + 2)));
    }
    Ok((name.to_string(), params))
}

fn parse_pipeline_param(param: &str) -> Result<PipelineParam, DslError> {
    let trimmed = param.trim();
    let (name_src, default_value) = match trimmed.split_once('=') {
        Some((name, default)) => {
            let default = default.trim();
            if default.is_empty() {
                return Err(DslError::InvalidValue(format!(
                    "pipeline parameter '{}' requires a default value after '='",
                    name.trim()
                ))
                .at_line_column(0, Some(trimmed.len() + 1)));
            }
            (name, Some(default.to_string()))
        }
        None => (trimmed, None),
    };
    Ok(PipelineParam {
        name: parse_pipeline_param_name(name_src)?,
        default_value,
        inferred_kind: None,
    })
}

pub(super) fn parse_pipeline_param_name(param: &str) -> Result<String, DslError> {
    let trimmed = param.trim();
    let value = trimmed
        .strip_prefix(':')
        .unwrap_or(trimmed)
        .trim()
        .to_string();
    if value.is_empty() {
        return Err(
            DslError::InvalidValue("pipeline parameter name cannot be empty".into())
                .at_line_column(0, Some(1)),
        );
    }
    Ok(value)
}

pub(super) fn split_pipeline_args(input: &str) -> Vec<String> {
    split_pipeline_args_with_columns(input, 1)
        .into_iter()
        .map(|(_, value)| value)
        .collect()
}

fn split_pipeline_args_with_columns(input: &str, base_column: usize) -> Vec<(usize, String)> {
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut in_string = false;
    let chars = input.char_indices().peekable();
    for (idx, ch) in chars {
        match ch {
            '"' => in_string = !in_string,
            ',' if !in_string => {
                let raw = &input[start..idx];
                let trimmed = raw.trim();
                if !trimmed.is_empty() {
                    let leading = raw.find(trimmed).unwrap_or(0);
                    parts.push((base_column + start + leading, trimmed.to_string()));
                }
                start = idx + 1;
            }
            _ => {}
        }
    }
    let raw_tail = &input[start..];
    let tail = raw_tail.trim();
    if !tail.is_empty() {
        let leading = raw_tail.find(tail).unwrap_or(0);
        parts.push((base_column + start + leading, tail.to_string()));
    }
    parts
}

pub(super) fn parse_pipeline_literal(value: &str) -> String {
    let value = value.trim();
    if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
        value[1..value.len() - 1].to_string()
    } else if let Some(atom) = value.strip_prefix(':') {
        atom.trim().to_string()
    } else {
        value.to_string()
    }
}

pub(super) fn parse_pipeline_single_arg(args: &[String], step: &str) -> Result<String, DslError> {
    if args.len() != 1 {
        return Err(DslError::InvalidValue(format!(
            "pipeline step '{step}' expects exactly one argument"
        ))
        .at_line_column(0, Some(1)));
    }
    Ok(parse_pipeline_literal(&args[0]))
}

pub(super) fn parse_pipeline_use_call(args: &[String]) -> Result<PipelineUseCall, DslError> {
    if args.is_empty() {
        return Err(DslError::InvalidValue(
            "pipeline step 'use' expects at least one argument".into(),
        )
        .at_line_column(0, Some(1)));
    }
    let function_name = parse_pipeline_literal(&args[0]);
    let mut positional_args = Vec::new();
    let mut named_args = BTreeMap::new();
    let mut named_section_started = false;

    for arg in &args[1..] {
        if looks_like_pipeline_keyword_arg(arg) {
            named_section_started = true;
            let (name, value) = arg.split_once(':').ok_or_else(|| {
                DslError::InvalidValue(format!(
                    "pipeline step 'use' expected named argument, got '{arg}'"
                ))
                .at_line_column(0, Some(1))
            })?;
            let name = parse_pipeline_param_name(name)?;
            if named_args.contains_key(&name) {
                return Err(DslError::InvalidValue(format!(
                    "pipeline step 'use' received duplicate named argument '{name}'"
                ))
                .at_line_column(0, Some(1)));
            }
            let value = value.trim();
            if value.is_empty() {
                return Err(DslError::InvalidValue(format!(
                    "pipeline step 'use' named argument '{name}' requires a value"
                ))
                .at_line_column(0, Some(1)));
            }
            named_args.insert(
                name,
                PipelineProvidedArg {
                    raw: value.to_string(),
                    value: parse_pipeline_literal(value),
                },
            );
        } else {
            if named_section_started {
                return Err(DslError::InvalidValue(
                    "pipeline step 'use' cannot place positional arguments after named arguments"
                        .into(),
                )
                .at_line_column(0, Some(1)));
            }
            positional_args.push(PipelineProvidedArg {
                raw: arg.to_string(),
                value: parse_pipeline_literal(arg),
            });
        }
    }

    Ok(PipelineUseCall {
        function_name,
        positional_args,
        named_args,
    })
}

fn build_pipeline_function_bindings(
    function: &PipelineFunction,
    use_call: &PipelineUseCall,
) -> Result<BTreeMap<String, String>, DslError> {
    let function_name = &use_call.function_name;
    let required_count = function
        .params
        .iter()
        .filter(|param| param.default_value.is_none())
        .count();
    if use_call.positional_args.len() > function.params.len() {
        return Err(DslError::InvalidValue(format!(
            "pipeline function '{function_name}' expects at most {} positional args, got {}",
            function.params.len(),
            use_call.positional_args.len()
        ))
        .at_line_column(0, Some(1)));
    }
    if use_call.positional_args.len() + use_call.named_args.len() < required_count
        || use_call.positional_args.len() + use_call.named_args.len() > function.params.len()
    {
        let arity = if required_count == function.params.len() {
            format!("{}", function.params.len())
        } else {
            format!("{} to {}", required_count, function.params.len())
        };
        return Err(DslError::InvalidValue(format!(
            "pipeline function '{function_name}' expects {arity} args, got {}",
            use_call.positional_args.len() + use_call.named_args.len()
        ))
        .at_line_column(0, Some(1)));
    }

    let mut bindings = BTreeMap::new();
    let mut consumed_named = BTreeMap::<String, ()>::new();
    for param in function.params.iter().take(use_call.positional_args.len()) {
        if use_call.named_args.contains_key(&param.name) {
            return Err(DslError::InvalidValue(format!(
                "pipeline function '{function_name}' received both positional and named values for parameter '{}'",
                param.name
            ))
            .at_line_column(0, Some(1)));
        }
    }
    for (index, param) in function.params.iter().enumerate() {
        if let Some(actual) = use_call.positional_args.get(index) {
            if let Some(kind) = param.inferred_kind {
                validate_pipeline_param_value_kind(
                    &actual.raw,
                    kind,
                    &format!(
                        "pipeline function '{function_name}' parameter '{}'",
                        param.name
                    ),
                )?;
            }
            bindings.insert(param.name.clone(), actual.value.clone());
            continue;
        }
        if let Some(actual) = use_call.named_args.get(&param.name) {
            if let Some(kind) = param.inferred_kind {
                validate_pipeline_param_value_kind(
                    &actual.raw,
                    kind,
                    &format!(
                        "pipeline function '{function_name}' parameter '{}'",
                        param.name
                    ),
                )?;
            }
            bindings.insert(param.name.clone(), actual.value.clone());
            consumed_named.insert(param.name.clone(), ());
            continue;
        }
        let default_value = param.default_value.as_ref().ok_or_else(|| {
            DslError::InvalidValue(format!(
                "pipeline function '{function_name}' is missing required parameter '{}'",
                param.name
            ))
            .at_line_column(0, Some(1))
        })?;
        let resolved = substitute_pipeline_arg(default_value, &bindings)?;
        if let Some(kind) = param.inferred_kind {
            validate_pipeline_param_value_kind(
                &resolved,
                kind,
                &format!(
                    "default value for pipeline function '{function_name}' parameter '{}'",
                    param.name
                ),
            )?;
        }
        bindings.insert(param.name.clone(), parse_pipeline_literal(&resolved));
    }
    if let Some(unknown_name) = use_call
        .named_args
        .keys()
        .find(|name| !consumed_named.contains_key(*name))
    {
        return Err(DslError::InvalidValue(format!(
            "pipeline function '{function_name}' does not define a parameter named '{unknown_name}'"
        ))
        .at_line_column(0, Some(1)));
    }
    Ok(bindings)
}

pub(super) fn substitute_pipeline_arg(
    arg: &str,
    bindings: &BTreeMap<String, String>,
) -> Result<String, DslError> {
    let mut result = arg.to_string();
    while let Some(start) = result.find("${") {
        let tail = &result[start + 2..];
        let end_rel = tail.find('}').ok_or_else(|| {
            DslError::InvalidValue(format!("unclosed pipeline placeholder in '{arg}'"))
                .at_line_column(0, Some(start + 1))
        })?;
        let end = start + 2 + end_rel;
        let key = result[start + 2..end].trim();
        let value = bindings.get(key).ok_or_else(|| {
            DslError::InvalidValue(format!("unknown pipeline parameter '{key}'"))
                .at_line_column(0, Some(start + 3))
        })?;
        result.replace_range(start..=end, value);
    }
    Ok(result)
}

fn looks_like_pipeline_keyword_arg(arg: &str) -> bool {
    let arg = arg.trim();
    if arg.starts_with(':') || arg.starts_with('"') {
        return false;
    }
    arg.split_once(':')
        .is_some_and(|(key, _)| !key.trim().is_empty())
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PipelineKeywordArg {
    value: String,
    value_column: usize,
}

fn parse_pipeline_keywords_with_columns(
    args: &[String],
    arg_columns: &[usize],
    step: &str,
) -> Result<BTreeMap<String, PipelineKeywordArg>, DslError> {
    let mut keywords = BTreeMap::new();
    for (arg, arg_column) in args.iter().zip(arg_columns.iter()) {
        let (key, value) = arg.split_once(':').ok_or_else(|| {
            DslError::InvalidValue(format!(
                "pipeline step '{step}' expected keyword argument, got '{arg}'"
            ))
            .at_line_column(0, Some(*arg_column))
        })?;
        let value_trimmed = value.trim();
        let value_offset = value.find(value_trimmed).unwrap_or(0);
        keywords.insert(
            key.trim().to_string(),
            PipelineKeywordArg {
                value: parse_pipeline_literal(value),
                value_column: arg_column + key.len() + 1 + value_offset,
            },
        );
    }
    Ok(keywords)
}

pub(super) fn lower_pipeline_window(
    args: &[String],
    arg_columns: &[usize],
    call_column: usize,
    output: &mut Vec<String>,
) -> Result<(), DslError> {
    if args.len() == 1 && !looks_like_pipeline_keyword_arg(&args[0]) {
        output.push(format!("window={}", parse_pipeline_literal(&args[0])));
        return Ok(());
    }
    let keywords = parse_pipeline_keywords_with_columns(args, arg_columns, "window")?;
    let duration_ms = keywords
        .get("duration_ms")
        .ok_or(DslError::MissingField("duration_ms").at_line_column(0, Some(call_column)))?;
    let lateness_ms = keywords
        .get("lateness_ms")
        .ok_or(DslError::MissingField("lateness_ms").at_line_column(0, Some(call_column)))?;
    output.push(format!("window.duration_ms={}", duration_ms.value));
    output.push(format!("window.lateness_ms={}", lateness_ms.value));
    Ok(())
}

pub(super) fn lower_pipeline_param(args: &[String]) -> Result<String, DslError> {
    if args.len() != 2 {
        return Err(DslError::InvalidValue(
            "pipeline step 'param' expects target and value".into(),
        )
        .at_line_column(0, Some(1)));
    }
    Ok(format!(
        "{}={}",
        parse_pipeline_literal(&args[0]),
        parse_pipeline_literal(&args[1])
    ))
}

pub(super) fn lower_pipeline_evidence(args: &[String]) -> Result<String, DslError> {
    if args.len() != 2 {
        return Err(DslError::InvalidValue(
            "pipeline step 'evidence' expects fact kind and tier".into(),
        )
        .at_line_column(0, Some(1)));
    }
    Ok(format!(
        "{}:{}",
        parse_pipeline_literal(&args[0]),
        parse_pipeline_literal(&args[1])
    ))
}

pub(super) fn lower_pipeline_rule(
    args: &[String],
    arg_columns: &[usize],
    call_column: usize,
    reason_rule: bool,
) -> Result<String, DslError> {
    let keywords = parse_pipeline_keywords_with_columns(
        args,
        arg_columns,
        if reason_rule {
            "reason_rule"
        } else {
            "program_rule"
        },
    )?;
    let predicate = keywords
        .get("predicate")
        .ok_or(DslError::MissingField("predicate").at_line_column(0, Some(call_column)))?;
    let signal_key = if reason_rule { "key_event" } else { "stage" };
    let signal = keywords
        .get(signal_key)
        .ok_or(DslError::MissingField(signal_key).at_line_column(0, Some(call_column)))?;
    let narrative = keywords
        .get("narrative")
        .ok_or(DslError::MissingField("narrative").at_line_column(0, Some(call_column)))?;
    let dedupe = keywords
        .get("dedupe")
        .ok_or(DslError::MissingField("dedupe").at_line_column(0, Some(call_column)))?;
    super::parse_flow_predicate(&predicate.value)
        .map_err(|err| err.reanchor_line_column(0, predicate.value_column))?;
    if reason_rule {
        super::parse_reason_key_event(&signal.value)
            .map_err(|err| err.reanchor_line_column(0, signal.value_column))?;
    } else {
        legacy::parse_stage(&signal.value)
            .map_err(|err| err.reanchor_line_column(0, signal.value_column))?;
    }
    super::parse_bool(&dedupe.value)
        .map_err(|err| err.reanchor_line_column(0, dedupe.value_column))?;
    let mut value = format!(
        "{};{};{};{}",
        predicate.value, signal.value, narrative.value, dedupe.value
    );
    if let Some(module) = keywords.get("module") {
        value.push(';');
        value.push_str(&module.value);
        if let Some(phase) = keywords.get("phase") {
            value.push(';');
            value.push_str(&phase.value);
        }
    } else if let Some(phase) = keywords.get("phase") {
        return Err(DslError::InvalidValue(format!(
            "pipeline rule phase '{}' requires module",
            phase.value
        ))
        .at_line_column(0, Some(phase.value_column)));
    }
    Ok(if reason_rule {
        format!("reason.rule={value}")
    } else {
        format!("rule={value}")
    })
}
