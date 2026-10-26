use super::*;
#[cfg(feature = "sqlite-journal")]
use leselang_command::reference_group_progress::{DurableGroupProgress, StoredGroupProgress};
use leselang_command::reference_group_progress::{
    MemoryGroupProgress, MemoryProgressDispatch, MemoryProgressGroup,
};

impl MemoryGroupProgress for EphemeralJournal {
    fn locate(&self, branch: &ContinuationToken) -> Option<MemoryProgressGroup<'_>> {
        self.merge_groups
            .iter()
            .find(|(_, g)| g.branch_tokens.contains(branch))
            .map(|(token, g)| MemoryProgressGroup {
                token,
                plan: &g.plan,
                branches: &g.branch_tokens,
                terminal: g.terminal_step.as_ref(),
            })
    }
    fn dispatch(&self, token: &ContinuationToken) -> Option<MemoryProgressDispatch<'_>> {
        self.dispatches.get(token).map(|d| MemoryProgressDispatch {
            request: &d.request,
            acknowledged: d.acknowledged,
            terminal: d.terminal_step.is_some(),
        })
    }
}

#[cfg(feature = "sqlite-journal")]
pub(super) struct SqliteGroupProgress<'a, 'connection>(pub &'a rusqlite::Transaction<'connection>);

#[cfg(feature = "sqlite-journal")]
impl DurableGroupProgress for SqliteGroupProgress<'_, '_> {
    fn locate(&mut self, branch: &str) -> Result<Option<StoredGroupProgress>, Fault> {
        self.0
            .query_row(
                "SELECT g.token, g.state, g.terminal_step,
                    SUM(CASE WHEN e.state = 'completed' THEN 1 ELSE 0 END),
                    COUNT(*), g.execution_order
             FROM vm_merge_groups g
             JOIN vm_merge_branches own ON own.group_token = g.token
             JOIN vm_merge_branches branches ON branches.group_token = g.token
             JOIN vm_effects e ON e.token = branches.branch_token
             WHERE own.branch_token = ?1
             GROUP BY g.token, g.state, g.terminal_step",
                [branch],
                |r| {
                    Ok(StoredGroupProgress {
                        token: r.get(0)?,
                        state: r.get(1)?,
                        terminal: r.get(2)?,
                        completed: r.get(3)?,
                        total: r.get(4)?,
                        order: r.get(5)?,
                    })
                },
            )
            .optional()
            .map_err(|e| journal_error("LSV4033", "failed to read merge progress", e))
    }
    fn plan(&mut self, owner: &str) -> Result<Vec<u8>, Fault> {
        self.0
            .query_row(
                "SELECT plan FROM vm_merge_groups WHERE token = ?1",
                [owner],
                |r| r.get(0),
            )
            .map_err(|e| journal_error("LSV4033", "failed to inspect parallel successor", e))
    }
    fn next_request(&mut self, owner: &str) -> Result<Vec<u8>, Fault> {
        self.0
            .query_row(
                "SELECT d.request FROM vm_merge_branches b
             JOIN vm_effects e ON e.token = b.branch_token
             JOIN vm_dispatches d ON d.token = b.branch_token
             WHERE b.group_token = ?1 AND e.state = 'pending'
             ORDER BY b.position LIMIT 1",
                [owner],
                |r| r.get(0),
            )
            .map_err(|e| journal_error("LSV4033", "failed to read next sequence step", e))
    }
    fn completed_tokens(&mut self, owner: &str) -> Result<Vec<ContinuationToken>, Fault> {
        let mut statement = self.0.prepare(
            "SELECT b.branch_token FROM vm_merge_branches b JOIN vm_effects e ON e.token = b.branch_token
             WHERE b.group_token = ?1 AND e.state = 'completed' ORDER BY b.position",
        ).map_err(|e| journal_error("LSV4033", "failed to inspect completed prefix", e))?;
        statement
            .query_map([owner], |r| Ok(ContinuationToken::from_wire(r.get(0)?)))
            .map_err(|e| journal_error("LSV4033", "failed to load completed prefix", e))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| journal_error("LSV4033", "invalid completed prefix", e))
    }
    fn branch_tokens(&mut self, owner: &str) -> Result<Vec<ContinuationToken>, Fault> {
        let mut statement = self.0.prepare(
            "SELECT branch_token FROM vm_merge_branches WHERE group_token = ?1 ORDER BY position",
        ).map_err(|e| journal_error("LSV4033", "failed to read completed group", e))?;
        statement
            .query_map([owner], |r| Ok(ContinuationToken::from_wire(r.get(0)?)))
            .map_err(|e| journal_error("LSV4033", "failed to read completed branches", e))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| journal_error("LSV4033", "invalid completed branches", e))
    }
}
