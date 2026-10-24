//! Product retry entry coordination, without lease or retry commit authority.

use crate::{
    ContinuationToken, DispatchLease, EffectError, FailedEffect, RetryDisposition, RetryPolicy,
    RetrySchedule, Step,
};
use leselang_runtime_core::{Fault, validate_clock};

/// Trusted synchronous owner. Expiration and replay retain native retention and
/// group semantics. Settlement validates the original lease against live state,
/// commits the authoritative outcome, then publishes it locally. Visibility can
/// fail after commit; hooks are never retried or rolled back by this coordinator.
/// Input bounds, provenance, reporting authority and trusted time remain caller
/// obligations. A retry proposal is not permission to dispatch or refresh fuel.
pub trait ReferenceRetryHost {
    fn expire_due(&mut self, now_ms: u64) -> Result<(), Fault>;
    fn replay_completed(&mut self, token: &ContinuationToken) -> Result<Option<Step>, Fault>;
    fn visible_step(&mut self, token: &ContinuationToken, completed: Step) -> Step;
    fn settle_error(
        &mut self,
        lease: &DispatchLease,
        now_ms: u64,
        disposition: &RetryDisposition,
    ) -> Result<RetryDisposition, Fault>;
}

/// Terminal replay precedes error/policy admission. Only pending work reaches
/// the existing planner, followed by native lease fencing and settlement.
pub fn report_effect_error<Host: ReferenceRetryHost + ?Sized>(
    lease: &DispatchLease,
    now_ms: u64,
    error: EffectError,
    policy: &RetryPolicy,
    host: &mut Host,
) -> Result<RetryDisposition, Fault> {
    validate_clock(now_ms)?;
    host.expire_due(now_ms)?;
    if let Some(completed) = host.replay_completed(&lease.request.continuation.token)? {
        return Ok(RetryDisposition::Terminal(
            host.visible_step(&lease.request.continuation.token, completed),
        ));
    }
    let schedule =
        crate::retry_policy::plan_effect_retry(lease.retry_count, now_ms, &error, policy)
            .map_err(effect_retry_fault)?;
    let disposition = match schedule {
        None => RetryDisposition::Terminal(Step::Failed(FailedEffect {
            error,
            retry_count: lease.retry_count,
        })),
        Some(point) => RetryDisposition::Scheduled(RetrySchedule {
            retry_count: point.retry_count,
            ready_at_ms: point.ready_at_ms,
            error,
        }),
    };
    host.settle_error(lease, now_ms, &disposition)
}

/// Product diagnostic mapping only; planning success is not lease authority.
pub fn effect_retry_fault(error: crate::retry_policy::EffectRetryError) -> Fault {
    use crate::retry_policy::EffectRetryError;
    Fault {
        code: match error {
            EffectRetryError::Metadata => "LSV2202",
            EffectRetryError::Policy => "LSV2201",
            EffectRetryError::Clock => "LSV2203",
        }
        .into(),
        message: error.to_string(),
    }
}
