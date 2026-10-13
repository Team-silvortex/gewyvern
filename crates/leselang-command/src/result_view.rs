//! Borrowed product reply views, separate from execution and capture admission.

use leselang_hir::{HirBranch, host_call::HostOperation, result_field::ResultField};
use leselang_runtime_core::{GroupCaptureLimits, ScalarValue};

use crate::{InvalidProjectedResult, ProjectedGroup, ProjectedGroupExt, ProjectedResult, Value};

/// Payload-free separation of a wrong view/member access and a failed projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResultViewError {
    InvalidAccess,
    InvalidProjection,
}

impl std::fmt::Display for ResultViewError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::InvalidAccess => "invalid product result view access",
            Self::InvalidProjection => "invalid product result projection",
        })
    }
}

impl std::error::Error for ResultViewError {}

impl From<InvalidProjectedResult> for ResultViewError {
    fn from(_: InvalidProjectedResult) -> Self {
        Self::InvalidProjection
    }
}

/// Read a previously admitted raw reply or saved capture without copying its owner.
///
/// Construction is not validation. Callers must bound input/storage, validate
/// complete groups and captures, and admit original operations, types, authority
/// and receipt provenance before use. In particular, raw scalar lookup does not
/// compare the operation tag, and member lookup does not certify group shape,
/// uniqueness, declared result tags or original arguments. Duplicate names retain
/// first-match behavior; complete admission must reject them before execution.
///
/// Views are borrowed data, not serializable execution handles. Reads copy only
/// selected scalars. Capture methods intentionally materialize owned projections;
/// no metering, dispatch, replay protection or implicit policy is installed.
#[derive(Clone, Copy)]
pub enum ResultView<'a> {
    Raw {
        value: &'a Value,
        operation: HostOperation,
    },
    Projected(&'a ProjectedResult),
    Group {
        value: &'a Value,
        branches: &'a [HirBranch],
    },
    ProjectedGroup(&'a ProjectedGroup),
}

impl<'a> ResultView<'a> {
    /// Copy an available scalar; never widen a saved vocabulary to current fields.
    /// Saved field lookup does not revalidate its projection or charge fuel.
    pub fn project_scalar(self, field: ResultField) -> Result<ScalarValue, ResultViewError> {
        match self {
            Self::Raw { value, .. } => value
                .project_scalar(field)
                .ok_or(ResultViewError::InvalidProjection),
            Self::Projected(result) => result.field(field).map_err(Into::into),
            Self::Group { .. } | Self::ProjectedGroup(_) => Err(ResultViewError::InvalidAccess),
        }
    }

    /// Borrow an original member selected by exact name and operation.
    /// Raw signature lookup precedes original-row lookup; no payload is cloned.
    pub fn select_member(
        self,
        name: &str,
        operation: HostOperation,
    ) -> Result<Self, ResultViewError> {
        match self {
            Self::Group {
                value: Value::Structured { fields },
                branches,
            } => {
                if !branches.iter().any(|branch| {
                    branch.name == name
                        && HostOperation::for_effect(&branch.effect) == Some(operation)
                }) {
                    return Err(ResultViewError::InvalidAccess);
                }
                let field = fields
                    .iter()
                    .find(|field| field.name == name)
                    .ok_or(ResultViewError::InvalidAccess)?;
                Ok(Self::Raw {
                    value: &field.value,
                    operation,
                })
            }
            Self::ProjectedGroup(saved) => {
                let member = saved
                    .members
                    .iter()
                    .find(|member| member.name == name && member.result.operation == operation)
                    .ok_or(ResultViewError::InvalidAccess)?;
                Ok(Self::Projected(&member.result))
            }
            _ => Err(ResultViewError::InvalidAccess),
        }
    }

    /// Project raw data, or copy saved bytes without upgrading or revalidating them.
    /// The latter requires prior capture admission, including aggregate copy bounds.
    pub fn capture_result(self) -> Result<ProjectedResult, ResultViewError> {
        match self {
            Self::Raw { value, operation } => {
                ProjectedResult::capture(operation, value).map_err(Into::into)
            }
            Self::Projected(result) => Ok(result.clone()),
            Self::Group { .. } | Self::ProjectedGroup(_) => Err(ResultViewError::InvalidAccess),
        }
    }

    /// Capture a raw group with explicit policy, or copy an already admitted group.
    ///
    /// Limits and the name callback apply only to raw materialization. Saved groups
    /// copy unchanged without validation or invoking the callback, even if a caller
    /// supplies different policy. Validate saved groups separately before entry.
    /// Raw capture retains projection-before-name-validation order. Callers bound
    /// native callback work; callback side effects are not rolled back and unwind
    /// propagates. Partial owned captures are dropped on ordinary failure/unwind.
    pub fn capture_group(
        self,
        limits: GroupCaptureLimits,
        validate_name: impl FnMut(&str) -> bool,
    ) -> Result<ProjectedGroup, ResultViewError> {
        match self {
            Self::Group { value, branches } => {
                ProjectedGroup::capture(branches, value, limits, validate_name).map_err(Into::into)
            }
            Self::ProjectedGroup(group) => Ok(group.clone()),
            _ => Err(ResultViewError::InvalidAccess),
        }
    }
}

impl std::fmt::Debug for ResultView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Raw { .. } => "RawResultView",
            Self::Projected(_) => "ProjectedResultView",
            Self::Group { .. } => "RawGroupView",
            Self::ProjectedGroup(_) => "ProjectedGroupView",
        })
    }
}
