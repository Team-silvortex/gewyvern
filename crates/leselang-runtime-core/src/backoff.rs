/// Exact capped exponential delay: `min(base_ms * 2^doublings, max_ms)`.
///
/// This allocation-free arithmetic accepts the full integer ranges, including
/// zero durations and a cap below the base. It never wraps or loops over the
/// exponent. Large exponents with a positive base return the cap; a zero base
/// always returns zero. Validation, retry classification, attempt numbering,
/// deadlines, jitter and wakeups belong to the host, not this helper.
///
/// ```
/// use leselang_runtime_core::capped_exponential_delay;
/// assert_eq!(capped_exponential_delay(10, 25, 0), 10);
/// assert_eq!(capped_exponential_delay(10, 25, 1), 20);
/// assert_eq!(capped_exponential_delay(10, 25, 2), 25);
/// assert_eq!(capped_exponential_delay(10, 25, u32::MAX), 25);
/// assert_eq!(capped_exponential_delay(0, 25, u32::MAX), 0);
/// ```
#[inline]
pub fn capped_exponential_delay(base_ms: u64, max_ms: u64, doublings: u32) -> u64 {
    if base_ms == 0 {
        return 0;
    }
    1_u64
        .checked_shl(doublings)
        .and_then(|multiplier| base_ms.checked_mul(multiplier))
        .unwrap_or(max_ms)
        .min(max_ms)
}
