//! Product capture materialization and re-entry over the shared scope kernels.

use leselang_hir::{
    Effect, Type,
    computation::Computation,
    effect_evaluation::{EffectEvaluationEnvironment, EffectEvaluationLimits},
    effect_reentry::{
        CapturedEffectControlResult, CapturedEffectHooks, resume_captured_effects_in_scope,
    },
    host_call::HostOperation,
    pure_evaluation::PureValue,
    result_field::ResultField,
};
use leselang_runtime_core::{
    CaptureSnapshotError, CaptureSnapshotHooks, CaptureSnapshotLimits, CapturedValue, Fuel,
    GroupCaptureLimits, ScalarValue, ScopeFrame, snapshot_captured_scope_state,
};

use crate::{ProjectedGroup, ProjectedResult, ResultBinding, ResultView, ResultViewError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResultCaptureLimits {
    pub snapshot: CaptureSnapshotLimits,
    pub groups: GroupCaptureLimits,
}

/// Trusted accounting and reference-profile refusal policy, never dispatch.
///
/// There are no default costs or fault codes. Hooks receive the original meter;
/// successful charges and native mutations survive later refusal or unwind.
/// Hooks must bound their own work and must not dispatch, refill fuel or infer
/// authority from saved data. Native errors move without formatting or cloning.
pub trait ResultCaptureHost {
    type Error;

    fn charge_scalar(&self, value: &ScalarValue, fuel: &mut Fuel) -> Result<(), Self::Error>;
    fn charge_result(&self, value: &ProjectedResult, fuel: &mut Fuel) -> Result<(), Self::Error>;
    fn charge_group(&self, value: &ProjectedGroup, fuel: &mut Fuel) -> Result<(), Self::Error>;
    fn projection_error(&self, error: ResultViewError) -> Self::Error;

    /// Called before charging each saved result/group, after earlier scalar rows.
    /// This is a row-level profile restriction, not complete frame admission.
    fn allow_saved_projection(&self) -> Result<(), Self::Error>;
}

/// Snapshot an admitted product scope without changing its original rows.
///
/// The shared kernel checks aggregate count/name byte limits before any hook.
/// Scalars charge before cloning; raw/saved projections materialize before their
/// host charge. Raw groups alone apply supplied member/name policy; saved groups
/// clone unchanged. Original row order within each category and the final body
/// copy are preserved. No partial snapshot is returned on failure. The name
/// predicate is released while the result/error is still locally owned.
///
/// This is not an ingress validator: the caller must bound the body and payloads
/// and admit names, types, schemas and raw reply identity before entry. Existing
/// saved fields are not widened or revalidated here. Snapshot success creates
/// mutable data, not an authenticated continuation or durable replay authority.
pub fn snapshot_preflighted_result_scope<'value, Host: ResultCaptureHost + ?Sized>(
    name: &str,
    body: &Computation,
    scope: &ScopeFrame<'_, '_, PureValue<ResultView<'value>>>,
    fuel: &mut Fuel,
    limits: ResultCaptureLimits,
    host: &Host,
    mut validate_group_name: impl FnMut(&str) -> bool,
) -> Result<ResultBinding, CaptureSnapshotError<Host::Error>> {
    let outcome = snapshot_captured_scope_state(
        name,
        body,
        scope,
        fuel,
        limits.snapshot,
        CaptureSnapshotHooks {
            row: |_: &str, value: &PureValue<ResultView<'value>>, fuel: &mut Fuel| match value {
                PureValue::Scalar(value) => {
                    host.charge_scalar(value, fuel)?;
                    Ok(CapturedValue::Scalar(value.clone()))
                }
                PureValue::Result(
                    view @ (ResultView::Group { .. } | ResultView::ProjectedGroup(_)),
                ) => {
                    let group = view
                        .capture_group(limits.groups, &mut validate_group_name)
                        .map_err(|error| host.projection_error(error))?;
                    host.charge_group(&group, fuel)?;
                    Ok(CapturedValue::Group(group))
                }
                PureValue::Result(view) => {
                    let result = view
                        .capture_result()
                        .map_err(|error| host.projection_error(error))?;
                    host.charge_result(&result, fuel)?;
                    Ok(CapturedValue::Result(result))
                }
            },
            body: |body: &Computation, _: &mut Fuel| Ok(body.clone()),
        },
    );
    drop(validate_group_name);
    outcome
}

/// Restore an admitted product binding and run its body in the shared evaluator.
///
/// Scalars charge before cloning; saved results/groups charge before borrowing
/// their exact original payloads. The pending view is inserted without a new
/// copy or charge. Shared restoration owns insertion/duplicate order and removes
/// the temporary suffix on return, refusal or unwind, retaining the parent scope.
/// Native failures remain external to scalar recovery. Limits and meter are
/// passed through unchanged; no fresh grant or default policy is introduced.
///
/// Complete frame/body/aggregate bounds, original reply correspondence, current
/// authority and journal admission must precede this entry. It is not a codec,
/// receipt check, cancellation/dispatch driver, retry fence or durable restore.
pub fn resume_preflighted_result_binding_in_scope<'expression, Environment, Host>(
    binding: &'expression ResultBinding,
    parent: &mut ScopeFrame<'_, 'expression, PureValue<ResultView<'expression>>>,
    environment: &Environment,
    fuel: &mut Fuel,
    limits: EffectEvaluationLimits,
    pending: ResultView<'expression>,
    host: &Host,
) -> CapturedEffectControlResult<'expression, ResultField, HostOperation, Effect, Type, Environment>
where
    Host: ResultCaptureHost + ?Sized,
    Environment: EffectEvaluationEnvironment<
            'expression,
            ResultField,
            HostOperation,
            Effect,
            Type,
            Result = ResultView<'expression>,
            Error = Host::Error,
        >,
{
    resume_captured_effects_in_scope(
        binding,
        parent,
        environment,
        fuel,
        limits,
        CapturedEffectHooks {
            scalar: |value: &ScalarValue, fuel: &mut Fuel| {
                host.charge_scalar(value, fuel)?;
                Ok(PureValue::Scalar(value.clone()))
            },
            result: |result: &'expression ProjectedResult, fuel: &mut Fuel| {
                host.allow_saved_projection()?;
                host.charge_result(result, fuel)?;
                Ok(PureValue::Result(ResultView::Projected(result)))
            },
            group: |group: &'expression ProjectedGroup, fuel: &mut Fuel| {
                host.allow_saved_projection()?;
                host.charge_group(group, fuel)?;
                Ok(PureValue::Result(ResultView::ProjectedGroup(group)))
            },
            pending: |_: &mut Fuel| Ok(PureValue::Result(pending)),
        },
    )
}
