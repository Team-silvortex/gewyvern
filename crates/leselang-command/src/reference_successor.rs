//! Product successor assembly; the live owner retains reservation and commit authority.

use crate::continuation_schema::preflighted_result_binding_schema_version;
use crate::reference_continuation::validate_continuation_size;
use crate::reference_reply::CompletionLease;
use crate::reference_request::validate_effect_request;
pub use crate::reference_request_build::ReservedEffectInput;
use crate::{ContinuationImage, ContinuationToken, EffectRequest, ResultBinding, Step, Value};
use leselang_hir::Effect;
use leselang_runtime_core::Fault;

/// Trusted synchronous owner, entered with bounded/admitted original execution
/// data and an evaluated successor. Request lookup returns original saved authority;
/// it must not trust a replacement supplied by the caller. Reservations and the
/// builder retain their existing charge and allocation effects, without refund.
/// Commit rechecks the complete pair and lease transactionally before publication,
/// then returns the authoritative winner. Static checks here cannot certify a
/// receipt, lease, reservation or current capability. No hook is retried, and
/// errors/unwinds do not roll back earlier native side effects.
pub trait ReferenceSuccessorHost {
    fn effect_request(&mut self, token: &ContinuationToken) -> Result<EffectRequest, Fault>;
    fn allocate_sequence(&mut self) -> Result<u64, Fault>;
    fn build_effect_request(
        &mut self,
        input: ReservedEffectInput<'_>,
    ) -> Result<EffectRequest, Fault>;
    fn record_result(
        &mut self,
        image: &ContinuationImage,
        lease: CompletionLease<'_>,
        step: Step,
    ) -> Step;
    fn commit_successor(
        &mut self,
        image: &ContinuationImage,
        lease: CompletionLease<'_>,
        parent: &EffectRequest,
        step: &Step,
        next: &EffectRequest,
    ) -> Step;
}

/// Observe original authority, reserve once, then assemble and validate the next
/// request. Missing/mismatched parents and reservation errors return directly;
/// operation/build/admission failures instead enter native terminal settlement.
/// The caller keeps its raw result and lease unchanged through the commit hook.
pub fn complete_successor<Host: ReferenceSuccessorHost + ?Sized>(
    image: &ContinuationImage,
    lease: CompletionLease<'_>,
    value: Value,
    effect: Effect,
    binding: Option<Box<ResultBinding>>,
    fuel: u64,
    host: &mut Host,
) -> Step {
    let parent = match host.effect_request(&image.token) {
        Ok(request) if request.continuation == *image => request,
        Ok(_) => return Step::Fault(invalid()),
        Err(error) => return Step::Fault(error),
    };
    let sequence = match host.allocate_sequence() {
        Ok(sequence) => sequence,
        Err(error) => return Step::Fault(error),
    };
    let (principal, capabilities) = parent.operation.authority();
    let Some(operation) = leselang_hir::host_call::HostOperation::for_effect(&effect) else {
        return host.record_result(image, lease, Step::Fault(invalid()));
    };
    let mut next = match host.build_effect_request(ReservedEffectInput {
        effect: &effect,
        result_type: operation.result_type(),
        sequence,
        principal: principal.clone(),
        capabilities: capabilities.clone(),
        expected_revision: image.expected_revision,
        deadline_ms: image.deadline_ms,
        deadline_at_ms: image.deadline_at_ms,
        fuel_remaining: fuel,
    }) {
        Ok(request) => request,
        Err(error) => return host.record_result(image, lease, Step::Fault(error)),
    };
    next.continuation.max_output_items = image.max_output_items;
    next.budget.max_output_items = image.max_output_items;
    if let Some(binding) = binding {
        next.continuation.schema_version = preflighted_result_binding_schema_version(&binding);
        next.continuation.result_binding = Some(binding);
    }
    if let Err(error) =
        validate_effect_request(&next).and_then(|()| validate_continuation_size(&next.continuation))
    {
        return host.record_result(image, lease, Step::Fault(error));
    }
    let step = Step::Done(value);
    host.commit_successor(image, lease, &parent, &step, &next)
}

fn invalid() -> Fault {
    Fault {
        code: "LSV1407".into(),
        message: "invalid result-driven successor or missing original effect authority".into(),
    }
}
