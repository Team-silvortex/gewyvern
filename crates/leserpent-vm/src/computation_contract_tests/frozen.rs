// Frozen pre-migration product computation policy. Test-only compatibility oracle.
use std::borrow::Cow;

use leselang_hir_core::call_evaluation::CallEvaluationError;
use leselang_hir_core::call_typing::{CallTypeError, MAX_CALL_ARGUMENTS};
use leselang_hir_core::effect_evaluation::{
    EffectEvaluationFailure, EffectEvaluationFault, EffectEvaluationLimits,
    EffectEvaluationOutcome, evaluate_effects_in_scope,
};
use leselang_hir_core::pure_evaluation::{
    PureEvaluationFailure, PureEvaluationFault, PureEvaluationLimits, PureValue,
    scalar_copy_cost as string_cost,
};
use leselang_hir_core::pure_typing::{
    MAX_TYPE_INFERENCE_BINDINGS, MAX_TYPE_INFERENCE_DEPTH, MAX_TYPE_INFERENCE_NODES,
};
use leselang_runtime_core::{
    CalculationFailure, CaptureScopeError, CaptureSnapshotError, CaptureSnapshotLimits, FoldError,
    Fuel, LoopError, ScalarError, ScopeFrame,
};
use leserpent_hir::computation::{Computation, ScalarValue};
use leserpent_hir::host_call::HostOperation;
use leserpent_hir::{Effect, HirBranch};

use crate::result_binding::{ProjectedGroup, ProjectedResult, group_capture_limits};
use crate::{Fault, ResultBinding, Value};

pub(super) use leselang_command::ResultView;

fn view_fault(error: leselang_command::ResultViewError) -> Fault {
    match error {
        leselang_command::ResultViewError::InvalidAccess => invalid(),
        leselang_command::ResultViewError::InvalidProjection => crate::result_binding::invalid(),
    }
}

fn result_environment<'a>()
-> leselang_command::PreflightedResultEnvironment<'a, 'static, ReferenceCaptureHost> {
    leselang_command::PreflightedResultEnvironment::new(
        &ReferenceCaptureHost {
            restore_saved_results: true,
        },
        leselang_command::ResultExecutionLimits {
            effects: execution_limits(),
            captures: leselang_command::ResultCaptureLimits {
                snapshot: CaptureSnapshotLimits {
                    max_bindings: MAX_TYPE_INFERENCE_BINDINGS,
                    max_name_bytes: leselang_hir_core::pure_typing::MAX_LOCAL_NAME_BYTES,
                },
                groups: group_capture_limits(),
            },
        },
    )
}

pub(super) use leselang_command::execution_outcome::ComputationOutcome as Outcome;

type LocalValue<'a> = PureValue<ResultView<'a>>;

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
    resume_captured_outcome(binding, ResultView::Group { value, branches }, false, fuel)
}

pub(super) fn resume_outcome<'a>(
    binding: &'a ResultBinding,
    value: &'a Value,
    operation: HostOperation,
    fuel: &mut Fuel,
) -> Result<Outcome<'a>, Fault> {
    resume_captured_outcome(binding, ResultView::Raw { value, operation }, true, fuel)
}

fn resume_captured_outcome<'a>(
    binding: &'a ResultBinding,
    pending: ResultView<'a>,
    restore_saved_results: bool,
    fuel: &mut Fuel,
) -> Result<Outcome<'a>, Fault> {
    let mut bindings =
        Vec::with_capacity(binding.locals.len() + binding.results.len() + binding.groups.len() + 1);
    let mut scope = ScopeFrame::new(&mut bindings);
    leselang_command::resume_preflighted_result_binding_in_scope(
        binding,
        &mut scope,
        &result_environment(),
        fuel,
        execution_limits(),
        pending,
        &ReferenceCaptureHost {
            restore_saved_results,
        },
    )
    .map(|outcome| effect_outcome(outcome.into_legacy()))
    .map_err(|error| match error {
        CaptureScopeError::Duplicate { .. } => invalid(),
        CaptureScopeError::Native { error, .. } => calculation_fault(effect_failure(error)),
    })
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

struct ReferenceCaptureHost {
    restore_saved_results: bool,
}

impl leselang_command::ResultCaptureHost for ReferenceCaptureHost {
    type Error = Fault;

    fn charge_scalar(&self, value: &ScalarValue, fuel: &mut Fuel) -> Result<(), Fault> {
        charge(fuel, 1 + string_cost(value))
    }

    fn charge_result(&self, result: &ProjectedResult, fuel: &mut Fuel) -> Result<(), Fault> {
        charge_projection(fuel, result)
    }

    fn charge_group(&self, group: &ProjectedGroup, fuel: &mut Fuel) -> Result<(), Fault> {
        charge_group_projection(fuel, group)
    }

    fn projection_error(&self, error: leselang_command::ResultViewError) -> Fault {
        view_fault(error)
    }

    fn allow_saved_projection(&self) -> Result<(), Fault> {
        // GroupResultBinding rejects saved result/group rows before re-entry.
        if self.restore_saved_results {
            Ok(())
        } else {
            Err(invalid())
        }
    }
}

impl leselang_command::ResultExecutionHost for ReferenceCaptureHost {
    fn preflight_effect(&self, _: &Computation) -> Result<(), Fault> {
        // Canonical HIR/type/authority gates already admitted the reference execution.
        Ok(())
    }
    fn before_prepare_effect(&self, _: &Computation) -> Result<(), Fault> {
        Ok(())
    }
    fn prepared_effect(&self, _: &Effect) -> Result<(), Fault> {
        Ok(())
    }
    fn before_projection(&self, _: leserpent_hir::result_field::ResultField) -> Result<(), Fault> {
        Ok(())
    }
    fn valid_group_name(&self, name: &str) -> bool {
        crate::valid_merge_branch_name(name)
    }
    fn preparation_failure(
        &self,
        failure: leselang_command::EffectPreparationError<Fault>,
    ) -> EvaluationFailure {
        preparation_failure(failure)
    }
    fn capture_failure(&self, failure: CaptureSnapshotError<Fault>) -> Fault {
        match failure {
            CaptureSnapshotError::Native { error, .. } => error,
            _ => invalid(),
        }
    }
}

fn evaluate_inner<'a>(
    expression: &'a Computation,
    fuel: &mut Fuel,
    scope: &mut ScopeFrame<'_, 'a, LocalValue<'a>>,
) -> Result<Outcome<'a>, EvaluationFailure> {
    evaluate_effects_in_scope(
        expression,
        scope,
        &result_environment(),
        fuel,
        execution_limits(),
    )
    .map(effect_outcome)
    .map_err(effect_failure)
}

fn execution_limits() -> EffectEvaluationLimits {
    EffectEvaluationLimits {
        pure: PureEvaluationLimits {
            max_nodes: MAX_TYPE_INFERENCE_NODES,
            max_depth: MAX_TYPE_INFERENCE_DEPTH,
            max_bindings: MAX_TYPE_INFERENCE_BINDINGS,
        },
        max_arguments: MAX_CALL_ARGUMENTS,
        max_branches: leserpent_hir::MAX_ALL_BRANCHES,
    }
}

fn effect_outcome<'a>(
    outcome: EffectEvaluationOutcome<ResultView<'a>, Cow<'a, Effect>, Box<ResultBinding>>,
) -> Outcome<'a> {
    match outcome {
        EffectEvaluationOutcome::Value(PureValue::Scalar(value)) => Outcome::Scalar(value),
        EffectEvaluationOutcome::Value(PureValue::Result(_)) => Outcome::Result,
        EffectEvaluationOutcome::Request(effect) => Outcome::Host(effect),
        EffectEvaluationOutcome::Suspended { request, capture } => Outcome::BoundHost {
            effect: request,
            binding: capture,
        },
    }
}

fn effect_failure(failure: EffectEvaluationFailure<Fault>) -> EvaluationFailure {
    match failure {
        CalculationFailure::Scalar(error) => CalculationFailure::Scalar(error),
        CalculationFailure::External(EffectEvaluationFault::Pure(failure)) => {
            pure_failure(CalculationFailure::External(failure))
        }
        CalculationFailure::External(EffectEvaluationFault::Native(fault)) => {
            CalculationFailure::External(fault)
        }
        CalculationFailure::External(_) => invalid().into(),
    }
}

fn pure_failure(failure: PureEvaluationFailure<Fault>) -> EvaluationFailure {
    match failure {
        CalculationFailure::Scalar(error) => CalculationFailure::Scalar(error),
        CalculationFailure::External(failure) => CalculationFailure::External(match failure {
            PureEvaluationFault::Native(fault) => fault,
            PureEvaluationFault::FuelExhausted => error("LSV1001", "computation fuel exhausted"),
            PureEvaluationFault::Loop(failure) => loop_fault(failure),
            PureEvaluationFault::Fold(failure) => fold_fault(failure),
            _ => invalid(),
        }),
    }
}

fn preparation_failure(
    failure: leselang_command::EffectPreparationError<Fault>,
) -> EvaluationFailure {
    use leselang_command::EffectPreparationError;
    match failure {
        EffectPreparationError::InvalidComputation => invalid().into(),
        EffectPreparationError::Evaluation(failure) => effect_failure(failure),
        EffectPreparationError::Call { operation, failure } => call_failure(failure, operation),
        EffectPreparationError::Resolution {
            operation,
            diagnostic_code,
        } => error(
            "LSV1404",
            &format!(
                "computed arguments rejected by {} ({diagnostic_code})",
                operation.name()
            ),
        )
        .into(),
    }
}

fn call_failure(
    failure: CallEvaluationError<Fault>,
    operation: HostOperation,
) -> EvaluationFailure {
    match failure {
        CallEvaluationError::Evaluation { failure, .. } => pure_failure(failure),
        CallEvaluationError::Scope(failure) => pure_failure(CalculationFailure::External(failure)),
        CallEvaluationError::FuelExhausted => error("LSV1001", "computation fuel exhausted").into(),
        CallEvaluationError::Argument { .. }
        | CallEvaluationError::Preflight(CallTypeError::Names(_)) => error(
            "LSV1404",
            &format!(
                "computed arguments rejected by {} (LSH1407)",
                operation.name()
            ),
        )
        .into(),
        _ => invalid().into(),
    }
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
