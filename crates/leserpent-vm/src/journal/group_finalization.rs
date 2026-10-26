use super::*;
#[cfg(feature = "sqlite-journal")]
use leselang_command::reference_group_finalization::{
    DurableGroupFinalization, GroupTerminalEncoding, StoredFinalizingBranch, StoredFinalizingGroup,
};
use leselang_command::reference_group_finalization::{
    GroupFinalizationOwner, MemoryGroupFinalization, MemoryGroupState, MemoryStoppedBranch,
};

impl GroupFinalizationOwner for EphemeralJournal {
    fn merge_prefix(
        &mut self,
        plan: &MergePlan,
        completions: Vec<BranchCompletion>,
    ) -> Result<Step, Fault> {
        group_binding::merge_prefix(plan, completions)
    }
    fn admit_successor(
        &mut self,
        token: &ContinuationToken,
        plan: &MergePlan,
        prefix: &Step,
    ) -> Result<Option<Step>, Fault> {
        group_binding::admit_ephemeral(self, token, plan, prefix)
    }
    fn finish(
        &mut self,
        plan: &MergePlan,
        completions: Vec<BranchCompletion>,
    ) -> Result<Step, Fault> {
        group_binding::finish(plan, completions)
    }
}
impl MemoryGroupFinalization for EphemeralJournal {
    fn locate(&self, branch: &ContinuationToken) -> Option<ContinuationToken> {
        self.merge_groups
            .iter()
            .find(|(_, g)| g.branch_tokens.contains(branch))
            .map(|(t, _)| t.clone())
    }
    fn group(&self, token: &ContinuationToken) -> Option<MemoryGroupState<'_>> {
        self.merge_groups.get(token).map(|g| MemoryGroupState {
            plan: &g.plan,
            branches: &g.branch_tokens,
            completed: g.terminal_step.is_some(),
        })
    }
    fn branch_terminal(&self, token: &ContinuationToken) -> Option<&Step> {
        self.dispatches
            .get(token)
            .and_then(|d| d.terminal_step.as_ref())
    }
    fn branch_mut(&mut self, token: &ContinuationToken) -> Option<MemoryStoppedBranch<'_>> {
        self.dispatches.get_mut(token).map(|d| MemoryStoppedBranch {
            acknowledged: &mut d.acknowledged,
            lease_expires_at_ms: &mut d.lease_expires_at_ms,
            terminal_step: &mut d.terminal_step,
        })
    }
    fn group_terminal_mut(&mut self, token: &ContinuationToken) -> Option<&mut Option<Step>> {
        self.merge_groups
            .get_mut(token)
            .map(|g| &mut g.terminal_step)
    }
}

#[cfg(feature = "sqlite-journal")]
pub(super) struct SqliteGroupFinalization<'a, 'connection>(
    pub &'a rusqlite::Transaction<'connection>,
);

#[cfg(feature = "sqlite-journal")]
impl GroupFinalizationOwner for SqliteGroupFinalization<'_, '_> {
    fn merge_prefix(
        &mut self,
        plan: &MergePlan,
        completions: Vec<BranchCompletion>,
    ) -> Result<Step, Fault> {
        group_binding::merge_prefix(plan, completions)
    }
    fn admit_successor(
        &mut self,
        token: &ContinuationToken,
        plan: &MergePlan,
        prefix: &Step,
    ) -> Result<Option<Step>, Fault> {
        group_binding::admit_successor(self.0, token, plan, prefix)
    }
    fn finish(
        &mut self,
        plan: &MergePlan,
        completions: Vec<BranchCompletion>,
    ) -> Result<Step, Fault> {
        group_binding::finish(plan, completions)
    }
}
#[cfg(feature = "sqlite-journal")]
impl DurableGroupFinalization for SqliteGroupFinalization<'_, '_> {
    fn locate(&mut self, branch: &str) -> Result<Option<StoredFinalizingGroup>, Fault> {
        self.0
            .query_row(
                "SELECT g.token, g.plan, g.state, g.terminal_step
             FROM vm_merge_groups g
             JOIN vm_merge_branches b ON b.group_token = g.token
             WHERE b.branch_token = ?1",
                [branch],
                |r| {
                    Ok(StoredFinalizingGroup {
                        token: r.get(0)?,
                        plan: r.get(1)?,
                        state: r.get(2)?,
                        terminal: r.get(3)?,
                    })
                },
            )
            .optional()
            .map_err(|e| journal_error("LSV4033", "failed to locate merge group", e))
    }
    fn branches(&mut self, token: &str) -> Result<Vec<StoredFinalizingBranch>, Fault> {
        let mut statement = self
            .0
            .prepare(
                "SELECT b.branch_name, e.state, e.terminal_step
             FROM vm_merge_branches b
             JOIN vm_effects e ON e.token = b.branch_token
             WHERE b.group_token = ?1
             ORDER BY b.position ASC",
            )
            .map_err(|e| journal_error("LSV4033", "failed to prepare merge completion", e))?;
        statement
            .query_map([token], |r| {
                Ok(StoredFinalizingBranch {
                    name: r.get(0)?,
                    state: r.get(1)?,
                    terminal: r.get(2)?,
                })
            })
            .map_err(|e| journal_error("LSV4033", "failed to scan merge completion", e))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| journal_error("LSV4033", "failed to read merge completion", e))
    }
    fn encode_terminal(
        &mut self,
        step: &Step,
        kind: GroupTerminalEncoding,
    ) -> Result<Vec<u8>, Fault> {
        encode_json_capped(
            step,
            MAX_JOURNAL_ENTRY_BYTES,
            match kind {
                GroupTerminalEncoding::SequenceStop => "sequence failure",
                GroupTerminalEncoding::MergeResult => "merge result",
            },
        )
    }
    fn pending_tokens(&mut self, token: &str) -> Result<Vec<String>, Fault> {
        let mut statement = self
            .0
            .prepare(
                "SELECT b.branch_token FROM vm_merge_branches b
             JOIN vm_effects e ON e.token = b.branch_token
             WHERE b.group_token = ?1 AND e.state = 'pending'",
            )
            .map_err(|e| journal_error("LSV4033", "failed to prepare sequence stop", e))?;
        statement
            .query_map([token], |r| r.get::<_, String>(0))
            .map_err(|e| journal_error("LSV4033", "failed to scan stopped sequence", e))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| journal_error("LSV4033", "invalid stopped sequence", e))
    }
    fn ensure_growth(&mut self, additional: usize) -> Result<(), Fault> {
        ensure_growth(self.0, additional)
    }
    fn complete_branch(&mut self, token: &str, bytes: &[u8]) -> Result<(), Fault> {
        complete_terminal_record(self.0, token, bytes)
    }
    fn stop_group(&mut self, token: &str, bytes: &[u8]) -> Result<(), Fault> {
        self.0.execute("UPDATE vm_merge_groups SET state = 'completed', terminal_step = ?2 WHERE token = ?1", params![token, bytes])
            .map_err(|e| journal_error("LSV4033", "failed to stop sequence", e))?;
        Ok(())
    }
    fn complete_group(&mut self, token: &str, bytes: &[u8]) -> Result<usize, Fault> {
        self.0
            .execute(
                "UPDATE vm_merge_groups
             SET state = 'completed', terminal_step = ?2
             WHERE token = ?1 AND state = 'pending'",
                params![token, bytes],
            )
            .map_err(|e| journal_error("LSV4033", "failed to complete merge group", e))
    }
}
