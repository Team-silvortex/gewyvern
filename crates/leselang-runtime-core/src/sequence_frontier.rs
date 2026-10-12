use std::fmt;

use crate::MAX_GROUP_GRAPH_MEMBERS;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SequentialMemberState {
    Completed,
    Pending,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SequenceFrontierSite {
    Member(usize),
    Prefix,
    Blocked,
}

pub enum SequenceFrontierError<Native> {
    InvalidLimits,
    Count,
    OutOfOrder {
        index: usize,
    },
    Native {
        site: SequenceFrontierSite,
        error: Native,
    },
}

impl<Native> fmt::Display for SequenceFrontierError<Native> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidLimits => "sequence frontier limit exceeds safety ceiling",
            Self::Count => "sequence frontier exceeds member limit",
            Self::OutOfOrder { .. } => "sequence completion is out of order",
            Self::Native { .. } => "sequence frontier native observation failed",
        })
    }
}
impl<Native> fmt::Debug for SequenceFrontierError<Native> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}
impl<Native> std::error::Error for SequenceFrontierError<Native> {}

/// Inspect a complete original sequential group without sorting or allocation.
/// Inclusive limits (ceiling 64) precede hooks. Empty input is structurally valid;
/// language arity, original row state/schema and identity remain adapter-owned.
/// Classify each row once in order. Completion after the first pending row refuses
/// before prefix decoding or blocked-dispatch observation. Then admit the complete
/// completed prefix once, followed by the blocked suffix once. The first pending
/// row is the active frontier and is excluded from the blocked suffix. If all rows
/// are complete, the frontier index is rows.len() and the blocked suffix is empty.
/// Both admission hooks still run on empty slices; native policy bounds their work.
///
/// Rows/errors need no Clone/Debug/Eq/serde/Default/Send/Sync bounds. Native faults
/// move unchanged with payload-free diagnostics and no Error::source. Uncalled
/// hook owners release before result/error handoff. Refusal or unwind stops the
/// pass without retry or later hooks; a second cleanup panic may abort. Native
/// callback side effects and cleanup cannot be rolled back or sandboxed here.
///
/// Success is transient inspection, not a dispatch lease, receipt or execution
/// grant. Hosts own result decoding, failure/output policy, original attempt/retry
/// evidence, current authority and same-snapshot graph/receipt validation. This
/// never evaluates a body, constructs fuel, dispatches, repairs storage, replays
/// or commits. Native recovery must commit before publishing a validated snapshot.
pub fn inspect_sequential_frontier<'rows, Row, Native>(
    rows: &'rows [Row],
    max_members: usize,
    mut classify: impl FnMut(usize, &'rows Row) -> Result<SequentialMemberState, Native>,
    admit_prefix: impl FnOnce(&'rows [Row]) -> Result<(), Native>,
    admit_blocked: impl FnOnce(usize, &'rows [Row]) -> Result<(), Native>,
) -> Result<(), SequenceFrontierError<Native>> {
    let outcome = (|| {
        if max_members > MAX_GROUP_GRAPH_MEMBERS {
            return Err(SequenceFrontierError::InvalidLimits);
        }
        if rows.len() > max_members {
            return Err(SequenceFrontierError::Count);
        }
        let mut frontier = rows.len();
        for (index, row) in rows.iter().enumerate() {
            let state = classify(index, row).map_err(|error| SequenceFrontierError::Native {
                site: SequenceFrontierSite::Member(index),
                error,
            })?;
            match state {
                SequentialMemberState::Pending if frontier == rows.len() => frontier = index,
                SequentialMemberState::Completed if frontier != rows.len() => {
                    return Err(SequenceFrontierError::OutOfOrder { index });
                }
                _ => {}
            }
        }
        admit_prefix(&rows[..frontier]).map_err(|error| SequenceFrontierError::Native {
            site: SequenceFrontierSite::Prefix,
            error,
        })?;
        let blocked_start = frontier.saturating_add(1).min(rows.len());
        admit_blocked(frontier, &rows[blocked_start..]).map_err(|error| {
            SequenceFrontierError::Native {
                site: SequenceFrontierSite::Blocked,
                error,
            }
        })?;
        Ok(())
    })();
    drop(classify);
    outcome
}
