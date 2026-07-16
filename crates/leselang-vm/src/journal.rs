use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::time::Duration;

use rusqlite::limits::Limit;
use rusqlite::{Connection, OpenFlags, OptionalExtension, TransactionBehavior, params};

use crate::{
    BranchCompletion, BranchOutcome, Cancellation, CancellationReason, ContinuationImage,
    ContinuationToken, DEFAULT_MAX_OUTPUT_ITEMS, DebuggerAuditContext, DebuggerAuditRecord,
    DispatchLease, EffectError, EffectRequest, Fault, MAX_CONTINUATION_BYTES,
    MAX_DISPATCH_ATTEMPTS, MAX_DISPATCH_LEASE_MS, MAX_SEMANTIC_RETRIES, MergePlan, RetentionPolicy,
    RetryDisposition, Step, continuation_age_order, encode_json_capped, merge_declared,
    valid_continuation_token, validate_effect_error, validate_effect_request, validate_image,
    validate_merge_plan, validate_value,
};

pub const JOURNAL_SCHEMA_VERSION: u32 = 6;
pub const MAX_JOURNAL_RECORDS: usize = 10_000;
pub const MAX_JOURNAL_ENTRY_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_JOURNAL_TOTAL_BYTES: usize = 64 * 1024 * 1024;
const JOURNAL_BUSY_TIMEOUT: Duration = Duration::from_secs(5);

pub(crate) struct JournalSnapshot {
    pub pending: BTreeMap<ContinuationToken, ContinuationImage>,
    pub completed: BTreeMap<ContinuationToken, Step>,
    pub next_sequence: u64,
}

pub(crate) struct JournalCompaction {
    pub removed_records: usize,
    pub pruned_tokens: Vec<ContinuationToken>,
    pub remaining_completed: usize,
    pub reclaimed_logical_bytes: usize,
}

pub(crate) enum MergeProgress {
    Standalone,
    Pending {
        merge_token: ContinuationToken,
        completed_branches: usize,
        total_branches: usize,
    },
    Completed {
        merge_token: ContinuationToken,
        step: Step,
    },
}

pub(crate) enum Journal {
    Ephemeral(EphemeralJournal),
    Sqlite(SqliteJournal),
}

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

pub(crate) struct SqliteJournal {
    connection: Connection,
}

struct JournalRecord {
    state: String,
    image: Vec<u8>,
    terminal_step: Option<Vec<u8>>,
}

struct DispatchRecord {
    request: Vec<u8>,
    state: String,
    attempt: u32,
    lease_expires_at_ms: Option<u64>,
    ready_at_ms: u64,
    retry_count: u32,
}

type RawDispatchRow = (Vec<u8>, String, i64, Option<i64>, i64, i64, Option<Vec<u8>>);

impl Journal {
    pub fn ephemeral() -> Self {
        Self::Ephemeral(EphemeralJournal::default())
    }

    pub fn open(path: &Path) -> Result<(Self, JournalSnapshot), Fault> {
        let journal = SqliteJournal::open(path)?;
        let snapshot = journal.load()?;
        Ok((Self::Sqlite(journal), snapshot))
    }

    pub fn allocate_sequence(&mut self, local_next: u64) -> Result<u64, Fault> {
        match self {
            Self::Ephemeral(_) => Ok(local_next),
            Self::Sqlite(journal) => journal.allocate_sequence(local_next),
        }
    }

    pub fn record_pending(
        &mut self,
        image: &ContinuationImage,
        request: Option<&EffectRequest>,
    ) -> Result<(), Fault> {
        match self {
            Self::Ephemeral(journal) => journal.record_pending(request),
            Self::Sqlite(journal) => journal.record_pending(image, request),
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn record_merge_graph(
        &mut self,
        group_token: &ContinuationToken,
        plan: &MergePlan,
        branches: &[(&str, &EffectRequest)],
    ) -> Result<(), Fault> {
        validate_merge_graph_input(group_token, plan, branches)?;
        match self {
            Self::Ephemeral(journal) => journal.record_merge_graph(group_token, plan, branches),
            Self::Sqlite(journal) => journal.record_merge_graph(group_token, plan, branches),
        }
    }

    pub fn claim_dispatch(
        &mut self,
        now_ms: u64,
        lease_ms: u64,
    ) -> Result<Option<DispatchLease>, Fault> {
        validate_lease_clock(now_ms, lease_ms)?;
        match self {
            Self::Ephemeral(journal) => journal.claim_dispatch(now_ms, lease_ms),
            Self::Sqlite(journal) => journal.claim_dispatch(now_ms, lease_ms),
        }
    }

    pub fn record_completed(
        &mut self,
        image: &ContinuationImage,
        step: &Step,
    ) -> Result<Step, Fault> {
        match self {
            Self::Ephemeral(journal) => journal.record_completed(image, step),
            Self::Sqlite(journal) => journal.record_completed(image, step),
        }
    }

    pub fn acknowledge_dispatch(
        &mut self,
        lease: &DispatchLease,
        now_ms: u64,
        step: &Step,
    ) -> Result<Step, Fault> {
        validate_lease_clock(now_ms, 1)?;
        match self {
            Self::Ephemeral(journal) => journal.acknowledge_dispatch(lease, now_ms, step),
            Self::Sqlite(journal) => journal.acknowledge_dispatch(lease, now_ms, step),
        }
    }

    pub fn cancel(&mut self, image: &ContinuationImage, step: &Step) -> Result<Step, Fault> {
        match self {
            Self::Ephemeral(journal) => journal.cancel(image, step),
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
            Self::Sqlite(journal) => journal.cancel_audited(image, step, audit, now_ms),
        }
    }

    pub fn debugger_audit(&self, command_id: &str) -> Result<Option<DebuggerAuditRecord>, Fault> {
        match self {
            Self::Ephemeral(journal) => Ok(journal
                .debugger_audits
                .get(command_id)
                .map(|entry| entry.record.clone())),
            Self::Sqlite(journal) => journal.debugger_audit(command_id),
        }
    }

    pub fn expire_due(&mut self, now_ms: u64) -> Result<Vec<(ContinuationImage, Step)>, Fault> {
        match self {
            Self::Ephemeral(journal) => journal.expire_due(now_ms),
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
            Self::Sqlite(journal) => journal.compact_completed(local_completed, policy),
        }
    }

    pub fn completed_exists(&self, token: &ContinuationToken) -> Result<bool, Fault> {
        match self {
            Self::Ephemeral(_) => Ok(true),
            Self::Sqlite(journal) => journal.completed_exists(token),
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn merge_result(&self, group_token: &ContinuationToken) -> Result<Option<Step>, Fault> {
        match self {
            Self::Ephemeral(journal) => Ok(journal.merge_result(group_token)),
            Self::Sqlite(journal) => journal.merge_result(group_token),
        }
    }

    pub fn merge_progress(&self, branch_token: &ContinuationToken) -> Result<MergeProgress, Fault> {
        match self {
            Self::Ephemeral(journal) => Ok(journal.merge_progress(branch_token)),
            Self::Sqlite(journal) => journal.merge_progress(branch_token),
        }
    }
}

impl EphemeralJournal {
    fn record_pending(&mut self, request: Option<&EffectRequest>) -> Result<(), Fault> {
        let Some(request) = request else {
            return Ok(());
        };
        validate_effect_request(request)?;
        let token = request.continuation.token.clone();
        if let Some(existing) = self.dispatches.get(&token) {
            return if existing.request == *request {
                Ok(())
            } else {
                Err(journal_fault(
                    "LSV4011",
                    "effect request conflicts with pending state",
                ))
            };
        }
        self.dispatches.insert(
            token,
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
        Ok(())
    }

    #[cfg_attr(not(test), allow(dead_code))]
    fn record_merge_graph(
        &mut self,
        group_token: &ContinuationToken,
        plan: &MergePlan,
        branches: &[(&str, &EffectRequest)],
    ) -> Result<(), Fault> {
        if self.merge_groups.contains_key(group_token)
            || self.dispatches.contains_key(group_token)
            || branches.iter().any(|(_, request)| {
                self.dispatches.contains_key(&request.continuation.token)
                    || self.merge_groups.contains_key(&request.continuation.token)
            })
        {
            return Err(journal_fault(
                "LSV4011",
                "merge graph token conflicts with pending state",
            ));
        }

        let branch_tokens = branches
            .iter()
            .map(|(_, request)| request.continuation.token.clone())
            .collect::<Vec<_>>();
        for (_, request) in branches {
            self.dispatches.insert(
                request.continuation.token.clone(),
                EphemeralDispatch {
                    request: (*request).clone(),
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
        self.merge_groups.insert(
            group_token.clone(),
            EphemeralMergeGroup {
                plan: plan.clone(),
                branch_tokens,
                terminal_step: None,
            },
        );
        Ok(())
    }

    fn claim_dispatch(
        &mut self,
        now_ms: u64,
        lease_ms: u64,
    ) -> Result<Option<DispatchLease>, Fault> {
        let candidate = self.dispatches.values_mut().find(|dispatch| {
            !dispatch.acknowledged
                && dispatch.ready_at_ms <= now_ms
                && dispatch
                    .lease_expires_at_ms
                    .is_none_or(|expires_at| expires_at <= now_ms)
        });
        let Some(dispatch) = candidate else {
            return Ok(None);
        };
        dispatch.attempt = next_attempt(dispatch.attempt)?;
        let lease_expires_at_ms = lease_expiration(now_ms, lease_ms)?;
        dispatch.lease_expires_at_ms = Some(lease_expires_at_ms);
        Ok(Some(DispatchLease {
            request: dispatch.request.clone(),
            attempt: dispatch.attempt,
            retry_count: dispatch.retry_count,
            lease_expires_at_ms,
        }))
    }

    fn record_completed(&mut self, image: &ContinuationImage, step: &Step) -> Result<Step, Fault> {
        if let Some(dispatch) = self.dispatches.get_mut(&image.token) {
            if dispatch.lease_expires_at_ms.is_some() && !dispatch.acknowledged {
                return Err(journal_fault(
                    "LSV4024",
                    "leased effect must be completed through dispatch acknowledgement",
                ));
            }
            dispatch.acknowledged = true;
            dispatch.terminal_step = Some(step.clone());
        }
        self.finalize_merge_group(&image.token)?;
        Ok(step.clone())
    }

    fn acknowledge_dispatch(
        &mut self,
        lease: &DispatchLease,
        now_ms: u64,
        step: &Step,
    ) -> Result<Step, Fault> {
        validate_terminal_step(step)?;
        let token = &lease.request.continuation.token;
        let Some(dispatch) = self.dispatches.get_mut(token) else {
            return Err(journal_fault("LSV4021", "dispatch lease is unknown"));
        };
        if dispatch.acknowledged {
            return Ok(step.clone());
        }
        if dispatch.request != lease.request
            || dispatch.attempt != lease.attempt
            || dispatch.retry_count != lease.retry_count
            || dispatch.lease_expires_at_ms != Some(lease.lease_expires_at_ms)
        {
            return Err(journal_fault(
                "LSV4022",
                "dispatch lease has been superseded",
            ));
        }
        if lease.lease_expires_at_ms < now_ms {
            return Err(journal_fault("LSV4023", "dispatch lease has expired"));
        }
        dispatch.acknowledged = true;
        dispatch.lease_expires_at_ms = None;
        dispatch.terminal_step = Some(step.clone());
        self.finalize_merge_group(token)?;
        Ok(step.clone())
    }

    fn cancel(&mut self, image: &ContinuationImage, step: &Step) -> Result<Step, Fault> {
        validate_cancellation_step(image, step)?;
        if let Some(dispatch) = self.dispatches.get_mut(&image.token) {
            dispatch.acknowledged = true;
            dispatch.lease_expires_at_ms = None;
            dispatch.terminal_step = Some(step.clone());
        }
        self.finalize_merge_group(&image.token)?;
        Ok(step.clone())
    }

    fn cancel_audited(
        &mut self,
        image: &ContinuationImage,
        step: &Step,
        audit: &DebuggerAuditContext,
        now_ms: u64,
    ) -> Result<(Step, DebuggerAuditRecord), Fault> {
        let command_id = audit.command_id.as_str();
        if let Some(existing) = self.debugger_audits.get(command_id) {
            if existing.token == image.token
                && existing.idempotency_key == audit.idempotency_key.as_str()
                && existing.principal_id == audit.principal.id
                && audit_matches_record(audit, &existing.record)
            {
                let authoritative = self
                    .dispatches
                    .get(&image.token)
                    .and_then(|dispatch| dispatch.terminal_step.clone())
                    .ok_or_else(|| {
                        journal_fault("LSV4032", "debugger audit has no terminal step")
                    })?;
                return Ok((authoritative, existing.record.clone()));
            }
            return Err(journal_fault("LSV4033", "debugger command audit conflicts"));
        }
        if self.debugger_audits.values().any(|existing| {
            existing.principal_id == audit.principal.id
                && existing.idempotency_key == audit.idempotency_key.as_str()
        }) {
            return Err(journal_fault(
                "LSV4033",
                "debugger idempotency key was reused",
            ));
        }
        let authoritative = self.cancel(image, step)?;
        let record = debugger_audit_record(audit, now_ms);
        self.debugger_audits.insert(
            command_id.to_string(),
            EphemeralDebuggerAudit {
                token: image.token.clone(),
                idempotency_key: audit.idempotency_key.as_str().to_string(),
                principal_id: audit.principal.id.clone(),
                record: record.clone(),
            },
        );
        Ok((authoritative, record))
    }

    fn expire_due(&mut self, now_ms: u64) -> Result<Vec<(ContinuationImage, Step)>, Fault> {
        let mut expired = Vec::new();
        for dispatch in self.dispatches.values_mut() {
            let image = &dispatch.request.continuation;
            if dispatch.acknowledged
                || image
                    .deadline_at_ms
                    .is_none_or(|deadline| deadline > now_ms)
            {
                continue;
            }
            let step = deadline_cancellation(image, now_ms);
            dispatch.acknowledged = true;
            dispatch.lease_expires_at_ms = None;
            dispatch.terminal_step = Some(step.clone());
            expired.push((image.clone(), step));
        }
        for (image, _) in &expired {
            self.finalize_merge_group(&image.token)?;
        }
        Ok(expired)
    }

    fn report_error(
        &mut self,
        lease: &DispatchLease,
        now_ms: u64,
        disposition: &RetryDisposition,
    ) -> Result<RetryDisposition, Fault> {
        let token = &lease.request.continuation.token;
        let Some(dispatch) = self.dispatches.get_mut(token) else {
            return Err(journal_fault("LSV4021", "dispatch lease is unknown"));
        };
        validate_retry_lease(dispatch, lease, now_ms)?;
        match disposition {
            RetryDisposition::Scheduled(schedule) => {
                if schedule.retry_count != dispatch.retry_count + 1
                    || schedule.ready_at_ms <= now_ms
                    || validate_effect_error(&schedule.error).is_err()
                {
                    return Err(journal_fault("LSV4026", "retry schedule is invalid"));
                }
                dispatch.retry_count = schedule.retry_count;
                dispatch.ready_at_ms = schedule.ready_at_ms;
                dispatch.last_error = Some(schedule.error.clone());
                dispatch.lease_expires_at_ms = None;
            }
            RetryDisposition::Terminal(step) => {
                validate_terminal_step(step)?;
                dispatch.acknowledged = true;
                dispatch.lease_expires_at_ms = None;
                dispatch.terminal_step = Some(step.clone());
            }
        }
        if matches!(disposition, RetryDisposition::Terminal(_)) {
            self.finalize_merge_group(token)?;
        }
        Ok(disposition.clone())
    }

    fn finalize_merge_group(&mut self, branch_token: &ContinuationToken) -> Result<(), Fault> {
        let group_token = self
            .merge_groups
            .iter()
            .find(|(_, group)| group.branch_tokens.contains(branch_token))
            .map(|(token, _)| token.clone());
        let Some(group_token) = group_token else {
            return Ok(());
        };
        let group = self
            .merge_groups
            .get(&group_token)
            .expect("located merge group remains present");
        if group.terminal_step.is_some() {
            return Ok(());
        }
        let plan = group.plan.clone();
        let branch_tokens = group.branch_tokens.clone();
        let mut completions = Vec::with_capacity(branch_tokens.len());
        for (branch, token) in plan.branches.iter().zip(branch_tokens) {
            let Some(step) = self
                .dispatches
                .get(&token)
                .and_then(|dispatch| dispatch.terminal_step.clone())
            else {
                return Ok(());
            };
            completions.push(BranchCompletion {
                branch: branch.clone(),
                outcome: terminal_step_outcome(step)?,
            });
        }
        let merged = merge_declared(&plan, completions, DEFAULT_MAX_OUTPUT_ITEMS)?;
        self.merge_groups
            .get_mut(&group_token)
            .expect("located merge group remains present")
            .terminal_step = Some(merged);
        Ok(())
    }

    fn merge_result(&self, group_token: &ContinuationToken) -> Option<Step> {
        self.merge_groups
            .get(group_token)
            .and_then(|group| group.terminal_step.clone())
    }

    fn merge_progress(&self, branch_token: &ContinuationToken) -> MergeProgress {
        let Some((merge_token, group)) = self
            .merge_groups
            .iter()
            .find(|(_, group)| group.branch_tokens.contains(branch_token))
        else {
            return MergeProgress::Standalone;
        };
        if let Some(step) = &group.terminal_step {
            return MergeProgress::Completed {
                merge_token: merge_token.clone(),
                step: step.clone(),
            };
        }
        MergeProgress::Pending {
            merge_token: merge_token.clone(),
            completed_branches: group
                .branch_tokens
                .iter()
                .filter(|token| {
                    self.dispatches
                        .get(*token)
                        .is_some_and(|dispatch| dispatch.terminal_step.is_some())
                })
                .count(),
            total_branches: group.branch_tokens.len(),
        }
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
        units.extend(self.merge_groups.iter().filter_map(|(token, group)| {
            group
                .terminal_step
                .as_ref()
                .map(|_| EphemeralCompactionUnit::Merge {
                    group_token: token.clone(),
                    age_token: group
                        .branch_tokens
                        .first()
                        .expect("validated merge group has branches")
                        .clone(),
                })
        }));
        units.sort_by(|left, right| continuation_age_order(left.age_token(), right.age_token()));
        let completed_count = units.len();
        let remove_count = completed_count
            .saturating_sub(policy.max_completed_records)
            .min(policy.max_delete_per_run);
        let mut reclaimed_logical_bytes = 0usize;
        let mut removed_tokens = Vec::with_capacity(remove_count);
        for unit in units.into_iter().take(remove_count) {
            let tokens = match unit {
                EphemeralCompactionUnit::Effect(token) => vec![token],
                EphemeralCompactionUnit::Merge { group_token, .. } => {
                    let group = self
                        .merge_groups
                        .remove(&group_token)
                        .expect("selected merge group remains present");
                    reclaimed_logical_bytes = reclaimed_logical_bytes
                        .saturating_add(
                            encode_json_capped(&group.plan, MAX_CONTINUATION_BYTES, "merge plan")?
                                .len(),
                        )
                        .saturating_add(
                            encode_json_capped(
                                group
                                    .terminal_step
                                    .as_ref()
                                    .expect("selected merge group is completed"),
                                MAX_JOURNAL_ENTRY_BYTES,
                                "merge result",
                            )?
                            .len(),
                        );
                    group.branch_tokens
                }
            };
            for token in tokens {
                if let Some((_, step)) = local_completed.iter().find(|(local, _)| local == &token) {
                    reclaimed_logical_bytes = reclaimed_logical_bytes.saturating_add(
                        encode_json_capped(step, MAX_JOURNAL_ENTRY_BYTES, "completed step")?.len(),
                    );
                }
                if let Some(dispatch) = self.dispatches.remove(&token) {
                    reclaimed_logical_bytes = reclaimed_logical_bytes.saturating_add(
                        encode_json_capped(
                            &dispatch.request,
                            MAX_JOURNAL_ENTRY_BYTES,
                            "effect request",
                        )?
                        .len(),
                    );
                    if let Some(error) = &dispatch.last_error {
                        reclaimed_logical_bytes = reclaimed_logical_bytes.saturating_add(
                            encode_json_capped(error, MAX_JOURNAL_ENTRY_BYTES, "effect error")?
                                .len(),
                        );
                    }
                }
                removed_tokens.push(token);
            }
        }
        Ok(JournalCompaction {
            removed_records: remove_count,
            pruned_tokens: removed_tokens,
            remaining_completed: completed_count.saturating_sub(remove_count),
            reclaimed_logical_bytes,
        })
    }
}

fn validate_retry_lease(
    dispatch: &EphemeralDispatch,
    lease: &DispatchLease,
    now_ms: u64,
) -> Result<(), Fault> {
    if dispatch.acknowledged
        || dispatch.request != lease.request
        || dispatch.attempt != lease.attempt
        || dispatch.retry_count != lease.retry_count
        || dispatch.ready_at_ms > now_ms
        || dispatch.lease_expires_at_ms != Some(lease.lease_expires_at_ms)
    {
        return Err(journal_fault(
            "LSV4022",
            "dispatch lease has been superseded",
        ));
    }
    if lease.lease_expires_at_ms < now_ms {
        return Err(journal_fault("LSV4023", "dispatch lease has expired"));
    }
    Ok(())
}

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
        connection
            .set_limit(
                Limit::SQLITE_LIMIT_LENGTH,
                i32::try_from(MAX_JOURNAL_ENTRY_BYTES + MAX_CONTINUATION_BYTES + 4096)
                    .expect("journal SQLite length limit fits i32"),
            )
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
        if version == 0 {
            connection
                .execute_batch(
                    "BEGIN IMMEDIATE;
                     CREATE TABLE IF NOT EXISTS vm_metadata (
                       singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
                       next_sequence INTEGER NOT NULL CHECK (next_sequence >= 1)
                     ) STRICT;
                     INSERT OR IGNORE INTO vm_metadata(singleton, next_sequence) VALUES (1, 1);
                     CREATE TABLE IF NOT EXISTS vm_effects (
                       token TEXT PRIMARY KEY,
                       state TEXT NOT NULL CHECK (state IN ('pending', 'completed')),
                       image BLOB NOT NULL,
                       deadline_at_ms INTEGER CHECK (deadline_at_ms IS NULL OR deadline_at_ms >= 0),
                       terminal_step BLOB,
                       CHECK ((state = 'pending' AND terminal_step IS NULL) OR
                              (state = 'completed' AND terminal_step IS NOT NULL))
                     ) STRICT;
                     CREATE TABLE IF NOT EXISTS vm_dispatches (
                       token TEXT PRIMARY KEY REFERENCES vm_effects(token) ON DELETE CASCADE,
                       request BLOB NOT NULL,
                       state TEXT NOT NULL CHECK (state IN ('ready', 'leased', 'acknowledged')),
                       attempt INTEGER NOT NULL CHECK (attempt >= 0),
                       lease_expires_at_ms INTEGER,
                       ready_at_ms INTEGER NOT NULL DEFAULT 0 CHECK (ready_at_ms >= 0),
                       retry_count INTEGER NOT NULL DEFAULT 0
                         CHECK (retry_count >= 0 AND retry_count <= 16),
                       last_error BLOB,
                       CHECK ((state = 'ready' AND lease_expires_at_ms IS NULL) OR
                              (state = 'leased' AND lease_expires_at_ms IS NOT NULL) OR
                              (state = 'acknowledged' AND lease_expires_at_ms IS NULL))
                     ) STRICT;
                     CREATE INDEX vm_effect_deadline_idx
                       ON vm_effects(state, deadline_at_ms);
                     CREATE INDEX vm_dispatch_ready_idx
                       ON vm_dispatches(state, ready_at_ms, lease_expires_at_ms);
                     PRAGMA user_version = 4;
                     COMMIT;",
                )
                .map_err(|error| journal_error("LSV4003", "failed to initialize journal", error))?;
        } else if version == 1 {
            connection
                .execute_batch(
                    "BEGIN IMMEDIATE;
                     CREATE TABLE vm_dispatches (
                       token TEXT PRIMARY KEY REFERENCES vm_effects(token) ON DELETE CASCADE,
                       request BLOB NOT NULL,
                       state TEXT NOT NULL CHECK (state IN ('ready', 'leased', 'acknowledged')),
                       attempt INTEGER NOT NULL CHECK (attempt >= 0),
                       lease_expires_at_ms INTEGER,
                       ready_at_ms INTEGER NOT NULL DEFAULT 0 CHECK (ready_at_ms >= 0),
                       retry_count INTEGER NOT NULL DEFAULT 0
                         CHECK (retry_count >= 0 AND retry_count <= 16),
                       last_error BLOB,
                       CHECK ((state = 'ready' AND lease_expires_at_ms IS NULL) OR
                              (state = 'leased' AND lease_expires_at_ms IS NOT NULL) OR
                              (state = 'acknowledged' AND lease_expires_at_ms IS NULL))
                     ) STRICT;
                     ALTER TABLE vm_effects ADD COLUMN deadline_at_ms INTEGER
                       CHECK (deadline_at_ms IS NULL OR deadline_at_ms >= 0);
                     CREATE INDEX vm_effect_deadline_idx
                       ON vm_effects(state, deadline_at_ms);
                     CREATE INDEX vm_dispatch_ready_idx
                       ON vm_dispatches(state, ready_at_ms, lease_expires_at_ms);
                     PRAGMA user_version = 4;
                     COMMIT;",
                )
                .map_err(|error| journal_error("LSV4003", "failed to migrate journal", error))?;
        } else if version == 2 {
            connection
                .execute_batch(
                    "BEGIN IMMEDIATE;
                     ALTER TABLE vm_effects ADD COLUMN deadline_at_ms INTEGER
                       CHECK (deadline_at_ms IS NULL OR deadline_at_ms >= 0);
                     CREATE INDEX vm_effect_deadline_idx
                       ON vm_effects(state, deadline_at_ms);
                     ALTER TABLE vm_dispatches ADD COLUMN ready_at_ms INTEGER NOT NULL DEFAULT 0
                       CHECK (ready_at_ms >= 0);
                     ALTER TABLE vm_dispatches ADD COLUMN retry_count INTEGER NOT NULL DEFAULT 0
                       CHECK (retry_count >= 0 AND retry_count <= 16);
                     ALTER TABLE vm_dispatches ADD COLUMN last_error BLOB;
                     CREATE INDEX vm_dispatch_ready_idx
                       ON vm_dispatches(state, ready_at_ms, lease_expires_at_ms);
                     PRAGMA user_version = 4;
                     COMMIT;",
                )
                .map_err(|error| journal_error("LSV4003", "failed to migrate journal", error))?;
        } else if version == 3 {
            connection
                .execute_batch(
                    "BEGIN IMMEDIATE;
                     ALTER TABLE vm_dispatches ADD COLUMN ready_at_ms INTEGER NOT NULL DEFAULT 0
                       CHECK (ready_at_ms >= 0);
                     ALTER TABLE vm_dispatches ADD COLUMN retry_count INTEGER NOT NULL DEFAULT 0
                       CHECK (retry_count >= 0 AND retry_count <= 16);
                     ALTER TABLE vm_dispatches ADD COLUMN last_error BLOB;
                     CREATE INDEX vm_dispatch_ready_idx
                       ON vm_dispatches(state, ready_at_ms, lease_expires_at_ms);
                     PRAGMA user_version = 4;
                     COMMIT;",
                )
                .map_err(|error| journal_error("LSV4003", "failed to migrate journal", error))?;
        } else if !matches!(version, 4 | 5 | JOURNAL_SCHEMA_VERSION) {
            return Err(journal_fault(
                "LSV4004",
                format!("unsupported journal version {version}, expected 1 through 6"),
            ));
        }

        if version < 5 {
            connection
                .execute_batch(
                    "BEGIN IMMEDIATE;
                 CREATE TABLE vm_merge_groups (
                   token TEXT PRIMARY KEY,
                   plan BLOB NOT NULL,
                   state TEXT NOT NULL CHECK (state IN ('pending', 'completed')),
                   terminal_step BLOB,
                   CHECK ((state = 'pending' AND terminal_step IS NULL) OR
                          (state = 'completed' AND terminal_step IS NOT NULL))
                 ) STRICT;
                 CREATE TABLE vm_merge_branches (
                   group_token TEXT NOT NULL
                     REFERENCES vm_merge_groups(token) ON DELETE CASCADE,
                   branch_token TEXT NOT NULL UNIQUE
                     REFERENCES vm_effects(token) ON DELETE CASCADE,
                   branch_name TEXT NOT NULL,
                   position INTEGER NOT NULL CHECK (position >= 0 AND position < 64),
                   PRIMARY KEY (group_token, branch_name),
                   UNIQUE (group_token, position)
                 ) STRICT;
                 CREATE INDEX vm_merge_branch_group_idx
                   ON vm_merge_branches(group_token, position);
                 PRAGMA user_version = 5;
                 COMMIT;",
                )
                .map_err(|error| {
                    journal_error("LSV4003", "failed to migrate merge journal", error)
                })?;
        }

        if version < 6 {
            connection
                .execute_batch(
                    "BEGIN IMMEDIATE;
                     CREATE TABLE vm_debugger_audit (
                       command_id TEXT PRIMARY KEY,
                       principal_id TEXT NOT NULL,
                       idempotency_key TEXT NOT NULL,
                       origin TEXT NOT NULL CHECK (origin IN ('gui', 'cli', 'leselang', 'model', 'compatibility_adapter')),
                       session_id TEXT NOT NULL,
                       expected_revision INTEGER NOT NULL CHECK (expected_revision >= 0),
                       observed_at_ms INTEGER NOT NULL CHECK (observed_at_ms >= 0),
                       continuation_token TEXT NOT NULL
                         REFERENCES vm_effects(token) ON DELETE CASCADE,
                       UNIQUE (principal_id, idempotency_key)
                     ) STRICT;
                     CREATE INDEX vm_debugger_audit_token_idx
                       ON vm_debugger_audit(continuation_token);
                     PRAGMA user_version = 6;
                     COMMIT;",
                )
                .map_err(|error| {
                    journal_error("LSV4003", "failed to migrate debugger audit journal", error)
                })?;
        }

        Ok(Self { connection })
    }

    fn load(&self) -> Result<JournalSnapshot, Fault> {
        let count = journal_record_count(&self.connection)?;
        if count > MAX_JOURNAL_RECORDS {
            return Err(journal_fault(
                "LSV4006",
                format!("journal has {count} records, limit is {MAX_JOURNAL_RECORDS}"),
            ));
        }
        let total_bytes = journal_payload_bytes(&self.connection)?;
        if total_bytes > MAX_JOURNAL_TOTAL_BYTES {
            return Err(journal_fault(
                "LSV4006",
                format!(
                    "journal payload is {total_bytes} bytes, limit is {MAX_JOURNAL_TOTAL_BYTES}"
                ),
            ));
        }

        let next_sequence: i64 = self
            .connection
            .query_row(
                "SELECT next_sequence FROM vm_metadata WHERE singleton = 1",
                [],
                |row| row.get(0),
            )
            .map_err(|error| journal_error("LSV4005", "failed to load journal metadata", error))?;
        let mut statement = self
            .connection
            .prepare(
                "SELECT token, state, image, deadline_at_ms, terminal_step
                 FROM vm_effects ORDER BY token ASC",
            )
            .map_err(|error| journal_error("LSV4005", "failed to prepare journal load", error))?;
        let mut rows = statement
            .query([])
            .map_err(|error| journal_error("LSV4005", "failed to load journal", error))?;
        let mut pending = BTreeMap::new();
        let mut completed = BTreeMap::new();

        while let Some(row) = rows
            .next()
            .map_err(|error| journal_error("LSV4005", "failed to read journal record", error))?
        {
            let token: String = row
                .get(0)
                .map_err(|error| journal_error("LSV4005", "invalid journal token", error))?;
            let state: String = row
                .get(1)
                .map_err(|error| journal_error("LSV4005", "invalid journal state", error))?;
            let image_bytes: Vec<u8> = row
                .get(2)
                .map_err(|error| journal_error("LSV4005", "invalid journal image", error))?;
            let image: ContinuationImage = decode_bounded(&image_bytes, MAX_CONTINUATION_BYTES)?;
            validate_image(&image)?;
            if image.token.as_str() != token {
                return Err(journal_fault(
                    "LSV4007",
                    "journal token does not match continuation image",
                ));
            }
            let stored_deadline: Option<i64> = row.get(3).map_err(|error| {
                journal_error("LSV4005", "invalid journal effect deadline", error)
            })?;
            let stored_deadline = stored_deadline
                .map(|value| {
                    u64::try_from(value)
                        .map_err(|_| journal_fault("LSV4007", "effect deadline is invalid"))
                })
                .transpose()?;
            if stored_deadline != image.deadline_at_ms {
                return Err(journal_fault(
                    "LSV4007",
                    "journal deadline does not match continuation image",
                ));
            }

            match state.as_str() {
                "pending" => {
                    pending.insert(image.token.clone(), image);
                }
                "completed" => {
                    let step_bytes: Vec<u8> = row.get(4).map_err(|error| {
                        journal_error("LSV4005", "invalid completed journal record", error)
                    })?;
                    let step: Step = decode_bounded(&step_bytes, MAX_JOURNAL_ENTRY_BYTES)?;
                    validate_terminal_step(&step)?;
                    completed.insert(image.token, step);
                }
                _ => return Err(journal_fault("LSV4007", "invalid journal record state")),
            }
        }

        drop(rows);
        drop(statement);
        validate_dispatch_records(&self.connection)?;
        validate_merge_graph_records(&self.connection)?;
        validate_debugger_audit_records(&self.connection)?;

        let next_sequence = u64::try_from(next_sequence)
            .map_err(|_| journal_fault("LSV4007", "journal sequence is invalid"))?;
        Ok(JournalSnapshot {
            pending,
            completed,
            next_sequence: next_sequence.max(1),
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
        type RawProgress = (String, String, Option<Vec<u8>>, i64, i64);
        let progress: Option<RawProgress> = self
            .connection
            .query_row(
                "SELECT g.token, g.state, g.terminal_step,
                        SUM(CASE WHEN e.state = 'completed' THEN 1 ELSE 0 END),
                        COUNT(*)
                 FROM vm_merge_groups g
                 JOIN vm_merge_branches own ON own.group_token = g.token
                 JOIN vm_merge_branches branches ON branches.group_token = g.token
                 JOIN vm_effects e ON e.token = branches.branch_token
                 WHERE own.branch_token = ?1
                 GROUP BY g.token, g.state, g.terminal_step",
                [branch_token.as_str()],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .optional()
            .map_err(|error| journal_error("LSV4033", "failed to read merge progress", error))?;
        let Some((merge_token, state, terminal, completed, total)) = progress else {
            return Ok(MergeProgress::Standalone);
        };
        let merge_token = ContinuationToken(merge_token);
        let completed_branches = usize::try_from(completed)
            .map_err(|_| journal_fault("LSV4007", "merge completion count is invalid"))?;
        let total_branches = usize::try_from(total)
            .map_err(|_| journal_fault("LSV4007", "merge branch count is invalid"))?;
        match (state.as_str(), terminal) {
            ("pending", None) if completed_branches < total_branches => {
                Ok(MergeProgress::Pending {
                    merge_token,
                    completed_branches,
                    total_branches,
                })
            }
            ("completed", Some(bytes)) if completed_branches == total_branches => {
                let step: Step = decode_bounded(&bytes, MAX_JOURNAL_ENTRY_BYTES)?;
                validate_terminal_step(&step)?;
                Ok(MergeProgress::Completed { merge_token, step })
            }
            _ => Err(journal_fault(
                "LSV4007",
                "merge progress has an invalid durable state",
            )),
        }
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
        let next = sequence.checked_add(1).ok_or_else(|| {
            journal_fault("LSV4009", "journal continuation sequence is exhausted")
        })?;
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
        let request_bytes = request
            .map(|request| encode_json_capped(request, MAX_JOURNAL_ENTRY_BYTES, "effect request"))
            .transpose()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| journal_error("LSV4010", "failed to lock journal", error))?;
        let existing = load_record(&transaction, image.token.as_str())?;
        if let Some(existing) = existing {
            if existing.state == "pending" && existing.image == image_bytes {
                if request_bytes.is_some() {
                    ensure_dispatch_matches(
                        &transaction,
                        image.token.as_str(),
                        request.expect("request bytes require request"),
                    )?;
                }
                return transaction.commit().map_err(|error| {
                    journal_error("LSV4010", "failed to commit pending journal record", error)
                });
            }
            return Err(journal_fault(
                "LSV4011",
                "continuation token conflicts with durable journal state",
            ));
        }
        let count = journal_record_count(&transaction)?;
        if count >= MAX_JOURNAL_RECORDS {
            return Err(journal_fault(
                "LSV4006",
                format!("journal record limit {MAX_JOURNAL_RECORDS} reached"),
            ));
        }
        ensure_growth(
            &transaction,
            image_bytes.len() + request_bytes.as_ref().map_or(0, Vec::len),
        )?;
        transaction
            .execute(
                "INSERT INTO vm_effects(token, state, image, deadline_at_ms, terminal_step)
                 VALUES (?1, 'pending', ?2, ?3, NULL)",
                params![
                    image.token.as_str(),
                    image_bytes,
                    image
                        .deadline_at_ms
                        .map(i64::try_from)
                        .transpose()
                        .map_err(|_| {
                            journal_fault("LSV4010", "effect deadline is out of range")
                        })?
                ],
            )
            .map_err(|error| journal_error("LSV4010", "failed to store pending effect", error))?;
        if let Some(request_bytes) = request_bytes {
            transaction
                .execute(
                    "INSERT INTO vm_dispatches(token, request, state, attempt, lease_expires_at_ms)
                     VALUES (?1, ?2, 'ready', 0, NULL)",
                    params![image.token.as_str(), request_bytes],
                )
                .map_err(|error| {
                    journal_error("LSV4010", "failed to store effect dispatch", error)
                })?;
        }
        transaction
            .commit()
            .map_err(|error| journal_error("LSV4010", "failed to commit pending effect", error))
    }

    #[cfg_attr(not(test), allow(dead_code))]
    fn record_merge_graph(
        &mut self,
        group_token: &ContinuationToken,
        plan: &MergePlan,
        branches: &[(&str, &EffectRequest)],
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
        let current_count = journal_record_count(&transaction)?;
        let added_records = branches.len().saturating_add(1);
        if current_count.saturating_add(added_records) > MAX_JOURNAL_RECORDS {
            return Err(journal_fault(
                "LSV4006",
                format!("journal record limit {MAX_JOURNAL_RECORDS} reached"),
            ));
        }
        ensure_growth(&transaction, additional_bytes)?;

        let group_conflict: bool = transaction
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
        if group_conflict {
            return Err(journal_fault(
                "LSV4011",
                "merge graph token conflicts with durable state",
            ));
        }

        transaction
            .execute(
                "INSERT INTO vm_merge_groups(token, plan, state, terminal_step)
                 VALUES (?1, ?2, 'pending', NULL)",
                params![group_token.as_str(), plan_bytes],
            )
            .map_err(|error| journal_error("LSV4032", "failed to store merge group", error))?;
        for (position, ((_, request), (name, image_bytes, request_bytes))) in
            branches.iter().zip(encoded.iter()).enumerate()
        {
            let position = i64::try_from(position)
                .map_err(|_| journal_fault("LSV4032", "merge branch position is invalid"))?;
            transaction
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
            transaction
                .execute(
                    "INSERT INTO vm_dispatches(token, request, state, attempt, lease_expires_at_ms)
                     VALUES (?1, ?2, 'ready', 0, NULL)",
                    params![request.continuation.token.as_str(), request_bytes],
                )
                .map_err(|error| {
                    journal_error("LSV4032", "failed to store merge branch dispatch", error)
                })?;
            transaction
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
        }
        transaction
            .commit()
            .map_err(|error| journal_error("LSV4032", "failed to commit merge graph", error))
    }

    fn claim_dispatch(
        &mut self,
        now_ms: u64,
        lease_ms: u64,
    ) -> Result<Option<DispatchLease>, Fault> {
        let now = i64::try_from(now_ms)
            .map_err(|_| journal_fault("LSV4015", "dispatch clock is out of range"))?;
        let expires_at = lease_expiration(now_ms, lease_ms)?;
        let expires_at_sql = i64::try_from(expires_at)
            .map_err(|_| journal_fault("LSV4015", "dispatch lease expiration is out of range"))?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| journal_error("LSV4016", "failed to lock dispatch journal", error))?;
        let candidate = transaction
            .query_row(
                "SELECT d.token, d.request, d.attempt, d.retry_count
                 FROM vm_dispatches d
                 JOIN vm_effects e ON e.token = d.token
                 WHERE e.state = 'pending'
                   AND ((d.state = 'ready' AND d.ready_at_ms <= ?1) OR
                        (d.state = 'leased' AND d.lease_expires_at_ms <= ?1))
                 ORDER BY d.token ASC
                 LIMIT 1",
                [now],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Vec<u8>>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                },
            )
            .optional()
            .map_err(|error| journal_error("LSV4016", "failed to select effect dispatch", error))?;
        let Some((token, request_bytes, attempt, retry_count)) = candidate else {
            transaction.commit().map_err(|error| {
                journal_error("LSV4016", "failed to close dispatch transaction", error)
            })?;
            return Ok(None);
        };
        let request: EffectRequest = decode_bounded(&request_bytes, MAX_JOURNAL_ENTRY_BYTES)?;
        validate_effect_request(&request)?;
        if request.continuation.token.as_str() != token {
            return Err(journal_fault(
                "LSV4007",
                "dispatch token does not match effect request",
            ));
        }
        let attempt = u32::try_from(attempt)
            .map_err(|_| journal_fault("LSV4007", "dispatch attempt is invalid"))?;
        let attempt = next_attempt(attempt)?;
        let retry_count = u32::try_from(retry_count)
            .map_err(|_| journal_fault("LSV4007", "dispatch retry count is invalid"))?;
        if retry_count > MAX_SEMANTIC_RETRIES {
            return Err(journal_fault(
                "LSV4007",
                "dispatch retry count exceeds runtime limit",
            ));
        }
        transaction
            .execute(
                "UPDATE vm_dispatches
                 SET state = 'leased', attempt = ?2, lease_expires_at_ms = ?3
                 WHERE token = ?1",
                params![token, i64::from(attempt), expires_at_sql],
            )
            .map_err(|error| journal_error("LSV4016", "failed to lease effect dispatch", error))?;
        transaction
            .commit()
            .map_err(|error| journal_error("LSV4016", "failed to commit effect lease", error))?;
        Ok(Some(DispatchLease {
            request,
            attempt,
            retry_count,
            lease_expires_at_ms: expires_at,
        }))
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
        let Some(existing) = load_record(&transaction, image.token.as_str())? else {
            return Err(journal_fault(
                "LSV4013",
                "pending continuation is missing from durable journal",
            ));
        };
        if existing.image != image_bytes {
            return Err(journal_fault(
                "LSV4014",
                "continuation image conflicts with durable journal state",
            ));
        }
        if existing.state == "completed" {
            let bytes = existing.terminal_step.ok_or_else(|| {
                journal_fault("LSV4007", "completed journal record has no terminal step")
            })?;
            let authoritative: Step = decode_bounded(&bytes, MAX_JOURNAL_ENTRY_BYTES)?;
            validate_terminal_step(&authoritative)?;
            transaction.commit().map_err(|error| {
                journal_error("LSV4012", "failed to close journal transaction", error)
            })?;
            return Ok(authoritative);
        }
        if let Some(dispatch) = load_dispatch(&transaction, image.token.as_str())?
            && dispatch.state == "leased"
        {
            return Err(journal_fault(
                "LSV4024",
                "leased effect must be completed through dispatch acknowledgement",
            ));
        }
        ensure_growth(&transaction, step_bytes.len())?;
        transaction
            .execute(
                "UPDATE vm_effects SET state = 'completed', terminal_step = ?2 WHERE token = ?1",
                params![image.token.as_str(), step_bytes],
            )
            .map_err(|error| journal_error("LSV4012", "failed to complete effect", error))?;
        transaction
            .execute(
                "UPDATE vm_dispatches
                 SET state = 'acknowledged', lease_expires_at_ms = NULL
                 WHERE token = ?1",
                [image.token.as_str()],
            )
            .map_err(|error| journal_error("LSV4012", "failed to acknowledge dispatch", error))?;
        finalize_merge_group(&transaction, image.token.as_str())?;
        transaction.commit().map_err(|error| {
            journal_error("LSV4012", "failed to commit completed effect", error)
        })?;
        Ok(step.clone())
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
        let Some(existing) = load_record(&transaction, image.token.as_str())? else {
            return Err(journal_fault("LSV4021", "dispatch continuation is unknown"));
        };
        if existing.image != image_bytes {
            return Err(journal_fault(
                "LSV4014",
                "dispatch continuation conflicts with durable journal state",
            ));
        }
        if existing.state == "completed" {
            let bytes = existing.terminal_step.ok_or_else(|| {
                journal_fault("LSV4007", "completed journal record has no terminal step")
            })?;
            let authoritative: Step = decode_bounded(&bytes, MAX_JOURNAL_ENTRY_BYTES)?;
            validate_terminal_step(&authoritative)?;
            transaction.commit().map_err(|error| {
                journal_error("LSV4020", "failed to close dispatch transaction", error)
            })?;
            return Ok(authoritative);
        }
        let Some(dispatch) = load_dispatch(&transaction, image.token.as_str())? else {
            return Err(journal_fault("LSV4021", "dispatch lease is unknown"));
        };
        let durable_request: EffectRequest =
            decode_bounded(&dispatch.request, MAX_JOURNAL_ENTRY_BYTES)?;
        if durable_request != lease.request
            || dispatch.state != "leased"
            || dispatch.attempt != lease.attempt
            || dispatch.retry_count != lease.retry_count
            || dispatch.ready_at_ms > now_ms
            || dispatch.lease_expires_at_ms != Some(lease.lease_expires_at_ms)
        {
            return Err(journal_fault(
                "LSV4022",
                "dispatch lease has been superseded",
            ));
        }
        if lease.lease_expires_at_ms < now_ms {
            return Err(journal_fault("LSV4023", "dispatch lease has expired"));
        }
        ensure_growth(&transaction, step_bytes.len())?;
        transaction
            .execute(
                "UPDATE vm_effects SET state = 'completed', terminal_step = ?2 WHERE token = ?1",
                params![image.token.as_str(), step_bytes],
            )
            .map_err(|error| journal_error("LSV4020", "failed to complete dispatch", error))?;
        transaction
            .execute(
                "UPDATE vm_dispatches
                 SET state = 'acknowledged', lease_expires_at_ms = NULL
                 WHERE token = ?1",
                [image.token.as_str()],
            )
            .map_err(|error| journal_error("LSV4020", "failed to acknowledge dispatch", error))?;
        finalize_merge_group(&transaction, image.token.as_str())?;
        transaction
            .commit()
            .map_err(|error| journal_error("LSV4020", "failed to commit dispatch result", error))?;
        Ok(step.clone())
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
        let Some(existing) = load_record(&transaction, image.token.as_str())? else {
            return Err(journal_fault("LSV4021", "dispatch continuation is unknown"));
        };
        if existing.image != image_bytes {
            return Err(journal_fault(
                "LSV4014",
                "dispatch continuation conflicts with durable journal state",
            ));
        }
        if existing.state == "completed" {
            let authoritative: Step = decode_bounded(
                &existing.terminal_step.ok_or_else(|| {
                    journal_fault("LSV4007", "completed journal record has no terminal step")
                })?,
                MAX_JOURNAL_ENTRY_BYTES,
            )?;
            validate_terminal_step(&authoritative)?;
            transaction.commit().map_err(|error| {
                journal_error("LSV4026", "failed to close retry transaction", error)
            })?;
            return Ok(RetryDisposition::Terminal(authoritative));
        }
        let Some(dispatch) = load_dispatch(&transaction, image.token.as_str())? else {
            return Err(journal_fault("LSV4021", "dispatch lease is unknown"));
        };
        let durable_request: EffectRequest =
            decode_bounded(&dispatch.request, MAX_JOURNAL_ENTRY_BYTES)?;
        if durable_request != lease.request
            || dispatch.state != "leased"
            || dispatch.attempt != lease.attempt
            || dispatch.retry_count != lease.retry_count
            || dispatch.ready_at_ms > now_ms
            || dispatch.lease_expires_at_ms != Some(lease.lease_expires_at_ms)
        {
            return Err(journal_fault(
                "LSV4022",
                "dispatch lease has been superseded",
            ));
        }
        if lease.lease_expires_at_ms < now_ms {
            return Err(journal_fault("LSV4023", "dispatch lease has expired"));
        }

        match disposition {
            RetryDisposition::Scheduled(schedule) => {
                if schedule.retry_count != dispatch.retry_count + 1
                    || schedule.retry_count > MAX_SEMANTIC_RETRIES
                    || schedule.ready_at_ms <= now_ms
                    || schedule.ready_at_ms > i64::MAX as u64
                    || validate_effect_error(&schedule.error).is_err()
                {
                    return Err(journal_fault("LSV4026", "retry schedule is invalid"));
                }
                let error_bytes = encode_json_capped(&schedule.error, 2_048, "effect error")?;
                ensure_growth(&transaction, error_bytes.len())?;
                transaction
                    .execute(
                        "UPDATE vm_dispatches
                         SET state = 'ready', retry_count = ?2, ready_at_ms = ?3,
                             last_error = ?4, lease_expires_at_ms = NULL
                         WHERE token = ?1",
                        params![
                            image.token.as_str(),
                            i64::from(schedule.retry_count),
                            i64::try_from(schedule.ready_at_ms).expect("validated retry clock"),
                            error_bytes
                        ],
                    )
                    .map_err(|error| {
                        journal_error("LSV4026", "failed to schedule effect retry", error)
                    })?;
            }
            RetryDisposition::Terminal(step) => {
                validate_terminal_step(step)?;
                let step_bytes =
                    encode_json_capped(step, MAX_JOURNAL_ENTRY_BYTES, "terminal step")?;
                ensure_growth(&transaction, step_bytes.len())?;
                complete_terminal_record(&transaction, image.token.as_str(), &step_bytes)?;
                finalize_merge_group(&transaction, image.token.as_str())?;
            }
        }
        transaction
            .commit()
            .map_err(|error| journal_error("LSV4026", "failed to commit effect retry", error))?;
        Ok(disposition.clone())
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
        let Some(existing) = load_record(&transaction, image.token.as_str())? else {
            return Err(journal_fault("LSV4013", "pending continuation is missing"));
        };
        if existing.image != image_bytes {
            return Err(journal_fault(
                "LSV4014",
                "continuation image conflicts with durable journal state",
            ));
        }
        if existing.state == "completed" {
            let authoritative: Step = decode_bounded(
                &existing.terminal_step.ok_or_else(|| {
                    journal_fault("LSV4007", "completed journal record has no terminal step")
                })?,
                MAX_JOURNAL_ENTRY_BYTES,
            )?;
            validate_terminal_step(&authoritative)?;
            transaction.commit().map_err(|error| {
                journal_error("LSV4025", "failed to close cancellation transaction", error)
            })?;
            return Ok(authoritative);
        }
        ensure_growth(&transaction, step_bytes.len())?;
        complete_terminal_record(&transaction, image.token.as_str(), &step_bytes)?;
        finalize_merge_group(&transaction, image.token.as_str())?;
        transaction.commit().map_err(|error| {
            journal_error("LSV4025", "failed to commit effect cancellation", error)
        })?;
        Ok(step.clone())
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
        if let Some((existing_command, token, idempotency_key, record)) =
            load_debugger_audit(&transaction, audit)?
        {
            if existing_command == audit.command_id.as_str()
                && token == image.token.as_str()
                && idempotency_key == audit.idempotency_key.as_str()
                && audit_matches_record(audit, &record)
            {
                let existing = load_record(&transaction, image.token.as_str())?
                    .ok_or_else(|| journal_fault("LSV4032", "debugger audit effect is missing"))?;
                let authoritative: Step = decode_bounded(
                    &existing.terminal_step.ok_or_else(|| {
                        journal_fault("LSV4032", "debugger audit has no terminal step")
                    })?,
                    MAX_JOURNAL_ENTRY_BYTES,
                )?;
                transaction.commit().map_err(|error| {
                    journal_error("LSV4032", "failed to close debugger audit replay", error)
                })?;
                return Ok((authoritative, record));
            }
            return Err(journal_fault("LSV4033", "debugger command audit conflicts"));
        }
        let existing = load_record(&transaction, image.token.as_str())?
            .ok_or_else(|| journal_fault("LSV4013", "pending continuation is missing"))?;
        if existing.image != image_bytes {
            return Err(journal_fault(
                "LSV4014",
                "continuation image conflicts with durable journal state",
            ));
        }
        if existing.state == "completed" {
            return Err(journal_fault(
                "LSV4033",
                "completed effect has no matching debugger audit",
            ));
        }
        let record = debugger_audit_record(audit, now_ms);
        transaction
            .execute(
                "INSERT INTO vm_debugger_audit(
                    command_id, principal_id, idempotency_key, origin, session_id,
                    expected_revision, observed_at_ms, continuation_token
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    audit.command_id.as_str(),
                    audit.principal.id.as_str(),
                    audit.idempotency_key.as_str(),
                    command_origin_label(audit.origin),
                    audit.session_id.as_str(),
                    i64::try_from(audit.expected_revision.0).map_err(|_| {
                        journal_fault("LSV4033", "debugger audit revision is out of range")
                    })?,
                    i64::try_from(now_ms).map_err(|_| {
                        journal_fault("LSV4033", "debugger audit clock is out of range")
                    })?,
                    image.token.as_str(),
                ],
            )
            .map_err(|error| {
                journal_error(
                    "LSV4033",
                    "failed to store debugger cancellation audit",
                    error,
                )
            })?;
        ensure_growth(&transaction, step_bytes.len())?;
        complete_terminal_record(&transaction, image.token.as_str(), &step_bytes)?;
        finalize_merge_group(&transaction, image.token.as_str())?;
        transaction.commit().map_err(|error| {
            journal_error(
                "LSV4032",
                "failed to commit debugger cancellation audit",
                error,
            )
        })?;
        Ok((step.clone(), record))
    }

    fn debugger_audit(&self, command_id: &str) -> Result<Option<DebuggerAuditRecord>, Fault> {
        self.connection
            .query_row(
                "SELECT command_id, principal_id, origin, session_id,
                        expected_revision, observed_at_ms
                 FROM vm_debugger_audit WHERE command_id = ?1",
                [command_id],
                decode_debugger_audit_row,
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
        let mut statement = transaction
            .prepare(
                "SELECT token, image FROM vm_effects
                 WHERE state = 'pending' AND deadline_at_ms IS NOT NULL AND deadline_at_ms <= ?1
                 ORDER BY token ASC",
            )
            .map_err(|error| journal_error("LSV4025", "failed to prepare timeout scan", error))?;
        let records = statement
            .query_map([now], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?))
            })
            .map_err(|error| journal_error("LSV4025", "failed to scan timed effects", error))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| journal_error("LSV4025", "failed to read timed effect", error))?;
        drop(statement);

        let mut expired = Vec::with_capacity(records.len());
        for (token, image_bytes) in records {
            let image: ContinuationImage = decode_bounded(&image_bytes, MAX_CONTINUATION_BYTES)?;
            validate_image(&image)?;
            if image.token.as_str() != token
                || image
                    .deadline_at_ms
                    .is_none_or(|deadline| deadline > now_ms)
            {
                return Err(journal_fault(
                    "LSV4007",
                    "timed effect deadline conflicts with continuation image",
                ));
            }
            let step = deadline_cancellation(&image, now_ms);
            let step_bytes = encode_json_capped(&step, MAX_JOURNAL_ENTRY_BYTES, "terminal step")?;
            ensure_growth(&transaction, step_bytes.len())?;
            complete_terminal_record(&transaction, &token, &step_bytes)?;
            finalize_merge_group(&transaction, &token)?;
            expired.push((image, step));
        }
        transaction
            .commit()
            .map_err(|error| journal_error("LSV4025", "failed to commit effect timeouts", error))?;
        Ok(expired)
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
        let remove_count = completed_count
            .saturating_sub(policy.max_completed_records)
            .min(policy.max_delete_per_run);
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

fn delete_completed_merge_group(
    transaction: &rusqlite::Transaction<'_>,
    group_token: &str,
) -> Result<usize, Fault> {
    let group_bytes: i64 = transaction
        .query_row(
            "SELECT length(plan) + length(terminal_step)
             FROM vm_merge_groups WHERE token = ?1 AND state = 'completed'",
            [group_token],
            |row| row.get(0),
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
    if !(2..=crate::MAX_MERGE_BRANCHES).contains(&branches.len())
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

fn debugger_audit_record(audit: &DebuggerAuditContext, observed_at_ms: u64) -> DebuggerAuditRecord {
    DebuggerAuditRecord {
        command_id: audit.command_id.clone(),
        principal_id: audit.principal.id.clone(),
        origin: audit.origin,
        session_id: audit.session_id.clone(),
        expected_revision: audit.expected_revision,
        observed_at_ms,
    }
}

fn audit_matches_record(audit: &DebuggerAuditContext, record: &DebuggerAuditRecord) -> bool {
    record.command_id == audit.command_id
        && record.principal_id == audit.principal.id
        && record.origin == audit.origin
        && record.session_id == audit.session_id
        && record.expected_revision == audit.expected_revision
}

fn command_origin_label(origin: leserpent_domain::CommandOrigin) -> &'static str {
    match origin {
        leserpent_domain::CommandOrigin::Gui => "gui",
        leserpent_domain::CommandOrigin::Cli => "cli",
        leserpent_domain::CommandOrigin::Leselang => "leselang",
        leserpent_domain::CommandOrigin::Model => "model",
        leserpent_domain::CommandOrigin::CompatibilityAdapter => "compatibility_adapter",
    }
}

fn parse_command_origin(value: &str) -> rusqlite::Result<leserpent_domain::CommandOrigin> {
    match value {
        "gui" => Ok(leserpent_domain::CommandOrigin::Gui),
        "cli" => Ok(leserpent_domain::CommandOrigin::Cli),
        "leselang" => Ok(leserpent_domain::CommandOrigin::Leselang),
        "model" => Ok(leserpent_domain::CommandOrigin::Model),
        "compatibility_adapter" => Ok(leserpent_domain::CommandOrigin::CompatibilityAdapter),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

fn decode_debugger_audit_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<DebuggerAuditRecord> {
    let command_id = crate::CommandId::new(row.get::<_, String>(0)?)
        .map_err(|_| rusqlite::Error::InvalidQuery)?;
    let principal_id = row.get::<_, String>(1)?;
    let origin = parse_command_origin(&row.get::<_, String>(2)?)?;
    let session_id = row.get::<_, String>(3)?;
    let expected_revision =
        u64::try_from(row.get::<_, i64>(4)?).map_err(|_| rusqlite::Error::InvalidQuery)?;
    let observed_at_ms =
        u64::try_from(row.get::<_, i64>(5)?).map_err(|_| rusqlite::Error::InvalidQuery)?;
    Ok(DebuggerAuditRecord {
        command_id,
        principal_id,
        origin,
        session_id,
        expected_revision: crate::Revision(expected_revision),
        observed_at_ms,
    })
}

fn load_debugger_audit(
    transaction: &rusqlite::Transaction<'_>,
    audit: &DebuggerAuditContext,
) -> Result<Option<(String, String, String, DebuggerAuditRecord)>, Fault> {
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
            |row| {
                let command_id = row.get::<_, String>(0)?;
                let token = row.get::<_, String>(1)?;
                let idempotency_key = row.get::<_, String>(2)?;
                let record = DebuggerAuditRecord {
                    command_id: crate::CommandId::new(command_id.clone())
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    principal_id: row.get(3)?,
                    origin: parse_command_origin(&row.get::<_, String>(4)?)?,
                    session_id: row.get(5)?,
                    expected_revision: crate::Revision(
                        u64::try_from(row.get::<_, i64>(6)?)
                            .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    ),
                    observed_at_ms: u64::try_from(row.get::<_, i64>(7)?)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                };
                Ok((command_id, token, idempotency_key, record))
            },
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

fn finalize_merge_group(
    transaction: &rusqlite::Transaction<'_>,
    branch_token: &str,
) -> Result<(), Fault> {
    type RawGroup = (String, Vec<u8>, String, Option<Vec<u8>>);
    let group: Option<RawGroup> = transaction
        .query_row(
            "SELECT g.token, g.plan, g.state, g.terminal_step
             FROM vm_merge_groups g
             JOIN vm_merge_branches b ON b.group_token = g.token
             WHERE b.branch_token = ?1",
            [branch_token],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()
        .map_err(|error| journal_error("LSV4033", "failed to locate merge group", error))?;
    let Some((group_token, plan_bytes, state, terminal_bytes)) = group else {
        return Ok(());
    };
    if state == "completed" {
        let terminal: Step = decode_bounded(
            &terminal_bytes.ok_or_else(|| {
                journal_fault("LSV4007", "completed merge group has no terminal step")
            })?,
            MAX_JOURNAL_ENTRY_BYTES,
        )?;
        validate_terminal_step(&terminal)?;
        return Ok(());
    }
    if state != "pending" || terminal_bytes.is_some() {
        return Err(journal_fault(
            "LSV4007",
            "merge group has an invalid durable state",
        ));
    }

    type RawBranch = (String, String, Option<Vec<u8>>);
    let branches = {
        let mut statement = transaction
            .prepare(
                "SELECT b.branch_name, e.state, e.terminal_step
                 FROM vm_merge_branches b
                 JOIN vm_effects e ON e.token = b.branch_token
                 WHERE b.group_token = ?1
                 ORDER BY b.position ASC",
            )
            .map_err(|error| {
                journal_error("LSV4033", "failed to prepare merge completion", error)
            })?;
        statement
            .query_map([&group_token], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })
            .map_err(|error| journal_error("LSV4033", "failed to scan merge completion", error))?
            .collect::<Result<Vec<RawBranch>, _>>()
            .map_err(|error| journal_error("LSV4033", "failed to read merge completion", error))?
    };
    if branches.iter().any(|(_, state, _)| state == "pending") {
        return Ok(());
    }
    if branches
        .iter()
        .any(|(_, state, terminal)| state != "completed" || terminal.is_none())
    {
        return Err(journal_fault(
            "LSV4007",
            "merge branch has an invalid durable state",
        ));
    }

    let plan: MergePlan = decode_bounded(&plan_bytes, MAX_CONTINUATION_BYTES)?;
    let completions = branches
        .into_iter()
        .map(|(branch, _, terminal)| {
            let step: Step = decode_bounded(
                &terminal.expect("completed branch checked above"),
                MAX_JOURNAL_ENTRY_BYTES,
            )?;
            Ok(BranchCompletion {
                branch,
                outcome: terminal_step_outcome(step)?,
            })
        })
        .collect::<Result<Vec<_>, Fault>>()?;
    let merged = merge_declared(&plan, completions, DEFAULT_MAX_OUTPUT_ITEMS)?;
    validate_terminal_step(&merged)?;
    let merged_bytes = encode_json_capped(&merged, MAX_JOURNAL_ENTRY_BYTES, "merge result")?;
    ensure_growth(transaction, merged_bytes.len())?;
    let updated = transaction
        .execute(
            "UPDATE vm_merge_groups
             SET state = 'completed', terminal_step = ?2
             WHERE token = ?1 AND state = 'pending'",
            params![group_token, merged_bytes],
        )
        .map_err(|error| journal_error("LSV4033", "failed to complete merge group", error))?;
    if updated != 1 {
        return Err(journal_fault(
            "LSV4033",
            "merge group completion lost its pending state",
        ));
    }
    Ok(())
}

fn terminal_step_outcome(step: Step) -> Result<BranchOutcome, Fault> {
    validate_terminal_step(&step)?;
    match step {
        Step::Done(value) => Ok(BranchOutcome::Value(value)),
        Step::Cancelled(cancellation) => Ok(BranchOutcome::Cancelled(cancellation)),
        Step::Failed(failure) => Ok(BranchOutcome::Failed(failure)),
        Step::Fault(fault) => Ok(BranchOutcome::Fault(fault)),
        Step::Effect(_) | Step::Effects(_) | Step::Waiting(_) | Step::Yield(_) => Err(
            journal_fault("LSV4007", "merge branch completion is not terminal"),
        ),
    }
}

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

fn load_dispatch(
    transaction: &rusqlite::Transaction<'_>,
    token: &str,
) -> Result<Option<DispatchRecord>, Fault> {
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

fn ensure_dispatch_matches(
    transaction: &rusqlite::Transaction<'_>,
    token: &str,
    request: &EffectRequest,
) -> Result<(), Fault> {
    let Some(existing) = load_dispatch(transaction, token)? else {
        return Err(journal_fault(
            "LSV4011",
            "pending effect is missing its durable dispatch",
        ));
    };
    let durable_request: EffectRequest =
        decode_bounded(&existing.request, MAX_JOURNAL_ENTRY_BYTES)?;
    if durable_request != *request {
        return Err(journal_fault(
            "LSV4011",
            "effect request conflicts with durable dispatch state",
        ));
    }
    Ok(())
}

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
        let token: String = row
            .get(0)
            .map_err(|error| journal_error("LSV4005", "invalid dispatch token", error))?;
        let bytes: Vec<u8> = row
            .get(1)
            .map_err(|error| journal_error("LSV4005", "invalid dispatch request", error))?;
        let request: EffectRequest = decode_bounded(&bytes, MAX_JOURNAL_ENTRY_BYTES)?;
        validate_effect_request(&request)?;
        if request.continuation.token.as_str() != token {
            return Err(journal_fault(
                "LSV4007",
                "dispatch token does not match effect request",
            ));
        }
        let effect_image: Vec<u8> = row.get(9).map_err(|error| {
            journal_error("LSV4005", "invalid dispatch continuation image", error)
        })?;
        let request_image = encode_json_capped(
            &request.continuation,
            MAX_CONTINUATION_BYTES,
            "continuation",
        )?;
        if request_image != effect_image {
            return Err(journal_fault(
                "LSV4007",
                "dispatch request conflicts with continuation state",
            ));
        }
        let state: String = row
            .get(2)
            .map_err(|error| journal_error("LSV4005", "invalid dispatch state", error))?;
        let attempt: i64 = row
            .get(3)
            .map_err(|error| journal_error("LSV4005", "invalid dispatch attempt", error))?;
        let effect_state: String = row
            .get(8)
            .map_err(|error| journal_error("LSV4005", "invalid dispatch effect state", error))?;
        let ready_at: i64 = row
            .get(5)
            .map_err(|error| journal_error("LSV4005", "invalid dispatch ready clock", error))?;
        let retry_count: i64 = row
            .get(6)
            .map_err(|error| journal_error("LSV4005", "invalid dispatch retry count", error))?;
        let last_error: Option<Vec<u8>> = row
            .get(7)
            .map_err(|error| journal_error("LSV4005", "invalid dispatch retry error", error))?;
        if attempt < 0 || attempt > i64::from(MAX_DISPATCH_ATTEMPTS) {
            return Err(journal_fault("LSV4007", "dispatch attempt is invalid"));
        }
        if ready_at < 0 || retry_count < 0 || retry_count > i64::from(MAX_SEMANTIC_RETRIES) {
            return Err(journal_fault("LSV4007", "dispatch retry state is invalid"));
        }
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
        if (state == "acknowledged") != (effect_state == "completed") {
            return Err(journal_fault(
                "LSV4007",
                "dispatch and continuation states are inconsistent",
            ));
        }
    }
    Ok(())
}

fn validate_debugger_audit_records(connection: &Connection) -> Result<(), Fault> {
    let index_count: u32 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master
             WHERE type = 'index' AND name = 'vm_debugger_audit_token_idx'",
            [],
            |row| row.get(0),
        )
        .map_err(|error| {
            journal_error("LSV4005", "failed to validate debugger audit schema", error)
        })?;
    if index_count != 1 {
        return Err(journal_fault(
            "LSV4007",
            "debugger audit schema is incomplete",
        ));
    }

    let audit_count: u32 = connection
        .query_row("SELECT COUNT(*) FROM vm_debugger_audit", [], |row| {
            row.get(0)
        })
        .map_err(|error| journal_error("LSV4005", "failed to count debugger audits", error))?;

    let mut statement = connection
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
        let command_id =
            crate::CommandId::new(row.get::<_, String>(0).map_err(|error| {
                journal_error("LSV4005", "invalid debugger audit command", error)
            })?)
            .map_err(|_| journal_fault("LSV4007", "debugger audit command is invalid"))?;
        let principal_id = row
            .get::<_, String>(1)
            .map_err(|error| journal_error("LSV4005", "invalid debugger audit principal", error))?;
        let idempotency_key =
            crate::IdempotencyKey::new(row.get::<_, String>(2).map_err(|error| {
                journal_error("LSV4005", "invalid debugger audit idempotency key", error)
            })?)
            .map_err(|_| journal_fault("LSV4007", "debugger audit idempotency key is invalid"))?;
        let origin =
            parse_command_origin(&row.get::<_, String>(3).map_err(|error| {
                journal_error("LSV4005", "invalid debugger audit origin", error)
            })?)
            .map_err(|error| journal_error("LSV4007", "debugger audit origin is invalid", error))?;
        let session_id = row
            .get::<_, String>(4)
            .map_err(|error| journal_error("LSV4005", "invalid debugger audit session", error))?;
        let expected_revision =
            u64::try_from(row.get::<_, i64>(5).map_err(|error| {
                journal_error("LSV4005", "invalid debugger audit revision", error)
            })?)
            .map_err(|_| journal_fault("LSV4007", "debugger audit revision is invalid"))?;
        let observed_at_ms =
            u64::try_from(row.get::<_, i64>(6).map_err(|error| {
                journal_error("LSV4005", "invalid debugger audit clock", error)
            })?)
            .map_err(|_| journal_fault("LSV4007", "debugger audit clock is invalid"))?;
        let token = ContinuationToken(row.get::<_, String>(7).map_err(|error| {
            journal_error("LSV4005", "invalid debugger audit continuation", error)
        })?);
        let context = DebuggerAuditContext {
            command_id,
            idempotency_key,
            principal: crate::Principal { id: principal_id },
            origin,
            session_id,
            expected_revision: crate::Revision(expected_revision),
        };
        crate::validate_debugger_audit(&context)
            .map_err(|_| journal_fault("LSV4007", "debugger audit identity is invalid"))?;
        if !valid_continuation_token(&token)
            || row.get::<_, String>(8).map_err(|error| {
                journal_error("LSV4005", "invalid debugger audit effect state", error)
            })? != "completed"
        {
            return Err(journal_fault(
                "LSV4007",
                "debugger audit continuation is invalid",
            ));
        }
        let terminal_bytes = row.get::<_, Vec<u8>>(9).map_err(|error| {
            journal_error("LSV4005", "invalid debugger audit terminal step", error)
        })?;
        let terminal: Step = decode_bounded(&terminal_bytes, MAX_JOURNAL_ENTRY_BYTES)?;
        if !matches!(
            terminal,
            Step::Cancelled(Cancellation {
                continuation,
                reason: CancellationReason::Requested,
                observed_at_ms: terminal_observed_at,
            }) if continuation == token && terminal_observed_at == observed_at_ms
        ) {
            return Err(journal_fault(
                "LSV4007",
                "debugger audit does not match its terminal cancellation",
            ));
        }
        validated_count += 1;
    }
    if validated_count != audit_count {
        return Err(journal_fault(
            "LSV4007",
            "debugger audit references a missing continuation",
        ));
    }
    Ok(())
}

#[cfg_attr(not(test), allow(dead_code))]
fn validate_merge_graph_input(
    group_token: &ContinuationToken,
    plan: &MergePlan,
    branches: &[(&str, &EffectRequest)],
) -> Result<(), Fault> {
    validate_merge_plan(plan)?;
    if !valid_continuation_token(group_token) {
        return Err(journal_fault("LSV4032", "merge group token is invalid"));
    }
    if branches.len() != plan.branches.len() {
        return Err(journal_fault(
            "LSV4032",
            "merge branch requests do not match the declared plan",
        ));
    }
    let mut tokens = BTreeSet::new();
    for (position, (name, request)) in branches.iter().enumerate() {
        validate_effect_request(request)?;
        if *name != plan.branches[position]
            || request.continuation.token == *group_token
            || !tokens.insert(request.continuation.token.clone())
        {
            return Err(journal_fault(
                "LSV4032",
                "merge branch requests do not match the declared plan",
            ));
        }
    }
    Ok(())
}

fn validate_merge_graph_records(connection: &Connection) -> Result<(), Fault> {
    type RawMergeGroup = (String, Vec<u8>, String, Option<Vec<u8>>);
    let groups = {
        let mut statement = connection
            .prepare(
                "SELECT token, plan, state, terminal_step
                 FROM vm_merge_groups ORDER BY token ASC",
            )
            .map_err(|error| {
                journal_error("LSV4005", "failed to prepare merge graph load", error)
            })?;
        statement
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })
            .map_err(|error| journal_error("LSV4005", "failed to load merge graphs", error))?
            .collect::<Result<Vec<RawMergeGroup>, _>>()
            .map_err(|error| journal_error("LSV4005", "failed to read merge graph", error))?
    };

    for (token, plan_bytes, state, terminal_bytes) in groups {
        let token = ContinuationToken(token);
        if !valid_continuation_token(&token) {
            return Err(journal_fault("LSV4007", "merge group token is invalid"));
        }
        let token_collision: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM vm_effects WHERE token = ?1)",
                [token.as_str()],
                |row| row.get(0),
            )
            .map_err(|error| {
                journal_error("LSV4005", "failed to validate merge group token", error)
            })?;
        if token_collision {
            return Err(journal_fault(
                "LSV4007",
                "merge group token conflicts with an effect token",
            ));
        }
        let plan: MergePlan = decode_bounded(&plan_bytes, MAX_CONTINUATION_BYTES)?;
        validate_merge_plan(&plan)
            .map_err(|_| journal_fault("LSV4007", "merge group plan is invalid"))?;
        let branches = {
            let mut statement = connection
                .prepare(
                    "SELECT b.branch_name, b.position, e.state
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
                    ))
                })
                .map_err(|error| journal_error("LSV4005", "failed to load merge branches", error))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| journal_error("LSV4005", "failed to read merge branches", error))?
        };
        if branches.len() != plan.branches.len()
            || branches.iter().enumerate().any(|(position, branch)| {
                branch.0 != plan.branches[position]
                    || usize::try_from(branch.1).ok() != Some(position)
            })
        {
            return Err(journal_fault(
                "LSV4007",
                "merge branches do not match their declared plan",
            ));
        }
        let all_completed = branches.iter().all(|branch| branch.2 == "completed");
        match (state.as_str(), terminal_bytes) {
            ("pending", None) if !all_completed => {}
            ("completed", Some(bytes)) if all_completed => {
                let terminal: Step = decode_bounded(&bytes, MAX_JOURNAL_ENTRY_BYTES)?;
                validate_terminal_step(&terminal)?;
            }
            _ => {
                return Err(journal_fault(
                    "LSV4007",
                    "merge group state conflicts with its branch graph",
                ));
            }
        }
    }
    Ok(())
}

fn validate_lease_clock(now_ms: u64, lease_ms: u64) -> Result<(), Fault> {
    if now_ms > i64::MAX as u64 {
        return Err(journal_fault("LSV4015", "dispatch clock is out of range"));
    }
    if lease_ms == 0 || lease_ms > MAX_DISPATCH_LEASE_MS {
        return Err(journal_fault(
            "LSV4015",
            format!("dispatch lease must be between 1 and {MAX_DISPATCH_LEASE_MS} ms"),
        ));
    }
    lease_expiration(now_ms, lease_ms).map(|_| ())
}

fn lease_expiration(now_ms: u64, lease_ms: u64) -> Result<u64, Fault> {
    now_ms
        .checked_add(lease_ms)
        .filter(|value| *value <= i64::MAX as u64)
        .ok_or_else(|| journal_fault("LSV4015", "dispatch lease expiration is out of range"))
}

fn next_attempt(current: u32) -> Result<u32, Fault> {
    let next = current
        .checked_add(1)
        .ok_or_else(|| journal_fault("LSV4017", "dispatch attempt counter is exhausted"))?;
    if next > MAX_DISPATCH_ATTEMPTS {
        return Err(journal_fault(
            "LSV4017",
            format!("dispatch attempt limit {MAX_DISPATCH_ATTEMPTS} reached"),
        ));
    }
    Ok(next)
}

fn decode_bounded<T: serde::de::DeserializeOwned>(bytes: &[u8], limit: usize) -> Result<T, Fault> {
    if bytes.len() > limit {
        return Err(journal_fault(
            "LSV4006",
            format!("journal entry exceeds {limit} bytes"),
        ));
    }
    serde_json::from_slice(bytes)
        .map_err(|error| journal_error("LSV4007", "invalid journal JSON", error))
}

fn validate_terminal_step(step: &Step) -> Result<(), Fault> {
    match step {
        Step::Done(value) if validate_value(value, 1).is_ok() => Ok(()),
        Step::Cancelled(cancellation)
            if !cancellation.continuation.as_str().is_empty()
                && cancellation.observed_at_ms <= i64::MAX as u64 =>
        {
            Ok(())
        }
        Step::Failed(failure)
            if failure.retry_count <= MAX_SEMANTIC_RETRIES
                && validate_effect_error(&failure.error).is_ok() =>
        {
            Ok(())
        }
        Step::Fault(fault) if !fault.code.is_empty() && fault.code.len() <= 32 => Ok(()),
        Step::Done(_) => Err(journal_fault(
            "LSV4007",
            "completed journal output exceeds runtime limit",
        )),
        Step::Effect(_) | Step::Effects(_) | Step::Waiting(_) | Step::Yield(_) => Err(
            journal_fault("LSV4007", "journal completion must be a terminal step"),
        ),
        Step::Fault(_) | Step::Cancelled(_) | Step::Failed(_) => Err(journal_fault(
            "LSV4007",
            "journal fault has an invalid diagnostic code",
        )),
    }
}

fn validate_cancellation_step(image: &ContinuationImage, step: &Step) -> Result<(), Fault> {
    validate_terminal_step(step)?;
    match step {
        Step::Cancelled(cancellation) if cancellation.continuation == image.token => Ok(()),
        _ => Err(journal_fault(
            "LSV4007",
            "cancellation does not match continuation image",
        )),
    }
}

fn deadline_cancellation(image: &ContinuationImage, observed_at_ms: u64) -> Step {
    Step::Cancelled(Cancellation {
        continuation: image.token.clone(),
        reason: CancellationReason::DeadlineExceeded,
        observed_at_ms,
    })
}

#[cfg(unix)]
fn tighten_permissions(path: &Path) -> Result<(), Fault> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .map_err(|error| journal_error("LSV4002", "failed to secure journal permissions", error))
}

#[cfg(unix)]
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
fn create_private_journal(path: &Path) -> Result<(), Fault> {
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map(|_| ())
        .map_err(|error| journal_error("LSV4002", "failed to create private journal", error))
}

#[cfg(not(unix))]
fn tighten_permissions(_path: &Path) -> Result<(), Fault> {
    Ok(())
}

fn journal_error(code: &str, context: &str, error: impl std::fmt::Display) -> Fault {
    journal_fault(code, format!("{context}: {error}"))
}

fn journal_fault(code: &str, message: impl Into<String>) -> Fault {
    Fault {
        code: code.to_string(),
        message: message.into(),
    }
}
