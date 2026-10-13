//! One product execution environment, with explicit borrowed host policy.

use std::{borrow::Cow, fmt, marker::PhantomData};

use leselang_hir::{
    Effect, Type,
    computation::Computation,
    effect_evaluation::{EffectEvaluationEnvironment, EffectEvaluationLimits},
    host_call::HostOperation,
    pure_evaluation::{PureEvaluationEnvironment, PureValue},
    result_field::ResultField,
};
use leselang_runtime_core::{
    CalculationFailure, CaptureSnapshotError, Fuel, ScalarValue, ScopeFrame,
};

use crate::{
    EffectPreparationError, ResultBinding, ResultCaptureHost, ResultCaptureLimits, ResultView,
    prepare_preflighted_effect_in_scope, snapshot_preflighted_result_scope,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResultExecutionLimits {
    pub effects: EffectEvaluationLimits,
    pub captures: ResultCaptureLimits,
}

/// Native policy for an already admitted reference-profile execution.
///
/// Every hook is explicit: no default grant, cost, name policy or fault mapping.
/// Cold preflight must check opaque payloads/signatures and required authority;
/// selected preparation can recheck live policy. Projection policy runs only on
/// selected field reads. `prepared_effect` observes/refuses prepared data, not a
/// dispatched effect or receipt. No hook may dispatch, refill fuel or retry work.
///
/// Native errors must remain native when mapping failures; recoverable scalar
/// failures must retain their typed provenance. Hooks own their work bounds and
/// may mutate or unwind; prior work is never rolled back. The environment borrows
/// this host, imposing no Clone/Debug/serde/Send/Sync bounds on it or its errors.
pub trait ResultExecutionHost: ResultCaptureHost {
    fn preflight_effect(&self, expression: &Computation) -> Result<(), Self::Error>;
    fn before_prepare_effect(&self, expression: &Computation) -> Result<(), Self::Error>;
    fn prepared_effect(&self, effect: &Effect) -> Result<(), Self::Error>;
    fn before_projection(&self, field: ResultField) -> Result<(), Self::Error>;
    fn valid_group_name(&self, name: &str) -> bool;
    fn preparation_failure(
        &self,
        failure: EffectPreparationError<Self::Error>,
    ) -> CalculationFailure<Self::Error>;
    fn capture_failure(&self, failure: CaptureSnapshotError<Self::Error>) -> Self::Error;
}

/// Shared field/member access, request preparation and snapshot construction.
///
/// Construction is not admission. The caller must validate complete source/HIR,
/// restored frame/reply provenance and current authority before execution, use
/// `limits().effects` for the shared outer evaluator and supply the original
/// meter. This environment provides no driver, durable storage or dispatch gate.
/// It never selects a body or implements a second evaluator. Recursive computed
/// groups use this same environment/host, not a detached default policy.
pub struct PreflightedResultEnvironment<'value, 'host, Host: ?Sized> {
    host: &'host Host,
    limits: ResultExecutionLimits,
    value: PhantomData<ResultView<'value>>,
}

impl<'value, 'host, Host: ResultExecutionHost + ?Sized>
    PreflightedResultEnvironment<'value, 'host, Host>
{
    pub fn new(host: &'host Host, limits: ResultExecutionLimits) -> Self {
        Self {
            host,
            limits,
            value: PhantomData,
        }
    }

    pub fn limits(&self) -> ResultExecutionLimits {
        self.limits
    }
}

impl<Host: ?Sized> fmt::Debug for PreflightedResultEnvironment<'_, '_, Host> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PreflightedResultEnvironment")
    }
}

impl<'value, Host: ResultExecutionHost + ?Sized>
    PureEvaluationEnvironment<ResultField, HostOperation>
    for PreflightedResultEnvironment<'value, '_, Host>
{
    type Result = ResultView<'value>;
    type Error = Host::Error;

    fn field(
        &self,
        result: &Self::Result,
        field: &ResultField,
    ) -> Result<ScalarValue, Self::Error> {
        self.host.before_projection(*field)?;
        result
            .project_scalar(*field)
            .map_err(|error| self.host.projection_error(error))
    }

    fn member(
        &self,
        group: &Self::Result,
        name: &str,
        operation: &HostOperation,
    ) -> Result<Self::Result, Self::Error> {
        group
            .select_member(name, *operation)
            .map_err(|error| self.host.projection_error(error))
    }
}

impl<'value, Host: ResultExecutionHost + ?Sized>
    EffectEvaluationEnvironment<'value, ResultField, HostOperation, Effect, Type>
    for PreflightedResultEnvironment<'value, '_, Host>
{
    type Request = Cow<'value, Effect>;
    type Capture = Box<ResultBinding>;

    fn preflight_effect(&self, expression: &'value Computation) -> Result<(), Self::Error> {
        self.host.preflight_effect(expression)
    }

    fn prepare_effect(
        &self,
        expression: &'value Computation,
        scope: &mut ScopeFrame<'_, 'value, PureValue<Self::Result>>,
        fuel: &mut Fuel,
    ) -> Result<Self::Request, CalculationFailure<Self::Error>> {
        self.host.before_prepare_effect(expression)?;
        let effect =
            prepare_preflighted_effect_in_scope(expression, scope, self, fuel, self.limits.effects)
                .map_err(|failure| self.host.preparation_failure(failure))?;
        self.host.prepared_effect(&effect)?;
        Ok(effect)
    }

    fn capture(
        &self,
        name: &'value str,
        body: &'value Computation,
        scope: &ScopeFrame<'_, 'value, PureValue<Self::Result>>,
        fuel: &mut Fuel,
    ) -> Result<Self::Capture, Self::Error> {
        snapshot_preflighted_result_scope(
            name,
            body,
            scope,
            fuel,
            self.limits.captures,
            self.host,
            |name| self.host.valid_group_name(name),
        )
        .map(Box::new)
        .map_err(|failure| self.host.capture_failure(failure))
    }
}
