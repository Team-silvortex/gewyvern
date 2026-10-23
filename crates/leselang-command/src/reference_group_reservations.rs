//! Product successor-reservation recovery policy, without storage or allocator authority.

use leselang_runtime_core::{Fault, ReservationObservation, validate_reservation_proof};

use crate::{MergePlan, reference_merge::invalid_group_binding};

/// Read original observations from the same snapshot as the admitted group graph.
/// The watermark must precede recovery repair/import, not be inferred from rows.
/// Hooks must not mutate storage, allocate, dispatch or publish a checkpoint.
/// Successful inspection is correspondence only, not proof of a trusted owner.
pub trait ReferenceReservationSource {
    fn allocation_watermark(&mut self) -> Result<i64, Fault>;
    fn effect_exists(&mut self, token: &str) -> Result<bool, Fault>;
    fn group_exists(&mut self, token: &str) -> Result<bool, Fault>;
}

/// Inspect reservations of an already bounded, admitted plan and request graph.
/// Preserve watermark -> effect -> group -> shape -> proof ordering, including
/// reads before refusal of a bad watermark. No reservation is repaired or created.
/// Cross-group uniqueness and complete receipt validation remain separate gates.
pub fn inspect_group_reservations<Source: ReferenceReservationSource + ?Sized>(
    plan: &MergePlan,
    source: &mut Source,
) -> Result<(), Fault> {
    let Some(binding) = &plan.result_binding else {
        return Ok(());
    };
    if binding.successor_sequence.is_none() {
        return Ok(());
    }
    let next_sequence = source.allocation_watermark()?;
    for (index, sequence) in binding.reserved_sequences().enumerate() {
        let exists = source.effect_exists(&format!("continuation-{sequence}"))?;
        let aliases_group = source.group_exists(&format!("merge-{sequence}"))?;
        let created = plan
            .branches
            .len()
            .checked_sub(
                binding
                    .plan_branches()
                    .map_err(|_| invalid_group_binding())?
                    .len(),
            )
            .ok_or_else(invalid_group_binding)?;
        validate_reservation_proof(
            sequence,
            index < created,
            ReservationObservation {
                allocation_watermark: u64::try_from(next_sequence).ok(),
                record_exists: exists,
                aliases_owner: aliases_group,
            },
        )
        .map_err(|_| invalid_group_binding())?;
    }
    Ok(())
}
