//! Product reply-entry coordination, not receipt or settlement authority.

use crate::reference_request::{validate_continuation_image, validate_effect_request};
use crate::{ContinuationImage, ContinuationToken, DispatchLease, EffectResult, Step};
use leselang_hir::Effect;
use leselang_runtime_core::{Fault, validate_clock};

pub type CompletionLease<'a> = Option<(&'a DispatchLease, u64)>;

/// Trusted live owner. Expiration validates trusted time before touching state;
/// replay reconciles authoritative completion/retention before returning a step.
/// Local image equality is not a receipt or lease certificate. Completion retains
/// native result checks, saved fuel, lease fencing and atomic journal settlement;
/// visible-step selection retains native group/successor progression. No callback
/// is retried, and earlier native side effects are not rolled back on refusal.
pub trait ReferenceReplyHost {
    fn expire_due(&mut self, now_ms: u64) -> Result<(), Fault>;
    fn replay_completed(&mut self, token: &ContinuationToken) -> Result<Option<Step>, Fault>;
    fn visible_step(&mut self, token: &ContinuationToken, completed: Step) -> Step;
    fn pending(&self, token: &ContinuationToken) -> Option<&ContinuationImage>;
    fn complete_result(
        &mut self,
        image: &ContinuationImage,
        lease: CompletionLease<'_>,
        result: EffectResult,
    ) -> Step;
}

/// Untimed reply entry. Input provenance and ingress bounds remain caller-owned.
/// A completed replay intentionally ignores the new reply; it cannot re-execute it.
pub fn resume<Host: ReferenceReplyHost + ?Sized>(
    image: &ContinuationImage,
    result: EffectResult,
    host: &mut Host,
) -> Step {
    if image.deadline_at_ms.is_some() {
        return fault(
            "LSV2111",
            "timed effects require resume_at with trusted scheduler time",
        );
    }
    resume_inner(image, result, host)
}

/// Explicit scheduler-time entry, preserving clock validation before expiration.
pub fn resume_at<Host: ReferenceReplyHost + ?Sized>(
    image: &ContinuationImage,
    now_ms: u64,
    result: EffectResult,
    host: &mut Host,
) -> Step {
    if let Err(error) = validate_clock(now_ms) {
        return Step::Fault(error);
    }
    if let Err(error) = host.expire_due(now_ms) {
        return Step::Fault(error);
    }
    resume_inner(image, result, host)
}

fn resume_inner<Host: ReferenceReplyHost + ?Sized>(
    image: &ContinuationImage,
    result: EffectResult,
    host: &mut Host,
) -> Step {
    match host.replay_completed(&image.token) {
        Ok(Some(completed)) => return host.visible_step(&image.token, completed),
        Ok(None) => {}
        Err(error) => return Step::Fault(error),
    }
    if let Err(error) = validate_continuation_image(image) {
        return Step::Fault(error);
    }
    let Some(stored) = host.pending(&image.token) else {
        return fault("LSV2004", "unknown continuation token");
    };
    if stored != image {
        return fault("LSV2005", "continuation image does not match pending state");
    }
    if matches!(
        image.pending_effect,
        Effect::RuntimeRefresh { .. }
            | Effect::RuntimeCapabilitiesRefresh { .. }
            | Effect::RuntimeDeploy { .. }
            | Effect::DebuggerCancel { .. }
    ) {
        return fault(
            "LSV2110",
            "mutating effects must be completed through dispatch acknowledgement",
        );
    }
    host.complete_result(image, None, result)
}

/// Dispatch reply entry. Expiration precedes request admission, which precedes
/// completed replay. The original borrowed lease reaches native settlement intact;
/// successful static admission never substitutes for transactional lease checks.
pub fn acknowledge_effect<Host: ReferenceReplyHost + ?Sized>(
    lease: &DispatchLease,
    now_ms: u64,
    result: EffectResult,
    host: &mut Host,
) -> Step {
    if let Err(error) = host.expire_due(now_ms) {
        return Step::Fault(error);
    }
    if let Err(error) = validate_effect_request(&lease.request) {
        return Step::Fault(error);
    }
    let image = &lease.request.continuation;
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
    host.complete_result(image, Some((lease, now_ms)), result)
}

fn fault(code: &str, message: &str) -> Step {
    Step::Fault(Fault {
        code: code.into(),
        message: message.into(),
    })
}
