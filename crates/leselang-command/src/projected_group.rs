//! Ordered product group captures over the core's unchanged generic containers.

use leselang_runtime_core::{GroupCaptureLimits, validate_projected_group};
use leserpent_hir::{HirBranch, host_call::HostOperation};

use crate::{InvalidProjectedResult, ProjectedResult, StructuredField, Value};

pub type ProjectedBinding = leselang_runtime_core::ProjectedBinding<ProjectedResult>;
pub type ProjectedGroup = leselang_runtime_core::ProjectedGroup<ProjectedResult>;
pub type ProjectedGroupBinding = leselang_runtime_core::ProjectedGroupBinding<ProjectedGroup>;

/// Product operations on exact core containers, not an execution or replay grant.
///
/// Callers supply member bounds and trusted name policy, and bound raw ingress,
/// aggregate payloads and callback work. No runtime defaults, metering, original
/// authority or journal ownership are inferred. Name callbacks are trusted native
/// code; side effects and unwinding are not rolled back by these data helpers.
pub trait ProjectedGroupExt: Sized {
    /// Preserve source shape/name/operation checks before materializing captures,
    /// then validate the stored group. Raw-result type admission is caller-owned.
    fn capture(
        branches: &[HirBranch],
        value: &Value,
        limits: GroupCaptureLimits,
        validate_name: impl FnMut(&str) -> bool,
    ) -> Result<Self, InvalidProjectedResult>;

    /// Check member bounds, ordered name/uniqueness rules, then result captures.
    fn validate(
        &self,
        limits: GroupCaptureLimits,
        validate_name: impl FnMut(&str) -> bool,
    ) -> Result<(), InvalidProjectedResult>;

    /// Compare ordered names and effect operation tags only, without validation.
    /// Neither original arguments nor the branch's declared result tag are checked.
    fn matches_signature(&self, branches: &[HirBranch]) -> bool;

    /// Validate saved data first, then compare the signature and stored fields.
    /// Matching exported fields does not authenticate a raw host result.
    fn matches_members(
        &self,
        branches: &[HirBranch],
        fields: &[StructuredField],
        limits: GroupCaptureLimits,
        validate_name: impl FnMut(&str) -> bool,
    ) -> Result<bool, InvalidProjectedResult>;
}

impl ProjectedGroupExt for ProjectedGroup {
    fn capture(
        branches: &[HirBranch],
        value: &Value,
        limits: GroupCaptureLimits,
        validate_name: impl FnMut(&str) -> bool,
    ) -> Result<Self, InvalidProjectedResult> {
        let Value::Structured { fields } = value else {
            return Err(InvalidProjectedResult);
        };
        if branches.len() != fields.len() {
            return Err(InvalidProjectedResult);
        }
        let members = branches
            .iter()
            .zip(fields)
            .map(|(branch, field)| {
                if branch.name != field.name {
                    return Err(InvalidProjectedResult);
                }
                let operation =
                    HostOperation::for_effect(&branch.effect).ok_or(InvalidProjectedResult)?;
                Ok(ProjectedBinding {
                    name: branch.name.clone(),
                    result: ProjectedResult::capture(operation, &field.value)?,
                })
            })
            .collect::<Result<Vec<_>, InvalidProjectedResult>>()?;
        let group = Self { members };
        group.validate(limits, validate_name)?;
        Ok(group)
    }

    fn validate(
        &self,
        limits: GroupCaptureLimits,
        validate_name: impl FnMut(&str) -> bool,
    ) -> Result<(), InvalidProjectedResult> {
        validate_projected_group(self, limits, validate_name, ProjectedResult::validate)
            .map_err(|_| InvalidProjectedResult)
    }

    fn matches_signature(&self, branches: &[HirBranch]) -> bool {
        self.members.len() == branches.len()
            && self.members.iter().zip(branches).all(|(member, branch)| {
                member.name == branch.name
                    && HostOperation::for_effect(&branch.effect) == Some(member.result.operation)
            })
    }

    fn matches_members(
        &self,
        branches: &[HirBranch],
        fields: &[StructuredField],
        limits: GroupCaptureLimits,
        validate_name: impl FnMut(&str) -> bool,
    ) -> Result<bool, InvalidProjectedResult> {
        self.validate(limits, validate_name)?;
        if !self.matches_signature(branches) || fields.len() != branches.len() {
            return Ok(false);
        }
        for ((member, branch), field) in self.members.iter().zip(branches).zip(fields) {
            if field.name != branch.name
                || !member
                    .result
                    .matches_capture(member.result.operation, &field.value)?
            {
                return Ok(false);
            }
        }
        Ok(true)
    }
}
