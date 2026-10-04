#![forbid(unsafe_code)]

//! Host-neutral admission, scalar/projection/control/recovery and structural accounting foundations.
//! Evaluation, fallback execution, charging rules and authority belong to the adapter.
//!
//! This is a runtime foundation, not yet the independent Leselang evaluator.

use std::fmt;

use serde::{Deserialize, Serialize};

mod argument_typing;
mod backoff;
mod catalog;
mod clock;
mod collection;
mod control;
mod fold;
mod fuel;
mod projection;
mod recovery;
mod scalar_contract;
mod scalar_ops;
mod scope;
mod signature;
mod structure;
mod value;

pub use argument_typing::{
    ArgumentTypeError, OperationTypeError, ScalarArgumentDomain, ScalarArgumentType,
    check_argument_type,
};
pub use backoff::capped_exponential_delay;
pub use catalog::{
    OperationCatalog, OperationCatalogError, OperationCatalogLimits, OperationSchema,
};
pub use clock::{ClockError, MAX_CLOCK_MS, checked_clock_add};
pub use collection::StringListBuilder;
pub use control::{
    BinarySelection, LoopBudget, LoopError, LoopStep, MAX_LOOP_ITERATIONS, select_binary_left,
};
pub use fold::{FoldCursor, FoldError};
pub use fuel::{Fuel, FuelExhausted};
pub use projection::{ProjectionError, ScalarProjectionField, validate_scalar_projection};
pub use recovery::CalculationFailure;
pub use scalar_contract::{ScalarContractError, ScalarTypeSet};
pub use scalar_ops::{BinaryOperator, ScalarError, UnaryOperator, apply_binary, apply_unary};
pub use scope::{ScopeError, ScopeFrame};
pub use signature::{
    NamedArgumentBindings, NamedArgumentError, NamedParameter, bind_named_arguments,
    validate_named_arguments,
};
pub use structure::{StructureBudget, StructureError};
pub use value::{
    MAX_SCALAR_STRING_BYTES, MAX_STRING_LIST_ITEMS, OptionalStringValue, ScalarType, ScalarValue,
    StringListValue,
};

pub const MAX_ADMISSION_ATTEMPTS: u32 = 32;
pub const MAX_ADMISSION_DELAY_MS: u64 = 60 * 60 * 1000;
pub const MAX_EXECUTION_TIMEOUT_MS: u64 = 24 * 60 * 60 * 1000;

/// Stable machine-readable fault shared with the reference VM.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Fault {
    pub code: String,
    pub message: String,
}

/// Bounded pre-admission retry, distinct from redelivery and semantic effect retry.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AdmissionPolicy {
    pub max_attempts: u32,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
}

impl Default for AdmissionPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 8,
            base_delay_ms: 250,
            max_delay_ms: 30_000,
        }
    }
}

impl AdmissionPolicy {
    /// Pure preflight, before the host constructs or takes ownership of any request.
    /// Diagnostics name the failed constraint without echoing submitted values.
    pub fn validate(self) -> Result<(), Fault> {
        if self.max_attempts == 0 || self.max_attempts > MAX_ADMISSION_ATTEMPTS {
            return Err(fault(
                "LSV2510",
                format!("admission max_attempts must be between 1 and {MAX_ADMISSION_ATTEMPTS}"),
            ));
        }
        if self.base_delay_ms == 0 || self.base_delay_ms > MAX_ADMISSION_DELAY_MS {
            return Err(fault(
                "LSV2510",
                format!(
                    "admission base_delay_ms must be between 1 and {MAX_ADMISSION_DELAY_MS} ms"
                ),
            ));
        }
        if self.max_delay_ms > MAX_ADMISSION_DELAY_MS {
            return Err(fault(
                "LSV2510",
                format!("admission max_delay_ms must not exceed {MAX_ADMISSION_DELAY_MS} ms"),
            ));
        }
        if self.max_delay_ms < self.base_delay_ms {
            return Err(fault(
                "LSV2510",
                "admission max_delay_ms must be at least base_delay_ms",
            ));
        }
        Ok(())
    }

    fn delay(self, attempts: u32) -> u64 {
        capped_exponential_delay(
            self.base_delay_ms,
            self.max_delay_ms,
            attempts.saturating_sub(1),
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct AdmissionWait {
    pub attempts: u32,
    pub retry_at_ms: u64,
    pub deadline_at_ms: u64,
}

/// Read-only scheduling metadata, never input, authority or a replayable handle.
/// `Pending` reports the next permitted poll, not a reservation or a readiness guarantee.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
pub enum AdmissionStatus {
    Pending(AdmissionWait),
    /// The start/rejection outcome belongs to its caller; it is not retained here.
    Finished {
        attempts: u32,
    },
}

/// Why the admission handle was consumed, not an execution result or replay authority.
///
/// `Started` means the adapter reported acceptance, not that the caller received its
/// output or the accepted work completed. `HostUncertain` preserves an unknown host
/// publication state after callback unwinding; it must never be treated as rejection
/// or automatic permission to resubmit. Cleanup unwinding does not erase a known end.
/// The metadata retains no input, success output, host error or panic payload.
///
/// This observation cannot be deserialized into a restorable handle:
///
/// ```compile_fail
/// use leselang_runtime_core::AdmissionEnd;
/// let end: AdmissionEnd = serde_json::from_str("\"started\"").unwrap();
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AdmissionEnd {
    Started,
    Rejected,
    Cancelled,
    Expired,
    Exhausted,
    HostUncertain,
}

/// Outcomes are observations, not persisted or restorable admission handles.
/// The host must handle accepted output; ignoring it does not undo admission.
///
/// ```compile_fail
/// #![deny(unused_must_use)]
/// use leselang_runtime_core::AdmissionPoll;
/// fn poll_result() -> AdmissionPoll<()> { AdmissionPoll::Started(()) }
/// poll_result();
/// ```
#[must_use = "handle the admission outcome; accepted work is not cancelled by ignoring it"]
#[derive(Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
pub enum AdmissionPoll<Started> {
    Waiting(AdmissionWait),
    Started(Started),
    Rejected(Fault),
    Finished,
}

/// Explicit host classification; the core never interprets host error codes.
#[derive(Debug, Eq, PartialEq)]
pub enum AdmissionAttempt<Started> {
    Started(Started),
    /// Temporary pressure. The adapter MUST have published no accepted work.
    Backpressured(Fault),
    Rejected(Fault),
}

/// Synchronous admission adapter for an opaque, immutable request.
///
/// The host validates authority and atomically admits work before returning `Started`.
/// `Backpressured` permits another call with the same input: no accepted continuation
/// or externally published work may have escaped that attempt. Unused monotonic
/// identity reservations may leave gaps, but must never be reused. The core cannot
/// enforce this host-side atomicity. Callbacks must themselves be bounded;
/// this lifecycle has no preemption, timers, hidden queue or global lock.
/// A callback panic propagates, but consumes the handle so unwinding cannot enable
/// a second attempt after a host may already have published work.
pub trait AdmissionAdapter<Input> {
    type Started;

    /// `now_ms < deadline_at_ms`; waiting never extends this absolute deadline.
    fn try_admit(
        &mut self,
        input: &Input,
        now_ms: u64,
        deadline_at_ms: u64,
    ) -> AdmissionAttempt<Self::Started>;
}

/// One host-owned, non-cloneable request. Terminal outcomes release its input.
/// Dropping it discards pending input without attempting admission.
///
/// ```compile_fail
/// #![deny(unused_must_use)]
/// use leselang_runtime_core::{Admission, AdmissionPolicy};
/// Admission::new((), 0, 1_000, AdmissionPolicy::default()).unwrap();
/// ```
///
/// Thread transfer is conditional on the input, not required for GUI-local hosts.
/// An input containing `Rc` must stay on its owning thread:
///
/// ```compile_fail
/// use std::{rc::Rc, thread};
/// use leselang_runtime_core::{Admission, AdmissionPolicy};
/// let handle = Admission::new(Rc::new(()), 0, 1_000, AdmissionPolicy::default()).unwrap();
/// thread::spawn(move || drop(handle));
/// ```
#[must_use = "retain or poll the admission handle; dropping it discards pending input"]
pub struct Admission<Input> {
    state: AdmissionState<Input>,
    policy: AdmissionPolicy,
    attempts: u32,
    retry_at_ms: u64,
    deadline_at_ms: u64,
    last_observed_at_ms: u64,
}

enum AdmissionState<Input> {
    Pending(Input),
    Finished(AdmissionEnd),
}

impl<Input> fmt::Debug for Admission<Input> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Admission")
            .field("status", &self.status())
            .field("terminal_reason", &self.terminal_reason())
            .field("policy", &self.policy)
            .field("last_observed_at_ms", &self.last_observed_at_ms)
            .finish()
    }
}

impl<Input> Admission<Input> {
    /// Captures input without calling the adapter or reserving any identity.
    pub fn new(
        input: Input,
        now_ms: u64,
        timeout_ms: u64,
        policy: AdmissionPolicy,
    ) -> Result<Self, Fault> {
        policy.validate()?;
        let deadline_at_ms = validate_execution_deadline(now_ms, timeout_ms)?;
        Ok(Self {
            state: AdmissionState::Pending(input),
            policy,
            attempts: 0,
            retry_at_ms: now_ms,
            deadline_at_ms,
            last_observed_at_ms: now_ms,
        })
    }

    pub fn attempts(&self) -> u32 {
        self.attempts
    }

    pub fn is_finished(&self) -> bool {
        matches!(self.state, AdmissionState::Finished(_))
    }

    /// Constant-size observation: no host call, clock update, expiry or attempt spent.
    pub fn status(&self) -> AdmissionStatus {
        if self.is_finished() {
            AdmissionStatus::Finished {
                attempts: self.attempts,
            }
        } else {
            AdmissionStatus::Pending(self.wait())
        }
    }

    /// Constant-size, payload-free metadata. Observation does not poll or expire input.
    /// `None` means still pending; the original `AdmissionPoll` belongs to its caller.
    ///
    /// ```
    /// use leselang_runtime_core::{Admission, AdmissionEnd, AdmissionPolicy};
    /// let mut handle = Admission::new((), 0, 1_000, AdmissionPolicy::default()).unwrap();
    /// assert_eq!(handle.terminal_reason(), None);
    /// assert!(handle.cancel());
    /// assert_eq!(handle.terminal_reason(), Some(AdmissionEnd::Cancelled));
    /// assert!(!handle.cancel());
    /// ```
    pub fn terminal_reason(&self) -> Option<AdmissionEnd> {
        match self.state {
            AdmissionState::Pending(_) => None,
            AdmissionState::Finished(reason) => Some(reason),
        }
    }

    /// Cancels only local, not-yet-admitted input, never work already accepted by a host.
    pub fn cancel(&mut self) -> bool {
        self.take_input(AdmissionEnd::Cancelled).is_some()
    }

    /// Clock errors leave the active handle unchanged; terminal outcomes consume it.
    /// Handling `Result` with `?` alone is not handling the admission outcome:
    ///
    /// ```compile_fail
    /// #![deny(unused_must_use)]
    /// use leselang_runtime_core::{
    ///     Admission, AdmissionAdapter, AdmissionAttempt, AdmissionPolicy, Fault,
    /// };
    /// struct Host;
    /// impl AdmissionAdapter<()> for Host {
    ///     type Started = ();
    ///     fn try_admit(&mut self, _: &(), _: u64, _: u64) -> AdmissionAttempt<()> {
    ///         AdmissionAttempt::Started(())
    ///     }
    /// }
    /// fn submit() -> Result<(), Fault> {
    ///     let mut handle = Admission::new((), 0, 1_000, AdmissionPolicy::default())?;
    ///     handle.poll(&mut Host, 0)?;
    ///     Ok(())
    /// }
    /// ```
    pub fn poll<Host: AdmissionAdapter<Input> + ?Sized>(
        &mut self,
        host: &mut Host,
        now_ms: u64,
    ) -> Result<AdmissionPoll<Host::Started>, Fault> {
        if self.is_finished() {
            return Ok(AdmissionPoll::Finished);
        }
        validate_clock(now_ms)?;
        if now_ms < self.last_observed_at_ms {
            return Err(fault("LSV2511", "admission clock moved backwards"));
        }
        self.last_observed_at_ms = now_ms;
        if now_ms >= self.deadline_at_ms {
            // Consume before host-owned input cleanup, even if its destructor unwinds.
            drop(self.take_input(AdmissionEnd::Expired));
            return Ok(AdmissionPoll::Rejected(fault(
                "LSV2513",
                "root expired before admission",
            )));
        }
        if now_ms < self.retry_at_ms {
            return Ok(AdmissionPoll::Waiting(self.wait()));
        }

        // Take ownership before entering host code: only explicit pressure can rearm.
        let Some(input) = self.take_input(AdmissionEnd::HostUncertain) else {
            return Ok(AdmissionPoll::Finished);
        };
        self.attempts += 1;
        match host.try_admit(&input, now_ms, self.deadline_at_ms) {
            AdmissionAttempt::Backpressured(_) => {
                if self.attempts >= self.policy.max_attempts {
                    self.state = AdmissionState::Finished(AdmissionEnd::Exhausted);
                    drop(input);
                    return Ok(AdmissionPoll::Rejected(fault(
                        "LSV2512",
                        format!(
                            "admission exhausted after {} {}",
                            self.attempts,
                            if self.attempts == 1 {
                                "attempt"
                            } else {
                                "attempts"
                            },
                        ),
                    )));
                }
                self.retry_at_ms = checked_clock_add(now_ms, self.policy.delay(self.attempts))
                    .unwrap_or(self.deadline_at_ms)
                    .min(self.deadline_at_ms);
                self.state = AdmissionState::Pending(input);
                Ok(AdmissionPoll::Waiting(self.wait()))
            }
            AdmissionAttempt::Rejected(error) => {
                self.state = AdmissionState::Finished(AdmissionEnd::Rejected);
                drop(input);
                Ok(AdmissionPoll::Rejected(error))
            }
            AdmissionAttempt::Started(started) => {
                self.state = AdmissionState::Finished(AdmissionEnd::Started);
                // Keep output locally owned until cleanup succeeds, including on unwind.
                drop(input);
                Ok(AdmissionPoll::Started(started))
            }
        }
    }

    fn take_input(&mut self, reason: AdmissionEnd) -> Option<Input> {
        match std::mem::replace(&mut self.state, AdmissionState::Finished(reason)) {
            AdmissionState::Pending(input) => Some(input),
            AdmissionState::Finished(existing) => {
                self.state = AdmissionState::Finished(existing);
                None
            }
        }
    }

    fn wait(&self) -> AdmissionWait {
        AdmissionWait {
            attempts: self.attempts,
            retry_at_ms: self.retry_at_ms,
            deadline_at_ms: self.deadline_at_ms,
        }
    }
}

/// A caller-owned clock in the portable signed 64-bit range used by the reference VM.
pub fn validate_clock(now_ms: u64) -> Result<(), Fault> {
    checked_clock_add(now_ms, 0)
        .map(|_| ())
        .map_err(|_| fault("LSV2011", "scheduler clock is out of range"))
}

/// Validate a bounded relative timeout and pin its absolute deadline.
pub fn validate_execution_deadline(now_ms: u64, timeout_ms: u64) -> Result<u64, Fault> {
    validate_clock(now_ms)?;
    if timeout_ms == 0 || timeout_ms > MAX_EXECUTION_TIMEOUT_MS {
        return Err(fault(
            "LSV2012",
            format!("effect timeout must be between 1 and {MAX_EXECUTION_TIMEOUT_MS} ms"),
        ));
    }
    checked_clock_add(now_ms, timeout_ms)
        .map_err(|_| fault("LSV2011", "effect absolute deadline is out of range"))
}

fn fault(code: &str, message: impl Into<String>) -> Fault {
    Fault {
        code: code.into(),
        message: message.into(),
    }
}
