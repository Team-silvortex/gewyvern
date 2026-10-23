//! Released product terminal data, not an accepted completion or cancellation.
//!
//! Decoding retains the original field policy, including unchecked identities.
//! Owners still validate payload bounds, provenance and current state before
//! accepting an outcome, finalizing a merge or publishing a journal transition.

use leselang_runtime_core::{FailedEffect, Fault};
use serde::{Deserialize, Serialize};

use crate::{ContinuationToken, EffectError, Value};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CancellationReason {
    Requested,
    DeadlineExceeded,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Cancellation {
    pub continuation: ContinuationToken,
    pub reason: CancellationReason,
    pub observed_at_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
pub enum BranchOutcome {
    Value(Value),
    Cancelled(Cancellation),
    Failed(FailedEffect<EffectError>),
    Fault(Fault),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BranchCompletion {
    pub branch: String,
    pub outcome: BranchOutcome,
}
