//! Fresh closed signature comparison, not a schema or execution certificate.

use std::fmt;

use crate::ir::GroupKind;
use crate::native_group::{NativeGroupMemberError, preflight_native_group_members};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeGroupSignatureSide {
    Left,
    Right,
}

/// Original side/member positions and opaque errors remain matchable. Neither
/// formatting nor Error::source exposes native payloads or name buffers.
///
/// ```compile_fail
/// use leselang_hir::native_group_signature::NativeGroupSignatureError;
/// let error = NativeGroupSignatureError::Native { member_index: 0, error: () };
/// let wire = serde_json::to_string(&error).unwrap();
/// ```
pub enum NativeGroupSignatureError<Error> {
    Members {
        side: NativeGroupSignatureSide,
        error: NativeGroupMemberError<Error>,
    },
    Native {
        member_index: usize,
        error: Error,
    },
}
impl<Error> fmt::Display for NativeGroupSignatureError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Members { .. } => "native group signature member preflight failed",
            Self::Native { .. } => "native group signature operation comparison failed",
        })
    }
}
impl<Error> fmt::Debug for NativeGroupSignatureError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}
impl<Error> std::error::Error for NativeGroupSignatureError<Error> {}

/// Compare two original closed signatures without copying native slots or names.
/// Fresh complete left then right member/name preflight precedes every native
/// comparison, including when both slices alias or the modes/counts differ.
/// Current inclusive arity, the 64-member safety ceiling and shared member-label
/// grammar apply separately to both inputs; zero denies groups before hooks.
/// No cached member view, derived equality or pointer shortcut is accepted.
///
/// Invalid inputs return a side/member error, not equality. Valid mode/count/name
/// order mismatches return false before native pairs. Equal spelling need not be
/// the same name buffer. Matching original member pairs reach the explicit native
/// comparator once in declaration order; false/error/unwind stops without retry.
/// No native Clone/Debug/equality/serde/Send requirement or heap collection applies.
/// The comparator may bridge different native member/schema representations.
///
/// Native callbacks must expose original names and corroborate operation/result
/// row identity and any relevant live policy. Core shape agreement cannot infer
/// native authenticity, payload bounds, types or grants. Hooks, interior state,
/// allocation and Drop remain trusted native behavior, not preempted or rolled
/// back. Borrowed graphs are not consumed; external callback effects may remain.
/// True is ephemeral metadata, never a durable type/identity/grant certificate,
/// result acceptance, fuel reservation, dispatch or nested graph promotion.
/// A successful comparison must not replace later admission after mutation.
///
/// ```compile_fail
/// use leselang_hir::{ir::GroupKind, native_group_signature::compare_native_group_signatures};
/// let members = [()];
/// let equal = compare_native_group_signatures(
///     (GroupKind::Sequence, &members), (GroupKind::Sequence, &members), 1,
///     |_| -> Result<_, ()> { Ok("member") }, |_| Ok("member"));
/// ```
pub fn compare_native_group_signatures<'left, 'right, Left, Right, Error>(
    left: (GroupKind, &'left [Left]),
    right: (GroupKind, &'right [Right]),
    max_members: usize,
    left_name: impl FnMut(&'left Left) -> Result<&'left str, Error>,
    right_name: impl FnMut(&'right Right) -> Result<&'right str, Error>,
    mut same: impl FnMut(&'left Left, &'right Right) -> Result<bool, Error>,
) -> Result<bool, NativeGroupSignatureError<Error>> {
    let left = preflight_native_group_members(left.0, left.1, max_members, left_name).map_err(
        |error| NativeGroupSignatureError::Members {
            side: NativeGroupSignatureSide::Left,
            error,
        },
    )?;
    let right = preflight_native_group_members(right.0, right.1, max_members, right_name).map_err(
        |error| NativeGroupSignatureError::Members {
            side: NativeGroupSignatureSide::Right,
            error,
        },
    )?;
    if left.kind() != right.kind()
        || left.members().len() != right.members().len()
        || (0..left.members().len()).any(|index| match (left.name(index), right.name(index)) {
            (Some(left), Some(right)) => left != right,
            _ => true,
        })
    {
        return Ok(false);
    }
    for (member_index, (left, right)) in left.members().iter().zip(right.members()).enumerate() {
        if !same(left, right).map_err(|error| NativeGroupSignatureError::Native {
            member_index,
            error,
        })? {
            return Ok(false);
        }
    }
    Ok(true)
}
