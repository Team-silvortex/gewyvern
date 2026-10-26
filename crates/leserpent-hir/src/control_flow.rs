use super::*;
use leselang_syntax::NamedArgument;

fn invalid(message: impl Into<String>, span: Span) -> Vec<Diagnostic> {
    vec![Diagnostic {
        code: "LSH1301".to_string(),
        message: message.into(),
        span: Some(span),
    }]
}

pub(super) fn lower_sequence(
    arguments: &[NamedArgument],
    span: Span,
) -> Result<LoweredEffect, Vec<Diagnostic>> {
    lower_sequence_with(arguments, span, &mut lower_effect)
}

pub(super) fn lower_sequence_with<'a>(
    arguments: &'a [NamedArgument],
    span: Span,
    lower: &mut impl FnMut(&'a Expression) -> Result<LoweredEffect, Vec<Diagnostic>>,
) -> Result<LoweredEffect, Vec<Diagnostic>> {
    if arguments.is_empty() || arguments.len() > MAX_SEQUENCE_STEPS {
        return Err(invalid("seq requires between 1 and 64 named steps", span));
    }
    let mut steps = Vec::new();
    let mut names = HashSet::new();
    for argument in arguments {
        if !names.insert(&argument.name) {
            return Err(invalid(
                format!("duplicate seq step '{}'", argument.name),
                argument.span,
            ));
        }
        let lowered = lower(&argument.value)?;
        append_steps(&mut steps, &argument.name, lowered, argument.span)?;
    }
    finish(steps, span)
}

pub(super) fn lower_repeat(
    arguments: &[NamedArgument],
    span: Span,
) -> Result<LoweredEffect, Vec<Diagnostic>> {
    lower_repeat_with(arguments, span, &mut lower_effect)
}

pub(super) fn lower_repeat_with<'a>(
    arguments: &'a [NamedArgument],
    span: Span,
    lower: &mut impl FnMut(&'a Expression) -> Result<LoweredEffect, Vec<Diagnostic>>,
) -> Result<LoweredEffect, Vec<Diagnostic>> {
    if arguments.len() != 2 {
        return Err(invalid("repeat requires exactly 'times' and 'body'", span));
    }
    let count = arguments.iter().find(|argument| argument.name == "times");
    let body = arguments.iter().find(|argument| argument.name == "body");
    let (Some(count), Some(body)) = (count, body) else {
        return Err(invalid("repeat requires exactly 'times' and 'body'", span));
    };
    let Expression::Integer { value, .. } = count.value else {
        return Err(invalid(
            "repeat times must be an integer from 1 through 64",
            count.span,
        ));
    };
    if !(1..=MAX_SEQUENCE_STEPS as u64).contains(&value) {
        return Err(invalid(
            "repeat times must be an integer from 1 through 64",
            count.span,
        ));
    }
    let lowered = lower(&body.value)?;
    let width = match &lowered.effect {
        Effect::Sequence { steps } => steps.len(),
        _ => 1,
    };
    if width.saturating_mul(value as usize) > MAX_SEQUENCE_STEPS {
        return Err(invalid("expanded control flow exceeds 64 effects", span));
    }
    computed_group::validate_repeat_expansion(&lowered.effect, value as usize, span)?;
    let mut steps = Vec::new();
    for index in 1..=value {
        append_steps(
            &mut steps,
            &format!("iteration_{index}"),
            LoweredEffect {
                effect: lowered.effect.clone(),
                result_type: lowered.result_type,
                required_capabilities: Vec::new(),
            },
            body.span,
        )?;
    }
    finish(steps, span)
}

fn append_steps(
    steps: &mut Vec<HirBranch>,
    name: &str,
    lowered: LoweredEffect,
    span: Span,
) -> Result<(), Vec<Diagnostic>> {
    match lowered.effect {
        Effect::Compute { ref expression } if expression.prepared_atomic_operation().is_none() => {
            return Err(invalid(
                "seq/repeat steps require prepared atomic calls, not result flows",
                span,
            ));
        }
        Effect::All { .. } => {
            return Err(invalid(
                "all cannot be nested inside sequential control flow",
                span,
            ));
        }
        Effect::Sequence { steps: nested } => {
            if steps.len().saturating_add(nested.len()) > MAX_SEQUENCE_STEPS {
                return Err(invalid("expanded control flow exceeds 64 effects", span));
            }
            for mut step in nested {
                step.name = format!("{name}__{}", step.name);
                steps.push(step);
            }
        }
        effect => {
            if steps.len() == MAX_SEQUENCE_STEPS {
                return Err(invalid("expanded control flow exceeds 64 effects", span));
            }
            steps.push(HirBranch {
                name: name.to_string(),
                effect,
                result_type: lowered.result_type,
            });
        }
    }
    Ok(())
}

fn finish(steps: Vec<HirBranch>, span: Span) -> Result<LoweredEffect, Vec<Diagnostic>> {
    let mut names = HashSet::new();
    for step in &steps {
        if step.name.len() > MAX_BRANCH_NAME_BYTES || !names.insert(&step.name) {
            return Err(invalid(
                "expanded step names must be unique and at most 64 bytes",
                span,
            ));
        }
    }
    let effect = Effect::Sequence { steps };
    let required_capabilities = required_capabilities_for_effect(&effect)
        .into_iter()
        .map(str::to_string)
        .collect();
    Ok(LoweredEffect {
        effect,
        result_type: Type::Structured,
        required_capabilities,
    })
}
