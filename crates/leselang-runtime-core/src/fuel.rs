use std::fmt;

/// Host-owned accounting for one execution context, not a wall-clock or memory bound.
///
/// There is no default grant, refill, clone or implicit wire representation. A host
/// chooses the initial amount and validates saved counters before constructing a
/// meter on re-entry. Reading the remaining count does not grant replay authority.
/// The host/evaluator must charge before the work it wants to bound; this primitive
/// cannot preempt native callbacks or force an adapter to follow its charging rules.
///
/// ```
/// use leselang_runtime_core::{Fuel, FuelExhausted};
/// let mut fuel = Fuel::new(3);
/// fuel.charge(2).unwrap();
/// assert_eq!(fuel.charge(2), Err(FuelExhausted));
/// assert_eq!(fuel.remaining(), 1);
/// fuel.charge(1).unwrap();
/// assert_eq!(fuel.remaining(), 0);
/// ```
///
/// ```compile_fail
/// use leselang_runtime_core::Fuel;
/// let fuel = Fuel::new(3);
/// let duplicate = fuel.clone();
/// ```
///
/// ```compile_fail
/// use leselang_runtime_core::Fuel;
/// let fuel = Fuel::new(3);
/// let moved = fuel;
/// assert_eq!(fuel.remaining(), 3);
/// ```
///
/// ```compile_fail
/// use leselang_runtime_core::Fuel;
/// let fuel: Fuel = serde_json::from_str("3").unwrap();
/// ```
///
/// ```compile_fail
/// use leselang_runtime_core::Fuel;
/// let wire = serde_json::to_string(&Fuel::new(3)).unwrap();
/// ```
#[must_use = "retain the meter and charge work against its host-granted budget"]
#[derive(Debug)]
pub struct Fuel {
    remaining: u64,
}

impl Fuel {
    /// The caller owns grant/restoration policy; no product-specific ceiling is applied.
    #[inline]
    pub const fn new(remaining: u64) -> Self {
        Self { remaining }
    }

    /// Read-only accounting metadata, not an independently restorable execution.
    #[inline]
    pub const fn remaining(&self) -> u64 {
        self.remaining
    }

    /// Debit atomically. Underflow leaves the counter unchanged, never wraps or panics.
    /// A zero cost is an accounting no-op, including on an empty meter.
    /// Exhaustion handling belongs to the evaluator; this error is not a retry grant.
    #[inline]
    pub fn charge(&mut self, cost: u64) -> Result<(), FuelExhausted> {
        let remaining = self.remaining.checked_sub(cost).ok_or(FuelExhausted)?;
        self.remaining = remaining;
        Ok(())
    }
}

/// Allocation-free accounting failure; the adapter owns diagnostic codes and context.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FuelExhausted;

impl fmt::Display for FuelExhausted {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("execution fuel exhausted")
    }
}

impl std::error::Error for FuelExhausted {}
