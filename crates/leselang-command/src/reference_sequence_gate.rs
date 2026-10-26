//! Reference sequential-predecessor policy over already-admitted native journals.

use crate::{ContinuationToken, ExecutionOrder, Step};
use leselang_runtime_core::Fault;

pub struct MemorySequenceGroup<'a> {
    pub order: ExecutionOrder,
    pub branches: &'a [ContinuationToken],
    pub terminal: bool,
}

/// Retain native group order and lazy borrowed terminal lookups. These observations
/// neither admit a graph nor reserve a dispatch; the caller retains journal ownership.
pub trait MemorySequenceJournal {
    fn groups(&self) -> impl Iterator<Item = MemorySequenceGroup<'_>>;
    fn terminal(&self, token: &ContinuationToken) -> Option<&Step>;
}

/// All owning sequential groups must permit the branch. The first occurrence
/// determines its prefix; completed owners refuse even their first member.
/// Only Done predecessors pass, not merely acknowledged or terminal dispatches.
pub fn ready_memory<Host: MemorySequenceJournal + ?Sized>(
    host: &Host,
    token: &ContinuationToken,
) -> bool {
    host.groups().all(|group| {
        if !group.order.is_sequential() {
            return true;
        }
        let Some(position) = group.branches.iter().position(|branch| branch == token) else {
            return true;
        };
        !group.terminal
            && group.branches[..position]
                .iter()
                .all(|token| matches!(host.terminal(token), Some(Step::Done(_))))
    })
}

/// The original native EXISTS query remains in the caller's connection/transaction.
/// Its completed-state observation is not a decoded terminal receipt. Native
/// graph admission and atomic dispatch/completion ownership remain separate gates.
pub trait DurableSequenceJournal {
    fn has_unfinished_predecessor(&mut self, token: &str) -> Result<bool, Fault>;
}

pub fn ensure_durable<Host: DurableSequenceJournal + ?Sized>(
    host: &mut Host,
    token: &str,
) -> Result<(), Fault> {
    if host.has_unfinished_predecessor(token)? {
        return Err(predecessor_pending());
    }
    Ok(())
}

pub(crate) fn predecessor_pending() -> Fault {
    Fault {
        code: "LSV2410".into(),
        message: "sequential predecessor has not completed".into(),
    }
}
