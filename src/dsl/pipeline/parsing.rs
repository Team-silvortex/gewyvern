use super::{PipelineProvidedArg, PipelineUseCall, looks_like_pipeline_keyword_arg};
use crate::dsl::{
    DslError, PipelineCall, PipelineLetBinding, PipelineModule, PipelineParam, frontend,
    function_types::parse_pipeline_value_kind_name,
};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn push_pipeline_function_call(
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

pub(crate) fn parse_pipeline_let_binding(
    line: &str,
) -> Result<Option<PipelineLetBinding>, DslError> {
    let Some(remainder) = line.strip_prefix("let ") else {
        return Ok(None);
    };
    validate_pipeline_string_delimiters(line)?;
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

pub(crate) fn parse_pipeline_call(
    line: &str,
) -> Result<(String, Vec<String>, Vec<usize>), DslError> {
    validate_pipeline_string_delimiters(line)?;
    if !line.contains('(') {
        return parse_pipeline_parenless_call(line);
    }
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

fn parse_pipeline_parenless_call(
    line: &str,
) -> Result<(String, Vec<String>, Vec<usize>), DslError> {
    let line = line.trim();
    let split_at = line.find(char::is_whitespace).ok_or_else(|| {
        DslError::InvalidValue(format!("invalid pipeline call '{line}'"))
            .at_line_column(0, Some(line.len() + 1))
    })?;
    let name = line[..split_at].trim();
    let arg = line[split_at..].trim();
    if name.is_empty() || arg.is_empty() {
        return Err(
            DslError::InvalidValue(format!("invalid pipeline call '{line}'"))
                .at_line_column(0, Some(line.len() + 1)),
        );
    }
    let arg_column = split_at + line[split_at..].find(arg).unwrap_or(0) + 1;
    let args_with_columns = split_pipeline_args_with_columns(arg, arg_column);
    if args_with_columns.is_empty() {
        return Err(
            DslError::InvalidValue(format!("invalid pipeline call '{line}'"))
                .at_line_column(0, Some(line.len() + 1)),
        );
    }
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

pub(crate) fn parse_pipeline_function_signature(
    signature: &str,
) -> Result<(String, Vec<PipelineParam>), DslError> {
    validate_pipeline_string_delimiters(signature)?;
    let open = signature.find('(').ok_or_else(|| {
        DslError::InvalidValue(format!("invalid function signature '{signature}'"))
            .at_line_column(0, Some(signature.len() + 1))
    })?;
    let close = signature.rfind(')').ok_or_else(|| {
        DslError::InvalidValue(format!("invalid function signature '{signature}'"))
            .at_line_column(0, Some(signature.len() + 1))
    })?;
    if close < open || close + 1 != signature.len() {
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
    if !is_pipeline_identifier(name) {
        return Err(
            DslError::InvalidValue(format!("invalid pipeline function name '{name}'"))
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
    let mut param_names = BTreeSet::new();
    if let Some(duplicate) = params
        .iter()
        .find(|param| !param_names.insert(param.name.as_str()))
    {
        return Err(DslError::InvalidValue(format!(
            "duplicate pipeline parameter '{}'",
            duplicate.name
        ))
        .at_line_column(0, Some(open + 2)));
    }
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
    let name_src = name_src.trim();
    let (name_src, declared_kind) =
        if let Some((candidate_name, candidate_kind)) = name_src.rsplit_once(':') {
            if !candidate_name.trim().is_empty() {
                (
                    candidate_name,
                    Some(
                        parse_pipeline_value_kind_name(candidate_kind.trim())
                            .map_err(|err| err.at_line_column(0, Some(trimmed.len() + 1)))?,
                    ),
                )
            } else {
                (name_src, None)
            }
        } else {
            (name_src, None)
        };
    Ok(PipelineParam {
        name: parse_pipeline_param_name(name_src)?,
        default_value,
        declared_kind,
        inferred_kind: None,
    })
}

pub(crate) fn parse_pipeline_param_name(param: &str) -> Result<String, DslError> {
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
    if !is_pipeline_identifier(&value) {
        return Err(
            DslError::InvalidValue(format!("invalid pipeline parameter name '{value}'"))
                .at_line_column(0, Some(1)),
        );
    }
    Ok(value)
}

fn is_pipeline_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '_')
        && chars.all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-'))
}

fn validate_pipeline_string_delimiters(input: &str) -> Result<(), DslError> {
    let mut in_string = false;
    let mut escaped = false;
    let mut opening_column = 1usize;
    for (index, ch) in input.char_indices() {
        if escaped {
            if !matches!(ch, '"' | '\\' | 'n' | 'r' | 't') {
                return Err(DslError::InvalidValue(format!(
                    "invalid pipeline string escape '\\{ch}'"
                ))
                .at_line_column(0, Some(index)));
            }
            escaped = false;
            continue;
        }
        if in_string && ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == '"' {
            in_string = !in_string;
            if in_string {
                opening_column = index + 1;
            }
        }
    }
    if in_string {
        return Err(
            DslError::InvalidValue("unclosed pipeline string literal".into())
                .at_line_column(0, Some(opening_column)),
        );
    }
    Ok(())
}

pub(crate) fn split_pipeline_args(input: &str) -> Vec<String> {
    split_pipeline_args_with_columns(input, 1)
        .into_iter()
        .map(|(_, value)| value)
        .collect()
}

fn split_pipeline_args_with_columns(input: &str, base_column: usize) -> Vec<(usize, String)> {
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (idx, ch) in input.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' if in_string => escaped = true,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escaped_quotes_and_commas_stay_inside_one_pipeline_argument() {
        let (_, args, _) = parse_pipeline_call(r#"reason_model("static:said \"hello, world\"")"#)
            .expect("escaped quotes must not terminate the argument");
        assert_eq!(args, vec![r#""static:said \"hello, world\"""#]);
    }
}

pub(crate) fn parse_pipeline_literal(value: &str) -> Result<String, DslError> {
    let value = value.trim();
    if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
        decode_pipeline_string_literal(&value[1..value.len() - 1])
    } else if let Some(atom) = value.strip_prefix(':') {
        Ok(atom.trim().to_string())
    } else {
        Ok(value.to_string())
    }
}

fn decode_pipeline_string_literal(value: &str) -> Result<String, DslError> {
    let mut decoded = String::with_capacity(value.len());
    let mut chars = value.char_indices();
    while let Some((index, ch)) = chars.next() {
        if ch != '\\' {
            decoded.push(ch);
            continue;
        }
        let Some((_, escaped)) = chars.next() else {
            return Err(DslError::InvalidValue(
                "pipeline string literal cannot end with an escape".into(),
            )
            .at_line_column(0, Some(index + 2)));
        };
        decoded.push(match escaped {
            '"' => '"',
            '\\' => '\\',
            'n' => '\n',
            'r' => '\r',
            't' => '\t',
            other => {
                return Err(DslError::InvalidValue(format!(
                    "invalid pipeline string escape '\\{other}'"
                ))
                .at_line_column(0, Some(index + 2)));
            }
        });
    }
    Ok(decoded)
}

pub(crate) fn parse_pipeline_single_arg(args: &[String], step: &str) -> Result<String, DslError> {
    if args.len() != 1 {
        return Err(DslError::InvalidValue(format!(
            "pipeline step '{step}' expects exactly one argument"
        ))
        .at_line_column(0, Some(1)));
    }
    parse_pipeline_literal(&args[0])
}

pub(crate) fn parse_pipeline_use_call(args: &[String]) -> Result<PipelineUseCall, DslError> {
    if args.is_empty() {
        return Err(DslError::InvalidValue(
            "pipeline step 'use' expects at least one argument".into(),
        )
        .at_line_column(0, Some(1)));
    }
    let function_name = parse_pipeline_literal(&args[0])?;
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
                    value: parse_pipeline_literal(value)?,
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
                value: parse_pipeline_literal(arg)?,
            });
        }
    }

    Ok(PipelineUseCall {
        function_name,
        positional_args,
        named_args,
    })
}
