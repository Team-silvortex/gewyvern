//! Released member/capture correspondence policy, without execution or storage authority.

use leselang_runtime_core::{CaptureCorrespondenceLimits, Fault};
use leserpent_hir::host_call::HostOperation;

use crate::reference_merge::invalid_group_binding as invalid;
use crate::reference_request::{continuation_admission_fault, result_binding_limits};
use crate::{
    CaptureReplayError, ContinuationAdmissionError, EffectRequest, GroupResultBinding,
    MAX_MERGE_BRANCHES, StructuredField, Value,
};

/// Check a member's type using the original successor-label precedence. This is
/// not value admission: callers first bound/admit the plan and raw value. Saved
/// bodies are only statically typed, never evaluated; matching types authenticate
/// neither a reply nor its originating request. Recheck after mutable data changes.
pub fn validate_member(group: &GroupResultBinding, name: &str, value: &Value) -> Result<(), Fault> {
    if let Some(index) = (0..group.reserved_sequences().count()).find(|index| {
        group
            .plan_successor_name_at(*index, MAX_MERGE_BRANCHES)
            .is_ok_and(|candidate| candidate == name)
    }) {
        let accepted = if group.captures_successor() {
            group.preflighted_capture_accepts_type_at(index, value.result_type())
        } else {
            value.result_type()
                == crate::validate_result_binding(
                    &group.binding,
                    &group.pending,
                    result_binding_limits(),
                    valid_merge_branch_name,
                )
                .map_err(|_| invalid_projection())?
        };
        return if accepted { Ok(()) } else { Err(invalid()) };
    }
    let branch = group
        .plan_branches()
        .map_err(|_| invalid())?
        .iter()
        .find(|branch| branch.name == name)
        .ok_or_else(invalid)?;
    if value.result_type() != branch.result_type {
        return Err(invalid());
    }
    Ok(())
}

/// Compare the original saved frame with an ordered capture prefix. Preserve
/// branches -> row shape -> saved-frame presence -> groups -> results admission,
/// including LSV1405 projection errors versus LSV1409 correspondence errors.
/// Inputs remain borrowed and unchanged. Owners must first bound/admit the plan,
/// request and raw values, and retain same-snapshot provenance and authority;
/// this check cannot grant re-entry, repair, settlement or checkpoint publication.
pub fn validate_saved_capture(
    group: &GroupResultBinding,
    request: &EffectRequest,
    fields: &[StructuredField],
    operations: &[HostOperation],
) -> Result<(), Fault> {
    let branches = group.plan_branches().map_err(|_| invalid())?;
    if fields.len() < branches.len() || fields.len() != operations.len() {
        return Err(invalid());
    }
    let binding = request
        .continuation
        .result_binding
        .as_ref()
        .ok_or_else(invalid)?;
    crate::validate_group_capture_correspondence(
        binding,
        branches,
        fields,
        operations,
        CaptureCorrespondenceLimits {
            max_saved_bindings: leserpent_hir::MAX_EFFECT_NESTING_DEPTH,
            max_original_rows: MAX_MERGE_BRANCHES,
        },
        result_binding_limits().groups,
        valid_merge_branch_name,
    )
    .map_err(|error| match error {
        CaptureReplayError::InvalidProjection => invalid_projection(),
        CaptureReplayError::Mismatch => invalid(),
    })
}

fn invalid_projection() -> Fault {
    continuation_admission_fault(ContinuationAdmissionError::Binding)
}

fn valid_merge_branch_name(name: &str) -> bool {
    crate::valid_merge_branch_name(name, crate::MAX_MERGE_BRANCH_NAME_BYTES)
}
