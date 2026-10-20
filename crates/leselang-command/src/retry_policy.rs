//! Leserpent semantic retry policy over the shared numeric scheduling kernel.

use leselang_runtime_core::{RetryPlanError, RetrySchedulePoint, RetryTiming, plan_retry_schedule};
use serde::{Deserialize, Serialize};

pub const DEFAULT_MAX_SEMANTIC_RETRIES: u32 = 3;
pub const MAX_SEMANTIC_RETRIES: u32 = 16;
pub const DEFAULT_RETRY_BASE_DELAY_MS: u64 = 250;
pub const DEFAULT_RETRY_MAX_DELAY_MS: u64 = 30_000;
pub const MAX_RETRY_DELAY_MS: u64 = 60 * 60 * 1_000;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RetryPolicy {
    pub max_retries: u32,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: DEFAULT_MAX_SEMANTIC_RETRIES,
            base_delay_ms: DEFAULT_RETRY_BASE_DELAY_MS,
            max_delay_ms: DEFAULT_RETRY_MAX_DELAY_MS,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectErrorClass {
    Transient,
    Permanent,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EffectError {
    pub class: EffectErrorClass,
    pub code: String,
    pub message: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectRetryError {
    Metadata,
    Policy,
    Clock,
}

impl std::fmt::Display for EffectRetryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Metadata => "effect error metadata is invalid",
            Self::Policy => "semantic retry policy exceeds runtime bounds",
            Self::Clock => "semantic retry clock overflow",
        })
    }
}

impl std::error::Error for EffectRetryError {}

pub fn validate_retry_policy(policy: &RetryPolicy) -> Result<(), EffectRetryError> {
    if policy.max_retries > MAX_SEMANTIC_RETRIES
        || policy.base_delay_ms == 0
        || policy.base_delay_ms > MAX_RETRY_DELAY_MS
        || policy.max_delay_ms < policy.base_delay_ms
        || policy.max_delay_ms > MAX_RETRY_DELAY_MS
    {
        return Err(EffectRetryError::Policy);
    }
    Ok(())
}

pub fn validate_effect_error(error: &EffectError) -> Result<(), EffectRetryError> {
    if error.code.is_empty()
        || error.code.len() > 64
        || !error
            .code
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
        || error.message.is_empty()
        || error.message.len() > 1_024
    {
        return Err(EffectRetryError::Metadata);
    }
    Ok(())
}

/// Validates metadata before classification. Permanent errors bypass retry
/// policy and clock checks; transient errors validate policy before planning,
/// including when the retry budget is already exhausted.
///
/// A proposal is not dispatch authority. The host still owns ingress clock and
/// deadline admission, terminal replay, lease fencing, persistence and commit.
/// No error payload is consumed, no state is changed and no fuel is replenished.
pub fn plan_effect_retry(
    current_retry_count: u32,
    now_ms: u64,
    error: &EffectError,
    policy: &RetryPolicy,
) -> Result<Option<RetrySchedulePoint>, EffectRetryError> {
    validate_effect_error(error)?;
    if error.class == EffectErrorClass::Permanent {
        return Ok(None);
    }
    validate_retry_policy(policy)?;
    plan_retry_schedule(
        current_retry_count,
        now_ms,
        RetryTiming {
            max_retries: policy.max_retries,
            base_delay_ms: policy.base_delay_ms,
            max_delay_ms: policy.max_delay_ms,
        },
    )
    .map_err(|error| match error {
        RetryPlanError::Timing => EffectRetryError::Policy,
        RetryPlanError::Clock(_) => EffectRetryError::Clock,
    })
}
