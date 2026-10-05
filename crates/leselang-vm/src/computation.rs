use std::borrow::Cow;

use leselang_hir::call_evaluation::{CallEvaluationError, CallEvaluationLimits};
use leselang_hir::call_typing::{CallTypeError, MAX_CALL_ARGUMENTS};
use leselang_hir::computation::{Computation, GroupKind, ScalarValue};
use leselang_hir::effect_evaluation::{
    EffectEvaluationEnvironment, EffectEvaluationFault, EffectEvaluationLimits,
    EffectEvaluationOutcome, evaluate_effects_in_scope,
};
use leselang_hir::host_call::HostOperation;
use leselang_hir::pure_evaluation::{
    PureEvaluationEnvironment, PureEvaluationFailure, PureEvaluationFault, PureEvaluationLimits,
    PureValue, scalar_copy_cost as string_cost,
};
use leselang_hir::pure_typing::{
    MAX_TYPE_INFERENCE_BINDINGS, MAX_TYPE_INFERENCE_DEPTH, MAX_TYPE_INFERENCE_NODES,
};
use leselang_hir::{Effect, HirBranch};
use leselang_runtime_core::{
    CalculationFailure, FoldError, Fuel, LoopError, ScalarError, ScopeFrame,
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

struct ResultEnvironment<'a>(std::marker::PhantomData<&'a Value>);

impl<'a> PureEvaluationEnvironment<leselang_hir::result_field::ResultField, HostOperation>
    for ResultEnvironment<'a>
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
        match group {
            ResultView::Group {
                value: Value::Structured { fields },
                branches,
            } => {
                if !branches.iter().any(|branch| {
                    branch.name == name
                        && HostOperation::for_effect(&branch.effect) == Some(*operation)
                }) {
                    return Err(invalid());
                }
                let field = fields
                    .iter()
                    .find(|field| field.name == name)
                    .ok_or_else(invalid)?;
                Ok(ResultView::Raw {
                    value: &field.value,
                    operation: *operation,
                })
            }
            ResultView::ProjectedGroup(saved) => {
                let member = saved
                    .members
                    .iter()
                    .find(|member| member.name == name && member.result.operation == *operation)
                    .ok_or_else(invalid)?;
                Ok(ResultView::Projected(&member.result))
            }
            _ => Err(invalid()),
        }
    }
}

pub(super) enum Outcome<'a> {
    Scalar(ScalarValue),
    Result,
    Host(Cow<'a, Effect>),
    BoundHost {
        effect: Cow<'a, Effect>,
        binding: Box<ResultBinding>,
    },
}

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

impl<'a>
    EffectEvaluationEnvironment<
        'a,
        leselang_hir::result_field::ResultField,
        HostOperation,
        Effect,
        leselang_hir::Type,
    > for ResultEnvironment<'a>
{
    type Request = Cow<'a, Effect>;
    type Capture = Box<ResultBinding>;

    fn preflight_effect(&self, _: &'a Computation) -> Result<(), Fault> {
        // The reference VM enters only after canonical HIR/type/authority gates.
        // Opaque effect graphs and cold signatures retain those product validators.
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
                        CallEvaluationLimits {
                            pure: PureEvaluationLimits {
                                max_nodes: MAX_TYPE_INFERENCE_NODES,
                                max_depth: MAX_TYPE_INFERENCE_DEPTH,
                                max_bindings: MAX_TYPE_INFERENCE_BINDINGS,
                            },
                            max_arguments: MAX_CALL_ARGUMENTS,
                        },
                    )
                    .map_err(|failure| call_failure(failure, *operation))?;
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
}

fn evaluate_inner<'a>(
    expression: &'a Computation,
    fuel: &mut Fuel,
    scope: &mut ScopeFrame<'_, 'a, LocalValue<'a>>,
) -> Result<Outcome<'a>, EvaluationFailure> {
    evaluate_effects_in_scope(
        expression,
        scope,
        &ResultEnvironment(std::marker::PhantomData),
        fuel,
        EffectEvaluationLimits {
            pure: PureEvaluationLimits {
                max_nodes: MAX_TYPE_INFERENCE_NODES,
                max_depth: MAX_TYPE_INFERENCE_DEPTH,
                max_bindings: MAX_TYPE_INFERENCE_BINDINGS,
            },
            max_arguments: MAX_CALL_ARGUMENTS,
            max_branches: leselang_hir::MAX_ALL_BRANCHES,
        },
    )
    .map(|outcome| match outcome {
        EffectEvaluationOutcome::Value(PureValue::Scalar(value)) => Outcome::Scalar(value),
        EffectEvaluationOutcome::Value(PureValue::Result(_)) => Outcome::Result,
        EffectEvaluationOutcome::Request(effect) => Outcome::Host(effect),
        EffectEvaluationOutcome::Suspended { request, capture } => Outcome::BoundHost {
            effect: request,
            binding: capture,
        },
    })
    .map_err(|failure| match failure {
        CalculationFailure::Scalar(error) => CalculationFailure::Scalar(error),
        CalculationFailure::External(EffectEvaluationFault::Pure(failure)) => {
            pure_failure(CalculationFailure::External(failure))
        }
        CalculationFailure::External(EffectEvaluationFault::Native(fault)) => {
            CalculationFailure::External(fault)
        }
        CalculationFailure::External(_) => invalid().into(),
    })
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
