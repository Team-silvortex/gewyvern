//! Closed native member admission, not graph typing or execution authority.

use std::fmt;

use crate::ir::GroupKind;
use crate::native_graph::MAX_NATIVE_GRAPH_MEMBERS;
use crate::pure_typing::valid_member_name;

/// Borrowed original members and once-observed names. This is ephemeral metadata,
/// not an authentic operation row, result value, live grant or graph certificate.
/// Native admission remains trusted and must be repeated under changed policy.
/// No native slots are owned, copied, formatted or serialized by this view.
///
/// ```compile_fail
/// use leselang_hir::{ir::GroupKind, native_group::admit_native_group_members};
/// let members = [()];
/// let view = admit_native_group_members(GroupKind::Sequence, &members, 1,
///     |_| -> Result<_, ()> { Ok("member") }, |_| Ok(())).unwrap();
/// let wire = serde_json::to_string(&view).unwrap();
/// ```
#[must_use = "borrowed member metadata is not current native execution authority"]
pub struct NativeGroupMembers<'group, Branch> {
    kind: GroupKind,
    members: &'group [Branch],
    names: [Option<&'group str>; MAX_NATIVE_GRAPH_MEMBERS],
}
impl<'group, Branch> NativeGroupMembers<'group, Branch> {
    pub fn kind(&self) -> GroupKind {
        self.kind
    }

    pub fn members(&self) -> &'group [Branch] {
        self.members
    }

    pub fn name(&self, index: usize) -> Option<&'group str> {
        self.names.get(index).copied().flatten()
    }
}
impl<Branch> fmt::Debug for NativeGroupMembers<'_, Branch> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NativeGroupMembers")
            .field("kind", &self.kind)
            .field("members", &self.members.len())
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeGroupMemberPhase {
    Name,
    Admission,
}

/// Original member positions and phases remain matchable; native failures are
/// never formatted or exposed through Error::source. No native error traits apply.
///
/// ```compile_fail
/// use leselang_hir::native_group::NativeGroupMemberError;
/// let wire = serde_json::to_string(&NativeGroupMemberError::<()>::Arity).unwrap();
/// ```
pub enum NativeGroupMemberError<Error> {
    InvalidLimit,
    Arity,
    InvalidName {
        member_index: usize,
    },
    DuplicateName {
        member_index: usize,
    },
    Native {
        member_index: usize,
        phase: NativeGroupMemberPhase,
        error: Error,
    },
}
impl<Error> fmt::Display for NativeGroupMemberError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidLimit => "native group member limit exceeds its safety ceiling",
            Self::Arity => "native group has invalid member count",
            Self::InvalidName { .. } => "native group has invalid member name",
            Self::DuplicateName { .. } => "native group has duplicate member name",
            Self::Native { .. } => "native group member observation failed",
        })
    }
}
impl<Error> fmt::Debug for NativeGroupMemberError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}
impl<Error> std::error::Error for NativeGroupMemberError<Error> {}

/// Admit one supplied closed native member slice. Sequence needs at least one
/// member and Parallel two. max_members is inclusive, capped at 64, and zero
/// denies all groups before hooks. All original names are observed once in order
/// and checked before ANY native member admission. Names retain the shared ASCII
/// member-label grammar, not lexical-variable grammar; no buffers are copied.
/// A fixed bounded name table avoids a heap collection or repeated name queries.
/// Each original branch is then admitted once in declaration order. No partially
/// admitted view is returned on error or unwind, and no hook is retried.
///
/// The adapter must supply the original slice and name references. Its admission
/// must corroborate the original payload/operation/declaration and flat atomic
/// eligibility; this core does not infer those facts, select schemas or flatten
/// nested graphs. Complete graph bounds, native payload limits and current live
/// policy remain separate gates (e.g. native_graph::inspect_native_graph). Names
/// and callbacks may have interior state: authenticity, work/allocation, Drop,
/// side effects and unwind remain trusted native policy, not an in-process sandbox.
/// This view does not join types/exports, dispatch effects, accept replies, spend
/// fuel or suspend execution. External native work is not rolled back on failure.
pub fn admit_native_group_members<'group, Branch, Error>(
    kind: GroupKind,
    members: &'group [Branch],
    max_members: usize,
    name: impl FnMut(&'group Branch) -> Result<&'group str, Error>,
    mut admit: impl FnMut(&'group Branch) -> Result<(), Error>,
) -> Result<NativeGroupMembers<'group, Branch>, NativeGroupMemberError<Error>> {
    let observed = preflight_native_group_members(kind, members, max_members, name)?;
    for (member_index, member) in members.iter().enumerate() {
        admit(member).map_err(|error| NativeGroupMemberError::Native {
            member_index,
            phase: NativeGroupMemberPhase::Admission,
            error,
        })?;
    }
    Ok(observed)
}

// Shared public entries finish this complete cold pass before native observations.
pub(crate) fn preflight_native_group_members<'group, Branch, Error>(
    kind: GroupKind,
    members: &'group [Branch],
    max_members: usize,
    mut name: impl FnMut(&'group Branch) -> Result<&'group str, Error>,
) -> Result<NativeGroupMembers<'group, Branch>, NativeGroupMemberError<Error>> {
    if max_members > MAX_NATIVE_GRAPH_MEMBERS {
        return Err(NativeGroupMemberError::InvalidLimit);
    }
    let minimum = match kind {
        GroupKind::Sequence => 1,
        GroupKind::Parallel => 2,
    };
    if !(minimum..=max_members).contains(&members.len()) {
        return Err(NativeGroupMemberError::Arity);
    }
    let mut names = [None; MAX_NATIVE_GRAPH_MEMBERS];
    for (member_index, member) in members.iter().enumerate() {
        let observed = name(member).map_err(|error| NativeGroupMemberError::Native {
            member_index,
            phase: NativeGroupMemberPhase::Name,
            error,
        })?;
        if !valid_member_name(observed) {
            return Err(NativeGroupMemberError::InvalidName { member_index });
        }
        if names[..member_index].contains(&Some(observed)) {
            return Err(NativeGroupMemberError::DuplicateName { member_index });
        }
        names[member_index] = Some(observed);
    }
    Ok(NativeGroupMembers {
        kind,
        members,
        names,
    })
}
