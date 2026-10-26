use super::*;
#[cfg(feature = "sqlite-journal")]
use leselang_command::reference_sequence_gate::DurableSequenceJournal;
use leselang_command::reference_sequence_gate::{MemorySequenceGroup, MemorySequenceJournal};

impl MemorySequenceJournal for EphemeralJournal {
    fn groups(&self) -> impl Iterator<Item = MemorySequenceGroup<'_>> {
        self.merge_groups.values().map(|g| MemorySequenceGroup {
            order: g.plan.order,
            branches: &g.branch_tokens,
            terminal: g.terminal_step.is_some(),
        })
    }
    fn terminal(&self, token: &ContinuationToken) -> Option<&Step> {
        self.dispatches
            .get(token)
            .and_then(|d| d.terminal_step.as_ref())
    }
}

#[cfg(feature = "sqlite-journal")]
pub(super) struct SqliteSequenceJournal<'a>(pub &'a Connection);

#[cfg(feature = "sqlite-journal")]
impl DurableSequenceJournal for SqliteSequenceJournal<'_> {
    fn has_unfinished_predecessor(&mut self, token: &str) -> Result<bool, Fault> {
        self.0.query_row(
            "SELECT EXISTS (
               SELECT 1 FROM vm_merge_branches own
               JOIN vm_merge_groups g ON g.token = own.group_token
               JOIN vm_merge_branches prior ON prior.group_token = own.group_token AND prior.position < own.position
               JOIN vm_effects e ON e.token = prior.branch_token
               WHERE own.branch_token = ?1 AND g.execution_order = 'sequential' AND e.state != 'completed'
             )", [token], |row| row.get(0)
        ).map_err(|error| journal_error("LSV4033", "failed to check sequential predecessor", error))
    }
}
