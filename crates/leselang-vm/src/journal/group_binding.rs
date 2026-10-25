use super::*;
use crate::group_binding::invalid;
#[cfg(feature = "sqlite-journal")]
use leselang_command::reference_bound_records::{
    ReferenceBoundRecordProofs, inspect_bound_group_records,
};
#[cfg(feature = "sqlite-journal")]
use leselang_command::reference_group_reservations::{
    ReferenceReservationSource, inspect_group_reservations,
};
#[cfg(all(test, feature = "sqlite-journal"))]
#[path = "bound_record_contract_tests.rs"]
mod bound_record_contract_tests;
#[cfg(test)]
#[path = "group_successor_commit_contract_tests.rs"]
mod successor_commit_contract_tests;

#[cfg(test)]
pub(super) fn needs_successor(plan: &MergePlan) -> bool {
    plan.order.has_group_successor()
        && plan
            .result_binding
            .as_ref()
            .is_some_and(|binding| !binding.has_successor(plan))
}

pub(super) fn merge_prefix(
    plan: &MergePlan,
    completions: Vec<BranchCompletion>,
) -> Result<Step, Fault> {
    terminal_merge(plan, crate::merge_group_prefix(plan, completions))
}

pub(super) fn finish(plan: &MergePlan, completions: Vec<BranchCompletion>) -> Result<Step, Fault> {
    terminal_merge(
        plan,
        crate::merge_declared(plan, completions, DEFAULT_MAX_OUTPUT_ITEMS),
    )
}

use leselang_command::reference_merge_output::terminal_merge;

#[cfg(test)]
fn successor_plan(plan: &MergePlan) -> Result<MergePlan, Fault> {
    let mut next = plan.clone();
    let binding = next.result_binding.as_ref().ok_or_else(invalid)?;
    next.branches
        .push(binding.successor_name_at(plan.branches.len() - binding.branches()?.len())?);
    validate_merge_plan(&next)?;
    Ok(next)
}

pub(super) fn prepare_admission(
    owner: &ContinuationToken,
    plan: &MergePlan,
    requests: &[EffectRequest],
    value: &crate::Value,
) -> Result<crate::group_binding::CaptureProgress, Fault> {
    let binding = plan.result_binding.as_ref().ok_or_else(invalid)?;
    if binding.has_successor(plan) {
        binding.advance_capture(owner, requests, value)
    } else {
        binding.prepare_initial(owner, requests.first().ok_or_else(invalid)?, value)
    }
}

pub(super) fn admit_ephemeral(
    journal: &mut EphemeralJournal,
    owner: &ContinuationToken,
    plan: &MergePlan,
    prefix: &Step,
) -> Result<Option<Step>, Fault> {
    leselang_command::reference_group_successor_commit::admit_memory(journal, owner, plan, prefix)
}

#[cfg(feature = "sqlite-journal")]
pub(super) fn admit_successor(
    transaction: &rusqlite::Transaction<'_>,
    owner: &ContinuationToken,
    plan: &MergePlan,
    prefix: &Step,
) -> Result<Option<Step>, Fault> {
    leselang_command::reference_group_successor_commit::admit_durable(
        &mut super::group_successor_commit::SqliteGroupSuccessor(transaction),
        owner,
        plan,
        prefix,
    )
}

#[cfg(feature = "sqlite-journal")]
pub(super) fn validate_owner(
    connection: &Connection,
    image: &ContinuationImage,
) -> Result<(), Fault> {
    let Some(owner) = &image.group_result else {
        return Ok(());
    };
    let stored: Option<(String, Vec<u8>)> = connection
        .query_row(
            "SELECT g.token, g.plan FROM vm_merge_branches b
         JOIN vm_merge_groups g ON g.token = b.group_token WHERE b.branch_token = ?1",
            [image.token.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|error| journal_error("LSV4007", "failed to validate group owner", error))?;
    let (token, bytes) = stored.ok_or_else(invalid)?;
    let plan: MergePlan = decode_bounded(&bytes, MAX_CONTINUATION_BYTES)?;
    if token != owner.as_str() || plan.result_binding.is_none() {
        return Err(invalid());
    }
    Ok(())
}

#[cfg(feature = "sqlite-journal")]
pub(super) fn validate_records(
    connection: &Connection,
    token: &ContinuationToken,
    plan: &MergePlan,
    terminal: Option<&Step>,
) -> Result<(), Fault> {
    if plan.result_binding.is_none() {
        return Ok(());
    }
    let mut statement = connection
        .prepare(
            "SELECT b.branch_name, d.request, e.terminal_step
         FROM vm_merge_branches b
         LEFT JOIN vm_dispatches d ON d.token = b.branch_token
         JOIN vm_effects e ON e.token = b.branch_token
         WHERE b.group_token = ?1 ORDER BY b.position ASC",
        )
        .map_err(|error| journal_error("LSV4007", "failed to load bound group requests", error))?;
    let rows = statement
        .query_map([token.as_str()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<Vec<u8>>>(1)?,
                row.get::<_, Option<Vec<u8>>>(2)?,
            ))
        })
        .map_err(|error| journal_error("LSV4007", "failed to read bound group requests", error))?;
    inspect_bound_group_records(
        token,
        plan,
        rows.map(|row| {
            row.map_err(|error| journal_error("LSV4007", "invalid bound group record", error))
        }),
        terminal,
        &mut BoundRecordProofs(connection),
    )
}

#[cfg(feature = "sqlite-journal")]
struct BoundRecordProofs<'a>(&'a Connection);

#[cfg(feature = "sqlite-journal")]
impl ReferenceBoundRecordProofs for BoundRecordProofs<'_> {
    fn validate_capture(
        &mut self,
        binding: &crate::GroupResultBinding,
        request: &EffectRequest,
        fields: &[crate::StructuredField],
        operations: &[leselang_hir::host_call::HostOperation],
    ) -> Result<(), Fault> {
        binding.validate_saved_capture(request, fields, operations)
    }
    fn validate_pair(
        &mut self,
        previous: &EffectRequest,
        completed: &Step,
        request: &EffectRequest,
    ) -> Result<(), Fault> {
        crate::successor::validate_pair(previous, completed, request)
    }
    fn validate_member(
        &mut self,
        binding: &crate::GroupResultBinding,
        name: &str,
        value: &crate::Value,
    ) -> Result<(), Fault> {
        binding.validate_member(name, value)
    }
    fn validate_graph(
        &mut self,
        token: &ContinuationToken,
        plan: &MergePlan,
        requests: &[(&str, &EffectRequest)],
    ) -> Result<(), Fault> {
        validate_merge_graph_input(token, plan, requests)
    }
    fn validate_reservations(&mut self, plan: &MergePlan) -> Result<(), Fault> {
        inspect_group_reservations(plan, &mut GroupReservationSource(self.0))
    }
}

#[cfg(feature = "sqlite-journal")]
struct GroupReservationSource<'a>(&'a Connection);

#[cfg(feature = "sqlite-journal")]
impl ReferenceReservationSource for GroupReservationSource<'_> {
    fn allocation_watermark(&mut self) -> Result<i64, Fault> {
        self.0
            .query_row(
                "SELECT next_sequence FROM vm_metadata WHERE singleton = 1",
                [],
                |row| row.get(0),
            )
            .map_err(|error| {
                journal_error("LSV4007", "failed to validate successor reservation", error)
            })
    }

    fn effect_exists(&mut self, token: &str) -> Result<bool, Fault> {
        self.0
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM vm_effects WHERE token = ?1)",
                [token],
                |row| row.get(0),
            )
            .map_err(|error| {
                journal_error("LSV4007", "failed to validate reserved successor", error)
            })
    }

    fn group_exists(&mut self, token: &str) -> Result<bool, Fault> {
        self.0
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM vm_merge_groups WHERE token = ?1)",
                [token],
                |row| row.get(0),
            )
            .map_err(|error| {
                journal_error(
                    "LSV4007",
                    "failed to validate reserved group identity",
                    error,
                )
            })
    }
}
