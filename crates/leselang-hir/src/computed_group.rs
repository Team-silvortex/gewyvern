use super::*;
use crate::computation::{Computation, ComputedBranch, GroupKind, MAX_COMPUTATION_NODES};
use crate::host_call::HostOperation;
use leselang_syntax::NamedArgument;

fn invalid(message: &str, span: Span) -> Vec<Diagnostic> {
    vec![Diagnostic {
        code: "LSH1406".into(),
        message: message.into(),
        span: Some(span),
    }]
}

pub(super) fn lower<'a>(
    callee: &str,
    arguments: &'a [NamedArgument],
    span: Span,
    scope: &mut computation::TypeScope<'_, 'a>,
    visited: &mut usize,
    depth: usize,
    functions: &mut functions::FunctionTemplates,
) -> Result<(Computation, Type), Vec<Diagnostic>> {
    let mut lower_member = |expression: &'a Expression| {
        let (value, result_type) = computation::lower_expression_with_functions(
            expression,
            scope,
            visited,
            depth + 1,
            functions,
        )?;
        let nested_group = matches!(expression, Expression::Call { callee, .. }
            if matches!(callee.as_str(), "seq" | "repeat" | "all"));
        if !nested_group && value.prepared_atomic_operation().is_none() {
            return Err(invalid(
                "group members require one uniform atomic operation after pure preparation",
                expression_span(expression),
            ));
        }
        let effect = into_effect(value);
        let required_capabilities = required_capabilities_for_effect(&effect)
            .into_iter()
            .map(str::to_string)
            .collect();
        Ok(LoweredEffect {
            effect,
            result_type,
            required_capabilities,
        })
    };
    let lowered = match callee {
        "seq" => control_flow::lower_sequence_with(arguments, span, &mut lower_member),
        "repeat" => control_flow::lower_repeat_with(arguments, span, &mut lower_member),
        "all" => lower_all_with(arguments, span, &mut lower_member),
        _ => return Err(invalid("unknown computed group", span)),
    }?;
    if !computation::contains_computation(&lowered.effect) {
        return Ok((
            Computation::Host {
                effect: Box::new(lowered.effect),
            },
            lowered.result_type,
        ));
    }
    let (group_kind, branches) = match lowered.effect {
        Effect::Sequence { steps } => (GroupKind::Sequence, steps),
        Effect::All { branches } => (GroupKind::Parallel, branches),
        _ => return Err(invalid("expected computed group", span)),
    };
    let mut computed = Vec::with_capacity(branches.len());
    for branch in branches {
        let value = match branch.effect {
            Effect::Compute { expression } if expression.prepared_atomic_operation().is_some() => {
                *expression
            }
            effect if HostOperation::for_effect(&effect).is_some() => Computation::Host {
                effect: Box::new(effect),
            },
            _ => {
                return Err(invalid(
                    "computed groups require prepared atomic members; mixed or nested parallel groups are unsupported",
                    span,
                ));
            }
        };
        computed.push(ComputedBranch {
            name: branch.name,
            value,
            result_type: branch.result_type,
        });
    }
    Ok((
        Computation::Group {
            group_kind,
            branches: computed,
        },
        Type::Structured,
    ))
}

// Intermediate effect groups reuse the existing flattening and naming rules.
// They never reach the VM or journal with unresolved computation leaves.
fn into_effect(value: Computation) -> Effect {
    match value {
        Computation::Host { effect } => *effect,
        Computation::Group {
            group_kind,
            branches,
        } => {
            let branches = branches
                .into_iter()
                .map(|branch| HirBranch {
                    name: branch.name,
                    effect: into_effect(branch.value),
                    result_type: branch.result_type,
                })
                .collect();
            match group_kind {
                GroupKind::Sequence => Effect::Sequence { steps: branches },
                GroupKind::Parallel => Effect::All { branches },
            }
        }
        expression => Effect::Compute {
            expression: Box::new(expression),
        },
    }
}

pub(super) fn validate_repeat_expansion(
    effect: &Effect,
    count: usize,
    span: Span,
) -> Result<(), Vec<Diagnostic>> {
    if !computation::contains_computation(effect) {
        return Ok(());
    }
    let limit = (MAX_COMPUTATION_NODES - 1)
        .checked_div(count)
        .ok_or_else(|| invalid("repeat count must be positive", span))?;
    let mut effects = vec![effect];
    let mut nodes = 0usize;
    let oversized = || {
        vec![Diagnostic {
            code: "LSH1405".into(),
            message: "repeated computation exceeds the expanded node limit".into(),
            span: Some(span),
        }]
    };
    while let Some(effect) = effects.pop() {
        match effect {
            Effect::Sequence { steps } | Effect::All { branches: steps } => {
                effects.extend(steps.iter().map(|step| &step.effect))
            }
            Effect::Compute { expression } => {
                nodes = nodes.saturating_add(
                    functions::source_cost(expression)
                        .map_err(|_| oversized())?
                        .nodes,
                );
            }
            _ => nodes += 2,
        }
        if nodes > limit {
            return Err(oversized());
        }
    }
    Ok(())
}
