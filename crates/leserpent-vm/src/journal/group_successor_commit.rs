use super::*;
use crate::group_binding::invalid;
use leselang_command::reference_group_successor_commit::{
    CaptureProgress, GroupSuccessorOwner, MemoryGroupSuccessor, MemorySuccessorGroup,
};
#[cfg(feature = "sqlite-journal")]
use leselang_command::reference_group_successor_commit::{
    DurableGroupSuccessor, SuccessorPlanEncoding,
};

impl GroupSuccessorOwner for EphemeralJournal {
    fn requests(&mut self, owner: &ContinuationToken) -> Result<Vec<EffectRequest>, Fault> {
        self.merge_groups
            .get(owner)
            .ok_or_else(invalid)?
            .branch_tokens
            .iter()
            .map(|token| {
                self.dispatches
                    .get(token)
                    .map(|d| d.request.clone())
                    .ok_or_else(invalid)
            })
            .collect()
    }
    fn prepare(
        &mut self,
        owner: &ContinuationToken,
        plan: &MergePlan,
        requests: &[EffectRequest],
        value: &crate::Value,
    ) -> Result<CaptureProgress, Fault> {
        group_binding::prepare_admission(owner, plan, requests, value)
    }
    fn validate_graph(
        &mut self,
        owner: &ContinuationToken,
        plan: &MergePlan,
        graph: &[(&str, &EffectRequest)],
    ) -> Result<(), Fault> {
        validate_merge_graph_input(owner, plan, graph)
    }
    fn encode_request(&mut self, request: &EffectRequest) -> Result<Vec<u8>, Fault> {
        encode_json_capped(request, MAX_JOURNAL_ENTRY_BYTES, "group successor request")
    }
}
impl MemoryGroupSuccessor for EphemeralJournal {
    fn contains_dispatch(&self, token: &ContinuationToken) -> bool {
        self.dispatches.contains_key(token)
    }
    fn contains_group(&self, token: &ContinuationToken) -> bool {
        self.merge_groups.contains_key(token)
    }
    fn insert_dispatch(&mut self, token: ContinuationToken, request: EffectRequest) {
        self.dispatches.insert(
            token,
            EphemeralDispatch {
                request,
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
    fn group_mut(&mut self, owner: &ContinuationToken) -> Option<MemorySuccessorGroup<'_>> {
        self.merge_groups
            .get_mut(owner)
            .map(|g| MemorySuccessorGroup {
                plan: &mut g.plan,
                branches: &mut g.branch_tokens,
            })
    }
}

#[cfg(feature = "sqlite-journal")]
pub(super) struct SqliteGroupSuccessor<'a, 'connection>(pub &'a rusqlite::Transaction<'connection>);
#[cfg(feature = "sqlite-journal")]
impl GroupSuccessorOwner for SqliteGroupSuccessor<'_, '_> {
    fn requests(&mut self, owner: &ContinuationToken) -> Result<Vec<EffectRequest>, Fault> {
        let mut statement = self.0.prepare(
            "SELECT d.request FROM vm_merge_branches b JOIN vm_dispatches d ON d.token = b.branch_token
             WHERE b.group_token = ?1 ORDER BY b.position"
        ).map_err(|e| journal_error("LSV4033", "failed to prepare group successor", e))?;
        let requests = statement
            .query_map([owner.as_str()], |r| r.get::<_, Vec<u8>>(0))
            .map_err(|e| journal_error("LSV4033", "failed to load group authority", e))?
            .map(|row| {
                decode_bounded::<EffectRequest>(
                    &row.map_err(|e| journal_error("LSV4033", "invalid group authority", e))?,
                    MAX_JOURNAL_ENTRY_BYTES,
                )
            })
            .collect::<Result<Vec<_>, Fault>>()?;
        drop(statement);
        Ok(requests)
    }
    fn prepare(
        &mut self,
        owner: &ContinuationToken,
        plan: &MergePlan,
        requests: &[EffectRequest],
        value: &crate::Value,
    ) -> Result<CaptureProgress, Fault> {
        group_binding::prepare_admission(owner, plan, requests, value)
    }
    fn validate_graph(
        &mut self,
        owner: &ContinuationToken,
        plan: &MergePlan,
        graph: &[(&str, &EffectRequest)],
    ) -> Result<(), Fault> {
        validate_merge_graph_input(owner, plan, graph)
    }
    fn encode_request(&mut self, request: &EffectRequest) -> Result<Vec<u8>, Fault> {
        encode_json_capped(request, MAX_JOURNAL_ENTRY_BYTES, "group successor request")
    }
}
#[cfg(feature = "sqlite-journal")]
impl DurableGroupSuccessor for SqliteGroupSuccessor<'_, '_> {
    fn record_count(&mut self) -> Result<i64, Fault> {
        self.0
            .query_row(
                "SELECT (SELECT COUNT(*) FROM vm_effects) + (SELECT COUNT(*) FROM vm_merge_groups)",
                [],
                |r| r.get(0),
            )
            .map_err(|e| journal_error("LSV4033", "failed to bound group successor", e))
    }
    fn encode_image(&mut self, image: &ContinuationImage) -> Result<Vec<u8>, Fault> {
        encode_json_capped(image, MAX_CONTINUATION_BYTES, "group successor image")
    }
    fn encode_plan(
        &mut self,
        plan: &MergePlan,
        kind: SuccessorPlanEncoding,
    ) -> Result<Vec<u8>, Fault> {
        encode_json_capped(
            plan,
            MAX_CONTINUATION_BYTES,
            match kind {
                SuccessorPlanEncoding::Next => "group successor plan",
                SuccessorPlanEncoding::Prefix => "group prefix plan",
            },
        )
    }
    fn ensure_growth(&mut self, additional: usize) -> Result<(), Fault> {
        ensure_growth(self.0, additional)
    }
    fn insert_effect(
        &mut self,
        token: &str,
        image: &[u8],
        deadline: Option<i64>,
    ) -> Result<(), Fault> {
        self.0.execute("INSERT INTO vm_effects(token, state, image, deadline_at_ms, terminal_step) VALUES (?1, 'pending', ?2, ?3, NULL)", params![token, image, deadline]).map_err(|e| journal_error("LSV4033", "failed to create group successor", e))?;
        Ok(())
    }
    fn insert_dispatch(&mut self, token: &str, bytes: &[u8]) -> Result<(), Fault> {
        self.0.execute("INSERT INTO vm_dispatches(token, request, state, attempt, lease_expires_at_ms) VALUES (?1, ?2, 'ready', 0, NULL)", params![token, bytes]).map_err(|e| journal_error("LSV4033", "failed to create group successor dispatch", e))?;
        Ok(())
    }
    fn update_plan(&mut self, owner: &str, bytes: &[u8]) -> Result<usize, Fault> {
        self.0
            .execute(
                "UPDATE vm_merge_groups SET plan = ?2 WHERE token = ?1 AND state = 'pending'",
                params![owner, bytes],
            )
            .map_err(|e| journal_error("LSV4033", "failed to append group successor plan", e))
    }
    fn link_branch(
        &mut self,
        owner: &str,
        token: &str,
        name: &str,
        position: usize,
    ) -> Result<(), Fault> {
        self.0.execute("INSERT INTO vm_merge_branches(group_token, branch_token, branch_name, position) VALUES (?1, ?2, ?3, ?4)", params![owner, token, name, position as i64]).map_err(|e| journal_error("LSV4033", "failed to link group successor", e))?;
        Ok(())
    }
}
