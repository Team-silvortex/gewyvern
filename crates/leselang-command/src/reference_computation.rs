//! Released product computation policy over the existing HIR/runtime-core evaluator.
//!
//! These are preflighted execution entries, not an untrusted-input sandbox. Callers
//! must bound/admit HIR, captures and raw replies, validate canonical types and
//! original reply correspondence, and retain current authority before entry.
//! The caller's meter is used unchanged: no default grant, refill or refund.
//! Results are computation observations, not authenticated requests or receipts.
//! Request construction, identities, dispatch, journals, settlement and retry
//! remain with the native owner. There is no second interpreter or storage here.

use std::borrow::Cow;

use leselang_hir::call_evaluation::CallEvaluationError;
use leselang_hir::call_typing::{CallTypeError, MAX_CALL_ARGUMENTS};
use leselang_hir::computation::{Computation, ScalarValue};
use leselang_hir::effect_evaluation::{
    EffectEvaluationFailure, EffectEvaluationFault, EffectEvaluationLimits,
    EffectEvaluationOutcome, evaluate_effects_in_scope,
};
use leselang_hir::host_call::HostOperation;
use leselang_hir::pure_evaluation::{
    PureEvaluationFailure, PureEvaluationFault, PureEvaluationLimits, PureValue,
    scalar_copy_cost as string_cost,
};
use leselang_hir::pure_typing::{
    MAX_TYPE_INFERENCE_BINDINGS, MAX_TYPE_INFERENCE_DEPTH, MAX_TYPE_INFERENCE_NODES,
};
use leselang_hir::{Effect, HirBranch};
use leselang_runtime_core::{
    CalculationFailure, CaptureScopeError, CaptureSnapshotError, CaptureSnapshotLimits, FoldError,
    Fuel, LoopError, ScalarError, ScopeFrame,
};

use crate::{ProjectedGroup, ProjectedResult};
use crate::{ResultBinding, Value};
use leselang_runtime_core::Fault;

use crate::ResultView;

fn group_capture_limits() -> leselang_runtime_core::GroupCaptureLimits {
    crate::reference_request::result_binding_limits().groups
}

fn invalid_projection() -> Fault {
    crate::reference_request::continuation_admission_fault(
        crate::ContinuationAdmissionError::Binding,
    )
}

fn valid_group_name(name: &str) -> bool {
    crate::valid_merge_branch_name(name, crate::MAX_MERGE_BRANCH_NAME_BYTES)
}

fn view_fault(error: crate::ResultViewError) -> Fault {
    match error {
        crate::ResultViewError::InvalidAccess => invalid(),
        crate::ResultViewError::InvalidProjection => invalid_projection(),
    }
}

fn result_environment<'a>() -> crate::PreflightedResultEnvironment<'a, 'static, ReferenceCaptureHost>
{
    crate::PreflightedResultEnvironment::new(
        &ReferenceCaptureHost {
            restore_saved_results: true,
        },
        crate::ResultExecutionLimits {
            effects: execution_limits(),
            captures: crate::ResultCaptureLimits {
                snapshot: CaptureSnapshotLimits {
                    max_bindings: MAX_TYPE_INFERENCE_BINDINGS,
                    max_name_bytes: leselang_hir::pure_typing::MAX_LOCAL_NAME_BYTES,
                },
                groups: group_capture_limits(),
            },
        },
    )
}

use crate::execution_outcome::ComputationOutcome as Outcome;

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

/// Evaluate admitted product HIR using the original caller-owned meter.
/// See the module-level admission and authority requirements.
pub fn evaluate<'a>(expression: &'a Computation, fuel: &mut Fuel) -> Result<Outcome<'a>, Fault> {
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

/// Resume an admitted atomic capture, requiring a scalar result.
/// See the module-level admission and authority requirements.
pub fn resume(
    binding: &ResultBinding,
    value: &Value,
    operation: HostOperation,
    fuel: &mut Fuel,
) -> Result<ScalarValue, Fault> {
    scalar(resume_outcome(binding, value, operation, fuel)?)
}

/// Resume an admitted group capture, requiring a scalar result.
/// See the module-level admission and authority requirements.
pub fn resume_group(
    binding: &ResultBinding,
    value: &Value,
    branches: &[HirBranch],
    fuel: &mut Fuel,
) -> Result<ScalarValue, Fault> {
    scalar(resume_group_outcome(binding, value, branches, fuel)?)
}

/// Resume an admitted group capture without admitting saved result/group rows.
/// See the module-level admission and authority requirements.
pub fn resume_group_outcome<'a>(
    binding: &'a ResultBinding,
    value: &'a Value,
    branches: &'a [HirBranch],
    fuel: &mut Fuel,
) -> Result<Outcome<'a>, Fault> {
    resume_captured_outcome(binding, ResultView::Group { value, branches }, false, fuel)
}

/// Resume an admitted atomic capture, retaining borrowed effects and owned captures.
/// See the module-level admission and authority requirements.
pub fn resume_outcome<'a>(
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
    crate::resume_preflighted_result_binding_in_scope(
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

impl crate::ResultCaptureHost for ReferenceCaptureHost {
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

    fn projection_error(&self, error: crate::ResultViewError) -> Fault {
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

impl crate::ResultExecutionHost for ReferenceCaptureHost {
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
    fn before_projection(&self, _: leselang_hir::result_field::ResultField) -> Result<(), Fault> {
        Ok(())
    }
    fn valid_group_name(&self, name: &str) -> bool {
        valid_group_name(name)
    }
    fn preparation_failure(
        &self,
        failure: crate::EffectPreparationError<Fault>,
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
        max_branches: leselang_hir::MAX_ALL_BRANCHES,
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

fn preparation_failure(failure: crate::EffectPreparationError<Fault>) -> EvaluationFailure {
    use crate::EffectPreparationError;
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

#[cfg(test)]
use leselang_hir::{
    effect_evaluation::EffectEvaluationEnvironment, pure_evaluation::PureEvaluationEnvironment,
};

#[cfg(test)]
trait ReferenceResultView {
    fn field(self, field: leselang_hir::result_field::ResultField) -> Result<ScalarValue, Fault>;
    fn snapshot(self) -> Result<ProjectedResult, Fault>;
    fn snapshot_group(self) -> Result<ProjectedGroup, Fault>;
}

#[cfg(test)]
impl ReferenceResultView for ResultView<'_> {
    fn field(self, field: leselang_hir::result_field::ResultField) -> Result<ScalarValue, Fault> {
        self.project_scalar(field).map_err(view_fault)
    }

    fn snapshot(self) -> Result<ProjectedResult, Fault> {
        self.capture_result().map_err(view_fault)
    }

    fn snapshot_group(self) -> Result<ProjectedGroup, Fault> {
        self.capture_group(group_capture_limits(), valid_group_name)
            .map_err(view_fault)
    }
}

#[cfg(test)]
#[path = "reference_computation/effect_preparation_tests.rs"]
mod effect_preparation_tests;

#[cfg(test)]
#[path = "reference_computation/result_view_tests.rs"]
mod result_view_tests;

#[cfg(test)]
mod recovery_tests {
    use super::*;
    use leselang_hir::computation::UnaryOperator;

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
        assert_eq!(fuel.remaining(), 100);
        drop(scope);
        let expression = Computation::Recover {
            value: Box::new(Computation::Unary {
                operator: leselang_hir::computation::UnaryOperator::ParseInteger,
                value: Box::new(Computation::Literal {
                    value: ScalarValue::String("invalid".into()),
                }),
            }),
            fallback: Box::new(Computation::Literal {
                value: ScalarValue::Integer(7),
            }),
        };
        let mut bindings = Vec::new();
        let mut scope = ScopeFrame::new(&mut bindings);
        let mut fuel = Fuel::new(0);
        let Err(failure) = evaluate_inner(&expression, &mut fuel, &mut scope) else {
            panic!()
        };
        assert!(!failure.is_recoverable());
        assert_eq!(calculation_fault(failure).code, "LSV1001");
        assert_eq!(fuel.remaining(), 0);
    }
}

#[cfg(test)]
mod captured_reentry_tests {
    use super::*;
    use crate::{ProjectedBinding, ProjectedField, ProjectedGroupBinding};
    use leselang_hir::computation::{BinaryOperator, ComputedArgument, UnaryOperator};
    use leselang_hir::result_field::ResultField;
    use leselang_runtime_core::ScalarBinding;

    #[derive(Debug, PartialEq)]
    enum Observed {
        Scalar(ScalarValue),
        Result,
        Host(Effect),
        Bound(Effect, Box<ResultBinding>),
    }
    fn observe(outcome: Result<Outcome<'_>, Fault>) -> Result<Observed, Fault> {
        outcome.map(|outcome| match outcome {
            Outcome::Scalar(value) => Observed::Scalar(value),
            Outcome::Result => Observed::Result,
            Outcome::Host(effect) => Observed::Host(effect.into_owned()),
            Outcome::BoundHost { effect, binding } => Observed::Bound(effect.into_owned(), binding),
        })
    }
    fn local(name: &str) -> Computation {
        Computation::Local { name: name.into() }
    }
    fn field(value: Computation) -> Computation {
        Computation::Field {
            value: Box::new(value),
            field: ResultField::NodeId,
        }
    }
    fn concat(left: Computation, right: Computation) -> Computation {
        Computation::Binary {
            operator: BinaryOperator::Concat,
            left: Box::new(left),
            right: Box::new(right),
        }
    }
    fn member(group: &str) -> Computation {
        Computation::Member {
            group: group.into(),
            name: "member".into(),
            operation: HostOperation::UiFocus,
        }
    }
    fn projection(node: &str) -> ProjectedResult {
        ProjectedResult {
            projection_version: 1,
            operation: HostOperation::UiFocus,
            fields: vec![ProjectedField {
                field: ResultField::NodeId,
                value: ScalarValue::String(node.into()),
            }],
        }
    }
    fn frame() -> ResultBinding {
        ResultBinding {
            name: "pending".into(),
            locals: vec![ScalarBinding {
                name: "prefix".into(),
                value: ScalarValue::String("p-".into()),
            }],
            results: vec![ProjectedBinding {
                name: "saved".into(),
                result: projection("a"),
            }],
            groups: vec![ProjectedGroupBinding {
                name: "group".into(),
                group: ProjectedGroup {
                    members: vec![ProjectedBinding {
                        name: "member".into(),
                        result: projection("b"),
                    }],
                },
            }],
            body: concat(
                local("prefix"),
                concat(
                    field(local("saved")),
                    concat(field(member("group")), field(local("pending"))),
                ),
            ),
        }
    }
    fn group_frame() -> ResultBinding {
        let mut binding = frame();
        binding.results.clear();
        binding.groups.clear();
        binding.body = concat(local("prefix"), field(member("pending")));
        binding
    }
    fn group() -> (Value, Vec<HirBranch>) {
        (
            Value::Structured {
                fields: vec![crate::StructuredField {
                    name: "member".into(),
                    value: Box::new(Value::UiFocus {
                        node_id: "g".into(),
                    }),
                }],
            },
            vec![HirBranch {
                name: "member".into(),
                effect: Effect::UiFocus {
                    node_id: "g".into(),
                },
                result_type: HostOperation::UiFocus.result_type(),
            }],
        )
    }

    // Frozen pre-refactor order is a compatibility oracle, never an execution path.
    fn legacy<'a>(
        binding: &'a ResultBinding,
        pending: ResultView<'a>,
        restore_results: bool,
        fuel: &mut Fuel,
    ) -> Result<Outcome<'a>, Fault> {
        let mut values = vec![];
        let mut scope = ScopeFrame::new(&mut values);
        for local in &binding.locals {
            charge(fuel, 1 + string_cost(&local.value))?;
            scope
                .push(&local.name, LocalValue::Scalar(local.value.clone()))
                .map_err(|_| invalid())?;
        }
        if restore_results {
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
        }
        scope
            .push(&binding.name, LocalValue::Result(pending))
            .map_err(|_| invalid())?;
        evaluate_inner(&binding.body, fuel, &mut scope).map_err(calculation_fault)
    }

    #[test]
    fn shared_atomic_and_group_reentry_match_frozen_results_and_every_original_fuel_boundary() {
        let binding = frame();
        let value = Value::UiFocus {
            node_id: "c".into(),
        };
        let grouped = group_frame();
        let (group_value, branches) = group();
        for grant in 0..=128 {
            let mut old_fuel = Fuel::new(grant);
            let mut new_fuel = Fuel::new(grant);
            let old = observe(legacy(
                &binding,
                ResultView::Raw {
                    value: &value,
                    operation: HostOperation::UiFocus,
                },
                true,
                &mut old_fuel,
            ));
            let new = observe(resume_outcome(
                &binding,
                &value,
                HostOperation::UiFocus,
                &mut new_fuel,
            ));
            assert_eq!(new, old, "atomic grant {grant}");
            assert_eq!(
                new_fuel.remaining(),
                old_fuel.remaining(),
                "atomic grant {grant}"
            );
            let mut old_fuel = Fuel::new(grant);
            let mut new_fuel = Fuel::new(grant);
            let old = observe(legacy(
                &grouped,
                ResultView::Group {
                    value: &group_value,
                    branches: &branches,
                },
                false,
                &mut old_fuel,
            ));
            let new = observe(resume_group_outcome(
                &grouped,
                &group_value,
                &branches,
                &mut new_fuel,
            ));
            assert_eq!(new, old, "group grant {grant}");
            assert_eq!(
                new_fuel.remaining(),
                old_fuel.remaining(),
                "group grant {grant}"
            );
        }
        assert_eq!(
            observe(resume_outcome(
                &binding,
                &value,
                HostOperation::UiFocus,
                &mut Fuel::new(128)
            ))
            .unwrap(),
            Observed::Scalar(ScalarValue::String("p-abc".into()))
        );
    }

    #[test]
    fn shared_duplicate_refusals_preserve_charge_before_insertion_and_original_faults() {
        let value = Value::UiFocus {
            node_id: "c".into(),
        };
        for position in 0..4 {
            let mut binding = frame();
            match position {
                0 => binding.locals.push(binding.locals[0].clone()),
                1 => binding.results[0].name = "prefix".into(),
                2 => binding.groups[0].name = "saved".into(),
                _ => binding.name = "group".into(),
            }
            for grant in 0..=64 {
                let mut old_fuel = Fuel::new(grant);
                let mut new_fuel = Fuel::new(grant);
                assert_eq!(
                    observe(resume_outcome(
                        &binding,
                        &value,
                        HostOperation::UiFocus,
                        &mut new_fuel
                    )),
                    observe(legacy(
                        &binding,
                        ResultView::Raw {
                            value: &value,
                            operation: HostOperation::UiFocus
                        },
                        true,
                        &mut old_fuel
                    ))
                );
                assert_eq!(new_fuel.remaining(), old_fuel.remaining());
            }
        }
    }

    #[test]
    fn body_fault_tail_and_successor_capture_keep_original_meter_and_owned_capture() {
        let value = Value::UiFocus {
            node_id: "c".into(),
        };
        let call = || Computation::Call {
            operation: HostOperation::UiFocus,
            arguments: vec![ComputedArgument {
                name: "node_id".into(),
                value: field(local("saved")),
            }],
        };
        for body in [
            Computation::Unary {
                operator: UnaryOperator::ParseInteger,
                value: Box::new(Computation::Literal {
                    value: ScalarValue::String("native-secret".into()),
                }),
            },
            call(),
            Computation::Bind {
                name: "next".into(),
                value: Box::new(call()),
                body: Box::new(field(local("next"))),
            },
        ] {
            let mut binding = frame();
            binding.body = body;
            for grant in 0..=128 {
                let mut old_fuel = Fuel::new(grant);
                let mut new_fuel = Fuel::new(grant);
                assert_eq!(
                    observe(resume_outcome(
                        &binding,
                        &value,
                        HostOperation::UiFocus,
                        &mut new_fuel
                    )),
                    observe(legacy(
                        &binding,
                        ResultView::Raw {
                            value: &value,
                            operation: HostOperation::UiFocus
                        },
                        true,
                        &mut old_fuel
                    ))
                );
                assert_eq!(new_fuel.remaining(), old_fuel.remaining());
            }
        }
    }

    #[test]
    fn group_reentry_cannot_admit_saved_projection_rows_rejected_by_its_native_contract() {
        let (value, branches) = group();
        for include_group in [false, true] {
            let mut binding = group_frame();
            if include_group {
                binding.groups = frame().groups;
            } else {
                binding.results = frame().results;
            }
            let mut fuel = Fuel::new(50);
            let fault =
                observe(resume_group_outcome(&binding, &value, &branches, &mut fuel)).unwrap_err();
            assert_eq!(fault.code, "LSV1402");
            assert_eq!(fuel.remaining(), 48);
        }
    }

    // Frozen capture preparation/copy order, never a second production path.
    fn legacy_snapshot<'a>(
        name: &str,
        body: &Computation,
        scope: &ScopeFrame<'_, 'a, LocalValue<'a>>,
        fuel: &mut Fuel,
    ) -> Result<Box<ResultBinding>, Fault> {
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
        Ok(Box::new(ResultBinding {
            name: name.to_owned(),
            locals,
            results,
            groups,
            body: body.clone(),
        }))
    }

    #[test]
    fn shared_snapshot_matches_frozen_mixed_scope_bytes_and_every_original_fuel_boundary() {
        let saved = frame();
        let raw = Value::UiFocus {
            node_id: "raw".into(),
        };
        let (raw_group, branches) = group();
        let mut values = vec![
            (
                "raw",
                LocalValue::Result(ResultView::Raw {
                    value: &raw,
                    operation: HostOperation::UiFocus,
                }),
            ),
            (
                "prefix",
                LocalValue::Scalar(ScalarValue::String("p-".into())),
            ),
            (
                "raw_group",
                LocalValue::Result(ResultView::Group {
                    value: &raw_group,
                    branches: &branches,
                }),
            ),
            (
                "saved",
                LocalValue::Result(ResultView::Projected(&saved.results[0].result)),
            ),
            (
                "saved_group",
                LocalValue::Result(ResultView::ProjectedGroup(&saved.groups[0].group)),
            ),
            ("last", LocalValue::Scalar(ScalarValue::Integer(8))),
        ];
        let scope = ScopeFrame::new(&mut values);
        let mut successes = 0;
        for grant in 0..=96 {
            let mut old_fuel = Fuel::new(grant);
            let mut new_fuel = Fuel::new(grant);
            let old = legacy_snapshot("pending", &saved.body, &scope, &mut old_fuel);
            let new = result_environment().capture("pending", &saved.body, &scope, &mut new_fuel);
            assert_eq!(new, old, "grant {grant}");
            assert_eq!(new_fuel.remaining(), old_fuel.remaining(), "grant {grant}");
            if let Ok(frame) = new {
                successes += 1;
                assert_eq!(
                    serde_json::to_vec(&frame).unwrap(),
                    serde_json::to_vec(&old.unwrap()).unwrap()
                );
                assert_eq!(
                    frame
                        .locals
                        .iter()
                        .map(|r| r.name.as_str())
                        .collect::<Vec<_>>(),
                    ["prefix", "last"]
                );
                assert_eq!(
                    frame
                        .results
                        .iter()
                        .map(|r| r.name.as_str())
                        .collect::<Vec<_>>(),
                    ["raw", "saved"]
                );
                assert_eq!(
                    frame
                        .groups
                        .iter()
                        .map(|r| r.name.as_str())
                        .collect::<Vec<_>>(),
                    ["raw_group", "saved_group"]
                );
            }
            assert_eq!(scope.len(), 6);
        }
        assert!(successes > 0);
    }

    #[test]
    fn shared_snapshot_native_projection_refusal_keeps_original_charge_precedence() {
        let saved = frame();
        let invalid_value = Value::Scalar {
            value: ScalarValue::Integer(7),
        };
        for invalid_view in [
            ResultView::Raw {
                value: &invalid_value,
                operation: HostOperation::UiFocus,
            },
            ResultView::Group {
                value: &invalid_value,
                branches: &[],
            },
        ] {
            let mut values = vec![
                (
                    "prefix",
                    LocalValue::Scalar(ScalarValue::String("p-".into())),
                ),
                ("bad", LocalValue::Result(invalid_view)),
                (
                    "never",
                    LocalValue::Result(ResultView::Projected(&saved.results[0].result)),
                ),
            ];
            let scope = ScopeFrame::new(&mut values);
            for grant in 0..=32 {
                let mut old_fuel = Fuel::new(grant);
                let mut new_fuel = Fuel::new(grant);
                assert_eq!(
                    result_environment().capture("pending", &saved.body, &scope, &mut new_fuel),
                    legacy_snapshot("pending", &saved.body, &scope, &mut old_fuel),
                );
                assert_eq!(new_fuel.remaining(), old_fuel.remaining());
            }
        }
    }

    #[test]
    fn snapshot_storage_preflight_rejects_untyped_oversize_before_preparation_without_dispatch() {
        let body = frame().body;
        let oversized = "n".repeat(leselang_hir::pure_typing::MAX_LOCAL_NAME_BYTES + 1);
        let mut rows = vec![
            ("prefix", LocalValue::Scalar(ScalarValue::Integer(1))),
            (
                oversized.as_str(),
                LocalValue::Scalar(ScalarValue::Integer(2)),
            ),
        ];
        let scope = ScopeFrame::new(&mut rows);
        let mut fuel = Fuel::new(100);
        let error = result_environment()
            .capture("pending", &body, &scope, &mut fuel)
            .unwrap_err();
        assert_eq!(error, invalid());
        assert_eq!(fuel.remaining(), 100);
        let mut rows = (0..=MAX_TYPE_INFERENCE_BINDINGS)
            .map(|_| ("n", LocalValue::Scalar(ScalarValue::None)))
            .collect();
        let scope = ScopeFrame::new(&mut rows);
        let error = result_environment()
            .capture("pending", &body, &scope, &mut fuel)
            .unwrap_err();
        assert_eq!(error, invalid());
        assert_eq!(fuel.remaining(), 100);
    }
}
