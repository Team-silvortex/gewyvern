use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Write};
use std::path::Path;

use leselang_command::{LoweringContext, PlannedOperation, lower_effect};
use leselang_hir::{Effect, HirProgram, Type, authorize};
use leserpent_domain::{
    CAPABILITY_RUNTIME_DEPLOY, CAPABILITY_RUNTIME_READ, CAPABILITY_RUNTIME_REFRESH, CapabilitySet,
    Command, CommandEnvelope, CommandId, CommandOrigin, CommandResult, CommandStatus, Confirmation,
    DOMAIN_SCHEMA_VERSION, IdempotencyKey, MAX_RUNTIME_LOG_QUERY_ENTRIES, Principal, Query,
    QueryEnvelope, QueryResult, Revision, RuntimeId, RuntimeLogRecord, RuntimeProjection,
};
use serde::{Deserialize, Deserializer, Serialize};

mod journal;

pub use journal::{
    JOURNAL_SCHEMA_VERSION, MAX_JOURNAL_ENTRY_BYTES, MAX_JOURNAL_RECORDS, MAX_JOURNAL_TOTAL_BYTES,
};
use journal::{Journal, MergeProgress};

pub const CONTINUATION_SCHEMA_VERSION: u32 = 1;
pub const MAX_CONTINUATION_BYTES: usize = 64 * 1024;
pub const DEFAULT_FUEL: u64 = 1_000;
pub const MAX_EXECUTION_FUEL: u64 = 1_000_000;
pub const DEFAULT_EFFECT_DEADLINE_MS: u64 = 30_000;
pub const MAX_EFFECT_DEADLINE_MS: u64 = 24 * 60 * 60 * 1_000;
pub const DEFAULT_MAX_OUTPUT_ITEMS: usize = 10_000;
pub const DEFAULT_DISPATCH_LEASE_MS: u64 = 30_000;
pub const MAX_DISPATCH_LEASE_MS: u64 = 5 * 60 * 1_000;
pub const MAX_DISPATCH_ATTEMPTS: u32 = 100;
pub const DEFAULT_MAX_SEMANTIC_RETRIES: u32 = 3;
pub const MAX_SEMANTIC_RETRIES: u32 = 16;
pub const DEFAULT_RETRY_BASE_DELAY_MS: u64 = 250;
pub const DEFAULT_RETRY_MAX_DELAY_MS: u64 = 30_000;
pub const MAX_RETRY_DELAY_MS: u64 = 60 * 60 * 1_000;
pub const DEFAULT_COMPLETED_RETENTION: usize = 5_000;
pub const DEFAULT_COMPACTION_BATCH: usize = 500;
pub const MAX_COMPACTION_BATCH: usize = 1_000;
pub const MAX_MERGE_BRANCHES: usize = 64;
pub const MAX_MERGE_BRANCH_NAME_BYTES: usize = 64;
pub const MAX_STRUCTURED_VALUE_DEPTH: usize = 16;

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct ContinuationToken(String);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ContinuationImage {
    pub schema_version: u32,
    pub token: ContinuationToken,
    pub program_counter: u32,
    pub expected_revision: Option<Revision>,
    pub result_type: Type,
    pub pending_effect: Effect,
    pub fuel_remaining: u64,
    pub deadline_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deadline_at_ms: Option<u64>,
    pub max_output_items: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResourceBudget {
    pub fuel_remaining: u64,
    pub deadline_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deadline_at_ms: Option<u64>,
    pub max_output_items: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct EffectRequest {
    pub effect_id: String,
    pub required_capability: String,
    pub operation: EffectOperation,
    pub continuation: ContinuationImage,
    pub budget: ResourceBudget,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
pub enum EffectOperation {
    Query(QueryEnvelope),
    Command(CommandEnvelope),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
pub enum EffectResult {
    Query(QueryResult),
    Command(Box<CommandResult>),
}

#[derive(Deserialize)]
#[serde(untagged)]
enum EffectRequestWire {
    Current {
        effect_id: String,
        required_capability: String,
        operation: EffectOperation,
        continuation: ContinuationImage,
        budget: ResourceBudget,
    },
    LegacyQuery {
        effect_id: String,
        required_capability: String,
        query: QueryEnvelope,
        continuation: ContinuationImage,
        budget: ResourceBudget,
    },
}

impl<'de> Deserialize<'de> for EffectRequest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = EffectRequestWire::deserialize(deserializer)?;
        Ok(match wire {
            EffectRequestWire::Current {
                effect_id,
                required_capability,
                operation,
                continuation,
                budget,
            } => Self {
                effect_id,
                required_capability,
                operation,
                continuation,
                budget,
            },
            EffectRequestWire::LegacyQuery {
                effect_id,
                required_capability,
                query,
                continuation,
                budget,
            } => Self {
                effect_id,
                required_capability,
                operation: EffectOperation::Query(query),
                continuation,
                budget,
            },
        })
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DispatchLease {
    pub request: EffectRequest,
    pub attempt: u32,
    #[serde(default)]
    pub retry_count: u32,
    pub lease_expires_at_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RetryPolicy {
    pub max_retries: u32,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: DEFAULT_MAX_SEMANTIC_RETRIES,
            base_delay_ms: DEFAULT_RETRY_BASE_DELAY_MS,
            max_delay_ms: DEFAULT_RETRY_MAX_DELAY_MS,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectErrorClass {
    Transient,
    Permanent,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EffectError {
    pub class: EffectErrorClass,
    pub code: String,
    pub message: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FailedEffect {
    pub error: EffectError,
    pub retry_count: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RetrySchedule {
    pub retry_count: u32,
    pub ready_at_ms: u64,
    pub error: EffectError,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
pub enum RetryDisposition {
    Scheduled(RetrySchedule),
    Terminal(Step),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RetentionPolicy {
    pub max_completed_records: usize,
    pub max_delete_per_run: usize,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            max_completed_records: DEFAULT_COMPLETED_RETENTION,
            max_delete_per_run: DEFAULT_COMPACTION_BATCH,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CompactionReport {
    pub removed_records: usize,
    pub remaining_completed: usize,
    pub reclaimed_logical_bytes: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MergePlan {
    pub branches: Vec<String>,
}

impl MergePlan {
    pub fn new(branches: impl IntoIterator<Item = impl Into<String>>) -> Result<Self, Fault> {
        let plan = Self {
            branches: branches.into_iter().map(Into::into).collect(),
        };
        validate_merge_plan(&plan)?;
        Ok(plan)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BranchCompletion {
    pub branch: String,
    pub outcome: BranchOutcome,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
pub enum BranchOutcome {
    Value(Value),
    Cancelled(Cancellation),
    Failed(FailedEffect),
    Fault(Fault),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StructuredField {
    pub name: String,
    pub value: Box<Value>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct NamedEffectRequest {
    pub branch: String,
    pub request: EffectRequest,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StructuredEffectBatch {
    pub merge_token: ContinuationToken,
    pub branches: Vec<NamedEffectRequest>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MergeWait {
    pub merge_token: ContinuationToken,
    pub completed_branches: usize,
    pub total_branches: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Value {
    RuntimeList {
        revision: Revision,
        runtimes: Vec<RuntimeProjection>,
    },
    RuntimeInspect {
        revision: Revision,
        runtime: Box<RuntimeProjection>,
    },
    RuntimeHistory {
        revision: Revision,
        entries: Vec<CommandResult>,
    },
    RuntimeLogs {
        revision: Revision,
        runtime_id: RuntimeId,
        runtime_name: String,
        entries: Vec<RuntimeLogRecord>,
    },
    RuntimeRefresh {
        result: Box<CommandResult>,
    },
    RuntimeCapabilitiesRefresh {
        result: Box<CommandResult>,
    },
    RuntimeDeploy {
        result: Box<CommandResult>,
    },
    Structured {
        fields: Vec<StructuredField>,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Fault {
    pub code: String,
    pub message: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CancellationReason {
    Requested,
    DeadlineExceeded,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Cancellation {
    pub continuation: ContinuationToken,
    pub reason: CancellationReason,
    pub observed_at_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DebuggerAuditContext {
    pub command_id: CommandId,
    pub idempotency_key: IdempotencyKey,
    pub principal: Principal,
    pub origin: CommandOrigin,
    pub session_id: String,
    pub expected_revision: Revision,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DebuggerAuditRecord {
    pub command_id: CommandId,
    pub principal_id: String,
    pub origin: CommandOrigin,
    pub session_id: String,
    pub expected_revision: Revision,
    pub observed_at_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
pub enum Step {
    Done(Value),
    Effect(Box<EffectRequest>),
    Effects(Box<StructuredEffectBatch>),
    Waiting(MergeWait),
    Yield(ContinuationImage),
    Cancelled(Cancellation),
    Failed(FailedEffect),
    Fault(Fault),
}

pub struct Vm {
    next_effect_id: u64,
    fuel_limit: u64,
    pending: BTreeMap<ContinuationToken, ContinuationImage>,
    completed: BTreeMap<ContinuationToken, Step>,
    journal: Journal,
}

impl Default for Vm {
    fn default() -> Self {
        Self::new(DEFAULT_FUEL)
    }
}

impl Vm {
    pub fn new(fuel_limit: u64) -> Self {
        Self {
            next_effect_id: 1,
            fuel_limit: fuel_limit.min(MAX_EXECUTION_FUEL),
            pending: BTreeMap::new(),
            completed: BTreeMap::new(),
            journal: Journal::ephemeral(),
        }
    }

    pub fn open_journal(path: impl AsRef<Path>, fuel_limit: u64) -> Result<Self, Fault> {
        let (journal, snapshot) = Journal::open(path.as_ref())?;
        Ok(Self {
            next_effect_id: snapshot.next_sequence,
            fuel_limit: fuel_limit.min(MAX_EXECUTION_FUEL),
            pending: snapshot.pending,
            completed: snapshot.completed,
            journal,
        })
    }

    pub fn start(
        &mut self,
        program: &HirProgram,
        principal: Principal,
        capabilities: CapabilitySet,
        expected_revision: Option<Revision>,
    ) -> Step {
        self.start_inner(
            program,
            principal,
            capabilities,
            expected_revision,
            DEFAULT_EFFECT_DEADLINE_MS,
            None,
        )
    }

    pub fn start_timed(
        &mut self,
        program: &HirProgram,
        principal: Principal,
        capabilities: CapabilitySet,
        expected_revision: Option<Revision>,
        now_ms: u64,
        timeout_ms: u64,
    ) -> Step {
        let deadline_at_ms = match validate_effect_clock(now_ms, timeout_ms) {
            Ok(deadline_at_ms) => deadline_at_ms,
            Err(error) => return Step::Fault(error),
        };
        self.start_inner(
            program,
            principal,
            capabilities,
            expected_revision,
            timeout_ms,
            Some(deadline_at_ms),
        )
    }

    fn start_inner(
        &mut self,
        program: &HirProgram,
        principal: Principal,
        capabilities: CapabilitySet,
        expected_revision: Option<Revision>,
        deadline_ms: u64,
        deadline_at_ms: Option<u64>,
    ) -> Step {
        if self.fuel_limit == 0 {
            return fault("LSV1001", "execution fuel exhausted");
        }
        if let Err(error) = authorize(program, &capabilities) {
            return fault(&error.code, error.message);
        }
        if let Effect::All { branches } = &program.function.effect {
            return self.start_all(
                branches,
                principal,
                capabilities,
                expected_revision,
                deadline_ms,
                deadline_at_ms,
            );
        }

        let sequence = match self.allocate_sequence() {
            Ok(sequence) => sequence,
            Err(error) => return Step::Fault(error),
        };
        let request = match self.build_effect_request(
            &program.function.effect,
            program.function.result_type,
            sequence,
            principal,
            capabilities,
            expected_revision,
            deadline_ms,
            deadline_at_ms,
        ) {
            Ok(request) => request,
            Err(error) => return Step::Fault(error),
        };
        if let Err(error) = self
            .journal
            .record_pending(&request.continuation, Some(&request))
        {
            return Step::Fault(error);
        }
        self.pending.insert(
            request.continuation.token.clone(),
            request.continuation.clone(),
        );
        Step::Effect(Box::new(request))
    }

    fn start_all(
        &mut self,
        branches: &[leselang_hir::HirBranch],
        principal: Principal,
        capabilities: CapabilitySet,
        expected_revision: Option<Revision>,
        deadline_ms: u64,
        deadline_at_ms: Option<u64>,
    ) -> Step {
        if branches
            .iter()
            .any(|branch| matches!(branch.effect, Effect::All { .. }))
        {
            return fault("LSV1002", "nested structured all is not yet supported");
        }
        let plan = match MergePlan::new(branches.iter().map(|branch| branch.name.clone())) {
            Ok(plan) => plan,
            Err(error) => return Step::Fault(error),
        };
        let group_sequence = match self.allocate_sequence() {
            Ok(sequence) => sequence,
            Err(error) => return Step::Fault(error),
        };
        let merge_token = ContinuationToken(format!("merge-{group_sequence}"));
        let mut named = Vec::with_capacity(branches.len());
        for branch in branches {
            let sequence = match self.allocate_sequence() {
                Ok(sequence) => sequence,
                Err(error) => return Step::Fault(error),
            };
            let request = match self.build_effect_request(
                &branch.effect,
                branch.result_type,
                sequence,
                principal.clone(),
                capabilities.clone(),
                expected_revision,
                deadline_ms,
                deadline_at_ms,
            ) {
                Ok(request) => request,
                Err(error) => return Step::Fault(error),
            };
            named.push(NamedEffectRequest {
                branch: branch.name.clone(),
                request,
            });
        }
        let graph = named
            .iter()
            .map(|branch| (branch.branch.as_str(), &branch.request))
            .collect::<Vec<_>>();
        if let Err(error) = self.journal.record_merge_graph(&merge_token, &plan, &graph) {
            return Step::Fault(error);
        }
        for branch in &named {
            self.pending.insert(
                branch.request.continuation.token.clone(),
                branch.request.continuation.clone(),
            );
        }
        Step::Effects(Box::new(StructuredEffectBatch {
            merge_token,
            branches: named,
        }))
    }

    #[allow(clippy::too_many_arguments)]
    fn build_effect_request(
        &self,
        effect: &Effect,
        result_type: Type,
        sequence: u64,
        principal: Principal,
        capabilities: CapabilitySet,
        expected_revision: Option<Revision>,
        deadline_ms: u64,
        deadline_at_ms: Option<u64>,
    ) -> Result<EffectRequest, Fault> {
        let image = ContinuationImage {
            schema_version: CONTINUATION_SCHEMA_VERSION,
            token: ContinuationToken(format!("continuation-{sequence}")),
            program_counter: 1,
            expected_revision,
            result_type,
            pending_effect: effect.clone(),
            fuel_remaining: self.fuel_limit - 1,
            deadline_ms,
            deadline_at_ms,
            max_output_items: DEFAULT_MAX_OUTPUT_ITEMS,
        };
        encode_continuation(&image)?;
        let plan = lower_effect(
            effect,
            &LoweringContext {
                principal,
                capabilities,
                expected_revision,
                command_id: CommandId::new(format!("leselang-command-{sequence}"))
                    .expect("generated command identifier is valid"),
                idempotency_key: IdempotencyKey::new(format!("leselang-effect-{sequence}"))
                    .expect("generated idempotency key is valid"),
                origin: CommandOrigin::Leselang,
                confirmation: if matches!(effect, Effect::RuntimeDeploy { .. }) {
                    Confirmation::Confirmed
                } else {
                    Confirmation::NotRequired
                },
                dry_run: false,
            },
        )
        .map_err(|error| Fault {
            code: "LSV1002".to_string(),
            message: format!("structured branch lowering failed: {error:?}"),
        })?;
        let operation = match plan.operation {
            PlannedOperation::Query(query) => EffectOperation::Query(query),
            PlannedOperation::Command(command) => EffectOperation::Command(command),
        };
        Ok(EffectRequest {
            effect_id: format!("effect-{sequence}"),
            required_capability: plan.required_capability,
            operation,
            continuation: image.clone(),
            budget: ResourceBudget {
                fuel_remaining: image.fuel_remaining,
                deadline_ms: image.deadline_ms,
                deadline_at_ms: image.deadline_at_ms,
                max_output_items: image.max_output_items,
            },
        })
    }

    pub fn restore(&mut self, image: ContinuationImage) -> Result<(), Fault> {
        validate_image(&image)?;
        if let Some(current) = self.pending.get(&image.token) {
            return if current == &image {
                Ok(())
            } else {
                Err(Fault {
                    code: "LSV2002".to_string(),
                    message: "continuation token conflicts with pending state".to_string(),
                })
            };
        }
        if self.completed.contains_key(&image.token) {
            return Err(Fault {
                code: "LSV2003".to_string(),
                message: "continuation was already consumed".to_string(),
            });
        }
        if let Some(sequence) = image
            .token
            .as_str()
            .strip_prefix("continuation-")
            .and_then(|value| value.parse::<u64>().ok())
        {
            self.next_effect_id = self.next_effect_id.max(sequence.saturating_add(1));
        }
        self.journal.record_pending(&image, None)?;
        self.pending.insert(image.token.clone(), image);
        Ok(())
    }

    pub fn resume(&mut self, image: &ContinuationImage, result: EffectResult) -> Step {
        if image.deadline_at_ms.is_some() {
            return fault(
                "LSV2111",
                "timed effects require resume_at with trusted scheduler time",
            );
        }
        self.resume_inner(image, result)
    }

    pub fn resume_at(
        &mut self,
        image: &ContinuationImage,
        now_ms: u64,
        result: EffectResult,
    ) -> Step {
        if let Err(error) = validate_scheduler_time(now_ms) {
            return Step::Fault(error);
        }
        if let Err(error) = self.expire_due(now_ms) {
            return Step::Fault(error);
        }
        self.resume_inner(image, result)
    }

    fn resume_inner(&mut self, image: &ContinuationImage, result: EffectResult) -> Step {
        match self.replay_completed(&image.token) {
            Ok(Some(completed)) => return self.visible_step(&image.token, completed),
            Ok(None) => {}
            Err(error) => return Step::Fault(error),
        }
        if let Err(error) = validate_image(image) {
            return Step::Fault(error);
        }
        let Some(stored) = self.pending.get(&image.token) else {
            return fault("LSV2004", "unknown continuation token");
        };
        if stored != image {
            return fault("LSV2005", "continuation image does not match pending state");
        }
        if matches!(
            image.pending_effect,
            Effect::RuntimeRefresh { .. }
                | Effect::RuntimeCapabilitiesRefresh { .. }
                | Effect::RuntimeDeploy { .. }
        ) {
            return fault(
                "LSV2110",
                "mutating effects must be completed through dispatch acknowledgement",
            );
        }

        let step = step_from_effect_result(image, None, result);
        let authoritative = match self.journal.record_completed(image, &step) {
            Ok(step) => step,
            Err(error) => return Step::Fault(error),
        };
        self.pending.remove(&image.token);
        self.completed
            .insert(image.token.clone(), authoritative.clone());
        self.visible_step(&image.token, authoritative)
    }

    pub fn claim_effect(
        &mut self,
        now_ms: u64,
        lease_ms: u64,
    ) -> Result<Option<DispatchLease>, Fault> {
        self.expire_due(now_ms)?;
        self.journal.claim_dispatch(now_ms, lease_ms)
    }

    pub fn acknowledge_effect(
        &mut self,
        lease: &DispatchLease,
        now_ms: u64,
        result: EffectResult,
    ) -> Step {
        if let Err(error) = self.expire_due(now_ms) {
            return Step::Fault(error);
        }
        if let Err(error) = validate_effect_request(&lease.request) {
            return Step::Fault(error);
        }
        let image = &lease.request.continuation;
        match self.replay_completed(&image.token) {
            Ok(Some(completed)) => return self.visible_step(&image.token, completed),
            Ok(None) => {}
            Err(error) => return Step::Fault(error),
        }
        let Some(stored) = self.pending.get(&image.token) else {
            return fault("LSV2004", "unknown continuation token");
        };
        if stored != image {
            return fault("LSV2005", "continuation image does not match pending state");
        }
        let step = step_from_effect_result(image, Some(&lease.request.operation), result);
        let authoritative = match self.journal.acknowledge_dispatch(lease, now_ms, &step) {
            Ok(step) => step,
            Err(error) => return Step::Fault(error),
        };
        self.pending.remove(&image.token);
        self.completed
            .insert(image.token.clone(), authoritative.clone());
        self.visible_step(&image.token, authoritative)
    }

    pub fn report_effect_error(
        &mut self,
        lease: &DispatchLease,
        now_ms: u64,
        error: EffectError,
        policy: &RetryPolicy,
    ) -> Result<RetryDisposition, Fault> {
        validate_scheduler_time(now_ms)?;
        self.expire_due(now_ms)?;
        if let Some(completed) = self.replay_completed(&lease.request.continuation.token)? {
            return Ok(RetryDisposition::Terminal(
                self.visible_step(&lease.request.continuation.token, completed),
            ));
        }
        validate_effect_error(&error)?;
        if error.class == EffectErrorClass::Transient {
            validate_retry_policy(policy)?;
        }

        let disposition = match error.class {
            EffectErrorClass::Permanent => RetryDisposition::Terminal(Step::Failed(FailedEffect {
                error,
                retry_count: lease.retry_count,
            })),
            EffectErrorClass::Transient if lease.retry_count >= policy.max_retries => {
                RetryDisposition::Terminal(Step::Failed(FailedEffect {
                    error,
                    retry_count: lease.retry_count,
                }))
            }
            EffectErrorClass::Transient => {
                let retry_count = lease.retry_count + 1;
                let delay_ms = retry_delay(policy, retry_count)?;
                let ready_at_ms = now_ms
                    .checked_add(delay_ms)
                    .filter(|ready_at| *ready_at <= i64::MAX as u64)
                    .ok_or_else(|| Fault {
                        code: "LSV2203".to_string(),
                        message: "semantic retry clock overflow".to_string(),
                    })?;
                RetryDisposition::Scheduled(RetrySchedule {
                    retry_count,
                    ready_at_ms,
                    error,
                })
            }
        };
        let authoritative = self.journal.report_error(lease, now_ms, &disposition)?;
        if let RetryDisposition::Terminal(step) = &authoritative {
            let image = &lease.request.continuation;
            self.pending.remove(&image.token);
            self.completed.insert(image.token.clone(), step.clone());
            return Ok(RetryDisposition::Terminal(
                self.visible_step(&image.token, step.clone()),
            ));
        }
        Ok(authoritative)
    }

    pub fn cancel_effect(&mut self, image: &ContinuationImage, now_ms: u64) -> Step {
        if let Err(error) = validate_scheduler_time(now_ms) {
            return Step::Fault(error);
        }
        if let Err(error) = self.expire_due(now_ms) {
            return Step::Fault(error);
        }
        match self.replay_completed(&image.token) {
            Ok(Some(completed)) => return self.visible_step(&image.token, completed),
            Ok(None) => {}
            Err(error) => return Step::Fault(error),
        }
        let Some(stored) = self.pending.get(&image.token) else {
            return fault("LSV2004", "unknown continuation token");
        };
        if stored != image {
            return fault("LSV2005", "continuation image does not match pending state");
        }
        self.finish_cancellation(image, CancellationReason::Requested, now_ms)
    }

    pub fn cancel_effect_audited(
        &mut self,
        image: &ContinuationImage,
        now_ms: u64,
        audit: &DebuggerAuditContext,
    ) -> Result<DebuggerAuditRecord, Fault> {
        validate_debugger_audit(audit)?;
        validate_scheduler_time(now_ms)?;
        self.expire_due(now_ms)?;
        if let Some(completed) = self.completed.get(&image.token) {
            if !matches!(
                completed,
                Step::Cancelled(Cancellation {
                    reason: CancellationReason::Requested,
                    ..
                })
            ) {
                return Err(Fault {
                    code: "LSV2014".to_string(),
                    message: "completed effect cannot accept debugger cancellation audit"
                        .to_string(),
                });
            }
        } else {
            let Some(stored) = self.pending.get(&image.token) else {
                return Err(Fault {
                    code: "LSV2004".to_string(),
                    message: "unknown continuation token".to_string(),
                });
            };
            if stored != image {
                return Err(Fault {
                    code: "LSV2005".to_string(),
                    message: "continuation image does not match pending state".to_string(),
                });
            }
        }
        let step = Step::Cancelled(Cancellation {
            continuation: image.token.clone(),
            reason: CancellationReason::Requested,
            observed_at_ms: now_ms,
        });
        let (authoritative, record) = self.journal.cancel_audited(image, &step, audit, now_ms)?;
        if matches!(authoritative, Step::Cancelled(_)) {
            self.pending.remove(&image.token);
            self.completed.insert(image.token.clone(), authoritative);
        }
        Ok(record)
    }

    pub fn debugger_audit(
        &self,
        command_id: &CommandId,
    ) -> Result<Option<DebuggerAuditRecord>, Fault> {
        self.journal.debugger_audit(command_id.as_str())
    }

    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    pub fn pending_continuations(&self) -> Vec<ContinuationImage> {
        self.pending.values().cloned().collect()
    }

    pub fn completed_count(&self) -> usize {
        self.completed.len()
    }

    pub fn merge_result(&self, merge_token: &ContinuationToken) -> Result<Option<Step>, Fault> {
        self.journal.merge_result(merge_token)
    }

    pub fn compact_journal(&mut self, policy: &RetentionPolicy) -> Result<CompactionReport, Fault> {
        validate_retention_policy(policy)?;
        let mut local_completed = self
            .completed
            .iter()
            .map(|(token, step)| (token.clone(), step.clone()))
            .collect::<Vec<_>>();
        local_completed.sort_by(|(left, _), (right, _)| continuation_age_order(left, right));
        let compacted = self.journal.compact_completed(&local_completed, policy)?;
        for token in &compacted.pruned_tokens {
            self.completed.remove(token);
        }
        Ok(CompactionReport {
            removed_records: compacted.removed_records,
            remaining_completed: compacted.remaining_completed,
            reclaimed_logical_bytes: compacted.reclaimed_logical_bytes,
        })
    }

    fn replay_completed(&mut self, token: &ContinuationToken) -> Result<Option<Step>, Fault> {
        let Some(step) = self.completed.get(token).cloned() else {
            return Ok(None);
        };
        if self.journal.completed_exists(token)? {
            return Ok(Some(step));
        }
        self.completed.remove(token);
        Ok(None)
    }

    fn visible_step(&self, branch_token: &ContinuationToken, branch_step: Step) -> Step {
        match self.journal.merge_progress(branch_token) {
            Ok(MergeProgress::Standalone) => branch_step,
            Ok(MergeProgress::Pending {
                merge_token,
                completed_branches,
                total_branches,
            }) => Step::Waiting(MergeWait {
                merge_token,
                completed_branches,
                total_branches,
            }),
            Ok(MergeProgress::Completed { merge_token, step }) => {
                debug_assert!(
                    self.journal
                        .merge_result(&merge_token)
                        .ok()
                        .flatten()
                        .is_some()
                );
                step
            }
            Err(error) => Step::Fault(error),
        }
    }

    fn allocate_sequence(&mut self) -> Result<u64, Fault> {
        loop {
            let sequence = self.journal.allocate_sequence(self.next_effect_id)?;
            self.next_effect_id = self.next_effect_id.saturating_add(1);
            let token = ContinuationToken(format!("continuation-{sequence}"));
            if !self.pending.contains_key(&token) && !self.completed.contains_key(&token) {
                return Ok(sequence);
            }
        }
    }

    fn expire_due(&mut self, now_ms: u64) -> Result<(), Fault> {
        validate_scheduler_time(now_ms)?;
        let expired = self.journal.expire_due(now_ms)?;
        for (image, step) in expired {
            self.pending.remove(&image.token);
            self.completed.insert(image.token, step);
        }
        Ok(())
    }

    fn finish_cancellation(
        &mut self,
        image: &ContinuationImage,
        reason: CancellationReason,
        observed_at_ms: u64,
    ) -> Step {
        let step = Step::Cancelled(Cancellation {
            continuation: image.token.clone(),
            reason,
            observed_at_ms,
        });
        let authoritative = match self.journal.cancel(image, &step) {
            Ok(step) => step,
            Err(error) => return Step::Fault(error),
        };
        self.pending.remove(&image.token);
        self.completed
            .insert(image.token.clone(), authoritative.clone());
        self.visible_step(&image.token, authoritative)
    }
}

impl ContinuationToken {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

pub fn encode_continuation(image: &ContinuationImage) -> Result<Vec<u8>, Fault> {
    validate_image(image)?;
    encode_json_capped(image, MAX_CONTINUATION_BYTES, "continuation")
}

fn encode_json_capped<T: Serialize>(
    value: &T,
    limit: usize,
    label: &str,
) -> Result<Vec<u8>, Fault> {
    let mut writer = CappedWriter::new(limit);
    if let Err(error) = serde_json::to_writer(&mut writer, value) {
        if writer.overflowed {
            return Err(Fault {
                code: "LSV3002".to_string(),
                message: format!("{label} exceeds {limit} bytes"),
            });
        }
        return Err(Fault {
            code: "LSV3001".to_string(),
            message: format!("failed to encode {label}: {error}"),
        });
    }
    Ok(writer.output)
}

pub fn decode_continuation(bytes: &[u8]) -> Result<ContinuationImage, Fault> {
    if bytes.len() > MAX_CONTINUATION_BYTES {
        return Err(Fault {
            code: "LSV3002".to_string(),
            message: format!("continuation exceeds {MAX_CONTINUATION_BYTES} bytes"),
        });
    }
    let image = serde_json::from_slice(bytes).map_err(|error| Fault {
        code: "LSV3003".to_string(),
        message: format!("invalid continuation: {error}"),
    })?;
    validate_image(&image)?;
    Ok(image)
}

fn validate_image(image: &ContinuationImage) -> Result<(), Fault> {
    if image.schema_version != CONTINUATION_SCHEMA_VERSION {
        return Err(Fault {
            code: "LSV2001".to_string(),
            message: format!(
                "unsupported continuation version {}, expected {}",
                image.schema_version, CONTINUATION_SCHEMA_VERSION
            ),
        });
    }
    if !valid_continuation_token(&image.token) {
        return Err(Fault {
            code: "LSV2006".to_string(),
            message: "invalid continuation token".to_string(),
        });
    }
    if image.fuel_remaining > MAX_EXECUTION_FUEL {
        return Err(Fault {
            code: "LSV2007".to_string(),
            message: "continuation fuel exceeds runtime limit".to_string(),
        });
    }
    if image.deadline_ms == 0 || image.deadline_ms > MAX_EFFECT_DEADLINE_MS {
        return Err(Fault {
            code: "LSV2008".to_string(),
            message: "continuation deadline exceeds runtime limit".to_string(),
        });
    }
    if image
        .deadline_at_ms
        .is_some_and(|deadline| deadline == 0 || deadline > i64::MAX as u64)
    {
        return Err(Fault {
            code: "LSV2011".to_string(),
            message: "continuation absolute deadline is out of range".to_string(),
        });
    }
    if image.max_output_items > DEFAULT_MAX_OUTPUT_ITEMS {
        return Err(Fault {
            code: "LSV2009".to_string(),
            message: "continuation output limit exceeds runtime maximum".to_string(),
        });
    }
    Ok(())
}

pub fn validate_effect_request(request: &EffectRequest) -> Result<(), Fault> {
    validate_image(&request.continuation)?;
    let token_suffix = request
        .continuation
        .token
        .as_str()
        .strip_prefix("continuation-")
        .ok_or_else(|| Fault {
            code: "LSV2010".to_string(),
            message: "effect request has a non-canonical continuation token".to_string(),
        })?;
    if request.effect_id != format!("effect-{token_suffix}") {
        return Err(Fault {
            code: "LSV2010".to_string(),
            message: "effect identifier does not match continuation token".to_string(),
        });
    }
    let operation_valid = match (&request.continuation.pending_effect, &request.operation) {
        (
            Effect::RuntimeList {
                filter: effect_filter,
            },
            EffectOperation::Query(query),
        ) => {
            validate_effect_identity(
                query.schema_version,
                &query.principal,
                &query.capabilities,
                &request.required_capability,
                CAPABILITY_RUNTIME_READ,
            )?;
            matches!(&query.query, Query::RuntimeList { filter } if filter == effect_filter)
        }
        (Effect::RuntimeInspect { runtime_id }, EffectOperation::Query(query)) => {
            validate_effect_identity(
                query.schema_version,
                &query.principal,
                &query.capabilities,
                &request.required_capability,
                CAPABILITY_RUNTIME_READ,
            )?;
            matches!(
                &query.query,
                Query::RuntimeInspect {
                    runtime_id: query_runtime_id,
                } if query_runtime_id == runtime_id
            )
        }
        (Effect::RuntimeHistory { runtime_id }, EffectOperation::Query(query)) => {
            validate_effect_identity(
                query.schema_version,
                &query.principal,
                &query.capabilities,
                &request.required_capability,
                CAPABILITY_RUNTIME_READ,
            )?;
            matches!(
                &query.query,
                Query::RuntimeHistory {
                    runtime_id: query_runtime_id,
                } if query_runtime_id == runtime_id
            )
        }
        (Effect::RuntimeLogs { runtime_id }, EffectOperation::Query(query)) => {
            validate_effect_identity(
                query.schema_version,
                &query.principal,
                &query.capabilities,
                &request.required_capability,
                CAPABILITY_RUNTIME_READ,
            )?;
            matches!(
                &query.query,
                Query::RuntimeLogs {
                    runtime_id: query_runtime_id,
                    after_sequence: None,
                    limit: MAX_RUNTIME_LOG_QUERY_ENTRIES,
                } if query_runtime_id == runtime_id
            )
        }
        (Effect::RuntimeRefresh { runtime_id }, EffectOperation::Command(command)) => {
            validate_effect_identity(
                command.schema_version,
                &command.principal,
                &command.capabilities,
                &request.required_capability,
                CAPABILITY_RUNTIME_REFRESH,
            )?;
            command.command_id
                == CommandId::new(format!("leselang-command-{token_suffix}"))
                    .expect("canonical command identifier is valid")
                && command.idempotency_key.as_str() == format!("leselang-effect-{token_suffix}")
                && command.expected_revision == request.continuation.expected_revision
                && command.origin == CommandOrigin::Leselang
                && command.confirmation == Confirmation::NotRequired
                && !command.dry_run
                && matches!(
                    &command.command,
                    Command::RuntimeRefresh { runtime_id: command_runtime_id }
                        if command_runtime_id == runtime_id
                )
        }
        (Effect::RuntimeCapabilitiesRefresh { runtime_id }, EffectOperation::Command(command)) => {
            validate_effect_identity(
                command.schema_version,
                &command.principal,
                &command.capabilities,
                &request.required_capability,
                CAPABILITY_RUNTIME_REFRESH,
            )?;
            command.command_id
                == CommandId::new(format!("leselang-command-{token_suffix}"))
                    .expect("canonical command identifier is valid")
                && command.idempotency_key.as_str() == format!("leselang-effect-{token_suffix}")
                && command.expected_revision == request.continuation.expected_revision
                && command.origin == CommandOrigin::Leselang
                && command.confirmation == Confirmation::NotRequired
                && !command.dry_run
                && matches!(
                    &command.command,
                    Command::RuntimeCapabilitiesRefresh { runtime_id: command_runtime_id }
                        if command_runtime_id == runtime_id
                )
        }
        (
            Effect::RuntimeDeploy {
                runtime_id,
                pipeline_kind,
                target,
            },
            EffectOperation::Command(command),
        ) => {
            validate_effect_identity(
                command.schema_version,
                &command.principal,
                &command.capabilities,
                &request.required_capability,
                CAPABILITY_RUNTIME_DEPLOY,
            )?;
            command.command_id
                == CommandId::new(format!("leselang-command-{token_suffix}"))
                    .expect("canonical command identifier is valid")
                && command.idempotency_key.as_str() == format!("leselang-effect-{token_suffix}")
                && command.expected_revision == request.continuation.expected_revision
                && command.origin == CommandOrigin::Leselang
                && command.confirmation == Confirmation::Confirmed
                && !command.dry_run
                && matches!(
                    &command.command,
                    Command::RuntimeDeploy {
                        runtime_id: command_runtime_id,
                        pipeline_kind: command_pipeline_kind,
                        target: command_target,
                    } if command_runtime_id == runtime_id
                        && command_pipeline_kind == pipeline_kind
                        && command_target == target
                )
        }
        _ => false,
    };
    if !operation_valid
        || request.budget.fuel_remaining != request.continuation.fuel_remaining
        || request.budget.deadline_ms != request.continuation.deadline_ms
        || request.budget.deadline_at_ms != request.continuation.deadline_at_ms
        || request.budget.max_output_items != request.continuation.max_output_items
    {
        return Err(Fault {
            code: "LSV2013".to_string(),
            message: "effect request does not match its continuation".to_string(),
        });
    }
    Ok(())
}

fn validate_scheduler_time(now_ms: u64) -> Result<(), Fault> {
    if now_ms > i64::MAX as u64 {
        return Err(Fault {
            code: "LSV2011".to_string(),
            message: "scheduler clock is out of range".to_string(),
        });
    }
    Ok(())
}

fn validate_debugger_audit(audit: &DebuggerAuditContext) -> Result<(), Fault> {
    let valid_session = !audit.session_id.is_empty()
        && audit.session_id.len() <= 128
        && audit
            .session_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':' | b'.'));
    let valid_principal =
        !audit.principal.id.is_empty()
            && audit.principal.id.len() <= 128
            && audit.principal.id.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':' | b'.')
            });
    if !valid_session || !valid_principal {
        return Err(Fault {
            code: "LSV2015".to_string(),
            message: "invalid debugger audit identity".to_string(),
        });
    }
    Ok(())
}

fn validate_effect_clock(now_ms: u64, timeout_ms: u64) -> Result<u64, Fault> {
    validate_scheduler_time(now_ms)?;
    if timeout_ms == 0 || timeout_ms > MAX_EFFECT_DEADLINE_MS {
        return Err(Fault {
            code: "LSV2012".to_string(),
            message: format!("effect timeout must be between 1 and {MAX_EFFECT_DEADLINE_MS} ms"),
        });
    }
    now_ms
        .checked_add(timeout_ms)
        .filter(|deadline| *deadline <= i64::MAX as u64)
        .ok_or_else(|| Fault {
            code: "LSV2011".to_string(),
            message: "effect absolute deadline is out of range".to_string(),
        })
}

fn validate_retry_policy(policy: &RetryPolicy) -> Result<(), Fault> {
    if policy.max_retries > MAX_SEMANTIC_RETRIES
        || policy.base_delay_ms == 0
        || policy.base_delay_ms > MAX_RETRY_DELAY_MS
        || policy.max_delay_ms < policy.base_delay_ms
        || policy.max_delay_ms > MAX_RETRY_DELAY_MS
    {
        return Err(Fault {
            code: "LSV2201".to_string(),
            message: "semantic retry policy exceeds runtime bounds".to_string(),
        });
    }
    Ok(())
}

fn validate_effect_error(error: &EffectError) -> Result<(), Fault> {
    if error.code.is_empty()
        || error.code.len() > 64
        || !error
            .code
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
        || error.message.is_empty()
        || error.message.len() > 1_024
    {
        return Err(Fault {
            code: "LSV2202".to_string(),
            message: "effect error metadata is invalid".to_string(),
        });
    }
    Ok(())
}

fn retry_delay(policy: &RetryPolicy, retry_count: u32) -> Result<u64, Fault> {
    let shift = retry_count.saturating_sub(1).min(63);
    let multiplier = 1_u64.checked_shl(shift).unwrap_or(u64::MAX);
    Ok(policy
        .base_delay_ms
        .saturating_mul(multiplier)
        .min(policy.max_delay_ms))
}

pub fn merge_declared(
    plan: &MergePlan,
    completions: Vec<BranchCompletion>,
    max_output_items: usize,
) -> Result<Step, Fault> {
    validate_merge_plan(plan)?;
    if max_output_items > DEFAULT_MAX_OUTPUT_ITEMS {
        return Err(Fault {
            code: "LSV2404".to_string(),
            message: "structured merge output budget exceeds runtime bounds".to_string(),
        });
    }
    if completions.len() != plan.branches.len() {
        return Err(Fault {
            code: "LSV2402".to_string(),
            message: "structured merge completion set is incomplete".to_string(),
        });
    }

    let expected = plan
        .branches
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let mut by_branch = BTreeMap::new();
    let mut output_items = 0usize;
    for completion in completions {
        if !valid_merge_branch_name(&completion.branch)
            || !expected.contains(completion.branch.as_str())
            || by_branch.contains_key(&completion.branch)
        {
            return Err(Fault {
                code: "LSV2402".to_string(),
                message: "structured merge completion set does not match its plan".to_string(),
            });
        }
        output_items = output_items.saturating_add(validate_branch_outcome(&completion.outcome)?);
        by_branch.insert(completion.branch, completion.outcome);
    }
    if output_items > max_output_items {
        return Err(Fault {
            code: "LSV2404".to_string(),
            message: format!(
                "structured merge returned {output_items} items, limit is {max_output_items}"
            ),
        });
    }

    let mut fields = Vec::with_capacity(plan.branches.len());
    let mut terminal = None;
    for branch in &plan.branches {
        let outcome = by_branch.remove(branch).ok_or_else(|| Fault {
            code: "LSV2402".to_string(),
            message: "structured merge completion set is incomplete".to_string(),
        })?;
        match outcome {
            BranchOutcome::Value(value) => fields.push(StructuredField {
                name: branch.clone(),
                value: Box::new(value),
            }),
            BranchOutcome::Cancelled(cancellation) => {
                terminal = Some(Step::Cancelled(cancellation));
                break;
            }
            BranchOutcome::Failed(failure) => {
                terminal = Some(Step::Failed(failure));
                break;
            }
            BranchOutcome::Fault(fault) => {
                terminal = Some(Step::Fault(fault));
                break;
            }
        }
    }
    let step = terminal.unwrap_or(Step::Done(Value::Structured { fields }));
    if let Step::Done(value) = &step {
        validate_value(value, 1)?;
    }
    encode_json_capped(&step, MAX_JOURNAL_ENTRY_BYTES, "structured merge output")?;
    Ok(step)
}

pub(crate) fn validate_merge_plan(plan: &MergePlan) -> Result<(), Fault> {
    let unique = plan.branches.iter().collect::<BTreeSet<_>>();
    if !(2..=MAX_MERGE_BRANCHES).contains(&plan.branches.len())
        || unique.len() != plan.branches.len()
        || plan
            .branches
            .iter()
            .any(|branch| !valid_merge_branch_name(branch))
    {
        return Err(Fault {
            code: "LSV2401".to_string(),
            message: "structured merge plan is invalid or exceeds runtime bounds".to_string(),
        });
    }
    Ok(())
}

fn valid_merge_branch_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_MERGE_BRANCH_NAME_BYTES
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn validate_branch_outcome(outcome: &BranchOutcome) -> Result<usize, Fault> {
    match outcome {
        BranchOutcome::Value(value) => validate_value(value, 2),
        BranchOutcome::Cancelled(cancellation)
            if valid_continuation_token(&cancellation.continuation)
                && cancellation.observed_at_ms <= i64::MAX as u64 =>
        {
            Ok(0)
        }
        BranchOutcome::Failed(failure)
            if failure.retry_count <= MAX_SEMANTIC_RETRIES
                && validate_effect_error(&failure.error).is_ok() =>
        {
            Ok(0)
        }
        BranchOutcome::Fault(fault)
            if !fault.code.is_empty() && fault.code.len() <= 32 && fault.message.len() <= 4_096 =>
        {
            Ok(0)
        }
        _ => Err(Fault {
            code: "LSV2403".to_string(),
            message: "structured merge branch outcome is invalid".to_string(),
        }),
    }
}

pub(crate) fn valid_continuation_token(token: &ContinuationToken) -> bool {
    !token.as_str().is_empty()
        && token.as_str().len() <= 128
        && token
            .as_str()
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

pub(crate) fn validate_value(value: &Value, depth: usize) -> Result<usize, Fault> {
    if depth > MAX_STRUCTURED_VALUE_DEPTH {
        return Err(Fault {
            code: "LSV2403".to_string(),
            message: "structured value nesting exceeds runtime bounds".to_string(),
        });
    }
    match value {
        Value::RuntimeList { runtimes, .. } if runtimes.len() <= DEFAULT_MAX_OUTPUT_ITEMS => {
            Ok(runtimes.len())
        }
        Value::RuntimeInspect { .. } => Ok(1),
        Value::RuntimeHistory { entries, .. } if entries.len() <= DEFAULT_MAX_OUTPUT_ITEMS => {
            Ok(entries.len())
        }
        Value::RuntimeLogs { entries, .. }
            if entries.len() <= usize::from(MAX_RUNTIME_LOG_QUERY_ENTRIES) =>
        {
            Ok(entries.len())
        }
        Value::RuntimeRefresh { .. }
        | Value::RuntimeCapabilitiesRefresh { .. }
        | Value::RuntimeDeploy { .. } => Ok(1),
        Value::Structured { fields } if (2..=MAX_MERGE_BRANCHES).contains(&fields.len()) => {
            let mut names = BTreeSet::new();
            let mut output_items = 0usize;
            for field in fields {
                if !valid_merge_branch_name(&field.name) || !names.insert(field.name.as_str()) {
                    return Err(Fault {
                        code: "LSV2403".to_string(),
                        message: "structured value fields are invalid".to_string(),
                    });
                }
                output_items = output_items
                    .saturating_add(validate_value(&field.value, depth.saturating_add(1))?);
            }
            Ok(output_items)
        }
        _ => Err(Fault {
            code: "LSV2403".to_string(),
            message: "structured value exceeds runtime bounds".to_string(),
        }),
    }
}

fn validate_retention_policy(policy: &RetentionPolicy) -> Result<(), Fault> {
    if policy.max_completed_records == 0
        || policy.max_completed_records > MAX_JOURNAL_RECORDS
        || policy.max_delete_per_run == 0
        || policy.max_delete_per_run > MAX_COMPACTION_BATCH
    {
        return Err(Fault {
            code: "LSV2301".to_string(),
            message: "journal retention policy exceeds runtime bounds".to_string(),
        });
    }
    Ok(())
}

fn continuation_age_order(
    left: &ContinuationToken,
    right: &ContinuationToken,
) -> std::cmp::Ordering {
    let sequence = |token: &ContinuationToken| {
        token
            .as_str()
            .strip_prefix("continuation-")
            .and_then(|value| value.parse::<u64>().ok())
    };
    match (sequence(left), sequence(right)) {
        (Some(left), Some(right)) => left.cmp(&right),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => left.cmp(right),
    }
}

fn validate_effect_identity(
    schema_version: u32,
    principal: &Principal,
    capabilities: &CapabilitySet,
    actual_capability: &str,
    expected_capability: &str,
) -> Result<(), Fault> {
    if actual_capability != expected_capability || !capabilities.contains(actual_capability) {
        return Err(Fault {
            code: "LSV2011".to_string(),
            message: "effect request capability is inconsistent".to_string(),
        });
    }
    if schema_version != DOMAIN_SCHEMA_VERSION
        || principal.id.is_empty()
        || principal.id.len() > 128
        || !principal
            .id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':' | b'.'))
    {
        return Err(Fault {
            code: "LSV2012".to_string(),
            message: "effect request envelope is invalid".to_string(),
        });
    }
    Ok(())
}

fn step_from_effect_result(
    image: &ContinuationImage,
    operation: Option<&EffectOperation>,
    result: EffectResult,
) -> Step {
    match (&image.pending_effect, image.result_type, operation, result) {
        (
            Effect::RuntimeList { .. },
            Type::RuntimeList,
            _,
            EffectResult::Query(QueryResult::RuntimeList { revision, runtimes }),
        ) => {
            if let Some(expected) = image.expected_revision
                && expected != revision
            {
                fault(
                    "LSV2101",
                    format!(
                        "effect revision conflict: expected {}, actual {}",
                        expected.0, revision.0
                    ),
                )
            } else if runtimes.len() > image.max_output_items {
                fault(
                    "LSV2102",
                    format!(
                        "effect returned {} items, limit is {}",
                        runtimes.len(),
                        image.max_output_items
                    ),
                )
            } else {
                Step::Done(Value::RuntimeList { revision, runtimes })
            }
        }
        (
            Effect::RuntimeInspect { runtime_id },
            Type::RuntimeInspect,
            _,
            EffectResult::Query(QueryResult::RuntimeInspect { revision, runtime }),
        ) if runtime.id == *runtime_id => {
            if let Some(expected) = image.expected_revision
                && expected != revision
            {
                fault(
                    "LSV2101",
                    format!(
                        "effect revision conflict: expected {}, actual {}",
                        expected.0, revision.0
                    ),
                )
            } else {
                Step::Done(Value::RuntimeInspect {
                    revision,
                    runtime: Box::new(runtime),
                })
            }
        }
        (
            Effect::RuntimeHistory { runtime_id },
            Type::RuntimeHistory,
            _,
            EffectResult::Query(QueryResult::RuntimeHistory { revision, entries }),
        ) if entries.iter().all(|entry| entry.runtime.id == *runtime_id) => {
            if let Some(expected) = image.expected_revision
                && expected != revision
            {
                fault(
                    "LSV2101",
                    format!(
                        "effect revision conflict: expected {}, actual {}",
                        expected.0, revision.0
                    ),
                )
            } else if entries.len() > image.max_output_items {
                fault(
                    "LSV2102",
                    format!(
                        "effect returned {} items, limit is {}",
                        entries.len(),
                        image.max_output_items
                    ),
                )
            } else {
                Step::Done(Value::RuntimeHistory { revision, entries })
            }
        }
        (
            Effect::RuntimeLogs { runtime_id },
            Type::RuntimeLogs,
            _,
            EffectResult::Query(QueryResult::RuntimeLogs {
                revision,
                runtime_id: result_runtime_id,
                runtime_name,
                entries,
            }),
        ) if result_runtime_id == *runtime_id => {
            if let Some(expected) = image.expected_revision
                && expected != revision
            {
                fault(
                    "LSV2101",
                    format!(
                        "effect revision conflict: expected {}, actual {}",
                        expected.0, revision.0
                    ),
                )
            } else {
                let output_limit = image
                    .max_output_items
                    .min(usize::from(MAX_RUNTIME_LOG_QUERY_ENTRIES));
                if entries.len() > output_limit {
                    fault(
                        "LSV2102",
                        format!(
                            "effect returned {} items, limit is {}",
                            entries.len(),
                            output_limit
                        ),
                    )
                } else {
                    Step::Done(Value::RuntimeLogs {
                        revision,
                        runtime_id: result_runtime_id,
                        runtime_name,
                        entries,
                    })
                }
            }
        }
        (
            Effect::RuntimeRefresh { runtime_id },
            Type::RuntimeRefresh,
            Some(EffectOperation::Command(command)),
            EffectResult::Command(result),
        ) if result.runtime.id == *runtime_id
            && result.command_id == command.command_id
            && result.status == CommandStatus::Applied =>
        {
            Step::Done(Value::RuntimeRefresh { result })
        }
        (
            Effect::RuntimeCapabilitiesRefresh { runtime_id },
            Type::RuntimeCapabilitiesRefresh,
            Some(EffectOperation::Command(command)),
            EffectResult::Command(result),
        ) if result.runtime.id == *runtime_id
            && result.command_id == command.command_id
            && result.status == CommandStatus::Applied =>
        {
            Step::Done(Value::RuntimeCapabilitiesRefresh { result })
        }
        (
            Effect::RuntimeDeploy { runtime_id, .. },
            Type::RuntimeDeploy,
            Some(EffectOperation::Command(command)),
            EffectResult::Command(result),
        ) if result.runtime.id == *runtime_id
            && result.command_id == command.command_id
            && result.status == CommandStatus::Applied =>
        {
            Step::Done(Value::RuntimeDeploy { result })
        }
        _ => fault("LSV2103", "effect result does not match pending effect"),
    }
}

struct CappedWriter {
    output: Vec<u8>,
    limit: usize,
    overflowed: bool,
}

impl CappedWriter {
    fn new(limit: usize) -> Self {
        Self {
            output: Vec::new(),
            limit,
            overflowed: false,
        }
    }
}

impl Write for CappedWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if self.output.len().saturating_add(buffer.len()) > self.limit {
            self.overflowed = true;
            return Err(io::Error::new(
                io::ErrorKind::FileTooLarge,
                "continuation exceeds limit",
            ));
        }
        self.output.extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn fault(code: &str, message: impl Into<String>) -> Step {
    Step::Fault(Fault {
        code: code.to_string(),
        message: message.into(),
    })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    use leselang_hir::lower;
    use leselang_syntax::parse;
    use leserpent_domain::{
        CAPABILITY_RUNTIME_READ, CAPABILITY_RUNTIME_REFRESH, InMemoryControlPlane, RuntimeId,
        RuntimeListFilter,
    };

    use super::*;

    static NEXT_TEMP_JOURNAL: AtomicU64 = AtomicU64::new(1);

    struct TempJournal {
        root: PathBuf,
        path: PathBuf,
    }

    impl TempJournal {
        fn new(label: &str) -> Self {
            let sequence = NEXT_TEMP_JOURNAL.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!(
                "leselang-vm-{label}-{}-{sequence}",
                std::process::id()
            ));
            fs::create_dir(&root).unwrap();
            let path = root.join("effects.sqlite3");
            Self { root, path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempJournal {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn program() -> HirProgram {
        lower(&parse(
            "fn main() = runtime.list(environment: \"production\", role: none)",
        ))
        .unwrap()
    }

    fn start(vm: &mut Vm, expected_revision: Option<Revision>) -> EffectRequest {
        let Step::Effect(request) = vm.start(
            &program(),
            Principal {
                id: "operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
            expected_revision,
        ) else {
            panic!("expected effect");
        };
        *request
    }

    fn start_timed(vm: &mut Vm, now_ms: u64, timeout_ms: u64) -> EffectRequest {
        let Step::Effect(request) = vm.start_timed(
            &program(),
            Principal {
                id: "operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
            None,
            now_ms,
            timeout_ms,
        ) else {
            panic!("expected timed effect");
        };
        *request
    }

    fn runtime_list_result(revision: u64) -> EffectResult {
        EffectResult::Query(QueryResult::RuntimeList {
            revision: Revision(revision),
            runtimes: Vec::new(),
        })
    }

    fn runtime_list_value(revision: u64) -> Value {
        Value::RuntimeList {
            revision: Revision(revision),
            runtimes: Vec::new(),
        }
    }

    fn completed_branch(branch: &str, revision: u64) -> BranchCompletion {
        BranchCompletion {
            branch: branch.to_string(),
            outcome: BranchOutcome::Value(runtime_list_value(revision)),
        }
    }

    fn transient_error() -> EffectError {
        EffectError {
            class: EffectErrorClass::Transient,
            code: "upstream.unavailable".to_string(),
            message: "runtime endpoint is temporarily unavailable".to_string(),
        }
    }

    fn start_refresh(vm: &mut Vm, expected_revision: Revision) -> EffectRequest {
        let program = lower(&parse(
            "fn main() = runtime.refresh(runtime_id: \"runtime-a\")",
        ))
        .unwrap();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_RUNTIME_REFRESH]),
            Some(expected_revision),
        ) else {
            panic!("expected refresh effect");
        };
        *request
    }

    fn start_capabilities_refresh(vm: &mut Vm, expected_revision: Revision) -> EffectRequest {
        let program = lower(&parse(
            "fn main() = runtime.refresh_capabilities(runtime_id: \"runtime-a\")",
        ))
        .unwrap();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_RUNTIME_REFRESH]),
            Some(expected_revision),
        ) else {
            panic!("expected capabilities refresh effect");
        };
        *request
    }

    fn start_deploy(vm: &mut Vm, expected_revision: Revision) -> EffectRequest {
        let program = lower(&parse(
            "fn main() = runtime.deploy(runtime_id: \"runtime-a\", pipeline_kind: \"http/request\", target: \"pid:42\")",
        ))
        .unwrap();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_RUNTIME_DEPLOY]),
            Some(expected_revision),
        ) else {
            panic!("expected deployment effect");
        };
        *request
    }

    #[test]
    fn stackless_start_emits_typed_runtime_list_effect() {
        let mut vm = Vm::default();
        let request = start(&mut vm, Some(Revision(7)));
        assert_eq!(request.required_capability, CAPABILITY_RUNTIME_READ);
        assert_eq!(request.continuation.program_counter, 1);
        assert_eq!(vm.pending_count(), 1);
        let EffectOperation::Query(query) = request.operation else {
            panic!("expected query operation");
        };
        let Query::RuntimeList { filter } = query.query else {
            panic!("runtime list must produce a list query");
        };
        assert_eq!(
            filter,
            RuntimeListFilter {
                environment: Some("production".to_string()),
                cluster: None,
                role: None,
            }
        );
    }

    #[test]
    fn runtime_inspect_reenters_with_one_typed_projection() {
        let program = lower(&parse(
            "fn main() = runtime.inspect(runtime_id: \"runtime-a\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
            Some(Revision(1)),
        ) else {
            panic!("expected inspect effect");
        };
        let EffectOperation::Query(query) = &request.operation else {
            panic!("inspect must produce a query");
        };
        assert!(matches!(
            &query.query,
            Query::RuntimeInspect { runtime_id } if runtime_id.as_str() == "runtime-a"
        ));
        let mut control = InMemoryControlPlane::default();
        control.register_runtime(RuntimeId::new("runtime-a").unwrap(), "A", "http://a");
        let result = control.query(query.clone()).unwrap();
        assert!(matches!(
            vm.resume(&request.continuation, EffectResult::Query(result)),
            Step::Done(Value::RuntimeInspect {
                revision: Revision(1),
                runtime,
            }) if runtime.id.as_str() == "runtime-a"
        ));
    }

    #[test]
    fn runtime_history_reenters_with_a_bounded_typed_sequence() {
        let program = lower(&parse(
            "fn main() = runtime.history(runtime_id: \"runtime-a\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
            Some(Revision(1)),
        ) else {
            panic!("expected history effect");
        };
        let EffectOperation::Query(query) = &request.operation else {
            panic!("history must produce a query");
        };
        assert!(matches!(
            &query.query,
            Query::RuntimeHistory { runtime_id } if runtime_id.as_str() == "runtime-a"
        ));
        let mut control = InMemoryControlPlane::default();
        control.register_runtime(RuntimeId::new("runtime-a").unwrap(), "A", "http://a");
        let result = control.query(query.clone()).unwrap();
        assert!(matches!(
            vm.resume(&request.continuation, EffectResult::Query(result)),
            Step::Done(Value::RuntimeHistory {
                revision: Revision(1),
                entries,
            }) if entries.is_empty()
        ));
    }

    #[test]
    fn runtime_logs_reenters_with_a_bounded_typed_sequence() {
        let program = lower(&parse(
            "fn main() = runtime.logs(runtime_id: \"runtime-a\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
            Some(Revision(3)),
        ) else {
            panic!("expected logs effect");
        };
        let EffectOperation::Query(query) = &request.operation else {
            panic!("logs must produce a query");
        };
        assert!(matches!(
            &query.query,
            Query::RuntimeLogs {
                runtime_id,
                after_sequence: None,
                limit: MAX_RUNTIME_LOG_QUERY_ENTRIES,
            } if runtime_id.as_str() == "runtime-a"
        ));
        let result = QueryResult::RuntimeLogs {
            revision: Revision(3),
            runtime_id: RuntimeId::new("runtime-a").unwrap(),
            runtime_name: "Runtime A".to_string(),
            entries: vec![RuntimeLogRecord {
                sequence: 1,
                level: leserpent_domain::RuntimeLogLevel::Warning,
                message: "packet loss detected".to_string(),
            }],
        };
        assert!(matches!(
            vm.resume(&request.continuation, EffectResult::Query(result)),
            Step::Done(Value::RuntimeLogs {
                revision: Revision(3),
                runtime_id,
                runtime_name,
                entries,
            }) if runtime_id.as_str() == "runtime-a"
                && runtime_name == "Runtime A"
                && entries.len() == 1
        ));
    }

    #[test]
    fn runtime_logs_rejects_an_overproducing_host() {
        let program = lower(&parse(
            "fn main() = runtime.logs(runtime_id: \"runtime-a\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
            None,
        ) else {
            panic!("expected logs effect");
        };
        let entry = RuntimeLogRecord {
            sequence: 1,
            level: leserpent_domain::RuntimeLogLevel::Info,
            message: "bounded".to_string(),
        };
        let result = QueryResult::RuntimeLogs {
            revision: Revision(1),
            runtime_id: RuntimeId::new("runtime-a").unwrap(),
            runtime_name: "Runtime A".to_string(),
            entries: vec![entry; usize::from(MAX_RUNTIME_LOG_QUERY_ENTRIES) + 1],
        };
        assert!(matches!(
            vm.resume(&request.continuation, EffectResult::Query(result)),
            Step::Fault(Fault { code, .. }) if code == "LSV2102"
        ));
    }

    #[test]
    fn structured_all_starts_as_an_ordered_atomic_effect_batch() {
        let structured = lower(&parse(
            "fn main() = all(read: runtime.list(), refresh: runtime.refresh(runtime_id: \"runtime-a\"))",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let unauthorized = vm.start(
            &structured,
            Principal {
                id: "operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
            Some(Revision(1)),
        );
        assert!(matches!(
            unauthorized,
            Step::Fault(Fault { ref code, .. }) if code == "LSH2001"
        ));

        let executable = lower(&parse(
            "fn main() = all(left: runtime.list(), right: runtime.list())",
        ))
        .unwrap();
        let started = vm.start(
            &executable,
            Principal {
                id: "operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
            None,
        );
        let Step::Effects(batch) = started else {
            panic!("expected structured effect batch");
        };
        assert_eq!(batch.merge_token.as_str(), "merge-1");
        assert_eq!(
            batch
                .branches
                .iter()
                .map(|branch| branch.branch.as_str())
                .collect::<Vec<_>>(),
            ["left", "right"]
        );
        assert_eq!(
            batch
                .branches
                .iter()
                .map(|branch| branch.request.continuation.token.as_str())
                .collect::<Vec<_>>(),
            ["continuation-2", "continuation-3"]
        );
        assert_eq!(vm.pending_count(), 2);
        assert_eq!(vm.completed_count(), 0);

        let first = vm.claim_effect(10, 100).unwrap().unwrap();
        assert!(matches!(
            vm.acknowledge_effect(&first, 11, runtime_list_result(1)),
            Step::Waiting(MergeWait {
                completed_branches: 1,
                total_branches: 2,
                ..
            })
        ));
        let second = vm.claim_effect(10, 100).unwrap().unwrap();
        assert!(matches!(
            vm.acknowledge_effect(&second, 11, runtime_list_result(2)),
            Step::Done(Value::Structured { .. })
        ));
        assert!(matches!(
            vm.merge_result(&batch.merge_token).unwrap(),
            Some(Step::Done(Value::Structured { .. }))
        ));
    }

    #[test]
    fn durable_structured_start_resumes_remaining_branches_after_restart() {
        let journal = TempJournal::new("structured-start-restart");
        let program = lower(&parse(
            "fn main() = all(left: runtime.list(), right: runtime.list())",
        ))
        .unwrap();
        let merge_token = {
            let mut vm = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
            let Step::Effects(batch) = vm.start(
                &program,
                Principal {
                    id: "operator".to_string(),
                },
                CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
                None,
            ) else {
                panic!("expected durable structured batch");
            };
            let first = vm.claim_effect(10, 100).unwrap().unwrap();
            assert!(matches!(
                vm.acknowledge_effect(&first, 11, runtime_list_result(1)),
                Step::Waiting(MergeWait {
                    completed_branches: 1,
                    total_branches: 2,
                    ..
                })
            ));
            batch.merge_token.clone()
        };

        let mut recovered = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        assert_eq!(recovered.pending_count(), 1);
        assert_eq!(recovered.completed_count(), 1);
        let second = recovered.claim_effect(20, 100).unwrap().unwrap();
        let Step::Done(Value::Structured { fields }) =
            recovered.acknowledge_effect(&second, 21, runtime_list_result(2))
        else {
            panic!("expected restart-safe structured result");
        };
        assert_eq!(
            fields
                .iter()
                .map(|field| field.name.as_str())
                .collect::<Vec<_>>(),
            ["left", "right"]
        );
        assert!(matches!(
            recovered.merge_result(&merge_token).unwrap(),
            Some(Step::Done(Value::Structured { .. }))
        ));
    }

    #[test]
    fn nested_all_fails_before_sequence_allocation() {
        let nested = lower(&parse(
            "fn main() = all(outer: all(left: runtime.list(), right: runtime.list()), tail: runtime.list())",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let unsupported = vm.start(
            &nested,
            Principal {
                id: "operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
            None,
        );
        assert!(matches!(
            unsupported,
            Step::Fault(Fault { ref code, .. }) if code == "LSV1002"
        ));
        assert_eq!(vm.pending_count(), 0);
        assert_eq!(
            start(&mut vm, None).continuation.token.as_str(),
            "continuation-1"
        );
    }

    fn merge_branch_requests() -> (EffectRequest, EffectRequest, MergePlan) {
        let mut source = Vm::default();
        let left = start(&mut source, None);
        let right = start(&mut source, None);
        let plan = MergePlan::new(["left", "right"]).unwrap();
        (left, right, plan)
    }

    #[test]
    fn durable_merge_graph_creation_recovers_every_branch_in_declared_order() {
        let journal = TempJournal::new("merge-graph-create");
        let (left, right, plan) = merge_branch_requests();
        {
            let mut vm = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
            vm.journal
                .record_merge_graph(
                    &ContinuationToken("merge-1".to_string()),
                    &plan,
                    &[("left", &left), ("right", &right)],
                )
                .unwrap();
        }

        let mut recovered = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        assert_eq!(recovered.pending_count(), 2);
        let first = recovered.claim_effect(10, 100).unwrap().unwrap();
        assert_eq!(first.request, left);
        let second = recovered.claim_effect(10, 100).unwrap().unwrap();
        assert_eq!(second.request, right);
        assert!(recovered.claim_effect(10, 100).unwrap().is_none());
    }

    #[test]
    fn ephemeral_merge_graph_creation_uses_the_same_declared_order() {
        let (left, right, plan) = merge_branch_requests();
        let mut vm = Vm::default();
        vm.journal
            .record_merge_graph(
                &ContinuationToken("merge-1".to_string()),
                &plan,
                &[("left", &left), ("right", &right)],
            )
            .unwrap();

        let first = vm.claim_effect(10, 100).unwrap().unwrap();
        assert_eq!(first.request, left);
        vm.journal
            .acknowledge_dispatch(&first, 11, &Step::Done(runtime_list_value(1)))
            .unwrap();
        let second = vm.claim_effect(10, 100).unwrap().unwrap();
        assert_eq!(second.request, right);
        vm.journal
            .acknowledge_dispatch(&second, 11, &Step::Done(runtime_list_value(2)))
            .unwrap();
        assert!(vm.claim_effect(10, 100).unwrap().is_none());
        let merged = vm
            .journal
            .merge_result(&ContinuationToken("merge-1".to_string()))
            .unwrap()
            .unwrap();
        let Step::Done(Value::Structured { fields }) = merged else {
            panic!("expected structured merge result");
        };
        assert_eq!(
            fields
                .iter()
                .map(|field| field.name.as_str())
                .collect::<Vec<_>>(),
            ["left", "right"]
        );
    }

    #[test]
    fn ephemeral_compaction_deletes_completed_merge_graphs_as_logical_units() {
        let mut source = Vm::default();
        let requests = (0..4).map(|_| start(&mut source, None)).collect::<Vec<_>>();
        let plan = MergePlan::new(["left", "right"]).unwrap();
        let first_token = ContinuationToken("merge-1".to_string());
        let second_token = ContinuationToken("merge-2".to_string());
        let mut vm = Vm::default();
        vm.journal
            .record_merge_graph(
                &first_token,
                &plan,
                &[("left", &requests[0]), ("right", &requests[1])],
            )
            .unwrap();
        vm.journal
            .record_merge_graph(
                &second_token,
                &plan,
                &[("left", &requests[2]), ("right", &requests[3])],
            )
            .unwrap();

        let mut local_completed = Vec::new();
        for revision in 1..=4 {
            let lease = vm.claim_effect(10, 100).unwrap().unwrap();
            let step = Step::Done(runtime_list_value(revision));
            vm.journal.acknowledge_dispatch(&lease, 11, &step).unwrap();
            local_completed.push((lease.request.continuation.token, step));
        }
        let compacted = vm
            .journal
            .compact_completed(
                &local_completed,
                &RetentionPolicy {
                    max_completed_records: 1,
                    max_delete_per_run: 1,
                },
            )
            .unwrap();
        assert_eq!(compacted.removed_records, 1);
        assert_eq!(compacted.remaining_completed, 1);
        assert_eq!(compacted.pruned_tokens.len(), 2);
        assert!(vm.journal.merge_result(&first_token).unwrap().is_none());
        assert!(vm.journal.merge_result(&second_token).unwrap().is_some());
    }

    #[test]
    fn durable_final_branch_atomically_commits_restart_safe_merge_result() {
        let journal = TempJournal::new("merge-final-completion");
        let (left, right, plan) = merge_branch_requests();
        let merge_token = ContinuationToken("merge-1".to_string());
        {
            let mut vm = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
            vm.journal
                .record_merge_graph(&merge_token, &plan, &[("left", &left), ("right", &right)])
                .unwrap();
        }
        {
            let mut vm = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
            let first = vm.claim_effect(10, 100).unwrap().unwrap();
            assert!(matches!(
                vm.acknowledge_effect(&first, 11, runtime_list_result(1)),
                Step::Waiting(_)
            ));
            assert!(vm.journal.merge_result(&merge_token).unwrap().is_none());

            let second = vm.claim_effect(10, 100).unwrap().unwrap();
            assert!(matches!(
                vm.acknowledge_effect(&second, 11, runtime_list_result(2)),
                Step::Done(Value::Structured { .. })
            ));
            assert!(matches!(
                vm.journal.merge_result(&merge_token).unwrap(),
                Some(Step::Done(Value::Structured { .. }))
            ));
        }

        let recovered = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        let Some(Step::Done(Value::Structured { fields })) =
            recovered.journal.merge_result(&merge_token).unwrap()
        else {
            panic!("expected recovered structured merge result");
        };
        assert_eq!(
            fields
                .iter()
                .map(|field| field.name.as_str())
                .collect::<Vec<_>>(),
            ["left", "right"]
        );
        assert!(matches!(
            fields[0].value.as_ref(),
            Value::RuntimeList {
                revision: Revision(1),
                ..
            }
        ));
        assert!(matches!(
            fields[1].value.as_ref(),
            Value::RuntimeList {
                revision: Revision(2),
                ..
            }
        ));
    }

    #[test]
    fn durable_merge_failure_rolls_back_the_final_branch_completion() {
        let journal = TempJournal::new("merge-final-rollback");
        let (left, right, plan) = merge_branch_requests();
        let merge_token = ContinuationToken("merge-1".to_string());
        {
            let mut vm = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
            vm.journal
                .record_merge_graph(&merge_token, &plan, &[("left", &left), ("right", &right)])
                .unwrap();
        }
        let mut vm = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        let first = vm.claim_effect(10, 100).unwrap().unwrap();
        assert!(matches!(
            vm.acknowledge_effect(&first, 11, runtime_list_result(1)),
            Step::Waiting(_)
        ));
        let connection = rusqlite::Connection::open(journal.path()).unwrap();
        connection
            .execute_batch(
                "CREATE TRIGGER reject_merge_terminal
                 BEFORE UPDATE OF state ON vm_merge_groups
                 WHEN NEW.state = 'completed'
                 BEGIN
                   SELECT RAISE(ABORT, 'injected merge completion failure');
                 END;",
            )
            .unwrap();
        let second = vm.claim_effect(20, 100).unwrap().unwrap();
        let failed = vm.acknowledge_effect(&second, 21, runtime_list_result(2));
        assert!(matches!(failed, Step::Fault(Fault { ref code, .. }) if code == "LSV4033"));

        let group_state: String = connection
            .query_row(
                "SELECT state FROM vm_merge_groups WHERE token = 'merge-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let second_state: String = connection
            .query_row(
                "SELECT state FROM vm_effects WHERE token = 'continuation-2'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let dispatch_state: String = connection
            .query_row(
                "SELECT state FROM vm_dispatches WHERE token = 'continuation-2'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(group_state, "pending");
        assert_eq!(second_state, "pending");
        assert_eq!(dispatch_state, "leased");
    }

    #[test]
    fn durable_merge_commits_declared_terminal_failure() {
        let journal = TempJournal::new("merge-terminal-failure");
        let (left, right, plan) = merge_branch_requests();
        let merge_token = ContinuationToken("merge-1".to_string());
        {
            let mut vm = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
            vm.journal
                .record_merge_graph(&merge_token, &plan, &[("left", &left), ("right", &right)])
                .unwrap();
        }

        let mut vm = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        let first = vm.claim_effect(10, 100).unwrap().unwrap();
        assert!(matches!(
            vm.acknowledge_effect(&first, 11, runtime_list_result(1)),
            Step::Waiting(_)
        ));
        let second = vm.claim_effect(20, 100).unwrap().unwrap();
        let disposition = vm
            .report_effect_error(
                &second,
                21,
                EffectError {
                    class: EffectErrorClass::Permanent,
                    code: "runtime.rejected".to_string(),
                    message: "runtime rejected the branch".to_string(),
                },
                &RetryPolicy::default(),
            )
            .unwrap();
        assert!(matches!(
            disposition,
            RetryDisposition::Terminal(Step::Failed(_))
        ));
        assert!(matches!(
            vm.journal.merge_result(&merge_token).unwrap(),
            Some(Step::Failed(FailedEffect {
                error: EffectError { ref code, .. },
                ..
            })) if code == "runtime.rejected"
        ));
    }

    #[test]
    fn retention_keeps_a_completed_merge_graph_within_the_configured_limit() {
        let journal = TempJournal::new("merge-compaction-protection");
        let (left, right, plan) = merge_branch_requests();
        let merge_token = ContinuationToken("merge-1".to_string());
        {
            let mut vm = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
            vm.journal
                .record_merge_graph(&merge_token, &plan, &[("left", &left), ("right", &right)])
                .unwrap();
        }
        let mut vm = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        for revision in [1, 2] {
            let lease = vm.claim_effect(10, 100).unwrap().unwrap();
            let step = vm.acknowledge_effect(&lease, 11, runtime_list_result(revision));
            assert!(if revision == 1 {
                matches!(step, Step::Waiting(_))
            } else {
                matches!(step, Step::Done(Value::Structured { .. }))
            });
        }
        let report = vm
            .compact_journal(&RetentionPolicy {
                max_completed_records: 1,
                max_delete_per_run: 100,
            })
            .unwrap();
        assert_eq!(report.removed_records, 0);
        drop(vm);

        let recovered = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        assert!(matches!(
            recovered.journal.merge_result(&merge_token).unwrap(),
            Some(Step::Done(Value::Structured { .. }))
        ));
    }

    #[test]
    fn durable_compaction_deletes_the_oldest_completed_merge_graph_as_one_unit() {
        let journal = TempJournal::new("merge-group-compaction");
        let mut source = Vm::default();
        let requests = (0..4).map(|_| start(&mut source, None)).collect::<Vec<_>>();
        let plan = MergePlan::new(["left", "right"]).unwrap();
        let first_token = ContinuationToken("merge-1".to_string());
        let second_token = ContinuationToken("merge-2".to_string());
        {
            let mut vm = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
            vm.journal
                .record_merge_graph(
                    &first_token,
                    &plan,
                    &[("left", &requests[0]), ("right", &requests[1])],
                )
                .unwrap();
            vm.journal
                .record_merge_graph(
                    &second_token,
                    &plan,
                    &[("left", &requests[2]), ("right", &requests[3])],
                )
                .unwrap();
        }

        let mut vm = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        for revision in 1..=4 {
            let lease = vm.claim_effect(10, 100).unwrap().unwrap();
            let step = vm.acknowledge_effect(&lease, 11, runtime_list_result(revision));
            assert!(if revision % 2 == 1 {
                matches!(step, Step::Waiting(_))
            } else {
                matches!(step, Step::Done(Value::Structured { .. }))
            });
        }
        let report = vm
            .compact_journal(&RetentionPolicy {
                max_completed_records: 1,
                max_delete_per_run: 1,
            })
            .unwrap();
        assert_eq!(report.removed_records, 1);
        assert_eq!(report.remaining_completed, 1);
        assert!(report.reclaimed_logical_bytes > 0);
        assert_eq!(vm.completed_count(), 2);
        assert!(vm.journal.merge_result(&first_token).unwrap().is_none());
        assert!(matches!(
            vm.journal.merge_result(&second_token).unwrap(),
            Some(Step::Done(Value::Structured { .. }))
        ));
        drop(vm);

        let connection = rusqlite::Connection::open(journal.path()).unwrap();
        let counts: (i64, i64, i64, i64) = connection
            .query_row(
                "SELECT
                   (SELECT COUNT(*) FROM vm_merge_groups),
                   (SELECT COUNT(*) FROM vm_merge_branches),
                   (SELECT COUNT(*) FROM vm_effects),
                   (SELECT COUNT(*) FROM vm_dispatches)",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(counts, (1, 2, 2, 2));
        drop(connection);

        let recovered = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        assert_eq!(recovered.completed_count(), 2);
        assert!(matches!(
            recovered.journal.merge_result(&second_token).unwrap(),
            Some(Step::Done(Value::Structured { .. }))
        ));
    }

    #[test]
    fn durable_merge_graph_creation_rolls_back_every_row_on_branch_failure() {
        let journal = TempJournal::new("merge-graph-rollback");
        let (left, right, plan) = merge_branch_requests();
        let mut vm = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        let connection = rusqlite::Connection::open(journal.path()).unwrap();
        connection
            .execute_batch(
                "CREATE TRIGGER reject_second_merge_branch
                 BEFORE INSERT ON vm_effects
                 WHEN NEW.token = 'continuation-2'
                 BEGIN
                   SELECT RAISE(ABORT, 'injected branch failure');
                 END;",
            )
            .unwrap();

        let error = vm
            .journal
            .record_merge_graph(
                &ContinuationToken("merge-1".to_string()),
                &plan,
                &[("left", &left), ("right", &right)],
            )
            .unwrap_err();
        assert_eq!(error.code, "LSV4032");
        for table in [
            "vm_merge_groups",
            "vm_merge_branches",
            "vm_effects",
            "vm_dispatches",
        ] {
            let count: i64 = connection
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .unwrap();
            assert_eq!(count, 0, "{table} must roll back with the graph");
        }
    }

    #[test]
    fn structured_merge_uses_declaration_order_not_arrival_order() {
        let plan = MergePlan::new(["inventory", "health", "history"]).unwrap();
        let merged = merge_declared(
            &plan,
            vec![
                completed_branch("history", 3),
                completed_branch("inventory", 1),
                completed_branch("health", 2),
            ],
            DEFAULT_MAX_OUTPUT_ITEMS,
        )
        .unwrap();
        let Step::Done(Value::Structured { fields }) = merged else {
            panic!("expected structured value");
        };
        assert_eq!(
            fields
                .iter()
                .map(|field| field.name.as_str())
                .collect::<Vec<_>>(),
            ["inventory", "health", "history"]
        );
        for (field, expected_revision) in fields.iter().zip(1..=3) {
            assert!(matches!(
                field.value.as_ref(),
                Value::RuntimeList { revision, .. } if *revision == Revision(expected_revision)
            ));
        }
    }

    #[test]
    fn structured_merge_terminal_winner_is_deterministic() {
        let plan = MergePlan::new(["primary", "secondary", "fallback"]).unwrap();
        let merged = merge_declared(
            &plan,
            vec![
                BranchCompletion {
                    branch: "fallback".to_string(),
                    outcome: BranchOutcome::Cancelled(Cancellation {
                        continuation: ContinuationToken("continuation-3".to_string()),
                        reason: CancellationReason::Requested,
                        observed_at_ms: 30,
                    }),
                },
                BranchCompletion {
                    branch: "secondary".to_string(),
                    outcome: BranchOutcome::Fault(Fault {
                        code: "SECOND".to_string(),
                        message: "secondary failed first in declared order".to_string(),
                    }),
                },
                completed_branch("primary", 1),
            ],
            DEFAULT_MAX_OUTPUT_ITEMS,
        )
        .unwrap();
        assert!(matches!(merged, Step::Fault(Fault { ref code, .. }) if code == "SECOND"));
    }

    #[test]
    fn structured_merge_rejects_invalid_sets_and_resource_budgets() {
        assert_eq!(MergePlan::new(["only"]).unwrap_err().code, "LSV2401");
        assert_eq!(
            MergePlan::new(["same", "same"]).unwrap_err().code,
            "LSV2401"
        );
        let plan = MergePlan::new(["left", "right"]).unwrap();
        assert_eq!(
            merge_declared(
                &plan,
                vec![completed_branch("left", 1), completed_branch("left", 2)],
                DEFAULT_MAX_OUTPUT_ITEMS,
            )
            .unwrap_err()
            .code,
            "LSV2402"
        );
        assert_eq!(
            merge_declared(
                &plan,
                vec![completed_branch("left", 1)],
                DEFAULT_MAX_OUTPUT_ITEMS,
            )
            .unwrap_err()
            .code,
            "LSV2402"
        );

        let mut control = InMemoryControlPlane::default();
        let runtime = control.register_runtime(
            RuntimeId::new("runtime-a").unwrap(),
            "Alpha",
            "http://runtime-a",
        );
        assert_eq!(
            merge_declared(
                &plan,
                vec![
                    BranchCompletion {
                        branch: "left".to_string(),
                        outcome: BranchOutcome::Value(Value::RuntimeList {
                            revision: Revision(1),
                            runtimes: vec![runtime],
                        }),
                    },
                    completed_branch("right", 2),
                ],
                0,
            )
            .unwrap_err()
            .code,
            "LSV2404"
        );
        assert_eq!(
            merge_declared(
                &plan,
                vec![
                    BranchCompletion {
                        branch: "left".to_string(),
                        outcome: BranchOutcome::Fault(Fault {
                            code: "TOO_LARGE".to_string(),
                            message: "x".repeat(4_097),
                        }),
                    },
                    completed_branch("right", 2),
                ],
                DEFAULT_MAX_OUTPUT_ITEMS,
            )
            .unwrap_err()
            .code,
            "LSV2403"
        );
    }

    #[test]
    fn structured_merge_rejects_excessive_nesting() {
        let mut nested = runtime_list_value(1);
        for depth in 0..MAX_STRUCTURED_VALUE_DEPTH {
            nested = Value::Structured {
                fields: vec![
                    StructuredField {
                        name: "leaf".to_string(),
                        value: Box::new(runtime_list_value(depth as u64)),
                    },
                    StructuredField {
                        name: "nested".to_string(),
                        value: Box::new(nested),
                    },
                ],
            };
        }
        let plan = MergePlan::new(["deep", "peer"]).unwrap();
        assert_eq!(
            merge_declared(
                &plan,
                vec![
                    BranchCompletion {
                        branch: "deep".to_string(),
                        outcome: BranchOutcome::Value(nested),
                    },
                    completed_branch("peer", 2),
                ],
                DEFAULT_MAX_OUTPUT_ITEMS,
            )
            .unwrap_err()
            .code,
            "LSV2403"
        );
    }

    #[test]
    fn legacy_query_effect_request_decodes_to_current_operation() {
        let mut vm = Vm::default();
        let request = start(&mut vm, None);
        let EffectOperation::Query(query) = request.operation.clone() else {
            panic!("expected query operation");
        };
        let legacy = serde_json::json!({
            "effect_id": request.effect_id,
            "required_capability": request.required_capability,
            "query": query,
            "continuation": request.continuation,
            "budget": request.budget,
        });
        let decoded: EffectRequest = serde_json::from_value(legacy).unwrap();
        assert_eq!(decoded, request);
    }

    #[test]
    fn mutating_effect_cannot_bypass_leased_command_envelope() {
        let mut vm = Vm::default();
        let request = start_refresh(&mut vm, Revision(1));
        let result = CommandResult {
            command_id: CommandId::new("leselang-command-1").unwrap(),
            status: CommandStatus::Applied,
            runtime: InMemoryControlPlane::default().register_runtime(
                RuntimeId::new("runtime-a").unwrap(),
                "Alpha",
                "http://runtime-a",
            ),
            events: Vec::new(),
        };
        let step = vm.resume(
            &request.continuation,
            EffectResult::Command(Box::new(result)),
        );
        assert!(matches!(step, Step::Fault(Fault { ref code, .. }) if code == "LSV2110"));
    }

    #[test]
    fn capabilities_refresh_reenters_only_through_dispatch_acknowledgement() {
        let mut vm = Vm::default();
        start_capabilities_refresh(&mut vm, Revision(1));
        let lease = vm.claim_effect(1_000, 50).unwrap().unwrap();
        let EffectOperation::Command(command) = lease.request.operation.clone() else {
            panic!("expected command operation");
        };
        assert!(matches!(
            command.command,
            Command::RuntimeCapabilitiesRefresh { .. }
        ));

        let mut control = InMemoryControlPlane::default();
        control.register_runtime(
            RuntimeId::new("runtime-a").unwrap(),
            "Alpha",
            "http://runtime-a",
        );
        let result = control.execute(command).unwrap();
        let completed = vm.acknowledge_effect(
            &lease,
            1_001,
            EffectResult::Command(Box::new(result.clone())),
        );
        assert!(matches!(
            completed,
            Step::Done(Value::RuntimeCapabilitiesRefresh { result: ref value })
                if **value == result
        ));
    }

    #[test]
    fn deployment_is_confirmed_and_reenters_only_through_dispatch_acknowledgement() {
        let mut vm = Vm::default();
        start_deploy(&mut vm, Revision(1));
        let lease = vm.claim_effect(1_000, 50).unwrap().unwrap();
        let EffectOperation::Command(command) = lease.request.operation.clone() else {
            panic!("expected deployment command");
        };
        assert_eq!(command.confirmation, Confirmation::Confirmed);
        assert!(matches!(
            command.command,
            Command::RuntimeDeploy {
                ref pipeline_kind,
                target: Some(ref target),
                ..
            } if pipeline_kind == "http/request" && target == "pid:42"
        ));

        let mut control = InMemoryControlPlane::default();
        control.register_runtime(
            RuntimeId::new("runtime-a").unwrap(),
            "Alpha",
            "http://runtime-a",
        );
        let result = control.execute(command).unwrap();
        let completed = vm.acknowledge_effect(
            &lease,
            1_001,
            EffectResult::Command(Box::new(result.clone())),
        );
        assert!(matches!(
            completed,
            Step::Done(Value::RuntimeDeploy { result: ref value }) if **value == result
        ));
    }

    #[test]
    fn completion_consumes_once_and_duplicate_delivery_is_idempotent() {
        let mut vm = Vm::default();
        let request = start(&mut vm, Some(Revision(7)));
        let result = runtime_list_result(7);
        let first = vm.resume(&request.continuation, result.clone());
        let duplicate = vm.resume(&request.continuation, result);
        assert_eq!(first, duplicate);
        assert!(matches!(first, Step::Done(Value::RuntimeList { .. })));
        assert_eq!(vm.pending_count(), 0);
    }

    #[test]
    fn ephemeral_compaction_is_bounded_and_orders_numeric_continuations() {
        let mut vm = Vm::default();
        let mut requests = Vec::new();
        for revision in 1..=11 {
            let request = start(&mut vm, None);
            assert!(matches!(
                vm.resume(&request.continuation, runtime_list_result(revision)),
                Step::Done(_)
            ));
            requests.push(request);
        }

        let report = vm
            .compact_journal(&RetentionPolicy {
                max_completed_records: 2,
                max_delete_per_run: 3,
            })
            .unwrap();
        assert_eq!(report.removed_records, 3);
        assert_eq!(report.remaining_completed, 8);
        assert!(report.reclaimed_logical_bytes > 0);
        assert_eq!(vm.completed_count(), 8);
        for request in requests.iter().take(3) {
            assert!(matches!(
                vm.resume(&request.continuation, runtime_list_result(99)),
                Step::Fault(Fault { ref code, .. }) if code == "LSV2004"
            ));
        }
        assert!(matches!(
            vm.resume(&requests[9].continuation, runtime_list_result(99)),
            Step::Done(Value::RuntimeList {
                revision: Revision(10),
                ..
            })
        ));
    }

    #[test]
    fn retention_policy_is_bounded() {
        let mut vm = Vm::default();
        for policy in [
            RetentionPolicy {
                max_completed_records: 0,
                max_delete_per_run: 1,
            },
            RetentionPolicy {
                max_completed_records: 1,
                max_delete_per_run: MAX_COMPACTION_BATCH + 1,
            },
        ] {
            assert_eq!(vm.compact_journal(&policy).unwrap_err().code, "LSV2301");
        }
    }

    #[test]
    fn continuation_survives_serialization_and_process_restart() {
        let mut first_vm = Vm::default();
        let request = start(&mut first_vm, None);
        let encoded = encode_continuation(&request.continuation).unwrap();
        let restored_image = decode_continuation(&encoded).unwrap();

        let mut restarted_vm = Vm::default();
        restarted_vm.restore(restored_image.clone()).unwrap();
        let step = restarted_vm.resume(&restored_image, runtime_list_result(11));
        assert!(matches!(
            step,
            Step::Done(Value::RuntimeList {
                revision: Revision(11),
                ..
            })
        ));
    }

    #[test]
    fn revision_conflict_is_typed_and_idempotently_replayed() {
        let mut vm = Vm::default();
        let request = start(&mut vm, Some(Revision(3)));
        let result = runtime_list_result(4);
        let first = vm.resume(&request.continuation, result.clone());
        let duplicate = vm.resume(&request.continuation, result);
        assert_eq!(first, duplicate);
        assert!(matches!(first, Step::Fault(Fault { ref code, .. }) if code == "LSV2101"));
    }

    #[test]
    fn missing_capability_and_invalid_images_fail_without_effects() {
        let mut vm = Vm::default();
        let step = vm.start(
            &program(),
            Principal {
                id: "operator".to_string(),
            },
            CapabilitySet::default(),
            None,
        );
        assert!(matches!(step, Step::Fault(Fault { ref code, .. }) if code == "LSH2001"));
        assert_eq!(vm.pending_count(), 0);

        let mut image = start(&mut vm, None).continuation;
        image.schema_version = 99;
        assert_eq!(encode_continuation(&image).unwrap_err().code, "LSV2001");
        assert_eq!(
            decode_continuation(&vec![b' '; MAX_CONTINUATION_BYTES + 1])
                .unwrap_err()
                .code,
            "LSV3002"
        );

        let mut forged_budget = start(&mut vm, None).continuation;
        forged_budget.deadline_ms = 0;
        assert_eq!(
            encode_continuation(&forged_budget).unwrap_err().code,
            "LSV2008"
        );
        forged_budget.deadline_ms = DEFAULT_EFFECT_DEADLINE_MS;
        forged_budget.max_output_items = DEFAULT_MAX_OUTPUT_ITEMS + 1;
        assert_eq!(
            encode_continuation(&forged_budget).unwrap_err().code,
            "LSV2009"
        );
    }

    #[test]
    fn source_to_hir_to_effect_to_domain_to_value_is_one_vertical_slice() {
        let mut vm = Vm::default();
        let unfiltered = lower(&parse("fn main() = runtime.list()")).unwrap();
        let Step::Effect(request) = vm.start(
            &unfiltered,
            Principal {
                id: "operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
            None,
        ) else {
            panic!("expected effect");
        };
        let request = *request;
        let mut control = InMemoryControlPlane::default();
        control.register_runtime(RuntimeId::new("runtime-b").unwrap(), "Bravo", "http://b");
        control.register_runtime(RuntimeId::new("runtime-a").unwrap(), "Alpha", "http://a");

        let EffectOperation::Query(query) = request.operation else {
            panic!("expected query operation");
        };
        let result = control.query(query).unwrap();
        let step = vm.resume(&request.continuation, EffectResult::Query(result));
        let Step::Done(Value::RuntimeList { runtimes, .. }) = step else {
            panic!("vertical slice should complete");
        };
        assert_eq!(runtimes[0].id.as_str(), "runtime-a");
        assert_eq!(runtimes[1].id.as_str(), "runtime-b");
    }

    #[test]
    fn restored_tokens_do_not_collide_with_new_effects() {
        let mut first = Vm::default();
        let restored = start(&mut first, None).continuation;
        let mut restarted = Vm::default();
        restarted.restore(restored).unwrap();
        let next = start(&mut restarted, None);
        assert_eq!(next.continuation.token.as_str(), "continuation-2");
    }

    #[test]
    fn output_and_serialized_continuation_limits_are_enforced() {
        let mut first = Vm::default();
        let mut image = start(&mut first, None).continuation;
        image.max_output_items = 0;
        let mut restarted = Vm::default();
        restarted.restore(image.clone()).unwrap();
        let mut control = InMemoryControlPlane::default();
        control.register_runtime(RuntimeId::new("runtime-a").unwrap(), "Alpha", "http://a");
        let result = control
            .query(QueryEnvelope {
                schema_version: DOMAIN_SCHEMA_VERSION,
                principal: Principal {
                    id: "operator".to_string(),
                },
                capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
                query: Query::RuntimeList {
                    filter: RuntimeListFilter::default(),
                },
            })
            .unwrap();
        assert!(matches!(
            restarted.resume(&image, EffectResult::Query(result)),
            Step::Fault(Fault { ref code, .. }) if code == "LSV2102"
        ));

        let large_value = "x".repeat(MAX_CONTINUATION_BYTES);
        let source = format!("fn main() = runtime.list(environment: \"{large_value}\")");
        let large_program = lower(&parse(&source)).unwrap();
        let step = Vm::default().start(
            &large_program,
            Principal {
                id: "operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
            None,
        );
        assert!(matches!(step, Step::Fault(Fault { ref code, .. }) if code == "LSV3002"));
    }

    #[test]
    fn durable_journal_recovers_pending_and_replays_completion_after_restart() {
        let journal = TempJournal::new("restart");
        let request = {
            let mut first = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
            let request = start(&mut first, None);
            assert_eq!(first.pending_count(), 1);
            request
        };

        let first_step = {
            let mut restarted = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
            assert_eq!(restarted.pending_count(), 1);
            assert_eq!(
                restarted.pending_continuations(),
                vec![request.continuation.clone()]
            );
            restarted.resume(&request.continuation, runtime_list_result(21))
        };
        assert!(matches!(
            first_step,
            Step::Done(Value::RuntimeList {
                revision: Revision(21),
                ..
            })
        ));

        let mut restarted_again = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        assert_eq!(restarted_again.pending_count(), 0);
        assert_eq!(restarted_again.completed_count(), 1);
        let replay = restarted_again.resume(&request.continuation, runtime_list_result(99));
        assert_eq!(replay, first_step);
    }

    #[test]
    fn durable_debugger_audit_survives_restart_and_enforces_idempotency() {
        let journal = TempJournal::new("debugger-audit-restart");
        let request;
        let audit = DebuggerAuditContext {
            command_id: CommandId::new("debugger-command-a").unwrap(),
            idempotency_key: IdempotencyKey::new("debugger-idempotency-a").unwrap(),
            principal: Principal {
                id: "debugger-operator".to_string(),
            },
            origin: CommandOrigin::Gui,
            session_id: "session-a".to_string(),
            expected_revision: Revision(7),
        };
        let first_record = {
            let mut vm = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
            request = start(&mut vm, Some(Revision(7)));
            vm.cancel_effect_audited(&request.continuation, 1_250, &audit)
                .unwrap()
        };
        assert_eq!(first_record.observed_at_ms, 1_250);

        let mut restarted = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        assert_eq!(
            restarted.debugger_audit(&audit.command_id).unwrap(),
            Some(first_record.clone())
        );
        assert_eq!(
            restarted
                .cancel_effect_audited(&request.continuation, 1_500, &audit)
                .unwrap(),
            first_record
        );

        let conflicting = DebuggerAuditContext {
            command_id: CommandId::new("debugger-command-b").unwrap(),
            ..audit.clone()
        };
        let error = restarted
            .cancel_effect_audited(&request.continuation, 1_750, &conflicting)
            .unwrap_err();
        assert_eq!(error.code, "LSV4033");

        let encoded = serde_json::to_string(&first_record).unwrap();
        assert!(!encoded.contains(request.continuation.token.as_str()));
        assert!(!encoded.contains(audit.idempotency_key.as_str()));
    }

    #[test]
    fn durable_journal_rejects_orphaned_debugger_audit() {
        let journal = TempJournal::new("debugger-audit-orphan");
        drop(Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap());
        let connection = rusqlite::Connection::open(journal.path()).unwrap();
        connection
            .pragma_update(None, "foreign_keys", false)
            .unwrap();
        connection
            .execute(
                "INSERT INTO vm_debugger_audit(
                    command_id, principal_id, idempotency_key, origin, session_id,
                    expected_revision, observed_at_ms, continuation_token
                 ) VALUES ('debugger-command-a', 'debugger-operator',
                           'debugger-idempotency-a', 'gui', 'session-a', 7, 1250,
                           'missing-continuation')",
                [],
            )
            .unwrap();
        drop(connection);

        let error = Vm::open_journal(journal.path(), DEFAULT_FUEL)
            .err()
            .unwrap();
        assert_eq!(error.code, "LSV4007");
    }

    #[test]
    fn durable_journal_replays_structured_merge_output() {
        let journal = TempJournal::new("structured-merge");
        let request;
        let merged = merge_declared(
            &MergePlan::new(["inventory", "health"]).unwrap(),
            vec![
                completed_branch("health", 2),
                completed_branch("inventory", 1),
            ],
            DEFAULT_MAX_OUTPUT_ITEMS,
        )
        .unwrap();
        {
            let mut vm = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
            request = start(&mut vm, None);
            vm.journal
                .record_completed(&request.continuation, &merged)
                .unwrap();
        }

        let mut restarted = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        assert_eq!(restarted.completed_count(), 1);
        assert_eq!(
            restarted.resume(&request.continuation, runtime_list_result(99)),
            merged
        );
    }

    #[test]
    fn durable_compaction_deletes_only_oldest_completed_records() {
        let journal = TempJournal::new("compaction");
        let pending;
        {
            let mut vm = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
            let mut completed = Vec::new();
            for revision in 1..=3 {
                let request = start(&mut vm, None);
                assert!(matches!(
                    vm.resume(&request.continuation, runtime_list_result(revision)),
                    Step::Done(_)
                ));
                completed.push(request);
            }
            pending = start(&mut vm, None);

            let report = vm
                .compact_journal(&RetentionPolicy {
                    max_completed_records: 1,
                    max_delete_per_run: 1,
                })
                .unwrap();
            assert_eq!(report.removed_records, 1);
            assert_eq!(report.remaining_completed, 2);
            assert!(report.reclaimed_logical_bytes > 0);
            assert_eq!(vm.pending_count(), 1);
            assert_eq!(vm.completed_count(), 2);
            assert!(matches!(
                vm.resume(&completed[0].continuation, runtime_list_result(99)),
                Step::Fault(Fault { ref code, .. }) if code == "LSV2004"
            ));
        }

        let connection = rusqlite::Connection::open(journal.path()).unwrap();
        let counts: (i64, i64, i64) = connection
            .query_row(
                "SELECT
                   (SELECT COUNT(*) FROM vm_effects WHERE state = 'pending'),
                   (SELECT COUNT(*) FROM vm_effects WHERE state = 'completed'),
                   (SELECT COUNT(*) FROM vm_dispatches)",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(counts, (1, 2, 3));
        drop(connection);

        let mut restarted = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        assert_eq!(restarted.pending_count(), 1);
        assert_eq!(restarted.completed_count(), 2);
        let report = restarted
            .compact_journal(&RetentionPolicy {
                max_completed_records: 1,
                max_delete_per_run: 10,
            })
            .unwrap();
        assert_eq!(report.removed_records, 1);
        assert_eq!(report.remaining_completed, 1);
        assert!(
            restarted
                .pending_continuations()
                .contains(&pending.continuation)
        );

        let connection = rusqlite::Connection::open(journal.path()).unwrap();
        let counts: (i64, i64, i64) = connection
            .query_row(
                "SELECT
                   (SELECT COUNT(*) FROM vm_effects WHERE state = 'pending'),
                   (SELECT COUNT(*) FROM vm_effects WHERE state = 'completed'),
                   (SELECT COUNT(*) FROM vm_dispatches)",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(counts, (1, 1, 2));
    }

    #[test]
    fn durable_compaction_invalidates_stale_live_vm_replay() {
        let journal = TempJournal::new("compaction-live-vms");
        let oldest;
        {
            let mut creator = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
            oldest = start(&mut creator, None);
            creator.resume(&oldest.continuation, runtime_list_result(1));
            let newest = start(&mut creator, None);
            creator.resume(&newest.continuation, runtime_list_result(2));
        }

        let mut stale = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        let mut compactor = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        let policy = RetentionPolicy {
            max_completed_records: 1,
            max_delete_per_run: 1,
        };
        assert_eq!(
            compactor.compact_journal(&policy).unwrap().removed_records,
            1
        );
        assert_eq!(stale.completed_count(), 2);
        assert!(matches!(
            stale.resume(&oldest.continuation, runtime_list_result(99)),
            Step::Fault(Fault { ref code, .. }) if code == "LSV2004"
        ));
        assert_eq!(stale.completed_count(), 1);

        let reconciled = stale.compact_journal(&policy).unwrap();
        assert_eq!(reconciled.removed_records, 0);
        assert_eq!(reconciled.remaining_completed, 1);
    }

    #[test]
    fn durable_sequence_allocation_does_not_collide_between_live_vms() {
        let journal = TempJournal::new("sequences");
        let mut first = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        let mut second = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();

        let first_request = start(&mut first, None);
        let second_request = start(&mut second, None);

        assert_eq!(first_request.continuation.token.as_str(), "continuation-1");
        assert_eq!(second_request.continuation.token.as_str(), "continuation-2");
        assert_ne!(first_request.effect_id, second_request.effect_id);
    }

    #[test]
    fn concurrent_completion_uses_the_first_durable_result() {
        let journal = TempJournal::new("completion-race");
        let mut first = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        let request = start(&mut first, None);
        let mut second = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();

        let first_step = first.resume(&request.continuation, runtime_list_result(31));
        let competing_step = second.resume(&request.continuation, runtime_list_result(32));

        assert_eq!(competing_step, first_step);
        assert!(matches!(
            competing_step,
            Step::Done(Value::RuntimeList {
                revision: Revision(31),
                ..
            })
        ));
    }

    #[test]
    fn dispatch_lease_is_fenced_and_acknowledged_in_ephemeral_mode() {
        let mut vm = Vm::default();
        let request = start(&mut vm, None);
        let lease = vm
            .claim_effect(1_000, DEFAULT_DISPATCH_LEASE_MS)
            .unwrap()
            .unwrap();
        assert_eq!(lease.request, request);
        assert_eq!(lease.attempt, 1);
        assert!(
            vm.claim_effect(1_001, DEFAULT_DISPATCH_LEASE_MS)
                .unwrap()
                .is_none()
        );

        let direct = vm.resume(&request.continuation, runtime_list_result(1));
        assert!(matches!(direct, Step::Fault(Fault { ref code, .. }) if code == "LSV4024"));

        let completed = vm.acknowledge_effect(&lease, 1_001, runtime_list_result(2));
        assert!(matches!(
            completed,
            Step::Done(Value::RuntimeList {
                revision: Revision(2),
                ..
            })
        ));
        assert_eq!(vm.pending_count(), 0);
    }

    #[test]
    fn timed_effect_requires_trusted_clock_and_completes_before_deadline() {
        let mut vm = Vm::default();
        let request = start_timed(&mut vm, 1_000, 50);
        assert_eq!(request.continuation.deadline_ms, 50);
        assert_eq!(request.continuation.deadline_at_ms, Some(1_050));
        assert_eq!(request.budget.deadline_at_ms, Some(1_050));

        let untrusted = vm.resume(&request.continuation, runtime_list_result(1));
        assert!(matches!(untrusted, Step::Fault(Fault { ref code, .. }) if code == "LSV2111"));
        let completed = vm.resume_at(&request.continuation, 1_049, runtime_list_result(2));
        assert!(matches!(
            completed,
            Step::Done(Value::RuntimeList {
                revision: Revision(2),
                ..
            })
        ));
    }

    #[test]
    fn timed_effect_rejects_invalid_scheduler_budgets() {
        let mut vm = Vm::default();
        let principal = Principal {
            id: "operator".to_string(),
        };
        let capabilities = CapabilitySet::new([CAPABILITY_RUNTIME_READ]);
        let zero = vm.start_timed(
            &program(),
            principal.clone(),
            capabilities.clone(),
            None,
            1_000,
            0,
        );
        assert!(matches!(zero, Step::Fault(Fault { ref code, .. }) if code == "LSV2012"));
        let overflow = vm.start_timed(
            &program(),
            principal,
            capabilities,
            None,
            i64::MAX as u64,
            1,
        );
        assert!(matches!(overflow, Step::Fault(Fault { ref code, .. }) if code == "LSV2011"));
        assert_eq!(vm.pending_count(), 0);
    }

    #[test]
    fn trusted_deadline_atomically_cancels_ready_effect() {
        let mut vm = Vm::default();
        let request = start_timed(&mut vm, 1_000, 50);

        assert!(vm.claim_effect(1_050, 100).unwrap().is_none());
        assert_eq!(vm.pending_count(), 0);
        assert_eq!(vm.completed_count(), 1);
        let replay = vm.cancel_effect(&request.continuation, 1_051);
        assert!(matches!(
            replay,
            Step::Cancelled(Cancellation {
                reason: CancellationReason::DeadlineExceeded,
                observed_at_ms: 1_050,
                ..
            })
        ));
    }

    #[test]
    fn deadline_precedes_requested_cancellation_at_same_clock_tick() {
        let mut vm = Vm::default();
        let request = start_timed(&mut vm, 1_000, 50);

        let cancelled = vm.cancel_effect(&request.continuation, 1_050);
        assert!(matches!(
            cancelled,
            Step::Cancelled(Cancellation {
                reason: CancellationReason::DeadlineExceeded,
                observed_at_ms: 1_050,
                ..
            })
        ));
    }

    #[test]
    fn requested_cancellation_fences_active_lease() {
        let mut vm = Vm::default();
        let request = start_timed(&mut vm, 1_000, 100);
        let lease = vm.claim_effect(1_001, 90).unwrap().unwrap();

        let cancelled = vm.cancel_effect(&request.continuation, 1_010);
        assert!(matches!(
            cancelled,
            Step::Cancelled(Cancellation {
                reason: CancellationReason::Requested,
                observed_at_ms: 1_010,
                ..
            })
        ));
        assert_eq!(
            vm.acknowledge_effect(&lease, 1_011, runtime_list_result(3)),
            cancelled
        );
        assert!(vm.claim_effect(1_200, 50).unwrap().is_none());
    }

    #[test]
    fn durable_timeout_survives_restart_and_fences_late_worker() {
        let journal = TempJournal::new("timeout-restart");
        let (request, lease) = {
            let mut first = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
            let request = start_timed(&mut first, 1_000, 50);
            let lease = first.claim_effect(1_001, 100).unwrap().unwrap();
            (request, lease)
        };

        let timeout = {
            let mut restarted = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
            assert!(restarted.claim_effect(1_050, 100).unwrap().is_none());
            assert_eq!(restarted.pending_count(), 0);
            restarted.cancel_effect(&request.continuation, 1_051)
        };
        assert!(matches!(
            timeout,
            Step::Cancelled(Cancellation {
                reason: CancellationReason::DeadlineExceeded,
                observed_at_ms: 1_050,
                ..
            })
        ));

        let mut late_worker = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        assert_eq!(
            late_worker.acknowledge_effect(&lease, 1_051, runtime_list_result(4)),
            timeout
        );
        assert!(late_worker.claim_effect(2_000, 100).unwrap().is_none());
    }

    #[test]
    fn semantic_retry_waits_for_deterministic_backoff_and_exhausts_typed() {
        let mut vm = Vm::default();
        start_timed(&mut vm, 1_000, 10_000);
        let first = vm.claim_effect(1_000, 100).unwrap().unwrap();
        let policy = RetryPolicy {
            max_retries: 1,
            base_delay_ms: 250,
            max_delay_ms: 1_000,
        };

        let scheduled = vm
            .report_effect_error(&first, 1_001, transient_error(), &policy)
            .unwrap();
        assert_eq!(
            scheduled,
            RetryDisposition::Scheduled(RetrySchedule {
                retry_count: 1,
                ready_at_ms: 1_251,
                error: transient_error(),
            })
        );
        assert!(vm.claim_effect(1_250, 100).unwrap().is_none());
        let second = vm.claim_effect(1_251, 100).unwrap().unwrap();
        assert_eq!(second.attempt, 2);
        assert_eq!(second.retry_count, 1);

        let exhausted = vm
            .report_effect_error(&second, 1_252, transient_error(), &policy)
            .unwrap();
        assert!(matches!(
            exhausted,
            RetryDisposition::Terminal(Step::Failed(FailedEffect { retry_count: 1, .. }))
        ));
        assert_eq!(vm.pending_count(), 0);
        assert_eq!(vm.completed_count(), 1);
        assert!(vm.claim_effect(2_000, 100).unwrap().is_none());
    }

    #[test]
    fn permanent_effect_error_fails_without_retry() {
        let mut vm = Vm::default();
        start(&mut vm, None);
        let lease = vm.claim_effect(1_000, 100).unwrap().unwrap();
        let error = EffectError {
            class: EffectErrorClass::Permanent,
            code: "request.rejected".to_string(),
            message: "upstream rejected the request".to_string(),
        };
        let failed = vm
            .report_effect_error(&lease, 1_001, error.clone(), &RetryPolicy::default())
            .unwrap();
        assert_eq!(
            failed,
            RetryDisposition::Terminal(Step::Failed(FailedEffect {
                error,
                retry_count: 0,
            }))
        );
    }

    #[test]
    fn retry_schedule_fences_old_lease_and_deadline_still_wins() {
        let mut vm = Vm::default();
        start_timed(&mut vm, 1_000, 100);
        let lease = vm.claim_effect(1_001, 90).unwrap().unwrap();
        let policy = RetryPolicy {
            max_retries: 2,
            base_delay_ms: 250,
            max_delay_ms: 250,
        };
        vm.report_effect_error(&lease, 1_002, transient_error(), &policy)
            .unwrap();
        let stale = vm.report_effect_error(&lease, 1_003, transient_error(), &policy);
        assert!(matches!(stale, Err(Fault { ref code, .. }) if code == "LSV4022"));

        assert!(vm.claim_effect(1_100, 100).unwrap().is_none());
        let terminal = vm.cancel_effect(&lease.request.continuation, 1_101);
        assert!(matches!(
            terminal,
            Step::Cancelled(Cancellation {
                reason: CancellationReason::DeadlineExceeded,
                observed_at_ms: 1_100,
                ..
            })
        ));
    }

    #[test]
    fn deadline_precedes_invalid_effect_error_report() {
        let mut vm = Vm::default();
        start_timed(&mut vm, 1_000, 50);
        let lease = vm.claim_effect(1_001, 100).unwrap().unwrap();
        let invalid = EffectError {
            class: EffectErrorClass::Transient,
            code: "bad code".to_string(),
            message: String::new(),
        };

        let disposition = vm
            .report_effect_error(&lease, 1_050, invalid, &RetryPolicy::default())
            .unwrap();
        assert!(matches!(
            disposition,
            RetryDisposition::Terminal(Step::Cancelled(Cancellation {
                reason: CancellationReason::DeadlineExceeded,
                observed_at_ms: 1_050,
                ..
            }))
        ));
    }

    #[test]
    fn durable_retry_schedule_survives_restart() {
        let journal = TempJournal::new("retry-restart");
        let policy = RetryPolicy {
            max_retries: 2,
            base_delay_ms: 50,
            max_delay_ms: 100,
        };
        {
            let mut first = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
            start(&mut first, None);
            let lease = first.claim_effect(1_000, 100).unwrap().unwrap();
            first
                .report_effect_error(&lease, 1_001, transient_error(), &policy)
                .unwrap();
        }
        let connection = rusqlite::Connection::open(journal.path()).unwrap();
        let persisted_error: Vec<u8> = connection
            .query_row("SELECT last_error FROM vm_dispatches", [], |row| row.get(0))
            .unwrap();
        assert_eq!(
            serde_json::from_slice::<EffectError>(&persisted_error).unwrap(),
            transient_error()
        );
        drop(connection);

        let mut restarted = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        assert!(restarted.claim_effect(1_050, 100).unwrap().is_none());
        let lease = restarted.claim_effect(1_051, 100).unwrap().unwrap();
        assert_eq!(lease.retry_count, 1);
        assert_eq!(lease.attempt, 2);
        let completed = restarted.acknowledge_effect(&lease, 1_052, runtime_list_result(8));
        assert!(matches!(
            completed,
            Step::Done(Value::RuntimeList {
                revision: Revision(8),
                ..
            })
        ));
    }

    #[test]
    fn mutating_semantic_retry_preserves_command_envelope() {
        let journal = TempJournal::new("mutation-semantic-retry");
        let first_lease = {
            let mut first = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
            start_refresh(&mut first, Revision(1));
            let lease = first.claim_effect(1_000, 100).unwrap().unwrap();
            first
                .report_effect_error(
                    &lease,
                    1_001,
                    transient_error(),
                    &RetryPolicy {
                        max_retries: 1,
                        base_delay_ms: 50,
                        max_delay_ms: 50,
                    },
                )
                .unwrap();
            lease
        };

        let mut restarted = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        let second_lease = restarted.claim_effect(1_051, 100).unwrap().unwrap();
        assert_eq!(second_lease.request, first_lease.request);
        assert_eq!(second_lease.retry_count, 1);
        let EffectOperation::Command(command) = &second_lease.request.operation else {
            panic!("expected command operation");
        };
        assert_eq!(command.idempotency_key.as_str(), "leselang-effect-1");
        assert_eq!(
            command.command_id,
            CommandId::new("leselang-command-1").unwrap()
        );
    }

    #[test]
    fn retry_policy_and_error_metadata_are_bounded() {
        let mut vm = Vm::default();
        start(&mut vm, None);
        let lease = vm.claim_effect(1_000, 100).unwrap().unwrap();
        let invalid_policy = RetryPolicy {
            max_retries: MAX_SEMANTIC_RETRIES + 1,
            ..RetryPolicy::default()
        };
        assert_eq!(
            vm.report_effect_error(&lease, 1_001, transient_error(), &invalid_policy)
                .unwrap_err()
                .code,
            "LSV2201"
        );
        let invalid_error = EffectError {
            class: EffectErrorClass::Transient,
            code: "bad code".to_string(),
            message: "invalid code shape".to_string(),
        };
        assert_eq!(
            vm.report_effect_error(&lease, 1_001, invalid_error, &RetryPolicy::default())
                .unwrap_err()
                .code,
            "LSV2202"
        );
    }

    #[test]
    fn durable_dispatch_reclaims_expired_lease_and_rejects_stale_worker() {
        let journal = TempJournal::new("dispatch-recovery");
        let mut first = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        let request = start(&mut first, None);
        let first_lease = first.claim_effect(1_000, 50).unwrap().unwrap();

        let mut restarted = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        assert!(restarted.claim_effect(1_049, 50).unwrap().is_none());
        let expired = first.acknowledge_effect(&first_lease, 1_051, runtime_list_result(40));
        assert!(matches!(expired, Step::Fault(Fault { ref code, .. }) if code == "LSV4023"));

        let second_lease = restarted.claim_effect(1_051, 50).unwrap().unwrap();
        assert_eq!(second_lease.request, request);
        assert_eq!(second_lease.attempt, 2);
        let stale = first.acknowledge_effect(&first_lease, 1_052, runtime_list_result(41));
        assert!(matches!(stale, Step::Fault(Fault { ref code, .. }) if code == "LSV4022"));

        let completed = restarted.acknowledge_effect(&second_lease, 1_052, runtime_list_result(42));
        assert!(matches!(
            completed,
            Step::Done(Value::RuntimeList {
                revision: Revision(42),
                ..
            })
        ));
        drop(restarted);

        let mut recovered = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        assert!(recovered.claim_effect(2_000, 50).unwrap().is_none());
        assert_eq!(recovered.completed_count(), 1);
    }

    #[test]
    fn durable_dispatch_has_single_claimant_across_live_vms() {
        let journal = TempJournal::new("dispatch-race");
        let mut first = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        start(&mut first, None);
        let mut second = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();

        assert!(first.claim_effect(10, 100).unwrap().is_some());
        assert!(second.claim_effect(10, 100).unwrap().is_none());
    }

    #[test]
    fn mutating_effect_redelivery_reuses_domain_idempotency_key() {
        let journal = TempJournal::new("mutation-redelivery");
        let mut control = InMemoryControlPlane::default();
        control.register_runtime(
            RuntimeId::new("runtime-a").unwrap(),
            "Alpha",
            "http://runtime-a",
        );

        let first_lease = {
            let mut first = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
            let request = start_refresh(&mut first, Revision(1));
            let EffectOperation::Command(command) = &request.operation else {
                panic!("expected command operation");
            };
            assert_eq!(command.idempotency_key.as_str(), "leselang-effect-1");
            first.claim_effect(1_000, 50).unwrap().unwrap()
        };
        let EffectOperation::Command(first_command) = first_lease.request.operation.clone() else {
            panic!("expected command operation");
        };
        let first_result = control.execute(first_command).unwrap();
        assert_eq!(first_result.runtime.refresh_count, 1);

        let mut restarted = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        let second_lease = restarted.claim_effect(1_051, 50).unwrap().unwrap();
        assert_eq!(second_lease.attempt, 2);
        assert_eq!(second_lease.request, first_lease.request);
        let EffectOperation::Command(second_command) = second_lease.request.operation.clone()
        else {
            panic!("expected command operation");
        };
        let replayed_result = control.execute(second_command).unwrap();
        assert_eq!(replayed_result, first_result);
        assert_eq!(replayed_result.runtime.refresh_count, 1);

        let completed = restarted.acknowledge_effect(
            &second_lease,
            1_052,
            EffectResult::Command(Box::new(replayed_result.clone())),
        );
        assert!(matches!(
            completed,
            Step::Done(Value::RuntimeRefresh { ref result }) if **result == replayed_result
        ));
        drop(restarted);

        let mut recovered = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        assert!(recovered.claim_effect(2_000, 50).unwrap().is_none());
        assert_eq!(recovered.completed_count(), 1);
    }

    #[test]
    fn journal_v1_migrates_without_forging_missing_dispatch_context() {
        let journal = TempJournal::new("v1-migration");
        let request = {
            let mut vm = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
            start(&mut vm, None)
        };
        let connection = rusqlite::Connection::open(journal.path()).unwrap();
        connection
            .execute_batch(
                "DROP TABLE vm_debugger_audit;
                 DROP TABLE vm_merge_branches;
                 DROP TABLE vm_merge_groups;
                 DROP TABLE vm_dispatches;
                 ALTER TABLE vm_effects RENAME TO vm_effects_v3;
                 CREATE TABLE vm_effects (
                   token TEXT PRIMARY KEY,
                   state TEXT NOT NULL CHECK (state IN ('pending', 'completed')),
                   image BLOB NOT NULL,
                   terminal_step BLOB,
                   CHECK ((state = 'pending' AND terminal_step IS NULL) OR
                          (state = 'completed' AND terminal_step IS NOT NULL))
                 ) STRICT;
                 INSERT INTO vm_effects(token, state, image, terminal_step)
                   SELECT token, state, image, terminal_step FROM vm_effects_v3;
                 DROP TABLE vm_effects_v3;
                 PRAGMA user_version = 1;",
            )
            .unwrap();
        drop(connection);

        let mut migrated = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        assert_eq!(migrated.pending_continuations(), vec![request.continuation]);
        assert!(migrated.claim_effect(10, 100).unwrap().is_none());
        let connection = rusqlite::Connection::open(journal.path()).unwrap();
        assert_eq!(
            connection
                .query_row("PRAGMA user_version", [], |row| row.get::<_, u32>(0))
                .unwrap(),
            JOURNAL_SCHEMA_VERSION
        );
    }

    #[test]
    fn journal_v2_migrates_absolute_deadline_column() {
        let journal = TempJournal::new("v2-migration");
        let connection = rusqlite::Connection::open(journal.path()).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE vm_metadata (
                   singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
                   next_sequence INTEGER NOT NULL CHECK (next_sequence >= 1)
                 ) STRICT;
                 INSERT INTO vm_metadata(singleton, next_sequence) VALUES (1, 1);
                 CREATE TABLE vm_effects (
                   token TEXT PRIMARY KEY,
                   state TEXT NOT NULL CHECK (state IN ('pending', 'completed')),
                   image BLOB NOT NULL,
                   terminal_step BLOB,
                   CHECK ((state = 'pending' AND terminal_step IS NULL) OR
                          (state = 'completed' AND terminal_step IS NOT NULL))
                 ) STRICT;
                 CREATE TABLE vm_dispatches (
                   token TEXT PRIMARY KEY REFERENCES vm_effects(token) ON DELETE CASCADE,
                   request BLOB NOT NULL,
                   state TEXT NOT NULL CHECK (state IN ('ready', 'leased', 'acknowledged')),
                   attempt INTEGER NOT NULL CHECK (attempt >= 0),
                   lease_expires_at_ms INTEGER,
                   CHECK ((state = 'ready' AND lease_expires_at_ms IS NULL) OR
                          (state = 'leased' AND lease_expires_at_ms IS NOT NULL) OR
                          (state = 'acknowledged' AND lease_expires_at_ms IS NULL))
                 ) STRICT;
                 PRAGMA user_version = 2;",
            )
            .unwrap();
        drop(connection);

        drop(Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap());
        let connection = rusqlite::Connection::open(journal.path()).unwrap();
        assert_eq!(
            connection
                .query_row("PRAGMA user_version", [], |row| row.get::<_, u32>(0))
                .unwrap(),
            JOURNAL_SCHEMA_VERSION
        );
        let deadline_columns: u32 = connection
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('vm_effects') WHERE name = 'deadline_at_ms'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(deadline_columns, 1);
        let retry_columns: u32 = connection
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('vm_dispatches')
                 WHERE name IN ('ready_at_ms', 'retry_count', 'last_error')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(retry_columns, 3);
    }

    #[test]
    fn journal_v3_migrates_semantic_retry_state() {
        let journal = TempJournal::new("v3-migration");
        let connection = rusqlite::Connection::open(journal.path()).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE vm_metadata (
                   singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
                   next_sequence INTEGER NOT NULL CHECK (next_sequence >= 1)
                 ) STRICT;
                 INSERT INTO vm_metadata(singleton, next_sequence) VALUES (1, 1);
                 CREATE TABLE vm_effects (
                   token TEXT PRIMARY KEY,
                   state TEXT NOT NULL CHECK (state IN ('pending', 'completed')),
                   image BLOB NOT NULL,
                   deadline_at_ms INTEGER CHECK (deadline_at_ms IS NULL OR deadline_at_ms >= 0),
                   terminal_step BLOB,
                   CHECK ((state = 'pending' AND terminal_step IS NULL) OR
                          (state = 'completed' AND terminal_step IS NOT NULL))
                 ) STRICT;
                 CREATE TABLE vm_dispatches (
                   token TEXT PRIMARY KEY REFERENCES vm_effects(token) ON DELETE CASCADE,
                   request BLOB NOT NULL,
                   state TEXT NOT NULL CHECK (state IN ('ready', 'leased', 'acknowledged')),
                   attempt INTEGER NOT NULL CHECK (attempt >= 0),
                   lease_expires_at_ms INTEGER,
                   CHECK ((state = 'ready' AND lease_expires_at_ms IS NULL) OR
                          (state = 'leased' AND lease_expires_at_ms IS NOT NULL) OR
                          (state = 'acknowledged' AND lease_expires_at_ms IS NULL))
                 ) STRICT;
                 CREATE INDEX vm_effect_deadline_idx ON vm_effects(state, deadline_at_ms);
                 PRAGMA user_version = 3;",
            )
            .unwrap();
        drop(connection);

        drop(Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap());
        let connection = rusqlite::Connection::open(journal.path()).unwrap();
        assert_eq!(
            connection
                .query_row("PRAGMA user_version", [], |row| row.get::<_, u32>(0))
                .unwrap(),
            JOURNAL_SCHEMA_VERSION
        );
        let retry_columns: u32 = connection
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('vm_dispatches')
                 WHERE name IN ('ready_at_ms', 'retry_count', 'last_error')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(retry_columns, 3);
    }

    #[test]
    fn journal_v4_migrates_merge_graph_schema_without_losing_effects() {
        let journal = TempJournal::new("v4-migration");
        let request = {
            let mut vm = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
            start(&mut vm, None)
        };
        let connection = rusqlite::Connection::open(journal.path()).unwrap();
        connection
            .execute_batch(
                "DROP TABLE vm_debugger_audit;
                 DROP TABLE vm_merge_branches;
                 DROP TABLE vm_merge_groups;
                 PRAGMA user_version = 4;",
            )
            .unwrap();
        drop(connection);

        let migrated = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        assert_eq!(migrated.pending_continuations(), vec![request.continuation]);
        let connection = rusqlite::Connection::open(journal.path()).unwrap();
        assert_eq!(
            connection
                .query_row("PRAGMA user_version", [], |row| row.get::<_, u32>(0))
                .unwrap(),
            JOURNAL_SCHEMA_VERSION
        );
        let merge_tables: u32 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'table' AND name IN ('vm_merge_groups', 'vm_merge_branches')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(merge_tables, 2);
    }

    #[test]
    fn journal_v4_rejects_preexisting_merge_table_names() {
        let journal = TempJournal::new("v4-merge-table-conflict");
        {
            let _vm = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        }
        let connection = rusqlite::Connection::open(journal.path()).unwrap();
        connection
            .execute_batch(
                "DROP TABLE vm_debugger_audit;
                 DROP TABLE vm_merge_branches;
                 DROP TABLE vm_merge_groups;
                 CREATE TABLE vm_merge_groups (untrusted BLOB);
                 PRAGMA user_version = 4;",
            )
            .unwrap();
        drop(connection);

        let error = Vm::open_journal(journal.path(), DEFAULT_FUEL)
            .err()
            .unwrap();
        assert_eq!(error.code, "LSV4003");
    }

    #[test]
    fn durable_journal_rejects_merge_graph_order_tampering() {
        let journal = TempJournal::new("merge-order-tamper");
        let (left, right) = {
            let mut vm = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
            (start(&mut vm, None), start(&mut vm, None))
        };
        let plan = MergePlan::new(["left", "right"]).unwrap();
        let connection = rusqlite::Connection::open(journal.path()).unwrap();
        connection
            .execute(
                "INSERT INTO vm_merge_groups(token, plan, state, terminal_step)
                 VALUES (?1, ?2, 'pending', NULL)",
                rusqlite::params!["merge-1", serde_json::to_vec(&plan).unwrap()],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO vm_merge_branches(group_token, branch_token, branch_name, position)
                 VALUES (?1, ?2, 'right', 0), (?1, ?3, 'left', 1)",
                rusqlite::params![
                    "merge-1",
                    left.continuation.token.as_str(),
                    right.continuation.token.as_str()
                ],
            )
            .unwrap();
        drop(connection);

        let error = Vm::open_journal(journal.path(), DEFAULT_FUEL)
            .err()
            .unwrap();
        assert_eq!(error.code, "LSV4007");
    }

    #[test]
    fn durable_journal_rejects_merge_group_effect_token_collision() {
        let journal = TempJournal::new("merge-token-collision");
        let (left, right) = {
            let mut vm = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
            (start(&mut vm, None), start(&mut vm, None))
        };
        let plan = MergePlan::new(["left", "right"]).unwrap();
        let connection = rusqlite::Connection::open(journal.path()).unwrap();
        connection
            .execute(
                "INSERT INTO vm_merge_groups(token, plan, state, terminal_step)
                 VALUES (?1, ?2, 'pending', NULL)",
                rusqlite::params![
                    left.continuation.token.as_str(),
                    serde_json::to_vec(&plan).unwrap()
                ],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO vm_merge_branches(group_token, branch_token, branch_name, position)
                 VALUES (?1, ?2, 'left', 0), (?1, ?3, 'right', 1)",
                rusqlite::params![
                    left.continuation.token.as_str(),
                    left.continuation.token.as_str(),
                    right.continuation.token.as_str()
                ],
            )
            .unwrap();
        drop(connection);

        let error = Vm::open_journal(journal.path(), DEFAULT_FUEL)
            .err()
            .unwrap();
        assert_eq!(error.code, "LSV4007");
    }

    #[test]
    fn durable_journal_accepts_legacy_query_request_encoding() {
        let journal = TempJournal::new("legacy-request");
        {
            let mut vm = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
            start(&mut vm, None);
        }
        let connection = rusqlite::Connection::open(journal.path()).unwrap();
        let request: Vec<u8> = connection
            .query_row("SELECT request FROM vm_dispatches", [], |row| row.get(0))
            .unwrap();
        let mut value: serde_json::Value = serde_json::from_slice(&request).unwrap();
        let operation = value.as_object_mut().unwrap().remove("operation").unwrap();
        assert_eq!(operation["kind"], "query");
        value
            .as_object_mut()
            .unwrap()
            .insert("query".to_string(), operation["payload"].clone());
        connection
            .execute(
                "UPDATE vm_dispatches SET request = ?1",
                [serde_json::to_vec(&value).unwrap()],
            )
            .unwrap();
        drop(connection);

        let mut recovered = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        let lease = recovered.claim_effect(10, 100).unwrap().unwrap();
        let completed = recovered.acknowledge_effect(&lease, 11, runtime_list_result(1));
        assert!(matches!(completed, Step::Done(Value::RuntimeList { .. })));
    }

    #[test]
    fn durable_journal_rejects_unknown_schema_versions() {
        let journal = TempJournal::new("version");
        drop(Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap());
        let connection = rusqlite::Connection::open(journal.path()).unwrap();
        connection
            .execute_batch("PRAGMA user_version = 99;")
            .unwrap();
        drop(connection);

        let error = Vm::open_journal(journal.path(), DEFAULT_FUEL)
            .err()
            .unwrap();
        assert_eq!(error.code, "LSV4004");
    }

    #[cfg(unix)]
    #[test]
    fn durable_journal_is_private_and_rejects_symbolic_links() {
        use std::os::unix::fs::{MetadataExt, symlink};

        let journal = TempJournal::new("permissions");
        drop(Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap());
        assert_eq!(fs::metadata(journal.path()).unwrap().mode() & 0o777, 0o600);

        let link = journal.root.join("journal-link.sqlite3");
        symlink(journal.path(), &link).unwrap();
        let error = Vm::open_journal(&link, DEFAULT_FUEL).err().unwrap();
        assert_eq!(error.code, "LSV4001");
    }
}
