//! Product request preparation on the shared HIR executor, without dispatch.

use std::{borrow::Cow, fmt};

use leselang_hir_core::{
    call_evaluation::{CallEvaluationError, CallEvaluationLimits},
    effect_evaluation::{
        EffectEvaluationEnvironment, EffectEvaluationFailure, EffectEvaluationLimits,
        EffectEvaluationOutcome, evaluate_effects_in_scope,
    },
    pure_evaluation::PureValue,
};
use leselang_runtime_core::{Fuel, ScopeFrame};
use leserpent_hir::{
    Effect, HirBranch, Type,
    computation::{Computation, GroupKind},
    host_call::HostOperation,
    result_field::ResultField,
};

/// Preparation failures retain typed calculation/native provenance for the host.
/// No native payload is formatted, cloned, serialized or made recoverable here.
pub enum EffectPreparationError<Native> {
    InvalidComputation,
    Evaluation(EffectEvaluationFailure<Native>),
    Call {
        operation: HostOperation,
        failure: CallEvaluationError<Native>,
    },
    Resolution {
        operation: HostOperation,
        diagnostic_code: String,
    },
}

impl<Native> fmt::Display for EffectPreparationError<Native> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidComputation => "request preparation contract rejected",
            Self::Evaluation(_) => "group member preparation failed",
            Self::Call { .. } => "computed call preparation failed",
            Self::Resolution { .. } => "computed call resolution rejected",
        })
    }
}

impl<Native> fmt::Debug for EffectPreparationError<Native> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}

impl<Native> std::error::Error for EffectPreparationError<Native> {}

/// Prepare an already admitted effect site, normally from the environment hook.
///
/// The caller must first run whole-expression structure/type/opaque-effect and
/// authority gates, including cold branches. This is not a standalone ingress:
/// raw Host payloads are borrowed unchanged, and group counts/names are assumed
/// preflighted. `limits` bounds computed calls and shared member evaluation; it
/// does not retroactively validate opaque payloads or the enclosing group.
///
/// The outer shared executor owns the root charge. Members reuse the original
/// scope, environment and fuel, in declaration order even for `all`; no effect
/// is dispatched in this stage. Native hooks must not dispatch either. A later
/// failure does not refund fuel or roll back native hook work. The host maps
/// errors without erasing scalar/native provenance, and rechecks live authority
/// at dispatch. Returned request data is not a receipt or execution grant.
pub fn prepare_preflighted_effect_in_scope<'expression, Environment>(
    expression: &'expression Computation,
    scope: &mut ScopeFrame<'_, 'expression, PureValue<Environment::Result>>,
    environment: &Environment,
    fuel: &mut Fuel,
    limits: EffectEvaluationLimits,
) -> Result<Cow<'expression, Effect>, EffectPreparationError<Environment::Error>>
where
    Environment: EffectEvaluationEnvironment<
            'expression,
            ResultField,
            HostOperation,
            Effect,
            Type,
            Request = Cow<'expression, Effect>,
        >,
{
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
                    .ok_or(EffectPreparationError::InvalidComputation)?;
                let outcome =
                    evaluate_effects_in_scope(&branch.value, scope, environment, fuel, limits)
                        .map_err(EffectPreparationError::Evaluation)?;
                let EffectEvaluationOutcome::Request(effect) = outcome else {
                    return Err(EffectPreparationError::InvalidComputation);
                };
                if HostOperation::for_effect(&effect) != Some(operation)
                    || branch.result_type != operation.result_type()
                {
                    return Err(EffectPreparationError::InvalidComputation);
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
                    environment,
                    fuel,
                    CallEvaluationLimits {
                        pure: limits.pure,
                        max_arguments: limits.max_arguments,
                    },
                )
                .map_err(|failure| EffectPreparationError::Call {
                    operation: *operation,
                    failure,
                })?;
            let effect = operation.resolve(&values).map_err(|diagnostics| {
                EffectPreparationError::Resolution {
                    operation: *operation,
                    diagnostic_code: diagnostics
                        .first()
                        .map_or("LSH1407", |diagnostic| diagnostic.code.as_str())
                        .to_owned(),
                }
            })?;
            Ok(Cow::Owned(effect))
        }
        _ => Err(EffectPreparationError::InvalidComputation),
    }
}
