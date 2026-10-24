//! Cancellation and retry settlement policy after caller-owned admission.

use crate::{
    Cancellation, CancellationReason, ContinuationImage, DebuggerAuditContext, DebuggerAuditRecord,
    DispatchLease, RetryDisposition, Step,
};
use leselang_runtime_core::Fault;

/// Native journal authority and owned cancellation publication. Hooks retain
/// validation, audit identity/idempotency checks, transactions and peer winners.
/// They run once; neither failure nor unwind retries or undoes native effects.
/// Publication may fail after commit. Input metadata grants no authority here.
pub trait ReferenceCancellationSettlementHost {
    fn cancel(&mut self, image: &ContinuationImage, proposed: &Step) -> Result<Step, Fault>;
    fn cancel_audited(
        &mut self,
        image: &ContinuationImage,
        proposed: &Step,
        audit: &DebuggerAuditContext,
        now_ms: u64,
    ) -> Result<(Step, DebuggerAuditRecord), Fault>;
    fn publish_cancellation(&mut self, image: &ContinuationImage, authoritative: Step) -> Step;
}

/// Native retry authority and borrowed terminal publication. The native owner
/// retains cache-copy order and visible-result selection. Scheduled dispositions
/// are returned intact without publication; terminal values remain borrowed from
/// the committed disposition until publication returns. Hooks are never retried.
pub trait ReferenceRetrySettlementHost {
    fn report_error(
        &mut self,
        lease: &DispatchLease,
        now_ms: u64,
        proposed: &RetryDisposition,
    ) -> Result<RetryDisposition, Fault>;
    fn publish_retry_terminal(&mut self, image: &ContinuationImage, authoritative: &Step) -> Step;
}

/// Construct the original cancellation proposal, then publish the native winner.
pub fn finish_cancellation<Host: ReferenceCancellationSettlementHost + ?Sized>(
    image: &ContinuationImage,
    reason: CancellationReason,
    observed_at_ms: u64,
    host: &mut Host,
) -> Step {
    let step = Step::Cancelled(Cancellation {
        continuation: image.token.clone(),
        reason,
        observed_at_ms,
    });
    let authoritative = match host.cancel(image, &step) {
        Ok(step) => step,
        Err(error) => return Step::Fault(error),
    };
    host.publish_cancellation(image, authoritative)
}

/// A non-cancellation winner returns its audit record without cache publication.
/// A cancellation winner publishes first; a visible fault then replaces the record
/// in this call's return value, without undoing the already committed audit.
pub fn finish_audited_cancellation<Host: ReferenceCancellationSettlementHost + ?Sized>(
    image: &ContinuationImage,
    step: &Step,
    audit: &DebuggerAuditContext,
    now_ms: u64,
    host: &mut Host,
) -> Result<DebuggerAuditRecord, Fault> {
    let (authoritative, record) = host.cancel_audited(image, step, audit, now_ms)?;
    if matches!(authoritative, Step::Cancelled(_))
        && let Step::Fault(error) = host.publish_cancellation(image, authoritative)
    {
        return Err(error);
    }
    Ok(record)
}

/// Distinguish a commit error from a committed terminal result whose visible step
/// is a fault. The latter remains `Ok(Terminal(Fault))`, not a retryable commit error.
pub fn settle_effect_error<Host: ReferenceRetrySettlementHost + ?Sized>(
    lease: &DispatchLease,
    now_ms: u64,
    disposition: &RetryDisposition,
    host: &mut Host,
) -> Result<RetryDisposition, Fault> {
    let authoritative = host.report_error(lease, now_ms, disposition)?;
    if let RetryDisposition::Terminal(step) = &authoritative {
        return Ok(RetryDisposition::Terminal(
            host.publish_retry_terminal(&lease.request.continuation, step),
        ));
    }
    Ok(authoritative)
}
