use std::{collections::HashSet, fmt};

use crate::{CapturedBindings, ProjectedGroup};

/// Explicit adapter policy; zero limits are allowed and never silently repaired.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CaptureValidationLimits {
    pub max_saved_bindings: usize,
    pub max_name_bytes: usize,
}

/// Inclusive member bounds. An inverted interval rejects every group.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GroupCaptureLimits {
    pub min_members: usize,
    pub max_members: usize,
}

/// Original position of a trusted native validation callback, without payloads.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CaptureValidationSite {
    Result { index: usize },
    Group { index: usize },
    Member { index: usize },
    Body,
}

/// Structural rejection or an unchanged, opaque adapter-owned validation error.
/// Diagnostics never format native errors, names or captured values. Pattern
/// matching exposes the original error; hosts own its diagnostics and lifetime.
#[derive(Eq, PartialEq)]
pub enum CaptureStructureError<External> {
    BindingCount,
    PendingName,
    ScalarName {
        index: usize,
    },
    ScalarValue {
        index: usize,
    },
    ResultName {
        index: usize,
    },
    GroupName {
        index: usize,
    },
    MemberCount,
    MemberName {
        index: usize,
    },
    DuplicateMember {
        index: usize,
    },
    Native {
        site: CaptureValidationSite,
        error: External,
    },
}

impl<External> fmt::Display for CaptureStructureError<External> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::BindingCount => "captured binding count exceeds policy",
            Self::PendingName => "pending capture name exceeds byte policy",
            Self::ScalarName { .. } => "captured scalar name exceeds byte policy",
            Self::ScalarValue { .. } => "captured scalar exceeds language bounds",
            Self::ResultName { .. } => "captured result name exceeds byte policy",
            Self::GroupName { .. } => "captured group name exceeds byte policy",
            Self::MemberCount => "captured group member count violates policy",
            Self::MemberName { .. } => "captured group member name rejected",
            Self::DuplicateMember { .. } => "captured group member name is duplicated",
            Self::Native { .. } => "native capture validation rejected",
        })
    }
}

impl<External> fmt::Debug for CaptureStructureError<External> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Native { site, .. } => f
                .debug_struct("Native")
                .field("site", site)
                .finish_non_exhaustive(),
            _ => fmt::Display::fmt(self, f),
        }
    }
}

impl<External> std::error::Error for CaptureStructureError<External> {}

/// Borrow-check saved count, UTF-8 name byte lengths and scalar bounds, then
/// validate opaque original result/group/body data through explicit native hooks.
///
/// Order is count, pending name, all scalar rows, result rows, group rows, body.
/// Within each result/group row its name precedes its once-only native callback;
/// a later invalid row does not undo earlier native validation. Scalar checks and
/// count/name refusal before the first native row call no native hook. Body runs
/// at most once and only after all saved rows pass. The pending name does not
/// count as a saved binding. Names are not normalized or syntax/scope checked.
///
/// Success describes borrowed data at this instant, not a validated handle,
/// schema/version/authority grant, total ingress bound or permission to resume.
/// Adapters still validate full lexical scope, original program/receipt identity,
/// schema, native resource domains and durable replay. There is no fuel refill,
/// body execution, serialization, cloning, normalization or hidden dispatch.
/// No native Clone/Debug/serde/Default/Send/Sync bound is imposed. Callbacks and
/// Drop are trusted, non-preemptible and may use interior mutability; native side
/// effects are not rolled back on rejection or unwind. Unwinding propagates.
pub fn validate_captured_bindings<Body, ResultData, Group, External>(
    frame: &CapturedBindings<Body, ResultData, Group>,
    limits: CaptureValidationLimits,
    mut validate_result: impl FnMut(&ResultData) -> Result<(), External>,
    mut validate_group: impl FnMut(&Group) -> Result<(), External>,
    validate_body: impl FnOnce(&Body) -> Result<(), External>,
) -> Result<(), CaptureStructureError<External>> {
    let count = frame
        .locals
        .len()
        .checked_add(frame.results.len())
        .and_then(|n| n.checked_add(frame.groups.len()));
    if count.is_none_or(|count| count > limits.max_saved_bindings) {
        return Err(CaptureStructureError::BindingCount);
    }
    if frame.name.len() > limits.max_name_bytes {
        return Err(CaptureStructureError::PendingName);
    }
    for (index, local) in frame.locals.iter().enumerate() {
        if local.name.len() > limits.max_name_bytes {
            return Err(CaptureStructureError::ScalarName { index });
        }
        if !local.value.is_bounded() {
            return Err(CaptureStructureError::ScalarValue { index });
        }
    }
    for (index, result) in frame.results.iter().enumerate() {
        if result.name.len() > limits.max_name_bytes {
            return Err(CaptureStructureError::ResultName { index });
        }
        validate_result(&result.result).map_err(|error| CaptureStructureError::Native {
            site: CaptureValidationSite::Result { index },
            error,
        })?;
    }
    for (index, group) in frame.groups.iter().enumerate() {
        if group.name.len() > limits.max_name_bytes {
            return Err(CaptureStructureError::GroupName { index });
        }
        validate_group(&group.group).map_err(|error| CaptureStructureError::Native {
            site: CaptureValidationSite::Group { index },
            error,
        })?;
    }
    validate_body(&frame.body).map_err(|error| CaptureStructureError::Native {
        site: CaptureValidationSite::Body,
        error,
    })
}

/// Borrow-check an explicitly bounded group, all native name predicates and
/// exact case-sensitive name uniqueness before any once-ordered result hook.
///
/// Count refusal calls no predicate. Names are checked in order; invalid syntax
/// precedes duplicate detection at that row. Only complete name admission permits
/// result validation, which stops at its first native error. The temporary set
/// holds borrowed original names, not copied buffers; at most `max_members` rows
/// are visited after count admission. Name hashing/equality use original strings.
/// Predicate/result callbacks are trusted native work and must be bounded by the
/// host; callbacks/interior mutations are not rolled back and unwind propagates.
/// This checks data, not original group signatures, authentic receipts, aggregate
/// native storage, execution authority or durable ownership. Nothing is reordered,
/// normalized, truncated, cloned, serialized, dispatched or replayed.
pub fn validate_projected_group<ResultData, External>(
    group: &ProjectedGroup<ResultData>,
    limits: GroupCaptureLimits,
    mut validate_name: impl FnMut(&str) -> bool,
    mut validate_result: impl FnMut(&ResultData) -> Result<(), External>,
) -> Result<(), CaptureStructureError<External>> {
    if group.members.len() < limits.min_members || group.members.len() > limits.max_members {
        return Err(CaptureStructureError::MemberCount);
    }
    let mut names = HashSet::new();
    for (index, member) in group.members.iter().enumerate() {
        if !validate_name(&member.name) {
            return Err(CaptureStructureError::MemberName { index });
        }
        if !names.insert(member.name.as_str()) {
            return Err(CaptureStructureError::DuplicateMember { index });
        }
    }
    for (index, member) in group.members.iter().enumerate() {
        validate_result(&member.result).map_err(|error| CaptureStructureError::Native {
            site: CaptureValidationSite::Member { index },
            error,
        })?;
    }
    Ok(())
}
