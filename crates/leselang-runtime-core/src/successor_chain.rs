use std::fmt;

use crate::MAX_GROUP_GRAPH_MEMBERS;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SuccessorChainSite {
    First,
    Link { parent: usize, next: usize },
}

/// Native faults move unchanged; diagnostics never expose their private payload.
pub enum SuccessorChainError<Native> {
    InvalidLimits,
    Count,
    Native {
        site: SuccessorChainSite,
        error: Native,
    },
}

impl<Native> fmt::Display for SuccessorChainError<Native> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidLimits => "successor chain limit exceeds safety ceiling",
            Self::Count => "successor chain requires a bounded adjacent pair",
            Self::Native { .. } => "successor chain native observation failed",
        })
    }
}

impl<Native> fmt::Debug for SuccessorChainError<Native> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}

impl<Native> std::error::Error for SuccessorChainError<Native> {}

/// Inspect original adjacent rows without sorting, copying or renumbering them.
/// Inclusive bounds (2..=max_members, ceiling 64) precede every native hook.
/// Observe the first row once, then each original adjacent pair once in order.
/// Native hooks own bounded decoding, frame correspondence, schema, identity,
/// capture, authority and budget policy, including their diagnostic precedence.
/// Rows, frames and errors need no Clone/Debug/Eq/serde/Send/Sync/Default bounds.
///
/// A frame remains borrowed through its link hook, then is released before the
/// next link. The final frame and callback owners are released before handoff.
/// Refusal or unwind stops the pass without retries or later observation. Native
/// side effects cannot be rolled back; a second cleanup panic may abort.
///
/// Success is only transient inspection, not a receipt, graph certificate or
/// execution grant. This never evaluates a body, reconstructs fuel, dispatches,
/// repairs allocation state, replays or commits storage. Recovery adapters must
/// retain the same snapshot and finish all native checks and commit before publish.
pub fn inspect_successor_chain<'rows, Row, Frame, Native>(
    rows: &'rows [Row],
    max_members: usize,
    first: impl FnOnce(&'rows Row) -> Result<Frame, Native>,
    mut next: impl FnMut(SuccessorChainSite, &Frame, &'rows Row, &'rows Row) -> Result<Frame, Native>,
) -> Result<(), SuccessorChainError<Native>> {
    let outcome = (|| {
        if max_members > MAX_GROUP_GRAPH_MEMBERS {
            return Err(SuccessorChainError::InvalidLimits);
        }
        if rows.len() < 2 || rows.len() > max_members {
            return Err(SuccessorChainError::Count);
        }
        let mut parent = first(&rows[0]).map_err(|error| SuccessorChainError::Native {
            site: SuccessorChainSite::First,
            error,
        })?;
        for (index, pair) in rows.windows(2).enumerate() {
            let site = SuccessorChainSite::Link {
                parent: index,
                next: index + 1,
            };
            let next_frame = next(site, &parent, &pair[0], &pair[1])
                .map_err(|error| SuccessorChainError::Native { site, error })?;
            drop(parent);
            parent = next_frame;
        }
        drop(parent);
        Ok(())
    })();
    drop(next);
    outcome
}
