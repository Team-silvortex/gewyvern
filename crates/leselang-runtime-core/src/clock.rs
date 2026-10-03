use std::fmt;

/// Portable scheduler range shared by the core and the reference VM's signed storage.
pub const MAX_CLOCK_MS: u64 = i64::MAX as u64;

/// Allocation-free arithmetic failure; the host adapter owns diagnostic codes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClockError {
    OutOfRange,
    Overflow,
}

impl fmt::Display for ClockError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::OutOfRange => "scheduler clock is out of range",
            Self::Overflow => "scheduler clock addition exceeds the portable range",
        })
    }
}

impl std::error::Error for ClockError {}

/// Add to a trusted host clock without wrapping, clamping or advancing any state.
///
/// Clock range is checked before addition. Zero offset is valid, including at
/// `MAX_CLOCK_MS`. Duration policy, clock-domain identity, monotonicity, deadline
/// expiry and any deliberate wakeup clamping belong to the caller. This function
/// does not read a system clock, schedule a timer or create execution authority.
///
/// ```
/// use leselang_runtime_core::{ClockError, MAX_CLOCK_MS, checked_clock_add};
/// assert_eq!(checked_clock_add(100, 25), Ok(125));
/// assert_eq!(checked_clock_add(MAX_CLOCK_MS, 0), Ok(MAX_CLOCK_MS));
/// assert_eq!(checked_clock_add(MAX_CLOCK_MS, 1), Err(ClockError::Overflow));
/// assert_eq!(checked_clock_add(u64::MAX, 0), Err(ClockError::OutOfRange));
/// ```
#[inline]
pub fn checked_clock_add(now_ms: u64, offset_ms: u64) -> Result<u64, ClockError> {
    if now_ms > MAX_CLOCK_MS {
        return Err(ClockError::OutOfRange);
    }
    now_ms
        .checked_add(offset_ms)
        .filter(|at_ms| *at_ms <= MAX_CLOCK_MS)
        .ok_or(ClockError::Overflow)
}
