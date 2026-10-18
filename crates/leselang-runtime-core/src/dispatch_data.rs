use serde::{Deserialize, Serialize};

/// Owned delivery data with a host-defined request, not a live lease capability.
///
/// Construction/movement imposes no Clone/Debug/serde/Send/Sync/'static bound on
/// native payloads. Derived traits and thread ownership remain conditional.
/// Adapters validate ingress bounds, the original request, live authority, clock,
/// counters and current storage under the same owner/transaction as settlement.
/// Decoding never proves those checks or grants dispatch, replay or retry.
///
/// Field order, the omitted retry counter's zero default and permissive outer
/// unknown fields preserve the reference wire contract. The request's own codec
/// remains authoritative. Debug/serde may expose payloads; hosts own redaction.
///
/// ```compile_fail
/// use leselang_runtime_core::DispatchLease;
/// struct Native;
/// let lease = DispatchLease { request: Native, attempt: 1, retry_count: 0,
///     lease_expires_at_ms: 100 };
/// let copy = lease.clone();
/// ```
///
/// ```compile_fail
/// use leselang_runtime_core::DispatchLease;
/// use std::rc::Rc;
/// fn send<T: Send>(_: T) {}
/// send(DispatchLease { request: Rc::new(()), attempt: 1, retry_count: 0,
///     lease_expires_at_ms: 100 });
/// ```
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DispatchLease<Request> {
    pub request: Request,
    pub attempt: u32,
    #[serde(default)]
    pub retry_count: u32,
    pub lease_expires_at_ms: u64,
}

/// Host failure and observed retry count, without implicit error classification.
/// Native error ownership and codecs are conditional, as for `DispatchLease`.
///
/// ```compile_fail
/// use leselang_runtime_core::FailedEffect;
/// struct Native;
/// let failed = FailedEffect { error: Native, retry_count: 1 };
/// let wire = serde_json::to_string(&failed);
/// ```
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FailedEffect<Error> {
    pub error: Error,
    pub retry_count: u32,
}

/// Proposed retry data, not a timer, dispatch grant or committed schedule.
/// Hosts keep semantic retry counts separate from lease delivery attempts and
/// admission attempts. Policy ceilings, deadlines and persistence stay native.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RetrySchedule<Error> {
    pub retry_count: u32,
    pub ready_at_ms: u64,
    pub error: Error,
}
