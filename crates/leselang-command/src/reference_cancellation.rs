//! Product cancellation entry policy, without cancellation or audit commit authority.

use crate::{
    Cancellation, CancellationReason, ContinuationImage, ContinuationToken, DebuggerAuditContext,
    DebuggerAuditRecord, Step,
};
use leselang_runtime_core::{Fault, validate_clock};

/// Trusted synchronous owner. Reconciliation observes current journal completion
/// and retention; the borrowed audited replay must not be cloned or replaced with
/// caller-supplied state. Finish hooks preserve native image/group checks, audit
/// identity/idempotency fences, transactional writes and commit-before-publication.
/// Visible-step selection can fail after commit; that is not permission to retry or
/// roll back. Input provenance, ingress bounds, debugger authority and trusted time
/// remain the caller's responsibility. A matching local image is not authorization.
pub trait ReferenceCancellationHost {
    fn expire_due(&mut self, now_ms: u64) -> Result<(), Fault>;
    fn replay_completed(&mut self, token: &ContinuationToken) -> Result<Option<Step>, Fault>;
    fn reconcile_completed(&mut self, token: &ContinuationToken) -> Result<Option<&Step>, Fault>;
    fn visible_step(&mut self, token: &ContinuationToken, completed: Step) -> Step;
    fn pending(&self, token: &ContinuationToken) -> Option<&ContinuationImage>;
    fn finish_cancellation(
        &mut self,
        image: &ContinuationImage,
        reason: CancellationReason,
        now_ms: u64,
    ) -> Step;
    fn finish_audited_cancellation(
        &mut self,
        image: &ContinuationImage,
        step: &Step,
        audit: &DebuggerAuditContext,
        now_ms: u64,
    ) -> Result<DebuggerAuditRecord, Fault>;
}

/// Ordinary cancellation replays authoritative completion before local image checks.
pub fn cancel_effect<Host: ReferenceCancellationHost + ?Sized>(
    image: &ContinuationImage,
    now_ms: u64,
    host: &mut Host,
) -> Step {
    if let Err(error) = validate_clock(now_ms) {
        return Step::Fault(error);
    }
    if let Err(error) = host.expire_due(now_ms) {
        return Step::Fault(error);
    }
    match host.replay_completed(&image.token) {
        Ok(Some(completed)) => return host.visible_step(&image.token, completed),
        Ok(None) => {}
        Err(error) => return Step::Fault(error),
    }
    let Some(stored) = host.pending(&image.token) else {
        return fault("LSV2004", "unknown continuation token");
    };
    if stored != image {
        return fault("LSV2005", "continuation image does not match pending state");
    }
    host.finish_cancellation(image, CancellationReason::Requested, now_ms)
}

/// Audit validation precedes time and replay. Only requested cancellation may
/// proceed to the native audit gate; ordinary cancellation cannot be retrofitted.
pub fn cancel_effect_audited<Host: ReferenceCancellationHost + ?Sized>(
    image: &ContinuationImage,
    now_ms: u64,
    audit: &DebuggerAuditContext,
    host: &mut Host,
) -> Result<DebuggerAuditRecord, Fault> {
    validate_audit(audit)?;
    validate_clock(now_ms)?;
    host.expire_due(now_ms)?;
    // Reconcile peer completion/retention just like ordinary cancellation.
    if let Some(completed) = host.reconcile_completed(&image.token)? {
        if !matches!(
            completed,
            Step::Cancelled(Cancellation {
                reason: CancellationReason::Requested,
                ..
            })
        ) {
            return Err(Fault {
                code: "LSV2014".to_string(),
                message: "completed effect cannot accept debugger cancellation audit".to_string(),
            });
        }
    } else {
        let Some(stored) = host.pending(&image.token) else {
            return Err(Fault {
                code: "LSV2004".to_string(),
                message: "unknown continuation token".to_string(),
            });
        };
        if stored != image {
            return Err(Fault {
                code: "LSV2005".to_string(),
                message: "continuation image does not match pending state".to_string(),
            });
        }
    }
    let step = Step::Cancelled(Cancellation {
        continuation: image.token.clone(),
        reason: CancellationReason::Requested,
        observed_at_ms: now_ms,
    });
    host.finish_audited_cancellation(image, &step, audit, now_ms)
}

/// Product diagnostic mapping only; metadata validity does not grant cancellation.
pub fn validate_audit(audit: &DebuggerAuditContext) -> Result<(), Fault> {
    crate::debugger_audit::validate_debugger_audit(audit).map_err(|error| Fault {
        code: "LSV2015".to_string(),
        message: error.to_string(),
    })
}

fn fault(code: &str, message: &str) -> Step {
    Step::Fault(Fault {
        code: code.into(),
        message: message.into(),
    })
}
