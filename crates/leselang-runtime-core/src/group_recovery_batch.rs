//! Complete, ordered stored-group pass within a caller-owned recovery snapshot.

use std::fmt;

use crate::{ReservationError, ReservationSummary, SequenceReservations};

/// Explicit aggregate limits, derived from trusted host policy rather than rows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GroupRecoveryBatchLimits {
    pub max_groups: usize,
    pub max_reservations: usize,
    pub max_sequence: u64,
}

/// Native admission, numeric identity observation and per-group proof.
/// All hooks must observe one original snapshot and bound their own work.
/// Admission includes owner syntax/namespace exclusion and bounded plan checks.
/// The iterator reports every original reserved successor in stored order; it must
/// not filter duplicates, allocate replacements or normalize invalid numbers.
pub trait GroupRecoveryBatchHost<Stored> {
    type Group;
    type Error;

    fn admit_group(&mut self, stored: Stored) -> Result<Self::Group, Self::Error>;
    /// Zero is valid for an opaque legacy owner with no numeric reservation.
    fn highest_sequence(&mut self, group: &Self::Group) -> u64;
    fn reserved_sequences<'a>(
        &'a mut self,
        group: &'a Self::Group,
    ) -> impl Iterator<Item = u64> + 'a;
    fn validate_order(&mut self, group: &Self::Group) -> Result<(), Self::Error>;
    /// Load original members and run the complete per-group recovery proof.
    /// This must not publish recovered state, repair watermarks or commit storage.
    fn inspect_group(&mut self, group: Self::Group) -> Result<(), Self::Error>;
}

/// Native payloads remain owned and are never formatted or exposed as sources.
pub enum GroupRecoveryBatchError<Native> {
    Limit,
    Admission(Native),
    Reservation(ReservationError),
    Order(Native),
    Group(Native),
}
impl<Native> fmt::Display for GroupRecoveryBatchError<Native> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Limit => "stored group count exceeds recovery limit",
            Self::Admission(_) => "stored group admission failed",
            Self::Reservation(_) => "stored group reservations conflict",
            Self::Order(_) => "stored group execution order conflicts",
            Self::Group(_) => "stored group recovery proof failed",
        })
    }
}
impl<Native> fmt::Debug for GroupRecoveryBatchError<Native> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
impl<Native> std::error::Error for GroupRecoveryBatchError<Native> {}

/// Observations returned only after all groups succeed; not a recovery certificate,
/// allocation grant or committed snapshot. The host still validates numeric owner
/// observations, authentic authority/receipts and the original allocation watermark.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GroupRecoveryBatchSummary {
    pub groups: usize,
    pub reservations: ReservationSummary,
    pub highest_sequence: u64,
}

/// Admit each group, observe its original maximum, insert all successor reservations,
/// check order, then inspect its complete graph before admitting the next group.
/// Reservations are global to this pass, but per-group errors retain precedence
/// over later groups' admission/reservation errors. No partial summary escapes.
///
/// The vector bounds group count before host calls; aggregate bytes and loading
/// must already be bounded by the adapter. Owned rows move without Clone/Debug/
/// serde/thread bounds, and unused rows drop before handoff on refusal or unwind.
/// The borrowed host remains caller-owned. Native hooks/iterators/destructors are
/// trusted: unwind propagates without retry or rollback; a second panic can abort.
///
/// Compose this pass with checkpoint staging and a final same-snapshot transaction
/// commit. It does not execute saved code, settle effects, authenticate storage,
/// prevent replay, repair identity allocation or provide a standalone durable VM.
pub fn inspect_group_recovery_batch<Stored, Host: GroupRecoveryBatchHost<Stored>>(
    groups: Vec<Stored>,
    limits: GroupRecoveryBatchLimits,
    host: &mut Host,
) -> Result<GroupRecoveryBatchSummary, GroupRecoveryBatchError<Host::Error>> {
    if groups.len() > limits.max_groups {
        return Err(GroupRecoveryBatchError::Limit);
    }
    let count = groups.len();
    let mut reservations = SequenceReservations::new(limits.max_reservations, limits.max_sequence);
    let mut highest_sequence = 0;
    for stored in groups {
        let group = host
            .admit_group(stored)
            .map_err(GroupRecoveryBatchError::Admission)?;
        highest_sequence = highest_sequence.max(host.highest_sequence(&group));
        for sequence in host.reserved_sequences(&group) {
            reservations
                .insert(sequence)
                .map_err(GroupRecoveryBatchError::Reservation)?;
        }
        host.validate_order(&group)
            .map_err(GroupRecoveryBatchError::Order)?;
        host.inspect_group(group)
            .map_err(GroupRecoveryBatchError::Group)?;
    }
    let reservations = reservations
        .finish()
        .map_err(GroupRecoveryBatchError::Reservation)?;
    Ok(GroupRecoveryBatchSummary {
        groups: count,
        highest_sequence: highest_sequence.max(reservations.highest_sequence),
        reservations,
    })
}
