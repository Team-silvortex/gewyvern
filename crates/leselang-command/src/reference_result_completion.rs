//! Product result-completion coordination, without journal or lease authority.

use crate::continuation_schema::{
    CONDITIONAL_CONTINUATION_SCHEMA_VERSION, DATAFLOW_CONTINUATION_SCHEMA_VERSION,
    GROUP_CAPTURE_CONTINUATION_SCHEMA_VERSION, GROUP_CONDITIONAL_CONTINUATION_SCHEMA_VERSION,
    GROUP_DATAFLOW_CONTINUATION_SCHEMA_VERSION, PARALLEL_GROUP_CONTINUATION_SCHEMA_VERSION,
    SUCCESSOR_CONTINUATION_SCHEMA_VERSION,
};
use crate::execution_outcome::ComputationOutcome;
use crate::reference_computation as computation;
use crate::reference_reply::CompletionLease;
use crate::{ContinuationImage, EffectOperation, EffectResult, ResultBinding, Step, Value};
use leselang_runtime_core::{Fault, Fuel, ScalarValue};
use leserpent_hir::Effect;

/// Trusted completion owner, entered only after bounded image/body and reply
/// admission. This is not an untrusted-input entry point. Conversion keeps
/// the original result-domain checks; validation checks raw output before any body
/// evaluation. Settlement owns authority lookup, sequence reservation, lease fences,
/// atomic journal writes and publication of the authoritative winner. Hooks run
/// once, without retry or rollback of earlier native side effects. Neither an image
/// nor the borrowed lease is an authentication certificate supplied by this layer.
pub trait ReferenceCompletionHost {
    fn atomic_step_from_effect_result(
        &mut self,
        image: &ContinuationImage,
        operation: Option<&EffectOperation>,
        result: EffectResult,
    ) -> Step;
    fn step_from_effect_result(
        &mut self,
        image: &ContinuationImage,
        operation: Option<&EffectOperation>,
        result: EffectResult,
    ) -> Step;
    fn validate_result_output(
        &mut self,
        image: &ContinuationImage,
        value: &Value,
    ) -> Result<(), Fault>;
    fn complete_successor(
        &mut self,
        image: &ContinuationImage,
        lease: CompletionLease<'_>,
        value: Value,
        effect: Effect,
        binding: Option<Box<ResultBinding>>,
        fuel: u64,
    ) -> Step;
    fn record_result(
        &mut self,
        image: &ContinuationImage,
        lease: CompletionLease<'_>,
        step: Step,
    ) -> Step;
}

/// Preserve schema-specific raw/bound routing and evaluation with saved fuel.
/// Group replies remain raw for the native group coordinator. A successor commits
/// the original raw value, not its projected/scalar continuation result.
pub fn complete_result<Host: ReferenceCompletionHost + ?Sized>(
    image: &ContinuationImage,
    lease: CompletionLease<'_>,
    result: EffectResult,
    host: &mut Host,
) -> Step {
    let operation = lease.map(|(lease, _)| &lease.request.operation);
    let step = if matches!(
        image.schema_version,
        SUCCESSOR_CONTINUATION_SCHEMA_VERSION
            | DATAFLOW_CONTINUATION_SCHEMA_VERSION
            | CONDITIONAL_CONTINUATION_SCHEMA_VERSION
    ) {
        let raw = host.atomic_step_from_effect_result(image, operation, result);
        if let Step::Done(value) = raw {
            match host
                .validate_result_output(image, &value)
                .and_then(|()| materialize(image, &value))
            {
                Ok(PreparedResult::Host {
                    effect,
                    binding,
                    fuel,
                }) => {
                    return host.complete_successor(image, lease, value, effect, binding, fuel);
                }
                Ok(PreparedResult::Scalar(value)) => Step::Done(Value::Scalar { value }),
                Err(error) => Step::Fault(error),
            }
        } else {
            raw
        }
    } else if matches!(
        image.schema_version,
        GROUP_CAPTURE_CONTINUATION_SCHEMA_VERSION
            | PARALLEL_GROUP_CONTINUATION_SCHEMA_VERSION
            | GROUP_DATAFLOW_CONTINUATION_SCHEMA_VERSION
            | GROUP_CONDITIONAL_CONTINUATION_SCHEMA_VERSION
    ) {
        host.atomic_step_from_effect_result(image, operation, result)
    } else {
        host.step_from_effect_result(image, operation, result)
    };
    host.record_result(image, lease, step)
}

enum PreparedResult {
    Scalar(ScalarValue),
    Host {
        effect: Effect,
        binding: Option<Box<ResultBinding>>,
        fuel: u64,
    },
}

fn materialize(image: &ContinuationImage, value: &Value) -> Result<PreparedResult, Fault> {
    let binding = image.result_binding.as_ref().ok_or_else(invalid)?;
    let mut fuel = Fuel::new(image.fuel_remaining);
    let operation = leserpent_hir::host_call::HostOperation::for_effect(&image.pending_effect)
        .ok_or_else(invalid)?;
    let (effect, binding) = match computation::resume_outcome(binding, value, operation, &mut fuel)?
    {
        ComputationOutcome::Scalar(value) => return Ok(PreparedResult::Scalar(value)),
        ComputationOutcome::Host(effect) => (effect, None),
        ComputationOutcome::BoundHost { effect, binding } => (effect, Some(binding)),
        ComputationOutcome::Result => return Err(invalid()),
    };
    if fuel.remaining() == 0 {
        return Err(Fault {
            code: "LSV1001".into(),
            message: "execution fuel exhausted before successor".into(),
        });
    }
    if leserpent_hir::host_call::HostOperation::for_effect(&effect).is_none() {
        return Err(invalid());
    }
    Ok(PreparedResult::Host {
        effect: effect.into_owned(),
        binding,
        fuel: fuel.remaining(),
    })
}

fn invalid() -> Fault {
    Fault {
        code: "LSV1407".into(),
        message: "invalid result-driven successor or missing original effect authority".into(),
    }
}
