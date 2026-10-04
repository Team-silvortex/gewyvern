use std::borrow::Cow;

use leselang_hir::computation::{
    BinaryOperator, Computation, GroupKind, ScalarValue, StringListValue, UnaryOperator,
};
use leselang_hir::host_call::HostOperation;
use leselang_hir::{Effect, HirBranch};
use leselang_runtime_core::{
    BinarySelection, CalculationFailure, FoldCursor, FoldError, Fuel, LoopBudget, LoopError,
    LoopStep, ScalarError, ScopeFrame, StringListBuilder, apply_binary, apply_unary,
    select_binary_left,
};

use crate::result_binding::{
    ProjectedBinding, ProjectedGroup, ProjectedGroupBinding, ProjectedResult,
};
use crate::{Fault, ResultBinding, ScalarBinding, Value};

#[derive(Clone, Copy)]
pub(super) enum ResultView<'a> {
    Raw {
        value: &'a Value,
        operation: HostOperation,
    },
    Projected(&'a ProjectedResult),
    Group {
        value: &'a Value,
        branches: &'a [HirBranch],
    },
    ProjectedGroup(&'a ProjectedGroup),
}

impl ResultView<'_> {
    fn field(self, field: leselang_hir::result_field::ResultField) -> Result<ScalarValue, Fault> {
        match self {
            Self::Raw { value, .. } => crate::result_binding::project(value, field),
            Self::Projected(result) => result.field(field),
            Self::Group { .. } | Self::ProjectedGroup(_) => Err(invalid()),
        }
    }

    fn snapshot(self) -> Result<ProjectedResult, Fault> {
        match self {
            Self::Raw { value, operation } => ProjectedResult::capture(operation, value),
            Self::Projected(result) => Ok(result.clone()),
            Self::Group { .. } | Self::ProjectedGroup(_) => Err(invalid()),
        }
    }

    fn snapshot_group(self) -> Result<ProjectedGroup, Fault> {
        match self {
            Self::Group { value, branches } => ProjectedGroup::capture(branches, value),
            Self::ProjectedGroup(group) => Ok(group.clone()),
            _ => Err(invalid()),
        }
    }
}

pub(super) enum Outcome<'a> {
    Scalar(ScalarValue),
    Result(ResultView<'a>),
    Host(Cow<'a, Effect>),
    BoundHost {
        effect: Cow<'a, Effect>,
        binding: Box<ResultBinding>,
    },
}

#[derive(Clone)]
enum LocalValue<'a> {
    Scalar(ScalarValue),
    Result(ResultView<'a>),
}

type EvaluationFailure = CalculationFailure<Fault>;

fn error(code: &str, message: &str) -> Fault {
    Fault {
        code: code.to_string(),
        message: message.to_string(),
    }
}

fn invalid() -> Fault {
    error("LSV1402", "computation violates its typed HIR contract")
}

fn charge(fuel: &mut Fuel, cost: u64) -> Result<(), Fault> {
    fuel.charge(cost)
        .map_err(|_| error("LSV1001", "computation fuel exhausted"))
}

fn string_cost(value: &ScalarValue) -> u64 {
    if let ScalarValue::StringList(value) = value {
        return list_cost(value);
    }
    value
        .text()
        .map_or(0, |value| value.len().div_ceil(64) as u64)
}

fn list_cost(value: &StringListValue) -> u64 {
    value
        .0
        .iter()
        .map(|item| 1 + item.len().div_ceil(64) as u64)
        .sum()
}

pub(super) fn evaluate<'a>(
    expression: &'a Computation,
    fuel: &mut Fuel,
) -> Result<Outcome<'a>, Fault> {
    let mut bindings = Vec::new();
    let mut scope = ScopeFrame::new(&mut bindings);
    evaluate_inner(expression, fuel, &mut scope).map_err(calculation_fault)
}

fn scalar(outcome: Outcome<'_>) -> Result<ScalarValue, Fault> {
    match outcome {
        Outcome::Scalar(value) => Ok(value),
        _ => Err(invalid()),
    }
}

pub(super) fn resume(
    binding: &ResultBinding,
    value: &Value,
    operation: HostOperation,
    fuel: &mut Fuel,
) -> Result<ScalarValue, Fault> {
    scalar(resume_outcome(binding, value, operation, fuel)?)
}

pub(super) fn resume_group(
    binding: &ResultBinding,
    value: &Value,
    branches: &[HirBranch],
    fuel: &mut Fuel,
) -> Result<ScalarValue, Fault> {
    scalar(resume_group_outcome(binding, value, branches, fuel)?)
}

pub(super) fn resume_group_outcome<'a>(
    binding: &'a ResultBinding,
    value: &'a Value,
    branches: &'a [HirBranch],
    fuel: &mut Fuel,
) -> Result<Outcome<'a>, Fault> {
    let mut bindings = Vec::with_capacity(binding.locals.len() + 1);
    let mut scope = ScopeFrame::new(&mut bindings);
    for local in &binding.locals {
        charge(fuel, 1 + string_cost(&local.value))?;
        scope
            .push(&local.name, LocalValue::Scalar(local.value.clone()))
            .map_err(|_| invalid())?;
    }
    scope
        .push(
            &binding.name,
            LocalValue::Result(ResultView::Group { value, branches }),
        )
        .map_err(|_| invalid())?;
    evaluate_inner(&binding.body, fuel, &mut scope).map_err(calculation_fault)
}

pub(super) fn resume_outcome<'a>(
    binding: &'a ResultBinding,
    value: &'a Value,
    operation: HostOperation,
    fuel: &mut Fuel,
) -> Result<Outcome<'a>, Fault> {
    let mut bindings = Vec::with_capacity(binding.locals.len() + 1);
    let mut scope = ScopeFrame::new(&mut bindings);
    for local in &binding.locals {
        charge(fuel, 1 + string_cost(&local.value))?;
        scope
            .push(&local.name, LocalValue::Scalar(local.value.clone()))
            .map_err(|_| invalid())?;
    }
    for result in &binding.results {
        charge_projection(fuel, &result.result)?;
        scope
            .push(
                &result.name,
                LocalValue::Result(ResultView::Projected(&result.result)),
            )
            .map_err(|_| invalid())?;
    }
    for group in &binding.groups {
        charge_group_projection(fuel, &group.group)?;
        scope
            .push(
                &group.name,
                LocalValue::Result(ResultView::ProjectedGroup(&group.group)),
            )
            .map_err(|_| invalid())?;
    }
    scope
        .push(
            &binding.name,
            LocalValue::Result(ResultView::Raw { value, operation }),
        )
        .map_err(|_| invalid())?;
    evaluate_inner(&binding.body, fuel, &mut scope).map_err(calculation_fault)
}

fn charge_projection(fuel: &mut Fuel, result: &ProjectedResult) -> Result<(), Fault> {
    charge(
        fuel,
        1 + result
            .fields
            .iter()
            .map(|field| 1 + string_cost(&field.value))
            .sum::<u64>(),
    )
}

fn charge_group_projection(fuel: &mut Fuel, group: &ProjectedGroup) -> Result<(), Fault> {
    charge(fuel, 1)?;
    for member in &group.members {
        charge_projection(fuel, &member.result)?;
    }
    Ok(())
}

fn evaluate_inner<'a>(
    expression: &'a Computation,
    fuel: &mut Fuel,
    scope: &mut ScopeFrame<'_, 'a, LocalValue<'a>>,
) -> Result<Outcome<'a>, EvaluationFailure> {
    charge(fuel, 1)?;
    let value = match expression {
        Computation::Literal { value } => {
            charge(fuel, string_cost(value))?;
            value.clone()
        }
        Computation::Strings { items } => {
            let mut values = StringListBuilder::with_capacity(items.len())
                .map_err(CalculationFailure::Scalar)?;
            for item in items {
                let ScalarValue::String(value) = scalar(evaluate_inner(item, fuel, scope)?)? else {
                    return Err(invalid().into());
                };
                values.try_push(value).map_err(CalculationFailure::Scalar)?;
            }
            let value = ScalarValue::StringList(values.finish());
            charge(fuel, string_cost(&value))?;
            value
        }
        Computation::Local { name } => {
            let value = scope.get(name).ok_or_else(invalid)?;
            match value {
                LocalValue::Scalar(value) => {
                    charge(fuel, string_cost(value))?;
                    value.clone()
                }
                LocalValue::Result(value) => return Ok(Outcome::Result(*value)),
            }
        }
        Computation::Bind { name, value, body } => {
            let value = match evaluate_inner(value, fuel, scope)? {
                Outcome::Scalar(value) => LocalValue::Scalar(value),
                Outcome::Result(value) => LocalValue::Result(value),
                Outcome::Host(effect) => {
                    let mut locals = Vec::with_capacity(scope.len());
                    let mut results = Vec::new();
                    let mut groups = Vec::new();
                    for (name, value) in scope.bindings() {
                        match value {
                            LocalValue::Scalar(value) => {
                                charge(fuel, 1 + string_cost(value))?;
                                locals.push(ScalarBinding {
                                    name: (*name).to_owned(),
                                    value: value.clone(),
                                });
                            }
                            LocalValue::Result(
                                value @ (ResultView::Group { .. } | ResultView::ProjectedGroup(_)),
                            ) => {
                                let group = value.snapshot_group()?;
                                charge_group_projection(fuel, &group)?;
                                groups.push(ProjectedGroupBinding {
                                    name: (*name).to_owned(),
                                    group,
                                });
                            }
                            LocalValue::Result(value) => {
                                let result = value.snapshot()?;
                                charge_projection(fuel, &result)?;
                                results.push(ProjectedBinding {
                                    name: (*name).to_owned(),
                                    result,
                                });
                            }
                        }
                    }
                    return Ok(Outcome::BoundHost {
                        effect,
                        binding: Box::new(ResultBinding {
                            name: name.clone(),
                            locals,
                            results,
                            groups,
                            body: body.as_ref().clone(),
                        }),
                    });
                }
                Outcome::BoundHost { .. } => return Err(invalid().into()),
            };
            let mut local = scope.nested();
            local.push(name, value).map_err(|_| invalid())?;
            return evaluate_inner(body, fuel, &mut local);
        }
        Computation::Loop {
            name,
            initial,
            condition,
            next,
            limit,
        } => {
            let initial = scalar(evaluate_inner(initial, fuel, scope)?)?;
            let mut budget = LoopBudget::new(initial.scalar_type(), *limit).map_err(loop_fault)?;
            let mut local = scope.nested();
            let slot = local
                .push(name, LocalValue::Scalar(initial))
                .map_err(|_| invalid())?;
            loop {
                let ScalarValue::Boolean(keep_going) =
                    scalar(evaluate_inner(condition, fuel, &mut local)?)?
                else {
                    return Err(invalid().into());
                };
                if budget.check_condition(keep_going).map_err(loop_fault)? == LoopStep::Done {
                    break;
                }
                let next = scalar(evaluate_inner(next, fuel, &mut local)?)?;
                budget.advance(&next).map_err(loop_fault)?;
                *local.get_local_mut(slot).ok_or_else(invalid)? = LocalValue::Scalar(next);
            }
            let (_, state) = local.pop().ok_or_else(invalid)?;
            let LocalValue::Scalar(state) = state else {
                return Err(invalid().into());
            };
            state
        }
        Computation::Fold {
            name,
            item,
            items,
            initial,
            next,
            limit,
        } => {
            let ScalarValue::StringList(items) = scalar(evaluate_inner(items, fuel, scope)?)?
            else {
                return Err(invalid().into());
            };
            let initial = scalar(evaluate_inner(initial, fuel, scope)?)?;
            let items_cost = list_cost(&items);
            let mut cursor =
                FoldCursor::new(items, initial.scalar_type(), *limit).map_err(fold_fault)?;
            charge(fuel, items_cost)?;
            let mut local = scope.nested();
            let slot = local
                .push(name, LocalValue::Scalar(initial))
                .map_err(|_| invalid())?;
            let item_slot = local
                .push(item, LocalValue::Scalar(ScalarValue::String(String::new())))
                .map_err(|_| invalid())?;
            while let Some(item) = cursor.next_item().map_err(fold_fault)? {
                charge(fuel, 1 + item.len().div_ceil(64) as u64)?;
                *local.get_local_mut(item_slot).ok_or_else(invalid)? =
                    LocalValue::Scalar(ScalarValue::String(item));
                let next = scalar(evaluate_inner(next, fuel, &mut local)?)?;
                cursor.advance(&next).map_err(fold_fault)?;
                *local.get_local_mut(slot).ok_or_else(invalid)? = LocalValue::Scalar(next);
            }
            local.pop();
            let (_, state) = local.pop().ok_or_else(invalid)?;
            let LocalValue::Scalar(state) = state else {
                return Err(invalid().into());
            };
            state
        }
        Computation::Member {
            group,
            name,
            operation,
        } => {
            let Some(LocalValue::Result(view)) = scope.get(group) else {
                return Err(invalid().into());
            };
            return match view {
                ResultView::Group {
                    value: Value::Structured { fields },
                    branches,
                } => {
                    if !branches.iter().any(|branch| {
                        branch.name == *name
                            && HostOperation::for_effect(&branch.effect) == Some(*operation)
                    }) {
                        return Err(invalid().into());
                    }
                    let field = fields
                        .iter()
                        .find(|field| field.name == *name)
                        .ok_or_else(invalid)?;
                    Ok(Outcome::Result(ResultView::Raw {
                        value: &field.value,
                        operation: *operation,
                    }))
                }
                ResultView::ProjectedGroup(saved) => {
                    let member = saved
                        .members
                        .iter()
                        .find(|member| {
                            member.name == *name && member.result.operation == *operation
                        })
                        .ok_or_else(invalid)?;
                    Ok(Outcome::Result(ResultView::Projected(&member.result)))
                }
                _ => Err(invalid().into()),
            };
        }
        Computation::Field { value, field } => {
            let Outcome::Result(value) = evaluate_inner(value, fuel, scope)? else {
                return Err(invalid().into());
            };
            let value = value.field(*field)?;
            charge(fuel, string_cost(&value))?;
            value
        }
        Computation::Choose {
            when,
            then,
            otherwise,
        } => {
            let ScalarValue::Boolean(when) = scalar(evaluate_inner(when, fuel, scope)?)? else {
                return Err(invalid().into());
            };
            return evaluate_inner(if when { then } else { otherwise }, fuel, scope);
        }
        Computation::Recover { value, fallback } => {
            match evaluate_inner(value, fuel, scope)
                .and_then(|outcome| scalar(outcome).map_err(CalculationFailure::External))
            {
                Ok(value) => value,
                // Only language data errors are recoverable, never resource or host failures.
                Err(failure) if failure.is_recoverable() => {
                    scalar(evaluate_inner(fallback, fuel, scope)?)?
                }
                Err(failure) => return Err(failure),
            }
        }
        Computation::Host { effect } => return Ok(Outcome::Host(Cow::Borrowed(effect))),
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
                let Outcome::Host(effect) = evaluate_inner(&branch.value, fuel, scope)? else {
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
            let effect = match group_kind {
                GroupKind::Sequence => Effect::Sequence { steps: resolved },
                GroupKind::Parallel => Effect::All { branches: resolved },
            };
            return Ok(Outcome::Host(Cow::Owned(effect)));
        }
        Computation::Call {
            operation,
            arguments,
        } => {
            let mut values = Vec::with_capacity(arguments.len());
            for argument in arguments {
                let value = scalar(evaluate_inner(&argument.value, fuel, scope)?)?;
                charge(fuel, string_cost(&value))?;
                values.push((argument.name.clone(), value));
            }
            let effect = operation.resolve(&values).map_err(|diagnostics| {
                let code = diagnostics
                    .first()
                    .map_or("LSH1407", |diagnostic| diagnostic.code.as_str());
                error(
                    "LSV1404",
                    &format!(
                        "computed arguments rejected by {} ({code})",
                        operation.name()
                    ),
                )
            })?;
            return Ok(Outcome::Host(Cow::Owned(effect)));
        }
        Computation::Unary { operator, value } => {
            let value = scalar(evaluate_inner(value, fuel, scope)?)?;
            charge(fuel, string_cost(&value))?;
            let value = apply_unary(*operator, value).map_err(CalculationFailure::Scalar)?;
            if *operator == UnaryOperator::ToString {
                charge(fuel, string_cost(&value))?;
            }
            value
        }
        Computation::Binary {
            operator,
            left,
            right,
        } => {
            let left = scalar(evaluate_inner(left, fuel, scope)?)?;
            let left =
                match select_binary_left(*operator, left).map_err(CalculationFailure::Scalar)? {
                    BinarySelection::Complete(value) => {
                        if *operator == BinaryOperator::ValueOr {
                            charge(fuel, string_cost(&value))?;
                        }
                        return Ok(Outcome::Scalar(value));
                    }
                    BinarySelection::NeedsRight(left) => left,
                };
            let right = scalar(evaluate_inner(right, fuel, scope)?)?;
            charge(fuel, string_cost(&left) + string_cost(&right))?;
            let value = apply_binary(*operator, left, right).map_err(CalculationFailure::Scalar)?;
            if matches!(
                operator,
                BinaryOperator::CharAt
                    | BinaryOperator::ItemAt
                    | BinaryOperator::Split
                    | BinaryOperator::Join
                    | BinaryOperator::Append
            ) {
                charge(fuel, string_cost(&value))?;
            }
            value
        }
    };
    Ok(Outcome::Scalar(value))
}

fn calculation_fault(failure: EvaluationFailure) -> Fault {
    match failure {
        CalculationFailure::Scalar(failure) => scalar_fault(failure),
        CalculationFailure::External(fault) => fault,
    }
}

fn scalar_fault(failure: ScalarError) -> Fault {
    let code = match failure {
        ScalarError::TypeMismatch | ScalarError::UnboundedOperand => return invalid(),
        ScalarError::IntegerArithmetic => "LSV1401",
        ScalarError::InvalidIntegerText | ScalarError::InvalidBooleanText => "LSV1408",
        ScalarError::StringLimit | ScalarError::StringListLimit => "LSV1403",
    };
    Fault {
        code: code.to_owned(),
        message: failure.to_string(),
    }
}

fn loop_fault(failure: LoopError) -> Fault {
    match failure {
        LoopError::IterationLimit => error("LSV1406", "loop iteration limit exhausted"),
        _ => invalid(),
    }
}

fn fold_fault(failure: FoldError) -> Fault {
    match failure {
        FoldError::IterationLimit => error("LSV1406", "fold iteration limit exhausted"),
        _ => invalid(),
    }
}

#[cfg(test)]
mod recovery_tests {
    use super::*;

    #[test]
    fn external_fault_codes_never_enter_the_scalar_recovery_channel() {
        for code in ["LSV1401", "LSV1408", "LSV1403", "LSV1001", "custom"] {
            let original = error(code, "original host payload");
            let pointer = original.message.as_ptr();
            let failure: EvaluationFailure = original.clone().into();
            assert!(!failure.is_recoverable());
            assert_eq!(calculation_fault(failure), original);
            let moved: EvaluationFailure = original.into();
            assert_eq!(calculation_fault(moved).message.as_ptr(), pointer);
        }
    }

    #[test]
    fn inner_evaluation_retains_typed_failures_until_the_outer_fault_boundary() {
        for (operator, operand, expected, code, recoverable) in [
            (
                UnaryOperator::ParseInteger,
                ScalarValue::String("secret".into()),
                ScalarError::InvalidIntegerText,
                "LSV1408",
                true,
            ),
            (
                UnaryOperator::ParseBoolean,
                ScalarValue::String("secret".into()),
                ScalarError::InvalidBooleanText,
                "LSV1408",
                true,
            ),
            (
                UnaryOperator::Not,
                ScalarValue::Integer(0),
                ScalarError::TypeMismatch,
                "LSV1402",
                false,
            ),
        ] {
            let expression = Computation::Unary {
                operator,
                value: Box::new(Computation::Literal { value: operand }),
            };
            let mut bindings = Vec::new();
            let mut scope = ScopeFrame::new(&mut bindings);
            let mut fuel = Fuel::new(100);
            let Err(failure) = evaluate_inner(&expression, &mut fuel, &mut scope) else {
                panic!()
            };
            assert!(matches!(&failure, CalculationFailure::Scalar(error) if *error == expected));
            assert_eq!(failure.is_recoverable(), recoverable);
            let fault = calculation_fault(failure);
            assert_eq!(fault.code, code);
            assert!(!fault.message.contains("secret"));
        }
    }

    #[test]
    fn forged_effects_and_exhausted_fuel_do_not_run_calculation_fallbacks() {
        let expression = Computation::Recover {
            value: Box::new(Computation::Host {
                effect: Box::new(Effect::UiFocus {
                    node_id: "a".into(),
                }),
            }),
            fallback: Box::new(Computation::Literal {
                value: ScalarValue::Integer(7),
            }),
        };
        let mut bindings = Vec::new();
        let mut scope = ScopeFrame::new(&mut bindings);
        let mut fuel = Fuel::new(100);
        let Err(failure) = evaluate_inner(&expression, &mut fuel, &mut scope) else {
            panic!()
        };
        assert!(matches!(&failure, CalculationFailure::External(_)));
        assert!(!failure.is_recoverable());
        assert_eq!(calculation_fault(failure).code, "LSV1402");
        assert_eq!(fuel.remaining(), 98);
        let mut fuel = Fuel::new(0);
        let Err(failure) = evaluate_inner(&expression, &mut fuel, &mut scope) else {
            panic!()
        };
        assert!(!failure.is_recoverable());
        assert_eq!(calculation_fault(failure).code, "LSV1001");
        assert_eq!(fuel.remaining(), 0);
    }
}
