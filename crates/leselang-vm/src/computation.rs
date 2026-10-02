use std::borrow::Cow;

use leselang_hir::computation::{
    BinaryOperator, Computation, GroupKind, MAX_SCALAR_STRING_BYTES, ScalarValue, UnaryOperator,
};
use leselang_hir::{Effect, HirBranch};

use crate::{Fault, ResultBinding, ScalarBinding, Value};

pub(super) enum Outcome<'a> {
    Scalar(ScalarValue),
    Result(&'a Value),
    Host(Cow<'a, Effect>),
    BoundHost {
        effect: Cow<'a, Effect>,
        binding: Box<ResultBinding>,
    },
}

#[derive(Clone)]
enum LocalValue<'a> {
    Scalar(ScalarValue),
    Result(&'a Value),
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
    match value {
        ScalarValue::String(value) => value.len().div_ceil(64) as u64,
        _ => 0,
    }
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
    fuel: &mut u64,
) -> Result<ScalarValue, Fault> {
    let mut scope = Vec::with_capacity(binding.locals.len() + 1);
    for local in &binding.locals {
        charge(fuel, 1 + string_cost(&local.value))?;
        scope.push((local.name.clone(), LocalValue::Scalar(local.value.clone())));
    }
    scope.push((binding.name.clone(), LocalValue::Result(value)));
    scalar(evaluate_inner(&binding.body, fuel, &mut scope)?)
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
                LocalValue::Result(value) => return Ok(Outcome::Result(value)),
            }
        }
        Computation::Bind { name, value, body } => {
            let value = match evaluate_inner(value, fuel, scope)? {
                Outcome::Scalar(value) => LocalValue::Scalar(value),
                Outcome::Result(value) => LocalValue::Result(value),
                Outcome::Host(effect) => {
                    let mut locals = Vec::with_capacity(scope.len());
                    for (name, value) in scope.iter() {
                        let LocalValue::Scalar(value) = value else {
                            return Err(invalid());
                        };
                        charge(fuel, 1 + string_cost(value))?;
                        locals.push(ScalarBinding {
                            name: name.clone(),
                            value: value.clone(),
                        });
                    }
                    return Ok(Outcome::BoundHost {
                        effect,
                        binding: Box::new(ResultBinding {
                            name: name.clone(),
                            locals,
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
        Computation::Field { value, field } => {
            let Outcome::Result(value) = evaluate_inner(value, fuel, scope)? else {
                return Err(invalid());
            };
            let value = crate::result_binding::project(value, *field)?;
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
                (UnaryOperator::Not, ScalarValue::Boolean(value)) => ScalarValue::Boolean(!value),
                (UnaryOperator::Len, ScalarValue::String(value)) => {
                    ScalarValue::Integer(value.chars().count() as u64)
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
            binary(*operator, left, right)?
        }
    };
    Ok(Outcome::Scalar(value))
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
        (And, Boolean(left), Boolean(right)) => Ok(Boolean(left && right)),
        (Or, Boolean(left), Boolean(right)) => Ok(Boolean(left || right)),
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
