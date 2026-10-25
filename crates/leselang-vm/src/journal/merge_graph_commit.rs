use super::*;
use leselang_command::reference_merge_graph_commit::MemoryMergeGraphJournal;
#[cfg(feature = "sqlite-journal")]
use leselang_command::reference_merge_graph_commit::{
    DurableMergeGraphTransaction, EncodedMergeBranch,
};

impl MemoryMergeGraphJournal for EphemeralJournal {
    fn contains_group(&self, token: &ContinuationToken) -> bool {
        self.merge_groups.contains_key(token)
    }
    fn contains_dispatch(&self, token: &ContinuationToken) -> bool {
        self.dispatches.contains_key(token)
    }
    fn check_capacity(&self, additional: usize, limits: SchedulerLimits) -> Result<(), Fault> {
        limits.check_admission(
            self.scheduler_pressure(0, limits).pending_dispatches,
            additional,
        )
    }
    fn insert_dispatch(&mut self, request: &EffectRequest) {
        self.dispatches.insert(
            request.continuation.token.clone(),
            EphemeralDispatch {
                request: request.clone(),
                attempt: 0,
                lease_expires_at_ms: None,
                ready_at_ms: 0,
                retry_count: 0,
                last_error: None,
                acknowledged: false,
                terminal_step: None,
            },
        );
    }
    fn insert_group(
        &mut self,
        token: &ContinuationToken,
        plan: &MergePlan,
        branch_tokens: Vec<ContinuationToken>,
    ) {
        self.merge_groups.insert(
            token.clone(),
            EphemeralMergeGroup {
                plan: plan.clone(),
                branch_tokens,
                terminal_step: None,
            },
        );
    }
}

#[cfg(feature = "sqlite-journal")]
pub(super) struct SqliteMergeGraphTransaction<'a>(pub(super) rusqlite::Transaction<'a>);

#[cfg(feature = "sqlite-journal")]
impl DurableMergeGraphTransaction for SqliteMergeGraphTransaction<'_> {
    fn record_count(&mut self) -> Result<usize, Fault> {
        journal_record_count(&self.0)
    }
    fn check_capacity(&mut self, additional: usize, limits: SchedulerLimits) -> Result<(), Fault> {
        limits.check_admission(
            sql_scheduler_pressure(&self.0, 0, limits)?.pending_dispatches,
            additional,
        )
    }
    fn ensure_growth(&mut self, additional: usize) -> Result<(), Fault> {
        ensure_growth(&self.0, additional)
    }
    fn group_conflicts(&mut self, group_token: &ContinuationToken) -> Result<bool, Fault> {
        let group_conflict: bool = self
            .0
            .query_row(
                "SELECT EXISTS(
                   SELECT 1 FROM vm_merge_groups WHERE token = ?1
                   UNION ALL
                   SELECT 1 FROM vm_effects WHERE token = ?1
                 )",
                [group_token.as_str()],
                |row| row.get(0),
            )
            .map_err(|error| {
                journal_error("LSV4032", "failed to validate merge group token", error)
            })?;

        Ok(group_conflict)
    }
    fn insert_group(
        &mut self,
        group_token: &ContinuationToken,
        plan: &MergePlan,
        plan_bytes: &[u8],
    ) -> Result<(), Fault> {
        self.0
            .execute(
                "INSERT INTO vm_merge_groups(token, plan, state, terminal_step, execution_order)
                 VALUES (?1, ?2, 'pending', NULL, ?3)",
                params![
                    group_token.as_str(),
                    plan_bytes,
                    execution_order_label(plan.order)
                ],
            )
            .map_err(|error| journal_error("LSV4032", "failed to store merge group", error))?;

        Ok(())
    }
    fn insert_branch(
        &mut self,
        group_token: &ContinuationToken,
        request: &EffectRequest,
        position: usize,
        encoded: &EncodedMergeBranch<'_>,
    ) -> Result<(), Fault> {
        let (name, image_bytes, request_bytes) = encoded;
        let position = i64::try_from(position)
            .map_err(|_| journal_fault("LSV4032", "merge branch position is invalid"))?;
        self.0
            .execute(
                "INSERT INTO vm_effects(token, state, image, deadline_at_ms, terminal_step)
                     VALUES (?1, 'pending', ?2, ?3, NULL)",
                params![
                    request.continuation.token.as_str(),
                    image_bytes,
                    request
                        .continuation
                        .deadline_at_ms
                        .map(i64::try_from)
                        .transpose()
                        .map_err(|_| {
                            journal_fault("LSV4032", "effect deadline is out of range")
                        })?
                ],
            )
            .map_err(|error| {
                journal_error("LSV4032", "failed to store merge branch effect", error)
            })?;
        self.0
            .execute(
                "INSERT INTO vm_dispatches(token, request, state, attempt, lease_expires_at_ms)
                     VALUES (?1, ?2, 'ready', 0, NULL)",
                params![request.continuation.token.as_str(), request_bytes],
            )
            .map_err(|error| {
                journal_error("LSV4032", "failed to store merge branch dispatch", error)
            })?;
        self.0
            .execute(
                "INSERT INTO vm_merge_branches(
                       group_token, branch_token, branch_name, position
                     ) VALUES (?1, ?2, ?3, ?4)",
                params![
                    group_token.as_str(),
                    request.continuation.token.as_str(),
                    name,
                    position
                ],
            )
            .map_err(|error| journal_error("LSV4032", "failed to link merge branch", error))?;

        Ok(())
    }
    fn advance_watermark(&mut self, sequence: u64) -> Result<(), Fault> {
        advance_sequence_watermark(&self.0, sequence).map(|_| ())
    }
    fn commit(self) -> Result<(), Fault> {
        self.0
            .commit()
            .map_err(|error| journal_error("LSV4032", "failed to commit merge graph", error))
    }
}
