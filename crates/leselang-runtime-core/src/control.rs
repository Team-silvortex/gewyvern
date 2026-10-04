use crate::{BinaryOperator, MAX_SCALAR_STRING_BYTES, ScalarError, ScalarType, ScalarValue};

pub const MAX_LOOP_ITERATIONS: u64 = 1_024;

/// A decision about an evaluated left value, not evaluation of the right expression.
/// This contains language data; formatting it is not a redacted observation API.
#[must_use = "respect the selection before evaluating the right expression"]
#[derive(Debug, Eq, PartialEq)]
pub enum BinarySelection {
    Complete(ScalarValue),
    /// Carries the original left value without cloning or certifying its full signature.
    NeedsRight(ScalarValue),
}

/// Select the original lazy paths of `and`, `or` and `value_or`.
///
/// The caller must preflight both expression types and capabilities, including
/// cold paths, then evaluate and charge the left expression before calling this.
/// A `Complete` value is bounded. `NeedsRight` is not a validation certificate:
/// evaluate/charge the right expression and use `apply_binary` to validate both
/// supplied values. Non-lazy operations defer validation to that eager API.
/// No expression, callback, fuel grant, host operation or authority is evaluated.
///
/// ```
/// use leselang_runtime_core::{BinaryOperator, BinarySelection, ScalarValue, select_binary_left};
/// assert_eq!(select_binary_left(BinaryOperator::And, ScalarValue::Boolean(false)),
///            Ok(BinarySelection::Complete(ScalarValue::Boolean(false))));
/// ```
pub fn select_binary_left(
    operator: BinaryOperator,
    left: ScalarValue,
) -> Result<BinarySelection, ScalarError> {
    match (operator, left) {
        (BinaryOperator::And, ScalarValue::Boolean(false)) => {
            Ok(BinarySelection::Complete(ScalarValue::Boolean(false)))
        }
        (BinaryOperator::Or, ScalarValue::Boolean(true)) => {
            Ok(BinarySelection::Complete(ScalarValue::Boolean(true)))
        }
        (BinaryOperator::And | BinaryOperator::Or, value @ ScalarValue::Boolean(_)) => {
            Ok(BinarySelection::NeedsRight(value))
        }
        (BinaryOperator::ValueOr, ScalarValue::OptionalString(value)) => match value.0 {
            Some(text) if text.len() <= MAX_SCALAR_STRING_BYTES => {
                Ok(BinarySelection::Complete(ScalarValue::String(text)))
            }
            Some(_) => Err(ScalarError::UnboundedOperand),
            None => Ok(BinarySelection::NeedsRight(ScalarValue::OptionalString(
                value,
            ))),
        },
        (BinaryOperator::And | BinaryOperator::Or | BinaryOperator::ValueOr, _) => {
            Err(ScalarError::TypeMismatch)
        }
        (_, left) => Ok(BinarySelection::NeedsRight(left)),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoopStep {
    Continue,
    Done,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LoopPhase {
    Condition,
    Advance,
    Done,
    Exhausted,
}

/// Allocation-free condition-first scalar loop accounting, not an interpreter.
///
/// The adapter owns bounded initial state, lexical scope, expression evaluation,
/// fuel, cleanup and error recovery. This budget sees only the initial type.
/// Only a successfully validated `advance` counts an
/// iteration. A false condition may end the loop at its limit; a true condition
/// at the limit exhausts it without evaluating another next expression.
/// No cloning, refill, default grant or serialization restores an execution.
/// A trusted adapter can construct a new budget; this is not an authority fence.
///
/// ```
/// use leselang_runtime_core::{LoopBudget, LoopStep, ScalarType, ScalarValue};
/// let mut budget = LoopBudget::new(ScalarType::Integer, 1).unwrap();
/// assert_eq!(budget.check_condition(true), Ok(LoopStep::Continue));
/// budget.advance(&ScalarValue::Integer(1)).unwrap();
/// assert_eq!(budget.check_condition(false), Ok(LoopStep::Done));
/// assert_eq!(budget.completed_iterations(), 1);
/// ```
///
/// ```compile_fail
/// use leselang_runtime_core::{LoopBudget, ScalarType};
/// let budget = LoopBudget::new(ScalarType::Integer, 1).unwrap();
/// let duplicate = budget.clone();
/// ```
///
/// ```compile_fail
/// use leselang_runtime_core::{LoopBudget, ScalarType};
/// let budget = LoopBudget::new(ScalarType::Integer, 1).unwrap();
/// let moved = budget;
/// assert_eq!(budget.completed_iterations(), 0);
/// ```
///
/// ```compile_fail
/// use leselang_runtime_core::LoopBudget;
/// let budget = LoopBudget::default();
/// ```
///
/// ```compile_fail
/// use leselang_runtime_core::LoopBudget;
/// let budget: LoopBudget = serde_json::from_str("{}").unwrap();
/// ```
///
/// ```compile_fail
/// use leselang_runtime_core::{LoopBudget, ScalarType};
/// let wire = serde_json::to_string(&LoopBudget::new(ScalarType::Integer, 1).unwrap()).unwrap();
/// ```
#[must_use = "retain one budget for the entire loop without resetting its counter"]
#[derive(Debug)]
pub struct LoopBudget {
    state_type: ScalarType,
    limit: u64,
    completed: u64,
    phase: LoopPhase,
}

impl LoopBudget {
    pub fn new(state_type: ScalarType, limit: u64) -> Result<Self, LoopError> {
        if limit > MAX_LOOP_ITERATIONS {
            return Err(LoopError::InvalidLimit);
        }
        Ok(Self {
            state_type,
            limit,
            completed: 0,
            phase: LoopPhase::Condition,
        })
    }

    pub const fn completed_iterations(&self) -> u64 {
        self.completed
    }

    /// Call only after evaluating the condition under the adapter's remaining fuel.
    pub fn check_condition(&mut self, keep_going: bool) -> Result<LoopStep, LoopError> {
        if self.phase != LoopPhase::Condition {
            return Err(LoopError::InvalidPhase);
        }
        if !keep_going {
            self.phase = LoopPhase::Done;
            return Ok(LoopStep::Done);
        }
        if self.completed == self.limit {
            self.phase = LoopPhase::Exhausted;
            return Err(LoopError::IterationLimit);
        }
        self.phase = LoopPhase::Advance;
        Ok(LoopStep::Continue)
    }

    /// Validate a next state before the adapter replaces its scoped value.
    /// Validation/ordering errors leave the counter and phase unchanged; they do
    /// not refund fuel or authorize retry of a failed expression/host operation.
    pub fn advance(&mut self, next: &ScalarValue) -> Result<(), LoopError> {
        if self.phase != LoopPhase::Advance {
            return Err(LoopError::InvalidPhase);
        }
        if next.scalar_type() != self.state_type {
            return Err(LoopError::StateTypeMismatch);
        }
        if !next.is_bounded() {
            return Err(LoopError::UnboundedState);
        }
        // The condition phase admits an advance only while completed < limit <= 1024.
        self.completed += 1;
        self.phase = LoopPhase::Condition;
        Ok(())
    }
}

/// Fixed, payload-free control failures; the adapter owns diagnostic/recovery policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoopError {
    InvalidLimit,
    InvalidPhase,
    StateTypeMismatch,
    UnboundedState,
    IterationLimit,
}

impl std::fmt::Display for LoopError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::InvalidLimit => "loop limit exceeds 1024 iterations",
            Self::InvalidPhase => "loop operation violates condition/advance order",
            Self::StateTypeMismatch => "loop next must preserve the initial state's scalar type",
            Self::UnboundedState => "loop next state exceeds its text or list bounds",
            Self::IterationLimit => "loop iteration limit exhausted",
        })
    }
}

impl std::error::Error for LoopError {}
