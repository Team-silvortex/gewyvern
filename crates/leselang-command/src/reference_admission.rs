//! Reference root admission policy over the original host-neutral lifecycle.

use crate::Step;
use leselang_host_contract::{CapabilitySet, Principal, Revision};
use leselang_runtime_core::{Admission, AdmissionAdapter, AdmissionAttempt, Fault};
use leserpent_hir::HirProgram;
use serde::Serialize;

pub use leselang_runtime_core::{
    AdmissionEnd, AdmissionPolicy, AdmissionStatus, AdmissionWait, MAX_ADMISSION_ATTEMPTS,
    MAX_ADMISSION_DELAY_MS,
};

/// Trusted synchronous native entry. It must validate trusted time, expire due
/// work, retain static authorization and recheck capacity at native commit. Only
/// `LSV2501` may mean retryable pre-publication capacity pressure; never return it
/// after publishing work or an uncertain commit. The captured authority is an
/// input snapshot, not a renewable grant or live dispatch permission. Live access
/// checks remain the dispatch host's responsibility. Callback work stays trusted.
pub trait ReferenceAdmissionHost {
    fn start_timed(
        &mut self,
        program: &HirProgram,
        principal: Principal,
        capabilities: CapabilitySet,
        expected_revision: Option<Revision>,
        now_ms: u64,
        timeout_ms: u64,
    ) -> Step;
}

/// Exact reference wire DTO. Ignoring accepted output does not cancel its work.
///
/// ```compile_fail
/// #![deny(unused_must_use)]
/// use leselang_command::reference_admission::AdmissionPoll;
/// fn outcome() -> AdmissionPoll { AdmissionPoll::Finished }
/// outcome();
/// ```
#[must_use = "handle the admission outcome; accepted VM work is not cancelled by ignoring it"]
#[derive(Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
pub enum AdmissionPoll {
    Waiting(AdmissionWait),
    Started(Box<Step>),
    Rejected(Fault),
    Finished,
}

impl From<leselang_runtime_core::AdmissionPoll<Box<Step>>> for AdmissionPoll {
    fn from(outcome: leselang_runtime_core::AdmissionPoll<Box<Step>>) -> Self {
        match outcome {
            leselang_runtime_core::AdmissionPoll::Waiting(wait) => Self::Waiting(wait),
            leselang_runtime_core::AdmissionPoll::Started(step) => Self::Started(step),
            leselang_runtime_core::AdmissionPoll::Rejected(error) => Self::Rejected(error),
            leselang_runtime_core::AdmissionPoll::Finished => Self::Finished,
        }
    }
}

struct AdmissionRoot {
    program: HirProgram,
    principal: Principal,
    capabilities: CapabilitySet,
    expected_revision: Option<Revision>,
}

/// Single-use authority-capturing reference adapter. The shared core owns clocks,
/// attempts, backoff, terminal state and unwind sealing. This is neither a queue
/// nor durable recovery; accepted work belongs to the native owner. Debug output
/// remains the core's redacted metadata, without scripts or authority payloads.
///
/// ```compile_fail
/// #![deny(unused_must_use)]
/// use leselang_command::reference_admission::{AdmissionPolicy, RootAdmission};
/// use leserpent_hir::lower;
/// use leselang_host_contract::{CapabilitySet, Principal};
/// use leselang_syntax::parse;
/// RootAdmission::new(lower(&parse("fn main() = true")).unwrap(),
///     Principal::new("operator").unwrap(), CapabilitySet::default(),
///     None, 0, 1_000, AdmissionPolicy::default()).unwrap();
/// ```
#[must_use = "retain or poll the root admission; dropping it discards pending input"]
#[derive(Debug)]
pub struct RootAdmission {
    inner: Admission<AdmissionRoot>,
}

impl RootAdmission {
    pub fn new(
        program: HirProgram,
        principal: Principal,
        capabilities: CapabilitySet,
        expected_revision: Option<Revision>,
        now_ms: u64,
        timeout_ms: u64,
        policy: AdmissionPolicy,
    ) -> Result<Self, Fault> {
        Ok(Self {
            inner: Admission::new(
                AdmissionRoot {
                    program,
                    principal,
                    capabilities,
                    expected_revision,
                },
                now_ms,
                timeout_ms,
                policy,
            )?,
        })
    }

    pub fn attempts(&self) -> u32 {
        self.inner.attempts()
    }
    pub fn is_finished(&self) -> bool {
        self.inner.is_finished()
    }
    pub fn status(&self) -> AdmissionStatus {
        self.inner.status()
    }
    pub fn terminal_reason(&self) -> Option<AdmissionEnd> {
        self.inner.terminal_reason()
    }
    pub fn cancel(&mut self) -> bool {
        self.inner.cancel()
    }

    /// Only a due attempt invokes the native entry. Clock errors preserve the
    /// active handle; native unwind stays sealed as HostUncertain without replay.
    pub fn poll<Host: ReferenceAdmissionHost + ?Sized>(
        &mut self,
        host: &mut Host,
        now_ms: u64,
    ) -> Result<AdmissionPoll, Fault> {
        self.inner
            .poll(&mut AdmissionOwner(host), now_ms)
            .map(AdmissionPoll::from)
    }
}

struct AdmissionOwner<'a, Host: ?Sized>(&'a mut Host);

impl<Host: ReferenceAdmissionHost + ?Sized> AdmissionAdapter<AdmissionRoot>
    for AdmissionOwner<'_, Host>
{
    type Started = Box<Step>;

    fn try_admit(
        &mut self,
        input: &AdmissionRoot,
        now_ms: u64,
        deadline_at_ms: u64,
    ) -> AdmissionAttempt<Self::Started> {
        let step = self.0.start_timed(
            &input.program,
            input.principal.clone(),
            input.capabilities.clone(),
            input.expected_revision,
            now_ms,
            deadline_at_ms - now_ms,
        );
        match step {
            Step::Fault(error) if error.code == "LSV2501" => AdmissionAttempt::Backpressured(error),
            Step::Fault(error) => AdmissionAttempt::Rejected(error),
            step => AdmissionAttempt::Started(Box::new(step)),
        }
    }
}
