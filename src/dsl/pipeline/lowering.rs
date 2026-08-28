use super::parsing::{
    is_pipeline_identifier, parse_pipeline_literal, parse_pipeline_literal_cow,
    parse_pipeline_single_arg, parse_pipeline_use_call,
};
use super::{PipelineKeywordArg, PipelineUseCall, looks_like_pipeline_keyword_arg};
use crate::dsl::{
    DslError, PipelineCall, PipelineFunction, PipelineModule,
    diagnostics::{
        pipeline_available_steps_message, pipeline_declared_functions_message,
        pipeline_declared_params_message, pipeline_unknown_placeholder_message,
    },
    function_types::{format_pipeline_function_signature, validate_pipeline_param_value_kind},
    legacy::{self, CanonicalAssignment, CanonicalAssignmentValue},
    predicate::{parse_narrative_template, parse_reason_narrative},
};
use crate::fragment::EvidenceTier;
use crate::ledger::FactKindTag;
use crate::program::ProgramRule;
use crate::reason::{ReasonProfile, ReasonRule};
use crate::template::FragmentParamValue;
use std::collections::BTreeMap;

pub(crate) fn lower_pipeline_module_to_assignments(
    module: &PipelineModule,
    allow_template_head: bool,
) -> Result<Vec<CanonicalAssignment>, DslError> {
    let mut output = Vec::new();
    if let Some(template) = &module.template {
        output.push(CanonicalAssignment::new(
            CanonicalAssignmentValue::Template(
                parse_pipeline_single_arg(&template.args, "template")
                    .map_err(|err| err.at_line(template.line_no))?,
            ),
            template.line_no,
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
    Ok(output)
}

fn lower_pipeline_calls(
    calls: &[PipelineCall],
    module: &PipelineModule,
    output: &mut Vec<CanonicalAssignment>,
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
    output: &mut Vec<CanonicalAssignment>,
    allow_template_head: bool,
    use_stack: &mut Vec<String>,
    bindings: &BTreeMap<String, String>,
    scope_context: &str,
) -> Result<(), DslError> {
    let line_no = call.line_no;
    let column_no = call.column_no;
    let resolved_args = if call.args.iter().any(|arg| arg.contains('$')) {
        let call_context = format!("{} while expanding {}", call.name, scope_context);
        Some(
            call.args
                .iter()
                .map(|arg| substitute_pipeline_arg(arg, bindings, &call_context))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|err| err.reanchor_line_column(line_no, column_no))?,
        )
    } else {
        None
    };
    let args = resolved_args.as_deref().unwrap_or(&call.args);
    match call.name.as_str() {
        "template" => {
            if !allow_template_head {
                return Err(DslError::InvalidValue(
                    "pipeline DSL supports exactly one template head".into(),
                )
                .at_line(line_no));
            }
            output.push(CanonicalAssignment::new(
                CanonicalAssignmentValue::Template(
                    parse_pipeline_single_arg(args, "template")
                        .map_err(|err| err.reanchor_line_column(line_no, column_no))?,
                ),
                line_no,
            ));
        }
        "use" => {
            let use_call = parse_pipeline_use_call(args)
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
                function_bindings.insert(
                    binding.name.clone(),
                    parse_pipeline_literal(&resolved)
                        .map_err(|err| err.reanchor_line_column(line_no, column_no))?,
                );
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
        "window" => lower_pipeline_window(args, &call.arg_columns, column_no, line_no, output)
            .map_err(|err| err.at_line(line_no))?,
        "reason" => {
            let id = parse_pipeline_single_arg(args, "reason")
                .map_err(|err| err.reanchor_line_column(line_no, column_no))?;
            let profile = ReasonProfile::from_id(&id).ok_or_else(|| {
                DslError::InvalidValue(format!("unknown reason profile '{id}'")).at_line_column(
                    line_no,
                    call.arg_columns.first().copied().or(Some(column_no)),
                )
            })?;
            output.push(CanonicalAssignment::new(
                CanonicalAssignmentValue::Reason(profile),
                line_no,
            ));
        }
        "reason_model" => output.push(CanonicalAssignment::new(
            CanonicalAssignmentValue::ReasonModel(
                parse_pipeline_single_arg(args, "reason_model")
                    .map_err(|err| err.reanchor_line_column(line_no, column_no))?,
            ),
            line_no,
        )),
        "fragment" => output.push(CanonicalAssignment::new(
            CanonicalAssignmentValue::Fragment(
                parse_pipeline_single_arg(args, "fragment")
                    .map_err(|err| err.reanchor_line_column(line_no, column_no))?,
            ),
            line_no,
        )),
        "program_model" => output.push(CanonicalAssignment::new(
            CanonicalAssignmentValue::ProgramModel(
                parse_pipeline_single_arg(args, "program_model")
                    .map_err(|err| err.reanchor_line_column(line_no, column_no))?,
            ),
            line_no,
        )),
        "operation" => {
            let value = parse_pipeline_single_arg(args, "operation")
                .map_err(|err| err.reanchor_line_column(line_no, column_no))?;
            output.push(CanonicalAssignment::new(
                CanonicalAssignmentValue::Operation(legacy::parse_operation(&value)),
                line_no,
            ));
        }
        "param" => output.push(CanonicalAssignment::new(
            lower_pipeline_param(args)
                .map_err(|err| err.reanchor_line_column(line_no, column_no))?,
            line_no,
        )),
        "evidence" => output.push(CanonicalAssignment::new(
            lower_pipeline_evidence(args)
                .map_err(|err| err.reanchor_line_column(line_no, column_no))?,
            line_no,
        )),
        "program_rule" => output.push(CanonicalAssignment::new(
            CanonicalAssignmentValue::ProgramRule(
                lower_pipeline_program_rule(args, &call.arg_columns, column_no)
                    .map_err(|err| err.at_line(line_no))?,
            ),
            line_no,
        )),
        "reason_rule" => output.push(CanonicalAssignment::new(
            CanonicalAssignmentValue::ReasonRule(
                lower_pipeline_reason_rule(args, &call.arg_columns, column_no)
                    .map_err(|err| err.at_line(line_no))?,
            ),
            line_no,
        )),
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
    if let Some(unknown_name) = use_call.named_args.keys().find(|name| {
        !function
            .params
            .iter()
            .any(|param| param.name == name.as_str())
    }) {
        return Err(DslError::InvalidValue(format!(
            "pipeline function call does not match {signature}: unknown named parameter '{unknown_name}'. {}",
            known_pipeline_params_message(function_name, function)
        ))
        .at_line_column(0, Some(1)));
    }
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
        bindings.insert(param.name.clone(), parse_pipeline_literal(&resolved)?);
    }
    Ok(bindings)
}

pub(crate) fn substitute_pipeline_arg(
    arg: &str,
    bindings: &BTreeMap<String, String>,
    context: &str,
) -> Result<String, DslError> {
    if !arg.contains('$') {
        return Ok(arg.to_string());
    }
    let (mut current, changed) = substitute_pipeline_arg_once(arg, bindings, context)?;
    if !changed || !current.contains('$') {
        return Ok(current);
    }
    let mut iterations = 1usize;
    loop {
        let (next, changed) = substitute_pipeline_arg_once(&current, bindings, context)?;
        current = next;
        if !changed || !current.contains('$') {
            return Ok(current);
        }
        iterations += 1;
        if iterations > 32 {
            return Err(DslError::InvalidValue(format!(
                "pipeline placeholder expansion exceeded 32 substitutions while {context}"
            ))
            .at_line_column(0, Some(1)));
        }
    }
}

fn substitute_pipeline_arg_once(
    arg: &str,
    bindings: &BTreeMap<String, String>,
    context: &str,
) -> Result<(String, bool), DslError> {
    let bytes = arg.as_bytes();
    let mut output = String::with_capacity(arg.len());
    let mut index = 0usize;
    let mut copy_start = 0usize;
    let mut changed = false;
    let mut in_string = false;
    let mut escaped = false;

    while index < bytes.len() {
        let byte = bytes[index];
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            index += 1;
            continue;
        }
        if byte == b'"' {
            in_string = true;
            index += 1;
            continue;
        }
        if byte != b'$' {
            index += 1;
            continue;
        }

        let next = index + 1;
        let Some(&next_byte) = bytes.get(next) else {
            index += 1;
            continue;
        };

        if next_byte == b'{' {
            let start_column = index + 1;
            let name_start = next + 1;
            let Some(close_offset) = bytes[name_start..].iter().position(|byte| *byte == b'}')
            else {
                return Err(DslError::InvalidValue(format!(
                    "unclosed pipeline placeholder in '{arg}'"
                ))
                .at_line_column(0, Some(start_column)));
            };
            let name_end = name_start + close_offset;
            let key = arg[name_start..name_end].trim();
            let value = bindings.get(key).ok_or_else(|| {
                let names = bindings.keys().cloned().collect::<Vec<_>>();
                DslError::InvalidValue(pipeline_unknown_placeholder_message(context, key, &names))
                    .at_line_column(0, Some(start_column + 2))
            })?;
            output.push_str(&arg[copy_start..index]);
            output.push_str(value);
            changed = true;
            index = name_end + 1;
            copy_start = index;
            continue;
        }

        if is_pipeline_placeholder_byte(next_byte) {
            let mut end = next + 1;
            while end < bytes.len() && is_pipeline_placeholder_byte(bytes[end]) {
                end += 1;
            }
            let key = &arg[next..end];
            let value = bindings.get(key).ok_or_else(|| {
                let names = bindings.keys().cloned().collect::<Vec<_>>();
                DslError::InvalidValue(pipeline_unknown_placeholder_message(context, key, &names))
                    .at_line_column(0, Some(index + 2))
            })?;
            output.push_str(&arg[copy_start..index]);
            output.push_str(value);
            changed = true;
            index = end;
            copy_start = index;
            continue;
        }

        index += 1;
    }

    output.push_str(&arg[copy_start..]);
    Ok((output, changed))
}

fn is_pipeline_placeholder_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')
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

fn parse_pipeline_keywords_with_columns<'a>(
    args: &'a [String],
    arg_columns: &[usize],
    step: &str,
) -> Result<BTreeMap<&'a str, PipelineKeywordArg<'a>>, DslError> {
    let mut keywords = BTreeMap::new();
    for (arg, arg_column) in args.iter().zip(arg_columns.iter()) {
        let (key, value) = arg.split_once(':').ok_or_else(|| {
            DslError::InvalidValue(format!(
                "pipeline step '{step}' expected keyword argument, got '{arg}'"
            ))
            .at_line_column(0, Some(*arg_column))
        })?;
        let key = key.trim();
        if !is_pipeline_identifier(key) {
            return Err(DslError::InvalidValue(format!(
                "pipeline step '{step}' received invalid keyword field name '{key}'"
            ))
            .at_line_column(0, Some(*arg_column)));
        }
        if keywords.contains_key(key) {
            let message = if matches!(step, "program_rule" | "reason_rule") {
                format!("pipeline rule received duplicate field '{key}'")
            } else {
                format!("pipeline step '{step}' received duplicate field '{key}'")
            };
            return Err(DslError::InvalidValue(message).at_line_column(0, Some(*arg_column)));
        }
        let value_trimmed = value.trim();
        let value_offset = value.find(value_trimmed).unwrap_or(0);
        keywords.insert(
            key,
            PipelineKeywordArg {
                value: parse_pipeline_literal_cow(value)?,
                value_column: arg_column + key.len() + 1 + value_offset,
            },
        );
    }
    Ok(keywords)
}

fn canonicalize_pipeline_rule_keywords<'a>(
    keywords: BTreeMap<&str, PipelineKeywordArg<'a>>,
    reason_rule: bool,
    call_column: usize,
) -> Result<BTreeMap<&'static str, PipelineKeywordArg<'a>>, DslError> {
    let mut canonical = BTreeMap::new();
    for (key, value) in keywords {
        let normalized = match key {
            "predicate" | "pred" => "predicate",
            "stage" => "stage",
            "key_event" | "event" if reason_rule => "key_event",
            "narrative" | "narr" => "narrative",
            "dedupe" => "dedupe",
            "module" | "mod" => "module",
            "phase" => "phase",
            _ => {
                return Err(DslError::InvalidValue(format!(
                    "pipeline rule received unknown field '{key}'"
                ))
                .at_line_column(0, Some(value.value_column)));
            }
        };
        if canonical.insert(normalized, value).is_some() {
            return Err(DslError::InvalidValue(format!(
                "pipeline rule received duplicate field '{normalized}'"
            ))
            .at_line_column(0, Some(call_column)));
        }
    }
    Ok(canonical)
}

fn normalize_pipeline_rule_args<'a>(
    args: &'a [String],
    arg_columns: &[usize],
    reason_rule: bool,
    call_column: usize,
) -> Result<BTreeMap<&'static str, PipelineKeywordArg<'a>>, DslError> {
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
                *field,
                PipelineKeywordArg {
                    value: parse_pipeline_literal_cow(arg)?,
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
            *field,
            PipelineKeywordArg {
                value: parse_pipeline_literal_cow(arg)?,
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
            return Err(DslError::InvalidValue(format!(
                "pipeline step '{step}' received duplicate rule field"
            ))
            .at_line_column(0, Some(call_column)));
        }
    }
    canonicalize_pipeline_rule_keywords(keywords, reason_rule, call_column)
}

pub(crate) fn lower_pipeline_window(
    args: &[String],
    arg_columns: &[usize],
    call_column: usize,
    line_no: usize,
    output: &mut Vec<CanonicalAssignment>,
) -> Result<(), DslError> {
    if args.len() == 1 && !looks_like_pipeline_keyword_arg(&args[0]) {
        let id = parse_pipeline_literal_cow(&args[0])?;
        let value_column = arg_columns.first().copied().unwrap_or(call_column);
        output.push(CanonicalAssignment::new(
            CanonicalAssignmentValue::Window(
                legacy::parse_window_profile(&id)
                    .map_err(|err| err.at_line_column(0, Some(value_column)))?,
            ),
            line_no,
        ));
        return Ok(());
    }
    let keywords = parse_pipeline_keywords_with_columns(args, arg_columns, "window")?;
    if let Some((field, value)) = keywords
        .iter()
        .find(|(field, _)| !matches!(**field, "duration_ms" | "lateness_ms"))
    {
        return Err(DslError::InvalidValue(format!(
            "pipeline step 'window' received unknown field '{field}'"
        ))
        .at_line_column(0, Some(value.value_column)));
    }
    let duration_ms = keywords
        .get("duration_ms")
        .ok_or(DslError::MissingField("duration_ms").at_line_column(0, Some(call_column)))?;
    let lateness_ms = keywords
        .get("lateness_ms")
        .ok_or(DslError::MissingField("lateness_ms").at_line_column(0, Some(call_column)))?;
    output.push(CanonicalAssignment::new(
        CanonicalAssignmentValue::WindowDuration(parse_pipeline_u64(duration_ms, "duration_ms")?),
        line_no,
    ));
    output.push(CanonicalAssignment::new(
        CanonicalAssignmentValue::WindowLateness(parse_pipeline_u64(lateness_ms, "lateness_ms")?),
        line_no,
    ));
    Ok(())
}

fn parse_pipeline_u64(arg: &PipelineKeywordArg<'_>, field: &str) -> Result<u64, DslError> {
    arg.value.parse::<u64>().map_err(|_| {
        DslError::InvalidValue(format!("invalid u64 for '{field}': '{}'", arg.value))
            .at_line_column(0, Some(arg.value_column))
    })
}

pub(crate) fn lower_pipeline_param(args: &[String]) -> Result<CanonicalAssignmentValue, DslError> {
    if args.len() != 2 {
        return Err(DslError::InvalidValue(
            "pipeline step 'param' expects target and value".into(),
        )
        .at_line_column(0, Some(1)));
    }
    let target = parse_pipeline_literal_cow(&args[0])?;
    let (fragment_id, key) = target
        .split_once('.')
        .ok_or_else(|| DslError::InvalidValue(format!("invalid param target '{target}'")))?;
    let value = parse_pipeline_literal_cow(&args[1])?;
    let value = if matches!(value.as_ref(), "true" | "false") {
        FragmentParamValue::Bool(crate::dsl::parse_bool(value.as_ref())?)
    } else if let Ok(value) = value.parse::<u64>() {
        FragmentParamValue::U64(value)
    } else {
        FragmentParamValue::String(value.into_owned())
    };
    Ok(CanonicalAssignmentValue::FragmentParam {
        fragment_id: fragment_id.trim().to_string(),
        key: key.trim().to_string(),
        value,
    })
}

pub(crate) fn lower_pipeline_evidence(
    args: &[String],
) -> Result<CanonicalAssignmentValue, DslError> {
    if args.len() != 2 {
        return Err(DslError::InvalidValue(
            "pipeline step 'evidence' expects fact kind and tier".into(),
        )
        .at_line_column(0, Some(1)));
    }
    let fact_kind_id = parse_pipeline_literal_cow(&args[0])?;
    let fact_kind = FactKindTag::from_str(fact_kind_id.as_ref()).ok_or_else(|| {
        DslError::InvalidValue(format!("unknown evidence fact kind '{fact_kind_id}'"))
    })?;
    let tier_id = parse_pipeline_literal_cow(&args[1])?;
    let tier = match tier_id.as_ref() {
        "core_requirement" => EvidenceTier::CoreRequirement,
        "optional_enhancement" => EvidenceTier::OptionalEnhancement,
        other => {
            return Err(DslError::InvalidValue(format!(
                "unknown evidence tier '{other}'"
            )));
        }
    };
    Ok(CanonicalAssignmentValue::EvidenceOverride { fact_kind, tier })
}

pub(crate) fn lower_pipeline_program_rule(
    args: &[String],
    arg_columns: &[usize],
    call_column: usize,
) -> Result<ProgramRule, DslError> {
    let keywords = normalize_pipeline_rule_args(args, arg_columns, false, call_column)?;
    let predicate = keywords
        .get("predicate")
        .ok_or(DslError::MissingField("predicate").at_line_column(0, Some(call_column)))?;
    let signal = keywords
        .get("stage")
        .ok_or(DslError::MissingField("stage").at_line_column(0, Some(call_column)))?;
    let narrative = keywords
        .get("narrative")
        .ok_or(DslError::MissingField("narrative").at_line_column(0, Some(call_column)))?;
    let dedupe = keywords
        .get("dedupe")
        .ok_or(DslError::MissingField("dedupe").at_line_column(0, Some(call_column)))?;
    let (module, phase) = lower_rule_scope(&keywords)?;
    Ok(ProgramRule {
        predicate: crate::dsl::parse_flow_predicate(predicate.value.as_ref())
            .map_err(|err| err.reanchor_line_column(0, predicate.value_column))?,
        signal: legacy::parse_stage(signal.value.as_ref())
            .map_err(|err| err.reanchor_line_column(0, signal.value_column))?,
        narrative: parse_narrative_template(narrative.value.as_ref()),
        dedupe: crate::dsl::parse_bool(dedupe.value.as_ref())
            .map_err(|err| err.reanchor_line_column(0, dedupe.value_column))?,
        module,
        phase,
    })
}

pub(crate) fn lower_pipeline_reason_rule(
    args: &[String],
    arg_columns: &[usize],
    call_column: usize,
) -> Result<ReasonRule, DslError> {
    let keywords = normalize_pipeline_rule_args(args, arg_columns, true, call_column)?;
    let predicate = keywords
        .get("predicate")
        .ok_or(DslError::MissingField("predicate").at_line_column(0, Some(call_column)))?;
    let key_event = keywords
        .get("key_event")
        .ok_or(DslError::MissingField("key_event").at_line_column(0, Some(call_column)))?;
    let narrative = keywords
        .get("narrative")
        .ok_or(DslError::MissingField("narrative").at_line_column(0, Some(call_column)))?;
    let dedupe = keywords
        .get("dedupe")
        .ok_or(DslError::MissingField("dedupe").at_line_column(0, Some(call_column)))?;
    let (module, phase) = lower_rule_scope(&keywords)?;
    Ok(ReasonRule {
        predicate: crate::dsl::parse_flow_predicate(predicate.value.as_ref())
            .map_err(|err| err.reanchor_line_column(0, predicate.value_column))?,
        signal: crate::dsl::parse_reason_key_event(key_event.value.as_ref())
            .map_err(|err| err.reanchor_line_column(0, key_event.value_column))?,
        narrative: parse_reason_narrative(narrative.value.as_ref()),
        dedupe: crate::dsl::parse_bool(dedupe.value.as_ref())
            .map_err(|err| err.reanchor_line_column(0, dedupe.value_column))?,
        module,
        phase,
    })
}

fn lower_rule_scope(
    keywords: &BTreeMap<&'static str, PipelineKeywordArg<'_>>,
) -> Result<(Option<String>, Option<String>), DslError> {
    let module = keywords
        .get("module")
        .map(|value| value.value.as_ref().to_string());
    let phase = keywords
        .get("phase")
        .map(|value| value.value.as_ref().to_string());
    if module.is_none()
        && let Some(phase) = keywords.get("phase")
    {
        return Err(DslError::InvalidValue(format!(
            "pipeline rule phase '{}' requires module",
            phase.value
        ))
        .at_line_column(0, Some(phase.value_column)));
    }
    Ok((module, phase))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholder_expansion_limit_rejects_partially_resolved_output() {
        let mut bindings = BTreeMap::new();
        for index in 0..34 {
            bindings.insert(format!("p{index}"), format!("$p{}", index + 1));
        }
        bindings.insert("p34".to_string(), "done".to_string());

        let err = substitute_pipeline_arg("$p0", &bindings, "test expansion")
            .expect_err("deep placeholder chains must fail closed");
        assert!(matches!(
            err.root(),
            DslError::InvalidValue(message)
                if message == "pipeline placeholder expansion exceeded 32 substitutions while test expansion"
        ));
    }

    #[test]
    fn placeholder_substitution_treats_strings_as_opaque() {
        let bindings = BTreeMap::from([("name".to_string(), "resolved".to_string())]);
        assert_eq!(
            substitute_pipeline_arg(r#""literal $name costs $5""#, &bindings, "test").unwrap(),
            r#""literal $name costs $5""#
        );
        assert_eq!(
            substitute_pipeline_arg("$name", &bindings, "test").unwrap(),
            "resolved"
        );
        assert_eq!(
            substitute_pipeline_arg("前-${name}-${ name }-后", &bindings, "test").unwrap(),
            "前-resolved-resolved-后"
        );
    }

    #[test]
    fn rule_aliases_share_the_same_duplicate_slot() {
        let args = vec![
            "pred: :process_bound".to_string(),
            "predicate: :process_bound".to_string(),
            "stage: :process_bound".to_string(),
            "narrative: :process_bound".to_string(),
            "dedupe: true".to_string(),
        ];
        let err = lower_pipeline_program_rule(&args, &[1; 5], 1)
            .expect_err("canonical aliases must still be rejected as duplicates");
        assert!(matches!(
            err.root(),
            DslError::InvalidValue(message)
                if message == "pipeline rule received duplicate field 'predicate'"
        ));
    }
}
