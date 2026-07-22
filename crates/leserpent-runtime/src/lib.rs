use leserpent_domain::bootstrap::{
    BootstrapError, BootstrapId, BootstrapPhase, DaemonSessionProof, DeploymentBootstrap,
    DeploymentBootstrapCheckpoint, DeploymentBootstrapSnapshot,
};
use leserpent_domain::{
    CommandPlan, CommandPlanError, CommandResult, CommandStatus, DOMAIN_SNAPSHOT_SCHEMA_VERSION,
    DomainError, DomainEvent, DomainSnapshot, DomainSnapshotError, InMemoryControlPlane,
    MAX_RUNTIME_LOG_MESSAGE_BYTES, MAX_RUNTIME_LOG_QUERY_ENTRIES, PlannedOperation, Query,
    QueryEnvelope, QueryResult, RUNTIME_CAPABILITY_DISCOVERY_EFFECT_KIND,
    RUNTIME_DEPLOYMENT_EFFECT_KIND, RUNTIME_STATUS_REFRESH_EFFECT_KIND, Revision,
    RuntimeCapabilityObservation, RuntimeCapabilityRefreshRequest, RuntimeDeploymentOutcome,
    RuntimeDeploymentRequest, RuntimeId, RuntimeLogLevel, RuntimeLogRecord, RuntimeProjection,
    RuntimeStatusObservation, RuntimeStatusRefreshRequest,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};
use std::fmt;
use std::path::Path;
use std::time::Duration;

mod persistence;

pub use persistence::{
    EffectLease, OrchestraDeleteRecord, OrchestraHistoryRecord, OrchestraPersistenceRecord,
};
use persistence::{EffectRecord, Journal, JournalEntryKind};

pub const EFFECT_QUEUE_CAPACITY: u64 = 10_000;
pub const MAX_EFFECT_ENQUEUE_BATCH: usize = 1_000;
pub const MAX_PERSISTED_RUNTIME_LOG_ENTRIES: usize = 4_096;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectEnqueue {
    pub effect_id: String,
    pub kind: String,
    pub payload: Vec<u8>,
    pub max_attempts: u32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EffectQueueStats {
    pub ready: u64,
    pub leased: u64,
    pub completed: u64,
    pub failed: u64,
    pub capacity: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeploymentEffectState {
    Pending,
    Completed,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeploymentEffectReceipt {
    pub state: DeploymentEffectState,
    pub attempt: u32,
    pub outcome: Option<RuntimeDeploymentOutcome>,
    pub error: Option<String>,
}

impl EffectQueueStats {
    pub fn active(self) -> u64 {
        self.ready.saturating_add(self.leased)
    }

    pub fn terminal(self) -> u64 {
        self.completed.saturating_add(self.failed)
    }

    pub fn total(self) -> u64 {
        self.active().saturating_add(self.terminal())
    }

    pub fn saturated(self) -> bool {
        self.active() >= self.capacity
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
pub enum PlanResult {
    Query(QueryResult),
    Command(CommandResult),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EffectExecution {
    Complete(Vec<u8>),
    Retry { error: String, after: Duration },
    Reject { error: String },
}

pub trait EffectExecutor {
    fn execute(&mut self, lease: &EffectLease) -> EffectExecution;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkerStep {
    Idle,
    Completed { effect_id: String, attempt: u32 },
    RetryScheduled { effect_id: String, attempt: u32 },
    Rejected { effect_id: String, attempt: u32 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeError {
    InvalidPlan(CommandPlanError),
    InvalidEffectOutcome(&'static str),
    Domain(DomainError),
    Bootstrap(BootstrapError),
    InvalidSnapshot(DomainSnapshotError),
    Storage(String),
    ReplayMismatch { sequence: i64 },
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPlan(error) => write!(formatter, "invalid command plan: {error}"),
            Self::InvalidEffectOutcome(reason) => {
                write!(formatter, "invalid effect outcome: {reason}")
            }
            Self::Domain(error) => write!(formatter, "domain execution failed: {error}"),
            Self::Bootstrap(error) => write!(formatter, "bootstrap state failed: {error}"),
            Self::InvalidSnapshot(error) => write!(formatter, "invalid runtime snapshot: {error}"),
            Self::Storage(error) => write!(formatter, "runtime storage failed: {error}"),
            Self::ReplayMismatch { sequence } => {
                write!(
                    formatter,
                    "runtime replay diverged at journal sequence {sequence}"
                )
            }
        }
    }
}

impl std::error::Error for RuntimeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidPlan(error) => Some(error),
            Self::Domain(error) => Some(error),
            Self::Bootstrap(error) => Some(error),
            Self::InvalidSnapshot(error) => Some(error),
            Self::InvalidEffectOutcome(_) | Self::Storage(_) | Self::ReplayMismatch { .. } => None,
        }
    }
}

pub struct ControlRuntime {
    control: InMemoryControlPlane,
    journal: Option<Journal>,
    ephemeral_logs: BTreeMap<RuntimeId, VecDeque<RuntimeLogRecord>>,
    next_ephemeral_log_sequence: u64,
}

impl Default for ControlRuntime {
    fn default() -> Self {
        Self {
            control: InMemoryControlPlane::default(),
            journal: None,
            ephemeral_logs: BTreeMap::new(),
            next_ephemeral_log_sequence: 1,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct RuntimeRegistration {
    runtime_id: String,
    name: String,
    endpoint: String,
}

impl ControlRuntime {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, RuntimeError> {
        let mut journal = Journal::open(path.as_ref()).map_err(RuntimeError::Storage)?;
        let snapshots = journal.load_snapshots().map_err(RuntimeError::Storage)?;
        let mut restored = None;
        let mut snapshot_error = None;
        for snapshot in snapshots {
            if snapshot.schema_version != DOMAIN_SNAPSHOT_SCHEMA_VERSION {
                snapshot_error = Some(format!(
                    "snapshot generation {} uses domain schema {}, expected {}",
                    snapshot.generation, snapshot.schema_version, DOMAIN_SNAPSHOT_SCHEMA_VERSION
                ));
                continue;
            }
            let state: DomainSnapshot = match serde_json::from_slice(&snapshot.payload) {
                Ok(state) => state,
                Err(error) => {
                    snapshot_error = Some(format!(
                        "snapshot generation {} is invalid JSON: {error}",
                        snapshot.generation
                    ));
                    continue;
                }
            };
            match InMemoryControlPlane::from_snapshot(state) {
                Ok(control) => {
                    restored = Some((control, snapshot.through_sequence));
                    break;
                }
                Err(error) => {
                    snapshot_error = Some(format!(
                        "snapshot generation {} failed domain validation: {error}",
                        snapshot.generation
                    ));
                }
            }
        }
        let (control, through_sequence) = match (restored, snapshot_error) {
            (Some(restored), _) => restored,
            (None, None) => (InMemoryControlPlane::default(), 0),
            (None, Some(error)) => return Err(RuntimeError::Storage(error)),
        };
        let entries = journal
            .load(through_sequence)
            .map_err(RuntimeError::Storage)?;
        let mut runtime = Self {
            control,
            journal: None,
            ephemeral_logs: BTreeMap::new(),
            next_ephemeral_log_sequence: 1,
        };
        for entry in entries {
            match entry.kind {
                JournalEntryKind::RuntimeRegistration => {
                    let registration: RuntimeRegistration = serde_json::from_slice(&entry.payload)
                        .map_err(|error| RuntimeError::Storage(error.to_string()))?;
                    runtime.control.register_runtime(
                        RuntimeId::new(registration.runtime_id).map_err(RuntimeError::Domain)?,
                        registration.name,
                        registration.endpoint,
                    );
                }
                JournalEntryKind::CommandPlan => {
                    if entry.terminal_error.is_some() {
                        continue;
                    }
                    let plan: CommandPlan = serde_json::from_slice(&entry.payload)
                        .map_err(|error| RuntimeError::Storage(error.to_string()))?;
                    plan.validate().map_err(RuntimeError::InvalidPlan)?;
                    let PlannedOperation::Command(command) = plan.operation else {
                        return Err(RuntimeError::Storage(
                            "journal contains a non-mutating command-plan entry".into(),
                        ));
                    };
                    let result = match runtime.control.execute(command) {
                        Ok(result) => result,
                        Err(error) if entry.outcome.is_none() => {
                            journal
                                .fail(entry.sequence, &error.to_string())
                                .map_err(RuntimeError::Storage)?;
                            continue;
                        }
                        Err(_) => {
                            return Err(RuntimeError::ReplayMismatch {
                                sequence: entry.sequence,
                            });
                        }
                    };
                    let encoded = serde_json::to_vec(&result)
                        .map_err(|error| RuntimeError::Storage(error.to_string()))?;
                    if let Some(expected) = entry.outcome {
                        if expected != encoded {
                            return Err(RuntimeError::ReplayMismatch {
                                sequence: entry.sequence,
                            });
                        }
                    } else {
                        journal
                            .complete(entry.sequence, &encoded)
                            .map_err(RuntimeError::Storage)?;
                    }
                }
                JournalEntryKind::RuntimeStatusObservation => {
                    let observation: RuntimeStatusObservation =
                        serde_json::from_slice(&entry.payload)
                            .map_err(|error| RuntimeError::Storage(error.to_string()))?;
                    let runtime_id =
                        RuntimeId::new(observation.runtime_id).map_err(RuntimeError::Domain)?;
                    let projection = runtime
                        .control
                        .complete_runtime_status_refresh(
                            &runtime_id,
                            observation.expected_revision,
                            observation.status,
                        )
                        .map_err(RuntimeError::Domain)?;
                    let encoded = serde_json::to_vec(&projection)
                        .map_err(|error| RuntimeError::Storage(error.to_string()))?;
                    if entry.outcome.as_deref() != Some(encoded.as_slice()) {
                        return Err(RuntimeError::ReplayMismatch {
                            sequence: entry.sequence,
                        });
                    }
                }
                JournalEntryKind::RuntimeCapabilityObservation => {
                    let observation: RuntimeCapabilityObservation =
                        serde_json::from_slice(&entry.payload)
                            .map_err(|error| RuntimeError::Storage(error.to_string()))?;
                    let runtime_id =
                        RuntimeId::new(observation.runtime_id).map_err(RuntimeError::Domain)?;
                    let projection = runtime
                        .control
                        .complete_runtime_capability_refresh(
                            &runtime_id,
                            observation.expected_revision,
                            observation.capabilities,
                        )
                        .map_err(RuntimeError::Domain)?;
                    let encoded = serde_json::to_vec(&projection)
                        .map_err(|error| RuntimeError::Storage(error.to_string()))?;
                    if entry.outcome.as_deref() != Some(encoded.as_slice()) {
                        let mut legacy: RuntimeProjection = entry
                            .outcome
                            .as_deref()
                            .and_then(|outcome| serde_json::from_slice(outcome).ok())
                            .ok_or(RuntimeError::ReplayMismatch {
                                sequence: entry.sequence,
                            })?;
                        if legacy.capabilities_observed_for_revision.is_none() {
                            legacy.capabilities_observed_for_revision =
                                Some(observation.expected_revision);
                        }
                        if legacy != projection {
                            return Err(RuntimeError::ReplayMismatch {
                                sequence: entry.sequence,
                            });
                        }
                    }
                }
            }
        }
        runtime.journal = Some(journal);
        let applied = runtime
            .control
            .snapshot()
            .applied_commands
            .into_iter()
            .map(|entry| entry.result)
            .collect::<Vec<_>>();
        for result in &applied {
            runtime.schedule_command_effects(result)?;
        }
        Ok(runtime)
    }

    pub fn create_snapshot(&mut self) -> Result<i64, RuntimeError> {
        let snapshot = self.control.snapshot();
        snapshot.validate().map_err(RuntimeError::InvalidSnapshot)?;
        let payload = serde_json::to_vec(&snapshot)
            .map_err(|error| RuntimeError::Storage(error.to_string()))?;
        let Some(journal) = &mut self.journal else {
            return Err(RuntimeError::Storage(
                "runtime snapshots require persistent storage".into(),
            ));
        };
        journal
            .save_snapshot(DOMAIN_SNAPSHOT_SCHEMA_VERSION, &payload)
            .map_err(RuntimeError::Storage)
    }

    pub fn heartbeat(&mut self) -> Result<(), RuntimeError> {
        let Some(journal) = &mut self.journal else {
            return Ok(());
        };
        journal.ensure_owner().map_err(RuntimeError::Storage)
    }

    pub fn runtime_event_state(&self) -> (Revision, Vec<RuntimeProjection>) {
        let snapshot = self.control.snapshot();
        let mut runtimes = snapshot.runtimes;
        runtimes.sort_by(|left, right| left.id.cmp(&right.id));
        (snapshot.revision, runtimes)
    }

    pub fn enqueue_effect(
        &mut self,
        effect_id: &str,
        kind: &str,
        payload: &[u8],
        max_attempts: u32,
    ) -> Result<(), RuntimeError> {
        let Some(journal) = &mut self.journal else {
            return Err(RuntimeError::Storage(
                "effect scheduling requires persistent storage".into(),
            ));
        };
        journal
            .enqueue_effect(effect_id, kind, payload, max_attempts)
            .map_err(RuntimeError::Storage)
    }

    pub fn enqueue_effect_batch(&mut self, effects: &[EffectEnqueue]) -> Result<u64, RuntimeError> {
        let Some(journal) = &mut self.journal else {
            return Err(RuntimeError::Storage(
                "effect scheduling requires persistent storage".into(),
            ));
        };
        journal
            .enqueue_effect_batch(effects)
            .map_err(RuntimeError::Storage)
    }

    pub fn effect_queue_stats(&mut self) -> Result<EffectQueueStats, RuntimeError> {
        let Some(journal) = &mut self.journal else {
            return Ok(EffectQueueStats {
                capacity: EFFECT_QUEUE_CAPACITY,
                ..EffectQueueStats::default()
            });
        };
        journal.effect_queue_stats().map_err(RuntimeError::Storage)
    }

    pub fn deployment_effect_receipt(
        &mut self,
        command_id: &str,
        request_id: &str,
    ) -> Result<Option<DeploymentEffectReceipt>, RuntimeError> {
        let Some(journal) = &mut self.journal else {
            return Err(RuntimeError::Storage(
                "deployment receipt requires persistent storage".into(),
            ));
        };
        let Some(record) = journal
            .effect_record(command_id)
            .map_err(RuntimeError::Storage)?
        else {
            return Ok(None);
        };
        deployment_receipt_from_record(record, request_id).map(Some)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn persist_orchestra_run_event(
        &mut self,
        run_id: &str,
        runtime_id: &str,
        request_id: Option<&str>,
        event_type: &str,
        to_outcome: &str,
        recorded_at: &str,
        run: &[u8],
        event: &[u8],
    ) -> Result<OrchestraPersistenceRecord, RuntimeError> {
        let Some(journal) = &mut self.journal else {
            return Err(RuntimeError::Storage(
                "Orchestra persistence requires persistent storage".into(),
            ));
        };
        journal
            .persist_orchestra_run_event(
                run_id,
                runtime_id,
                request_id,
                event_type,
                to_outcome,
                recorded_at,
                run,
                event,
            )
            .map_err(RuntimeError::Storage)
    }

    pub fn load_orchestra_history(
        &mut self,
        runtime_id: Option<&str>,
        run_id: Option<&str>,
        offset: u32,
        limit: u16,
    ) -> Result<OrchestraHistoryRecord, RuntimeError> {
        let Some(journal) = &mut self.journal else {
            return Err(RuntimeError::Storage(
                "Orchestra history requires persistent storage".into(),
            ));
        };
        journal
            .load_orchestra_history(runtime_id, run_id, offset, limit)
            .map_err(RuntimeError::Storage)
    }

    pub fn delete_orchestra_runtimes(
        &mut self,
        runtime_ids: &[String],
    ) -> Result<OrchestraDeleteRecord, RuntimeError> {
        let Some(journal) = &mut self.journal else {
            return Err(RuntimeError::Storage(
                "Orchestra delete requires persistent storage".into(),
            ));
        };
        journal
            .delete_orchestra_runtimes(runtime_ids)
            .map_err(RuntimeError::Storage)
    }

    pub fn prune_terminal_effects(
        &mut self,
        retain: u64,
        batch_limit: u64,
    ) -> Result<u64, RuntimeError> {
        let Some(journal) = &mut self.journal else {
            return Err(RuntimeError::Storage(
                "effect retention requires persistent storage".into(),
            ));
        };
        journal
            .prune_terminal_effects(retain, batch_limit)
            .map_err(RuntimeError::Storage)
    }

    pub fn claim_effect(
        &mut self,
        worker_id: &str,
        lease_duration: Duration,
    ) -> Result<Option<EffectLease>, RuntimeError> {
        self.claim_effect_excluding(worker_id, lease_duration, &[])
    }

    pub fn claim_effect_excluding(
        &mut self,
        worker_id: &str,
        lease_duration: Duration,
        excluded_kinds: &[String],
    ) -> Result<Option<EffectLease>, RuntimeError> {
        let Some(journal) = &mut self.journal else {
            return Err(RuntimeError::Storage(
                "effect scheduling requires persistent storage".into(),
            ));
        };
        journal
            .claim_effect_excluding(worker_id, lease_duration, excluded_kinds)
            .map_err(RuntimeError::Storage)
    }

    pub fn renew_effect(
        &mut self,
        lease: &EffectLease,
        lease_duration: Duration,
    ) -> Result<EffectLease, RuntimeError> {
        let Some(journal) = &mut self.journal else {
            return Err(RuntimeError::Storage(
                "effect scheduling requires persistent storage".into(),
            ));
        };
        journal
            .renew_effect(lease, lease_duration)
            .map_err(RuntimeError::Storage)
    }

    pub fn complete_effect(
        &mut self,
        lease: &EffectLease,
        outcome: &[u8],
    ) -> Result<(), RuntimeError> {
        let Some(journal) = &mut self.journal else {
            return Err(RuntimeError::Storage(
                "effect scheduling requires persistent storage".into(),
            ));
        };
        journal
            .complete_effect(lease, outcome)
            .map_err(RuntimeError::Storage)
    }

    pub fn complete_bootstrap_effect(
        &mut self,
        lease: &EffectLease,
        outcome: &[u8],
        checkpoint: &DeploymentBootstrapCheckpoint,
    ) -> Result<(), RuntimeError> {
        checkpoint.validate().map_err(RuntimeError::Bootstrap)?;
        if !matches!(
            checkpoint.state.phase,
            BootstrapPhase::Bootstrapped | BootstrapPhase::Failed
        ) {
            return Err(RuntimeError::InvalidEffectOutcome(
                "bootstrap effect did not reach a terminal deployment phase",
            ));
        }
        let payload = serde_json::to_vec(checkpoint)
            .map_err(|error| RuntimeError::Storage(error.to_string()))?;
        let Some(journal) = &mut self.journal else {
            return Err(RuntimeError::Storage(
                "bootstrap handoff requires persistent storage".into(),
            ));
        };
        journal
            .complete_effect_with_bootstrap_checkpoint(
                lease,
                outcome,
                checkpoint.state.bootstrap_id.as_str(),
                bootstrap_phase_label(checkpoint.state.phase),
                checkpoint.revision,
                &payload,
            )
            .map_err(RuntimeError::Storage)
    }

    pub fn bootstrap_checkpoint(
        &mut self,
        bootstrap_id: &BootstrapId,
    ) -> Result<Option<DeploymentBootstrapCheckpoint>, RuntimeError> {
        let Some(journal) = &mut self.journal else {
            return Err(RuntimeError::Storage(
                "bootstrap handoff requires persistent storage".into(),
            ));
        };
        let Some(record) = journal
            .bootstrap_checkpoint(bootstrap_id.as_str())
            .map_err(RuntimeError::Storage)?
        else {
            return Ok(None);
        };
        let checkpoint: DeploymentBootstrapCheckpoint = serde_json::from_slice(&record.payload)
            .map_err(|_| RuntimeError::Storage("bootstrap checkpoint is invalid JSON".into()))?;
        checkpoint.validate().map_err(|error| {
            RuntimeError::Storage(format!("invalid bootstrap checkpoint: {error}"))
        })?;
        if checkpoint.revision != record.revision
            || checkpoint.state.bootstrap_id != *bootstrap_id
            || bootstrap_phase_label(checkpoint.state.phase) != record.phase
        {
            return Err(RuntimeError::Storage(
                "bootstrap checkpoint identity or revision diverged".into(),
            ));
        }
        Ok(Some(checkpoint))
    }

    pub fn bind_bootstrap_session(
        &mut self,
        bootstrap_id: &BootstrapId,
        proof: DaemonSessionProof,
    ) -> Result<DeploymentBootstrapSnapshot, RuntimeError> {
        let checkpoint = self
            .bootstrap_checkpoint(bootstrap_id)?
            .ok_or_else(|| RuntimeError::Storage("bootstrap checkpoint was not found".into()))?;
        let mut bootstrap =
            DeploymentBootstrap::resume(&checkpoint).map_err(RuntimeError::Bootstrap)?;
        let state = bootstrap
            .bind_session(proof)
            .map_err(RuntimeError::Bootstrap)?;
        if checkpoint.state.phase == BootstrapPhase::SessionBound {
            return Ok(state);
        }
        let next_revision = checkpoint
            .revision
            .checked_add(1)
            .ok_or_else(|| RuntimeError::Storage("bootstrap revision overflow".into()))?;
        let next = bootstrap
            .checkpoint(next_revision)
            .map_err(RuntimeError::Bootstrap)?;
        let payload =
            serde_json::to_vec(&next).map_err(|error| RuntimeError::Storage(error.to_string()))?;
        let Some(journal) = &mut self.journal else {
            return Err(RuntimeError::Storage(
                "bootstrap handoff requires persistent storage".into(),
            ));
        };
        journal
            .update_bootstrap_checkpoint(
                bootstrap_id.as_str(),
                checkpoint.revision,
                bootstrap_phase_label(next.state.phase),
                &payload,
            )
            .map_err(RuntimeError::Storage)?;
        Ok(state)
    }

    pub fn fail_effect(
        &mut self,
        lease: &EffectLease,
        error: &str,
        retry_after: Duration,
    ) -> Result<(), RuntimeError> {
        let Some(journal) = &mut self.journal else {
            return Err(RuntimeError::Storage(
                "effect scheduling requires persistent storage".into(),
            ));
        };
        journal
            .fail_effect(lease, error, retry_after)
            .map_err(RuntimeError::Storage)
    }

    pub fn reject_effect(&mut self, lease: &EffectLease, error: &str) -> Result<(), RuntimeError> {
        let Some(journal) = &mut self.journal else {
            return Err(RuntimeError::Storage(
                "effect scheduling requires persistent storage".into(),
            ));
        };
        journal
            .reject_effect(lease, error)
            .map_err(RuntimeError::Storage)
    }

    pub fn run_effect_once(
        &mut self,
        worker_id: &str,
        lease_duration: Duration,
        executor: &mut impl EffectExecutor,
    ) -> Result<WorkerStep, RuntimeError> {
        let Some(lease) = self.claim_effect(worker_id, lease_duration)? else {
            return Ok(WorkerStep::Idle);
        };
        let execution = executor.execute(&lease);
        self.settle_effect(&lease, execution)
    }

    pub fn settle_effect(
        &mut self,
        lease: &EffectLease,
        execution: EffectExecution,
    ) -> Result<WorkerStep, RuntimeError> {
        let effect_id = lease.effect_id.clone();
        let attempt = lease.attempt;
        match execution {
            EffectExecution::Complete(outcome) => {
                if lease.kind == RUNTIME_STATUS_REFRESH_EFFECT_KIND {
                    match self.complete_runtime_status_effect(lease, &outcome) {
                        Ok(()) => Ok(WorkerStep::Completed { effect_id, attempt }),
                        Err(
                            error @ (RuntimeError::Domain(_)
                            | RuntimeError::InvalidEffectOutcome(_)),
                        ) => {
                            self.reject_effect(
                                lease,
                                &format!("runtime status observation was rejected: {error}"),
                            )?;
                            Ok(WorkerStep::Rejected { effect_id, attempt })
                        }
                        Err(error) => Err(error),
                    }
                } else if lease.kind == RUNTIME_CAPABILITY_DISCOVERY_EFFECT_KIND {
                    match self.complete_runtime_capability_effect(lease, &outcome) {
                        Ok(()) => Ok(WorkerStep::Completed { effect_id, attempt }),
                        Err(
                            error @ (RuntimeError::Domain(_)
                            | RuntimeError::InvalidEffectOutcome(_)),
                        ) => {
                            self.reject_effect(
                                lease,
                                &format!("runtime capability observation was rejected: {error}"),
                            )?;
                            Ok(WorkerStep::Rejected { effect_id, attempt })
                        }
                        Err(error) => Err(error),
                    }
                } else {
                    self.complete_effect(lease, &outcome)?;
                    Ok(WorkerStep::Completed { effect_id, attempt })
                }
            }
            EffectExecution::Retry { error, after } => {
                self.fail_effect(lease, &error, after)?;
                Ok(WorkerStep::RetryScheduled { effect_id, attempt })
            }
            EffectExecution::Reject { error } => {
                self.reject_effect(lease, &error)?;
                Ok(WorkerStep::Rejected { effect_id, attempt })
            }
        }
    }

    fn complete_runtime_status_effect(
        &mut self,
        lease: &EffectLease,
        outcome: &[u8],
    ) -> Result<(), RuntimeError> {
        let observation: RuntimeStatusObservation = serde_json::from_slice(outcome)
            .map_err(|_| RuntimeError::InvalidEffectOutcome("invalid runtime status JSON"))?;
        let runtime_id =
            RuntimeId::new(observation.runtime_id.clone()).map_err(RuntimeError::Domain)?;
        let mut staged = self.control.clone();
        let projection = staged
            .complete_runtime_status_refresh(
                &runtime_id,
                observation.expected_revision,
                observation.status.clone(),
            )
            .map_err(RuntimeError::Domain)?;
        let payload = serde_json::to_vec(&observation)
            .map_err(|error| RuntimeError::Storage(error.to_string()))?;
        let projection = serde_json::to_vec(&projection)
            .map_err(|error| RuntimeError::Storage(error.to_string()))?;
        let Some(journal) = &mut self.journal else {
            return Err(RuntimeError::Storage(
                "status effect completion requires persistent storage".into(),
            ));
        };
        journal
            .complete_effect_with_journal(
                lease,
                JournalEntryKind::RuntimeStatusObservation,
                &payload,
                &projection,
                outcome,
            )
            .map_err(RuntimeError::Storage)?;
        self.control = staged;
        Ok(())
    }

    fn complete_runtime_capability_effect(
        &mut self,
        lease: &EffectLease,
        outcome: &[u8],
    ) -> Result<(), RuntimeError> {
        let observation: RuntimeCapabilityObservation = serde_json::from_slice(outcome)
            .map_err(|_| RuntimeError::InvalidEffectOutcome("invalid runtime capability JSON"))?;
        let runtime_id =
            RuntimeId::new(observation.runtime_id.clone()).map_err(RuntimeError::Domain)?;
        let mut staged = self.control.clone();
        let projection = staged
            .complete_runtime_capability_refresh(
                &runtime_id,
                observation.expected_revision,
                observation.capabilities.clone(),
            )
            .map_err(RuntimeError::Domain)?;
        let payload = serde_json::to_vec(&observation)
            .map_err(|error| RuntimeError::Storage(error.to_string()))?;
        let projection = serde_json::to_vec(&projection)
            .map_err(|error| RuntimeError::Storage(error.to_string()))?;
        let Some(journal) = &mut self.journal else {
            return Err(RuntimeError::Storage(
                "capability effect completion requires persistent storage".into(),
            ));
        };
        journal
            .complete_effect_with_journal(
                lease,
                JournalEntryKind::RuntimeCapabilityObservation,
                &payload,
                &projection,
                outcome,
            )
            .map_err(RuntimeError::Storage)?;
        self.control = staged;
        Ok(())
    }

    pub fn register_runtime(
        &mut self,
        id: RuntimeId,
        name: impl Into<String>,
        endpoint: impl Into<String>,
    ) -> Result<RuntimeProjection, RuntimeError> {
        let registration = RuntimeRegistration {
            runtime_id: id.as_str().to_string(),
            name: name.into(),
            endpoint: endpoint.into(),
        };
        if let Some(journal) = &mut self.journal {
            let payload = serde_json::to_vec(&registration)
                .map_err(|error| RuntimeError::Storage(error.to_string()))?;
            journal
                .append(JournalEntryKind::RuntimeRegistration, &payload)
                .map_err(RuntimeError::Storage)?;
        }
        Ok(self
            .control
            .register_runtime(id, registration.name, registration.endpoint))
    }

    pub fn ensure_runtime_registered(
        &mut self,
        id: RuntimeId,
        name: impl Into<String>,
        endpoint: impl Into<String>,
    ) -> Result<RuntimeProjection, RuntimeError> {
        let name = name.into();
        let endpoint = endpoint.into();
        if let Some(existing) = self.control.runtime_projection(&id) {
            if existing.name != name || existing.endpoint != endpoint {
                return Err(RuntimeError::Storage(format!(
                    "configured runtime '{}' does not match persisted registration",
                    id.as_str()
                )));
            }
            return Ok(existing.clone());
        }
        self.register_runtime(id, name, endpoint)
    }

    pub fn append_runtime_log(
        &mut self,
        runtime_id: &RuntimeId,
        level: RuntimeLogLevel,
        message: impl Into<String>,
    ) -> Result<u64, RuntimeError> {
        if self.control.runtime_projection(runtime_id).is_none() {
            return Err(RuntimeError::Domain(DomainError::RuntimeNotFound {
                runtime_id: runtime_id.as_str().to_string(),
            }));
        }
        let message = message.into();
        if message.len() > MAX_RUNTIME_LOG_MESSAGE_BYTES {
            return Err(RuntimeError::Domain(DomainError::InvalidQuery {
                reason: "runtime log message exceeds the source limit",
            }));
        }
        if let Some(journal) = &mut self.journal {
            return journal
                .append_runtime_log(runtime_id, level, &message)
                .map_err(RuntimeError::Storage);
        }

        let sequence = self.next_ephemeral_log_sequence;
        self.next_ephemeral_log_sequence = self
            .next_ephemeral_log_sequence
            .checked_add(1)
            .ok_or_else(|| RuntimeError::Storage("runtime log sequence exhausted".into()))?;
        let logs = self.ephemeral_logs.entry(runtime_id.clone()).or_default();
        logs.push_back(RuntimeLogRecord {
            sequence,
            level,
            message,
        });
        while logs.len() > MAX_PERSISTED_RUNTIME_LOG_ENTRIES {
            logs.pop_front();
        }
        Ok(sequence)
    }

    pub fn execute_plan(&mut self, plan: CommandPlan) -> Result<PlanResult, RuntimeError> {
        if let Some(journal) = &mut self.journal {
            journal.ensure_owner().map_err(RuntimeError::Storage)?;
        }
        plan.validate().map_err(RuntimeError::InvalidPlan)?;
        match plan.operation {
            PlannedOperation::Query(query) => {
                if matches!(query.query, Query::RuntimeLogs { .. }) {
                    self.execute_runtime_logs_query(query)
                        .map(PlanResult::Query)
                } else {
                    self.control
                        .query(query)
                        .map(PlanResult::Query)
                        .map_err(RuntimeError::Domain)
                }
            }
            PlannedOperation::Command(command) => {
                if matches!(
                    &command.command,
                    leserpent_domain::Command::DebuggerCancel { .. }
                ) {
                    return Err(RuntimeError::Domain(DomainError::InvalidQuery {
                        reason: "debugger commands require the Leselang VM authority",
                    }));
                }
                let payload = serde_json::to_vec(&CommandPlan {
                    schema_version: plan.schema_version,
                    required_capability: plan.required_capability,
                    operation: PlannedOperation::Command(command.clone()),
                })
                .map_err(|error| RuntimeError::Storage(error.to_string()))?;
                let sequence = match &mut self.journal {
                    Some(journal) => Some(
                        journal
                            .append(JournalEntryKind::CommandPlan, &payload)
                            .map_err(RuntimeError::Storage)?,
                    ),
                    None => None,
                };
                let result = match self.control.execute(command) {
                    Ok(result) => result,
                    Err(error) => {
                        if let (Some(journal), Some(sequence)) = (&mut self.journal, sequence) {
                            journal
                                .fail(sequence, &error.to_string())
                                .map_err(RuntimeError::Storage)?;
                        }
                        return Err(RuntimeError::Domain(error));
                    }
                };
                if let (Some(journal), Some(sequence)) = (&mut self.journal, sequence) {
                    let outcome = serde_json::to_vec(&result)
                        .map_err(|error| RuntimeError::Storage(error.to_string()))?;
                    journal
                        .complete(sequence, &outcome)
                        .map_err(RuntimeError::Storage)?;
                }
                self.schedule_command_effects(&result)?;
                Ok(PlanResult::Command(result))
            }
        }
    }

    fn execute_runtime_logs_query(
        &mut self,
        query: QueryEnvelope,
    ) -> Result<QueryResult, RuntimeError> {
        let Query::RuntimeLogs {
            runtime_id,
            after_sequence,
            limit,
        } = query.query
        else {
            return Err(RuntimeError::Domain(DomainError::InvalidQuery {
                reason: "expected a runtime logs query",
            }));
        };
        if limit == 0 || limit > MAX_RUNTIME_LOG_QUERY_ENTRIES {
            return Err(RuntimeError::Domain(DomainError::InvalidQuery {
                reason: "runtime log limit must be between 1 and 256",
            }));
        }
        let inspected = self
            .control
            .query(QueryEnvelope {
                schema_version: query.schema_version,
                principal: query.principal,
                capabilities: query.capabilities,
                query: Query::RuntimeInspect {
                    runtime_id: runtime_id.clone(),
                },
            })
            .map_err(RuntimeError::Domain)?;
        let QueryResult::RuntimeInspect { revision, runtime } = inspected else {
            return Err(RuntimeError::Domain(DomainError::InvalidQuery {
                reason: "runtime inspection returned an unexpected result",
            }));
        };
        let entries = if let Some(journal) = &self.journal {
            journal
                .load_runtime_logs(&runtime_id, after_sequence, limit)
                .map_err(RuntimeError::Storage)?
        } else {
            let logs = self.ephemeral_logs.get(&runtime_id);
            let mut entries = logs
                .into_iter()
                .flatten()
                .filter(|entry| after_sequence.is_none_or(|after| entry.sequence > after))
                .cloned()
                .collect::<Vec<_>>();
            if entries.len() > usize::from(limit) {
                let start = if after_sequence.is_none() {
                    entries.len() - usize::from(limit)
                } else {
                    0
                };
                entries = entries[start..start + usize::from(limit)].to_vec();
            }
            entries
        };
        Ok(QueryResult::RuntimeLogs {
            revision,
            runtime_id,
            runtime_name: runtime.name,
            entries,
        })
    }

    fn schedule_command_effects(&mut self, result: &CommandResult) -> Result<(), RuntimeError> {
        if result.status != CommandStatus::Applied || self.journal.is_none() {
            return Ok(());
        }
        for event in &result.events {
            let (command_id, kind, payload) = match event {
                DomainEvent::RuntimeRegistered { .. }
                | DomainEvent::RuntimeRegistrationUpdated { .. }
                | DomainEvent::RuntimeDiscoveryIntakeApplied { .. } => continue,
                DomainEvent::RuntimeRefreshRequested {
                    runtime_id,
                    revision,
                    command_id,
                } => (
                    command_id,
                    RUNTIME_STATUS_REFRESH_EFFECT_KIND,
                    serde_json::to_vec(&RuntimeStatusRefreshRequest {
                        runtime_id: runtime_id.as_str().to_string(),
                        expected_revision: *revision,
                    }),
                ),
                DomainEvent::RuntimeCapabilitiesRefreshRequested {
                    runtime_id,
                    revision,
                    command_id,
                } => (
                    command_id,
                    RUNTIME_CAPABILITY_DISCOVERY_EFFECT_KIND,
                    serde_json::to_vec(&RuntimeCapabilityRefreshRequest {
                        runtime_id: runtime_id.as_str().to_string(),
                        expected_revision: *revision,
                    }),
                ),
                DomainEvent::RuntimeDeploymentRequested {
                    runtime_id,
                    command_id,
                    request_id,
                    pipeline_kind,
                    requested_by,
                    target,
                    ..
                } => (
                    command_id,
                    RUNTIME_DEPLOYMENT_EFFECT_KIND,
                    serde_json::to_vec(&RuntimeDeploymentRequest {
                        runtime_id: runtime_id.as_str().to_string(),
                        request_id: request_id.clone(),
                        pipeline_kind: pipeline_kind.clone(),
                        requested_by: requested_by.clone(),
                        confirmed: true,
                        target: target.clone(),
                    }),
                ),
            };
            let payload = payload.map_err(|error| RuntimeError::Storage(error.to_string()))?;
            self.enqueue_effect(command_id.as_str(), kind, &payload, 3)?;
        }
        Ok(())
    }
}

fn deployment_receipt_from_record(
    record: EffectRecord,
    request_id: &str,
) -> Result<DeploymentEffectReceipt, RuntimeError> {
    if record.kind != RUNTIME_DEPLOYMENT_EFFECT_KIND {
        return Err(RuntimeError::InvalidEffectOutcome(
            "deployment receipt effect kind mismatch",
        ));
    }
    let request: RuntimeDeploymentRequest = serde_json::from_slice(&record.payload)
        .map_err(|_| RuntimeError::InvalidEffectOutcome("invalid deployment effect request"))?;
    if request.request_id != request_id {
        return Err(RuntimeError::InvalidEffectOutcome(
            "deployment receipt request identity mismatch",
        ));
    }
    let (state, outcome, error) = match record.state.as_str() {
        "ready" | "leased" if record.outcome.is_none() => {
            (DeploymentEffectState::Pending, None, None)
        }
        "completed" => {
            let bytes = record.outcome.ok_or(RuntimeError::InvalidEffectOutcome(
                "completed deployment effect omitted its outcome",
            ))?;
            let outcome: RuntimeDeploymentOutcome =
                serde_json::from_slice(&bytes).map_err(|_| {
                    RuntimeError::InvalidEffectOutcome("invalid deployment effect outcome")
                })?;
            if outcome.request_id != request_id {
                return Err(RuntimeError::InvalidEffectOutcome(
                    "deployment outcome request identity mismatch",
                ));
            }
            (DeploymentEffectState::Completed, Some(outcome), None)
        }
        "failed" if record.outcome.is_none() => (
            DeploymentEffectState::Failed,
            None,
            Some(
                record
                    .last_error
                    .unwrap_or_else(|| "deployment effect failed".into()),
            ),
        ),
        _ => {
            return Err(RuntimeError::InvalidEffectOutcome(
                "deployment effect terminal state is inconsistent",
            ));
        }
    };
    Ok(DeploymentEffectReceipt {
        state,
        attempt: record.attempt,
        outcome,
        error,
    })
}

fn bootstrap_phase_label(phase: BootstrapPhase) -> &'static str {
    match phase {
        BootstrapPhase::Planned => "planned",
        BootstrapPhase::Deploying => "deploying",
        BootstrapPhase::Bootstrapped => "bootstrapped",
        BootstrapPhase::SessionBound => "session_bound",
        BootstrapPhase::Failed => "failed",
    }
}

#[cfg(test)]
mod tests {
    use leselang_command::{LoweringContext, PlannedOperation, lower_effect, plan_runtime_deploy};
    use leselang_hir::lower;
    use leselang_syntax::parse;
    use leserpent_domain::{
        CAPABILITY_RUNTIME_DEPLOY, CAPABILITY_RUNTIME_READ, CAPABILITY_RUNTIME_REFRESH,
        CapabilitySet, CommandId, CommandOrigin, Confirmation, IdempotencyKey, Principal,
        QueryResult, RefreshStatus, Revision, RuntimeCapabilityObservation,
        RuntimeCapabilitySnapshot, RuntimeStatusObservation, RuntimeStatusSnapshot,
    };
    use rusqlite::Connection;
    use std::collections::VecDeque;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    fn context() -> LoweringContext {
        LoweringContext {
            principal: Principal {
                id: "operator-a".into(),
            },
            capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_READ, CAPABILITY_RUNTIME_REFRESH]),
            expected_revision: None,
            command_id: CommandId::new("command-a").unwrap(),
            idempotency_key: IdempotencyKey::new("effect-a").unwrap(),
            origin: CommandOrigin::Leselang,
            confirmation: Confirmation::NotRequired,
            dry_run: false,
        }
    }

    fn capability_snapshot() -> RuntimeCapabilitySnapshot {
        RuntimeCapabilitySnapshot {
            source: "gewyvern-api".into(),
            service: "gewyvern-api".into(),
            version: "1.2.0".into(),
            latest_snapshot: true,
            authenticated_deployment: true,
            serve_required: true,
            external_sidecar_context: true,
            target_path_segment_encoding: "percent-encoding".into(),
            target_direct_path_chars: "A-Z a-z 0-9 . _ ~ :".into(),
            endpoints: vec!["/v1/capabilities".into(), "/v1/deployments".into()],
            extensions: BTreeMap::from([("protocol_catalog".into(), true)]),
        }
    }

    fn temp_journal(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "leserpent-runtime-{label}-{}-{unique}.sqlite",
            std::process::id()
        ))
    }

    fn refresh_plan(expected_revision: Revision) -> CommandPlan {
        let program = lower(&parse(
            "fn main() = runtime.refresh(runtime_id: \"runtime-a\")",
        ))
        .unwrap();
        let mut context = context();
        context.expected_revision = Some(expected_revision);
        lower_effect(&program.function.effect, &context).unwrap()
    }

    fn capabilities_refresh_plan(expected_revision: Revision) -> CommandPlan {
        let program = lower(&parse(
            "fn main() = runtime.refresh_capabilities(runtime_id: \"runtime-a\")",
        ))
        .unwrap();
        let mut context = context();
        context.expected_revision = Some(expected_revision);
        lower_effect(&program.function.effect, &context).unwrap()
    }

    fn list_plan() -> CommandPlan {
        let program = lower(&parse("fn main() = runtime.list()")).unwrap();
        lower_effect(&program.function.effect, &context()).unwrap()
    }

    fn logs_plan(
        runtime_id: &str,
        after_sequence: Option<u64>,
        limit: u16,
        capabilities: CapabilitySet,
    ) -> CommandPlan {
        CommandPlan {
            schema_version: leserpent_domain::COMMAND_PLAN_SCHEMA_VERSION,
            required_capability: CAPABILITY_RUNTIME_READ.to_string(),
            operation: PlannedOperation::Query(QueryEnvelope {
                schema_version: leserpent_domain::DOMAIN_SCHEMA_VERSION,
                principal: Principal {
                    id: "operator-a".into(),
                },
                capabilities,
                query: Query::RuntimeLogs {
                    runtime_id: RuntimeId::new(runtime_id).unwrap(),
                    after_sequence,
                    limit,
                },
            }),
        }
    }

    struct ScriptedExecutor {
        outcomes: VecDeque<EffectExecution>,
    }

    impl EffectExecutor for ScriptedExecutor {
        fn execute(&mut self, _lease: &EffectLease) -> EffectExecution {
            self.outcomes.pop_front().unwrap()
        }
    }

    #[test]
    fn source_to_plan_to_runtime_query_is_one_vertical_slice() {
        let program = lower(&parse("fn main() = runtime.list(role: none)")).unwrap();
        let plan = lower_effect(&program.function.effect, &context()).unwrap();
        let mut runtime = ControlRuntime::default();
        runtime
            .register_runtime(
                RuntimeId::new("runtime-a").unwrap(),
                "Runtime A",
                "https://runtime-a.invalid",
            )
            .unwrap();

        let PlanResult::Query(QueryResult::RuntimeList { runtimes, .. }) =
            runtime.execute_plan(plan).unwrap()
        else {
            panic!("runtime.list must return a query result");
        };
        assert_eq!(runtimes.len(), 1);
        assert_eq!(runtimes[0].id, RuntimeId::new("runtime-a").unwrap());
    }

    #[test]
    fn runtime_logs_are_capability_gated_windowed_and_durable() {
        let path = temp_journal("runtime-logs");
        let runtime_id = RuntimeId::new("runtime-a").unwrap();
        {
            let mut runtime = ControlRuntime::open(&path).unwrap();
            runtime
                .register_runtime(runtime_id.clone(), "Runtime A", "https://runtime-a.invalid")
                .unwrap();
            assert_eq!(
                runtime
                    .append_runtime_log(&runtime_id, RuntimeLogLevel::Debug, "first")
                    .unwrap(),
                1
            );
            assert_eq!(
                runtime
                    .append_runtime_log(&runtime_id, RuntimeLogLevel::Info, "second\nline")
                    .unwrap(),
                2
            );
            assert_eq!(
                runtime
                    .append_runtime_log(&runtime_id, RuntimeLogLevel::Error, "third")
                    .unwrap(),
                3
            );

            assert!(matches!(
                runtime.execute_plan(logs_plan("runtime-a", None, 2, CapabilitySet::default(),)),
                Err(RuntimeError::InvalidPlan(
                    CommandPlanError::MissingCapability {
                        capability: CAPABILITY_RUNTIME_READ,
                    }
                ))
            ));
            let PlanResult::Query(result) = runtime
                .execute_plan(logs_plan(
                    "runtime-a",
                    None,
                    2,
                    CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
                ))
                .unwrap()
            else {
                panic!("runtime.logs must return a query result");
            };
            let QueryResult::RuntimeLogs { entries, .. } = &result else {
                panic!("runtime.logs must return log entries");
            };
            assert_eq!(
                entries
                    .iter()
                    .map(|entry| entry.sequence)
                    .collect::<Vec<_>>(),
                [2, 3]
            );
            let encoded = serde_json::to_string(&result).unwrap();
            assert!(!encoded.contains("runtime-a.invalid"));
            assert!(!encoded.contains("endpoint"));
        }

        let mut recovered = ControlRuntime::open(&path).unwrap();
        let PlanResult::Query(QueryResult::RuntimeLogs { entries, .. }) = recovered
            .execute_plan(logs_plan(
                "runtime-a",
                Some(2),
                10,
                CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
            ))
            .unwrap()
        else {
            panic!("runtime.logs must return a query result");
        };
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].sequence, 3);
        drop(recovered);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn runtime_logs_reject_invalid_limits_and_oversized_messages() {
        let mut runtime = ControlRuntime::default();
        let runtime_id = RuntimeId::new("runtime-a").unwrap();
        runtime
            .register_runtime(runtime_id.clone(), "Runtime A", "local")
            .unwrap();
        assert!(matches!(
            runtime.append_runtime_log(
                &runtime_id,
                RuntimeLogLevel::Info,
                "x".repeat(MAX_RUNTIME_LOG_MESSAGE_BYTES + 1),
            ),
            Err(RuntimeError::Domain(DomainError::InvalidQuery { .. }))
        ));
        assert!(matches!(
            runtime.execute_plan(logs_plan(
                "runtime-a",
                None,
                0,
                CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
            )),
            Err(RuntimeError::Domain(DomainError::InvalidQuery { .. }))
        ));
    }

    #[test]
    fn refresh_plan_preserves_domain_idempotency() {
        let program = lower(&parse(
            "fn main() = runtime.refresh(runtime_id: \"runtime-a\")",
        ))
        .unwrap();
        let mut runtime = ControlRuntime::default();
        let projection = runtime
            .register_runtime(
                RuntimeId::new("runtime-a").unwrap(),
                "Runtime A",
                "https://runtime-a.invalid",
            )
            .unwrap();
        let mut context = context();
        context.expected_revision = Some(projection.revision);
        let plan = lower_effect(&program.function.effect, &context).unwrap();

        let first = runtime.execute_plan(plan.clone()).unwrap();
        let replay = runtime.execute_plan(plan).unwrap();
        assert_eq!(first, replay);
    }

    #[test]
    fn forged_capability_label_is_rejected_before_domain_dispatch() {
        let program = lower(&parse("fn main() = runtime.list()")).unwrap();
        let mut plan = lower_effect(&program.function.effect, &context()).unwrap();
        plan.required_capability = CAPABILITY_RUNTIME_REFRESH.to_string();
        assert!(matches!(
            ControlRuntime::default().execute_plan(plan),
            Err(RuntimeError::InvalidPlan(
                CommandPlanError::CapabilityMismatch {
                    expected: CAPABILITY_RUNTIME_READ
                }
            ))
        ));
    }

    #[test]
    fn plan_result_shape_keeps_query_and_command_distinct() {
        let program = lower(&parse("fn main() = runtime.list()")).unwrap();
        let plan = lower_effect(&program.function.effect, &context()).unwrap();
        assert!(matches!(plan.operation, PlannedOperation::Query(_)));
    }

    #[test]
    fn sqlite_journal_rebuilds_registration_and_completed_command() {
        let path = temp_journal("completed-replay");
        {
            let mut runtime = ControlRuntime::open(&path).unwrap();
            let projection = runtime
                .register_runtime(
                    RuntimeId::new("runtime-a").unwrap(),
                    "Runtime A",
                    "https://runtime-a.invalid",
                )
                .unwrap();
            runtime
                .execute_plan(refresh_plan(projection.revision))
                .unwrap();
        }

        let mut recovered = ControlRuntime::open(&path).unwrap();
        let PlanResult::Query(QueryResult::RuntimeList { runtimes, .. }) =
            recovered.execute_plan(list_plan()).unwrap()
        else {
            panic!("runtime.list must return a query result");
        };
        assert_eq!(runtimes.len(), 1);
        assert_eq!(runtimes[0].refresh_count, 1);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn sqlite_journal_recovers_pending_command_and_seals_outcome() {
        let path = temp_journal("pending-replay");
        {
            let mut runtime = ControlRuntime::open(&path).unwrap();
            let projection = runtime
                .register_runtime(
                    RuntimeId::new("runtime-a").unwrap(),
                    "Runtime A",
                    "https://runtime-a.invalid",
                )
                .unwrap();
            runtime
                .execute_plan(refresh_plan(projection.revision))
                .unwrap();
        }
        Connection::open(&path)
            .unwrap()
            .execute(
                "UPDATE runtime_journal SET outcome = NULL WHERE kind = 'command_plan'",
                [],
            )
            .unwrap();

        let mut recovered = ControlRuntime::open(&path).unwrap();
        let PlanResult::Query(QueryResult::RuntimeList { runtimes, .. }) =
            recovered.execute_plan(list_plan()).unwrap()
        else {
            panic!("runtime.list must return a query result");
        };
        assert_eq!(runtimes[0].refresh_count, 1);
        let sealed: i64 = Connection::open(&path)
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM runtime_journal WHERE kind = 'command_plan' AND outcome IS NOT NULL",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(sealed, 1);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn terminal_command_failure_does_not_poison_restart() {
        let path = temp_journal("terminal-failure");
        {
            let mut runtime = ControlRuntime::open(&path).unwrap();
            runtime
                .register_runtime(
                    RuntimeId::new("runtime-a").unwrap(),
                    "Runtime A",
                    "https://runtime-a.invalid",
                )
                .unwrap();
            assert!(matches!(
                runtime.execute_plan(refresh_plan(Revision(999))),
                Err(RuntimeError::Domain(DomainError::RevisionConflict { .. }))
            ));
        }

        let mut recovered = ControlRuntime::open(&path).unwrap();
        let PlanResult::Query(QueryResult::RuntimeList { runtimes, .. }) =
            recovered.execute_plan(list_plan()).unwrap()
        else {
            panic!("runtime.list must return a query result");
        };
        assert_eq!(runtimes[0].refresh_count, 0);
        fs::remove_file(path).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn sqlite_journal_is_private_and_rejects_symbolic_links() {
        use std::os::unix::fs::{PermissionsExt, symlink};

        let path = temp_journal("private");
        drop(ControlRuntime::open(&path).unwrap());
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );

        let link = path.with_extension("link.sqlite");
        symlink(&path, &link).unwrap();
        assert!(matches!(
            ControlRuntime::open(&link),
            Err(RuntimeError::Storage(ref error)) if error.contains("symbolic link")
        ));
        fs::remove_file(link).unwrap();
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn sqlite_owner_lease_excludes_a_second_live_runtime() {
        let path = temp_journal("exclusive-owner");
        let owner = ControlRuntime::open(&path).unwrap();
        assert!(matches!(
            ControlRuntime::open(&path),
            Err(RuntimeError::Storage(ref error)) if error.contains("owned by another live process")
        ));
        drop(owner);
        drop(ControlRuntime::open(&path).unwrap());
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn sqlite_expired_owner_can_be_replaced_and_old_writer_is_fenced() {
        let path = temp_journal("expired-owner");
        let mut stale = ControlRuntime::open(&path).unwrap();
        Connection::open(&path)
            .unwrap()
            .execute(
                "UPDATE runtime_owner SET lease_expires_at_unix_ms = 0 WHERE id = 1",
                [],
            )
            .unwrap();
        let replacement = ControlRuntime::open(&path).unwrap();
        assert!(matches!(
            stale.heartbeat(),
            Err(RuntimeError::Storage(ref error)) if error.contains("ownership lease was lost")
        ));
        assert!(matches!(
            stale.execute_plan(list_plan()),
            Err(RuntimeError::Storage(ref error)) if error.contains("ownership lease was lost")
        ));
        assert!(matches!(
            stale.register_runtime(
                RuntimeId::new("runtime-stale").unwrap(),
                "Stale",
                "https://stale.invalid",
            ),
            Err(RuntimeError::Storage(ref error)) if error.contains("ownership lease was lost")
        ));
        drop(replacement);
        drop(stale);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn sqlite_journal_migrates_v1_and_replays_existing_records() {
        let path = temp_journal("v1-migration");
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE runtime_metadata (
                     key TEXT PRIMARY KEY,
                     value INTEGER NOT NULL
                 ) STRICT;
                 CREATE TABLE runtime_journal (
                     sequence INTEGER PRIMARY KEY AUTOINCREMENT,
                     kind TEXT NOT NULL,
                     payload BLOB NOT NULL,
                     outcome BLOB,
                     terminal_error TEXT
                 ) STRICT;
                 INSERT INTO runtime_metadata (key, value) VALUES ('schema_version', 1);",
            )
            .unwrap();
        let registration = RuntimeRegistration {
            runtime_id: "runtime-a".into(),
            name: "Runtime A".into(),
            endpoint: "https://runtime-a.invalid".into(),
        };
        connection
            .execute(
                "INSERT INTO runtime_journal (kind, payload) VALUES ('runtime_registration', ?1)",
                [serde_json::to_vec(&registration).unwrap()],
            )
            .unwrap();
        drop(connection);

        let mut migrated = ControlRuntime::open(&path).unwrap();
        let PlanResult::Query(QueryResult::RuntimeList { runtimes, .. }) =
            migrated.execute_plan(list_plan()).unwrap()
        else {
            panic!("runtime.list must return a query result");
        };
        assert_eq!(runtimes.len(), 1);
        assert_eq!(runtimes[0].id, RuntimeId::new("runtime-a").unwrap());

        let connection = Connection::open(&path).unwrap();
        let schema: i64 = connection
            .query_row(
                "SELECT value FROM runtime_metadata WHERE key = 'schema_version'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let migration_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM runtime_schema_migrations",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let legacy_timestamp: i64 = connection
            .query_row(
                "SELECT created_at_unix_ms FROM runtime_journal WHERE sequence = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(schema, 11);
        assert_eq!(migration_count, 11);
        assert_eq!(legacy_timestamp, 0);
        drop(connection);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn sqlite_journal_rejects_incomplete_current_schema() {
        let path = temp_journal("incomplete-v11");
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE runtime_metadata (
                     key TEXT PRIMARY KEY,
                     value INTEGER NOT NULL
                 ) STRICT;
                 CREATE TABLE runtime_journal (
                     sequence INTEGER PRIMARY KEY AUTOINCREMENT,
                     kind TEXT NOT NULL,
                     payload BLOB NOT NULL,
                     outcome BLOB,
                     terminal_error TEXT
                 ) STRICT;
                 INSERT INTO runtime_metadata (key, value) VALUES ('schema_version', 11);",
            )
            .unwrap();
        drop(connection);

        assert!(matches!(
            ControlRuntime::open(&path),
            Err(RuntimeError::Storage(ref error)) if error.contains("invalid runtime journal schema 11")
        ));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn sqlite_journal_migrates_complete_v6_semantics_to_current() {
        let path = temp_journal("v6-semantic-migration");
        drop(ControlRuntime::open(&path).unwrap());
        let connection = Connection::open(&path).unwrap();
        connection
            .execute(
                "DELETE FROM runtime_schema_migrations WHERE version = 7",
                [],
            )
            .unwrap();
        connection
            .execute(
                "DELETE FROM runtime_schema_migrations WHERE version = 8",
                [],
            )
            .unwrap();
        connection
            .execute(
                "DELETE FROM runtime_schema_migrations WHERE version = 9",
                [],
            )
            .unwrap();
        connection
            .execute(
                "DELETE FROM runtime_schema_migrations WHERE version = 10",
                [],
            )
            .unwrap();
        connection
            .execute(
                "DELETE FROM runtime_schema_migrations WHERE version = 11",
                [],
            )
            .unwrap();
        connection
            .execute("DROP TABLE bootstrap_handoffs", [])
            .unwrap();
        connection
            .execute("DROP TABLE orchestra_events", [])
            .unwrap();
        connection.execute("DROP TABLE orchestra_runs", []).unwrap();
        connection.execute("DROP TABLE runtime_logs", []).unwrap();
        connection
            .execute(
                "UPDATE runtime_metadata SET value = 6 WHERE key = 'schema_version'",
                [],
            )
            .unwrap();
        drop(connection);

        drop(ControlRuntime::open(&path).unwrap());
        let connection = Connection::open(&path).unwrap();
        let schema: i64 = connection
            .query_row(
                "SELECT value FROM runtime_metadata WHERE key = 'schema_version'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let migration: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM runtime_schema_migrations WHERE version = 7",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(schema, 11);
        assert_eq!(migration, 1);
        drop(connection);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn sqlite_journal_rejects_unknown_current_entry_kind_before_replay() {
        let path = temp_journal("current-unknown-kind");
        drop(ControlRuntime::open(&path).unwrap());
        Connection::open(&path)
            .unwrap()
            .execute(
                "INSERT INTO runtime_journal (kind, payload, created_at_unix_ms)
                 VALUES ('future_kind', x'7b7d', 0)",
                [],
            )
            .unwrap();
        assert!(matches!(
            ControlRuntime::open(&path),
            Err(RuntimeError::Storage(ref error))
                if error.contains("invalid runtime journal schema 11 journal kind")
        ));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn sqlite_journal_rejects_extra_migration_history() {
        let path = temp_journal("current-extra-migration");
        drop(ControlRuntime::open(&path).unwrap());
        Connection::open(&path)
            .unwrap()
            .execute(
                "INSERT INTO runtime_schema_migrations (version, applied_at_unix_ms)
                 VALUES (12, 0)",
                [],
            )
            .unwrap();
        assert!(matches!(
            ControlRuntime::open(&path),
            Err(RuntimeError::Storage(ref error))
                if error.contains("invalid runtime journal schema 11 migration history")
        ));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn sqlite_snapshot_restores_idempotency_then_replays_incremental_journal() {
        let path = temp_journal("snapshot-incremental");
        let (plan, first, through_sequence) = {
            let mut runtime = ControlRuntime::open(&path).unwrap();
            let projection = runtime
                .register_runtime(
                    RuntimeId::new("runtime-a").unwrap(),
                    "Runtime A",
                    "https://runtime-a.invalid",
                )
                .unwrap();
            let plan = refresh_plan(projection.revision);
            let first = runtime.execute_plan(plan.clone()).unwrap();
            let through_sequence = runtime.create_snapshot().unwrap();
            runtime
                .register_runtime(
                    RuntimeId::new("runtime-b").unwrap(),
                    "Runtime B",
                    "https://runtime-b.invalid",
                )
                .unwrap();
            (plan, first, through_sequence)
        };
        assert_eq!(through_sequence, 2);

        let mut recovered = ControlRuntime::open(&path).unwrap();
        assert_eq!(recovered.execute_plan(plan).unwrap(), first);
        let PlanResult::Query(QueryResult::RuntimeList { runtimes, .. }) =
            recovered.execute_plan(list_plan()).unwrap()
        else {
            panic!("runtime.list must return a query result");
        };
        assert_eq!(runtimes.len(), 2);
        assert_eq!(runtimes[0].refresh_count, 1);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn sqlite_snapshot_rejects_corrupted_payload() {
        let path = temp_journal("snapshot-corruption");
        {
            let mut runtime = ControlRuntime::open(&path).unwrap();
            runtime
                .register_runtime(
                    RuntimeId::new("runtime-a").unwrap(),
                    "Runtime A",
                    "https://runtime-a.invalid",
                )
                .unwrap();
            runtime.create_snapshot().unwrap();
        }
        Connection::open(&path)
            .unwrap()
            .execute(
                "UPDATE runtime_snapshots SET payload = x'7b7d' WHERE generation = (SELECT MAX(generation) FROM runtime_snapshots)",
                [],
            )
            .unwrap();
        assert!(matches!(
            ControlRuntime::open(&path),
            Err(RuntimeError::Storage(ref error)) if error.contains("failed integrity validation")
        ));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn sqlite_snapshot_checksum_covers_replay_boundary() {
        let path = temp_journal("snapshot-boundary-corruption");
        {
            let mut runtime = ControlRuntime::open(&path).unwrap();
            runtime
                .register_runtime(
                    RuntimeId::new("runtime-a").unwrap(),
                    "Runtime A",
                    "https://runtime-a.invalid",
                )
                .unwrap();
            runtime.create_snapshot().unwrap();
        }
        Connection::open(&path)
            .unwrap()
            .execute(
                "UPDATE runtime_snapshots SET through_sequence = through_sequence + 1 WHERE generation = (SELECT MAX(generation) FROM runtime_snapshots)",
                [],
            )
            .unwrap();
        assert!(matches!(
            ControlRuntime::open(&path),
            Err(RuntimeError::Storage(ref error)) if error.contains("failed integrity validation")
        ));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn sqlite_snapshot_falls_back_to_prior_generation_and_replays_suffix() {
        let path = temp_journal("snapshot-fallback");
        {
            let mut runtime = ControlRuntime::open(&path).unwrap();
            runtime
                .register_runtime(
                    RuntimeId::new("runtime-a").unwrap(),
                    "Runtime A",
                    "https://runtime-a.invalid",
                )
                .unwrap();
            runtime.create_snapshot().unwrap();
            runtime
                .register_runtime(
                    RuntimeId::new("runtime-b").unwrap(),
                    "Runtime B",
                    "https://runtime-b.invalid",
                )
                .unwrap();
            runtime.create_snapshot().unwrap();
        }
        let connection = Connection::open(&path).unwrap();
        let oldest_sequence: i64 = connection
            .query_row("SELECT MIN(sequence) FROM runtime_journal", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(oldest_sequence, 2);
        connection
            .execute(
                "UPDATE runtime_snapshots SET payload = x'7b7d'
                 WHERE generation = (SELECT MAX(generation) FROM runtime_snapshots)",
                [],
            )
            .unwrap();
        drop(connection);

        let mut recovered = ControlRuntime::open(&path).unwrap();
        let PlanResult::Query(QueryResult::RuntimeList { runtimes, .. }) =
            recovered.execute_plan(list_plan()).unwrap()
        else {
            panic!("runtime.list must return a query result");
        };
        assert_eq!(runtimes.len(), 2);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn sqlite_snapshot_reports_error_when_every_generation_is_unsupported() {
        let path = temp_journal("snapshot-all-unsupported");
        {
            let mut runtime = ControlRuntime::open(&path).unwrap();
            runtime
                .register_runtime(
                    RuntimeId::new("runtime-a").unwrap(),
                    "Runtime A",
                    "https://runtime-a.invalid",
                )
                .unwrap();
            runtime.create_snapshot().unwrap();
            runtime
                .register_runtime(
                    RuntimeId::new("runtime-b").unwrap(),
                    "Runtime B",
                    "https://runtime-b.invalid",
                )
                .unwrap();
            runtime.create_snapshot().unwrap();
        }

        let mut connection = Connection::open(&path).unwrap();
        let snapshots = {
            let mut statement = connection
                .prepare("SELECT generation, through_sequence, payload FROM runtime_snapshots")
                .unwrap();
            statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, Vec<u8>>(2)?,
                    ))
                })
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap()
        };
        let transaction = connection.transaction().unwrap();
        for (generation, through_sequence, payload) in snapshots {
            let unsupported_schema = leserpent_domain::DOMAIN_SNAPSHOT_SCHEMA_VERSION + 1;
            let checksum =
                persistence::snapshot_checksum(unsupported_schema, through_sequence, &payload);
            transaction
                .execute(
                    "UPDATE runtime_snapshots
                     SET domain_schema = ?1, checksum = ?2
                     WHERE generation = ?3",
                    rusqlite::params![unsupported_schema, checksum, generation],
                )
                .unwrap();
        }
        transaction.commit().unwrap();

        assert!(matches!(
            ControlRuntime::open(&path),
            Err(RuntimeError::Storage(ref error))
                if error.contains("uses domain schema") && error.contains("expected")
        ));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn sqlite_snapshot_compaction_is_bounded_per_checkpoint() {
        let path = temp_journal("bounded-compaction");
        {
            let mut runtime = ControlRuntime::open(&path).unwrap();
            for index in 0..1_005 {
                runtime
                    .register_runtime(
                        RuntimeId::new(format!("runtime-{index}")).unwrap(),
                        format!("Runtime {index}"),
                        format!("https://runtime-{index}.invalid"),
                    )
                    .unwrap();
            }
            runtime.create_snapshot().unwrap();
            runtime.create_snapshot().unwrap();
        }
        let connection = Connection::open(&path).unwrap();
        let remaining: i64 = connection
            .query_row("SELECT COUNT(*) FROM runtime_journal", [], |row| row.get(0))
            .unwrap();
        let generations: i64 = connection
            .query_row("SELECT COUNT(*) FROM runtime_snapshots", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(remaining, 5);
        assert_eq!(generations, 2);
        drop(connection);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn sqlite_journal_migrates_v3_snapshot_into_generation_history() {
        let path = temp_journal("v3-snapshot-migration");
        {
            let mut runtime = ControlRuntime::open(&path).unwrap();
            runtime
                .register_runtime(
                    RuntimeId::new("runtime-a").unwrap(),
                    "Runtime A",
                    "https://runtime-a.invalid",
                )
                .unwrap();
            runtime.create_snapshot().unwrap();
        }
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "ALTER TABLE runtime_snapshots RENAME TO runtime_snapshots_v4;
                 CREATE TABLE runtime_snapshots (
                     id INTEGER PRIMARY KEY CHECK (id = 1),
                     domain_schema INTEGER NOT NULL CHECK (domain_schema >= 1),
                     through_sequence INTEGER NOT NULL CHECK (through_sequence >= 0),
                     payload BLOB NOT NULL,
                     checksum TEXT NOT NULL,
                     created_at_unix_ms INTEGER NOT NULL CHECK (created_at_unix_ms >= 0)
                 ) STRICT;
                 INSERT INTO runtime_snapshots
                     (id, domain_schema, through_sequence, payload, checksum, created_at_unix_ms)
                 SELECT 1, domain_schema, through_sequence, payload, checksum, created_at_unix_ms
                 FROM runtime_snapshots_v4;
                 DROP TABLE runtime_snapshots_v4;
                 DROP TABLE runtime_owner;
                 DROP TABLE runtime_effect_tasks;
                 DROP TABLE runtime_logs;
                 DROP TABLE orchestra_events;
                 DROP TABLE orchestra_runs;
                 DROP TABLE bootstrap_handoffs;
                 DELETE FROM runtime_schema_migrations WHERE version >= 4;
                 UPDATE runtime_metadata SET value = 3 WHERE key = 'schema_version';",
            )
            .unwrap();
        drop(connection);

        let mut migrated = ControlRuntime::open(&path).unwrap();
        let PlanResult::Query(QueryResult::RuntimeList { runtimes, .. }) =
            migrated.execute_plan(list_plan()).unwrap()
        else {
            panic!("runtime.list must return a query result");
        };
        assert_eq!(runtimes.len(), 1);
        let connection = Connection::open(&path).unwrap();
        let schema: i64 = connection
            .query_row(
                "SELECT value FROM runtime_metadata WHERE key = 'schema_version'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let generation: i64 = connection
            .query_row("SELECT generation FROM runtime_snapshots", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(schema, 11);
        assert_eq!(generation, 1);
        drop(connection);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn sqlite_scheduler_enqueues_idempotently_and_completes_once() {
        let path = temp_journal("scheduler-complete");
        let mut runtime = ControlRuntime::open(&path).unwrap();
        runtime
            .enqueue_effect("effect-a", "gewyvern.refresh", br#"{"runtime":"a"}"#, 3)
            .unwrap();
        runtime
            .enqueue_effect("effect-a", "gewyvern.refresh", br#"{"runtime":"a"}"#, 3)
            .unwrap();
        assert!(
            runtime
                .enqueue_effect("effect-a", "gewyvern.refresh", b"different", 3)
                .is_err()
        );
        let lease = runtime
            .claim_effect("worker-a", Duration::from_secs(30))
            .unwrap()
            .unwrap();
        assert_eq!(lease.effect_id, "effect-a");
        assert_eq!(lease.attempt, 1);
        let lease = runtime
            .renew_effect(&lease, Duration::from_secs(30))
            .unwrap();
        runtime.complete_effect(&lease, b"ok").unwrap();
        assert!(
            runtime
                .claim_effect("worker-a", Duration::from_secs(30))
                .unwrap()
                .is_none()
        );
        drop(runtime);
        let connection = Connection::open(&path).unwrap();
        let (count, outcome): (i64, Vec<u8>) = connection
            .query_row(
                "SELECT COUNT(*), outcome FROM runtime_effect_tasks WHERE state = 'completed'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(count, 1);
        assert_eq!(outcome, b"ok");
        drop(connection);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn lost_effect_lease_rolls_back_bootstrap_checkpoint_insertion() {
        let path = temp_journal("bootstrap-atomic-settlement");
        let mut runtime = ControlRuntime::open(&path).unwrap();
        runtime
            .enqueue_effect(
                "bootstrap-effect",
                "leserpent.host.bootstrap",
                b"request",
                3,
            )
            .unwrap();
        let lease = runtime
            .claim_effect("worker-a", Duration::from_millis(1))
            .unwrap()
            .unwrap();
        let bootstrap_id = BootstrapId::new("bootstrap-atomic-1").unwrap();
        let checkpoint = DeploymentBootstrapCheckpoint::new(
            1,
            DeploymentBootstrapSnapshot {
                bootstrap_id: bootstrap_id.clone(),
                phase: BootstrapPhase::Bootstrapped,
                target: leserpent_domain::bootstrap::BootstrapTarget {
                    transport: leserpent_domain::bootstrap::BootstrapTransport::Ssh,
                    host: "host.example".into(),
                    port: 22,
                },
                bootstrap_credential_present: true,
                daemon_id: Some(
                    leserpent_domain::bootstrap::DaemonId::new("daemon-host-example").unwrap(),
                ),
                endpoint: Some("https://host.example:9443/".into()),
                session_credential_handle: Some(
                    leserpent_domain::bootstrap::CredentialHandle::new(
                        "vault:leserpentd:host-example",
                    )
                    .unwrap(),
                ),
                trust_credential_handle: Some(
                    leserpent_domain::bootstrap::CredentialHandle::new(
                        "vault:leserpent-ca:host-example",
                    )
                    .unwrap(),
                ),
                fault_code: None,
                mutation_authorized: false,
            },
            Some(
                leserpent_domain::bootstrap::CredentialHandle::new("vault:ssh:host-example")
                    .unwrap(),
            ),
        )
        .unwrap();
        std::thread::sleep(Duration::from_millis(5));

        assert!(matches!(
            runtime.complete_bootstrap_effect(&lease, b"outcome", &checkpoint),
            Err(RuntimeError::Storage(ref error)) if error.contains("lease was lost or expired")
        ));
        assert!(
            runtime
                .bootstrap_checkpoint(&bootstrap_id)
                .unwrap()
                .is_none()
        );
        drop(runtime);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn configured_runtime_registration_is_idempotent_and_rejects_drift() {
        let path = temp_journal("configured-runtime-registration");
        let mut runtime = ControlRuntime::open(&path).unwrap();
        let id = RuntimeId::new("runtime-a").unwrap();
        let first = runtime
            .ensure_runtime_registered(id.clone(), "runtime-a", "http://127.0.0.1:9411")
            .unwrap();
        let replay = runtime
            .ensure_runtime_registered(id.clone(), "runtime-a", "http://127.0.0.1:9411")
            .unwrap();
        assert_eq!(first, replay);
        assert!(matches!(
            runtime.ensure_runtime_registered(id, "runtime-a", "http://127.0.0.1:9511"),
            Err(RuntimeError::Storage(ref error)) if error.contains("does not match")
        ));
        drop(runtime);
        let connection = Connection::open(&path).unwrap();
        let registrations: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM runtime_journal WHERE kind = 'runtime_registration'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(registrations, 1);
        drop(connection);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn orchestra_run_and_event_persist_atomically_with_idempotent_read_back() {
        let path = temp_journal("orchestra-atomic-persistence");
        let mut runtime = ControlRuntime::open(&path).unwrap();
        let run = br#"{"runId":"orun-1","runtimeId":"runtime-a","outcome":"queued"}"#;
        let event = br#"{"runId":"orun-1","runtimeId":"runtime-a","eventType":"run_queued","toOutcome":"queued","recordedAt":"2026-01-01T00:00:00Z"}"#;
        let first = runtime
            .persist_orchestra_run_event(
                "orun-1",
                "runtime-a",
                Some("request-1"),
                "run_queued",
                "queued",
                "2026-01-01T00:00:00Z",
                run,
                event,
            )
            .unwrap();
        assert_eq!(first.run, run);
        assert_eq!(first.event, event);
        assert_eq!(first.event_count, 1);
        let replay = runtime
            .persist_orchestra_run_event(
                "orun-1",
                "runtime-a",
                Some("request-1"),
                "run_queued",
                "queued",
                "2026-01-01T00:00:00Z",
                run,
                event,
            )
            .unwrap();
        assert_eq!(replay.event_count, 1);

        let changed_run = br#"{"runId":"orun-1","runtimeId":"runtime-a","outcome":"running"}"#;
        let changed_event = br#"{"runId":"orun-1","runtimeId":"runtime-a","eventType":"run_queued","toOutcome":"queued","recordedAt":"2026-01-01T00:00:00Z","drift":true}"#;
        assert!(
            runtime
                .persist_orchestra_run_event(
                    "orun-1",
                    "runtime-a",
                    Some("request-1"),
                    "run_queued",
                    "queued",
                    "2026-01-01T00:00:00Z",
                    changed_run,
                    event,
                )
                .is_err()
        );
        assert!(
            runtime
                .persist_orchestra_run_event(
                    "orun-1",
                    "runtime-a",
                    Some("request-1"),
                    "run_queued",
                    "queued",
                    "2026-01-01T00:00:00Z",
                    run,
                    changed_event,
                )
                .is_err()
        );
        let after_rollback = runtime
            .persist_orchestra_run_event(
                "orun-1",
                "runtime-a",
                Some("request-1"),
                "run_queued",
                "queued",
                "2026-01-01T00:00:00Z",
                run,
                event,
            )
            .unwrap();
        assert_eq!(after_rollback.run, run);
        assert_eq!(after_rollback.event_count, 1);
        let run_history = runtime
            .load_orchestra_history(Some("runtime-a"), None, 0, 1)
            .unwrap();
        assert_eq!(run_history.runs, [run]);
        assert!(run_history.events.is_empty());
        assert_eq!(run_history.next_offset, None);
        let event_history = runtime
            .load_orchestra_history(Some("runtime-a"), Some("orun-1"), 0, 1)
            .unwrap();
        assert_eq!(event_history.events, [(1, event.to_vec())]);
        assert!(event_history.runs.is_empty());
        assert_eq!(event_history.next_offset, None);
        assert!(
            runtime
                .load_orchestra_history(None, Some("orun-1"), 0, 1)
                .is_err()
        );
        assert!(runtime.load_orchestra_history(None, None, 0, 65).is_err());
        let duplicate_request_run =
            br#"{"runId":"orun-2","runtimeId":"runtime-a","outcome":"queued"}"#;
        let duplicate_request_event = br#"{"runId":"orun-2","runtimeId":"runtime-a","eventType":"run_queued","toOutcome":"queued","recordedAt":"2026-01-01T00:00:01Z"}"#;
        assert!(
            runtime
                .persist_orchestra_run_event(
                    "orun-2",
                    "runtime-a",
                    Some("request-1"),
                    "run_queued",
                    "queued",
                    "2026-01-01T00:00:01Z",
                    duplicate_request_run,
                    duplicate_request_event,
                )
                .is_err()
        );
        assert!(
            runtime
                .persist_orchestra_run_event(
                    "orun-1",
                    "runtime-b",
                    Some("request-1"),
                    "run_queued",
                    "queued",
                    "2026-01-01T00:00:00Z",
                    run,
                    event,
                )
                .is_err()
        );
        let deleted = runtime
            .delete_orchestra_runtimes(&["runtime-a".into()])
            .unwrap();
        assert_eq!(deleted.deleted_runtime_count, 1);
        assert_eq!(deleted.deleted_run_count, 1);
        assert_eq!(deleted.deleted_event_count, 1);
        assert!(
            runtime
                .load_orchestra_history(Some("runtime-a"), None, 0, 1)
                .unwrap()
                .runs
                .is_empty()
        );
        assert!(
            runtime
                .delete_orchestra_runtimes(&["runtime-a".into(), "runtime-a".into()])
                .is_err()
        );
        for index in 0..33 {
            let run_id = format!("bounded-{index:02}");
            let request_id = format!("bounded-request-{index:02}");
            let recorded_at = format!("2026-01-01T00:01:{index:02}Z");
            let bounded_run = format!(
                "{{\"runId\":\"{run_id}\",\"runtimeId\":\"runtime-a\",\"outcome\":\"queued\"}}"
            );
            let bounded_event = format!(
                "{{\"runId\":\"{run_id}\",\"runtimeId\":\"runtime-a\",\"eventType\":\"run_queued\",\"toOutcome\":\"queued\",\"recordedAt\":\"{recorded_at}\"}}"
            );
            runtime
                .persist_orchestra_run_event(
                    &run_id,
                    "runtime-a",
                    Some(&request_id),
                    "run_queued",
                    "queued",
                    &recorded_at,
                    bounded_run.as_bytes(),
                    bounded_event.as_bytes(),
                )
                .unwrap();
        }
        let bounded = runtime
            .load_orchestra_history(Some("runtime-a"), None, 0, 64)
            .unwrap();
        assert_eq!(bounded.runs.len(), 32);
        assert!(bounded.runs.iter().all(|run| {
            !run.windows(b"bounded-00".len())
                .any(|value| value == b"bounded-00")
        }));
        drop(runtime);
        let connection = Connection::open(&path).unwrap();
        let schema: i64 = connection
            .query_row(
                "SELECT value FROM runtime_metadata WHERE key = 'schema_version'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(schema, 11);
        drop(connection);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn deployment_receipt_tracks_persisted_terminal_outcome_and_fences_identity() {
        let path = temp_journal("deployment-receipt");
        let mut runtime = ControlRuntime::open(&path).unwrap();
        let request = RuntimeDeploymentRequest {
            runtime_id: "runtime-a".into(),
            request_id: "deploy-1".into(),
            pipeline_kind: "http/request".into(),
            requested_by: "operator.example".into(),
            confirmed: true,
            target: Some("pid:42".into()),
        };
        runtime
            .enqueue_effect(
                "deploy-command",
                RUNTIME_DEPLOYMENT_EFFECT_KIND,
                &serde_json::to_vec(&request).unwrap(),
                3,
            )
            .unwrap();
        assert_eq!(
            runtime
                .deployment_effect_receipt("deploy-command", "deploy-1")
                .unwrap()
                .unwrap()
                .state,
            DeploymentEffectState::Pending
        );
        assert!(matches!(
            runtime.deployment_effect_receipt("deploy-command", "other"),
            Err(RuntimeError::InvalidEffectOutcome(
                "deployment receipt request identity mismatch"
            ))
        ));
        let lease = runtime
            .claim_effect("worker-a", Duration::from_secs(30))
            .unwrap()
            .unwrap();
        let outcome = RuntimeDeploymentOutcome {
            deployment_id: "gdep-1".into(),
            request_id: "deploy-1".into(),
            pipeline_kind: "http/request".into(),
            requested_by: "operator.example".into(),
            status: "accepted".into(),
            accepted_unix_ms: 1_700_000_000_000,
            target: Some("pid:42".into()),
            replayed: false,
        };
        runtime
            .complete_effect(&lease, &serde_json::to_vec(&outcome).unwrap())
            .unwrap();
        let receipt = runtime
            .deployment_effect_receipt("deploy-command", "deploy-1")
            .unwrap()
            .unwrap();
        assert_eq!(receipt.state, DeploymentEffectState::Completed);
        assert_eq!(receipt.attempt, 1);
        assert_eq!(receipt.outcome, Some(outcome));
        assert_eq!(receipt.error, None);

        runtime
            .enqueue_effect("not-deployment", "other.effect", b"{}", 1)
            .unwrap();
        assert!(matches!(
            runtime.deployment_effect_receipt("not-deployment", "deploy-1"),
            Err(RuntimeError::InvalidEffectOutcome(
                "deployment receipt effect kind mismatch"
            ))
        ));
        drop(runtime);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn sqlite_scheduler_batch_enqueue_is_atomic_and_idempotent() {
        let path = temp_journal("scheduler-batch-enqueue");
        let mut runtime = ControlRuntime::open(&path).unwrap();
        let batch = vec![
            EffectEnqueue {
                effect_id: "batch-a".into(),
                kind: "test.batch".into(),
                payload: b"a".to_vec(),
                max_attempts: 3,
            },
            EffectEnqueue {
                effect_id: "batch-b".into(),
                kind: "test.batch".into(),
                payload: b"b".to_vec(),
                max_attempts: 3,
            },
        ];
        assert_eq!(runtime.enqueue_effect_batch(&batch).unwrap(), 2);
        assert_eq!(runtime.enqueue_effect_batch(&batch).unwrap(), 0);
        assert_eq!(runtime.effect_queue_stats().unwrap().ready, 2);

        let conflicting = vec![
            EffectEnqueue {
                effect_id: "batch-c".into(),
                kind: "test.batch".into(),
                payload: b"c".to_vec(),
                max_attempts: 3,
            },
            EffectEnqueue {
                effect_id: "batch-a".into(),
                kind: "test.batch".into(),
                payload: b"changed".to_vec(),
                max_attempts: 3,
            },
        ];
        assert!(runtime.enqueue_effect_batch(&conflicting).is_err());
        assert_eq!(runtime.effect_queue_stats().unwrap().ready, 2);

        let duplicated = vec![batch[0].clone(), batch[0].clone()];
        assert!(runtime.enqueue_effect_batch(&duplicated).is_err());
        assert!(runtime.enqueue_effect_batch(&[]).is_err());
        let oversized = (0..=MAX_EFFECT_ENQUEUE_BATCH)
            .map(|index| EffectEnqueue {
                effect_id: format!("oversized-{index}"),
                kind: "test.batch".into(),
                payload: Vec::new(),
                max_attempts: 1,
            })
            .collect::<Vec<_>>();
        assert!(runtime.enqueue_effect_batch(&oversized).is_err());
        drop(runtime);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn sqlite_scheduler_batch_capacity_preserves_idempotent_replay() {
        let path = temp_journal("scheduler-batch-capacity");
        let mut runtime = ControlRuntime::open(&path).unwrap();
        for start in (0..EFFECT_QUEUE_CAPACITY).step_by(MAX_EFFECT_ENQUEUE_BATCH) {
            let batch = (start..start + MAX_EFFECT_ENQUEUE_BATCH as u64)
                .map(|index| EffectEnqueue {
                    effect_id: format!("capacity-{index}"),
                    kind: "test.capacity".into(),
                    payload: b"capacity".to_vec(),
                    max_attempts: 1,
                })
                .collect::<Vec<_>>();
            assert_eq!(
                runtime.enqueue_effect_batch(&batch).unwrap(),
                MAX_EFFECT_ENQUEUE_BATCH as u64
            );
        }
        assert!(runtime.effect_queue_stats().unwrap().saturated());
        let existing = EffectEnqueue {
            effect_id: "capacity-0".into(),
            kind: "test.capacity".into(),
            payload: b"capacity".to_vec(),
            max_attempts: 1,
        };
        assert_eq!(
            runtime
                .enqueue_effect_batch(std::slice::from_ref(&existing))
                .unwrap(),
            0
        );
        let overflow = EffectEnqueue {
            effect_id: "capacity-overflow".into(),
            kind: "test.capacity".into(),
            payload: b"overflow".to_vec(),
            max_attempts: 1,
        };
        assert!(runtime.enqueue_effect_batch(&[existing, overflow]).is_err());
        assert_eq!(
            runtime.effect_queue_stats().unwrap().active(),
            EFFECT_QUEUE_CAPACITY
        );
        drop(runtime);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn effect_queue_stats_and_retention_preserve_active_work() {
        let path = temp_journal("scheduler-observability");
        let mut runtime = ControlRuntime::open(&path).unwrap();

        runtime
            .enqueue_effect("effect-completed", "test.effect", b"completed", 3)
            .unwrap();
        let completed = runtime
            .claim_effect("worker-a", Duration::from_secs(30))
            .unwrap()
            .unwrap();
        runtime.complete_effect(&completed, b"ok").unwrap();

        runtime
            .enqueue_effect("effect-failed", "test.effect", b"failed", 3)
            .unwrap();
        let failed = runtime
            .claim_effect("worker-a", Duration::from_secs(30))
            .unwrap()
            .unwrap();
        runtime.reject_effect(&failed, "terminal").unwrap();

        runtime
            .enqueue_effect("effect-leased", "test.effect", b"leased", 3)
            .unwrap();
        let leased = runtime
            .claim_effect("worker-a", Duration::from_secs(30))
            .unwrap()
            .unwrap();
        runtime
            .enqueue_effect("effect-ready", "test.effect", b"ready", 3)
            .unwrap();

        let stats = runtime.effect_queue_stats().unwrap();
        assert_eq!(stats.ready, 1);
        assert_eq!(stats.leased, 1);
        assert_eq!(stats.completed, 1);
        assert_eq!(stats.failed, 1);
        assert_eq!(stats.active(), 2);
        assert_eq!(stats.terminal(), 2);
        assert_eq!(stats.total(), 4);
        assert!(!stats.saturated());

        assert_eq!(runtime.prune_terminal_effects(1, 1).unwrap(), 1);
        let compacted = runtime.effect_queue_stats().unwrap();
        assert_eq!(compacted.active(), 2);
        assert_eq!(compacted.terminal(), 1);
        assert_eq!(runtime.prune_terminal_effects(0, 100).unwrap(), 1);
        assert_eq!(runtime.effect_queue_stats().unwrap().total(), 2);
        assert!(runtime.prune_terminal_effects(0, 0).is_err());

        runtime.complete_effect(&leased, b"still-owned").unwrap();
        drop(runtime);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn sqlite_scheduler_redelivers_expired_lease_and_fences_old_attempt() {
        let path = temp_journal("scheduler-redelivery");
        let mut runtime = ControlRuntime::open(&path).unwrap();
        runtime
            .enqueue_effect("effect-a", "gewyvern.refresh", b"payload", 3)
            .unwrap();
        let stale = runtime
            .claim_effect("worker-a", Duration::from_secs(30))
            .unwrap()
            .unwrap();
        Connection::open(&path)
            .unwrap()
            .execute(
                "UPDATE runtime_effect_tasks SET lease_expires_at_unix_ms = 0
                 WHERE effect_id = 'effect-a'",
                [],
            )
            .unwrap();
        let replacement = runtime
            .claim_effect("worker-b", Duration::from_secs(30))
            .unwrap()
            .unwrap();
        assert_eq!(replacement.attempt, 2);
        assert!(matches!(
            runtime.complete_effect(&stale, b"stale"),
            Err(RuntimeError::Storage(ref error)) if error.contains("lease was lost or expired")
        ));
        runtime.complete_effect(&replacement, b"fresh").unwrap();
        drop(runtime);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn sqlite_scheduler_failure_respects_max_attempts() {
        let path = temp_journal("scheduler-max-attempts");
        let mut runtime = ControlRuntime::open(&path).unwrap();
        runtime
            .enqueue_effect("effect-a", "gewyvern.refresh", b"payload", 2)
            .unwrap();
        let first = runtime
            .claim_effect("worker-a", Duration::from_secs(30))
            .unwrap()
            .unwrap();
        runtime
            .fail_effect(&first, "temporary", Duration::ZERO)
            .unwrap();
        let second = runtime
            .claim_effect("worker-a", Duration::from_secs(30))
            .unwrap()
            .unwrap();
        assert_eq!(second.attempt, 2);
        runtime
            .fail_effect(&second, "terminal", Duration::ZERO)
            .unwrap();
        assert!(
            runtime
                .claim_effect("worker-a", Duration::from_secs(30))
                .unwrap()
                .is_none()
        );
        drop(runtime);
        let state: String = Connection::open(&path)
            .unwrap()
            .query_row(
                "SELECT state FROM runtime_effect_tasks WHERE effect_id = 'effect-a'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(state, "failed");
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn sqlite_scheduler_seals_expired_final_attempt_after_worker_crash() {
        let path = temp_journal("scheduler-final-crash");
        let mut runtime = ControlRuntime::open(&path).unwrap();
        runtime
            .enqueue_effect("effect-a", "gewyvern.refresh", b"payload", 1)
            .unwrap();
        runtime
            .claim_effect("worker-a", Duration::from_secs(30))
            .unwrap()
            .unwrap();
        Connection::open(&path)
            .unwrap()
            .execute(
                "UPDATE runtime_effect_tasks SET lease_expires_at_unix_ms = 0
                 WHERE effect_id = 'effect-a'",
                [],
            )
            .unwrap();
        assert!(
            runtime
                .claim_effect("worker-b", Duration::from_secs(30))
                .unwrap()
                .is_none()
        );
        drop(runtime);
        let state: String = Connection::open(&path)
            .unwrap()
            .query_row(
                "SELECT state FROM runtime_effect_tasks WHERE effect_id = 'effect-a'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(state, "failed");
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn synchronous_worker_step_drives_retry_complete_reject_and_idle() {
        let path = temp_journal("worker-step");
        let mut runtime = ControlRuntime::open(&path).unwrap();
        runtime
            .enqueue_effect("effect-a", "adapter.refresh", b"payload-a", 3)
            .unwrap();
        let mut executor = ScriptedExecutor {
            outcomes: VecDeque::from([
                EffectExecution::Retry {
                    error: "temporary".into(),
                    after: Duration::ZERO,
                },
                EffectExecution::Complete(b"ok".to_vec()),
            ]),
        };
        assert_eq!(
            runtime
                .run_effect_once("worker-a", Duration::from_secs(30), &mut executor)
                .unwrap(),
            WorkerStep::RetryScheduled {
                effect_id: "effect-a".into(),
                attempt: 1,
            }
        );
        assert_eq!(
            runtime
                .run_effect_once("worker-a", Duration::from_secs(30), &mut executor)
                .unwrap(),
            WorkerStep::Completed {
                effect_id: "effect-a".into(),
                attempt: 2,
            }
        );

        runtime
            .enqueue_effect("effect-b", "adapter.unknown", b"payload-b", 3)
            .unwrap();
        let mut rejector = ScriptedExecutor {
            outcomes: VecDeque::from([EffectExecution::Reject {
                error: "adapter is not installed".into(),
            }]),
        };
        assert_eq!(
            runtime
                .run_effect_once("worker-a", Duration::from_secs(30), &mut rejector)
                .unwrap(),
            WorkerStep::Rejected {
                effect_id: "effect-b".into(),
                attempt: 1,
            }
        );
        assert_eq!(
            runtime
                .run_effect_once("worker-a", Duration::from_secs(30), &mut rejector)
                .unwrap(),
            WorkerStep::Idle
        );
        drop(runtime);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn status_effect_atomically_updates_projection_and_replays() {
        let path = temp_journal("status-effect-replay");
        {
            let mut runtime = ControlRuntime::open(&path).unwrap();
            let registered = runtime
                .register_runtime(
                    RuntimeId::new("runtime-a").unwrap(),
                    "Runtime A",
                    "http://127.0.0.1:9411",
                )
                .unwrap();
            let PlanResult::Command(refresh) = runtime
                .execute_plan(refresh_plan(registered.revision))
                .unwrap()
            else {
                panic!("runtime.refresh must return a command result");
            };
            let observation = RuntimeStatusObservation {
                runtime_id: "runtime-a".into(),
                expected_revision: refresh.runtime.revision,
                status: RuntimeStatusSnapshot {
                    status_source: "gewyvern-api".into(),
                    status_fetched_at: Some("1234".into()),
                    has_latest_snapshot: true,
                    target_count: Some(2),
                    ..RuntimeStatusSnapshot::default()
                },
            };
            let mut executor = ScriptedExecutor {
                outcomes: VecDeque::from([EffectExecution::Complete(
                    serde_json::to_vec(&observation).unwrap(),
                )]),
            };
            assert_eq!(
                runtime
                    .run_effect_once("worker-a", Duration::from_secs(30), &mut executor)
                    .unwrap(),
                WorkerStep::Completed {
                    effect_id: "command-a".into(),
                    attempt: 1,
                }
            );
            let PlanResult::Query(QueryResult::RuntimeList { runtimes, .. }) =
                runtime.execute_plan(list_plan()).unwrap()
            else {
                panic!("runtime.list must return a query result");
            };
            assert_eq!(runtimes[0].refresh_status, RefreshStatus::Ready);
            assert_eq!(runtimes[0].status.target_count, Some(2));
        }

        let mut recovered = ControlRuntime::open(&path).unwrap();
        let PlanResult::Query(QueryResult::RuntimeList { runtimes, .. }) =
            recovered.execute_plan(list_plan()).unwrap()
        else {
            panic!("runtime.list must return a query result");
        };
        assert_eq!(runtimes[0].refresh_status, RefreshStatus::Ready);
        assert_eq!(runtimes[0].status.status_source, "gewyvern-api");
        assert_eq!(runtimes[0].status.target_count, Some(2));
        drop(recovered);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn capability_effect_atomically_updates_projection_and_replays() {
        let path = temp_journal("capability-effect-replay");
        {
            let mut runtime = ControlRuntime::open(&path).unwrap();
            let registered = runtime
                .register_runtime(
                    RuntimeId::new("runtime-a").unwrap(),
                    "Runtime A",
                    "http://127.0.0.1:9411",
                )
                .unwrap();
            runtime
                .enqueue_effect(
                    "capability-a",
                    RUNTIME_CAPABILITY_DISCOVERY_EFFECT_KIND,
                    br#"{"runtime_id":"runtime-a","expected_revision":1}"#,
                    3,
                )
                .unwrap();
            let observation = RuntimeCapabilityObservation {
                runtime_id: "runtime-a".into(),
                expected_revision: registered.revision,
                capabilities: capability_snapshot(),
            };
            let mut executor = ScriptedExecutor {
                outcomes: VecDeque::from([EffectExecution::Complete(
                    serde_json::to_vec(&observation).unwrap(),
                )]),
            };
            assert_eq!(
                runtime
                    .run_effect_once("worker-a", Duration::from_secs(30), &mut executor)
                    .unwrap(),
                WorkerStep::Completed {
                    effect_id: "capability-a".into(),
                    attempt: 1,
                }
            );
            let (_, runtimes) = runtime.runtime_event_state();
            assert_eq!(runtimes[0].revision, Revision(2));
            assert_eq!(runtimes[0].capabilities, capability_snapshot());
            assert_eq!(
                runtimes[0].capabilities_observed_for_revision,
                Some(Revision(1))
            );
        }

        let recovered = ControlRuntime::open(&path).unwrap();
        let (_, runtimes) = recovered.runtime_event_state();
        assert_eq!(runtimes[0].revision, Revision(2));
        assert_eq!(runtimes[0].capabilities, capability_snapshot());
        assert_eq!(
            runtimes[0].capabilities_observed_for_revision,
            Some(Revision(1))
        );
        drop(recovered);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn legacy_capability_outcome_replays_with_observation_binding() {
        let path = temp_journal("legacy-capability-binding");
        {
            let mut runtime = ControlRuntime::open(&path).unwrap();
            let registered = runtime
                .register_runtime(
                    RuntimeId::new("runtime-a").unwrap(),
                    "Runtime A",
                    "http://127.0.0.1:9411",
                )
                .unwrap();
            runtime
                .enqueue_effect(
                    "capability-a",
                    RUNTIME_CAPABILITY_DISCOVERY_EFFECT_KIND,
                    br#"{"runtime_id":"runtime-a","expected_revision":1}"#,
                    3,
                )
                .unwrap();
            let observation = RuntimeCapabilityObservation {
                runtime_id: "runtime-a".into(),
                expected_revision: registered.revision,
                capabilities: capability_snapshot(),
            };
            let mut executor = ScriptedExecutor {
                outcomes: VecDeque::from([EffectExecution::Complete(
                    serde_json::to_vec(&observation).unwrap(),
                )]),
            };
            runtime
                .run_effect_once("worker-a", Duration::from_secs(30), &mut executor)
                .unwrap();
        }
        let connection = Connection::open(&path).unwrap();
        let outcome: Vec<u8> = connection
            .query_row(
                "SELECT outcome FROM runtime_journal WHERE kind = 'runtime_capability_observation'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let mut legacy: serde_json::Value = serde_json::from_slice(&outcome).unwrap();
        legacy
            .as_object_mut()
            .unwrap()
            .remove("capabilities_observed_for_revision");
        connection
            .execute(
                "UPDATE runtime_journal SET outcome = ?1 WHERE kind = 'runtime_capability_observation'",
                [serde_json::to_vec(&legacy).unwrap()],
            )
            .unwrap();
        drop(connection);

        let recovered = ControlRuntime::open(&path).unwrap();
        let (_, runtimes) = recovered.runtime_event_state();
        assert_eq!(
            runtimes[0].capabilities_observed_for_revision,
            Some(Revision(1))
        );
        drop(recovered);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn stale_capability_effect_is_rejected_without_projection_mutation() {
        let path = temp_journal("capability-effect-stale");
        let mut runtime = ControlRuntime::open(&path).unwrap();
        runtime
            .register_runtime(
                RuntimeId::new("runtime-a").unwrap(),
                "Runtime A",
                "http://127.0.0.1:9411",
            )
            .unwrap();
        runtime
            .enqueue_effect(
                "capability-stale",
                RUNTIME_CAPABILITY_DISCOVERY_EFFECT_KIND,
                br#"{"runtime_id":"runtime-a","expected_revision":99}"#,
                3,
            )
            .unwrap();
        let observation = RuntimeCapabilityObservation {
            runtime_id: "runtime-a".into(),
            expected_revision: Revision(99),
            capabilities: capability_snapshot(),
        };
        let mut executor = ScriptedExecutor {
            outcomes: VecDeque::from([EffectExecution::Complete(
                serde_json::to_vec(&observation).unwrap(),
            )]),
        };
        assert!(matches!(
            runtime
                .run_effect_once("worker-a", Duration::from_secs(30), &mut executor)
                .unwrap(),
            WorkerStep::Rejected { .. }
        ));
        let (_, runtimes) = runtime.runtime_event_state();
        assert_eq!(runtimes[0].revision, Revision(1));
        assert_eq!(
            runtimes[0].capabilities,
            RuntimeCapabilitySnapshot::default()
        );
        drop(runtime);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn restart_repairs_missing_effect_for_applied_refresh_command() {
        let path = temp_journal("refresh-effect-repair");
        {
            let mut runtime = ControlRuntime::open(&path).unwrap();
            let registered = runtime
                .register_runtime(
                    RuntimeId::new("runtime-a").unwrap(),
                    "Runtime A",
                    "http://127.0.0.1:9411",
                )
                .unwrap();
            runtime
                .execute_plan(refresh_plan(registered.revision))
                .unwrap();
            assert_eq!(runtime.effect_queue_stats().unwrap().ready, 1);
            Connection::open(&path)
                .unwrap()
                .execute(
                    "DELETE FROM runtime_effect_tasks WHERE effect_id = 'command-a'",
                    [],
                )
                .unwrap();
        }

        let mut recovered = ControlRuntime::open(&path).unwrap();
        assert_eq!(recovered.effect_queue_stats().unwrap().ready, 1);
        let lease = recovered
            .claim_effect("repair-worker", Duration::from_secs(30))
            .unwrap()
            .unwrap();
        assert_eq!(lease.effect_id, "command-a");
        assert_eq!(lease.kind, RUNTIME_STATUS_REFRESH_EFFECT_KIND);
        let request: RuntimeStatusRefreshRequest = serde_json::from_slice(&lease.payload).unwrap();
        assert_eq!(request.runtime_id, "runtime-a");
        assert_eq!(request.expected_revision, Revision(2));
        drop(recovered);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn capabilities_refresh_command_schedules_a_revision_fenced_discovery() {
        let path = temp_journal("capabilities-refresh-command");
        let mut runtime = ControlRuntime::open(&path).unwrap();
        let registered = runtime
            .register_runtime(
                RuntimeId::new("runtime-a").unwrap(),
                "Runtime A",
                "http://127.0.0.1:9411",
            )
            .unwrap();

        runtime
            .execute_plan(capabilities_refresh_plan(registered.revision))
            .unwrap();
        let lease = runtime
            .claim_effect("worker-a", Duration::from_secs(30))
            .unwrap()
            .unwrap();
        assert_eq!(lease.kind, RUNTIME_CAPABILITY_DISCOVERY_EFFECT_KIND);
        let request: RuntimeCapabilityRefreshRequest =
            serde_json::from_slice(&lease.payload).unwrap();
        assert_eq!(request.runtime_id, "runtime-a");
        assert_eq!(request.expected_revision, Revision(2));

        drop(runtime);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn deployment_command_schedules_only_the_typed_confirmed_intent() {
        let path = temp_journal("deployment-command");
        let mut runtime = ControlRuntime::open(&path).unwrap();
        let registered = runtime
            .register_runtime(
                RuntimeId::new("runtime-a").unwrap(),
                "Runtime A",
                "http://127.0.0.1:9411",
            )
            .unwrap();
        let mut lowering = context();
        lowering.capabilities = CapabilitySet::new([CAPABILITY_RUNTIME_DEPLOY]);
        lowering.expected_revision = Some(registered.revision);
        lowering.idempotency_key = IdempotencyKey::new("deploy-request").unwrap();
        lowering.confirmation = Confirmation::Confirmed;
        let plan =
            plan_runtime_deploy(&registered.id, "http/request", Some("pid:42"), &lowering).unwrap();

        runtime.execute_plan(plan).unwrap();
        let lease = runtime
            .claim_effect("worker-a", Duration::from_secs(30))
            .unwrap()
            .unwrap();
        assert_eq!(lease.kind, RUNTIME_DEPLOYMENT_EFFECT_KIND);
        let request: RuntimeDeploymentRequest = serde_json::from_slice(&lease.payload).unwrap();
        assert_eq!(request.runtime_id, "runtime-a");
        assert_eq!(request.request_id, "deploy-request");
        assert_eq!(request.pipeline_kind, "http/request");
        assert_eq!(request.requested_by, "operator-a");
        assert!(request.confirmed);
        assert_eq!(request.target.as_deref(), Some("pid:42"));

        drop(runtime);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn malformed_status_effect_outcome_is_rejected_without_stopping_worker() {
        let path = temp_journal("status-effect-invalid");
        let mut runtime = ControlRuntime::open(&path).unwrap();
        runtime
            .enqueue_effect(
                "status-invalid",
                RUNTIME_STATUS_REFRESH_EFFECT_KIND,
                br#"{}"#,
                3,
            )
            .unwrap();
        let mut executor = ScriptedExecutor {
            outcomes: VecDeque::from([EffectExecution::Complete(b"not-json".to_vec())]),
        };
        assert_eq!(
            runtime
                .run_effect_once("worker-a", Duration::from_secs(30), &mut executor)
                .unwrap(),
            WorkerStep::Rejected {
                effect_id: "status-invalid".into(),
                attempt: 1,
            }
        );
        assert_eq!(
            runtime
                .run_effect_once("worker-a", Duration::from_secs(30), &mut executor)
                .unwrap(),
            WorkerStep::Idle
        );
        drop(runtime);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn sqlite_journal_rejects_unknown_schema_and_divergent_outcome() {
        let schema_path = temp_journal("unknown-schema");
        drop(ControlRuntime::open(&schema_path).unwrap());
        Connection::open(&schema_path)
            .unwrap()
            .execute(
                "UPDATE runtime_metadata SET value = 999 WHERE key = 'schema_version'",
                [],
            )
            .unwrap();
        assert!(matches!(
            ControlRuntime::open(&schema_path),
            Err(RuntimeError::Storage(ref error)) if error.contains("unsupported runtime journal schema 999")
        ));
        fs::remove_file(schema_path).unwrap();

        let outcome_path = temp_journal("divergent-outcome");
        {
            let mut runtime = ControlRuntime::open(&outcome_path).unwrap();
            let projection = runtime
                .register_runtime(
                    RuntimeId::new("runtime-a").unwrap(),
                    "Runtime A",
                    "https://runtime-a.invalid",
                )
                .unwrap();
            runtime
                .execute_plan(refresh_plan(projection.revision))
                .unwrap();
        }
        Connection::open(&outcome_path)
            .unwrap()
            .execute(
                "UPDATE runtime_journal SET outcome = x'7b7d' WHERE kind = 'command_plan'",
                [],
            )
            .unwrap();
        assert!(matches!(
            ControlRuntime::open(&outcome_path),
            Err(RuntimeError::ReplayMismatch { .. })
        ));
        fs::remove_file(outcome_path).unwrap();
    }

    #[test]
    fn event_state_is_revisioned_and_stably_sorted() {
        let path = temp_journal("event-state");
        let mut runtime = ControlRuntime::open(&path).unwrap();
        runtime
            .register_runtime(
                RuntimeId::new("runtime-b").unwrap(),
                "B",
                "https://b.invalid",
            )
            .unwrap();
        runtime
            .register_runtime(
                RuntimeId::new("runtime-a").unwrap(),
                "A",
                "https://a.invalid",
            )
            .unwrap();
        let (revision, runtimes) = runtime.runtime_event_state();
        assert_eq!(revision, Revision(2));
        assert_eq!(
            runtimes
                .iter()
                .map(|runtime| runtime.id.as_str())
                .collect::<Vec<_>>(),
            ["runtime-a", "runtime-b"]
        );
        drop(runtime);
        fs::remove_file(path).unwrap();
    }
}
