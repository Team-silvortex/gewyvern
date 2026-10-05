//! Bounded synchronous execution of the shared pure IR, without effect dispatch.

use self::evaluate_preflighted_in_scope as evaluate;
use std::fmt;

use leselang_runtime_core::{
    BinaryOperator, BinarySelection, CalculationFailure, FoldCursor, FoldError, Fuel, LoopBudget,
    LoopError, LoopStep, ScalarValue, ScopeFrame, StringListBuilder, StringListValue,
    StructureBudget, UnaryOperator, apply_binary, apply_unary, select_binary_left,
};

use crate::ir::Computation;
use crate::pure_typing::{
    MAX_TYPE_INFERENCE_BINDINGS, MAX_TYPE_INFERENCE_DEPTH, MAX_TYPE_INFERENCE_NODES, PureTypeError,
    preflight_with_budget, valid_local_name,
};

/// A scalar or host-owned result view. No wire codec or implicit payload formatter.
/// Borrowed views allow lexical reads without copying native result payloads.
#[derive(Clone)]
pub enum PureValue<Result> {
    Scalar(ScalarValue),
    Result(Result),
}

impl<Result> fmt::Debug for PureValue<Result> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Scalar(value) => formatter
                .debug_tuple("Scalar")
                .field(&value.scalar_type())
                .finish(),
            Self::Result(_) => formatter.write_str("Result(<native>)"),
        }
    }
}

/// Developer-owned projection of actual native values, not type metadata.
///
/// Reject fields/members not exported by this exact result. Native work, cloning,
/// allocation and unwinding are the adapter's responsibility; fuel bounds only
/// language work. Callbacks must not dispatch effects or grant host authority.
/// Field/Operation/effect/IR-result slots need no Clone, serde or Send bounds.
pub trait PureEvaluationEnvironment<Field, Operation> {
    type Result: Clone;
    type Error;

    fn field(&self, result: &Self::Result, field: &Field) -> Result<ScalarValue, Self::Error>;

    fn member(
        &self,
        group: &Self::Result,
        name: &str,
        operation: &Operation,
    ) -> Result<Self::Result, Self::Error>;
}

/// Inclusive physical-IR and active-scope policy, independent of execution fuel.
/// Ceilings match the shared type walker: 16,384 nodes, depth 64, 1,024 bindings.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PureEvaluationLimits {
    pub max_nodes: usize,
    pub max_depth: usize,
    pub max_bindings: usize,
}

/// Structural/resource/native failures never enter scalar recovery, even if a
/// native error resembles a language diagnostic. Original native errors move out.
pub enum PureEvaluationFault<Native> {
    InvalidLimits,
    InvalidScope,
    Preflight(PureTypeError),
    InvalidContract,
    BindingLimit,
    FuelExhausted,
    Loop(LoopError),
    Fold(FoldError),
    Native(Native),
}

impl<Native> fmt::Display for PureEvaluationFault<Native> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidLimits => "pure evaluation limits exceed safety ceilings",
            Self::InvalidScope => "pure evaluation scope is invalid",
            Self::Preflight(_) => "pure evaluation physical IR is invalid",
            Self::InvalidContract => "pure evaluation violates its typed IR contract",
            Self::BindingLimit => "pure evaluation active binding limit exceeded",
            Self::FuelExhausted => "pure evaluation fuel exhausted",
            Self::Loop(_) => "pure evaluation loop rejected",
            Self::Fold(_) => "pure evaluation fold rejected",
            Self::Native(_) => "pure evaluation native projection rejected",
        })
    }
}

impl<Native> fmt::Debug for PureEvaluationFault<Native> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}

impl<Native> std::error::Error for PureEvaluationFault<Native> {}

pub type PureEvaluationFailure<Native> = CalculationFailure<PureEvaluationFault<Native>>;

/// Language copy/scan cost; native result payloads are not measured or serialized.
pub fn scalar_copy_cost(value: &ScalarValue) -> u64 {
    match value {
        ScalarValue::StringList(value) => string_list_copy_cost(value),
        _ => value
            .text()
            .map_or(0, |text| text.len().div_ceil(64) as u64),
    }
}

fn string_list_copy_cost(value: &StringListValue) -> u64 {
    value
        .0
        .iter()
        .map(|text| 1 + text.len().div_ceil(64) as u64)
        .sum()
}

/// Execute in a borrowed lexical frame without cloning its visible prefix.
///
/// Entire physical IR (including cold arms) and prefix names/scalar bounds are
/// checked before charging fuel or invoking/cloning native values. This is not
/// static type inference: callers use `infer_pure_type` to check cold types.
/// Evaluation is lazy, synchronous and effect-free; Host/Call/Group are rejected.
/// Temporary bindings unwind through ScopeFrame; prefix entries never change.
/// Each selected node costs one fuel unit, with additional bounded scalar work.
///
/// ```
/// use leselang_hir::ir::Computation;
/// use leselang_hir::pure_evaluation::{
///     PureEvaluationEnvironment, PureEvaluationLimits, PureValue, evaluate_pure_in_scope,
/// };
/// use leselang_runtime_core::{Fuel, ScalarValue, ScopeFrame};
/// struct Numbers;
/// impl PureEvaluationEnvironment<(), ()> for Numbers {
///     type Result = ();
///     type Error = ();
///     fn field(&self, _: &(), _: &()) -> Result<ScalarValue, ()> { Err(()) }
///     fn member(&self, _: &(), _: &str, _: &()) -> Result<(), ()> { Err(()) }
/// }
/// let expression = Computation::<(), (), (), ()>::Literal {
///     value: ScalarValue::Integer(42),
/// };
/// let mut bindings = Vec::new();
/// let mut scope = ScopeFrame::new(&mut bindings);
/// let mut fuel = Fuel::new(10);
/// let result = evaluate_pure_in_scope(&expression, &mut scope, &Numbers, &mut fuel,
///     PureEvaluationLimits { max_nodes: 1, max_depth: 0, max_bindings: 0 }).unwrap();
/// assert!(matches!(result, PureValue::Scalar(ScalarValue::Integer(42))));
/// assert_eq!(fuel.remaining(), 9);
/// ```
pub fn evaluate_pure_in_scope<'names, Field, Operation, HostEffect, IrResult, Environment>(
    expression: &'names Computation<Field, Operation, HostEffect, IrResult>,
    scope: &mut ScopeFrame<'_, 'names, PureValue<Environment::Result>>,
    environment: &Environment,
    fuel: &mut Fuel,
    limits: PureEvaluationLimits,
) -> Result<PureValue<Environment::Result>, PureEvaluationFailure<Environment::Error>>
where
    Environment: PureEvaluationEnvironment<Field, Operation>,
{
    preflight_value_scope(scope, limits)?;
    let mut budget = StructureBudget::new(limits.max_nodes, limits.max_depth);
    preflight_with_budget(expression, 0, &mut budget).map_err(PureEvaluationFault::Preflight)?;
    evaluate_preflighted_in_scope(expression, scope, environment, fuel, limits.max_bindings)
}

pub(crate) fn preflight_value_scope<NativeResult, NativeError>(
    scope: &ScopeFrame<'_, '_, PureValue<NativeResult>>,
    limits: PureEvaluationLimits,
) -> Result<(), PureEvaluationFault<NativeError>> {
    if limits.max_nodes > MAX_TYPE_INFERENCE_NODES
        || limits.max_depth > MAX_TYPE_INFERENCE_DEPTH
        || limits.max_bindings > MAX_TYPE_INFERENCE_BINDINGS
    {
        return Err(PureEvaluationFault::InvalidLimits);
    }
    if scope.len() > limits.max_bindings
        || scope
            .bindings()
            .iter()
            .enumerate()
            .any(|(index, (name, value))| {
                !valid_local_name(name)
                    || scope.bindings()[..index]
                        .iter()
                        .any(|(previous, _)| previous == name)
                    || matches!(value, PureValue::Scalar(value) if !value.is_bounded())
            })
    {
        return Err(PureEvaluationFault::InvalidScope);
    }
    Ok(())
}

fn charge<Native>(fuel: &mut Fuel, cost: u64) -> Result<(), PureEvaluationFailure<Native>> {
    fuel.charge(cost)
        .map_err(|_| PureEvaluationFault::FuelExhausted.into())
}

fn scalar<ResultTag, Native>(
    value: PureValue<ResultTag>,
) -> Result<ScalarValue, PureEvaluationFailure<Native>> {
    match value {
        PureValue::Scalar(value) => Ok(value),
        PureValue::Result(_) => Err(PureEvaluationFault::InvalidContract.into()),
    }
}

fn bind<'names, ResultTag, Native>(
    scope: &mut ScopeFrame<'_, 'names, PureValue<ResultTag>>,
    name: &'names str,
    value: PureValue<ResultTag>,
    max_bindings: usize,
) -> Result<usize, PureEvaluationFailure<Native>> {
    if scope.len() >= max_bindings {
        return Err(PureEvaluationFault::BindingLimit.into());
    }
    scope
        .push(name, value)
        .map_err(|_| PureEvaluationFault::InvalidContract.into())
}

// Internal callers must preflight the entire physical tree and value prefix
// under the fixed safety ceilings before entering this recursive executor.
pub(crate) fn evaluate_preflighted_in_scope<
    'names,
    Field,
    Operation,
    HostEffect,
    IrResult,
    Environment,
>(
    expression: &'names Computation<Field, Operation, HostEffect, IrResult>,
    scope: &mut ScopeFrame<'_, 'names, PureValue<Environment::Result>>,
    environment: &Environment,
    fuel: &mut Fuel,
    max_bindings: usize,
) -> Result<PureValue<Environment::Result>, PureEvaluationFailure<Environment::Error>>
where
    Environment: PureEvaluationEnvironment<Field, Operation>,
{
    use Computation::*;
    charge(fuel, 1)?;
    let value = match expression {
        Literal { value } => {
            charge(fuel, scalar_copy_cost(value))?;
            value.clone()
        }
        Strings { items } => {
            let mut values = StringListBuilder::with_capacity(items.len())
                .map_err(CalculationFailure::Scalar)?;
            for item in items {
                let ScalarValue::String(value) =
                    scalar(evaluate(item, scope, environment, fuel, max_bindings)?)?
                else {
                    return Err(PureEvaluationFault::InvalidContract.into());
                };
                values.try_push(value).map_err(CalculationFailure::Scalar)?;
            }
            let value = ScalarValue::StringList(values.finish());
            charge(fuel, scalar_copy_cost(&value))?;
            value
        }
        Local { name } => match scope.get(name) {
            Some(PureValue::Scalar(value)) => {
                charge(fuel, scalar_copy_cost(value))?;
                value.clone()
            }
            Some(PureValue::Result(value)) => return Ok(PureValue::Result(value.clone())),
            None => return Err(PureEvaluationFault::InvalidContract.into()),
        },
        Bind { name, value, body } => {
            let value = evaluate(value, scope, environment, fuel, max_bindings)?;
            let mut local = scope.nested();
            bind(&mut local, name, value, max_bindings)?;
            return evaluate(body, &mut local, environment, fuel, max_bindings);
        }
        Member {
            group,
            name,
            operation,
        } => {
            let Some(PureValue::Result(group)) = scope.get(group) else {
                return Err(PureEvaluationFault::InvalidContract.into());
            };
            return environment
                .member(group, name, operation)
                .map(PureValue::Result)
                .map_err(|error| PureEvaluationFault::Native(error).into());
        }
        Field { value, field } => {
            let PureValue::Result(value) = evaluate(value, scope, environment, fuel, max_bindings)?
            else {
                return Err(PureEvaluationFault::InvalidContract.into());
            };
            let value = environment
                .field(&value, field)
                .map_err(PureEvaluationFault::Native)?;
            if !value.is_bounded() {
                return Err(PureEvaluationFault::InvalidContract.into());
            }
            charge(fuel, scalar_copy_cost(&value))?;
            value
        }
        Choose {
            when,
            then,
            otherwise,
        } => {
            let ScalarValue::Boolean(when) =
                scalar(evaluate(when, scope, environment, fuel, max_bindings)?)?
            else {
                return Err(PureEvaluationFault::InvalidContract.into());
            };
            return evaluate(
                if when { then } else { otherwise },
                scope,
                environment,
                fuel,
                max_bindings,
            );
        }
        Recover { value, fallback } => {
            match evaluate(value, scope, environment, fuel, max_bindings).and_then(scalar) {
                Ok(value) => value,
                Err(failure) if failure.is_recoverable() => {
                    scalar(evaluate(fallback, scope, environment, fuel, max_bindings)?)?
                }
                Err(failure) => return Err(failure),
            }
        }
        Loop {
            name,
            initial,
            condition,
            next,
            limit,
        } => {
            let initial = scalar(evaluate(initial, scope, environment, fuel, max_bindings)?)?;
            let mut budget = LoopBudget::new(initial.scalar_type(), *limit)
                .map_err(PureEvaluationFault::Loop)?;
            let mut local = scope.nested();
            let slot = bind(&mut local, name, PureValue::Scalar(initial), max_bindings)?;
            loop {
                let ScalarValue::Boolean(keep_going) = scalar(evaluate(
                    condition,
                    &mut local,
                    environment,
                    fuel,
                    max_bindings,
                )?)?
                else {
                    return Err(PureEvaluationFault::InvalidContract.into());
                };
                if budget
                    .check_condition(keep_going)
                    .map_err(PureEvaluationFault::Loop)?
                    == LoopStep::Done
                {
                    break;
                }
                let next = scalar(evaluate(next, &mut local, environment, fuel, max_bindings)?)?;
                budget.advance(&next).map_err(PureEvaluationFault::Loop)?;
                *local
                    .get_local_mut(slot)
                    .ok_or(PureEvaluationFault::InvalidContract)? = PureValue::Scalar(next);
            }
            let (_, state) = local.pop().ok_or(PureEvaluationFault::InvalidContract)?;
            return Ok(state);
        }
        Fold {
            name,
            item,
            items,
            initial,
            next,
            limit,
        } => {
            let ScalarValue::StringList(items) =
                scalar(evaluate(items, scope, environment, fuel, max_bindings)?)?
            else {
                return Err(PureEvaluationFault::InvalidContract.into());
            };
            let initial = scalar(evaluate(initial, scope, environment, fuel, max_bindings)?)?;
            let items_cost = string_list_copy_cost(&items);
            let mut cursor = FoldCursor::new(items, initial.scalar_type(), *limit)
                .map_err(PureEvaluationFault::Fold)?;
            charge(fuel, items_cost)?;
            let mut local = scope.nested();
            let slot = bind(&mut local, name, PureValue::Scalar(initial), max_bindings)?;
            let item_slot = bind(
                &mut local,
                item,
                PureValue::Scalar(ScalarValue::String(String::new())),
                max_bindings,
            )?;
            while let Some(item) = cursor.next_item().map_err(PureEvaluationFault::Fold)? {
                charge(fuel, 1 + item.len().div_ceil(64) as u64)?;
                *local
                    .get_local_mut(item_slot)
                    .ok_or(PureEvaluationFault::InvalidContract)? =
                    PureValue::Scalar(ScalarValue::String(item));
                let next = scalar(evaluate(next, &mut local, environment, fuel, max_bindings)?)?;
                cursor.advance(&next).map_err(PureEvaluationFault::Fold)?;
                *local
                    .get_local_mut(slot)
                    .ok_or(PureEvaluationFault::InvalidContract)? = PureValue::Scalar(next);
            }
            local.pop();
            let (_, state) = local.pop().ok_or(PureEvaluationFault::InvalidContract)?;
            return Ok(state);
        }
        Unary { operator, value } => {
            let value = scalar(evaluate(value, scope, environment, fuel, max_bindings)?)?;
            charge(fuel, scalar_copy_cost(&value))?;
            let value = apply_unary(*operator, value).map_err(CalculationFailure::Scalar)?;
            if *operator == UnaryOperator::ToString {
                charge(fuel, scalar_copy_cost(&value))?;
            }
            value
        }
        Binary {
            operator,
            left,
            right,
        } => {
            let left = scalar(evaluate(left, scope, environment, fuel, max_bindings)?)?;
            let left =
                match select_binary_left(*operator, left).map_err(CalculationFailure::Scalar)? {
                    BinarySelection::Complete(value) => {
                        if *operator == BinaryOperator::ValueOr {
                            charge(fuel, scalar_copy_cost(&value))?;
                        }
                        return Ok(PureValue::Scalar(value));
                    }
                    BinarySelection::NeedsRight(left) => left,
                };
            let right = scalar(evaluate(right, scope, environment, fuel, max_bindings)?)?;
            charge(fuel, scalar_copy_cost(&left) + scalar_copy_cost(&right))?;
            let value = apply_binary(*operator, left, right).map_err(CalculationFailure::Scalar)?;
            if matches!(
                operator,
                BinaryOperator::CharAt
                    | BinaryOperator::ItemAt
                    | BinaryOperator::Split
                    | BinaryOperator::Join
                    | BinaryOperator::Append
            ) {
                charge(fuel, scalar_copy_cost(&value))?;
            }
            value
        }
        Host { .. } | Call { .. } | Group { .. } => {
            return Err(PureEvaluationFault::InvalidContract.into());
        }
    };
    Ok(PureValue::Scalar(value))
}
