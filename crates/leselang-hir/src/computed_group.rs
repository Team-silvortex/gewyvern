use super::*;
use crate::computation::{Computation, ComputedBranch, GroupKind, MAX_COMPUTATION_NODES};
use crate::host_call::HostOperation;

fn invalid(message: &str, span: Span) -> Vec<Diagnostic> {
    vec![Diagnostic {
        code: "LSH1406".into(),
        message: message.into(),
        span: Some(span),
    }]
}

pub(super) fn lower<'a>(
    expression: &'a Expression,
    scope: &mut computation::TypeScope<'_, 'a>,
    visited: &mut usize,
    depth: usize,
    functions: &mut functions::FunctionTemplates,
) -> Result<(Computation, Type), Vec<Diagnostic>> {
    let span = expression_span(expression);
    let Expression::Call {
        callee, arguments, ..
    } = expression
    else {
        return Err(invalid("expected a computed group", span));
    };
    if matches!(callee.as_str(), "seq" | "all")
        && !arguments.iter().any(|argument| {
            matches!(&argument.value,
            Expression::Call { callee, .. } if matches!(callee.as_str(), "seq" | "repeat" | "all"))
        })
    {
        return lower_flat(expression, span, scope, visited, depth, functions);
    }
    if callee == "repeat"
        && !arguments.iter().any(|argument| {
            argument.name == "body"
                && matches!(&argument.value, Expression::Call { callee, .. }
                    if matches!(callee.as_str(), "seq" | "all" | "repeat"))
        })
    {
        return lower_repeat_flat(expression, span, scope, visited, depth, functions);
    }
    if callee == "seq" {
        return lower_sequence_nested(expression, span, scope, visited, depth, functions);
    }
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
    let lowered = match callee.as_str() {
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

fn lower_sequence_nested<'a>(
    expression: &'a Expression,
    span: Span,
    scope: &mut computation::TypeScope<'_, 'a>,
    visited: &mut usize,
    depth: usize,
    functions: &mut functions::FunctionTemplates,
) -> Result<(Computation, Type), Vec<Diagnostic>> {
    use crate::group_source::{GroupSourceError, GroupSourceLimits};
    use crate::sequence_source::{
        SequenceSourceError, SequenceSourceMember, lower_sequence_source,
    };
    let value = lower_sequence_source(
        expression,
        GroupSourceLimits {
            source: crate::source_call::SourceCallLimits {
                max_source_nodes: MAX_COMPUTATION_NODES,
                max_source_depth: MAX_EFFECT_NESTING_DEPTH,
                max_lowered_nodes: MAX_COMPUTATION_NODES,
                max_lowered_depth: MAX_EFFECT_NESTING_DEPTH,
                max_arguments: crate::source_call::MAX_SOURCE_CALL_ARGUMENTS,
            },
            max_branches: MAX_ALL_BRANCHES,
        },
        |_, argument| {
            let (value, result_type) = computation::lower_expression_with_functions(
                &argument.value,
                scope,
                visited,
                depth + 1,
                functions,
            )?;
            let nested = matches!(&argument.value, Expression::Call { callee, .. }
                if matches!(callee.as_str(), "seq" | "repeat"));
            if !nested {
                return Ok(SequenceSourceMember::Atomic { value, result_type });
            }
            let branches = match value {
                Computation::Group {
                    group_kind: GroupKind::Sequence,
                    branches,
                } => branches,
                Computation::Host { effect } => {
                    let Effect::Sequence { steps } = *effect else {
                        return Err(invalid(
                            "expected a lowered sequential child",
                            argument.span,
                        ));
                    };
                    steps
                        .into_iter()
                        .map(|branch| ComputedBranch {
                            name: branch.name,
                            value: match branch.effect {
                                Effect::Compute { expression } => *expression,
                                effect => Computation::Host {
                                    effect: Box::new(effect),
                                },
                            },
                            result_type: branch.result_type,
                        })
                        .collect()
                }
                _ => {
                    return Err(invalid(
                        "expected a lowered sequential child",
                        argument.span,
                    ));
                }
            };
            Ok(SequenceSourceMember::Sequence { branches })
        },
        |_, _, argument, branch| {
            if branch
                .value
                .prepared_atomic_operation()
                .is_some_and(|operation| operation.result_type() == branch.result_type)
            {
                Ok(())
            } else {
                Err(invalid(
                    "group members require one uniform atomic operation after pure preparation",
                    expression_span(&argument.value),
                ))
            }
        },
    )
    .map_err(|error| {
        let diagnostic = |message: &str, at| {
            vec![Diagnostic {
                code: "LSH1301".into(),
                message: message.into(),
                span: Some(at),
            }]
        };
        match error {
            SequenceSourceError::Native { error, .. } => error,
            SequenceSourceError::ParallelMember { span, .. } => {
                diagnostic("all cannot be nested inside sequential control flow", span)
            }
            SequenceSourceError::Width { span, .. } => {
                diagnostic("expanded control flow exceeds 64 effects", span)
            }
            SequenceSourceError::Name { .. } | SequenceSourceError::Collision { .. } => diagnostic(
                "expanded step names must be unique and at most 64 bytes",
                span,
            ),
            SequenceSourceError::Header(GroupSourceError::Arity { kind, span }) => {
                vec![Diagnostic {
                    code: if kind == GroupKind::Sequence {
                        "LSH1301"
                    } else {
                        "LSH1201"
                    }
                    .into(),
                    message: if kind == GroupKind::Sequence {
                        "seq requires between 1 and 64 named steps"
                    } else {
                        "all requires between 2 and 64 named branches"
                    }
                    .into(),
                    span: Some(span),
                }]
            }
            SequenceSourceError::Header(
                GroupSourceError::MemberName { span, .. }
                | GroupSourceError::DuplicateMember { span, .. },
            ) => diagnostic(
                "group member name must be a unique bounded identifier",
                span,
            ),
            SequenceSourceError::Candidate { span, .. }
            | SequenceSourceError::MemberKind { span, .. }
            | SequenceSourceError::NotSequence { span } => invalid(
                "group members require one uniform atomic operation after pure preparation",
                span,
            ),
            _ => vec![Diagnostic {
                code: "LSH1405".into(),
                message: "computation exceeds its node or nesting limit".into(),
                span: Some(span),
            }],
        }
    })?;
    finish_flat_group(value, span)
}

fn lower_flat<'a>(
    expression: &'a Expression,
    span: Span,
    scope: &mut computation::TypeScope<'_, 'a>,
    visited: &mut usize,
    depth: usize,
    functions: &mut functions::FunctionTemplates,
) -> Result<(Computation, Type), Vec<Diagnostic>> {
    use crate::group_source::{
        GroupSourceError, GroupSourceLimits, GroupSourcePhase, lower_flat_group_source,
    };
    let value = lower_flat_group_source(
        expression,
        GroupSourceLimits {
            source: crate::source_call::SourceCallLimits {
                max_source_nodes: MAX_COMPUTATION_NODES,
                max_source_depth: MAX_EFFECT_NESTING_DEPTH,
                max_lowered_nodes: MAX_COMPUTATION_NODES,
                max_lowered_depth: MAX_EFFECT_NESTING_DEPTH,
                max_arguments: crate::source_call::MAX_SOURCE_CALL_ARGUMENTS,
            },
            max_branches: MAX_ALL_BRANCHES,
        },
        |_, argument| {
            computation::lower_expression_with_functions(
                &argument.value,
                scope,
                visited,
                depth + 1,
                functions,
            )
        },
        |_, argument, branch| {
            if branch
                .value
                .prepared_atomic_operation()
                .is_some_and(|operation| operation.result_type() == branch.result_type)
            {
                Ok(())
            } else {
                Err(invalid(
                    "group members require one uniform atomic operation after pure preparation",
                    expression_span(&argument.value),
                ))
            }
        },
    )
    .map_err(|error| match error {
        GroupSourceError::Native {
            index,
            phase: GroupSourcePhase::Lower,
            mut error,
            ..
        } => {
            if let Expression::Call {
                callee, arguments, ..
            } = expression
                && callee == "all"
            {
                // Construction has failed; only collect bounded sibling diagnostics.
                for argument in arguments.iter().skip(index + 1) {
                    match computation::lower_expression_with_functions(
                        &argument.value,
                        scope,
                        visited,
                        depth + 1,
                        functions,
                    ) {
                        Ok((value, result_type))
                            if value.prepared_atomic_operation().is_some_and(|operation| {
                                operation.result_type() == result_type
                            }) => {}
                        Ok(_) => error.extend(invalid(
                            "group members require one uniform atomic operation after pure preparation",
                            expression_span(&argument.value),
                        )),
                        Err(mut diagnostics) => error.append(&mut diagnostics),
                    }
                }
            }
            error
        }
        GroupSourceError::Native { error, .. } => error,
        GroupSourceError::Arity { kind, span } => vec![Diagnostic {
            code: if kind == GroupKind::Sequence {
                "LSH1301"
            } else {
                "LSH1201"
            }
            .into(),
            message: if kind == GroupKind::Sequence {
                "seq requires between 1 and 64 named steps"
            } else {
                "all requires between 2 and 64 named branches"
            }
            .into(),
            span: Some(span),
        }],
        GroupSourceError::DuplicateMember { kind, span: at, .. } => vec![Diagnostic {
            code: if kind == GroupKind::Sequence {
                "LSH1301"
            } else {
                "LSH1202"
            }
            .into(),
            message: "duplicate group member name".into(),
            span: Some(at),
        }],
        GroupSourceError::MemberName { kind, span, .. } => vec![Diagnostic {
            code: if kind == GroupKind::Sequence {
                "LSH1301"
            } else {
                "LSH1203"
            }
            .into(),
            message: "group member name must be a bounded identifier".into(),
            span: Some(span),
        }],
        GroupSourceError::Candidate { span, .. }
        | GroupSourceError::NestedMember { span, .. }
        | GroupSourceError::NotGroup { span } => invalid(
            "group members require one uniform atomic operation after pure preparation",
            span,
        ),
        _ => vec![Diagnostic {
            code: "LSH1405".into(),
            message: "computation exceeds its node or nesting limit".into(),
            span: Some(span),
        }],
    })?;
    finish_flat_group(value, span)
}

fn finish_flat_group(
    value: Computation,
    span: Span,
) -> Result<(Computation, Type), Vec<Diagnostic>> {
    let Computation::Group {
        group_kind,
        branches,
    } = value
    else {
        return Err(invalid("expected computed group", span));
    };
    if branches
        .iter()
        .all(|branch| matches!(branch.value, Computation::Host { .. }))
    {
        let mut opaque = Vec::with_capacity(branches.len());
        for branch in branches {
            let Computation::Host { effect } = branch.value else {
                return Err(invalid("expected atomic host group member", span));
            };
            opaque.push(HirBranch {
                name: branch.name,
                effect: *effect,
                result_type: branch.result_type,
            });
        }
        let effect = match group_kind {
            GroupKind::Sequence => Effect::Sequence { steps: opaque },
            GroupKind::Parallel => Effect::All { branches: opaque },
        };
        return Ok((
            Computation::Host {
                effect: Box::new(effect),
            },
            Type::Structured,
        ));
    }
    Ok((
        Computation::Group {
            group_kind,
            branches,
        },
        Type::Structured,
    ))
}

struct RepeatAdapter<'adapter, 'scope, 'source> {
    scope: &'adapter mut computation::TypeScope<'scope, 'source>,
    visited: &'adapter mut usize,
    depth: usize,
    functions: &'adapter mut functions::FunctionTemplates,
}

impl<'source>
    crate::repeat_source::RepeatSourceAdapter<
        'source,
        crate::result_field::ResultField,
        HostOperation,
        Effect,
        Type,
    > for RepeatAdapter<'_, '_, 'source>
{
    type Error = Vec<Diagnostic>;
    fn lower_body(
        &mut self,
        body: &'source leselang_syntax::NamedArgument,
    ) -> Result<(Computation, Type), Self::Error> {
        computation::lower_expression_with_functions(
            &body.value,
            self.scope,
            self.visited,
            self.depth + 1,
            self.functions,
        )
    }
    fn host_cost(
        &mut self,
        effect: &Effect,
    ) -> Result<crate::source_cost::SourceCostExtra, Self::Error> {
        functions::host_source_extra(
            effect,
            crate::source_cost::SourceCostLimits {
                max_nodes: crate::pure_typing::MAX_TYPE_INFERENCE_NODES,
                max_depth: crate::pure_typing::MAX_TYPE_INFERENCE_DEPTH,
            },
        )
        .map_err(|_| {
            vec![Diagnostic {
                code: "LSH1405".into(),
                message: "repeat native source cost exceeds its bounds".into(),
                span: None,
            }]
        })
    }
    fn materialize(
        &mut self,
        _: usize,
        _: &'source leselang_syntax::NamedArgument,
        original: &ComputedBranch,
    ) -> Result<(Computation, Type), Self::Error> {
        Ok((original.value.clone(), original.result_type))
    }
    fn admit(
        &mut self,
        _: usize,
        body: &'source leselang_syntax::NamedArgument,
        original: &ComputedBranch,
        candidate: &ComputedBranch,
    ) -> Result<(), Self::Error> {
        let operation = original.value.prepared_atomic_operation();
        if operation.is_some_and(|operation| operation.result_type() == original.result_type)
            && candidate.value.prepared_atomic_operation() == operation
            && candidate.result_type == original.result_type
        {
            Ok(())
        } else {
            Err(invalid(
                "repeat body requires one uniform atomic operation after pure preparation",
                body.span,
            ))
        }
    }
}

fn lower_repeat_flat<'source>(
    expression: &'source Expression,
    span: Span,
    scope: &mut computation::TypeScope<'_, 'source>,
    visited: &mut usize,
    depth: usize,
    functions: &mut functions::FunctionTemplates,
) -> Result<(Computation, Type), Vec<Diagnostic>> {
    use crate::repeat_source::{RepeatSourceError, RepeatSourceLimits, lower_flat_repeat_source};
    let value = lower_flat_repeat_source(
        expression,
        RepeatSourceLimits {
            source: crate::source_call::SourceCallLimits {
                max_source_nodes: MAX_COMPUTATION_NODES,
                max_source_depth: MAX_EFFECT_NESTING_DEPTH,
                max_lowered_nodes: MAX_COMPUTATION_NODES,
                max_lowered_depth: MAX_EFFECT_NESTING_DEPTH,
                max_arguments: crate::source_call::MAX_SOURCE_CALL_ARGUMENTS,
            },
            expanded: crate::source_cost::SourceCostLimits {
                max_nodes: MAX_COMPUTATION_NODES,
                max_depth: MAX_EFFECT_NESTING_DEPTH,
            },
            max_repetitions: MAX_SEQUENCE_STEPS,
        },
        &mut RepeatAdapter {
            scope,
            visited,
            depth,
            functions,
        },
    )
    .map_err(|error| match error {
        RepeatSourceError::Native { error, .. } => error,
        RepeatSourceError::Cost {
            error: crate::source_cost::SourceCostError::Observation { error, .. },
            ..
        } => error,
        RepeatSourceError::Output {
            error: leselang_runtime_core::StructureError::NodeLimit,
            ..
        }
        | RepeatSourceError::Cost {
            error:
                crate::source_cost::SourceCostError::Structure(
                    leselang_runtime_core::StructureError::NodeLimit,
                ),
            ..
        } => vec![Diagnostic {
            code: "LSH1405".into(),
            message: "repeated computation exceeds the expanded node limit".into(),
            span: Some(span),
        }],
        RepeatSourceError::Names { span } | RepeatSourceError::NotRepeat { span } => {
            vec![Diagnostic {
                code: "LSH1301".into(),
                message: "repeat requires exactly 'times' and 'body'".into(),
                span: Some(span),
            }]
        }
        RepeatSourceError::Count { span } => vec![Diagnostic {
            code: "LSH1301".into(),
            message: "repeat times must be an integer from 1 through 64".into(),
            span: Some(span),
        }],
        RepeatSourceError::Candidate { span, .. } | RepeatSourceError::NestedBody { span } => {
            invalid(
                "repeat body requires one uniform atomic operation after pure preparation",
                span,
            )
        }
        _ => vec![Diagnostic {
            code: "LSH1405".into(),
            message: "repeated computation exceeds its node or nesting limit".into(),
            span: Some(span),
        }],
    })?;
    finish_flat_group(value, span)
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
