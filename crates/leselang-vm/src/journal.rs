use crate::group_binding::ReferenceGroupResultBinding;
use crate::scheduling::ReferenceSchedulerLimits;
#[cfg(test)]
use crate::scheduling::ReferenceSchedulerPressure;
use std::collections::{BTreeMap, BTreeSet};
#[cfg(feature = "sqlite-journal")]
use std::{fs, path::Path, time::Duration};

#[cfg(test)]
use crate::MAX_DISPATCH_LEASE_MS;
#[cfg(all(test, feature = "sqlite-journal"))]
use crate::MAX_SEMANTIC_RETRIES;
#[cfg(feature = "sqlite-journal")]
use crate::validate_effect_error;
#[cfg(any(test, feature = "sqlite-journal"))]
use crate::validate_effect_request;
#[cfg(feature = "sqlite-journal")]
use crate::validate_image;
#[cfg(test)]
use crate::{Cancellation, CancellationReason};
#[cfg(feature = "sqlite-journal")]
use leselang_command::debugger_audit::command_origin_label;
#[cfg(test)]
use leselang_command::debugger_audit::{audit_matches_record, debugger_audit_record};
#[cfg(feature = "sqlite-journal")]
use leselang_command::reference_checkpoint::{ReferenceCheckpointSource, recover_checkpoint};
#[cfg(test)]
use leselang_command::reference_completion::deadline_cancellation;
use leselang_command::reference_completion::sequence_stop;
#[cfg(test)]
use leselang_command::reference_completion::terminal_step_outcome;
#[cfg(any(test, feature = "sqlite-journal"))]
use leselang_command::reference_completion::{validate_cancellation_step, validate_terminal_step};
#[cfg(test)]
use leselang_command::reference_dispatch::next_attempt;
#[cfg(test)]
pub(super) use leselang_command::reference_dispatch::validate_lease_clock;
#[cfg(test)]
use leselang_command::reference_dispatch::validate_proposed_retry;
use leselang_command::reference_dispatch::{
    lease_expiration, validate_dispatch_clock, validate_observed_lease,
};
#[cfg(feature = "sqlite-journal")]
use leselang_command::reference_dispatch_recovery::{
    ReferenceDispatchSource, inspect_dispatch_record,
};
#[cfg(feature = "sqlite-journal")]
use leselang_command::reference_group_recovery::{
    RecoveredMergeBranch, ReferenceGroupProofs, inspect_group_record,
};
#[cfg(feature = "sqlite-journal")]
use leselang_command::reference_recovery::{
    RecoveredEffectRecord, ReferenceRecordSource, decode_journal_value as decode_bounded,
    inspect_effect_record,
};
#[cfg(feature = "sqlite-journal")]
use leselang_runtime_core::dispatch_lease_capacity_exhausted;
use leselang_runtime_core::{DispatchLeaseObservation, GroupGraphError};
#[cfg(all(test, feature = "sqlite-journal"))]
use leselang_runtime_core::{
    GroupReceiptError, RecoveryRecord, StoredGroupState, validate_group_receipt,
};
#[cfg(feature = "sqlite-journal")]
use rusqlite::{
    Connection, OpenFlags, OptionalExtension, TransactionBehavior, limits::Limit, params,
};

use crate::{
    BranchCompletion, ContinuationImage, ContinuationToken, DEFAULT_MAX_OUTPUT_ITEMS,
    DebuggerAuditContext, DebuggerAuditRecord, DispatchClaim, DispatchLease, EffectError,
    EffectRequest, ExecutionOrder, Fault, MAX_CONTINUATION_BYTES, MAX_DISPATCH_ATTEMPTS, MergePlan,
    RetentionPolicy, RetryDisposition, SchedulerLimits, SchedulerPressure, Step,
    continuation_age_order, encode_json_capped, measure_json_capped, valid_continuation_token,
    validate_continuation_encoding_size, validate_merge_plan, validate_value,
};

mod acknowledgement_commit;
#[cfg(test)]
mod acknowledgement_commit_contract_tests;
mod audit_commit;
#[cfg(test)]
mod audit_commit_contract_tests;
#[cfg(feature = "sqlite-journal")]
mod audit_read;
#[cfg(all(test, feature = "sqlite-journal"))]
mod audit_read_contract_tests;
#[cfg(all(test, feature = "sqlite-journal"))]
mod audit_recovery_contract_tests;
#[cfg(all(test, feature = "sqlite-journal"))]
mod bounded_decode_tests;
mod cancellation_commit;
#[cfg(test)]
mod cancellation_commit_contract_tests;
mod claim_commit;
#[cfg(test)]
mod claim_commit_contract_tests;
mod completion_commit;
#[cfg(test)]
mod completion_commit_contract_tests;
mod expiration_commit;
#[cfg(test)]
mod expiration_commit_contract_tests;
mod group_binding;
mod group_finalization;
#[cfg(test)]
mod group_finalization_contract_tests;
mod group_progress;
#[cfg(test)]
mod group_progress_contract_tests;
mod group_successor_commit;
mod merge_graph_commit;
#[cfg(test)]
mod merge_graph_commit_contract_tests;
mod pending_commit;
#[cfg(test)]
mod pending_commit_contract_tests;
#[cfg(test)]
mod reference_completion_contract_tests;
#[cfg(test)]
mod retention_tests;
mod retry_commit;
#[cfg(test)]
mod retry_commit_contract_tests;
#[cfg(all(test, feature = "sqlite-journal"))]
mod schema_contract_tests;
mod successor;

pub use leselang_command::reference_checkpoint::MAX_JOURNAL_TOTAL_BYTES;
pub use leselang_command::reference_journal_schema::JOURNAL_SCHEMA_VERSION;
pub use leselang_command::reference_recovery::MAX_JOURNAL_ENTRY_BYTES;
pub use leselang_command::retention_policy::MAX_JOURNAL_RECORDS;
#[cfg(feature = "sqlite-journal")]
const JOURNAL_BUSY_TIMEOUT: Duration = Duration::from_secs(5);
#[cfg(feature = "sqlite-journal")]
const CLAIM_DISPATCH_SQL: &str =
    "SELECT d.token, d.request, d.attempt, d.retry_count, e.image
     FROM vm_dispatches d INDEXED BY vm_dispatch_attempt_order_idx
     JOIN vm_effects e ON e.token = d.token
     WHERE e.state = 'pending'
       AND d.state IN ('ready', 'leased')
       AND NOT EXISTS (
         SELECT 1 FROM vm_merge_branches own
         JOIN vm_merge_groups g ON g.token = own.group_token
         JOIN vm_merge_branches prior ON prior.group_token = own.group_token AND prior.position < own.position
         JOIN vm_effects predecessor ON predecessor.token = prior.branch_token
         WHERE own.branch_token = d.token AND g.execution_order = 'sequential'
           AND predecessor.state != 'completed'
       )
       AND ((d.state = 'ready' AND d.ready_at_ms <= ?1) OR
            (d.state = 'leased' AND d.lease_expires_at_ms <= ?1))
     ORDER BY d.attempt ASC, length(d.token) ASC, d.token ASC
     LIMIT 1";

#[cfg(feature = "sqlite-journal")]
pub(crate) struct JournalSnapshot {
    pub pending: BTreeMap<ContinuationToken, ContinuationImage>,
    pub completed: BTreeMap<ContinuationToken, Step>,
    pub next_sequence: u64,
}

pub(crate) use leselang_command::reference_settlement::maintenance::JournalCompaction;

pub(crate) use leselang_command::reference_settlement::visibility::{
    EffectObservation, MergeProgress,
};

pub(crate) enum Journal {
    Ephemeral(EphemeralJournal),
    #[cfg(feature = "sqlite-journal")]
    Sqlite(SqliteJournal),
}

#[cfg(test)]
#[path = "group_request_contract_tests.rs"]
mod group_request_contract_tests;

#[cfg(all(test, feature = "sqlite-journal"))]
#[path = "journal/group_recovery_contract_tests.rs"]
mod group_recovery_contract_tests;

#[cfg(all(test, feature = "sqlite-journal"))]
#[path = "journal/record_recovery_contract_tests.rs"]
mod record_recovery_contract_tests;

#[cfg(all(test, feature = "sqlite-journal"))]
#[path = "journal/dispatch_recovery_contract_tests.rs"]
mod dispatch_recovery_contract_tests;

#[cfg(all(test, feature = "sqlite-journal"))]
#[path = "journal/finalization_contract_tests.rs"]
mod finalization_contract_tests;

#[derive(Default)]
pub(crate) struct EphemeralJournal {
    dispatches: BTreeMap<ContinuationToken, EphemeralDispatch>,
    merge_groups: BTreeMap<ContinuationToken, EphemeralMergeGroup>,
    debugger_audits: BTreeMap<String, EphemeralDebuggerAudit>,
}

struct EphemeralDebuggerAudit {
    token: ContinuationToken,
    idempotency_key: String,
    principal_id: String,
    record: DebuggerAuditRecord,
}

struct EphemeralMergeGroup {
    plan: MergePlan,
    branch_tokens: Vec<ContinuationToken>,
    terminal_step: Option<Step>,
}

struct EphemeralDispatch {
    request: EffectRequest,
    attempt: u32,
    lease_expires_at_ms: Option<u64>,
    ready_at_ms: u64,
    retry_count: u32,
    last_error: Option<EffectError>,
    acknowledged: bool,
    terminal_step: Option<Step>,
}

enum EphemeralCompactionUnit {
    Effect(ContinuationToken),
    Merge {
        group_token: ContinuationToken,
        age_token: ContinuationToken,
    },
}

impl EphemeralCompactionUnit {
    fn age_token(&self) -> &ContinuationToken {
        match self {
            Self::Effect(token) => token,
            Self::Merge { age_token, .. } => age_token,
        }
    }
}

#[cfg(feature = "sqlite-journal")]
pub(crate) struct SqliteJournal {
    connection: Connection,
}

#[cfg(feature = "sqlite-journal")]
struct JournalRecord {
    state: String,
    image: Vec<u8>,
    terminal_step: Option<Vec<u8>>,
}

#[cfg(feature = "sqlite-journal")]
use leselang_command::reference_recovery::StoredDispatch as DispatchRecord;

#[cfg(feature = "sqlite-journal")]
type RawDispatchRow = (Vec<u8>, String, i64, Option<i64>, i64, i64, Option<Vec<u8>>);

impl Journal {
    pub fn ephemeral() -> Self {
        Self::Ephemeral(EphemeralJournal::default())
    }

    #[cfg(feature = "sqlite-journal")]
    pub fn open(path: &Path) -> Result<(Self, JournalSnapshot), Fault> {
        let mut journal = SqliteJournal::open(path)?;
        let snapshot = journal.load()?;
        Ok((Self::Sqlite(journal), snapshot))
    }

    pub fn allocate_sequence(&mut self, local_next: u64) -> Result<u64, Fault> {
        match self {
            Self::Ephemeral(_) => Ok(local_next),
            #[cfg(feature = "sqlite-journal")]
            Self::Sqlite(journal) => journal.allocate_sequence(local_next),
        }
    }

    pub fn record_pending(
        &mut self,
        _image: &ContinuationImage,
        request: Option<&EffectRequest>,
        limits: SchedulerLimits,
    ) -> Result<(), Fault> {
        match self {
            Self::Ephemeral(journal) => journal.record_pending(request, limits),
            #[cfg(feature = "sqlite-journal")]
            Self::Sqlite(journal) => journal.record_pending(_image, request, limits),
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    #[cfg(feature = "sqlite-journal")]
    pub fn record_merge_graph(
        &mut self,
        group_token: &ContinuationToken,
        plan: &MergePlan,
        branches: &[(&str, &EffectRequest)],
    ) -> Result<(), Fault> {
        self.record_merge_graph_with_limits(group_token, plan, branches, SchedulerLimits::default())
    }

    pub fn record_merge_graph_with_limits(
        &mut self,
        group_token: &ContinuationToken,
        plan: &MergePlan,
        branches: &[(&str, &EffectRequest)],
        limits: SchedulerLimits,
    ) -> Result<(), Fault> {
        validate_merge_graph_input(group_token, plan, branches)?;
        match self {
            Self::Ephemeral(journal) => {
                journal.record_merge_graph(group_token, plan, branches, limits)
            }
            #[cfg(feature = "sqlite-journal")]
            Self::Sqlite(journal) => {
                journal.record_merge_graph(group_token, plan, branches, limits)
            }
        }
    }

    pub fn scheduler_pressure(
        &self,
        now_ms: u64,
        limits: SchedulerLimits,
    ) -> Result<SchedulerPressure, Fault> {
        validate_dispatch_clock(now_ms)?;
        match self {
            Self::Ephemeral(journal) => Ok(journal.scheduler_pressure(now_ms, limits)),
            #[cfg(feature = "sqlite-journal")]
            Self::Sqlite(journal) => sql_scheduler_pressure(&journal.connection, now_ms, limits),
        }
    }

    pub fn check_admission(&self, additional: usize, limits: SchedulerLimits) -> Result<(), Fault> {
        limits.check_admission(
            self.scheduler_pressure(0, limits)?.pending_dispatches,
            additional,
        )
    }

    pub fn claim_dispatch(
        &mut self,
        now_ms: u64,
        lease_ms: u64,
        limits: SchedulerLimits,
    ) -> Result<DispatchClaim, Fault> {
        let expires_at_ms = lease_expiration(now_ms, lease_ms)?;
        match self {
            Self::Ephemeral(journal) => journal.claim_dispatch(now_ms, expires_at_ms, limits),
            #[cfg(feature = "sqlite-journal")]
            Self::Sqlite(journal) => journal.claim_dispatch(now_ms, expires_at_ms, limits),
        }
    }

    pub fn record_completed(
        &mut self,
        image: &ContinuationImage,
        step: &Step,
    ) -> Result<Step, Fault> {
        match self {
            Self::Ephemeral(journal) => journal.record_completed(image, step),
            #[cfg(feature = "sqlite-journal")]
            Self::Sqlite(journal) => journal.record_completed(image, step),
        }
    }

    pub fn acknowledge_dispatch(
        &mut self,
        lease: &DispatchLease,
        now_ms: u64,
        step: &Step,
    ) -> Result<Step, Fault> {
        validate_dispatch_clock(now_ms)?;
        match self {
            Self::Ephemeral(journal) => journal.acknowledge_dispatch(lease, now_ms, step),
            #[cfg(feature = "sqlite-journal")]
            Self::Sqlite(journal) => journal.acknowledge_dispatch(lease, now_ms, step),
        }
    }

    pub fn cancel(&mut self, image: &ContinuationImage, step: &Step) -> Result<Step, Fault> {
        match self {
            Self::Ephemeral(journal) => journal.cancel(image, step),
            #[cfg(feature = "sqlite-journal")]
            Self::Sqlite(journal) => journal.cancel(image, step),
        }
    }

    pub fn cancel_audited(
        &mut self,
        image: &ContinuationImage,
        step: &Step,
        audit: &DebuggerAuditContext,
        now_ms: u64,
    ) -> Result<(Step, DebuggerAuditRecord), Fault> {
        match self {
            Self::Ephemeral(journal) => journal.cancel_audited(image, step, audit, now_ms),
            #[cfg(feature = "sqlite-journal")]
            Self::Sqlite(journal) => journal.cancel_audited(image, step, audit, now_ms),
        }
    }

    pub fn debugger_audit(&self, command_id: &str) -> Result<Option<DebuggerAuditRecord>, Fault> {
        match self {
            Self::Ephemeral(journal) => Ok(journal
                .debugger_audits
                .get(command_id)
                .map(|entry| entry.record.clone())),
            #[cfg(feature = "sqlite-journal")]
            Self::Sqlite(journal) => journal.debugger_audit(command_id),
        }
    }

    pub fn expire_due(&mut self, now_ms: u64) -> Result<Vec<(ContinuationImage, Step)>, Fault> {
        match self {
            Self::Ephemeral(journal) => journal.expire_due(now_ms),
            #[cfg(feature = "sqlite-journal")]
            Self::Sqlite(journal) => journal.expire_due(now_ms),
        }
    }

    pub fn report_error(
        &mut self,
        lease: &DispatchLease,
        now_ms: u64,
        disposition: &RetryDisposition,
    ) -> Result<RetryDisposition, Fault> {
        match self {
            Self::Ephemeral(journal) => journal.report_error(lease, now_ms, disposition),
            #[cfg(feature = "sqlite-journal")]
            Self::Sqlite(journal) => journal.report_error(lease, now_ms, disposition),
        }
    }

    pub fn compact_completed(
        &mut self,
        local_completed: &[(ContinuationToken, Step)],
        policy: &RetentionPolicy,
    ) -> Result<JournalCompaction, Fault> {
        match self {
            Self::Ephemeral(journal) => journal.compact_completed(local_completed, policy),
            #[cfg(feature = "sqlite-journal")]
            Self::Sqlite(journal) => journal.compact_completed(local_completed, policy),
        }
    }

    pub fn completed_exists(&self, _token: &ContinuationToken) -> Result<bool, Fault> {
        match self {
            Self::Ephemeral(_) => Ok(true),
            #[cfg(feature = "sqlite-journal")]
            Self::Sqlite(journal) => journal.completed_exists(_token),
        }
    }

    pub fn observe_effect(&self, token: &ContinuationToken) -> Result<EffectObservation, Fault> {
        match self {
            Self::Ephemeral(journal) => Ok(match journal.dispatches.get(token) {
                None => EffectObservation::Untracked,
                Some(dispatch) => match &dispatch.terminal_step {
                    None => EffectObservation::Pending,
                    Some(step) => EffectObservation::Completed(Box::new(step.clone())),
                },
            }),
            #[cfg(feature = "sqlite-journal")]
            Self::Sqlite(journal) => {
                // A missing terminal alone cannot distinguish pending work from retention.
                let record = journal
                    .connection
                    .query_row(
                        "SELECT state, terminal_step FROM vm_effects WHERE token = ?1",
                        [token.as_str()],
                        |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<Vec<u8>>>(1)?)),
                    )
                    .optional()
                    .map_err(|error| {
                        journal_error("LSV4031", "failed to observe journal effect", error)
                    })?;
                let Some((state, terminal)) = record else {
                    return Ok(EffectObservation::Missing);
                };
                match (state.as_str(), terminal) {
                    ("pending", None) => Ok(EffectObservation::Pending),
                    ("completed", Some(bytes)) => {
                        let step: Step = decode_bounded(&bytes, MAX_JOURNAL_ENTRY_BYTES)?;
                        validate_terminal_step(&step)?;
                        Ok(EffectObservation::Completed(Box::new(step)))
                    }
                    _ => Err(journal_fault(
                        "LSV4007",
                        "effect has an invalid durable state",
                    )),
                }
            }
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn merge_result(&self, group_token: &ContinuationToken) -> Result<Option<Step>, Fault> {
        match self {
            Self::Ephemeral(journal) => Ok(journal.merge_result(group_token)),
            #[cfg(feature = "sqlite-journal")]
            Self::Sqlite(journal) => journal.merge_result(group_token),
        }
    }

    pub fn merge_progress(&self, branch_token: &ContinuationToken) -> Result<MergeProgress, Fault> {
        match self {
            Self::Ephemeral(journal) => Ok(journal.merge_progress(branch_token)),
            #[cfg(feature = "sqlite-journal")]
            Self::Sqlite(journal) => journal.merge_progress(branch_token),
        }
    }
}

impl EphemeralJournal {
    fn scheduler_pressure(&self, now_ms: u64, limits: SchedulerLimits) -> SchedulerPressure {
        let mut pressure = SchedulerPressure {
            pending_dispatches: 0,
            active_leases: 0,
            limits,
        };
        for dispatch in self
            .dispatches
            .values()
            .filter(|dispatch| !dispatch.acknowledged)
        {
            pressure.pending_dispatches += 1;
            if dispatch
                .lease_expires_at_ms
                .is_some_and(|expires_at| expires_at > now_ms)
            {
                pressure.active_leases += 1;
            }
        }
        pressure
    }

    fn record_pending(
        &mut self,
        request: Option<&EffectRequest>,
        limits: SchedulerLimits,
    ) -> Result<(), Fault> {
        leselang_command::reference_pending_commit::register_memory(self, request, limits)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    fn record_merge_graph(
        &mut self,
        group_token: &ContinuationToken,
        plan: &MergePlan,
        branches: &[(&str, &EffectRequest)],
        limits: SchedulerLimits,
    ) -> Result<(), Fault> {
        leselang_command::reference_merge_graph_commit::register_memory(
            self,
            group_token,
            plan,
            branches,
            limits,
        )
    }

    fn claim_dispatch(
        &mut self,
        now_ms: u64,
        lease_expires_at_ms: u64,
        limits: SchedulerLimits,
    ) -> Result<DispatchClaim, Fault> {
        leselang_command::reference_claim_commit::claim_memory(
            self,
            now_ms,
            lease_expires_at_ms,
            limits,
        )
    }

    fn record_completed(&mut self, image: &ContinuationImage, step: &Step) -> Result<Step, Fault> {
        leselang_command::reference_completion_commit::complete_memory(self, image, step)
    }

    fn acknowledge_dispatch(
        &mut self,
        lease: &DispatchLease,
        now_ms: u64,
        step: &Step,
    ) -> Result<Step, Fault> {
        leselang_command::reference_acknowledgement_commit::acknowledge_memory(
            self, lease, now_ms, step,
        )
    }

    fn cancel(&mut self, image: &ContinuationImage, step: &Step) -> Result<Step, Fault> {
        leselang_command::reference_cancellation_commit::cancel_memory(self, image, step)
    }

    fn cancel_audited(
        &mut self,
        image: &ContinuationImage,
        step: &Step,
        audit: &DebuggerAuditContext,
        now_ms: u64,
    ) -> Result<(Step, DebuggerAuditRecord), Fault> {
        leselang_command::reference_audit_commit::cancel_memory(self, image, step, audit, now_ms)
    }

    fn expire_due(&mut self, now_ms: u64) -> Result<Vec<(ContinuationImage, Step)>, Fault> {
        leselang_command::reference_expiration_commit::expire_memory(self, now_ms)
    }

    fn report_error(
        &mut self,
        lease: &DispatchLease,
        now_ms: u64,
        disposition: &RetryDisposition,
    ) -> Result<RetryDisposition, Fault> {
        leselang_command::reference_retry_commit::report_memory(self, lease, now_ms, disposition)
    }

    fn finalize_merge_group(&mut self, branch_token: &ContinuationToken) -> Result<(), Fault> {
        leselang_command::reference_group_finalization::finalize_memory(self, branch_token)
    }

    fn merge_result(&self, group_token: &ContinuationToken) -> Option<Step> {
        self.merge_groups
            .get(group_token)
            .and_then(|group| group.terminal_step.clone())
    }

    fn sequence_ready(&self, token: &ContinuationToken) -> bool {
        self.merge_groups.values().all(|group| {
            if !group.plan.order.is_sequential() {
                return true;
            }
            let Some(position) = group
                .branch_tokens
                .iter()
                .position(|branch| branch == token)
            else {
                return true;
            };
            group.terminal_step.is_none()
                && group.branch_tokens[..position].iter().all(|token| {
                    self.dispatches.get(token).is_some_and(|dispatch| {
                        matches!(dispatch.terminal_step, Some(Step::Done(_)))
                    })
                })
        })
    }

    fn merge_progress(&self, branch_token: &ContinuationToken) -> MergeProgress {
        leselang_command::reference_group_progress::read_memory(self, branch_token)
    }

    fn compact_completed(
        &mut self,
        local_completed: &[(ContinuationToken, Step)],
        policy: &RetentionPolicy,
    ) -> Result<JournalCompaction, Fault> {
        let protected = self
            .merge_groups
            .values()
            .flat_map(|group| group.branch_tokens.iter())
            .collect::<BTreeSet<_>>();
        let mut units = local_completed
            .iter()
            .filter(|(token, _)| !protected.contains(token))
            .map(|(token, _)| EphemeralCompactionUnit::Effect(token.clone()))
            .collect::<Vec<_>>();
        for (token, group) in &self.merge_groups {
            if group.terminal_step.is_some() {
                let age_token = group.branch_tokens.first().ok_or_else(|| {
                    journal_fault("LSV4007", "completed merge group has no branches")
                })?;
                units.push(EphemeralCompactionUnit::Merge {
                    group_token: token.clone(),
                    age_token: age_token.clone(),
                });
            }
        }
        units.sort_by(|left, right| continuation_age_order(left.age_token(), right.age_token()));
        let completed_count = units.len();
        let remove_count = policy.removal_count(completed_count);
        let mut reclaimed_logical_bytes = 0usize;
        let mut removed_tokens = Vec::with_capacity(remove_count);
        // Preflight every selected unit before mutating any map. Counting shares
        // the wire encoder's bounds without materializing disposable JSON buffers.
        for unit in units.iter().take(remove_count) {
            let tokens = match unit {
                EphemeralCompactionUnit::Effect(token) => std::slice::from_ref(token),
                EphemeralCompactionUnit::Merge { group_token, .. } => {
                    let group = self.merge_groups.get(group_token).ok_or_else(|| {
                        journal_fault("LSV4033", "selected merge group disappeared")
                    })?;
                    let terminal_step = group.terminal_step.as_ref().ok_or_else(|| {
                        journal_fault("LSV4007", "selected merge group is not completed")
                    })?;
                    reclaimed_logical_bytes = reclaimed_logical_bytes
                        .saturating_add(measure_json_capped(
                            &group.plan,
                            MAX_CONTINUATION_BYTES,
                            "merge plan",
                        )?)
                        .saturating_add(measure_json_capped(
                            terminal_step,
                            MAX_JOURNAL_ENTRY_BYTES,
                            "merge result",
                        )?);
                    group.branch_tokens.as_slice()
                }
            };
            for token in tokens {
                if let Some((_, step)) = local_completed.iter().find(|(local, _)| local == token) {
                    reclaimed_logical_bytes = reclaimed_logical_bytes.saturating_add(
                        measure_json_capped(step, MAX_JOURNAL_ENTRY_BYTES, "completed step")?,
                    );
                }
                if let Some(dispatch) = self.dispatches.get(token) {
                    reclaimed_logical_bytes =
                        reclaimed_logical_bytes.saturating_add(measure_json_capped(
                            &dispatch.request,
                            MAX_JOURNAL_ENTRY_BYTES,
                            "effect request",
                        )?);
                    if let Some(error) = &dispatch.last_error {
                        reclaimed_logical_bytes = reclaimed_logical_bytes.saturating_add(
                            measure_json_capped(error, MAX_JOURNAL_ENTRY_BYTES, "effect error")?,
                        );
                    }
                }
                removed_tokens.push(token.clone());
            }
        }
        for unit in units.into_iter().take(remove_count) {
            if let EphemeralCompactionUnit::Merge { group_token, .. } = unit {
                self.merge_groups.remove(&group_token);
            }
        }
        for token in &removed_tokens {
            self.dispatches.remove(token);
        }
        if !removed_tokens.is_empty() {
            // Match SQLite's audit cascade once per batch, retaining live group members.
            self.debugger_audits
                .retain(|_, audit| self.dispatches.contains_key(&audit.token));
        }
        Ok(JournalCompaction {
            removed_records: remove_count,
            pruned_tokens: removed_tokens,
            remaining_completed: completed_count.saturating_sub(remove_count),
            reclaimed_logical_bytes,
        })
    }
}

fn validate_ephemeral_lease(
    dispatch: &EphemeralDispatch,
    lease: &DispatchLease,
    now_ms: u64,
) -> Result<(), Fault> {
    validate_observed_lease(
        lease,
        now_ms,
        DispatchLeaseObservation {
            same_request: dispatch.request == lease.request,
            active: !dispatch.acknowledged,
            attempt: dispatch.attempt,
            retry_count: dispatch.retry_count,
            ready_at_ms: dispatch.ready_at_ms,
            expires_at_ms: dispatch.lease_expires_at_ms,
        },
    )
}

#[cfg(feature = "sqlite-journal")]
fn validate_stored_lease(
    dispatch: &DispatchRecord,
    lease: &DispatchLease,
    now_ms: u64,
    same_request: bool,
) -> Result<(), Fault> {
    validate_observed_lease(
        lease,
        now_ms,
        DispatchLeaseObservation {
            same_request,
            active: dispatch.state == "leased",
            attempt: dispatch.attempt,
            retry_count: dispatch.retry_count,
            ready_at_ms: dispatch.ready_at_ms,
            expires_at_ms: dispatch.lease_expires_at_ms,
        },
    )
}

#[cfg(feature = "sqlite-journal")]
impl SqliteJournal {
    fn open(path: &Path) -> Result<Self, Fault> {
        if path.as_os_str().is_empty() {
            return Err(journal_fault("LSV4001", "journal path is empty"));
        }
        if fs::symlink_metadata(path)
            .map(|metadata| metadata.file_type().is_symlink())
            .unwrap_or(false)
        {
            return Err(journal_fault(
                "LSV4001",
                "journal path must not be a symbolic link",
            ));
        }
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        if !parent.is_dir() {
            return Err(journal_fault(
                "LSV4001",
                "journal parent directory does not exist",
            ));
        }
        let file_name = path
            .file_name()
            .ok_or_else(|| journal_fault("LSV4001", "journal path must include a file name"))?;
        let canonical_parent = fs::canonicalize(parent)
            .map_err(|error| journal_error("LSV4001", "failed to resolve journal parent", error))?;
        let resolved_path = canonical_parent.join(file_name);
        let path = resolved_path.as_path();

        if !path.exists() {
            create_private_journal(path)?;
        }

        let flags = OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_NO_MUTEX
            | OpenFlags::SQLITE_OPEN_NOFOLLOW;
        let connection = Connection::open_with_flags(path, flags)
            .map_err(|error| journal_error("LSV4002", "failed to open journal", error))?;
        tighten_permissions(path)?;
        let sqlite_length_limit =
            i32::try_from(MAX_JOURNAL_ENTRY_BYTES + MAX_CONTINUATION_BYTES + 4096)
                .map_err(|_| journal_fault("LSV4002", "journal SQLite length limit is invalid"))?;
        connection
            .set_limit(Limit::SQLITE_LIMIT_LENGTH, sqlite_length_limit)
            .map_err(|error| journal_error("LSV4002", "failed to bound journal", error))?;
        connection
            .busy_timeout(JOURNAL_BUSY_TIMEOUT)
            .map_err(|error| journal_error("LSV4002", "failed to configure journal", error))?;
        connection
            .execute_batch(
                "PRAGMA journal_mode = DELETE;
                 PRAGMA synchronous = FULL;
                 PRAGMA foreign_keys = ON;
                 PRAGMA secure_delete = ON;
                 PRAGMA trusted_schema = OFF;",
            )
            .map_err(|error| journal_error("LSV4002", "failed to configure journal", error))?;

        let version: u32 = connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .map_err(|error| journal_error("LSV4003", "failed to read journal version", error))?;
        leselang_command::reference_journal_schema::migrate(version, |sql, context| {
            connection
                .execute_batch(sql)
                .map_err(|error| journal_error("LSV4003", context, error))
        })?;

        Ok(Self { connection })
    }

    fn load(&mut self) -> Result<JournalSnapshot, Fault> {
        // Validation and allocator repair must observe the same complete graph.
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| journal_error("LSV4005", "failed to lock journal load", error))?;
        let (snapshot, next_sequence) = recover_checkpoint(SqliteRecoveryFinalizer {
            transaction: Some(transaction),
        })?;
        let (pending, completed) = snapshot.into_parts();
        Ok(JournalSnapshot {
            pending,
            completed,
            next_sequence,
        })
    }

    fn completed_exists(&self, token: &ContinuationToken) -> Result<bool, Fault> {
        self.connection
            .query_row(
                "SELECT EXISTS(
                   SELECT 1 FROM vm_effects WHERE token = ?1 AND state = 'completed'
                 )",
                [token.as_str()],
                |row| row.get(0),
            )
            .map_err(|error| {
                journal_error(
                    "LSV4031",
                    "failed to verify completed journal record",
                    error,
                )
            })
    }

    fn merge_result(&self, group_token: &ContinuationToken) -> Result<Option<Step>, Fault> {
        let record = self
            .connection
            .query_row(
                "SELECT state, terminal_step FROM vm_merge_groups WHERE token = ?1",
                [group_token.as_str()],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<Vec<u8>>>(1)?)),
            )
            .optional()
            .map_err(|error| journal_error("LSV4033", "failed to read merge result", error))?;
        let Some((state, terminal)) = record else {
            return Ok(None);
        };
        match (state.as_str(), terminal) {
            ("pending", None) => Ok(None),
            ("completed", Some(bytes)) => {
                let step: Step = decode_bounded(&bytes, MAX_JOURNAL_ENTRY_BYTES)?;
                validate_terminal_step(&step)?;
                Ok(Some(step))
            }
            _ => Err(journal_fault(
                "LSV4007",
                "merge result has an invalid durable state",
            )),
        }
    }

    fn merge_progress(&self, branch_token: &ContinuationToken) -> Result<MergeProgress, Fault> {
        // Progress, current request and completed prefix must describe the same commit.
        let transaction = self.connection.unchecked_transaction().map_err(|error| {
            journal_error("LSV4033", "failed to snapshot merge progress", error)
        })?;
        leselang_command::reference_group_progress::read_durable(
            &mut group_progress::SqliteGroupProgress(&transaction),
            branch_token,
        )
    }

    fn allocate_sequence(&mut self, local_next: u64) -> Result<u64, Fault> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| journal_error("LSV4008", "failed to lock journal", error))?;
        let stored: i64 = transaction
            .query_row(
                "SELECT next_sequence FROM vm_metadata WHERE singleton = 1",
                [],
                |row| row.get(0),
            )
            .map_err(|error| journal_error("LSV4008", "failed to allocate sequence", error))?;
        let stored = u64::try_from(stored)
            .map_err(|_| journal_fault("LSV4007", "journal sequence is invalid"))?;
        let sequence = stored.max(local_next);
        let next = leselang_runtime_core::SequenceWatermarkRequirement::after_observation(
            sequence,
            i64::MAX as u64,
        )
        .map_err(|_| journal_fault("LSV4009", "journal continuation sequence is exhausted"))?
        .required_watermark();
        let next = i64::try_from(next)
            .map_err(|_| journal_fault("LSV4009", "journal continuation sequence is exhausted"))?;
        transaction
            .execute(
                "UPDATE vm_metadata SET next_sequence = ?1 WHERE singleton = 1",
                [next],
            )
            .map_err(|error| journal_error("LSV4008", "failed to store sequence", error))?;
        transaction
            .commit()
            .map_err(|error| journal_error("LSV4008", "failed to commit sequence", error))?;
        Ok(sequence)
    }

    fn record_pending(
        &mut self,
        image: &ContinuationImage,
        request: Option<&EffectRequest>,
        limits: SchedulerLimits,
    ) -> Result<(), Fault> {
        validate_image(image)?;
        if let Some(request) = request {
            validate_effect_request(request)?;
            if &request.continuation != image {
                return Err(journal_fault(
                    "LSV4011",
                    "effect request continuation does not match pending image",
                ));
            }
        }
        let image_bytes = encode_json_capped(image, MAX_CONTINUATION_BYTES, "continuation")?;
        let sequence = crate::canonical_continuation_sequence(&image.token)
            .ok_or_else(|| journal_fault("LSV4007", "journal continuation sequence is invalid"))?;
        let request_bytes = request
            .map(|request| encode_json_capped(request, MAX_JOURNAL_ENTRY_BYTES, "effect request"))
            .transpose()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| journal_error("LSV4010", "failed to lock journal", error))?;
        leselang_command::reference_pending_commit::register_durable(
            pending_commit::SqlitePendingTransaction(transaction),
            image,
            request,
            limits,
            leselang_command::reference_pending_commit::PendingEncoding {
                image: &image_bytes,
                request: request_bytes.as_deref(),
                sequence,
            },
        )
    }

    #[cfg_attr(not(test), allow(dead_code))]
    fn record_merge_graph(
        &mut self,
        group_token: &ContinuationToken,
        plan: &MergePlan,
        branches: &[(&str, &EffectRequest)],
        limits: SchedulerLimits,
    ) -> Result<(), Fault> {
        let plan_bytes = encode_json_capped(plan, MAX_CONTINUATION_BYTES, "merge plan")?;
        let encoded = branches
            .iter()
            .map(|(name, request)| {
                Ok((
                    *name,
                    encode_json_capped(
                        &request.continuation,
                        MAX_CONTINUATION_BYTES,
                        "continuation",
                    )?,
                    encode_json_capped(request, MAX_JOURNAL_ENTRY_BYTES, "effect request")?,
                ))
            })
            .collect::<Result<Vec<_>, Fault>>()?;
        let additional_bytes = plan_bytes.len()
            + encoded
                .iter()
                .map(|(_, image, request)| image.len().saturating_add(request.len()))
                .sum::<usize>();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| journal_error("LSV4032", "failed to lock merge graph", error))?;
        leselang_command::reference_merge_graph_commit::register_durable(
            merge_graph_commit::SqliteMergeGraphTransaction(transaction),
            group_token,
            plan,
            branches,
            limits,
            leselang_command::reference_merge_graph_commit::MergeGraphEncoding {
                plan: &plan_bytes,
                branches: &encoded,
                additional_bytes,
            },
        )
    }

    fn claim_dispatch(
        &mut self,
        now_ms: u64,
        expires_at: u64,
        limits: SchedulerLimits,
    ) -> Result<DispatchClaim, Fault> {
        let now = i64::try_from(now_ms)
            .map_err(|_| journal_fault("LSV4015", "dispatch clock is out of range"))?;
        let expires_at_sql = i64::try_from(expires_at)
            .map_err(|_| journal_fault("LSV4015", "dispatch lease expiration is out of range"))?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| journal_error("LSV4016", "failed to lock dispatch journal", error))?;
        leselang_command::reference_claim_commit::claim_durable(
            claim_commit::SqliteClaimTransaction {
                transaction,
                now,
                expires_at: expires_at_sql,
            },
            now_ms,
            expires_at,
            limits,
        )
    }

    fn record_completed(&mut self, image: &ContinuationImage, step: &Step) -> Result<Step, Fault> {
        validate_image(image)?;
        validate_terminal_step(step)?;
        let image_bytes = encode_json_capped(image, MAX_CONTINUATION_BYTES, "continuation")?;
        let step_bytes = encode_json_capped(step, MAX_JOURNAL_ENTRY_BYTES, "terminal step")?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| journal_error("LSV4012", "failed to lock journal", error))?;
        leselang_command::reference_completion_commit::complete_durable(
            completion_commit::SqliteCompletionTransaction(transaction),
            image,
            step,
            &image_bytes,
            &step_bytes,
        )
    }

    fn acknowledge_dispatch(
        &mut self,
        lease: &DispatchLease,
        now_ms: u64,
        step: &Step,
    ) -> Result<Step, Fault> {
        validate_effect_request(&lease.request)?;
        validate_terminal_step(step)?;
        if lease.attempt == 0 || lease.attempt > MAX_DISPATCH_ATTEMPTS {
            return Err(journal_fault("LSV4022", "dispatch attempt is invalid"));
        }
        let image = &lease.request.continuation;
        let image_bytes = encode_json_capped(image, MAX_CONTINUATION_BYTES, "continuation")?;
        encode_json_capped(&lease.request, MAX_JOURNAL_ENTRY_BYTES, "effect request")?;
        let step_bytes = encode_json_capped(step, MAX_JOURNAL_ENTRY_BYTES, "terminal step")?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| journal_error("LSV4020", "failed to lock dispatch journal", error))?;
        leselang_command::reference_acknowledgement_commit::acknowledge_durable(
            acknowledgement_commit::SqliteAcknowledgementTransaction(transaction),
            lease,
            now_ms,
            step,
            &image_bytes,
            &step_bytes,
        )
    }

    fn report_error(
        &mut self,
        lease: &DispatchLease,
        now_ms: u64,
        disposition: &RetryDisposition,
    ) -> Result<RetryDisposition, Fault> {
        validate_effect_request(&lease.request)?;
        let image = &lease.request.continuation;
        let image_bytes = encode_json_capped(image, MAX_CONTINUATION_BYTES, "continuation")?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| journal_error("LSV4026", "failed to lock retry journal", error))?;
        leselang_command::reference_retry_commit::report_durable(
            retry_commit::SqliteRetryTransaction(transaction),
            lease,
            now_ms,
            disposition,
            &image_bytes,
        )
    }

    fn cancel(&mut self, image: &ContinuationImage, step: &Step) -> Result<Step, Fault> {
        validate_image(image)?;
        validate_cancellation_step(image, step)?;
        let image_bytes = encode_json_capped(image, MAX_CONTINUATION_BYTES, "continuation")?;
        let step_bytes = encode_json_capped(step, MAX_JOURNAL_ENTRY_BYTES, "terminal step")?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| {
                journal_error("LSV4025", "failed to lock cancellation journal", error)
            })?;
        leselang_command::reference_cancellation_commit::cancel_durable(
            cancellation_commit::SqliteCancellationTransaction(transaction),
            image,
            step,
            &image_bytes,
            &step_bytes,
        )
    }

    fn cancel_audited(
        &mut self,
        image: &ContinuationImage,
        step: &Step,
        audit: &DebuggerAuditContext,
        now_ms: u64,
    ) -> Result<(Step, DebuggerAuditRecord), Fault> {
        validate_image(image)?;
        validate_cancellation_step(image, step)?;
        let image_bytes = encode_json_capped(image, MAX_CONTINUATION_BYTES, "continuation")?;
        let step_bytes = encode_json_capped(step, MAX_JOURNAL_ENTRY_BYTES, "terminal step")?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| {
                journal_error("LSV4032", "failed to lock debugger audit journal", error)
            })?;
        leselang_command::reference_audit_commit::cancel_durable(
            audit_commit::SqliteAuditTransaction(transaction),
            image,
            step,
            audit,
            now_ms,
            &image_bytes,
            &step_bytes,
        )
    }

    fn debugger_audit(&self, command_id: &str) -> Result<Option<DebuggerAuditRecord>, Fault> {
        self.connection
            .query_row(
                "SELECT command_id, principal_id, origin, session_id,
                        expected_revision, observed_at_ms
                 FROM vm_debugger_audit WHERE command_id = ?1",
                [command_id],
                audit_read::decode_record,
            )
            .optional()
            .map_err(|error| {
                journal_error(
                    "LSV4032",
                    "failed to read debugger cancellation audit",
                    error,
                )
            })
    }

    fn expire_due(&mut self, now_ms: u64) -> Result<Vec<(ContinuationImage, Step)>, Fault> {
        let now = i64::try_from(now_ms)
            .map_err(|_| journal_fault("LSV4015", "dispatch clock is out of range"))?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| journal_error("LSV4025", "failed to lock timeout journal", error))?;
        leselang_command::reference_expiration_commit::expire_durable(
            expiration_commit::SqliteExpirationTransaction { transaction, now },
            now_ms,
        )
    }

    fn compact_completed(
        &mut self,
        local_completed: &[(ContinuationToken, Step)],
        policy: &RetentionPolicy,
    ) -> Result<JournalCompaction, Fault> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| {
                journal_error("LSV4030", "failed to lock journal compaction", error)
            })?;
        let completed_count: i64 = transaction
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
            })?;
        let completed_count = usize::try_from(completed_count)
            .map_err(|_| journal_fault("LSV4007", "completed journal count is invalid"))?;
        let remove_count = policy.removal_count(completed_count);
        if remove_count == 0 {
            let pruned_tokens = stale_local_completed(&transaction, local_completed)?;
            transaction.commit().map_err(|error| {
                journal_error("LSV4030", "failed to close journal compaction", error)
            })?;
            return Ok(JournalCompaction {
                removed_records: 0,
                pruned_tokens,
                remaining_completed: completed_count,
                reclaimed_logical_bytes: 0,
            });
        }

        let limit = i64::try_from(remove_count)
            .map_err(|_| journal_fault("LSV4030", "journal compaction batch is invalid"))?;
        type RawCandidate = (i64, String);
        let candidates = {
            let mut statement = transaction
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
            rows.collect::<Result<Vec<RawCandidate>, _>>()
                .map_err(|error| {
                    journal_error("LSV4030", "failed to read journal compaction", error)
                })?
        };

        let mut removed_units = 0usize;
        let mut reclaimed_logical_bytes = 0usize;
        for (kind, token) in candidates {
            match kind {
                0 => {
                    let logical_bytes = completed_effect_logical_bytes(&transaction, &token)?;
                    let deleted = transaction
                        .execute(
                            "DELETE FROM vm_effects
                             WHERE token = ?1 AND state = 'completed'
                               AND NOT EXISTS (
                                 SELECT 1 FROM vm_merge_branches
                                 WHERE branch_token = vm_effects.token
                               )",
                            [&token],
                        )
                        .map_err(|error| {
                            journal_error(
                                "LSV4030",
                                "failed to delete completed journal record",
                                error,
                            )
                        })?;
                    if deleted != 1 {
                        return Err(journal_fault(
                            "LSV4030",
                            "completed journal record changed during compaction",
                        ));
                    }
                    reclaimed_logical_bytes = reclaimed_logical_bytes.saturating_add(logical_bytes);
                }
                1 => {
                    reclaimed_logical_bytes = reclaimed_logical_bytes
                        .saturating_add(delete_completed_merge_group(&transaction, &token)?);
                }
                _ => {
                    return Err(journal_fault(
                        "LSV4007",
                        "journal compaction candidate kind is invalid",
                    ));
                }
            }
            removed_units += 1;
        }
        let pruned_tokens = stale_local_completed(&transaction, local_completed)?;
        transaction.commit().map_err(|error| {
            journal_error("LSV4030", "failed to commit journal compaction", error)
        })?;
        Ok(JournalCompaction {
            remaining_completed: completed_count.saturating_sub(removed_units),
            removed_records: removed_units,
            pruned_tokens,
            reclaimed_logical_bytes,
        })
    }
}

#[cfg(feature = "sqlite-journal")]
fn recover_effect_record(
    connection: &Connection,
    row: &rusqlite::Row<'_>,
) -> Result<RecoveredEffectRecord, Fault> {
    let token: String = row
        .get(0)
        .map_err(|error| journal_error("LSV4005", "invalid journal token", error))?;
    let state: String = row
        .get(1)
        .map_err(|error| journal_error("LSV4005", "invalid journal state", error))?;
    inspect_effect_record(
        &mut SqliteRecoveryRecord { connection, row },
        &token,
        &state,
    )
}

#[cfg(feature = "sqlite-journal")]
struct SqliteRecoveryRecord<'a, 'row> {
    connection: &'a Connection,
    row: &'a rusqlite::Row<'row>,
}

#[cfg(feature = "sqlite-journal")]
impl ReferenceRecordSource for SqliteRecoveryRecord<'_, '_> {
    type Bytes = Vec<u8>;

    fn read_image(&mut self) -> Result<Vec<u8>, Fault> {
        self.row
            .get(2)
            .map_err(|error| journal_error("LSV4005", "invalid journal image", error))
    }
    fn read_deadline(&mut self) -> Result<Option<i64>, Fault> {
        self.row
            .get(3)
            .map_err(|error| journal_error("LSV4005", "invalid journal effect deadline", error))
    }
    fn validate_owner(&mut self, image: &ContinuationImage) -> Result<(), Fault> {
        group_binding::validate_owner(self.connection, image)
    }
    fn read_completed(&mut self) -> Result<Vec<u8>, Fault> {
        self.row
            .get(4)
            .map_err(|error| journal_error("LSV4005", "invalid completed journal record", error))
    }
    fn validate_successor(
        &mut self,
        image: &ContinuationImage,
        completed: Option<&Step>,
    ) -> Result<(), Fault> {
        successor::validate_record(self.connection, image, completed)
    }
}

#[cfg(feature = "sqlite-journal")]
fn completed_effect_logical_bytes(connection: &Connection, token: &str) -> Result<usize, Fault> {
    let bytes: i64 = connection
        .query_row(
            "SELECT length(e.image) + length(e.terminal_step) +
                    COALESCE(length(d.request), 0) +
                    COALESCE(length(d.last_error), 0)
             FROM vm_effects e
             LEFT JOIN vm_dispatches d ON d.token = e.token
             WHERE e.token = ?1 AND e.state = 'completed'",
            [token],
            |row| row.get(0),
        )
        .map_err(|error| {
            journal_error("LSV4030", "failed to size completed journal record", error)
        })?;
    usize::try_from(bytes)
        .map_err(|_| journal_fault("LSV4007", "journal compaction size is invalid"))
}

#[cfg(feature = "sqlite-journal")]
fn delete_completed_merge_group(
    transaction: &rusqlite::Transaction<'_>,
    group_token: &str,
) -> Result<usize, Fault> {
    let (group_bytes, plan_bytes): (i64, Vec<u8>) = transaction
        .query_row(
            "SELECT length(plan) + length(terminal_step), plan
             FROM vm_merge_groups WHERE token = ?1 AND state = 'completed'",
            [group_token],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|error| journal_error("LSV4030", "failed to size completed merge group", error))?;
    let branches = {
        let mut statement = transaction
            .prepare(
                "SELECT e.token,
                        length(e.image) + length(e.terminal_step) +
                        COALESCE(length(d.request), 0) +
                        COALESCE(length(d.last_error), 0),
                        e.state
                 FROM vm_merge_branches b
                 JOIN vm_effects e ON e.token = b.branch_token
                 LEFT JOIN vm_dispatches d ON d.token = e.token
                 WHERE b.group_token = ?1
                 ORDER BY b.position ASC",
            )
            .map_err(|error| {
                journal_error("LSV4030", "failed to prepare merge group deletion", error)
            })?;
        statement
            .query_map([group_token], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(|error| {
                journal_error("LSV4030", "failed to scan merge group deletion", error)
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| {
                journal_error("LSV4030", "failed to read merge group deletion", error)
            })?
    };
    let plan: MergePlan = decode_bounded(&plan_bytes, MAX_CONTINUATION_BYTES)?;
    validate_merge_plan(&plan)?;
    if branches.len() != plan.branches.len()
        || branches.iter().any(|(_, _, state)| state != "completed")
    {
        return Err(journal_fault(
            "LSV4030",
            "completed merge group has an incomplete branch set",
        ));
    }

    let mut reclaimed = usize::try_from(group_bytes)
        .map_err(|_| journal_fault("LSV4007", "merge group size is invalid"))?;
    for (_, bytes, _) in &branches {
        reclaimed = reclaimed.saturating_add(
            usize::try_from(*bytes)
                .map_err(|_| journal_fault("LSV4007", "merge branch size is invalid"))?,
        );
    }
    let deleted = transaction
        .execute(
            "DELETE FROM vm_merge_groups WHERE token = ?1 AND state = 'completed'",
            [group_token],
        )
        .map_err(|error| {
            journal_error("LSV4030", "failed to delete completed merge group", error)
        })?;
    if deleted != 1 {
        return Err(journal_fault(
            "LSV4030",
            "completed merge group changed during compaction",
        ));
    }
    for (token, _, _) in branches {
        let deleted = transaction
            .execute(
                "DELETE FROM vm_effects WHERE token = ?1 AND state = 'completed'",
                [&token],
            )
            .map_err(|error| journal_error("LSV4030", "failed to delete merge branch", error))?;
        if deleted != 1 {
            return Err(journal_fault(
                "LSV4030",
                "merge branch changed during compaction",
            ));
        }
    }
    Ok(reclaimed)
}

#[cfg(feature = "sqlite-journal")]
fn stale_local_completed(
    connection: &Connection,
    local_completed: &[(ContinuationToken, Step)],
) -> Result<Vec<ContinuationToken>, Fault> {
    let mut statement = connection
        .prepare("SELECT token FROM vm_effects WHERE state = 'completed'")
        .map_err(|error| {
            journal_error(
                "LSV4030",
                "failed to prepare completed journal reconciliation",
                error,
            )
        })?;
    let persisted = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|error| {
            journal_error(
                "LSV4030",
                "failed to scan completed journal reconciliation",
                error,
            )
        })?
        .collect::<Result<BTreeSet<_>, _>>()
        .map_err(|error| {
            journal_error(
                "LSV4030",
                "failed to read completed journal reconciliation",
                error,
            )
        })?;
    Ok(local_completed
        .iter()
        .filter(|(token, _)| !persisted.contains(token.as_str()))
        .map(|(token, _)| token.clone())
        .collect())
}

#[cfg(feature = "sqlite-journal")]
fn complete_terminal_record(
    transaction: &rusqlite::Transaction<'_>,
    token: &str,
    step_bytes: &[u8],
) -> Result<(), Fault> {
    transaction
        .execute(
            "UPDATE vm_effects SET state = 'completed', terminal_step = ?2 WHERE token = ?1",
            params![token, step_bytes],
        )
        .map_err(|error| journal_error("LSV4025", "failed to cancel effect", error))?;
    transaction
        .execute(
            "UPDATE vm_dispatches
             SET state = 'acknowledged', lease_expires_at_ms = NULL
             WHERE token = ?1",
            [token],
        )
        .map_err(|error| journal_error("LSV4025", "failed to close cancelled dispatch", error))?;
    Ok(())
}

#[cfg(all(test, feature = "sqlite-journal"))]
fn parse_command_origin(value: &str) -> rusqlite::Result<leselang_host_contract::CommandOrigin> {
    leselang_command::debugger_audit::parse_command_origin(value)
        .ok_or(rusqlite::Error::InvalidQuery)
}

#[cfg(feature = "sqlite-journal")]
fn load_debugger_audit(
    transaction: &rusqlite::Transaction<'_>,
    audit: &DebuggerAuditContext,
) -> Result<Option<leselang_command::reference_audit_read::StoredAudit>, Fault> {
    transaction
        .query_row(
            "SELECT command_id, continuation_token, idempotency_key,
                    principal_id, origin, session_id, expected_revision, observed_at_ms
             FROM vm_debugger_audit
             WHERE command_id = ?1 OR (principal_id = ?2 AND idempotency_key = ?3)",
            params![
                audit.command_id.as_str(),
                audit.principal.id.as_str(),
                audit.idempotency_key.as_str(),
            ],
            audit_read::decode_replay,
        )
        .optional()
        .map_err(|error| {
            journal_error(
                "LSV4032",
                "failed to load debugger cancellation audit",
                error,
            )
        })
}

#[cfg(feature = "sqlite-journal")]
fn finalize_merge_group(
    transaction: &rusqlite::Transaction<'_>,
    branch_token: &str,
) -> Result<(), Fault> {
    leselang_command::reference_group_finalization::finalize_durable(
        &mut group_finalization::SqliteGroupFinalization(transaction),
        branch_token,
    )
}

#[cfg(feature = "sqlite-journal")]
fn ensure_sequence_ready(connection: &Connection, token: &str) -> Result<(), Fault> {
    let blocked: bool = connection.query_row(
        "SELECT EXISTS (
           SELECT 1 FROM vm_merge_branches own
           JOIN vm_merge_groups g ON g.token = own.group_token
           JOIN vm_merge_branches prior ON prior.group_token = own.group_token AND prior.position < own.position
           JOIN vm_effects e ON e.token = prior.branch_token
           WHERE own.branch_token = ?1 AND g.execution_order = 'sequential' AND e.state != 'completed'
         )", [token], |row| row.get(0)
    ).map_err(|error| journal_error("LSV4033", "failed to check sequential predecessor", error))?;
    if blocked {
        return Err(journal_fault(
            "LSV2410",
            "sequential predecessor has not completed",
        ));
    }
    Ok(())
}

#[cfg(feature = "sqlite-journal")]
fn journal_payload_bytes(connection: &Connection) -> Result<usize, Fault> {
    let bytes: i64 = connection
        .query_row(
            "SELECT
               COALESCE((SELECT SUM(length(image) + COALESCE(length(terminal_step), 0))
                         FROM vm_effects), 0) +
               COALESCE((SELECT SUM(length(request) + COALESCE(length(last_error), 0))
                         FROM vm_dispatches), 0) +
               COALESCE((SELECT SUM(length(plan) + COALESCE(length(terminal_step), 0))
                         FROM vm_merge_groups), 0)",
            [],
            |row| row.get(0),
        )
        .map_err(|error| journal_error("LSV4005", "failed to size journal payload", error))?;
    usize::try_from(bytes).map_err(|_| journal_fault("LSV4007", "journal payload size is invalid"))
}

#[cfg(feature = "sqlite-journal")]
fn journal_record_count(connection: &Connection) -> Result<usize, Fault> {
    let count: i64 = connection
        .query_row(
            "SELECT
               (SELECT COUNT(*) FROM vm_effects) +
               (SELECT COUNT(*) FROM vm_merge_groups)",
            [],
            |row| row.get(0),
        )
        .map_err(|error| journal_error("LSV4005", "failed to count journal records", error))?;
    usize::try_from(count).map_err(|_| journal_fault("LSV4007", "journal record count is invalid"))
}

#[cfg(feature = "sqlite-journal")]
fn ensure_growth(transaction: &rusqlite::Transaction<'_>, additional: usize) -> Result<(), Fault> {
    let current = journal_payload_bytes(transaction)?;
    if current.saturating_add(additional) > MAX_JOURNAL_TOTAL_BYTES {
        return Err(journal_fault(
            "LSV4006",
            format!("journal payload limit {MAX_JOURNAL_TOTAL_BYTES} reached"),
        ));
    }
    Ok(())
}

#[cfg(feature = "sqlite-journal")]
fn load_record(
    transaction: &rusqlite::Transaction<'_>,
    token: &str,
) -> Result<Option<JournalRecord>, Fault> {
    transaction
        .query_row(
            "SELECT state, image, terminal_step FROM vm_effects WHERE token = ?1",
            [token],
            |row| {
                Ok(JournalRecord {
                    state: row.get(0)?,
                    image: row.get(1)?,
                    terminal_step: row.get(2)?,
                })
            },
        )
        .optional()
        .map_err(|error| journal_error("LSV4005", "failed to read journal record", error))
}

#[cfg(feature = "sqlite-journal")]
fn load_dispatch(transaction: &Connection, token: &str) -> Result<Option<DispatchRecord>, Fault> {
    transaction
        .query_row(
            "SELECT request, state, attempt, lease_expires_at_ms, ready_at_ms, retry_count,
                    last_error
             FROM vm_dispatches WHERE token = ?1",
            [token],
            |row| {
                let attempt: i64 = row.get(2)?;
                let expires: Option<i64> = row.get(3)?;
                let ready_at: i64 = row.get(4)?;
                let retry_count: i64 = row.get(5)?;
                let last_error: Option<Vec<u8>> = row.get(6)?;
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    attempt,
                    expires,
                    ready_at,
                    retry_count,
                    last_error,
                ))
            },
        )
        .optional()
        .map_err(|error| journal_error("LSV4005", "failed to read dispatch record", error))?
        .map(
            |(request, state, attempt, expires, ready_at, retry_count, last_error): RawDispatchRow| {
                let retry_count = u32::try_from(retry_count)
                    .map_err(|_| journal_fault("LSV4007", "dispatch retry count is invalid"))?;
                if (retry_count == 0) != last_error.is_none() {
                    return Err(journal_fault(
                        "LSV4007",
                        "dispatch retry history is inconsistent",
                    ));
                }
                if let Some(bytes) = last_error {
                    let error: EffectError = decode_bounded(&bytes, 2_048)?;
                    validate_effect_error(&error)?;
                }
                Ok(DispatchRecord {
                    request,
                    state,
                    attempt: u32::try_from(attempt)
                        .map_err(|_| journal_fault("LSV4007", "dispatch attempt is invalid"))?,
                    lease_expires_at_ms: expires
                        .map(|value| {
                            u64::try_from(value).map_err(|_| {
                                journal_fault("LSV4007", "dispatch lease expiration is invalid")
                            })
                        })
                        .transpose()?,
                    ready_at_ms: u64::try_from(ready_at)
                        .map_err(|_| journal_fault("LSV4007", "dispatch ready clock is invalid"))?,
                    retry_count,
                })
            },
        )
        .transpose()
}

#[cfg(feature = "sqlite-journal")]
fn validate_dispatch_records(connection: &Connection) -> Result<(), Fault> {
    let mut statement = connection
        .prepare(
            "SELECT d.token, d.request, d.state, d.attempt, d.lease_expires_at_ms,
                    d.ready_at_ms, d.retry_count, d.last_error, e.state, e.image
             FROM vm_dispatches d JOIN vm_effects e ON e.token = d.token
             ORDER BY d.token ASC",
        )
        .map_err(|error| journal_error("LSV4005", "failed to prepare dispatch load", error))?;
    let mut rows = statement
        .query([])
        .map_err(|error| journal_error("LSV4005", "failed to load dispatch records", error))?;
    while let Some(row) = rows
        .next()
        .map_err(|error| journal_error("LSV4005", "failed to read dispatch record", error))?
    {
        recover_dispatch_record(row)?;
    }
    Ok(())
}

#[cfg(feature = "sqlite-journal")]
fn recover_dispatch_record(row: &rusqlite::Row<'_>) -> Result<(), Fault> {
    let token: String = row
        .get(0)
        .map_err(|error| journal_error("LSV4005", "invalid dispatch token", error))?;
    inspect_dispatch_record(&mut SqliteRecoveryDispatch { row }, &token)
}

#[cfg(feature = "sqlite-journal")]
struct SqliteRecoveryDispatch<'a, 'row> {
    row: &'a rusqlite::Row<'row>,
}

#[cfg(feature = "sqlite-journal")]
impl ReferenceDispatchSource for SqliteRecoveryDispatch<'_, '_> {
    type Bytes = Vec<u8>;

    fn read_request(&mut self) -> Result<Vec<u8>, Fault> {
        self.row
            .get(1)
            .map_err(|error| journal_error("LSV4005", "invalid dispatch request", error))
    }
    fn read_image(&mut self) -> Result<Vec<u8>, Fault> {
        self.row
            .get(9)
            .map_err(|error| journal_error("LSV4005", "invalid dispatch continuation image", error))
    }
    fn read_metadata(
        &mut self,
    ) -> Result<leselang_runtime_core::StoredDispatchMetadata<Vec<u8>>, Fault> {
        let state: String = self
            .row
            .get(2)
            .map_err(|error| journal_error("LSV4005", "invalid dispatch state", error))?;
        let attempt: i64 = self
            .row
            .get(3)
            .map_err(|error| journal_error("LSV4005", "invalid dispatch attempt", error))?;
        let effect_state: String = self
            .row
            .get(8)
            .map_err(|error| journal_error("LSV4005", "invalid dispatch effect state", error))?;
        let ready_at: i64 = self
            .row
            .get(5)
            .map_err(|error| journal_error("LSV4005", "invalid dispatch ready clock", error))?;
        let retry_count: i64 = self
            .row
            .get(6)
            .map_err(|error| journal_error("LSV4005", "invalid dispatch retry count", error))?;
        let last_error: Option<Vec<u8>> = self
            .row
            .get(7)
            .map_err(|error| journal_error("LSV4005", "invalid dispatch retry error", error))?;
        Ok(leselang_runtime_core::StoredDispatchMetadata {
            attempt,
            ready_at_ms: ready_at,
            retry_count,
            last_error,
            acknowledged: state == "acknowledged",
            effect_completed: effect_state == "completed",
        })
    }
}

#[cfg(feature = "sqlite-journal")]
struct SqliteRecoveryFinalizer<'connection> {
    transaction: Option<rusqlite::Transaction<'connection>>,
}

#[cfg(feature = "sqlite-journal")]
impl SqliteRecoveryFinalizer<'_> {
    fn connection(&self) -> Result<&Connection, Fault> {
        self.transaction
            .as_deref()
            .ok_or_else(|| journal_fault("LSV4005", "journal load transaction is closed"))
    }
}

#[cfg(feature = "sqlite-journal")]
impl ReferenceCheckpointSource for SqliteRecoveryFinalizer<'_> {
    fn record_count(&mut self) -> Result<usize, Fault> {
        journal_record_count(self.connection()?)
    }

    fn payload_bytes(&mut self) -> Result<usize, Fault> {
        journal_payload_bytes(self.connection()?)
    }

    fn stage_records(
        &mut self,
        push: &mut dyn FnMut(RecoveredEffectRecord) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        let connection = self.connection()?;
        let mut statement = connection
            .prepare(
                "SELECT token, state, image, deadline_at_ms, terminal_step
                 FROM vm_effects ORDER BY token ASC",
            )
            .map_err(|error| journal_error("LSV4005", "failed to prepare journal load", error))?;
        let mut rows = statement
            .query([])
            .map_err(|error| journal_error("LSV4005", "failed to load journal", error))?;
        while let Some(row) = rows
            .next()
            .map_err(|error| journal_error("LSV4005", "failed to read journal record", error))?
        {
            push(recover_effect_record(connection, row)?)?;
        }
        Ok(())
    }
}

#[cfg(feature = "sqlite-journal")]
impl leselang_runtime_core::CheckpointFinalizer for SqliteRecoveryFinalizer<'_> {
    type Commit = u64;
    type Error = Fault;

    fn validate_dispatches(&mut self) -> Result<(), Fault> {
        validate_dispatch_records(self.connection()?)
    }
    fn validate_graphs(&mut self) -> Result<u64, Fault> {
        validate_merge_graph_records(self.connection()?)
    }
    fn validate_audits(&mut self) -> Result<(), Fault> {
        validate_debugger_audit_records(self.connection()?)
    }
    fn advance_watermark(&mut self, highest_sequence: u64) -> Result<u64, Fault> {
        advance_sequence_watermark(self.connection()?, highest_sequence)
    }
    fn commit(&mut self) -> Result<(), Fault> {
        let transaction = self
            .transaction
            .take()
            .ok_or_else(|| journal_fault("LSV4005", "journal load transaction is closed"))?;
        transaction
            .commit()
            .map_err(|error| journal_error("LSV4005", "failed to commit journal load", error))
    }
}

#[cfg(feature = "sqlite-journal")]
fn validate_debugger_audit_records(connection: &Connection) -> Result<(), Fault> {
    leselang_command::reference_audit_recovery::validate_audit_records(&mut SqliteAuditSource {
        connection,
    })
}

#[cfg(feature = "sqlite-journal")]
struct SqliteAuditSource<'a> {
    connection: &'a Connection,
}

#[cfg(feature = "sqlite-journal")]
impl leselang_command::reference_audit_recovery::ReferenceAuditSource for SqliteAuditSource<'_> {
    fn index_count(&mut self) -> Result<u32, Fault> {
        self.connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
             WHERE type = 'index' AND name = 'vm_debugger_audit_token_idx'",
                [],
                |row| row.get(0),
            )
            .map_err(|error| {
                journal_error("LSV4005", "failed to validate debugger audit schema", error)
            })
    }
    fn audit_count(&mut self) -> Result<u32, Fault> {
        self.connection
            .query_row("SELECT COUNT(*) FROM vm_debugger_audit", [], |row| {
                row.get(0)
            })
            .map_err(|error| journal_error("LSV4005", "failed to count debugger audits", error))
    }
    fn inspect_records(&mut self) -> Result<u32, Fault> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT a.command_id, a.principal_id, a.idempotency_key, a.origin,
                    a.session_id, a.expected_revision, a.observed_at_ms,
                    a.continuation_token, e.state, e.terminal_step
             FROM vm_debugger_audit a
             JOIN vm_effects e ON e.token = a.continuation_token
             ORDER BY a.command_id ASC",
            )
            .map_err(|error| {
                journal_error("LSV4005", "failed to prepare debugger audit load", error)
            })?;
        let mut rows = statement
            .query([])
            .map_err(|error| journal_error("LSV4005", "failed to load debugger audits", error))?;
        let mut validated_count = 0_u32;
        while let Some(row) = rows
            .next()
            .map_err(|error| journal_error("LSV4005", "failed to read debugger audit", error))?
        {
            leselang_command::reference_audit_recovery::inspect_audit_record(
                &mut SqliteAuditRow { row },
            )?;
            validated_count += 1;
        }
        Ok(validated_count)
    }
}

#[cfg(feature = "sqlite-journal")]
struct SqliteAuditRow<'a, 'row> {
    row: &'a rusqlite::Row<'row>,
}

#[cfg(feature = "sqlite-journal")]
impl leselang_command::reference_audit_recovery::ReferenceAuditRow for SqliteAuditRow<'_, '_> {
    type Bytes = Vec<u8>;
    type InvalidOrigin = rusqlite::Error;
    fn read_text(
        &mut self,
        column: leselang_command::reference_audit_recovery::AuditTextColumn,
    ) -> Result<String, Fault> {
        self.row
            .get(column.index())
            .map_err(|error| journal_error("LSV4005", column.error_context(), error))
    }
    fn read_integer(
        &mut self,
        column: leselang_command::reference_audit_recovery::AuditIntegerColumn,
    ) -> Result<i64, Fault> {
        self.row
            .get(column.index())
            .map_err(|error| journal_error("LSV4005", column.error_context(), error))
    }
    fn read_terminal(&mut self) -> Result<Vec<u8>, Fault> {
        self.row.get(9).map_err(|error| {
            journal_error("LSV4005", "invalid debugger audit terminal step", error)
        })
    }
    fn invalid_origin(&mut self) -> rusqlite::Error {
        rusqlite::Error::InvalidQuery
    }
}

struct MergeGraphHost;

impl leselang_command::MergeGraphAdmissionHost<ContinuationToken> for MergeGraphHost {
    type Error = Fault;
    fn token_text<'a>(&self, token: &'a ContinuationToken) -> &'a str {
        token.as_str()
    }
    fn valid_token(&self, token: &ContinuationToken) -> bool {
        valid_continuation_token(token)
    }
    fn canonical_sequence(&self, token: &ContinuationToken) -> Option<u64> {
        crate::canonical_continuation_sequence(token)
    }
    fn valid_group_name(&self, name: &str) -> bool {
        crate::valid_merge_branch_name(name)
    }
    fn validate_plan_encoding(&self, plan: &MergePlan) -> Result<(), Fault> {
        crate::group_binding::validate_plan_encoding(plan)
    }
}

#[cfg_attr(not(test), allow(dead_code))]
fn validate_merge_graph_input(
    group_token: &ContinuationToken,
    plan: &MergePlan,
    branches: &[(&str, &EffectRequest)],
) -> Result<(), Fault> {
    leselang_command::validate_merge_graph(
        group_token,
        plan,
        branches,
        leselang_command::MergeGraphAdmissionLimits {
            plan: crate::merge_plan_admission_limits(),
            frame: crate::continuation_admission_limits(),
        },
        |request| request.as_view(),
        &MergeGraphHost,
    )
    .map_err(|error| match error {
        leselang_command::MergeGraphAdmissionError::Plan(error) => {
            crate::merge_plan_admission_fault(error)
        }
        leselang_command::MergeGraphAdmissionError::Owner => {
            journal_fault("LSV4032", "merge group token is invalid")
        }
        leselang_command::MergeGraphAdmissionError::Graph(GroupGraphError::Native {
            error,
            ..
        }) => crate::request_admission_fault(error),
        leselang_command::MergeGraphAdmissionError::Graph(_) => journal_fault(
            "LSV4032",
            "merge branch requests do not match the declared plan",
        ),
        leselang_command::MergeGraphAdmissionError::Binding(error) => {
            crate::group_binding::admission_fault(error)
        }
        leselang_command::MergeGraphAdmissionError::Correspondence
        | leselang_command::MergeGraphAdmissionError::UnexpectedGroupOwner => {
            crate::group_binding::invalid()
        }
    })
}

#[cfg(all(test, feature = "sqlite-journal"))]
use leselang_command::reference_group_batch::merge_reserved_sequence;
#[cfg(feature = "sqlite-journal")]
use leselang_command::reference_group_batch::{
    AdmittedMergeGroup, ReferenceGroupBatchProofs, StoredMergeGroup as RawMergeGroup,
    execution_order_label, inspect_group_batch,
};

#[cfg(feature = "sqlite-journal")]
fn validate_merge_graph_records(connection: &Connection) -> Result<u64, Fault> {
    let groups = {
        let mut statement = connection
            .prepare(
                "SELECT token, plan, state, terminal_step, execution_order
                 FROM vm_merge_groups ORDER BY token ASC",
            )
            .map_err(|error| {
                journal_error("LSV4005", "failed to prepare merge graph load", error)
            })?;
        statement
            .query_map([], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            })
            .map_err(|error| journal_error("LSV4005", "failed to load merge graphs", error))?
            .collect::<Result<Vec<RawMergeGroup>, _>>()
            .map_err(|error| journal_error("LSV4005", "failed to read merge graph", error))?
    };

    inspect_group_batch(groups, &mut RecoveredGroupsHost(connection))
}

#[cfg(feature = "sqlite-journal")]
struct RecoveredGroupsHost<'a>(&'a Connection);

#[cfg(feature = "sqlite-journal")]
impl ReferenceGroupBatchProofs<Vec<u8>> for RecoveredGroupsHost<'_> {
    fn token_conflicts(&mut self, token: &ContinuationToken) -> Result<bool, Fault> {
        self.0
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM vm_effects WHERE token = ?1)",
                [token.as_str()],
                |row| row.get(0),
            )
            .map_err(|error| {
                journal_error("LSV4005", "failed to validate merge group token", error)
            })
    }
    fn inspect_group(&mut self, group: AdmittedMergeGroup) -> Result<(), Fault> {
        let AdmittedMergeGroup {
            token,
            plan,
            state,
            terminal,
            ..
        } = group;
        let branches: Vec<RecoveredMergeBranch> = {
            let mut statement = self
                .0
                .prepare(
                    "SELECT b.branch_name, b.position, e.state, e.terminal_step, b.branch_token
                     FROM vm_merge_branches b
                     JOIN vm_effects e ON e.token = b.branch_token
                     WHERE b.group_token = ?1
                     ORDER BY b.position ASC",
                )
                .map_err(|error| {
                    journal_error("LSV4005", "failed to prepare merge branches", error)
                })?;
            statement
                .query_map([token.as_str()], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<Vec<u8>>>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                })
                .map_err(|error| journal_error("LSV4005", "failed to load merge branches", error))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| journal_error("LSV4005", "failed to read merge branches", error))?
        };
        validate_recovered_group(self.0, &token, &plan, &branches, &state, terminal)
    }
}

#[cfg(feature = "sqlite-journal")]
fn validate_recovered_group(
    connection: &Connection,
    token: &ContinuationToken,
    plan: &MergePlan,
    branches: &[RecoveredMergeBranch],
    state: &str,
    terminal_bytes: Option<Vec<u8>>,
) -> Result<(), Fault> {
    inspect_group_record(
        &mut RecoveredGroupHost {
            connection,
            token,
            plan,
            branches,
        },
        token,
        plan,
        branches,
        state,
        terminal_bytes,
    )
}

#[cfg(feature = "sqlite-journal")]
struct RecoveredGroupHost<'a> {
    connection: &'a Connection,
    token: &'a ContinuationToken,
    plan: &'a MergePlan,
    branches: &'a [RecoveredMergeBranch],
}

#[cfg(feature = "sqlite-journal")]
impl ReferenceGroupProofs for RecoveredGroupHost<'_> {
    fn validate_chain(&mut self) -> Result<(), Fault> {
        successor::validate_chain(self.connection, self.token)
    }
    fn validate_blocked_dispatches(&mut self, first_pending: usize) -> Result<(), Fault> {
        validate_blocked_dispatches(self.connection, self.token, first_pending)
    }
    fn validate_unbound_receipt(&mut self, terminal: &Step) -> Result<(), Fault> {
        validate_unbound_group_receipt(self.plan, self.branches, terminal)
    }
    fn validate_records(&mut self, terminal: Option<&Step>) -> Result<(), Fault> {
        // Keep original watermark/cold-reservation proofs inside the same transaction.
        group_binding::validate_records(self.connection, self.token, self.plan, terminal)
    }
}

#[cfg(all(test, feature = "sqlite-journal"))]
fn validate_pending_sequence(
    connection: &Connection,
    token: &ContinuationToken,
    branches: &[RecoveredMergeBranch],
) -> Result<(), Fault> {
    leselang_command::reference_group_recovery::inspect_pending_sequence(
        branches,
        |first_pending| validate_blocked_dispatches(connection, token, first_pending),
    )
}

#[cfg(feature = "sqlite-journal")]
fn validate_blocked_dispatches(
    connection: &Connection,
    token: &ContinuationToken,
    first_pending: usize,
) -> Result<(), Fault> {
    let advanced: bool = connection
        .query_row(
            "SELECT EXISTS (
           SELECT 1 FROM vm_merge_branches b JOIN vm_dispatches d ON d.token = b.branch_token
           WHERE b.group_token = ?1 AND b.position > ?2 AND (d.attempt != 0 OR d.retry_count != 0)
         )",
            params![token.as_str(), first_pending as i64],
            |row| row.get(0),
        )
        .map_err(|error| {
            journal_error("LSV4007", "failed to validate sequential dispatches", error)
        })?;
    if advanced {
        return Err(journal_fault(
            "LSV4007",
            "sequence dispatched a blocked step",
        ));
    }
    Ok(())
}

#[cfg(all(test, feature = "sqlite-journal"))]
fn recover_group_receipt(
    state: &str,
    all_completed: bool,
    terminal_bytes: Option<Vec<u8>>,
    admit: impl FnOnce(&Step) -> Result<(), Fault>,
) -> Result<Option<Step>, Fault> {
    let invalid_state = || {
        journal_fault(
            "LSV4007",
            "merge group state conflicts with its branch graph",
        )
    };
    let state = match state {
        "pending" => StoredGroupState::Pending,
        "completed" => StoredGroupState::Completed,
        _ => return Err(invalid_state()),
    };
    validate_group_receipt(state, all_completed, terminal_bytes, |bytes| {
        let terminal: Step = decode_bounded(&bytes, MAX_JOURNAL_ENTRY_BYTES)?;
        validate_terminal_step(&terminal)?;
        admit(&terminal)?;
        Ok(terminal)
    })
    .map_err(|error| match error {
        GroupReceiptError::Native(error) => error,
        GroupReceiptError::State => invalid_state(),
    })
}

#[cfg(feature = "sqlite-journal")]
use leselang_command::reference_group_recovery::validate_unbound_group_receipt;

#[cfg(feature = "sqlite-journal")]
fn advance_sequence_watermark(connection: &Connection, used_sequence: u64) -> Result<u64, Fault> {
    let required = leselang_runtime_core::SequenceWatermarkRequirement::after_observation(
        used_sequence,
        i64::MAX as u64,
    )
    .map_err(|_| journal_fault("LSV4009", "journal continuation sequence is exhausted"))?;
    let stored: i64 = connection
        .query_row(
            "SELECT next_sequence FROM vm_metadata WHERE singleton = 1",
            [],
            |row| row.get(0),
        )
        .map_err(|error| journal_error("LSV4008", "failed to load sequence watermark", error))?;
    let stored = u64::try_from(stored)
        .map_err(|_| journal_fault("LSV4007", "journal sequence is invalid"))?;
    let update = required
        .reconcile(stored)
        .map_err(|_| journal_fault("LSV4007", "journal sequence is invalid"))?;
    if update.write_required {
        let next = i64::try_from(update.next_sequence)
            .map_err(|_| journal_fault("LSV4009", "journal continuation sequence is exhausted"))?;
        connection
            .execute(
                "UPDATE vm_metadata SET next_sequence = ?1 WHERE singleton = 1",
                [next],
            )
            .map_err(|error| {
                journal_error("LSV4008", "failed to store sequence watermark", error)
            })?;
    }
    Ok(update.next_sequence)
}

#[cfg(unix)]
#[cfg(feature = "sqlite-journal")]
fn tighten_permissions(path: &Path) -> Result<(), Fault> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .map_err(|error| journal_error("LSV4002", "failed to secure journal permissions", error))
}

#[cfg(unix)]
#[cfg(feature = "sqlite-journal")]
fn create_private_journal(path: &Path) -> Result<(), Fault> {
    use std::os::unix::fs::OpenOptionsExt;

    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map(|_| ())
        .map_err(|error| journal_error("LSV4002", "failed to create private journal", error))
}

#[cfg(not(unix))]
#[cfg(feature = "sqlite-journal")]
fn create_private_journal(path: &Path) -> Result<(), Fault> {
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map(|_| ())
        .map_err(|error| journal_error("LSV4002", "failed to create private journal", error))
}

#[cfg(not(unix))]
#[cfg(feature = "sqlite-journal")]
fn tighten_permissions(_path: &Path) -> Result<(), Fault> {
    Ok(())
}

#[cfg(feature = "sqlite-journal")]
fn sql_lease_capacity_exhausted(
    connection: &Connection,
    now: i64,
    limit: usize,
) -> Result<bool, Fault> {
    let sql_limit = i64::try_from(limit)
        .map_err(|_| journal_fault("LSV2500", "active lease limit is out of range"))?;
    let active: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM (
           SELECT 1 FROM vm_dispatches d CROSS JOIN vm_effects e ON e.token = d.token
           WHERE d.state = 'leased' AND d.lease_expires_at_ms > ?1 AND e.state = 'pending'
           LIMIT ?2
         )",
            params![now, sql_limit],
            |row| row.get(0),
        )
        .map_err(|error| journal_error("LSV4040", "failed to read active lease capacity", error))?;
    let active = usize::try_from(active)
        .map_err(|_| journal_fault("LSV4007", "active lease count is invalid"))?;
    Ok(dispatch_lease_capacity_exhausted(active, limit))
}

#[cfg(feature = "sqlite-journal")]
fn sql_scheduler_pressure(
    connection: &Connection,
    now_ms: u64,
    limits: SchedulerLimits,
) -> Result<SchedulerPressure, Fault> {
    let now = i64::try_from(now_ms)
        .map_err(|_| journal_fault("LSV4015", "dispatch clock is out of range"))?;
    let (pending, active) = connection.query_row(
        "SELECT COUNT(*), COALESCE(SUM(CASE WHEN d.state = 'leased' AND d.lease_expires_at_ms > ?1 THEN 1 ELSE 0 END), 0)
         FROM vm_dispatches d JOIN vm_effects e ON e.token = d.token
         WHERE e.state = 'pending' AND d.state IN ('ready', 'leased')",
        [now],
        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
    ).map_err(|error| journal_error("LSV4040", "failed to read scheduler pressure", error))?;
    Ok(SchedulerPressure {
        pending_dispatches: usize::try_from(pending)
            .map_err(|_| journal_fault("LSV4007", "pending dispatch count is invalid"))?,
        active_leases: usize::try_from(active)
            .map_err(|_| journal_fault("LSV4007", "active lease count is invalid"))?,
        limits,
    })
}

#[cfg(feature = "sqlite-journal")]
fn journal_error(code: &str, context: &str, error: impl std::fmt::Display) -> Fault {
    journal_fault(code, format!("{context}: {error}"))
}

fn journal_fault(code: &str, message: impl Into<String>) -> Fault {
    Fault {
        code: code.to_string(),
        message: message.into(),
    }
}

#[cfg(test)]
mod outcome_tests {
    use super::*;
    use crate::{EffectErrorClass, FailedEffect, MergeWait, StructuredEffectBatch, Value};

    #[test]
    fn adapter_step_decode_does_not_replace_original_terminal_admission() {
        let request = crate::continuation_contract_tests::requests().remove(0);
        let not_terminal = "journal completion must be a terminal step";
        let diagnostic = "journal fault has an invalid diagnostic code";
        let cancellation = |token: &str, observed_at_ms| {
            Step::Cancelled(Cancellation {
                continuation: ContinuationToken::from_wire(token.into()),
                reason: CancellationReason::Requested,
                observed_at_ms,
            })
        };
        let cases = [
            (Step::Effect(Box::new(request.clone())), Some(not_terminal)),
            (
                Step::Effects(Box::new(StructuredEffectBatch {
                    merge_token: ContinuationToken::from_wire("merge-1".into()),
                    branches: vec![],
                })),
                Some(not_terminal),
            ),
            (
                Step::Waiting(MergeWait {
                    merge_token: ContinuationToken::from_wire("invalid/token".into()),
                    completed_branches: 3,
                    total_branches: 0,
                }),
                Some(not_terminal),
            ),
            (Step::Yield(request.continuation), Some(not_terminal)),
            (
                Step::Done(Value::UiFocus {
                    node_id: String::new(),
                }),
                Some("completed journal output exceeds runtime limit"),
            ),
            (
                Step::Done(Value::UiFocus {
                    node_id: "a".into(),
                }),
                None,
            ),
            (cancellation("", 0), Some(diagnostic)),
            (cancellation("continuation-1", u64::MAX), Some(diagnostic)),
            (cancellation("invalid/token", 0), None),
            (
                Step::Failed(FailedEffect {
                    retry_count: u32::MAX,
                    error: EffectError {
                        class: EffectErrorClass::Transient,
                        code: "busy".into(),
                        message: "retry".into(),
                    },
                }),
                Some(diagnostic),
            ),
            (
                Step::Fault(Fault {
                    code: String::new(),
                    message: "raw".into(),
                }),
                Some(diagnostic),
            ),
            (
                Step::Fault(Fault {
                    code: "LEGACY".into(),
                    message: "x".repeat(4097),
                }),
                None,
            ),
        ];
        assert_eq!(cases.len(), 12);
        for (step, refusal) in cases {
            let bytes = serde_json::to_vec(&step).unwrap();
            let decoded: leselang_command::Step = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(decoded, step);
            assert_eq!(
                validate_terminal_step(&decoded),
                refusal.map_or(Ok(()), |message| Err(journal_fault("LSV4007", message)))
            );
        }
    }

    #[test]
    fn dispatch_planning_preserves_native_fault_codes_and_precedence() {
        let max = leselang_runtime_core::MAX_CLOCK_MS;
        for (now, duration, message) in [
            (max + 1, 0, "dispatch clock is out of range".to_string()),
            (
                u64::MAX,
                u64::MAX,
                "dispatch clock is out of range".to_string(),
            ),
            (
                max,
                0,
                format!("dispatch lease must be between 1 and {MAX_DISPATCH_LEASE_MS} ms"),
            ),
            (
                max,
                MAX_DISPATCH_LEASE_MS + 1,
                format!("dispatch lease must be between 1 and {MAX_DISPATCH_LEASE_MS} ms"),
            ),
            (
                max,
                1,
                "dispatch lease expiration is out of range".to_string(),
            ),
        ] {
            assert_eq!(
                lease_expiration(now, duration),
                Err(journal_fault("LSV4015", &message))
            );
            assert_eq!(
                validate_lease_clock(now, duration),
                Err(journal_fault("LSV4015", &message))
            );
        }
        assert_eq!(lease_expiration(max - 1, 1), Ok(max));
        assert_eq!(validate_dispatch_clock(max), Ok(()));
        assert_eq!(next_attempt(0), Ok(1));
        assert_eq!(
            next_attempt(MAX_DISPATCH_ATTEMPTS - 1),
            Ok(MAX_DISPATCH_ATTEMPTS)
        );
        for current in [MAX_DISPATCH_ATTEMPTS, MAX_DISPATCH_ATTEMPTS + 1] {
            assert_eq!(
                next_attempt(current),
                Err(journal_fault(
                    "LSV4017",
                    format!("dispatch attempt limit {MAX_DISPATCH_ATTEMPTS} reached")
                ))
            );
        }
        assert_eq!(
            next_attempt(u32::MAX),
            Err(journal_fault(
                "LSV4017",
                "dispatch attempt counter is exhausted"
            ))
        );
    }

    #[test]
    fn sequence_stop_preserves_frozen_early_exit_and_native_fault_precedence() {
        fn frozen<'a>(steps: impl IntoIterator<Item = &'a Step>) -> Result<Option<Step>, Fault> {
            let mut items = 0usize;
            for step in steps {
                validate_terminal_step(step)?;
                let Step::Done(value) = step else {
                    return Ok(Some(step.clone()));
                };
                items = items.saturating_add(validate_value(value, 0)?);
                if items > DEFAULT_MAX_OUTPUT_ITEMS {
                    return Ok(Some(Step::Fault(journal_fault(
                        "LSV2404",
                        "sequential output exceeds runtime bounds",
                    ))));
                }
            }
            Ok(None)
        }
        let mut control = leserpent_domain::InMemoryControlPlane::default();
        let runtime = control.register_runtime(
            leselang_host_contract::RuntimeId::new("runtime-a").unwrap(),
            "runtime",
            "http://runtime.local",
        );
        let large = Step::Done(crate::Value::RuntimeList {
            revision: crate::Revision(7),
            runtimes: vec![runtime; 6000],
        });
        let focus = Step::Done(crate::Value::UiFocus {
            node_id: "a".into(),
        });
        let invalid = Step::Done(crate::Value::UiFocus {
            node_id: String::new(),
        });
        let failure = Step::Fault(journal_fault("ORIGINAL", "original first failure"));
        let cancelled = Step::Cancelled(Cancellation {
            continuation: ContinuationToken::from_wire("continuation-1".into()),
            reason: CancellationReason::Requested,
            observed_at_ms: 1,
        });
        let cases: &[&[&Step]] = &[
            &[],
            &[&focus],
            &[&large, &focus],
            &[&large, &large],
            &[&invalid],
            &[&focus, &invalid, &failure],
            &[&failure, &invalid],
            &[&large, &failure, &invalid],
            &[&large, &large, &invalid],
            &[&cancelled, &large, &large],
            &[&focus, &cancelled, &invalid],
            &[&large, &large, &cancelled],
        ];
        for steps in cases {
            assert_eq!(
                sequence_stop(steps.iter().copied()),
                frozen(steps.iter().copied())
            );
        }
        let panic_after_stop = std::iter::once(&failure).chain(std::iter::from_fn(|| {
            panic!("later terminal must not be observed")
        }));
        assert_eq!(sequence_stop(panic_after_stop).unwrap(), Some(failure));
    }
}

#[cfg(test)]
#[cfg(feature = "sqlite-journal")]
mod scheduling_tests {
    use super::*;

    #[test]
    fn malformed_retry_proposals_preserve_the_original_lease_in_both_journals() {
        for durable in [false, true] {
            let root = TempRoot::new();
            let path = root.0.join("retry.sqlite3");
            let mut vm = if durable {
                crate::Vm::open_journal(&path, 100).unwrap()
            } else {
                crate::Vm::new(100)
            };
            let program = leselang_hir::lower(&leselang_syntax::parse(
                r#"fn main() = bind(result: ui.focus(node_id: "node"), body: true)"#,
            ))
            .unwrap();
            let Step::Effect(request) = vm.start(
                &program,
                crate::Principal::new("operator").unwrap(),
                crate::CapabilitySet::new(["ui.presentation"]),
                None,
            ) else {
                panic!("pending effect")
            };
            let lease = vm.claim_effect(100, 100).unwrap().unwrap();
            for (count, ready) in [(1, i64::MAX as u64 + 1), (0, 110), (2, 110), (1, 101)] {
                let disposition = RetryDisposition::Scheduled(crate::RetrySchedule {
                    retry_count: count,
                    ready_at_ms: ready,
                    error: EffectError {
                        class: crate::EffectErrorClass::Transient,
                        code: "busy".into(),
                        message: "native busy".into(),
                    },
                });
                assert_eq!(
                    vm.journal
                        .report_error(&lease, 101, &disposition)
                        .unwrap_err()
                        .code,
                    "LSV4026"
                );
                assert_eq!(vm.scheduler_pressure(101).unwrap().active_leases, 1);
            }
            if durable {
                drop(vm);
                vm = crate::Vm::open_journal(&path, 0).unwrap();
            }
            assert_eq!(
                vm.pending_continuations(),
                vec![request.continuation.clone()]
            );
            let done = vm.acknowledge_effect(
                &lease,
                102,
                crate::EffectResult::Presentation(crate::PresentationResult::Focus {
                    node_id: "node".into(),
                }),
            );
            assert!(matches!(done, Step::Done(_)));
        }
    }

    #[test]
    fn retry_proposal_counter_bounds_refuse_without_overflow_or_native_error_formatting() {
        for (current, next) in [
            (MAX_SEMANTIC_RETRIES, MAX_SEMANTIC_RETRIES + 1),
            (u32::MAX, 0),
        ] {
            let schedule = crate::RetrySchedule {
                retry_count: next,
                ready_at_ms: 2,
                error: EffectError {
                    class: crate::EffectErrorClass::Transient,
                    code: "busy".into(),
                    message: "native detail must not appear".into(),
                },
            };
            let fault = validate_proposed_retry(current, &schedule, 1).unwrap_err();
            assert_eq!(fault.code, "LSV4026");
            assert_eq!(fault.message, "retry schedule is invalid");
        }
    }

    #[test]
    fn watermark_planning_preserves_frozen_sqlite_fault_order_and_writes() {
        fn frozen(connection: &Connection, used_sequence: u64) -> Result<u64, Fault> {
            let required = used_sequence
                .checked_add(1)
                .and_then(|v| i64::try_from(v).ok())
                .ok_or_else(|| {
                    journal_fault("LSV4009", "journal continuation sequence is exhausted")
                })?;
            let stored: i64 = connection
                .query_row(
                    "SELECT next_sequence FROM vm_metadata WHERE singleton = 1",
                    [],
                    |row| row.get(0),
                )
                .map_err(|e| journal_error("LSV4008", "failed to load sequence watermark", e))?;
            if stored < 1 {
                return Err(journal_fault("LSV4007", "journal sequence is invalid"));
            }
            if required > stored {
                connection
                    .execute(
                        "UPDATE vm_metadata SET next_sequence = ?1 WHERE singleton = 1",
                        [required],
                    )
                    .map_err(|e| {
                        journal_error("LSV4008", "failed to store sequence watermark", e)
                    })?;
            }
            Ok(stored.max(required) as u64)
        }
        fn connection(stored: Option<i64>, reject_write: bool) -> Connection {
            let connection = Connection::open_in_memory().unwrap();
            if let Some(stored) = stored {
                connection.execute_batch("CREATE TABLE vm_metadata(singleton INTEGER PRIMARY KEY, next_sequence INTEGER NOT NULL);").unwrap();
                connection
                    .execute("INSERT INTO vm_metadata VALUES(1, ?1)", [stored])
                    .unwrap();
                if reject_write {
                    connection.execute_batch("CREATE TRIGGER reject_watermark BEFORE UPDATE ON vm_metadata BEGIN SELECT RAISE(ABORT, 'injected watermark failure'); END;").unwrap();
                }
            }
            connection
        }
        for used in [
            0,
            1,
            9,
            crate::MAX_EFFECT_SEQUENCE,
            i64::MAX as u64,
            u64::MAX,
        ] {
            for stored in [None, Some(-1), Some(0), Some(1), Some(10), Some(i64::MAX)] {
                for reject_write in [false, true] {
                    let original = connection(stored, reject_write);
                    let shared = connection(stored, reject_write);
                    assert_eq!(
                        advance_sequence_watermark(&shared, used),
                        frozen(&original, used),
                        "used={used}, stored={stored:?}, reject={reject_write}"
                    );
                    if stored.is_some() {
                        let read = |connection: &Connection| {
                            connection
                                .query_row("SELECT next_sequence FROM vm_metadata", [], |row| {
                                    row.get::<_, i64>(0)
                                })
                                .unwrap()
                        };
                        assert_eq!(read(&shared), read(&original));
                    }
                }
            }
        }
    }

    struct TempRoot(std::path::PathBuf);
    impl TempRoot {
        fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
            let root = Self(std::env::temp_dir().join(format!(
                "leselang-journal-scheduling-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            )));
            fs::create_dir(&root.0).unwrap();
            root
        }
    }
    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn simple_batch() -> crate::StructuredEffectBatch {
        let program = leselang_hir::lower(&leselang_syntax::parse(
            r#"fn main() = all(left: ui.focus(node_id: "a"), right: ui.focus(node_id: "b"))"#,
        ))
        .unwrap();
        let Step::Effects(batch) = crate::Vm::new(10000).start_timed(
            &program,
            crate::Principal::new("operator").unwrap(),
            crate::CapabilitySet::new(["ui.presentation"]),
            Some(crate::Revision(7)),
            100,
            1000,
        ) else {
            panic!()
        };
        *batch
    }

    fn pending_sequence_fixture(root: &TempRoot) -> (SqliteJournal, crate::StructuredEffectBatch) {
        let path = root.0.join("sequence.sqlite3");
        let program = leselang_hir::lower(&leselang_syntax::parse(
            r#"fn main() = seq(first: ui.focus(node_id: "first"),
                second: ui.focus(node_id: "second"), third: ui.focus(node_id: "third"))"#,
        ))
        .unwrap();
        let mut vm = crate::Vm::open_journal(&path, 500).unwrap();
        let started = vm.start(
            &program,
            crate::Principal::new("operator").unwrap(),
            crate::CapabilitySet::new(["ui.presentation"]),
            None,
        );
        let Step::Effect(first) = started else {
            panic!("expected first sequence effect: {started:?}")
        };
        assert!(matches!(
            vm.resume(
                &first.continuation,
                crate::EffectResult::Presentation(crate::PresentationResult::Focus {
                    node_id: "first".into()
                })
            ),
            Step::Effect(_)
        ));
        drop(vm);
        let journal = SqliteJournal::open(&path).unwrap();
        let merge_token = ContinuationToken::from_wire(
            journal
                .connection
                .query_row("SELECT token FROM vm_merge_groups", [], |row| row.get(0))
                .unwrap(),
        );
        let branches = journal
            .connection
            .prepare(
                "SELECT b.branch_name, d.request FROM vm_merge_branches b
             JOIN vm_dispatches d ON d.token = b.branch_token
             WHERE b.group_token = ?1 ORDER BY b.position ASC",
            )
            .unwrap()
            .query_map([merge_token.as_str()], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?))
            })
            .unwrap()
            .map(|row| {
                let (branch, bytes) = row.unwrap();
                crate::NamedEffectRequest {
                    branch,
                    request: serde_json::from_slice(&bytes).unwrap(),
                }
            })
            .collect();
        (
            journal,
            crate::StructuredEffectBatch {
                merge_token,
                branches,
            },
        )
    }

    fn sequence_rows(
        connection: &Connection,
        token: &ContinuationToken,
    ) -> Vec<RecoveredMergeBranch> {
        connection
            .prepare(
                "SELECT b.branch_name, b.position, e.state, e.terminal_step, b.branch_token
             FROM vm_merge_branches b JOIN vm_effects e ON e.token = b.branch_token
             WHERE b.group_token = ?1 ORDER BY b.position ASC",
            )
            .unwrap()
            .query_map([token.as_str()], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
    }

    #[test]
    fn sequence_frontier_preserves_frozen_sqlite_phase_and_fault_order() {
        fn frozen(
            connection: &Connection,
            token: &ContinuationToken,
            branches: &[RecoveredMergeBranch],
        ) -> Result<(), Fault> {
            let first_pending = branches
                .iter()
                .position(|branch| branch.2 == "pending")
                .unwrap_or(branches.len());
            if branches[first_pending..]
                .iter()
                .any(|branch| branch.2 != "pending")
            {
                return Err(journal_fault(
                    "LSV4007",
                    "sequence completion is out of order",
                ));
            }
            let terminal_steps = branches[..first_pending]
                .iter()
                .map(|branch| {
                    decode_bounded::<Step>(
                        branch.3.as_deref().ok_or_else(|| {
                            journal_fault("LSV4007", "completed sequence step has no result")
                        })?,
                        MAX_JOURNAL_ENTRY_BYTES,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            if sequence_stop(terminal_steps.iter())?.is_some() {
                return Err(journal_fault(
                    "LSV4007",
                    "failed sequence still has pending successors",
                ));
            }
            let advanced: bool = connection.query_row(
                "SELECT EXISTS (
                   SELECT 1 FROM vm_merge_branches b JOIN vm_dispatches d ON d.token = b.branch_token
                   WHERE b.group_token = ?1 AND b.position > ?2 AND (d.attempt != 0 OR d.retry_count != 0)
                 )", params![token.as_str(), first_pending as i64], |row| row.get(0)
            ).map_err(|error| journal_error("LSV4007", "failed to validate sequential dispatches", error))?;
            if advanced {
                return Err(journal_fault(
                    "LSV4007",
                    "sequence dispatched a blocked step",
                ));
            }
            Ok(())
        }
        let root = TempRoot::new();
        let (mut journal, batch) = pending_sequence_fixture(&root);
        let original = sequence_rows(&journal.connection, &batch.merge_token);
        for case in 0..12 {
            let transaction = journal.connection.transaction().unwrap();
            let mut rows = original.clone();
            match case {
                0 => {}
                1 => rows.clear(),
                2 => {
                    rows[0].2 = "pending".into();
                    rows[0].3 = None;
                }
                3 => {
                    for row in &mut rows {
                        row.2 = "completed".into();
                        row.3 = original[0].3.clone();
                    }
                }
                4 => {
                    rows[2].2 = "completed".into();
                    rows[0].3 = Some(vec![0]);
                }
                5 => rows[0].3 = None,
                6 => rows[0].3 = Some(vec![0]),
                7 | 8 => {
                    rows[0].3 = Some(
                        serde_json::to_vec(&Step::Fault(journal_fault("LSV1001", "original stop")))
                            .unwrap(),
                    )
                }
                9..=11 => {}
                _ => unreachable!(),
            }
            if matches!(case, 8 | 9) {
                transaction
                    .execute(
                        "UPDATE vm_dispatches SET attempt = 1 WHERE token = ?1",
                        [&rows[2].4],
                    )
                    .unwrap();
            } else if case == 10 {
                transaction
                    .execute(
                        "UPDATE vm_dispatches SET attempt = 1 WHERE token = ?1",
                        [&rows[1].4],
                    )
                    .unwrap();
            } else if case == 11 {
                transaction
                    .execute(
                        "UPDATE vm_dispatches SET retry_count = 1 WHERE token = ?1",
                        [&rows[2].4],
                    )
                    .unwrap();
            }
            let expected = frozen(&transaction, &batch.merge_token, &rows);
            assert_eq!(
                validate_pending_sequence(&transaction, &batch.merge_token, &rows),
                expected,
                "case {case}"
            );
            assert_eq!(expected.is_ok(), matches!(case, 0..=3 | 10), "case {case}");
            transaction.rollback().unwrap();
        }
    }

    #[test]
    fn invalid_sequence_frontier_blocks_snapshot_and_watermark_repair_without_mutation() {
        fn snapshot(connection: &Connection) -> Vec<Vec<Vec<rusqlite::types::Value>>> {
            [
                ("vm_metadata", "singleton"),
                ("vm_effects", "token"),
                ("vm_dispatches", "token"),
                ("vm_merge_groups", "token"),
                ("vm_merge_branches", "group_token, position"),
            ]
            .into_iter()
            .map(|(table, order)| {
                let mut statement = connection
                    .prepare(&format!("SELECT * FROM {table} ORDER BY {order}"))
                    .unwrap();
                let columns = statement.column_count();
                statement
                    .query_map([], |row| {
                        (0..columns)
                            .map(|column| row.get(column))
                            .collect::<Result<Vec<_>, _>>()
                    })
                    .unwrap()
                    .collect::<Result<Vec<_>, _>>()
                    .unwrap()
            })
            .collect()
        }
        for case in 0..4 {
            let root = TempRoot::new();
            let (mut journal, batch) = pending_sequence_fixture(&root);
            let original = sequence_rows(&journal.connection, &batch.merge_token);
            let watermark: i64 = journal
                .connection
                .query_row("SELECT next_sequence FROM vm_metadata", [], |row| {
                    row.get(0)
                })
                .unwrap();
            let third = &batch.branches[2].request.continuation.token;
            let expected = match case {
                0 => {
                    let bytes = serde_json::to_vec(&Step::Done(crate::Value::UiFocus {
                        node_id: "third".into(),
                    }))
                    .unwrap();
                    journal.connection.execute("UPDATE vm_effects SET state = 'completed', terminal_step = ?2 WHERE token = ?1", params![third.as_str(), bytes]).unwrap();
                    journal
                        .connection
                        .execute(
                            "UPDATE vm_dispatches SET state = 'acknowledged' WHERE token = ?1",
                            [third.as_str()],
                        )
                        .unwrap();
                    "sequence completion is out of order"
                }
                1 => {
                    journal
                        .connection
                        .execute(
                            "UPDATE vm_dispatches SET attempt = 1 WHERE token = ?1",
                            [third.as_str()],
                        )
                        .unwrap();
                    "sequence dispatched a blocked step"
                }
                2 => {
                    let bytes =
                        serde_json::to_vec(&Step::Fault(journal_fault("LSV1001", "original stop")))
                            .unwrap();
                    journal
                        .connection
                        .execute(
                            "UPDATE vm_effects SET terminal_step = ?2 WHERE token = ?1",
                            params![&original[0].4, bytes],
                        )
                        .unwrap();
                    "failed sequence still has pending successors"
                }
                3 => {
                    let bytes = serde_json::to_vec(&EffectError {
                        class: crate::EffectErrorClass::Transient,
                        code: "original_retry".into(),
                        message: "original retry".into(),
                    })
                    .unwrap();
                    journal.connection.execute("UPDATE vm_dispatches SET retry_count = 1, last_error = ?2 WHERE token = ?1", params![third.as_str(), bytes]).unwrap();
                    "sequence dispatched a blocked step"
                }
                _ => unreachable!(),
            };
            journal
                .connection
                .execute("UPDATE vm_metadata SET next_sequence = 1", [])
                .unwrap();
            let before = snapshot(&journal.connection);
            assert_eq!(
                journal.load().err().unwrap(),
                journal_fault("LSV4007", expected)
            );
            assert_eq!(snapshot(&journal.connection), before);
            journal.connection.execute("UPDATE vm_effects SET state = 'pending', terminal_step = NULL WHERE token = ?1", [third.as_str()]).unwrap();
            journal.connection.execute("UPDATE vm_dispatches SET state = 'ready', attempt = 0, retry_count = 0, last_error = NULL WHERE token = ?1", [third.as_str()]).unwrap();
            journal
                .connection
                .execute(
                    "UPDATE vm_effects SET terminal_step = ?2 WHERE token = ?1",
                    params![&original[0].4, &original[0].3],
                )
                .unwrap();
            let recovered = journal.load().unwrap();
            assert_eq!(recovered.pending.len(), 2);
            assert_eq!(recovered.completed.len(), 1);
            assert_eq!(recovered.next_sequence, u64::try_from(watermark).unwrap());
        }
    }

    #[test]
    fn successor_chain_inspection_preserves_frozen_sqlite_fault_order() {
        fn frozen(connection: &Connection, group: &ContinuationToken) -> Result<(), Fault> {
            let mut statement = connection
                .prepare(
                    "SELECT b.branch_token, e.terminal_step FROM vm_merge_branches b
                 JOIN vm_effects e ON e.token = b.branch_token
                 WHERE b.group_token = ?1 ORDER BY b.position",
                )
                .map_err(|error| {
                    journal_error("LSV4034", "failed to inspect result chain", error)
                })?;
            let records = statement
                .query_map([group.as_str()], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, Option<Vec<u8>>>(1)?))
                })
                .map_err(|error| journal_error("LSV4034", "failed to load result chain", error))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| journal_error("LSV4034", "invalid result-chain record", error))?;
            if records.len() < 2 || records.len() > crate::MAX_MERGE_BRANCHES {
                return Err(crate::successor::invalid());
            }
            let mut parent = successor::request_for(connection, &records[0].0)?;
            for pair in records.windows(2) {
                let result: Step = decode_bounded(
                    pair[0].1.as_deref().ok_or_else(crate::successor::invalid)?,
                    MAX_JOURNAL_ENTRY_BYTES,
                )?;
                let next = successor::request_for(connection, &pair[1].0)?;
                crate::successor::validate_pair(&parent, &result, &next)?;
                parent = next;
            }
            Ok(())
        }

        let root = TempRoot::new();
        let path = root.0.join("chain.sqlite3");
        let program = leselang_hir::lower(&leselang_syntax::parse(
            r#"fn main() = bind(first: ui.focus(node_id: "a"), body:
                bind(second: ui.focus(node_id: field(value: first, name: "node_id")), body:
                    ui.focus(node_id: field(value: second, name: "node_id"))))"#,
        ))
        .unwrap();
        let mut vm = crate::Vm::open_journal(&path, 500).unwrap();
        let Step::Effect(first) = vm.start(
            &program,
            crate::Principal::new("operator").unwrap(),
            crate::CapabilitySet::new(["ui.presentation"]),
            Some(crate::Revision(7)),
        ) else {
            panic!()
        };
        let reply = || {
            crate::EffectResult::Presentation(crate::PresentationResult::Focus {
                node_id: "a".into(),
            })
        };
        let Step::Effect(second) = vm.resume(&first.continuation, reply()) else {
            panic!()
        };
        let Step::Effect(third) = vm.resume(&second.continuation, reply()) else {
            panic!()
        };
        drop(vm);
        let mut connection = Connection::open(&path).unwrap();
        let group = ContinuationToken::from_wire(
            connection
                .query_row("SELECT token FROM vm_merge_groups", [], |row| row.get(0))
                .unwrap(),
        );
        let tokens = [
            &first.continuation.token,
            &second.continuation.token,
            &third.continuation.token,
        ];
        assert_eq!(frozen(&connection, &group), Ok(()));
        for case in 0..11 {
            let transaction = connection.transaction().unwrap();
            match case {
                0 => {}
                1 => {
                    transaction
                        .execute(
                            "DELETE FROM vm_merge_branches WHERE group_token = ?1",
                            [group.as_str()],
                        )
                        .unwrap();
                }
                2 => {
                    transaction
                        .execute(
                            "DELETE FROM vm_merge_branches WHERE group_token = ?1 AND position > 0",
                            [group.as_str()],
                        )
                        .unwrap();
                }
                3 => {
                    transaction
                        .execute(
                            "UPDATE vm_dispatches SET request = X'00' WHERE token = ?1",
                            [tokens[0].as_str()],
                        )
                        .unwrap();
                }
                4 => {
                    transaction
                        .execute(
                            "UPDATE vm_effects SET state = 'pending', terminal_step = NULL WHERE token = ?1",
                            [tokens[0].as_str()],
                        )
                        .unwrap();
                }
                5 => {
                    transaction
                        .execute(
                            "UPDATE vm_effects SET terminal_step = X'00' WHERE token = ?1",
                            [tokens[0].as_str()],
                        )
                        .unwrap();
                    transaction
                        .execute(
                            "UPDATE vm_dispatches SET request = X'00' WHERE token = ?1",
                            [tokens[1].as_str()],
                        )
                        .unwrap();
                }
                6 => {
                    transaction
                        .execute(
                            "UPDATE vm_dispatches SET request = X'00' WHERE token = ?1",
                            [tokens[1].as_str()],
                        )
                        .unwrap();
                }
                7 => {
                    transaction
                        .execute(
                            "UPDATE vm_effects SET terminal_step = X'00' WHERE token = ?1",
                            [tokens[1].as_str()],
                        )
                        .unwrap();
                }
                8 => {
                    transaction
                        .execute(
                            "UPDATE vm_effects SET state = 'completed', terminal_step = X'00' WHERE token = ?1",
                            [tokens[2].as_str()],
                        )
                        .unwrap();
                }
                9 | 10 => {
                    let value = if case == 9 {
                        crate::Value::UiFocus {
                            node_id: "different-original-result".into(),
                        }
                    } else {
                        crate::Value::Scalar {
                            value: crate::ScalarValue::Boolean(true),
                        }
                    };
                    let bytes = serde_json::to_vec(&Step::Done(value)).unwrap();
                    transaction
                        .execute(
                            "UPDATE vm_effects SET terminal_step = ?2 WHERE token = ?1",
                            params![tokens[0].as_str(), bytes],
                        )
                        .unwrap();
                }
                _ => unreachable!(),
            }
            let expected = frozen(&transaction, &group);
            assert_eq!(
                successor::validate_chain(&transaction, &group),
                expected,
                "case {case}"
            );
            assert_eq!(expected.is_ok(), matches!(case, 0 | 8), "case {case}");
            transaction.rollback().unwrap();
        }
    }

    #[test]
    fn group_receipt_native_admission_runs_once_after_state_decode_and_terminal_validation() {
        use std::cell::Cell;

        let valid = serde_json::to_vec(&Step::Done(crate::Value::Scalar {
            value: crate::ScalarValue::Integer(7),
        }))
        .unwrap();
        let nonterminal = serde_json::to_vec(&Step::Waiting(crate::MergeWait {
            merge_token: ContinuationToken::from_wire("merge-private".into()),
            completed_branches: 1,
            total_branches: 2,
        }))
        .unwrap();
        for (state, complete, bytes, expected_calls) in [
            ("pending", false, None, 0),
            ("invalid", true, Some(valid.clone()), 0),
            ("completed", false, Some(valid.clone()), 0),
            ("pending", false, Some(valid.clone()), 0),
            ("completed", true, Some(b"invalid-wire".to_vec()), 0),
            ("completed", true, Some(nonterminal), 0),
            ("completed", true, Some(valid), 1),
        ] {
            let calls = Cell::new(0);
            let native = journal_fault("LSV4700", "private native refusal");
            let result = recover_group_receipt(state, complete, bytes, |_| {
                calls.set(calls.get() + 1);
                Err(native.clone())
            });
            assert_eq!(calls.get(), expected_calls);
            if expected_calls == 1 {
                assert_eq!(result, Err(native));
            } else if state == "pending" && !complete && result.is_ok() {
                assert_eq!(result, Ok(None));
            } else {
                assert!(result.is_err());
            }
        }
    }

    #[test]
    fn group_receipt_recovery_preserves_frozen_state_and_decode_fault_precedence() {
        fn original(
            state: &str,
            all_completed: bool,
            bytes: Option<Vec<u8>>,
        ) -> Result<Option<Step>, Fault> {
            match (state, bytes) {
                ("pending", None) if !all_completed => Ok(None),
                ("completed", Some(bytes)) if all_completed => {
                    let step: Step = decode_bounded(&bytes, MAX_JOURNAL_ENTRY_BYTES)?;
                    validate_terminal_step(&step)?;
                    Ok(Some(step))
                }
                _ => Err(journal_fault(
                    "LSV4007",
                    "merge group state conflicts with its branch graph",
                )),
            }
        }
        let valid = serde_json::to_vec(&Step::Done(crate::Value::Scalar {
            value: crate::ScalarValue::Integer(7),
        }))
        .unwrap();
        for state in ["pending", "completed", "invalid"] {
            for completed in [false, true] {
                for bytes in [
                    None,
                    Some(valid.clone()),
                    Some(b"private-invalid-wire".to_vec()),
                ] {
                    assert_eq!(
                        recover_group_receipt(state, completed, bytes.clone(), |_| Ok(())),
                        original(state, completed, bytes)
                    );
                }
            }
        }
    }

    #[test]
    fn group_graph_admission_preserves_frozen_fault_precedence() {
        fn original(
            owner: &ContinuationToken,
            plan: &MergePlan,
            branches: &[(&str, &EffectRequest)],
        ) -> Result<(), Fault> {
            validate_merge_plan(plan)?;
            if !valid_continuation_token(owner) {
                return Err(journal_fault("LSV4032", "merge group token is invalid"));
            }
            let mismatch = || {
                journal_fault(
                    "LSV4032",
                    "merge branch requests do not match the declared plan",
                )
            };
            if branches.len() != plan.branches.len() {
                return Err(mismatch());
            }
            let mut tokens = std::collections::HashSet::with_capacity(branches.len());
            for (position, (name, request)) in branches.iter().enumerate() {
                validate_effect_request(request)?;
                if *name != plan.branches[position]
                    || request.continuation.token == *owner
                    || !tokens.insert(request.continuation.token.as_str())
                {
                    return Err(mismatch());
                }
            }
            if let Some(binding) = &plan.result_binding {
                binding.validate_requests(owner, plan, branches)?;
            } else if branches
                .iter()
                .any(|(_, request)| request.continuation.group_result.is_some())
            {
                return Err(crate::group_binding::invalid());
            }
            Ok(())
        }
        let batch = simple_batch();
        let plan = MergePlan::new(["left", "right"]).unwrap();
        let left = &batch.branches[0].request;
        let right = &batch.branches[1].request;
        let mut invalid = right.clone();
        invalid.continuation.deadline_ms = 0;
        for (owner, graph) in [
            (
                batch.merge_token.clone(),
                vec![("left", left), ("right", right)],
            ),
            (batch.merge_token.clone(), vec![("left", &invalid)]),
            (
                batch.merge_token.clone(),
                vec![("left", left), ("right", left)],
            ),
            (
                left.continuation.token.clone(),
                vec![("left", left), ("right", right)],
            ),
            (
                batch.merge_token.clone(),
                vec![("right", right), ("left", left)],
            ),
            (
                batch.merge_token.clone(),
                vec![("left", left), ("wrong", &invalid)],
            ),
            (
                ContinuationToken::from_wire(String::new()),
                vec![("left", &invalid)],
            ),
        ] {
            assert_eq!(
                validate_merge_graph_input(&owner, &plan, &graph),
                original(&owner, &plan, &graph),
            );
        }
    }

    #[test]
    fn late_group_graph_refusal_rolls_back_recovery_without_repairing_watermark() {
        let batch = simple_batch();
        let plan = MergePlan::new(["left", "right"]).unwrap();
        let graph = batch
            .branches
            .iter()
            .map(|branch| (branch.branch.as_str(), &branch.request))
            .collect::<Vec<_>>();
        let root = TempRoot::new();
        let mut journal = SqliteJournal::open(&root.0.join("group-recovery.sqlite3")).unwrap();
        journal
            .record_merge_graph(
                &batch.merge_token,
                &plan,
                &graph,
                SchedulerLimits::default(),
            )
            .unwrap();
        journal
            .connection
            .execute("UPDATE vm_metadata SET next_sequence = 1", [])
            .unwrap();
        journal
            .connection
            .execute(
                "UPDATE vm_merge_branches SET position = 63 WHERE branch_name = 'left'",
                [],
            )
            .unwrap();
        let error = journal.load().err().unwrap();
        assert_eq!(
            error,
            journal_fault("LSV4007", "merge branches do not match their declared plan")
        );
        assert_eq!(
            journal
                .connection
                .query_row("SELECT next_sequence FROM vm_metadata", [], |row| row
                    .get::<_, i64>(0),)
                .unwrap(),
            1
        );
        journal
            .connection
            .execute(
                "UPDATE vm_merge_branches SET position = 0 WHERE branch_name = 'left'",
                [],
            )
            .unwrap();
        let snapshot = journal.load().unwrap();
        assert_eq!(snapshot.pending.len(), 2);
        assert!(snapshot.completed.is_empty());
        assert_eq!(snapshot.next_sequence, 4);
    }

    fn recovery_fixture(root: &TempRoot) -> (SqliteJournal, EffectRequest, EffectRequest, Step) {
        let program = leselang_hir::lower(&leselang_syntax::parse(
            r#"fn main() = ui.focus(node_id: "original")"#,
        ))
        .unwrap();
        let mut vm = crate::Vm::new(100);
        let mut start = || {
            let Step::Effect(request) = vm.start(
                &program,
                crate::Principal::new("operator").unwrap(),
                crate::CapabilitySet::new(["ui.presentation"]),
                Some(crate::Revision(7)),
            ) else {
                panic!()
            };
            *request
        };
        let first = start();
        let second = start();
        let terminal = vm.resume(
            &first.continuation,
            crate::EffectResult::Presentation(crate::PresentationResult::Focus {
                node_id: "original".into(),
            }),
        );
        let mut journal = SqliteJournal::open(&root.0.join("recovery.sqlite3")).unwrap();
        for request in [&first, &second] {
            journal
                .record_pending(
                    &request.continuation,
                    Some(request),
                    SchedulerLimits::default(),
                )
                .unwrap();
        }
        assert_eq!(
            journal
                .record_completed(&first.continuation, &terminal)
                .unwrap(),
            terminal
        );
        (journal, first, second, terminal)
    }

    #[test]
    fn recovery_commit_failure_releases_staging_and_rolls_back_the_watermark() {
        let root = TempRoot::new();
        let (mut journal, first, second, terminal) = recovery_fixture(&root);
        journal
            .connection
            .execute_batch(
                "UPDATE vm_metadata SET next_sequence = 1;
             CREATE TRIGGER reject_recovery_commit BEFORE UPDATE ON vm_metadata
             BEGIN SELECT RAISE(ABORT, 'injected recovery failure'); END;",
            )
            .unwrap();
        let original: Vec<(String, Vec<u8>, Option<Vec<u8>>)> = journal
            .connection
            .prepare("SELECT token, image, terminal_step FROM vm_effects ORDER BY token ASC")
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert!(matches!(journal.load(), Err(error) if error.code == "LSV4008"));
        assert_eq!(
            journal
                .connection
                .query_row("SELECT next_sequence FROM vm_metadata", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
        journal
            .connection
            .execute_batch("DROP TRIGGER reject_recovery_commit;")
            .unwrap();
        let snapshot = journal.load().unwrap();
        assert_eq!(snapshot.next_sequence, 3);
        assert_eq!(
            snapshot.pending[&second.continuation.token],
            second.continuation
        );
        assert_eq!(snapshot.completed[&first.continuation.token], terminal);
        let recovered: Vec<(String, Vec<u8>, Option<Vec<u8>>)> = journal
            .connection
            .prepare("SELECT token, image, terminal_step FROM vm_effects ORDER BY token ASC")
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(original, recovered);
        drop(journal);
        let mut vm = crate::Vm::open_journal(root.0.join("recovery.sqlite3"), 100).unwrap();
        assert_eq!(
            vm.resume(
                &first.continuation,
                crate::EffectResult::Presentation(crate::PresentationResult::Focus {
                    node_id: "late-forged-reply".into(),
                })
            ),
            terminal
        );
        assert_eq!(vm.pending_count(), 1);
        assert_eq!(vm.claim_effect(1, 20).unwrap().unwrap().request, second);
    }

    #[test]
    fn recovery_rejects_a_late_invalid_row_without_exposing_the_valid_prefix() {
        let root = TempRoot::new();
        let (mut journal, first, second, terminal) = recovery_fixture(&root);
        journal
            .connection
            .execute_batch("UPDATE vm_metadata SET next_sequence = 1;")
            .unwrap();
        journal
            .connection
            .execute(
                "UPDATE vm_effects SET deadline_at_ms = 7 WHERE token = ?1",
                [second.continuation.token.as_str()],
            )
            .unwrap();
        assert!(
            matches!(journal.load(), Err(error) if error.code == "LSV4007" && error.message == "journal deadline does not match continuation image")
        );
        assert_eq!(
            journal
                .connection
                .query_row("SELECT next_sequence FROM vm_metadata", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
        journal
            .connection
            .execute(
                "UPDATE vm_effects SET deadline_at_ms = NULL WHERE token = ?1",
                [second.continuation.token.as_str()],
            )
            .unwrap();
        let snapshot = journal.load().unwrap();
        assert_eq!(snapshot.completed[&first.continuation.token], terminal);
        assert_eq!(
            snapshot.pending[&second.continuation.token],
            second.continuation
        );
        assert_eq!(snapshot.next_sequence, 3);
    }

    #[test]
    fn direct_graph_admission_reserves_numeric_group_and_branch_ids_but_accepts_opaque_groups() {
        let batch = simple_batch();
        let plan = MergePlan::new(["left", "right"]).unwrap();
        let graph = batch
            .branches
            .iter()
            .map(|branch| (branch.branch.as_str(), &branch.request))
            .collect::<Vec<_>>();
        for (group, expected) in [
            ("merge-100", 101),
            ("continuation-200", 201),
            ("legacy-group", 4),
        ] {
            let root = TempRoot::new();
            let path = root.0.join("flow.sqlite3");
            let (mut journal, _) = Journal::open(&path).unwrap();
            journal
                .record_merge_graph(&ContinuationToken::from_wire(group.into()), &plan, &graph)
                .unwrap();
            let (_, snapshot) = Journal::open(&path).unwrap();
            assert_eq!(snapshot.next_sequence, expected);
            assert_eq!(journal.allocate_sequence(1).unwrap(), expected);
        }
    }

    #[test]
    fn graph_watermark_failure_rolls_back_every_record_and_allows_a_retry() {
        let batch = simple_batch();
        let plan = MergePlan::new(["left", "right"]).unwrap();
        let graph = batch
            .branches
            .iter()
            .map(|branch| (branch.branch.as_str(), &branch.request))
            .collect::<Vec<_>>();
        let root = TempRoot::new();
        let mut journal = SqliteJournal::open(&root.0.join("flow.sqlite3")).unwrap();
        journal
            .connection
            .execute_batch(
                "CREATE TRIGGER reject_watermark BEFORE UPDATE ON vm_metadata
             BEGIN SELECT RAISE(ABORT, 'injected failure'); END;",
            )
            .unwrap();
        assert_eq!(
            journal
                .record_merge_graph(
                    &batch.merge_token,
                    &plan,
                    &graph,
                    SchedulerLimits::default()
                )
                .unwrap_err()
                .code,
            "LSV4008"
        );
        for table in [
            "vm_effects",
            "vm_dispatches",
            "vm_merge_groups",
            "vm_merge_branches",
        ] {
            assert_eq!(
                journal
                    .connection
                    .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| row
                        .get::<_, i64>(0))
                    .unwrap(),
                0
            );
        }
        journal
            .connection
            .execute_batch("DROP TRIGGER reject_watermark;")
            .unwrap();
        journal
            .record_merge_graph(
                &batch.merge_token,
                &plan,
                &graph,
                SchedulerLimits::default(),
            )
            .unwrap();
        assert_eq!(journal.load().unwrap().next_sequence, 4);
    }

    #[test]
    fn opaque_or_noncanonical_group_tokens_are_not_numeric_reservations() {
        let plan = MergePlan::new(["left", "right"]).unwrap();
        for token in [
            "legacy-group",
            "merge-0",
            "merge-01",
            "merge-+1",
            "merge-9223372036854775807",
        ] {
            assert_eq!(
                merge_reserved_sequence(&ContinuationToken::from_wire(token.into()), &plan),
                0
            );
        }
        assert_eq!(
            merge_reserved_sequence(&ContinuationToken::from_wire("merge-99".into()), &plan),
            99
        );
    }

    #[test]
    fn claim_query_uses_active_attempt_order_index_without_temporary_sort() {
        let root = TempRoot::new();
        let journal = SqliteJournal::open(&root.0.join("flow.sqlite3")).unwrap();
        let mut query = journal
            .connection
            .prepare(&format!("EXPLAIN QUERY PLAN {CLAIM_DISPATCH_SQL}"))
            .unwrap();
        let plan = query
            .query_map([100], |row| row.get::<_, String>(3))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert!(
            plan.iter()
                .any(|detail| detail.contains("vm_dispatch_attempt_order_idx")),
            "claim did not use the active ordering index: {plan:?}"
        );
        assert!(
            plan.iter().all(|detail| !detail.contains("TEMP B-TREE")),
            "claim performed a temporary sort: {plan:?}"
        );
    }
}
