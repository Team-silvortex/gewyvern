//! Complete move-only preparation while a caller-owned lexical scope is live.

use std::fmt;

use leselang_runtime_core::Fuel;

use crate::ir::GroupKind;
use crate::native_group::{NativeGroupMemberError, NativeGroupMembers, admit_native_group_members};

/// Original member position/reference plus one opaque native preparation.
/// The core constructs these links; they confer no dispatch or reply authority.
#[must_use = "route the original prepared member or deliberately discard it"]
pub struct PreparedNativeMember<'group, Branch, Prepared> {
    index: usize,
    original: &'group Branch,
    prepared: Prepared,
}
impl<'group, Branch, Prepared> PreparedNativeMember<'group, Branch, Prepared> {
    pub fn index(&self) -> usize {
        self.index
    }
    pub fn original(&self) -> &'group Branch {
        self.original
    }
    pub fn prepared(&self) -> &Prepared {
        &self.prepared
    }
    pub fn into_parts(self) -> (usize, &'group Branch, Prepared) {
        (self.index, self.original, self.prepared)
    }
}
impl<Branch, Prepared> fmt::Debug for PreparedNativeMember<'_, Branch, Prepared> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PreparedNativeMember")
            .field("index", &self.index)
            .finish()
    }
}

/// Complete declaration-ordered preparation, never a partial dispatch batch.
/// Borrowed names/original slots are retained without native Clone/Debug/serde bounds.
/// Preparations may borrow native state under ordinary Rust lifetime rules; the
/// adapter must own or safely retain every actual operand beyond lexical cleanup.
/// Extraction is mutable native data, not an authentic schema/grant/receipt.
///
/// ```compile_fail
/// use leselang_hir_core::native_group_preparation::PreparedNativeGroup;
/// fn copy(group: PreparedNativeGroup<'_, (), ()>) { let copied = group.clone(); }
/// ```
///
/// ```compile_fail
/// use leselang_hir_core::native_group_preparation::PreparedNativeGroup;
/// fn wire(group: PreparedNativeGroup<'_, (), ()>) {
///     let encoded = serde_json::to_string(&group);
/// }
/// ```
#[must_use = "publish only the complete preparation or deliberately discard it"]
pub struct PreparedNativeGroup<'group, Branch, Prepared> {
    metadata: NativeGroupMembers<'group, Branch>,
    members: Vec<PreparedNativeMember<'group, Branch, Prepared>>,
}
impl<'group, Branch, Prepared> PreparedNativeGroup<'group, Branch, Prepared> {
    pub fn metadata(&self) -> &NativeGroupMembers<'group, Branch> {
        &self.metadata
    }
    pub fn members(&self) -> &[PreparedNativeMember<'group, Branch, Prepared>] {
        &self.members
    }
    pub fn into_parts(
        self,
    ) -> (
        NativeGroupMembers<'group, Branch>,
        Vec<PreparedNativeMember<'group, Branch, Prepared>>,
    ) {
        (self.metadata, self.members)
    }
}
impl<Branch, Prepared> fmt::Debug for PreparedNativeGroup<'_, Branch, Prepared> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PreparedNativeGroup")
            .field("kind", &self.metadata.kind())
            .field("members", &self.members.len())
            .finish()
    }
}

pub enum NativeGroupPreparationError<Error> {
    Admission(NativeGroupMemberError<Error>),
    /// None is the group root; Some is the original member whose debit failed.
    Fuel {
        member_index: Option<usize>,
    },
    Native {
        member_index: usize,
        error: Error,
    },
}
impl<Error> fmt::Display for NativeGroupPreparationError<Error> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Admission(_) => "native group preparation admission rejected",
            Self::Fuel { .. } => "native group preparation fuel exhausted",
            Self::Native { .. } => "native group member preparation rejected",
        })
    }
}
impl<Error> fmt::Debug for NativeGroupPreparationError<Error> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
impl<Error> std::error::Error for NativeGroupPreparationError<Error> {}

/// Prepare a complete native group, borrowing the caller's original fuel meter.
/// Shared inclusive arity (Seq >=1, All >=2, <=64), all once-observed names and
/// every cold native member admission precede any fuel debit or preparation.
/// One group-root unit and one unit per member precede its once-only callback.
/// The callback may close over a still-live lexical ScopeFrame and use shared call
/// evaluation; it must bound/charge actual operands and recheck live native policy.
/// No per-member meter reset, native copying or implicit dispatch is introduced.
///
/// Complete preparation alone is handed off. Failure/unwind drops the prepared
/// prefix, returns no partial batch and never retries/refunds earlier work. Callback
/// side effects cannot be rolled back; callbacks/Drop are trusted and a second
/// cleanup panic may abort. Caller ingress/aggregate memory/native work bounds,
/// complete graph shape/cold typing and lexical-scope admission remain separate.
/// This generic member-slice entry does not infer IR, schema authenticity, flatness,
/// grants, identity uniqueness, group-result domains, scheduling or durable replay.
pub fn prepare_native_group_members<'group, Branch, Prepared, Error>(
    kind: GroupKind,
    members: &'group [Branch],
    max_members: usize,
    fuel: &mut Fuel,
    name: impl FnMut(&'group Branch) -> Result<&'group str, Error>,
    admit: impl FnMut(&'group Branch) -> Result<(), Error>,
    mut prepare: impl FnMut(usize, &'group Branch, &mut Fuel) -> Result<Prepared, Error>,
) -> Result<PreparedNativeGroup<'group, Branch, Prepared>, NativeGroupPreparationError<Error>> {
    let metadata = admit_native_group_members(kind, members, max_members, name, admit)
        .map_err(NativeGroupPreparationError::Admission)?;
    fuel.charge(1)
        .map_err(|_| NativeGroupPreparationError::Fuel { member_index: None })?;
    let mut prepared = Vec::with_capacity(members.len());
    for (member_index, original) in members.iter().enumerate() {
        fuel.charge(1)
            .map_err(|_| NativeGroupPreparationError::Fuel {
                member_index: Some(member_index),
            })?;
        let value = prepare(member_index, original, fuel).map_err(|error| {
            NativeGroupPreparationError::Native {
                member_index,
                error,
            }
        })?;
        prepared.push(PreparedNativeMember {
            index: member_index,
            original,
            prepared: value,
        });
    }
    Ok(PreparedNativeGroup {
        metadata,
        members: prepared,
    })
}
