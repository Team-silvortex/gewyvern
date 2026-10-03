use std::borrow::Cow;

use leselang_hir::computation::{
    BinaryOperator, Computation, GroupKind, MAX_SCALAR_STRING_BYTES, OptionalStringValue,
    ScalarValue, UnaryOperator,
};
use leselang_hir::host_call::HostOperation;
use leselang_hir::{Effect, HirBranch};

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

fn error(code: &str, message: &str) -> Fault {
    Fault {
        code: code.to_string(),
        message: message.to_string(),
    }
}

fn invalid() -> Fault {
    error("LSV1402", "computation violates its typed HIR contract")
}

fn charge(fuel: &mut u64, cost: u64) -> Result<(), Fault> {
    *fuel = fuel
        .checked_sub(cost)
        .ok_or_else(|| error("LSV1001", "computation fuel exhausted"))?;
    Ok(())
}

fn string_cost(value: &ScalarValue) -> u64 {
    value
        .text()
        .map_or(0, |value| value.len().div_ceil(64) as u64)
}

pub(super) fn evaluate<'a>(
    expression: &'a Computation,
    fuel: &mut u64,
) -> Result<Outcome<'a>, Fault> {
    evaluate_inner(expression, fuel, &mut Vec::new())
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
    fuel: &mut u64,
) -> Result<ScalarValue, Fault> {
    scalar(resume_outcome(binding, value, operation, fuel)?)
}

pub(super) fn resume_group(
    binding: &ResultBinding,
    value: &Value,
    branches: &[HirBranch],
    fuel: &mut u64,
) -> Result<ScalarValue, Fault> {
    scalar(resume_group_outcome(binding, value, branches, fuel)?)
}

pub(super) fn resume_group_outcome<'a>(
    binding: &'a ResultBinding,
    value: &'a Value,
    branches: &'a [HirBranch],
    fuel: &mut u64,
) -> Result<Outcome<'a>, Fault> {
    let mut scope = Vec::with_capacity(binding.locals.len() + 1);
    for local in &binding.locals {
        charge(fuel, 1 + string_cost(&local.value))?;
        scope.push((local.name.clone(), LocalValue::Scalar(local.value.clone())));
    }
    scope.push((
        binding.name.clone(),
        LocalValue::Result(ResultView::Group { value, branches }),
    ));
    evaluate_inner(&binding.body, fuel, &mut scope)
}

pub(super) fn resume_outcome<'a>(
    binding: &'a ResultBinding,
    value: &'a Value,
    operation: HostOperation,
    fuel: &mut u64,
) -> Result<Outcome<'a>, Fault> {
    let mut scope = Vec::with_capacity(binding.locals.len() + 1);
    for local in &binding.locals {
        charge(fuel, 1 + string_cost(&local.value))?;
        scope.push((local.name.clone(), LocalValue::Scalar(local.value.clone())));
    }
    for result in &binding.results {
        charge_projection(fuel, &result.result)?;
        scope.push((
            result.name.clone(),
            LocalValue::Result(ResultView::Projected(&result.result)),
        ));
    }
    for group in &binding.groups {
        charge_group_projection(fuel, &group.group)?;
        scope.push((
            group.name.clone(),
            LocalValue::Result(ResultView::ProjectedGroup(&group.group)),
        ));
    }
    scope.push((
        binding.name.clone(),
        LocalValue::Result(ResultView::Raw { value, operation }),
    ));
    evaluate_inner(&binding.body, fuel, &mut scope)
}

fn charge_projection(fuel: &mut u64, result: &ProjectedResult) -> Result<(), Fault> {
    charge(
        fuel,
        1 + result
            .fields
            .iter()
            .map(|field| 1 + string_cost(&field.value))
            .sum::<u64>(),
    )
}

fn charge_group_projection(fuel: &mut u64, group: &ProjectedGroup) -> Result<(), Fault> {
    charge(fuel, 1)?;
    for member in &group.members {
        charge_projection(fuel, &member.result)?;
    }
    Ok(())
}

fn evaluate_inner<'a>(
    expression: &'a Computation,
    fuel: &mut u64,
    scope: &mut Vec<(String, LocalValue<'a>)>,
) -> Result<Outcome<'a>, Fault> {
    charge(fuel, 1)?;
    let value = match expression {
        Computation::Literal { value } => {
            charge(fuel, string_cost(value))?;
            value.clone()
        }
        Computation::Local { name } => {
            let value = &scope
                .iter()
                .find(|(bound, _)| bound == name)
                .ok_or_else(invalid)?
                .1;
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
                    for (name, value) in scope.iter() {
                        match value {
                            LocalValue::Scalar(value) => {
                                charge(fuel, 1 + string_cost(value))?;
                                locals.push(ScalarBinding {
                                    name: name.clone(),
                                    value: value.clone(),
                                });
                            }
                            LocalValue::Result(
                                value @ (ResultView::Group { .. } | ResultView::ProjectedGroup(_)),
                            ) => {
                                let group = value.snapshot_group()?;
                                charge_group_projection(fuel, &group)?;
                                groups.push(ProjectedGroupBinding {
                                    name: name.clone(),
                                    group,
                                });
                            }
                            LocalValue::Result(value) => {
                                let result = value.snapshot()?;
                                charge_projection(fuel, &result)?;
                                results.push(ProjectedBinding {
                                    name: name.clone(),
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
                Outcome::BoundHost { .. } => return Err(invalid()),
            };
            scope.push((name.clone(), value));
            let result = evaluate_inner(body, fuel, scope);
            scope.pop();
            return result;
        }
        Computation::Loop {
            name,
            initial,
            condition,
            next,
            limit,
        } => {
            let initial = scalar(evaluate_inner(initial, fuel, scope)?)?;
            let state_type = initial.scalar_type();
            let slot = scope.len();
            scope.push((name.clone(), LocalValue::Scalar(initial)));
            let result = (|| {
                for iteration in 0..=*limit {
                    let ScalarValue::Boolean(keep_going) =
                        scalar(evaluate_inner(condition, fuel, scope)?)?
                    else {
                        return Err(invalid());
                    };
                    if !keep_going {
                        return Ok(());
                    }
                    if iteration == *limit {
                        return Err(error("LSV1406", "loop iteration limit exhausted"));
                    }
                    let next = scalar(evaluate_inner(next, fuel, scope)?)?;
                    if next.scalar_type() != state_type {
                        return Err(invalid());
                    }
                    scope.get_mut(slot).ok_or_else(invalid)?.1 = LocalValue::Scalar(next);
                }
                Err(invalid())
            })();
            // Restore the enclosing scope on both normal exit and calculation failure.
            let (_, state) = scope.pop().ok_or_else(invalid)?;
            result?;
            let LocalValue::Scalar(state) = state else {
                return Err(invalid());
            };
            state
        }
        Computation::Member {
            group,
            name,
            operation,
        } => {
            let Some((_, LocalValue::Result(view))) =
                scope.iter().find(|(bound, _)| bound == group)
            else {
                return Err(invalid());
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
                        return Err(invalid());
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
                _ => Err(invalid()),
            };
        }
        Computation::Field { value, field } => {
            let Outcome::Result(value) = evaluate_inner(value, fuel, scope)? else {
                return Err(invalid());
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
                return Err(invalid());
            };
            return evaluate_inner(if when { then } else { otherwise }, fuel, scope);
        }
        Computation::Recover { value, fallback } => {
            match evaluate_inner(value, fuel, scope).and_then(scalar) {
                Ok(value) => value,
                // Only language data errors are recoverable, never resource or host failures.
                Err(fault) if matches!(fault.code.as_str(), "LSV1401" | "LSV1408") => {
                    scalar(evaluate_inner(fallback, fuel, scope)?)?
                }
                Err(fault) => return Err(fault),
            }
        }
        Computation::Host { effect } => return Ok(Outcome::Host(Cow::Borrowed(effect))),
        Computation::Group {
            group_kind,
            branches,
        } => {
            let mut resolved = Vec::with_capacity(branches.len());
            for branch in branches {
                let Outcome::Host(effect) = evaluate_inner(&branch.value, fuel, scope)? else {
                    return Err(invalid());
                };
                if matches!(
                    effect.as_ref(),
                    Effect::Compute { .. } | Effect::Sequence { .. } | Effect::All { .. }
                ) {
                    return Err(invalid());
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
            match (operator, value) {
                (UnaryOperator::OptionalString, ScalarValue::String(value)) => {
                    ScalarValue::OptionalString(OptionalStringValue(Some(value)))
                }
                (UnaryOperator::OptionalString, ScalarValue::None) => {
                    ScalarValue::OptionalString(OptionalStringValue(None))
                }
                (UnaryOperator::HasValue, ScalarValue::OptionalString(value)) => {
                    ScalarValue::Boolean(value.0.is_some())
                }
                (UnaryOperator::Not, ScalarValue::Boolean(value)) => ScalarValue::Boolean(!value),
                (UnaryOperator::Len, ScalarValue::String(value)) => {
                    ScalarValue::Integer(value.chars().count() as u64)
                }
                (UnaryOperator::ToString, value) => {
                    let value = match value {
                        ScalarValue::Integer(value) => value.to_string(),
                        ScalarValue::Boolean(value) => value.to_string(),
                        ScalarValue::String(value) => value,
                        ScalarValue::None | ScalarValue::OptionalString(_) => return Err(invalid()),
                    };
                    let value = ScalarValue::String(value);
                    charge(fuel, string_cost(&value))?;
                    value
                }
                (UnaryOperator::ParseInteger, ScalarValue::String(value)) => {
                    // Reject Rust's optional '+' and all non-decimal spellings explicitly.
                    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
                        return Err(invalid_integer_text());
                    }
                    ScalarValue::Integer(value.parse::<u64>().map_err(|_| invalid_integer_text())?)
                }
                (UnaryOperator::ParseBoolean, ScalarValue::String(value)) => {
                    ScalarValue::Boolean(match value.as_str() {
                        "true" => true,
                        "false" => false,
                        _ => {
                            return Err(error(
                                "LSV1408",
                                "invalid boolean text: expected true or false",
                            ));
                        }
                    })
                }
                _ => return Err(invalid()),
            }
        }
        Computation::Binary {
            operator,
            left,
            right,
        } => {
            let left = scalar(evaluate_inner(left, fuel, scope)?)?;
            match (operator, &left) {
                (BinaryOperator::ValueOr, ScalarValue::OptionalString(value))
                    if value.0.is_some() =>
                {
                    charge(fuel, string_cost(&left))?;
                    return Ok(Outcome::Scalar(ScalarValue::String(
                        value.0.as_ref().ok_or_else(invalid)?.clone(),
                    )));
                }
                (BinaryOperator::And, ScalarValue::Boolean(false)) => {
                    return Ok(Outcome::Scalar(left));
                }
                (BinaryOperator::Or, ScalarValue::Boolean(true)) => {
                    return Ok(Outcome::Scalar(left));
                }
                _ => {}
            }
            let right = scalar(evaluate_inner(right, fuel, scope)?)?;
            charge(fuel, string_cost(&left) + string_cost(&right))?;
            let value = binary(*operator, left, right)?;
            if *operator == BinaryOperator::CharAt {
                charge(fuel, string_cost(&value))?;
            }
            value
        }
    };
    Ok(Outcome::Scalar(value))
}

fn invalid_integer_text() -> Fault {
    error(
        "LSV1408",
        "invalid integer text: expected ASCII decimal within u64",
    )
}

fn binary(op: BinaryOperator, left: ScalarValue, right: ScalarValue) -> Result<ScalarValue, Fault> {
    use BinaryOperator::*;
    use ScalarValue::*;
    if matches!(op, Eq | Ne) && left.scalar_type() == right.scalar_type() {
        return Ok(Boolean(if op == Eq {
            left == right
        } else {
            left != right
        }));
    }
    match (op, left, right) {
        (ValueOr, OptionalString(value), String(fallback)) if value.0.is_none() => {
            Ok(String(fallback))
        }
        (And, Boolean(left), Boolean(right)) => Ok(Boolean(left && right)),
        (Or, Boolean(left), Boolean(right)) => Ok(Boolean(left || right)),
        (Contains, String(left), String(right)) => Ok(Boolean(left.contains(&right))),
        (StartsWith, String(left), String(right)) => Ok(Boolean(left.starts_with(&right))),
        (EndsWith, String(left), String(right)) => Ok(Boolean(left.ends_with(&right))),
        (CharAt, String(left), Integer(index)) => {
            // Check the platform conversion; indices are Unicode scalars, never UTF-8 bytes.
            let value = usize::try_from(index)
                .ok()
                .and_then(|index| left.chars().nth(index))
                .map(|value| value.to_string());
            Ok(OptionalString(OptionalStringValue(value)))
        }
        (Concat, String(mut left), String(right)) => {
            if left.len().saturating_add(right.len()) > MAX_SCALAR_STRING_BYTES {
                return Err(error("LSV1403", "computed string exceeds 4096 bytes"));
            }
            left.push_str(&right);
            Ok(String(left))
        }
        (op, Integer(left), Integer(right)) => {
            let value = match op {
                Lt => return Ok(Boolean(left < right)),
                Le => return Ok(Boolean(left <= right)),
                Gt => return Ok(Boolean(left > right)),
                Ge => return Ok(Boolean(left >= right)),
                Add => left.checked_add(right),
                Sub => left.checked_sub(right),
                Mul => left.checked_mul(right),
                Div => left.checked_div(right),
                Rem => left.checked_rem(right),
                _ => return Err(invalid()),
            };
            value.map(Integer).ok_or_else(|| {
                error(
                    "LSV1401",
                    "integer overflow, underflow, or division by zero",
                )
            })
        }
        _ => Err(invalid()),
    }
}
