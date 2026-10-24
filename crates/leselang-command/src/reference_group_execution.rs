//! Product group re-entry coordination over the original evaluator and proofs.

use leselang_hir::{Effect, HirBranch, host_call::HostOperation};
use leselang_runtime_core::{Fault, Fuel, ScalarValue};

use crate::execution_outcome::ComputationOutcome as Outcome;
use crate::reference_group_binding::validate_saved_capture;
use crate::reference_group_successor::GroupSuccessorContext;
use crate::reference_merge::invalid_group_binding as invalid;
use crate::{ContinuationToken, EffectRequest, GroupResultBinding, ResultBinding, Step, Value};

/// Prepared output only. The original journal still admits and settles the group.
pub enum CaptureProgress {
    Done(ScalarValue),
    Pending(Box<EffectRequest>),
}

/// Trusted execution/proof boundaries on the same original owner. Inputs must be
/// bounded/admitted and backed by authentic receipts and complete group state.
/// Hooks use the supplied remaining meter; never replenish it, dispatch, settle,
/// retry or publish partial work. Mutation and unwind remain native side effects,
/// not rolled back by this coordinator. No Clone/Send/static requirement is added.
pub trait ReferenceGroupExecutionHost {
    fn resume_group<'a>(
        &mut self,
        binding: &'a ResultBinding,
        value: &'a Value,
        branches: &'a [HirBranch],
        fuel: &mut Fuel,
    ) -> Result<Outcome<'a>, Fault>;
    fn resume_capture<'a>(
        &mut self,
        binding: &'a ResultBinding,
        value: &'a Value,
        operation: HostOperation,
        fuel: &mut Fuel,
    ) -> Result<Outcome<'a>, Fault>;
    fn finish_group(
        &mut self,
        binding: &ResultBinding,
        value: &Value,
        branches: &[HirBranch],
        fuel: &mut Fuel,
    ) -> Result<ScalarValue, Fault>;
    fn build_successor(
        &mut self,
        group: &GroupResultBinding,
        effect: &Effect,
        binding: Option<Box<ResultBinding>>,
        context: GroupSuccessorContext<'_>,
    ) -> Result<EffectRequest, Fault>;
    fn validate_successor(
        &mut self,
        parent: &EffectRequest,
        result: &Step,
        next: &EffectRequest,
    ) -> Result<(), Fault>;
}

/// Preserve initial outcome classification before the zero-fuel successor gate.
pub fn prepare_initial<Host: ReferenceGroupExecutionHost + ?Sized>(
    group: &GroupResultBinding,
    owner: &ContinuationToken,
    first: &EffectRequest,
    value: &Value,
    host: &mut Host,
) -> Result<CaptureProgress, Fault> {
    let mut fuel = Fuel::new(group.fuel_remaining);
    let (effect, binding) = match host.resume_group(
        &group.binding,
        value,
        group.plan_branches().map_err(|_| invalid())?,
        &mut fuel,
    )? {
        Outcome::Scalar(value) if group.is_conditional() => {
            return Ok(CaptureProgress::Done(value));
        }
        Outcome::Host(effect) if !group.captures_successor() => (effect, None),
        Outcome::BoundHost { effect, binding } if group.captures_successor() => {
            (effect, Some(binding))
        }
        _ => return Err(invalid()),
    };
    require_successor_fuel(&fuel)?;
    host.build_successor(
        group,
        &effect,
        binding,
        GroupSuccessorContext {
            owner,
            first,
            fuel_remaining: fuel.remaining(),
            position: 0,
        },
    )
    .map(|request| CaptureProgress::Pending(Box::new(request)))
}

/// Validate the ordered saved prefix before native evaluation, then validate the
/// original parent/result/next relationship before handing back prepared data.
pub fn advance_capture<Host: ReferenceGroupExecutionHost + ?Sized>(
    group: &GroupResultBinding,
    owner: &ContinuationToken,
    requests: &[EffectRequest],
    value: &Value,
    host: &mut Host,
) -> Result<CaptureProgress, Fault> {
    let Value::Structured { fields } = value else {
        return Err(invalid());
    };
    let (tail, prefix) = fields.split_last().ok_or_else(invalid)?;
    let request = requests.last().ok_or_else(invalid)?;
    let operations = requests
        .iter()
        .take(prefix.len())
        .map(|request| {
            HostOperation::for_effect(&request.continuation.pending_effect).ok_or_else(invalid)
        })
        .collect::<Result<Vec<_>, _>>()?;
    validate_saved_capture(group, request, prefix, &operations)?;
    let image = &request.continuation;
    let saved = image.result_binding.as_ref().ok_or_else(invalid)?;
    let operation = HostOperation::for_effect(&image.pending_effect).ok_or_else(invalid)?;
    let mut fuel = Fuel::new(image.fuel_remaining);
    match host.resume_capture(saved, &tail.value, operation, &mut fuel)? {
        Outcome::Scalar(value) => Ok(CaptureProgress::Done(value)),
        Outcome::BoundHost { effect, binding } if group.is_dataflow() || group.is_conditional() => {
            require_successor_fuel(&fuel)?;
            let next = host.build_successor(
                group,
                &effect,
                Some(binding),
                GroupSuccessorContext {
                    owner,
                    first: requests.first().ok_or_else(invalid)?,
                    fuel_remaining: fuel.remaining(),
                    position: requests.len() - group.plan_branches().map_err(|_| invalid())?.len(),
                },
            )?;
            host.validate_successor(request, &Step::Done((*tail.value).clone()), &next)?;
            Ok(CaptureProgress::Pending(Box::new(next)))
        }
        _ => Err(invalid()),
    }
}

pub fn finish<Host: ReferenceGroupExecutionHost + ?Sized>(
    group: &GroupResultBinding,
    value: &Value,
    host: &mut Host,
) -> Step {
    let mut fuel = Fuel::new(group.fuel_remaining);
    match group
        .plan_branches()
        .map_err(|_| invalid())
        .and_then(|branches| host.finish_group(&group.binding, value, branches, &mut fuel))
    {
        Ok(value) => Step::Done(Value::Scalar { value }),
        Err(error) => Step::Fault(error),
    }
}

fn require_successor_fuel(fuel: &Fuel) -> Result<(), Fault> {
    if fuel.remaining() == 0 {
        Err(Fault {
            code: "LSV1001".into(),
            message: "execution fuel exhausted before group successor".into(),
        })
    } else {
        Ok(())
    }
}
