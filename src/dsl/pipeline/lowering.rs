use super::parsing::{parse_pipeline_literal, parse_pipeline_single_arg, parse_pipeline_use_call};
use super::{PipelineKeywordArg, PipelineUseCall, looks_like_pipeline_keyword_arg};
use crate::dsl::{
    DslError, PipelineCall, PipelineFunction, PipelineModule,
    diagnostics::{
        pipeline_available_steps_message, pipeline_declared_functions_message,
        pipeline_declared_params_message, pipeline_unknown_placeholder_message,
    },
    function_types::{format_pipeline_function_signature, validate_pipeline_param_value_kind},
    legacy,
};
use std::collections::BTreeMap;

pub(crate) fn lower_pipeline_module_to_legacy(
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
            "pipeline DSL must start with template(...) or template :name".into(),
        ));
    }

    lower_pipeline_calls(
        &module.body,
        module,
        &mut output,
        allow_template_head,
        &mut Vec::new(),
        &BTreeMap::new(),
        "entry pipeline",
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
    scope_context: &str,
) -> Result<(), DslError> {
    for call in calls {
        lower_pipeline_call(
            call,
            module,
            output,
            allow_template_head,
            use_stack,
            bindings,
            scope_context,
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
    scope_context: &str,
) -> Result<(), DslError> {
    let line_no = call.line_no;
    let column_no = call.column_no;
    let call_context = format!("{} while expanding {}", call.name, scope_context);
    let resolved_args = call
        .args
        .iter()
        .map(|arg| substitute_pipeline_arg(arg, bindings, &call_context))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| err.reanchor_line_column(line_no, column_no))?;
    match call.name.as_str() {
        "template" => {
            if !allow_template_head {
                return Err(DslError::InvalidValue(
                    "pipeline DSL supports exactly one template head".into(),
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
                let declared = module.functions.keys().cloned().collect::<Vec<_>>();
                DslError::InvalidValue(format!(
                    "unknown pipeline function '{function_name}'. {}",
                    pipeline_declared_functions_message(&declared)
                ))
                .at_line(line_no)
            })?;
            let mut function_bindings = build_pipeline_function_bindings(function, &use_call)
                .map_err(|err| err.reanchor_line_column(line_no, column_no))?;
            let function_signature =
                format_pipeline_function_signature(&function_name, &function.params);
            for binding in &function.local_bindings {
                let binding_context =
                    format!("local binding '{}' in {function_signature}", binding.name);
                let resolved =
                    substitute_pipeline_arg(&binding.value, &function_bindings, &binding_context)
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
                &function_signature,
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
            return Err(DslError::InvalidValue(format!(
                "unknown pipeline DSL step '{other}'. {}",
                pipeline_available_steps_message()
            ))
            .at_line(line_no));
        }
    }
    Ok(())
}

fn build_pipeline_function_bindings(
    function: &PipelineFunction,
    use_call: &PipelineUseCall,
) -> Result<BTreeMap<String, String>, DslError> {
    let function_name = &use_call.function_name;
    let signature = format_pipeline_function_signature(function_name, &function.params);
    let required_count = function
        .params
        .iter()
        .filter(|param| param.default_value.is_none())
        .count();
    if use_call.positional_args.len() > function.params.len() {
        return Err(DslError::InvalidValue(format!(
            "pipeline function call does not match {signature}: expected at most {} positional args, got {}",
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
            "pipeline function call does not match {signature}: expected {arity} args, got {}",
            use_call.positional_args.len() + use_call.named_args.len()
        ))
        .at_line_column(0, Some(1)));
    }

    let mut bindings = BTreeMap::new();
    let mut consumed_named = BTreeMap::<String, ()>::new();
    for param in function.params.iter().take(use_call.positional_args.len()) {
        if use_call.named_args.contains_key(&param.name) {
            return Err(DslError::InvalidValue(format!(
                "pipeline function call does not match {signature}: parameter '{}' received both positional and named values",
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
                "pipeline function call does not match {signature}: missing required parameter '{}'",
                param.name
            ))
            .at_line_column(0, Some(1))
        })?;
        let default_context = format!(
            "default value for parameter '{}' in {signature}",
            param.name
        );
        let resolved = substitute_pipeline_arg(default_value, &bindings, &default_context)?;
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
            "pipeline function call does not match {signature}: unknown named parameter '{unknown_name}'. {}",
            known_pipeline_params_message(function_name, function)
        ))
        .at_line_column(0, Some(1)));
    }
    Ok(bindings)
}

pub(crate) fn substitute_pipeline_arg(
    arg: &str,
    bindings: &BTreeMap<String, String>,
    context: &str,
) -> Result<String, DslError> {
    let mut current = arg.to_string();
    let mut iterations = 0usize;
    loop {
        let (next, changed) = substitute_pipeline_arg_once(&current, bindings, context)?;
        current = next;
        if !changed || !current.contains('$') {
            return Ok(current);
        }
        iterations += 1;
        if iterations > 32 {
            return Ok(current);
        }
    }
}

fn substitute_pipeline_arg_once(
    arg: &str,
    bindings: &BTreeMap<String, String>,
    context: &str,
) -> Result<(String, bool), DslError> {
    let chars = arg.char_indices().collect::<Vec<_>>();
    let mut output = String::with_capacity(arg.len());
    let mut index = 0usize;
    let mut changed = false;

    while index < chars.len() {
        let (byte_idx, ch) = chars[index];
        if ch != '$' {
            output.push(ch);
            index += 1;
            continue;
        }

        let Some((_, next_ch)) = chars.get(index + 1).copied() else {
            output.push('$');
            index += 1;
            continue;
        };

        if next_ch == '{' {
            let start_column = byte_idx + 1;
            let mut end_index = index + 2;
            while end_index < chars.len() && chars[end_index].1 != '}' {
                end_index += 1;
            }
            if end_index == chars.len() {
                return Err(DslError::InvalidValue(format!(
                    "unclosed pipeline placeholder in '{arg}'"
                ))
                .at_line_column(0, Some(start_column)));
            }
            let name_start = chars[index + 2].0;
            let name_end = chars[end_index].0;
            let key = arg[name_start..name_end].trim();
            let value = bindings.get(key).ok_or_else(|| {
                let names = bindings.keys().cloned().collect::<Vec<_>>();
                DslError::InvalidValue(pipeline_unknown_placeholder_message(context, key, &names))
                    .at_line_column(0, Some(start_column + 2))
            })?;
            output.push_str(value);
            changed = true;
            index = end_index + 1;
            continue;
        }

        if is_pipeline_placeholder_char(next_ch) {
            let start_index = index + 1;
            let mut end_index = start_index + 1;
            while end_index < chars.len() && is_pipeline_placeholder_char(chars[end_index].1) {
                end_index += 1;
            }
            let name_start = chars[start_index].0;
            let name_end = chars
                .get(end_index)
                .map(|(pos, _)| *pos)
                .unwrap_or_else(|| arg.len());
            let key = &arg[name_start..name_end];
            let value = bindings.get(key).ok_or_else(|| {
                let names = bindings.keys().cloned().collect::<Vec<_>>();
                DslError::InvalidValue(pipeline_unknown_placeholder_message(context, key, &names))
                    .at_line_column(0, Some(byte_idx + 2))
            })?;
            output.push_str(value);
            changed = true;
            index = end_index;
            continue;
        }

        output.push('$');
        index += 1;
    }

    Ok((output, changed))
}

fn is_pipeline_placeholder_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-')
}

fn known_pipeline_params_message(function_name: &str, function: &PipelineFunction) -> String {
    if function.params.is_empty() {
        return pipeline_declared_params_message(
            &format_pipeline_function_signature(function_name, &function.params),
            &[],
        );
    }
    let signature = format_pipeline_function_signature(function_name, &function.params);
    let known = function
        .params
        .iter()
        .map(|param| param.name.clone())
        .collect::<Vec<_>>();
    pipeline_declared_params_message(&signature, &known)
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

fn canonicalize_pipeline_rule_keywords(
    keywords: BTreeMap<String, PipelineKeywordArg>,
    reason_rule: bool,
    call_column: usize,
) -> Result<BTreeMap<String, PipelineKeywordArg>, DslError> {
    let mut canonical = BTreeMap::new();
    for (key, value) in keywords {
        let normalized = match key.as_str() {
            "predicate" | "pred" => "predicate",
            "stage" => "stage",
            "key_event" | "event" if reason_rule => "key_event",
            "narrative" | "narr" => "narrative",
            "dedupe" => "dedupe",
            "module" | "mod" => "module",
            "phase" => "phase",
            _ => key.as_str(),
        };
        if canonical.insert(normalized.to_string(), value).is_some() {
            return Err(DslError::InvalidValue(format!(
                "pipeline rule received duplicate field '{normalized}'"
            ))
            .at_line_column(0, Some(call_column)));
        }
    }
    Ok(canonical)
}

fn normalize_pipeline_rule_args(
    args: &[String],
    arg_columns: &[usize],
    reason_rule: bool,
    call_column: usize,
) -> Result<BTreeMap<String, PipelineKeywordArg>, DslError> {
    let signal_key = if reason_rule { "key_event" } else { "stage" };
    let step = if reason_rule {
        "reason_rule"
    } else {
        "program_rule"
    };
    let positional_limit = 4usize;
    let has_keywords = args.iter().any(|arg| looks_like_pipeline_keyword_arg(arg));
    if !has_keywords {
        if args.len() != positional_limit {
            return Err(DslError::InvalidValue(format!(
                "pipeline step '{step}' positional shorthand expects exactly 4 arguments"
            ))
            .at_line_column(0, Some(call_column)));
        }
        let mut keywords = BTreeMap::new();
        let fields = ["predicate", signal_key, "narrative", "dedupe"];
        for ((field, arg), arg_column) in fields.iter().zip(args.iter()).zip(arg_columns.iter()) {
            keywords.insert(
                (*field).to_string(),
                PipelineKeywordArg {
                    value: parse_pipeline_literal(arg),
                    value_column: *arg_column,
                },
            );
        }
        return Ok(keywords);
    }

    let positional_count = args
        .iter()
        .take_while(|arg| !looks_like_pipeline_keyword_arg(arg))
        .count();
    if positional_count > positional_limit {
        return Err(DslError::InvalidValue(format!(
            "pipeline step '{step}' positional shorthand accepts at most 4 leading arguments"
        ))
        .at_line_column(0, Some(call_column)));
    }
    if args
        .iter()
        .skip(positional_count)
        .any(|arg| !looks_like_pipeline_keyword_arg(arg))
    {
        return Err(DslError::InvalidValue(format!(
            "pipeline step '{step}' cannot place positional arguments after named arguments"
        ))
        .at_line_column(0, Some(call_column)));
    }

    let mut keywords = BTreeMap::new();
    let fields = ["predicate", signal_key, "narrative", "dedupe"];
    for ((field, arg), arg_column) in fields
        .iter()
        .zip(args.iter().take(positional_count))
        .zip(arg_columns.iter().take(positional_count))
    {
        keywords.insert(
            (*field).to_string(),
            PipelineKeywordArg {
                value: parse_pipeline_literal(arg),
                value_column: *arg_column,
            },
        );
    }
    let remaining_keywords = parse_pipeline_keywords_with_columns(
        &args[positional_count..],
        &arg_columns[positional_count..],
        step,
    )?;
    for (key, value) in remaining_keywords {
        if keywords.insert(key, value).is_some() {
            return Err(DslError::InvalidValue(
                format!("pipeline step '{step}' received duplicate rule field")
            )
            .at_line_column(0, Some(call_column)));
        }
    }
    canonicalize_pipeline_rule_keywords(keywords, reason_rule, call_column)
}

pub(crate) fn lower_pipeline_window(
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

pub(crate) fn lower_pipeline_param(args: &[String]) -> Result<String, DslError> {
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

pub(crate) fn lower_pipeline_evidence(args: &[String]) -> Result<String, DslError> {
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

pub(crate) fn lower_pipeline_rule(
    args: &[String],
    arg_columns: &[usize],
    call_column: usize,
    reason_rule: bool,
) -> Result<String, DslError> {
    let keywords = normalize_pipeline_rule_args(args, arg_columns, reason_rule, call_column)?;
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
    crate::dsl::parse_flow_predicate(&predicate.value)
        .map_err(|err| err.reanchor_line_column(0, predicate.value_column))?;
    if reason_rule {
        crate::dsl::parse_reason_key_event(&signal.value)
            .map_err(|err| err.reanchor_line_column(0, signal.value_column))?;
    } else {
        legacy::parse_stage(&signal.value)
            .map_err(|err| err.reanchor_line_column(0, signal.value_column))?;
    }
    crate::dsl::parse_bool(&dedupe.value)
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
