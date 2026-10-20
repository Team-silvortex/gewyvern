//! Admission of an original durable dispatch before checkpoint publication.

use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DispatchRecoveryLimits {
    pub max_attempts: u32,
    pub max_retries: u32,
}

/// Faithful stored observations, not repaired counters or execution authority.
/// Signed inputs preserve rejection of corrupt negative storage values. Clock
/// domain, state decoding and row/schema integrity remain native policy.
pub struct StoredDispatchMetadata<Retry> {
    pub attempt: i64,
    pub ready_at_ms: i64,
    pub retry_count: i64,
    pub last_error: Option<Retry>,
    pub acknowledged: bool,
    pub effect_completed: bool,
}

/// Reads/proofs must use one original storage snapshot and bound native work.
/// The adapter owns codecs, schema checks, authentic requests and current authority.
pub trait DispatchRecoveryHost {
    type Request;
    type Retry;
    type Error;

    fn admit_request(&mut self) -> Result<Self::Request, Self::Error>;
    fn validate_identity(&mut self, request: &Self::Request) -> Result<(), Self::Error>;
    fn validate_image(&mut self, request: &Self::Request) -> Result<(), Self::Error>;
    fn read_metadata(&mut self) -> Result<StoredDispatchMetadata<Self::Retry>, Self::Error>;
    fn validate_retry_error(&mut self, original: Self::Retry) -> Result<(), Self::Error>;
}

/// Stage-only diagnostics never format native requests, retry data or errors.
pub enum DispatchRecoveryError<Native> {
    Request(Native),
    Identity(Native),
    Image(Native),
    Metadata(Native),
    Attempt,
    RetryState,
    RetryHistory,
    RetryError(Native),
    CompletionState,
}

impl<Native> fmt::Display for DispatchRecoveryError<Native> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Request(_) => "stored dispatch request admission failed",
            Self::Identity(_) => "stored dispatch identity conflicts",
            Self::Image(_) => "stored dispatch image conflicts",
            Self::Metadata(_) => "stored dispatch metadata read failed",
            Self::Attempt => "stored dispatch attempt is invalid",
            Self::RetryState => "stored dispatch retry state is invalid",
            Self::RetryHistory => "stored dispatch retry history is inconsistent",
            Self::RetryError(_) => "stored dispatch retry error admission failed",
            Self::CompletionState => "stored dispatch completion state conflicts",
        })
    }
}
impl<Native> fmt::Debug for DispatchRecoveryError<Native> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
impl<Native> std::error::Error for DispatchRecoveryError<Native> {}

/// Admit the request, prove identity/image, then read metadata before checking
/// attempts, retry counters/history, original retry error, and completion parity.
/// Zero ceilings allow stored zero counters, not any new delivery/retry. No value
/// is clamped or reset. An absent retry error is never decoded; a present error is
/// moved to the native decoder only after numeric/history checks pass.
///
/// Original owners need no Clone, Debug, serde, comparison or thread traits; the
/// borrowed host may be unsized. Refusal/unwind stops without retry. Native hooks
/// and destructors remain trusted and their side effects cannot be rolled back.
/// This pass owns no storage, evaluator, lease, fuel or dispatch. A success is not
/// replay permission: retain schema/global graph/authority checks and atomically
/// commit the full checkpoint before publishing any recovered execution state.
pub fn inspect_dispatch_recovery<Host: DispatchRecoveryHost + ?Sized>(
    host: &mut Host,
    limits: DispatchRecoveryLimits,
) -> Result<(), DispatchRecoveryError<Host::Error>> {
    let request = host
        .admit_request()
        .map_err(DispatchRecoveryError::Request)?;
    host.validate_identity(&request)
        .map_err(DispatchRecoveryError::Identity)?;
    host.validate_image(&request)
        .map_err(DispatchRecoveryError::Image)?;
    let metadata = host
        .read_metadata()
        .map_err(DispatchRecoveryError::Metadata)?;
    if metadata.attempt < 0 || metadata.attempt > i64::from(limits.max_attempts) {
        return Err(DispatchRecoveryError::Attempt);
    }
    if metadata.ready_at_ms < 0
        || metadata.retry_count < 0
        || metadata.retry_count > i64::from(limits.max_retries)
    {
        return Err(DispatchRecoveryError::RetryState);
    }
    if (metadata.retry_count == 0) != metadata.last_error.is_none() {
        return Err(DispatchRecoveryError::RetryHistory);
    }
    if let Some(original) = metadata.last_error {
        host.validate_retry_error(original)
            .map_err(DispatchRecoveryError::RetryError)?;
    }
    if metadata.acknowledged != metadata.effect_completed {
        return Err(DispatchRecoveryError::CompletionState);
    }
    drop(request);
    Ok(())
}
