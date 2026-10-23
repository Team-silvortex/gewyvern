//! Released product identity data and interpretation, not identity allocation.
//!
//! Native runtime hosts keep their own identity schemas. These strings carry no
//! authority to resume, cancel, reserve or restore work. The owner must validate
//! the complete request and its current journal/lease state before acting.

use std::cmp::Ordering;

use serde::{Deserialize, Serialize};

pub const MAX_EFFECT_SEQUENCE: u64 = i64::MAX as u64 - 1;

/// Unchecked wire identity, with the released string representation and ordering.
///
/// The inner field remains private:
/// ```compile_fail
/// use leselang_command::ContinuationToken;
/// let token = ContinuationToken("continuation-1".to_string());
/// ```
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct ContinuationToken(String);

impl ContinuationToken {
    /// Preserve raw wire/storage data without validating, normalizing or allocating
    /// an identity. Like deserialization, this may retain an invalid token so the
    /// caller can reject it at its original admission boundary and error priority.
    pub fn from_wire(value: String) -> Self {
        Self(value)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Lexical compatibility check only; opaque legacy group IDs remain valid.
pub fn valid_continuation_token(token: &ContinuationToken) -> bool {
    !token.as_str().is_empty()
        && token.as_str().len() <= 128
        && token
            .as_str()
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

pub fn canonical_continuation_sequence(token: &ContinuationToken) -> Option<u64> {
    canonical_sequence(token, "continuation-")
}

pub fn canonical_merge_sequence(token: &ContinuationToken) -> Option<u64> {
    canonical_sequence(token, "merge-")
}

fn canonical_sequence(token: &ContinuationToken, prefix: &str) -> Option<u64> {
    let suffix = token.as_str().strip_prefix(prefix)?;
    let sequence = suffix.parse::<u64>().ok()?;
    (sequence > 0 && sequence <= MAX_EFFECT_SEQUENCE && sequence.to_string() == suffix)
        .then_some(sequence)
}

/// Released scheduling/retention age ordering, not canonical identity validation.
/// Numeric continuation suffixes sort first; opaque identities sort lexically.
/// Non-canonical numeric suffixes intentionally retain their historical ordering.
pub fn continuation_age_order(left: &ContinuationToken, right: &ContinuationToken) -> Ordering {
    let sequence = |token: &ContinuationToken| {
        token
            .as_str()
            .strip_prefix("continuation-")
            .and_then(|value| value.parse::<u64>().ok())
    };
    match (sequence(left), sequence(right)) {
        (Some(left), Some(right)) => left.cmp(&right),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => left.cmp(right),
    }
}
