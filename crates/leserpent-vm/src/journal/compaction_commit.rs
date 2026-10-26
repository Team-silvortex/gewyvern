use super::*;
#[cfg(feature = "sqlite-journal")]
use leselang_command::reference_compaction_commit::DurableCompactionTransaction;
use leselang_command::reference_compaction_commit::{
    MemoryCompactionDispatch, MemoryCompactionGroup, MemoryCompactionJournal,
};

fn group_view(group: &EphemeralMergeGroup) -> MemoryCompactionGroup<'_> {
    MemoryCompactionGroup {
        plan: &group.plan,
        branches: &group.branch_tokens,
        terminal: group.terminal_step.as_ref(),
    }
}

impl MemoryCompactionJournal for EphemeralJournal {
    fn groups(&self) -> impl Iterator<Item = (&ContinuationToken, MemoryCompactionGroup<'_>)> {
        self.merge_groups
            .iter()
            .map(|(token, group)| (token, group_view(group)))
    }
    fn group(&self, token: &ContinuationToken) -> Option<MemoryCompactionGroup<'_>> {
        self.merge_groups.get(token).map(group_view)
    }
    fn dispatch(&self, token: &ContinuationToken) -> Option<MemoryCompactionDispatch<'_>> {
        self.dispatches
            .get(token)
            .map(|dispatch| MemoryCompactionDispatch {
                request: &dispatch.request,
                last_error: dispatch.last_error.as_ref(),
            })
    }
    fn measure<T: serde::Serialize>(
        &self,
        value: &T,
        limit: usize,
        label: &str,
    ) -> Result<usize, Fault> {
        measure_json_capped(value, limit, label)
    }
    fn remove_group(&mut self, token: &ContinuationToken) {
        self.merge_groups.remove(token);
    }
    fn remove_dispatch(&mut self, token: &ContinuationToken) {
        self.dispatches.remove(token);
    }
    fn retain_live_audits(&mut self) {
        self.debugger_audits
            .retain(|_, audit| self.dispatches.contains_key(&audit.token));
    }
}

#[cfg(feature = "sqlite-journal")]
pub(super) struct SqliteCompactionTransaction<'a>(pub rusqlite::Transaction<'a>);

#[cfg(feature = "sqlite-journal")]
impl DurableCompactionTransaction for SqliteCompactionTransaction<'_> {
    fn completed_count(&mut self) -> Result<i64, Fault> {
        self.0
            .query_row(
                "SELECT
               (SELECT COUNT(*) FROM vm_effects e
                WHERE e.state = 'completed'
                  AND NOT EXISTS (
                    SELECT 1 FROM vm_merge_branches b WHERE b.branch_token = e.token
                  )) +
               (SELECT COUNT(*) FROM vm_merge_groups WHERE state = 'completed')",
                [],
                |row| row.get(0),
            )
            .map_err(|error| {
                journal_error(
                    "LSV4030",
                    "failed to count completed journal records",
                    error,
                )
            })
    }
    fn candidates(&mut self, limit: i64) -> Result<Vec<(i64, String)>, Fault> {
        let mut statement = self
            .0
            .prepare(
                "SELECT kind, token FROM (
               SELECT 0 AS kind, e.token AS token, e.rowid AS age
               FROM vm_effects e
               WHERE e.state = 'completed'
                 AND NOT EXISTS (
                   SELECT 1 FROM vm_merge_branches b WHERE b.branch_token = e.token
                 )
               UNION ALL
               SELECT 1 AS kind, g.token AS token, MIN(e.rowid) AS age
               FROM vm_merge_groups g
               JOIN vm_merge_branches b ON b.group_token = g.token
               JOIN vm_effects e ON e.token = b.branch_token
               WHERE g.state = 'completed'
               GROUP BY g.token
             ) ORDER BY age ASC, kind ASC, token ASC
             LIMIT ?1",
            )
            .map_err(|error| {
                journal_error("LSV4030", "failed to prepare journal compaction", error)
            })?;
        let rows = statement
            .query_map([limit], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|error| {
                journal_error("LSV4030", "failed to scan journal compaction", error)
            })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| journal_error("LSV4030", "failed to read journal compaction", error))
    }
    fn effect_bytes(&mut self, token: &str) -> Result<usize, Fault> {
        completed_effect_logical_bytes(&self.0, token)
    }
    fn delete_effect(&mut self, token: &str) -> Result<usize, Fault> {
        self.0
            .execute(
                "DELETE FROM vm_effects
             WHERE token = ?1 AND state = 'completed'
               AND NOT EXISTS (
                 SELECT 1 FROM vm_merge_branches
                 WHERE branch_token = vm_effects.token
               )",
                [token],
            )
            .map_err(|error| {
                journal_error(
                    "LSV4030",
                    "failed to delete completed journal record",
                    error,
                )
            })
    }
    fn delete_group(&mut self, token: &str) -> Result<usize, Fault> {
        delete_completed_merge_group(&self.0, token)
    }
    fn stale_local(
        &mut self,
        local: &[(ContinuationToken, Step)],
    ) -> Result<Vec<ContinuationToken>, Fault> {
        stale_local_completed(&self.0, local)
    }
    fn commit(self, no_removal: bool) -> Result<(), Fault> {
        self.0.commit().map_err(|error| {
            journal_error(
                "LSV4030",
                if no_removal {
                    "failed to close journal compaction"
                } else {
                    "failed to commit journal compaction"
                },
                error,
            )
        })
    }
}
