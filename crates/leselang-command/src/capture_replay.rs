//! Product capture comparisons, without evaluation or durable replay authority.

use leselang_runtime_core::{
    CaptureCorrespondenceError, CaptureCorrespondenceLimits, CaptureCorrespondenceSite,
    CaptureExtensionBindings, CaptureExtensionDomain, CaptureExtensionError, CurrentCapture,
    GroupCaptureLimits, validate_capture_correspondence, validate_capture_extension,
};
use leserpent_hir::{Effect, HirBranch, host_call::HostOperation};

use crate::{
    InvalidProjectedResult, ProjectedBinding, ProjectedGroupBinding, ProjectedGroupExt,
    ResultBinding, StructuredField, Value,
};

/// Payload-free categories; callers retain their context-specific fault codes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CaptureReplayError {
    /// Unsupported input shape, policy limit or capture correspondence mismatch.
    Mismatch,
    /// A saved capture or required raw field failed projection validation.
    InvalidProjection,
}

impl std::fmt::Display for CaptureReplayError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Mismatch => "product capture correspondence mismatch",
            Self::InvalidProjection => "invalid product capture projection",
        })
    }
}

impl std::error::Error for CaptureReplayError {}

impl From<InvalidProjectedResult> for CaptureReplayError {
    fn from(_: InvalidProjectedResult) -> Self {
        Self::InvalidProjection
    }
}

struct ProductCaptureExtension<'a> {
    prior: &'a ResultBinding,
    effect: &'a Effect,
    value: &'a Value,
    operation: Option<HostOperation>,
}

impl CaptureExtensionDomain<ProjectedGroupBinding, ProjectedBinding>
    for ProductCaptureExtension<'_>
{
    type Error = CaptureReplayError;

    fn same_group_data(
        &mut self,
        old: &ProjectedGroupBinding,
        saved: &ProjectedGroupBinding,
    ) -> bool {
        old.group == saved.group
    }

    fn same_group_binding(
        &mut self,
        old: &ProjectedGroupBinding,
        saved: &ProjectedGroupBinding,
    ) -> bool {
        old == saved
    }

    fn begin_results(&mut self) -> Result<(), Self::Error> {
        self.operation =
            Some(HostOperation::for_effect(self.effect).ok_or(CaptureReplayError::Mismatch)?);
        Ok(())
    }

    fn current_result(&mut self, saved: &ProjectedBinding) -> Result<CurrentCapture, Self::Error> {
        // Replay comparisons never synthesize fields absent from a legacy capture.
        let matches_result = saved.result.matches_capture(
            self.operation.ok_or(CaptureReplayError::Mismatch)?,
            self.value,
        )?;
        Ok(CurrentCapture {
            matches_result,
            matches_binding: saved.name == self.prior.name,
        })
    }

    fn same_result_data(&mut self, old: &ProjectedBinding, saved: &ProjectedBinding) -> bool {
        old.result == saved.result
    }

    fn same_result_binding(&mut self, old: &ProjectedBinding, saved: &ProjectedBinding) -> bool {
        old == saved
    }
}

/// Check preserved named captures and the current named result without copying frames.
///
/// Bounds precede group retention, then operation lookup and saved-result checks.
/// Aliases may share data, but cannot replace an original named binding or the
/// current result's original name. Later malformed results are not hidden by an
/// earlier match. The shared core also enforces its inclusive safety ceiling.
/// Raw-field projection retains its existing selected-scalar copies.
///
/// Callers must first admit both frames and the raw value. This deliberately does
/// not validate scalar locals, bodies, original effect arguments, native reply
/// identity, schema succession, authentic receipts, authority or durable state.
/// It neither executes code nor allocates fuel, writes storage or grants replay.
pub fn validate_result_binding_extension(
    prior: &ResultBinding,
    saved: &ResultBinding,
    effect: &Effect,
    value: &Value,
    max_saved_bindings: usize,
) -> Result<(), CaptureReplayError> {
    validate_capture_extension(
        CaptureExtensionBindings {
            groups: &prior.groups,
            results: &prior.results,
        },
        CaptureExtensionBindings {
            groups: &saved.groups,
            results: &saved.results,
        },
        max_saved_bindings,
        ProductCaptureExtension {
            prior,
            effect,
            value,
            operation: None,
        },
    )
    .map_err(|error| match error {
        CaptureExtensionError::Native { error, .. } => error,
        _ => CaptureReplayError::Mismatch,
    })
}

/// Compare saved groups/results to an original ordered result prefix.
///
/// Row shape and explicit aggregate limits precede all name/projection hooks.
/// Groups validate before signature/field matching, then every saved result scans
/// every original row, including rows after a match. Result aliases are compared
/// by stored fields, not by row name. Legacy field sets and error order stay closed.
/// Frames and rows stay borrowed; field projection retains selected-scalar copies
/// and group validation may allocate scratch space.
///
/// This is not frame validation, raw-result admission or proof of original branch
/// arguments. Callers bound input/callback work and own authentic identities,
/// receipt provenance, authority and same-snapshot durable recovery. Name callbacks
/// are trusted; their side effects are not rolled back and unwinding propagates.
pub fn validate_group_capture_correspondence(
    saved: &ResultBinding,
    branches: &[HirBranch],
    fields: &[StructuredField],
    operations: &[HostOperation],
    limits: CaptureCorrespondenceLimits,
    group_limits: GroupCaptureLimits,
    mut validate_group_name: impl FnMut(&str) -> bool,
) -> Result<(), CaptureReplayError> {
    if fields.len() < branches.len() || fields.len() != operations.len() {
        return Err(CaptureReplayError::Mismatch);
    }
    validate_capture_correspondence(
        &saved.groups,
        &saved.results,
        fields,
        limits,
        |_, group| {
            group
                .group
                .matches_members(
                    branches,
                    &fields[..branches.len()],
                    group_limits,
                    &mut validate_group_name,
                )
                .map_err(CaptureReplayError::from)
        },
        |site, result, field| {
            let CaptureCorrespondenceSite::Result { original, .. } = site else {
                return Err(CaptureReplayError::Mismatch);
            };
            result
                .result
                .matches_capture(operations[original], &field.value)
                .map_err(CaptureReplayError::from)
        },
    )
    .map_err(|error| match error {
        CaptureCorrespondenceError::Native { error, .. } => error,
        _ => CaptureReplayError::Mismatch,
    })
}
