use super::*;
use leselang_hir::computation::{ComputedArgument, ComputedBranch, GroupKind, UnaryOperator};

struct Frozen;

impl<'a> PureEvaluationEnvironment<leselang_hir::result_field::ResultField, HostOperation>
    for &'a Frozen
{
    type Result = ResultView<'a>;
    type Error = Fault;

    fn field(
        &self,
        result: &Self::Result,
        field: &leselang_hir::result_field::ResultField,
    ) -> Result<ScalarValue, Fault> {
        result.field(*field)
    }

    fn member(
        &self,
        group: &Self::Result,
        name: &str,
        operation: &HostOperation,
    ) -> Result<Self::Result, Fault> {
        group.select_member(name, *operation).map_err(view_fault)
    }
}

impl<'a>
    EffectEvaluationEnvironment<
        'a,
        leselang_hir::result_field::ResultField,
        HostOperation,
        Effect,
        leselang_hir::Type,
    > for &'a Frozen
{
    type Request = Cow<'a, Effect>;
    type Capture = Box<ResultBinding>;

    fn preflight_effect(&self, _: &'a Computation) -> Result<(), Fault> {
        Ok(())
    }

    fn prepare_effect(
        &self,
        expression: &'a Computation,
        scope: &mut ScopeFrame<'_, 'a, LocalValue<'a>>,
        fuel: &mut Fuel,
    ) -> Result<Self::Request, EvaluationFailure> {
        match expression {
            Computation::Host { effect } => Ok(Cow::Borrowed(effect)),
            Computation::Group {
                group_kind,
                branches,
            } => {
                let mut resolved = Vec::with_capacity(branches.len());
                for branch in branches {
                    let operation = branch
                        .value
                        .prepared_atomic_operation()
                        .ok_or_else(invalid)?;
                    let outcome = evaluate_effects_in_scope(
                        &branch.value,
                        scope,
                        self,
                        fuel,
                        execution_limits(),
                    )
                    .map_err(effect_failure)?;
                    let EffectEvaluationOutcome::Request(effect) = outcome else {
                        return Err(invalid().into());
                    };
                    if HostOperation::for_effect(&effect) != Some(operation)
                        || branch.result_type != operation.result_type()
                    {
                        return Err(invalid().into());
                    }
                    resolved.push(HirBranch {
                        name: branch.name.clone(),
                        effect: effect.into_owned(),
                        result_type: branch.result_type,
                    });
                }
                Ok(Cow::Owned(match group_kind {
                    GroupKind::Sequence => Effect::Sequence { steps: resolved },
                    GroupKind::Parallel => Effect::All { branches: resolved },
                }))
            }
            Computation::Call {
                operation,
                arguments,
            } => {
                let values = operation
                    .evaluate_computed_arguments(
                        arguments,
                        scope,
                        self,
                        fuel,
                        leselang_hir::call_evaluation::CallEvaluationLimits {
                            pure: execution_limits().pure,
                            max_arguments: MAX_CALL_ARGUMENTS,
                        },
                    )
                    .map_err(|failure| call_failure(failure, *operation))?;
                let effect = operation.resolve(&values).map_err(|diagnostics| {
                    let code = diagnostics.first().map_or("LSH1407", |d| d.code.as_str());
                    error(
                        "LSV1404",
                        &format!(
                            "computed arguments rejected by {} ({code})",
                            operation.name(),
                        ),
                    )
                })?;
                Ok(Cow::Owned(effect))
            }
            _ => Err(invalid().into()),
        }
    }

    fn capture(
        &self,
        name: &'a str,
        body: &'a Computation,
        scope: &ScopeFrame<'_, 'a, LocalValue<'a>>,
        fuel: &mut Fuel,
    ) -> Result<Self::Capture, Fault> {
        result_environment().capture(name, body, scope, fuel)
    }
}

fn literal(value: ScalarValue) -> Computation {
    Computation::Literal { value }
}

fn focus_call(value: Computation) -> Computation {
    Computation::Call {
        operation: HostOperation::UiFocus,
        arguments: vec![ComputedArgument {
            name: "node_id".into(),
            value,
        }],
    }
}

fn observe_request(
    result: Result<Cow<'_, Effect>, EvaluationFailure>,
) -> Result<(bool, Effect), (bool, Fault)> {
    result
        .map(|effect| (matches!(effect, Cow::Borrowed(_)), effect.into_owned()))
        .map_err(|failure| (failure.is_recoverable(), calculation_fault(failure)))
}

#[test]
fn effect_preparation_preserves_frozen_requests_fuel_fault_order_and_scope() {
    let call = focus_call(literal(ScalarValue::String("a".into())));
    let group = |branches| Computation::Group {
        group_kind: GroupKind::Sequence,
        branches,
    };
    let branch = |name: &str, value, result_type| ComputedBranch {
        name: name.into(),
        value,
        result_type,
    };
    let mut cases = vec![
        Computation::Host {
            effect: Box::new(Effect::UiFocus {
                node_id: "a".into(),
            }),
        },
        call.clone(),
        focus_call(literal(ScalarValue::Integer(9))),
        focus_call(literal(ScalarValue::String(String::new()))),
        Computation::Call {
            operation: HostOperation::UiFocus,
            arguments: vec![],
        },
        literal(ScalarValue::None),
        group(vec![]),
        group(vec![branch("a", call.clone(), leselang_hir::Type::UiFocus)]),
        group(vec![branch(
            "a",
            call.clone(),
            leselang_hir::Type::UiActivate,
        )]),
        group(vec![branch(
            "a",
            literal(ScalarValue::None),
            leselang_hir::Type::UiFocus,
        )]),
        focus_call(Computation::Unary {
            operator: UnaryOperator::ParseInteger,
            value: Box::new(literal(ScalarValue::String("secret".into()))),
        }),
    ];
    for mode in ["seq", "all"] {
        for tail in [
            "ui.focus(node_id: concat(left: prefix, right: \"b\"))",
            "bind(n: parse_integer(value: \"bad\"), body: ui.focus(node_id: prefix))",
        ] {
            let source = format!(
                "fn main() = bind(prefix: \"p-\", body: {mode}(a: ui.focus(node_id: prefix), b: {tail}))",
            );
            let program = leselang_hir::lower(&leselang_syntax::parse(&source)).unwrap();
            let Effect::Compute { expression } = program.function.effect else {
                panic!()
            };
            cases.push(*expression);
        }
    }
    for expression in cases {
        let bytes = serde_json::to_vec(&expression).unwrap();
        for grant in 0..160 {
            let mut before = Fuel::new(grant);
            let mut after = Fuel::new(grant);
            let mut old_scope = Vec::new();
            let mut new_scope = Vec::new();
            let old = (&Frozen).prepare_effect(
                &expression,
                &mut ScopeFrame::new(&mut old_scope),
                &mut before,
            );
            let new = result_environment().prepare_effect(
                &expression,
                &mut ScopeFrame::new(&mut new_scope),
                &mut after,
            );
            assert_eq!(
                observe_request(new),
                observe_request(old),
                "grant {grant}: {expression:?}"
            );
            assert_eq!(before.remaining(), after.remaining());
            assert!(old_scope.is_empty() && new_scope.is_empty());

            let mut before = Fuel::new(grant);
            let mut after = Fuel::new(grant);
            let old = evaluate_effects_in_scope(
                &expression,
                &mut ScopeFrame::new(&mut old_scope),
                &&Frozen,
                &mut before,
                execution_limits(),
            )
            .map(effect_outcome)
            .map_err(effect_failure);
            let new = evaluate_inner(
                &expression,
                &mut after,
                &mut ScopeFrame::new(&mut new_scope),
            );
            let encode = |outcome: Result<Outcome<'_>, EvaluationFailure>| {
                outcome
                    .map(|outcome| match outcome {
                        Outcome::Scalar(value) => serde_json::to_vec(&value).unwrap(),
                        Outcome::Result => b"result".to_vec(),
                        Outcome::Host(effect) => serde_json::to_vec(&effect).unwrap(),
                        Outcome::BoundHost { effect, binding } => {
                            serde_json::to_vec(&(effect, binding)).unwrap()
                        }
                    })
                    .map_err(|failure| (failure.is_recoverable(), calculation_fault(failure)))
            };
            assert_eq!(encode(new), encode(old), "grant {grant}: {expression:?}");
            assert_eq!(before.remaining(), after.remaining());
            assert!(old_scope.is_empty() && new_scope.is_empty());
        }
        assert_eq!(serde_json::to_vec(&expression).unwrap(), bytes);
    }
}
