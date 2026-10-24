//! Product result-settlement routing, without storage or local-cache ownership.

pub mod interruption;
pub mod maintenance;
pub mod visibility;

use crate::reference_reply::CompletionLease;
use crate::{ContinuationImage, DispatchLease, EffectRequest, Step};
use leselang_runtime_core::Fault;

/// Trusted owner after result/continuation admission. Commit hooks retain all
/// validation, lease fences, peer-winner selection and transactional rollback.
/// Publication updates native caches and discovers the visible group/successor
/// outcome only after a successful commit. A publication fault or unwind does
/// not undo that commit. Hooks are never retried and borrowed inputs grant no
/// authority. Backend-specific replay semantics are not normalized here.
pub trait ReferenceSettlementHost {
    fn record_completed(
        &mut self,
        image: &ContinuationImage,
        proposed: &Step,
    ) -> Result<Step, Fault>;
    fn acknowledge_dispatch(
        &mut self,
        lease: &DispatchLease,
        now_ms: u64,
        proposed: &Step,
    ) -> Result<Step, Fault>;
    fn record_successor(
        &mut self,
        parent: &EffectRequest,
        proposed: &Step,
        next: &EffectRequest,
        lease: CompletionLease<'_>,
    ) -> Result<Step, Fault>;
    fn publish_completed(&mut self, image: &ContinuationImage, authoritative: Step) -> Step;
}

/// Select the original unleased/leased commit path and publish only its winner.
pub fn record_result<Host: ReferenceSettlementHost + ?Sized>(
    image: &ContinuationImage,
    lease: CompletionLease<'_>,
    step: Step,
    host: &mut Host,
) -> Step {
    let recorded = match lease {
        Some((lease, now_ms)) => host.acknowledge_dispatch(lease, now_ms, &step),
        None => host.record_completed(image, &step),
    };
    let authoritative = match recorded {
        Ok(step) => step,
        Err(error) => return Step::Fault(error),
    };
    host.publish_completed(image, authoritative)
}

/// Preserve the original parent/result/next borrows through the native transaction.
/// The owner may return a competing committed result instead of the proposed step.
pub fn commit_successor<Host: ReferenceSettlementHost + ?Sized>(
    image: &ContinuationImage,
    lease: CompletionLease<'_>,
    parent: &EffectRequest,
    step: &Step,
    next: &EffectRequest,
    host: &mut Host,
) -> Step {
    let authoritative = match host.record_successor(parent, step, next, lease) {
        Ok(step) => step,
        Err(error) => return Step::Fault(error),
    };
    host.publish_completed(image, authoritative)
}
