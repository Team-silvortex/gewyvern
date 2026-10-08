//! Fresh closed native export assembly, not a cached admission or execution grant.

use std::fmt;

use crate::group_exports::{GroupExport, GroupExports};
use crate::ir::GroupKind;
use crate::native_group::{NativeGroupMemberError, preflight_native_group_members};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeGroupExportPhase {
    Operation { member_index: usize },
    Signature,
}

/// Cold/member and native observation failures retain positions and original
/// opaque errors without formatting payloads or exposing private source chains.
///
/// ```compile_fail
/// use leselang_hir::native_group_exports::NativeGroupExportError;
/// use leselang_hir::native_group::NativeGroupMemberError;
/// let error = NativeGroupExportError::<()>::Members(NativeGroupMemberError::Arity);
/// let wire = serde_json::to_string(&error).unwrap();
/// ```
pub enum NativeGroupExportError<Error> {
    Members(NativeGroupMemberError<Error>),
    Native {
        phase: NativeGroupExportPhase,
        error: Error,
    },
}
impl<Error> fmt::Display for NativeGroupExportError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Members(_) => "native group export member preflight failed",
            Self::Native { .. } => "native group export observation or signature admission failed",
        })
    }
}
impl<Error> fmt::Debug for NativeGroupExportError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}
impl<Error> std::error::Error for NativeGroupExportError<Error> {}

/// Assemble one fresh ordered native signature. Explicit current arity/label
/// limits and ALL once-borrowed names precede native operation observation.
/// Each original member then produces one opaque observation in declaration
/// order, with no native Clone/Debug/PartialEq/serde/Send bounds or name copying.
/// The bounded vector contains export metadata, never a converted native graph.
/// No prior NativeGroupMembers view or cached grant is accepted by this entry.
///
/// The final FnOnce admission is mandatory and sees the original mode/member
/// slice and complete ordered candidate. It must corroborate original names,
/// operation/result/schema identity, flat eligibility and current live policy
/// after the observations; those native facts are not inferred by the core.
/// Native graph/payload bounds and later type/canonical/authority admission stay
/// separate. Adapters must supply original slices/names; hooks, payload allocation,
/// interior mutation and destructors remain trusted, not sandboxed or preempted.
///
/// No partially observed exports escape on failure/unwind, and hooks never retry.
/// Completed owned observations are released normally; the borrowed native graph
/// is not consumed. Drop may itself unwind; external work is not rolled back.
/// Success returns mutable ephemeral GroupExports, not a reusable identity/type
/// certificate, receipt, fuel reservation, dispatcher or durable continuation.
/// This neither unions incompatible signatures nor enables nested graph exports.
///
/// ```compile_fail
/// use leselang_hir::{ir::GroupKind, native_group_exports::observe_native_group_exports};
/// let members = [()];
/// let output = observe_native_group_exports(GroupKind::Sequence, &members, 1,
///     |_| -> Result<_, ()> { Ok("member") }, |_| Ok(()));
/// ```
pub fn observe_native_group_exports<'group, Branch, Operation, Error>(
    kind: GroupKind,
    members: &'group [Branch],
    max_members: usize,
    name: impl FnMut(&'group Branch) -> Result<&'group str, Error>,
    mut operation: impl FnMut(&'group Branch) -> Result<Operation, Error>,
    admit: impl FnOnce(
        GroupKind,
        &'group [Branch],
        &[GroupExport<'group, Operation>],
    ) -> Result<(), Error>,
) -> Result<GroupExports<'group, Operation>, NativeGroupExportError<Error>> {
    let observed = preflight_native_group_members(kind, members, max_members, name)
        .map_err(NativeGroupExportError::Members)?;
    let mut exports = Vec::with_capacity(members.len());
    for (member_index, member) in members.iter().enumerate() {
        let name = observed
            .name(member_index)
            .ok_or(NativeGroupExportError::Members(
                NativeGroupMemberError::InvalidName { member_index },
            ))?;
        let operation = operation(member).map_err(|error| NativeGroupExportError::Native {
            phase: NativeGroupExportPhase::Operation { member_index },
            error,
        })?;
        exports.push(GroupExport { name, operation });
    }
    admit(observed.kind(), observed.members(), &exports).map_err(|error| {
        NativeGroupExportError::Native {
            phase: NativeGroupExportPhase::Signature,
            error,
        }
    })?;
    Ok(GroupExports {
        kind: observed.kind(),
        members: exports,
    })
}
