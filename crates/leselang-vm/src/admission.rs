use leselang_hir::HirProgram;
use leselang_host_contract::{CapabilitySet, Principal, Revision};
use leselang_runtime_core::{Admission, AdmissionAdapter, AdmissionAttempt};
use serde::Serialize;

use crate::{Fault, Step, Vm};

pub use leselang_runtime_core::{
    AdmissionEnd, AdmissionPolicy, AdmissionStatus, AdmissionWait, MAX_ADMISSION_ATTEMPTS,
    MAX_ADMISSION_DELAY_MS,
};

/// Concrete compatibility DTO; admission state and scheduling remain in the core.
/// Ignoring accepted output never cancels the VM work it represents.
///
/// ```compile_fail
/// #![deny(unused_must_use)]
/// use leselang_vm::AdmissionPoll;
/// fn poll_result() -> AdmissionPoll { AdmissionPoll::Finished }
/// poll_result();
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

/// The reference VM's authority-capturing adapter to the host-neutral lifecycle.
///
/// ```compile_fail
/// #![deny(unused_must_use)]
/// use leselang_hir::lower;
/// use leselang_host_contract::{CapabilitySet, Principal};
/// use leselang_syntax::parse;
/// use leselang_vm::{AdmissionPolicy, RootAdmission};
/// let program = lower(&parse("fn main() = true")).unwrap();
/// RootAdmission::new(
///     program, Principal::new("operator").unwrap(), CapabilitySet::default(),
///     None, 0, 1_000, AdmissionPolicy::default(),
/// ).unwrap();
/// ```
#[must_use = "retain or poll the root admission; dropping it discards pending input"]
#[derive(Debug)]
pub struct RootAdmission {
    inner: Admission<AdmissionRoot>,
}

impl RootAdmission {
    /// Captures immutable input and authority without entering a VM or reserving identity.
    ///
    /// ```
    /// use leselang_hir::lower;
    /// use leselang_host_contract::{CapabilitySet, Principal, Revision};
    /// use leselang_syntax::parse;
    /// use leselang_vm::{AdmissionPolicy, AdmissionPoll, RootAdmission, Step, Vm};
    ///
    /// let program = lower(&parse("fn main() = true")).unwrap();
    /// let mut admission = RootAdmission::new(
    ///     program, Principal::new("operator").unwrap(), CapabilitySet::default(),
    ///     Some(Revision(7)), 100, 1_000, AdmissionPolicy::default(),
    /// ).unwrap();
    /// let mut vm = Vm::default();
    /// assert!(matches!(admission.poll(&mut vm, 100).unwrap(),
    ///     AdmissionPoll::Started(step) if matches!(*step, Step::Done(_))));
    /// assert_eq!(admission.poll(&mut vm, 101).unwrap(), AdmissionPoll::Finished);
    /// ```
    pub fn new(
        program: HirProgram,
        principal: Principal,
        capabilities: CapabilitySet,
        expected_revision: Option<Revision>,
        now_ms: u64,
        timeout_ms: u64,
        policy: AdmissionPolicy,
    ) -> Result<Self, Fault> {
        let input = AdmissionRoot {
            program,
            principal,
            capabilities,
            expected_revision,
        };
        Ok(Self {
            inner: Admission::new(input, now_ms, timeout_ms, policy)?,
        })
    }

    pub fn attempts(&self) -> u32 {
        self.inner.attempts()
    }

    pub fn is_finished(&self) -> bool {
        self.inner.is_finished()
    }

    /// Observes scheduling metadata without evaluating HIR or entering the VM/journal.
    pub fn status(&self) -> AdmissionStatus {
        self.inner.status()
    }

    /// Observes only why admission ended, never accepted work or a replayable result.
    pub fn terminal_reason(&self) -> Option<AdmissionEnd> {
        self.inner.terminal_reason()
    }

    /// Cancels only a not-yet-admitted root; admitted work belongs to the VM/host.
    pub fn cancel(&mut self) -> bool {
        self.inner.cancel()
    }

    /// Clock errors leave the active handle unchanged; terminal outcomes consume it.
    pub fn poll(&mut self, vm: &mut Vm, now_ms: u64) -> Result<AdmissionPoll, Fault> {
        self.inner.poll(vm, now_ms).map(AdmissionPoll::from)
    }
}

impl AdmissionAdapter<AdmissionRoot> for Vm {
    type Started = Box<Step>;

    fn try_admit(
        &mut self,
        input: &AdmissionRoot,
        now_ms: u64,
        deadline_at_ms: u64,
    ) -> AdmissionAttempt<Self::Started> {
        let step = self.start_timed(
            &input.program,
            input.principal.clone(),
            input.capabilities.clone(),
            input.expected_revision,
            now_ms,
            deadline_at_ms - now_ms,
        );
        match step {
            // Only the VM's transactional, pre-publication capacity rejection is retryable.
            Step::Fault(error) if error.code == "LSV2501" => AdmissionAttempt::Backpressured(error),
            Step::Fault(error) => AdmissionAttempt::Rejected(error),
            step => AdmissionAttempt::Started(Box::new(step)),
        }
    }
}
