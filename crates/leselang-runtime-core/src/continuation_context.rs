use std::fmt;

use crate::ResourceBudget;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContinuationContextSite {
    Authority,
    Revision,
}

pub enum ContinuationContextError<Native> {
    Authority,
    Revision,
    Deadline,
    AbsoluteDeadline,
    Output,
    Native {
        site: ContinuationContextSite,
        error: Native,
    },
}

impl<Native> fmt::Display for ContinuationContextError<Native> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Authority => "continuation inherited authority differs",
            Self::Revision => "continuation inherited revision differs",
            Self::Deadline => "continuation inherited relative deadline differs",
            Self::AbsoluteDeadline => "continuation inherited absolute deadline differs",
            Self::Output => "continuation inherited output limit differs",
            Self::Native { .. } => "continuation inherited context observation failed",
        })
    }
}
impl<Native> fmt::Debug for ContinuationContextError<Native> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}
impl<Native> std::error::Error for ContinuationContextError<Native> {}

/// Inspect inherited context agreement without allocation or payload copies.
/// Native authority agreement precedes native revision agreement (each hook at
/// most once), then relative deadline, exact optional absolute deadline and output
/// limit equality. Refusal/unwind skips later hooks and never retries. Authority
/// and revision payloads stay native; no Eq/Clone/Debug/serde/Default/Send/Sync
/// bounds, identity parsing, name normalization or product clock policy is added.
/// Both hook owners release before result/error handoff. Native errors move
/// unchanged with redacted diagnostics and no Error::source. Native side effects
/// and cleanup cannot be rolled back or sandboxed; a second cleanup panic may abort.
///
/// This is transient agreement, not authenticity, current authorization or an
/// execution grant. Zero/maximum metadata is compared exactly, not admitted as a
/// valid budget: hosts must separately validate ingress and original/current
/// authority. Fuel is deliberately ignored, since native allocation/consumption
/// checks have different positions around this phase. Original identity, revision
/// semantics, schema/body/result admission, token order, fuel progression, current
/// deadline checks and same-snapshot commit-before-publish remain host-owned.
/// No meter construction, refill, evaluation, dispatch, replay or storage writes.
pub fn validate_inherited_context<Native>(
    original: &ResourceBudget,
    saved: &ResourceBudget,
    mut same_authority: impl FnMut() -> Result<bool, Native>,
    mut same_revision: impl FnMut() -> Result<bool, Native>,
) -> Result<(), ContinuationContextError<Native>> {
    let outcome = (|| {
        if !same_authority().map_err(|error| ContinuationContextError::Native {
            site: ContinuationContextSite::Authority,
            error,
        })? {
            return Err(ContinuationContextError::Authority);
        }
        if !same_revision().map_err(|error| ContinuationContextError::Native {
            site: ContinuationContextSite::Revision,
            error,
        })? {
            return Err(ContinuationContextError::Revision);
        }
        if original.deadline_ms != saved.deadline_ms {
            return Err(ContinuationContextError::Deadline);
        }
        if original.deadline_at_ms != saved.deadline_at_ms {
            return Err(ContinuationContextError::AbsoluteDeadline);
        }
        if original.max_output_items != saved.max_output_items {
            return Err(ContinuationContextError::Output);
        }
        Ok(())
    })();
    drop(same_authority);
    drop(same_revision);
    outcome
}
