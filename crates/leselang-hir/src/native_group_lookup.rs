//! Fresh closed member lookup, not native schema or execution authority.

use std::fmt;

use crate::ir::GroupKind;
use crate::native_graph::MAX_NATIVE_GRAPH_MEMBERS;
use crate::native_group::{NativeGroupMemberError, preflight_native_group_members};
use crate::pure_typing::valid_member_name;

/// Query/member positions and original opaque errors remain matchable. Debug,
/// Display and Error::source never expose name buffers or native payloads.
///
/// ```compile_fail
/// use leselang_hir::native_group_lookup::NativeGroupLookupError;
/// let error = NativeGroupLookupError::<()>::InvalidQuery;
/// let wire = serde_json::to_string(&error).unwrap();
/// ```
pub enum NativeGroupLookupError<Error> {
    InvalidQuery,
    Members(NativeGroupMemberError<Error>),
    Native { member_index: usize, error: Error },
}
impl<Error> fmt::Display for NativeGroupLookupError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidQuery => "native group member query has an invalid name",
            Self::Members(_) => "native group member query preflight failed",
            Self::Native { .. } => "native selected member admission failed",
        })
    }
}
impl<Error> fmt::Debug for NativeGroupLookupError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}
impl<Error> std::error::Error for NativeGroupLookupError<Error> {}

/// Look up one original member of a closed group after fresh cold preflight.
/// An invalid ceiling precedes query validation; an invalid query precedes every
/// name hook. Current inclusive arity and ALL once-borrowed original names then
/// precede selected native admission. Sequence needs one member, Parallel two;
/// the explicit maximum is capped at 64 and zero denies groups before hooks.
/// Query names use the same bounded ASCII member-label grammar as group names.
///
/// The fixed borrowed name table checks the complete group, including invalid or
/// duplicate tails after an otherwise matching member. Missing names return None
/// without native admission. One matching original slot reaches the mandatory
/// FnOnce native hook; false also returns None, error retains its member index.
/// Success returns that same borrowed member, never a copied or converted graph.
/// No native Clone/Debug/equality/serde/Send bounds or heap collection apply.
/// No cached admitted view, implicit operation equality or fallback lookup exists.
///
/// Adapters must expose original names and corroborate the selected operation,
/// result/schema identity and relevant current policy against the actual query.
/// Complete native graph/payload bounds and other rows remain separate gates;
/// this entry neither infers atomic eligibility nor admits the whole native graph.
/// Hooks, interior mutation, work/allocation and Drop remain trusted native policy,
/// not sandboxed or preempted. Failure/unwind stops without retry or consumption of
/// borrowed slots; callback/cleanup effects cannot be rolled back. Captured native
/// hook state is dropped normally even when there is no match and no invocation.
///
/// A returned reference keeps a borrow, not a reusable type/grant/receipt/fuel or
/// execution certificate. Interior state or later policy can invalidate it; fresh
/// relevant admission is required before use. No native result values are read,
/// replies accepted, operations dispatched or nested type profiles promoted.
///
/// ```compile_fail
/// use leselang_hir::{ir::GroupKind, native_group_lookup::lookup_native_group_member};
/// let members = [()];
/// let selected = lookup_native_group_member(GroupKind::Sequence, &members, 1,
///     "member", |_| -> Result<_, ()> { Ok("member") });
/// ```
pub fn lookup_native_group_member<'group, Member, Error>(
    kind: GroupKind,
    members: &'group [Member],
    max_members: usize,
    query_name: &str,
    name: impl FnMut(&'group Member) -> Result<&'group str, Error>,
    admit: impl FnOnce(&'group Member) -> Result<bool, Error>,
) -> Result<Option<&'group Member>, NativeGroupLookupError<Error>> {
    if max_members > MAX_NATIVE_GRAPH_MEMBERS {
        return Err(NativeGroupLookupError::Members(
            NativeGroupMemberError::InvalidLimit,
        ));
    }
    if !valid_member_name(query_name) {
        return Err(NativeGroupLookupError::InvalidQuery);
    }
    let observed = preflight_native_group_members(kind, members, max_members, name)
        .map_err(NativeGroupLookupError::Members)?;
    for (member_index, member) in observed.members().iter().enumerate() {
        if observed.name(member_index) == Some(query_name) {
            return admit(member)
                .map(|accepted| accepted.then_some(member))
                .map_err(|error| NativeGroupLookupError::Native {
                    member_index,
                    error,
                });
        }
    }
    Ok(None)
}
