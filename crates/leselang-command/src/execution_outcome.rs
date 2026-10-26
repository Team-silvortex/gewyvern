//! Released execution observations and dispatch data, not state-transition authority.
//!
//! Plain data retains the original per-type serde policy. Nested EffectRequest
//! decoding still checks complete request consistency; Yield frames and terminal
//! data do not gain semantic admission. Ingress bounds, provenance, clocks,
//! leases, group membership, retries and durable settlement remain owner checks.
//! Concrete enums preserve variant imports through the released VM reexports.

use crate::{
    Cancellation, ContinuationImage, ContinuationToken, EffectError, EffectRequest,
    SchedulerPressure, Value,
};
use leselang_runtime_core::Fault;
use serde::{Deserialize, Serialize};

/// In-process product computation observation, not a wire format or execution grant.
/// Borrowed/owned effects and captured bindings retain their original ownership.
pub enum ComputationOutcome<'a> {
    Scalar(leselang_runtime_core::ScalarValue),
    Result,
    Host(std::borrow::Cow<'a, leserpent_hir::Effect>),
    BoundHost {
        effect: std::borrow::Cow<'a, leserpent_hir::Effect>,
        binding: Box<crate::ResultBinding>,
    },
}

/// Released specialization of host-neutral owned dispatch data; no wire conversion.
pub type DispatchLease = leselang_runtime_core::DispatchLease<EffectRequest>;

pub type FailedEffect = leselang_runtime_core::FailedEffect<EffectError>;
pub type RetrySchedule = leselang_runtime_core::RetrySchedule<EffectError>;

// Keep the released enum (including variant imports); terminal policy stays product-owned.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
pub enum RetryDisposition {
    Scheduled(RetrySchedule),
    Terminal(Step),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct NamedEffectRequest {
    pub branch: String,
    pub request: EffectRequest,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StructuredEffectBatch {
    pub merge_token: ContinuationToken,
    pub branches: Vec<NamedEffectRequest>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MergeWait {
    pub merge_token: ContinuationToken,
    pub completed_branches: usize,
    pub total_branches: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
pub enum Step {
    Done(Value),
    Effect(Box<EffectRequest>),
    Effects(Box<StructuredEffectBatch>),
    Waiting(MergeWait),
    Yield(ContinuationImage),
    Cancelled(Cancellation),
    Failed(FailedEffect),
    Fault(Fault),
}

/// Backpressure never grants a lease or increments a delivery attempt.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    content = "payload",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum DispatchClaim {
    Leased(Box<DispatchLease>),
    Idle,
    Backpressured(SchedulerPressure),
}
