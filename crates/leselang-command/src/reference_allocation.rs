//! Product sequence selection over native reservations and local cache occupancy.
//!
//! This is trusted owner coordination, not a durable allocator or an execution
//! grant. Native storage retains transactionality, shared-worker uniqueness and
//! its original fault precedence. Local cache reads cannot certify global absence.

use crate::ContinuationToken;
use crate::continuation_identity::MAX_EFFECT_SEQUENCE;
use leselang_runtime_core::{Fault, SequenceWatermarkRequirement};

/// Hooks operate on the same serially entered owner. The next allocation must
/// honor the published local watermark or a newer native reservation frontier.
/// A collision consumes its candidate and causes a fresh allocation, never a retry
/// or rollback of the old reservation.
/// Errors and unwinds stop immediately without compensating earlier effects.
pub trait ReferenceAllocationHost {
    fn local_next(&self) -> u64;
    fn allocate_sequence(&mut self, local_next: u64) -> Result<u64, Fault>;
    fn publish_next(&mut self, next_sequence: u64);
    fn pending_contains(&self, token: &ContinuationToken) -> bool;
    fn completed_contains(&self, token: &ContinuationToken) -> bool;
}

/// Preserve native allocation -> product range -> watermark -> pending/completed
/// lookup order. Do not prevalidate the local hint, normalize backend faults, scan
/// ahead or reclaim collisions. Pending occupancy short-circuits completed lookup.
pub fn allocate_sequence<Host: ReferenceAllocationHost + ?Sized>(
    host: &mut Host,
) -> Result<u64, Fault> {
    loop {
        let local_next = host.local_next();
        let sequence = host.allocate_sequence(local_next)?;
        if sequence == 0 || sequence > MAX_EFFECT_SEQUENCE {
            return Err(exhausted());
        }
        let next = SequenceWatermarkRequirement::after_observation(sequence, u64::MAX)
            .map_err(|_| exhausted())?
            .required_watermark();
        host.publish_next(next);
        let token = ContinuationToken::from_wire(format!("continuation-{sequence}"));
        if !host.pending_contains(&token) && !host.completed_contains(&token) {
            return Ok(sequence);
        }
    }
}

fn exhausted() -> Fault {
    Fault {
        code: "LSV1003".to_string(),
        message: "continuation sequence is exhausted".to_string(),
    }
}
