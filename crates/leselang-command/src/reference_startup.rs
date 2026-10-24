//! Reference startup policy, not a storage implementation or recovery proof.
//!
//! The native owner opens, validates and commits recovery before returning its
//! maps. These move-only carriers do not authenticate records or grant dispatch.
//! Closures may borrow native state; no Clone, Send or static bound is required.

use std::collections::BTreeMap;

use leselang_runtime_core::Fault;

use crate::reference_request::MAX_EXECUTION_FUEL;
use crate::scheduler_policy::validate_scheduler_limits;
use crate::{ContinuationImage, ContinuationToken, SchedulerLimits, Step};

/// Trusted native recovery output. Constructing this value is not validation.
pub struct RecoveredJournal<Journal> {
    pub next_sequence: u64,
    pub pending: BTreeMap<ContinuationToken, ContinuationImage>,
    pub completed: BTreeMap<ContinuationToken, Step>,
    pub journal: Journal,
}

/// Product state to move into a newly constructed owner, never a wire snapshot.
pub struct StartupState<Journal> {
    pub next_sequence: u64,
    pub fuel_limit: u64,
    pub pending: BTreeMap<ContinuationToken, ContinuationImage>,
    pub completed: BTreeMap<ContinuationToken, Step>,
    pub journal: Journal,
    pub scheduler_limits: SchedulerLimits,
}

/// Infallible default startup does not open storage or run recovery/validation.
pub fn in_memory<Journal>(
    fuel_limit: u64,
    create: impl FnOnce() -> Journal,
) -> StartupState<Journal> {
    StartupState {
        next_sequence: 1,
        fuel_limit: fuel_limit.min(MAX_EXECUTION_FUEL),
        pending: BTreeMap::new(),
        completed: BTreeMap::new(),
        journal: create(),
        scheduler_limits: SchedulerLimits::default(),
    }
}

pub fn in_memory_with_limits<Journal>(
    fuel_limit: u64,
    limits: SchedulerLimits,
    create: impl FnOnce() -> Journal,
) -> Result<StartupState<Journal>, Fault> {
    validate_limits(limits)?;
    let mut state = in_memory(fuel_limit, create);
    state.scheduler_limits = limits;
    Ok(state)
}

/// Validate policy before invoking the native open/recovery hook, including any
/// path conversion. Native errors and unwinds propagate without retry, cleanup
/// compensation or an in-memory fallback. A failed native open may have effects.
///
/// Move the original recovered maps/watermark without filtering, reclamping or
/// applying admission limits to existing work. Native recovery owns their proof;
/// only fuel for future roots is capped here, not budgets in saved continuations.
pub fn recover<Journal>(
    fuel_limit: u64,
    limits: SchedulerLimits,
    open: impl FnOnce() -> Result<RecoveredJournal<Journal>, Fault>,
) -> Result<StartupState<Journal>, Fault> {
    validate_limits(limits)?;
    let recovered = open()?;
    Ok(StartupState {
        next_sequence: recovered.next_sequence,
        fuel_limit: fuel_limit.min(MAX_EXECUTION_FUEL),
        pending: recovered.pending,
        completed: recovered.completed,
        journal: recovered.journal,
        scheduler_limits: limits,
    })
}

/// Same released diagnostic for startup and live policy replacement.
pub fn validate_limits(limits: SchedulerLimits) -> Result<(), Fault> {
    validate_scheduler_limits(limits).map_err(|error| Fault {
        code: "LSV2500".into(),
        message: error.to_string(),
    })
}
