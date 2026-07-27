use leserpent_domain::bootstrap::{
    BootstrapError, BootstrapId, BootstrapPhase, DaemonSessionProof, DeploymentBootstrap,
    DeploymentBootstrapCheckpoint, DeploymentBootstrapSnapshot,
};
use leserpent_domain::provisioning::{
    PROVISIONING_SERVICE_PROTOCOL_VERSION, ProvisioningError, ProvisioningId, ProvisioningPhase,
    RuntimeProvisioning, RuntimeProvisioningCheckpoint, RuntimeProvisioningSnapshot,
    RuntimeRegistrationProof,
};
use leserpent_domain::retirement::{
    RetirementError, RetirementId, RetirementPhase, RuntimeRetirement, RuntimeRetirementCheckpoint,
    RuntimeRetirementSnapshot,
};
use leserpent_domain::{
    Command, CommandId, CommandPlan, CommandPlanError, CommandResult, CommandStatus,
    DOMAIN_SNAPSHOT_SCHEMA_VERSION, DomainError, DomainEvent, DomainSnapshot, DomainSnapshotError,
    InMemoryControlPlane, MAX_RUNTIME_LOG_MESSAGE_BYTES, MAX_RUNTIME_LOG_QUERY_ENTRIES,
    PlannedOperation, Query, QueryEnvelope, QueryResult, RUNTIME_CAPABILITY_DISCOVERY_EFFECT_KIND,
    RUNTIME_DEPLOYMENT_EFFECT_KIND, RUNTIME_STATUS_REFRESH_EFFECT_KIND, Revision,
    RuntimeCapabilityObservation, RuntimeCapabilityRefreshRequest, RuntimeDeploymentOutcome,
    RuntimeDeploymentRequest, RuntimeId, RuntimeLogLevel, RuntimeLogRecord, RuntimeProjection,
    RuntimeStatusObservation, RuntimeStatusRefreshRequest,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;
use std::path::Path;
use std::time::Duration;

mod persistence;

pub use persistence::{
    EffectLease, OrchestraDeleteRecord, OrchestraHistoryRecord, OrchestraPersistenceRecord,
};
use persistence::{
    EffectRecord, Journal, JournalEntryKind, ORCHESTRA_DELETE_REPLAY_HORIZON_PINNED_ERROR,
};

pub const EFFECT_QUEUE_CAPACITY: u64 = 10_000;
pub const MAX_EFFECT_ENQUEUE_BATCH: usize = 1_000;
pub const MAX_PERSISTED_RUNTIME_LOG_ENTRIES: usize = 4_096;
pub const RUNTIME_UNREGISTRATION_REPLAY_HORIZON: usize = 256;
pub const ORCHESTRA_DELETE_REPLAY_HORIZON: usize = 4_096;
pub const ORCHESTRA_DELETE_REPLAY_WARNING_AVAILABLE_CAPACITY: u64 = 512;
pub const ORCHESTRA_DELETE_REPLAY_CRITICAL_AVAILABLE_CAPACITY: u64 = 128;
pub const ORCHESTRA_DELETE_REPLAY_WARNING_RECOVERY_AVAILABLE_CAPACITY: u64 = 768;
pub const ORCHESTRA_DELETE_REPLAY_CRITICAL_RECOVERY_AVAILABLE_CAPACITY: u64 = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OrchestraDeleteReplayAdmissionPressure {
    Healthy,
    Warning,
    Critical,
    Blocked,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeUnregistrationReplayHorizon {
    pub capacity: u64,
    pub retained: u64,
    pub oldest_generation: Option<u64>,
    pub newest_generation: Option<u64>,
    pub next_generation: u64,
    pub evicted_through_generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OrchestraDeleteReplayHorizon {
    pub capacity: u64,
    pub retained: u64,
    pub oldest_generation: Option<u64>,
    pub newest_generation: Option<u64>,
    pub next_generation: u64,
    pub evicted_through_generation: u64,
    pub protected_from_generation: Option<u64>,
    pub checkpointed_through_generation: Option<u64>,
}

impl OrchestraDeleteReplayHorizon {
    pub fn available_capacity(self) -> u64 {
        self.capacity.saturating_sub(self.retained)
    }

    pub fn saturated(self) -> bool {
        self.available_capacity() == 0
    }

    pub fn admission_blocked(self) -> bool {
        self.saturated() && self.protected_from_generation.is_some()
    }

    pub fn admission_pressure(self) -> OrchestraDeleteReplayAdmissionPressure {
        if self.protected_from_generation.is_none() {
            return OrchestraDeleteReplayAdmissionPressure::Healthy;
        }
        match self.available_capacity() {
            0 => OrchestraDeleteReplayAdmissionPressure::Blocked,
            available if available <= ORCHESTRA_DELETE_REPLAY_CRITICAL_AVAILABLE_CAPACITY => {
                OrchestraDeleteReplayAdmissionPressure::Critical
            }
            available if available <= ORCHESTRA_DELETE_REPLAY_WARNING_AVAILABLE_CAPACITY => {
                OrchestraDeleteReplayAdmissionPressure::Warning
            }
            _ => OrchestraDeleteReplayAdmissionPressure::Healthy,
        }
    }

    pub fn admission_pressure_with_hysteresis(
        self,
        previous: OrchestraDeleteReplayAdmissionPressure,
    ) -> OrchestraDeleteReplayAdmissionPressure {
        if self.protected_from_generation.is_none() {
            return OrchestraDeleteReplayAdmissionPressure::Healthy;
        }
        let available = self.available_capacity();
        if available == 0 {
            return OrchestraDeleteReplayAdmissionPressure::Blocked;
        }
        if available <= ORCHESTRA_DELETE_REPLAY_CRITICAL_AVAILABLE_CAPACITY {
            return OrchestraDeleteReplayAdmissionPressure::Critical;
        }
        if matches!(
            previous,
            OrchestraDeleteReplayAdmissionPressure::Critical
                | OrchestraDeleteReplayAdmissionPressure::Blocked
        ) && available <= ORCHESTRA_DELETE_REPLAY_CRITICAL_RECOVERY_AVAILABLE_CAPACITY
        {
            return OrchestraDeleteReplayAdmissionPressure::Critical;
        }
        if available <= ORCHESTRA_DELETE_REPLAY_WARNING_AVAILABLE_CAPACITY {
            return OrchestraDeleteReplayAdmissionPressure::Warning;
        }
        if previous != OrchestraDeleteReplayAdmissionPressure::Healthy
            && available <= ORCHESTRA_DELETE_REPLAY_WARNING_RECOVERY_AVAILABLE_CAPACITY
        {
            return OrchestraDeleteReplayAdmissionPressure::Warning;
        }
        OrchestraDeleteReplayAdmissionPressure::Healthy
    }

    pub fn checkpoint_lag_generations(self) -> u64 {
        match (self.newest_generation, self.checkpointed_through_generation) {
            (Some(newest), Some(checkpointed)) => newest.saturating_sub(checkpointed),
            (Some(_), None) => self.retained,
            (None, _) => 0,
        }
    }

    pub fn operator_action_required(self) -> bool {
        self.admission_pressure() != OrchestraDeleteReplayAdmissionPressure::Healthy
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeUnregisterTarget {
    pub runtime_id: RuntimeId,
    pub expected_revision: Revision,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeUnregisterResult {
    pub command_id: CommandId,
    pub operation_generation: u64,
    pub removed: Vec<RuntimeUnregisterTarget>,
    pub deleted_orchestra_runtime_count: u32,
    pub deleted_orchestra_run_count: u64,
    pub deleted_orchestra_event_count: u64,
    pub removed_at_unix_ms: i64,
    pub replayed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OrchestraDeleteCommandResult {
    pub command_id: CommandId,
    pub operation_generation: u64,
    pub runtime_ids: Vec<String>,
    pub deleted_runtime_count: u32,
    pub deleted_run_count: u64,
    pub deleted_event_count: u64,
    pub committed_at_unix_ms: i64,
    pub replayed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeUnregistrationReceipt {
    pub operation_generation: u64,
    pub removed: Vec<RuntimeUnregisterTarget>,
    pub deleted_orchestra_runtime_count: u32,
    pub deleted_orchestra_run_count: u64,
    pub deleted_orchestra_event_count: u64,
    pub removed_at_unix_ms: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeUnregistrationReceiptLookup {
    pub command_id: CommandId,
    pub receipt: Option<RuntimeUnregistrationReceipt>,
    pub replay_horizon: RuntimeUnregistrationReplayHorizon,
}

fn command_runtime_id(command: &Command) -> Option<&RuntimeId> {
    match command {
        Command::RuntimeRegister { runtime_id, .. }
        | Command::RuntimeRegistrationUpdate { runtime_id, .. }
        | Command::RuntimeDiscoveryIntake { runtime_id, .. }
        | Command::RuntimeRefresh { runtime_id }
        | Command::RuntimeCapabilitiesRefresh { runtime_id }
        | Command::RuntimeDeploy { runtime_id, .. } => Some(runtime_id),
        Command::DebuggerCancel { .. } => None,
    }
}

fn stamp_runtime(
    control: &mut InMemoryControlPlane,
    runtime_id: &RuntimeId,
    registered: bool,
    timestamp_unix_ms: i64,
) -> Result<RuntimeProjection, RuntimeError> {
    let timestamp_unix_ms = u64::try_from(timestamp_unix_ms)
        .map_err(|_| RuntimeError::Storage("runtime authority timestamp is negative".into()))?;
    control
        .stamp_runtime_authority_time(runtime_id, registered, timestamp_unix_ms)
        .map_err(RuntimeError::Domain)
}

fn stamp_result_if_mutated(
    control: &mut InMemoryControlPlane,
    result: &CommandResult,
    prior_revision: Option<Revision>,
    timestamp_unix_ms: i64,
) -> Result<(), RuntimeError> {
    if result.status == CommandStatus::Applied && prior_revision != Some(result.runtime.revision) {
        stamp_runtime(
            control,
            &result.runtime.id,
            prior_revision.is_none(),
            timestamp_unix_ms,
        )?;
    }
    Ok(())
}

fn unstamped_runtime_projection(runtime: &RuntimeProjection) -> RuntimeProjection {
    let mut runtime = runtime.clone();
    runtime.registered_at_unix_ms = None;
    runtime.updated_at_unix_ms = None;
    runtime
}

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
    Provisioning(ProvisioningError),
    Retirement(RetirementError),
    InvalidSnapshot(DomainSnapshotError),
    OrchestraDeleteReplayHorizonSaturated,
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
            Self::Provisioning(error) => {
                write!(formatter, "runtime provisioning state failed: {error}")
            }
            Self::Retirement(error) => {
                write!(formatter, "runtime retirement state failed: {error}")
            }
            Self::InvalidSnapshot(error) => write!(formatter, "invalid runtime snapshot: {error}"),
            Self::OrchestraDeleteReplayHorizonSaturated => {
                formatter.write_str(ORCHESTRA_DELETE_REPLAY_HORIZON_PINNED_ERROR)
            }
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
            Self::Provisioning(error) => Some(error),
            Self::Retirement(error) => Some(error),
            Self::InvalidSnapshot(error) => Some(error),
            Self::InvalidEffectOutcome(_)
            | Self::OrchestraDeleteReplayHorizonSaturated
            | Self::Storage(_)
            | Self::ReplayMismatch { .. } => None,
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct RuntimeUnregistration {
    runtime_id: String,
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
                    let runtime_id =
                        RuntimeId::new(registration.runtime_id).map_err(RuntimeError::Domain)?;
                    runtime.control.register_runtime(
                        runtime_id.clone(),
                        registration.name,
                        registration.endpoint,
                    );
                    stamp_runtime(
                        &mut runtime.control,
                        &runtime_id,
                        true,
                        entry.created_at_unix_ms,
                    )?;
                }
                JournalEntryKind::RuntimeUnregistration => {
                    let unregistration: RuntimeUnregistration =
                        serde_json::from_slice(&entry.payload)
                            .map_err(|error| RuntimeError::Storage(error.to_string()))?;
                    let runtime_id =
                        RuntimeId::new(unregistration.runtime_id).map_err(RuntimeError::Domain)?;
                    if !runtime.control.unregister_runtime(&runtime_id) {
                        return Err(RuntimeError::ReplayMismatch {
                            sequence: entry.sequence,
                        });
                    }
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
                    let prior_revision = runtime
                        .control
                        .runtime_projection(command_runtime_id(&command.command).ok_or_else(
                            || {
                                RuntimeError::Storage(
                                    "journal contains an unsupported debugger command".into(),
                                )
                            },
                        )?)
                        .map(|projection| projection.revision);
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
                    stamp_result_if_mutated(
                        &mut runtime.control,
                        &result,
                        prior_revision,
                        entry.created_at_unix_ms,
                    )?;
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
                    let encoded = serde_json::to_vec(&unstamped_runtime_projection(&projection))
                        .map_err(|error| RuntimeError::Storage(error.to_string()))?;
                    if entry.outcome.as_deref() != Some(encoded.as_slice()) {
                        return Err(RuntimeError::ReplayMismatch {
                            sequence: entry.sequence,
                        });
                    }
                    stamp_runtime(
                        &mut runtime.control,
                        &runtime_id,
                        false,
                        entry.created_at_unix_ms,
                    )?;
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
                    let encoded = serde_json::to_vec(&unstamped_runtime_projection(&projection))
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
                        if legacy != unstamped_runtime_projection(&projection) {
                            return Err(RuntimeError::ReplayMismatch {
                                sequence: entry.sequence,
                            });
                        }
                    }
                    stamp_runtime(
                        &mut runtime.control,
                        &runtime_id,
                        false,
                        entry.created_at_unix_ms,
                    )?;
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

    pub fn enqueue_bootstrap_effect(
        &mut self,
        effect_id: &str,
        kind: &str,
        payload: &[u8],
        max_attempts: u32,
        checkpoint: &DeploymentBootstrapCheckpoint,
    ) -> Result<(), RuntimeError> {
        checkpoint.validate().map_err(RuntimeError::Bootstrap)?;
        if checkpoint.revision != 1 || checkpoint.state.phase != BootstrapPhase::Planned {
            return Err(RuntimeError::InvalidEffectOutcome(
                "bootstrap submission must begin at planned revision 1",
            ));
        }
        let checkpoint_payload = serde_json::to_vec(checkpoint)
            .map_err(|error| RuntimeError::Storage(error.to_string()))?;
        let Some(journal) = &mut self.journal else {
            return Err(RuntimeError::Storage(
                "bootstrap submission requires persistent storage".into(),
            ));
        };
        journal
            .enqueue_effect_with_authority_checkpoint(
                effect_id,
                kind,
                payload,
                max_attempts,
                persistence::AUTHORITY_KIND_DAEMON_BOOTSTRAP,
                checkpoint.state.bootstrap_id.as_str(),
                bootstrap_phase_label(checkpoint.state.phase),
                checkpoint.revision,
                &checkpoint_payload,
            )
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

    pub fn runtime_unregistration_replay_horizon(
        &mut self,
    ) -> Result<RuntimeUnregistrationReplayHorizon, RuntimeError> {
        let Some(journal) = &mut self.journal else {
            return Ok(RuntimeUnregistrationReplayHorizon {
                capacity: u64::try_from(RUNTIME_UNREGISTRATION_REPLAY_HORIZON)
                    .map_err(|_| RuntimeError::Storage("replay horizon is invalid".into()))?,
                retained: 0,
                oldest_generation: None,
                newest_generation: None,
                next_generation: 1,
                evicted_through_generation: 0,
            });
        };
        journal
            .runtime_unregistration_replay_horizon()
            .map_err(RuntimeError::Storage)
    }

    pub fn runtime_unregistration_receipt(
        &mut self,
        command_id: CommandId,
    ) -> Result<RuntimeUnregistrationReceiptLookup, RuntimeError> {
        let Some(journal) = &mut self.journal else {
            return Ok(RuntimeUnregistrationReceiptLookup {
                command_id,
                receipt: None,
                replay_horizon: RuntimeUnregistrationReplayHorizon {
                    capacity: u64::try_from(RUNTIME_UNREGISTRATION_REPLAY_HORIZON)
                        .map_err(|_| RuntimeError::Storage("replay horizon is invalid".into()))?,
                    retained: 0,
                    oldest_generation: None,
                    newest_generation: None,
                    next_generation: 1,
                    evicted_through_generation: 0,
                },
            });
        };
        let lookup = journal
            .runtime_unregistration_receipt_lookup(command_id.as_str())
            .map_err(RuntimeError::Storage)?;
        let receipt = lookup
            .operation
            .map(|record| {
                let removed: Vec<RuntimeUnregisterTarget> = serde_json::from_slice(&record.request)
                    .map_err(|error| RuntimeError::Storage(error.to_string()))?;
                if removed.iter().any(|target| {
                    self.control
                        .runtime_projection(&target.runtime_id)
                        .is_some()
                }) {
                    return Err(RuntimeError::Storage(
                        "runtime unregistration projection tombstone is inconsistent".into(),
                    ));
                }
                Ok(RuntimeUnregistrationReceipt {
                    operation_generation: record.generation,
                    removed,
                    deleted_orchestra_runtime_count: record.deleted_runtime_count,
                    deleted_orchestra_run_count: record.deleted_run_count,
                    deleted_orchestra_event_count: record.deleted_event_count,
                    removed_at_unix_ms: record.removed_at_unix_ms,
                })
            })
            .transpose()?;
        Ok(RuntimeUnregistrationReceiptLookup {
            command_id,
            receipt,
            replay_horizon: lookup.replay_horizon,
        })
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
        from_outcome: Option<&str>,
        to_outcome: &str,
        run_outcome: &str,
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
                from_outcome,
                to_outcome,
                run_outcome,
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

    pub fn orchestra_delete_replay_horizon(
        &mut self,
    ) -> Result<OrchestraDeleteReplayHorizon, RuntimeError> {
        let Some(journal) = &mut self.journal else {
            return Ok(OrchestraDeleteReplayHorizon {
                capacity: u64::try_from(ORCHESTRA_DELETE_REPLAY_HORIZON)
                    .map_err(|_| RuntimeError::Storage("replay horizon is invalid".into()))?,
                retained: 0,
                oldest_generation: None,
                newest_generation: None,
                next_generation: 1,
                evicted_through_generation: 0,
                protected_from_generation: None,
                checkpointed_through_generation: None,
            });
        };
        journal
            .orchestra_delete_replay_horizon()
            .map_err(RuntimeError::Storage)
    }

    pub fn checkpoint_orchestra_delete_replay_horizon(
        &mut self,
        minimum_retained_generation: u64,
        observed_through_generation: u64,
    ) -> Result<OrchestraDeleteReplayHorizon, RuntimeError> {
        let Some(journal) = &mut self.journal else {
            return Err(RuntimeError::Storage(
                "Orchestra delete replay checkpoint requires persistent storage".into(),
            ));
        };
        journal
            .checkpoint_orchestra_delete_replay_horizon(
                minimum_retained_generation,
                observed_through_generation,
            )
            .map_err(RuntimeError::Storage)
    }

    pub fn delete_orchestra_runtimes_idempotent(
        &mut self,
        command_id: CommandId,
        runtime_ids: &[String],
    ) -> Result<OrchestraDeleteCommandResult, RuntimeError> {
        let Some(journal) = &mut self.journal else {
            return Err(RuntimeError::Storage(
                "idempotent Orchestra delete requires persistent storage".into(),
            ));
        };
        let (record, replayed) = journal
            .delete_orchestra_runtimes_idempotent(command_id.as_str(), runtime_ids)
            .map_err(|error| {
                if error == "Orchestra delete operation idempotency conflict" {
                    RuntimeError::Domain(DomainError::IdempotencyConflict {
                        key: command_id.as_str().to_string(),
                    })
                } else if error == ORCHESTRA_DELETE_REPLAY_HORIZON_PINNED_ERROR {
                    RuntimeError::OrchestraDeleteReplayHorizonSaturated
                } else {
                    RuntimeError::Storage(error)
                }
            })?;
        let runtime_ids: Vec<String> = serde_json::from_slice(&record.request)
            .map_err(|error| RuntimeError::Storage(error.to_string()))?;
        Ok(OrchestraDeleteCommandResult {
            command_id,
            operation_generation: record.generation,
            runtime_ids,
            deleted_runtime_count: record.deleted_runtime_count,
            deleted_run_count: record.deleted_run_count,
            deleted_event_count: record.deleted_event_count,
            committed_at_unix_ms: record.committed_at_unix_ms,
            replayed,
        })
    }

    pub fn unregister_runtimes(
        &mut self,
        command_id: CommandId,
        targets: Vec<RuntimeUnregisterTarget>,
    ) -> Result<RuntimeUnregisterResult, RuntimeError> {
        if targets.is_empty() || targets.len() > 128 {
            return Err(RuntimeError::Domain(DomainError::InvalidQuery {
                reason: "runtime unregistration requires between 1 and 128 targets",
            }));
        }
        let mut unique = BTreeSet::new();
        for target in &targets {
            if !unique.insert(target.runtime_id.clone()) {
                return Err(RuntimeError::Domain(DomainError::InvalidQuery {
                    reason: "runtime unregistration targets must be unique",
                }));
            }
        }
        let request = serde_json::to_vec(&targets)
            .map_err(|error| RuntimeError::Storage(error.to_string()))?;
        let Some(journal) = &mut self.journal else {
            return Err(RuntimeError::Storage(
                "runtime unregistration requires persistent storage".into(),
            ));
        };
        if let Some(record) = journal
            .runtime_unregistration_operation(command_id.as_str())
            .map_err(RuntimeError::Storage)?
        {
            if record.request != request {
                return Err(RuntimeError::Domain(DomainError::IdempotencyConflict {
                    key: command_id.as_str().to_string(),
                }));
            }
            if targets.iter().any(|target| {
                self.control
                    .runtime_projection(&target.runtime_id)
                    .is_some()
            }) {
                return Err(RuntimeError::Storage(
                    "runtime unregistration projection tombstone is inconsistent".into(),
                ));
            }
            return Ok(RuntimeUnregisterResult {
                command_id,
                operation_generation: record.generation,
                removed: targets,
                deleted_orchestra_runtime_count: record.deleted_runtime_count,
                deleted_orchestra_run_count: record.deleted_run_count,
                deleted_orchestra_event_count: record.deleted_event_count,
                removed_at_unix_ms: record.removed_at_unix_ms,
                replayed: true,
            });
        }

        let mut staged = self.control.clone();
        let mut runtime_ids = Vec::with_capacity(targets.len());
        let mut unregistrations = Vec::with_capacity(targets.len());
        for target in &targets {
            let projection = staged
                .runtime_projection(&target.runtime_id)
                .ok_or_else(|| {
                    RuntimeError::Domain(DomainError::RuntimeNotFound {
                        runtime_id: target.runtime_id.as_str().to_string(),
                    })
                })?;
            if projection.revision != target.expected_revision {
                return Err(RuntimeError::Domain(DomainError::RevisionConflict {
                    expected: target.expected_revision,
                    actual: projection.revision,
                }));
            }
            if !staged.unregister_runtime(&target.runtime_id) {
                return Err(RuntimeError::ReplayMismatch { sequence: 0 });
            }
            runtime_ids.push(target.runtime_id.as_str().to_string());
            unregistrations.push(
                serde_json::to_vec(&RuntimeUnregistration {
                    runtime_id: target.runtime_id.as_str().to_string(),
                })
                .map_err(|error| RuntimeError::Storage(error.to_string()))?,
            );
        }
        let record = journal
            .commit_runtime_unregistration_operation(
                command_id.as_str(),
                &request,
                &runtime_ids,
                &unregistrations,
            )
            .map_err(RuntimeError::Storage)?;
        self.control = staged;
        for target in &targets {
            self.ephemeral_logs.remove(&target.runtime_id);
        }
        Ok(RuntimeUnregisterResult {
            command_id,
            operation_generation: record.generation,
            removed: targets,
            deleted_orchestra_runtime_count: record.deleted_runtime_count,
            deleted_orchestra_run_count: record.deleted_run_count,
            deleted_orchestra_event_count: record.deleted_event_count,
            removed_at_unix_ms: record.removed_at_unix_ms,
            replayed: false,
        })
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
            .complete_effect_with_authority_checkpoint(
                lease,
                outcome,
                persistence::AUTHORITY_KIND_DAEMON_BOOTSTRAP,
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
            .authority_checkpoint(
                persistence::AUTHORITY_KIND_DAEMON_BOOTSTRAP,
                bootstrap_id.as_str(),
            )
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
            .update_authority_checkpoint(
                persistence::AUTHORITY_KIND_DAEMON_BOOTSTRAP,
                bootstrap_id.as_str(),
                checkpoint.revision,
                bootstrap_phase_label(next.state.phase),
                &payload,
            )
            .map_err(RuntimeError::Storage)?;
        Ok(state)
    }

    pub fn enqueue_provisioning_effect(
        &mut self,
        effect_id: &str,
        kind: &str,
        payload: &[u8],
        max_attempts: u32,
        checkpoint: &RuntimeProvisioningCheckpoint,
    ) -> Result<(), RuntimeError> {
        checkpoint.validate().map_err(RuntimeError::Provisioning)?;
        if checkpoint.revision != 1 || checkpoint.state.phase != ProvisioningPhase::Planned {
            return Err(RuntimeError::InvalidEffectOutcome(
                "provisioning submission must begin at planned revision 1",
            ));
        }
        let checkpoint_payload = serde_json::to_vec(checkpoint)
            .map_err(|error| RuntimeError::Storage(error.to_string()))?;
        let Some(journal) = &mut self.journal else {
            return Err(RuntimeError::Storage(
                "runtime provisioning requires persistent storage".into(),
            ));
        };
        journal
            .enqueue_effect_with_authority_checkpoint(
                effect_id,
                kind,
                payload,
                max_attempts,
                persistence::AUTHORITY_KIND_GEWYVERN_PROVISIONING,
                checkpoint.state.provisioning_id.as_str(),
                provisioning_phase_label(checkpoint.state.phase),
                checkpoint.revision,
                &checkpoint_payload,
            )
            .map_err(RuntimeError::Storage)
    }

    pub fn complete_provisioning_effect(
        &mut self,
        lease: &EffectLease,
        outcome: &[u8],
        checkpoint: &RuntimeProvisioningCheckpoint,
    ) -> Result<(), RuntimeError> {
        checkpoint.validate().map_err(RuntimeError::Provisioning)?;
        if !matches!(
            checkpoint.state.phase,
            ProvisioningPhase::ServiceReady | ProvisioningPhase::Failed
        ) {
            return Err(RuntimeError::InvalidEffectOutcome(
                "provisioning effect did not reach a terminal installation phase",
            ));
        }
        let payload = serde_json::to_vec(checkpoint)
            .map_err(|error| RuntimeError::Storage(error.to_string()))?;
        let Some(journal) = &mut self.journal else {
            return Err(RuntimeError::Storage(
                "runtime provisioning requires persistent storage".into(),
            ));
        };
        journal
            .complete_effect_with_authority_checkpoint(
                lease,
                outcome,
                persistence::AUTHORITY_KIND_GEWYVERN_PROVISIONING,
                checkpoint.state.provisioning_id.as_str(),
                provisioning_phase_label(checkpoint.state.phase),
                checkpoint.revision,
                &payload,
            )
            .map_err(RuntimeError::Storage)
    }

    pub fn provisioning_checkpoint(
        &mut self,
        provisioning_id: &ProvisioningId,
    ) -> Result<Option<RuntimeProvisioningCheckpoint>, RuntimeError> {
        let Some(journal) = &mut self.journal else {
            return Err(RuntimeError::Storage(
                "runtime provisioning requires persistent storage".into(),
            ));
        };
        let Some(record) = journal
            .authority_checkpoint(
                persistence::AUTHORITY_KIND_GEWYVERN_PROVISIONING,
                provisioning_id.as_str(),
            )
            .map_err(RuntimeError::Storage)?
        else {
            return Ok(None);
        };
        let checkpoint: RuntimeProvisioningCheckpoint = serde_json::from_slice(&record.payload)
            .map_err(|_| RuntimeError::Storage("provisioning checkpoint is invalid JSON".into()))?;
        checkpoint.validate().map_err(|error| {
            RuntimeError::Storage(format!("invalid provisioning checkpoint: {error}"))
        })?;
        if checkpoint.revision != record.revision
            || checkpoint.state.provisioning_id != *provisioning_id
            || provisioning_phase_label(checkpoint.state.phase) != record.phase
        {
            return Err(RuntimeError::Storage(
                "provisioning checkpoint identity or revision diverged".into(),
            ));
        }
        Ok(Some(checkpoint))
    }

    pub fn accept_provisioning_registration(
        &mut self,
        provisioning_id: &ProvisioningId,
        proof: RuntimeRegistrationProof,
    ) -> Result<RuntimeProvisioningSnapshot, RuntimeError> {
        let checkpoint = self
            .provisioning_checkpoint(provisioning_id)?
            .ok_or_else(|| RuntimeError::Storage("provisioning checkpoint was not found".into()))?;
        if checkpoint.state.phase == ProvisioningPhase::RuntimeRegistered {
            let mut provisioning =
                RuntimeProvisioning::resume(&checkpoint).map_err(RuntimeError::Provisioning)?;
            return provisioning
                .accept_registration(proof)
                .map_err(RuntimeError::Provisioning);
        }
        self.commit_provisioning_registration(&checkpoint, proof, None)
    }

    pub fn register_ready_provisioning(
        &mut self,
        provisioning_id: &ProvisioningId,
    ) -> Result<RuntimeProvisioningSnapshot, RuntimeError> {
        let checkpoint = self
            .provisioning_checkpoint(provisioning_id)?
            .ok_or_else(|| RuntimeError::Storage("provisioning checkpoint was not found".into()))?;
        if checkpoint.state.phase == ProvisioningPhase::RuntimeRegistered {
            return Ok(checkpoint.state);
        }
        let proof = registration_proof_from_ready(&checkpoint)?;
        self.commit_provisioning_registration(&checkpoint, proof, None)
    }

    pub fn complete_provisioning_effect_and_register(
        &mut self,
        lease: &EffectLease,
        outcome: &[u8],
        ready: &RuntimeProvisioningCheckpoint,
    ) -> Result<RuntimeProvisioningSnapshot, RuntimeError> {
        let proof = registration_proof_from_ready(ready)?;
        self.commit_provisioning_registration(ready, proof, Some((lease, outcome)))
    }

    fn commit_provisioning_registration(
        &mut self,
        ready: &RuntimeProvisioningCheckpoint,
        proof: RuntimeRegistrationProof,
        leased_effect: Option<(&EffectLease, &[u8])>,
    ) -> Result<RuntimeProvisioningSnapshot, RuntimeError> {
        ready.validate().map_err(RuntimeError::Provisioning)?;
        if ready.revision != 2 || ready.state.phase != ProvisioningPhase::ServiceReady {
            return Err(RuntimeError::InvalidEffectOutcome(
                "provisioning registration requires a revision-2 ready checkpoint",
            ));
        }
        let current = self
            .provisioning_checkpoint(&ready.state.provisioning_id)?
            .ok_or_else(|| RuntimeError::Storage("provisioning checkpoint was not found".into()))?;
        match leased_effect {
            Some(_) => {
                if current.revision != 1
                    || current.state.phase != ProvisioningPhase::Planned
                    || current.state.provisioning_id != ready.state.provisioning_id
                    || current.state.runtime_id != ready.state.runtime_id
                    || current.state.target != ready.state.target
                {
                    return Err(RuntimeError::InvalidEffectOutcome(
                        "planned provisioning checkpoint does not bind the ready service",
                    ));
                }
            }
            None if current != *ready => {
                return Err(RuntimeError::InvalidEffectOutcome(
                    "ready provisioning checkpoint changed before registration",
                ));
            }
            None => {}
        }
        let mut provisioning =
            RuntimeProvisioning::resume(ready).map_err(RuntimeError::Provisioning)?;
        let state = provisioning
            .accept_registration(proof)
            .map_err(RuntimeError::Provisioning)?;
        let registered = provisioning
            .checkpoint(3)
            .map_err(RuntimeError::Provisioning)?;
        let checkpoint_payload = serde_json::to_vec(&registered)
            .map_err(|error| RuntimeError::Storage(error.to_string()))?;
        let runtime_id = state.runtime_id.clone();
        let runtime_name = runtime_id.as_str().to_string();
        let endpoint = state
            .endpoint
            .clone()
            .ok_or(RuntimeError::InvalidEffectOutcome(
                "registered provisioning state has no endpoint",
            ))?;
        let mut staged = self.control.clone();
        let registration = match staged.runtime_projection(&runtime_id) {
            Some(existing) if existing.name == runtime_name && existing.endpoint == endpoint => {
                None
            }
            Some(_) => {
                return Err(RuntimeError::Storage(format!(
                    "runtime '{}' is already registered with different authority",
                    runtime_id.as_str()
                )));
            }
            None => {
                staged.register_runtime(runtime_id.clone(), runtime_name.clone(), endpoint.clone());
                Some(RuntimeRegistration {
                    runtime_id: runtime_id.as_str().to_string(),
                    name: runtime_name,
                    endpoint,
                })
            }
        };
        let registration_payload = registration
            .as_ref()
            .map(serde_json::to_vec)
            .transpose()
            .map_err(|error| RuntimeError::Storage(error.to_string()))?;
        let Some(journal) = &mut self.journal else {
            return Err(RuntimeError::Storage(
                "runtime provisioning requires persistent storage".into(),
            ));
        };
        let timestamp = journal
            .commit_provisioning_registration(
                leased_effect,
                registration_payload.as_deref(),
                state.provisioning_id.as_str(),
                current.revision,
                registered.revision,
                &checkpoint_payload,
            )
            .map_err(RuntimeError::Storage)?;
        if registration.is_some() {
            stamp_runtime(&mut staged, &runtime_id, true, timestamp)?;
        }
        self.control = staged;
        Ok(state)
    }

    pub fn enqueue_retirement_effect(
        &mut self,
        effect_id: &str,
        kind: &str,
        payload: &[u8],
        max_attempts: u32,
        checkpoint: &RuntimeRetirementCheckpoint,
    ) -> Result<(), RuntimeError> {
        checkpoint.validate().map_err(RuntimeError::Retirement)?;
        if checkpoint.revision != 1 || checkpoint.state.phase != RetirementPhase::Planned {
            return Err(RuntimeError::InvalidEffectOutcome(
                "retirement submission must begin at planned revision 1",
            ));
        }
        let registered_endpoint = self
            .control
            .runtime_projection(&checkpoint.state.runtime_id)
            .ok_or(RuntimeError::InvalidEffectOutcome(
                "retirement runtime is not registered",
            ))?
            .endpoint
            .clone();
        let provisioning = self
            .provisioning_checkpoint(&checkpoint.state.provisioning_id)?
            .ok_or(RuntimeError::InvalidEffectOutcome(
                "retirement provisioning authority was not found",
            ))?;
        if provisioning.state.phase != ProvisioningPhase::RuntimeRegistered
            || provisioning.state.runtime_id != checkpoint.state.runtime_id
            || provisioning.state.target != checkpoint.state.target
            || provisioning.state.endpoint.as_deref() != Some(registered_endpoint.as_str())
        {
            return Err(RuntimeError::InvalidEffectOutcome(
                "retirement identity does not match registered provisioning authority",
            ));
        }
        let checkpoint_payload = serde_json::to_vec(checkpoint)
            .map_err(|error| RuntimeError::Storage(error.to_string()))?;
        let Some(journal) = &mut self.journal else {
            return Err(RuntimeError::Storage(
                "runtime retirement requires persistent storage".into(),
            ));
        };
        journal
            .enqueue_effect_with_authority_checkpoint(
                effect_id,
                kind,
                payload,
                max_attempts,
                persistence::AUTHORITY_KIND_GEWYVERN_RETIREMENT,
                checkpoint.state.retirement_id.as_str(),
                retirement_phase_label(checkpoint.state.phase),
                checkpoint.revision,
                &checkpoint_payload,
            )
            .map_err(RuntimeError::Storage)
    }

    pub fn retirement_checkpoint(
        &mut self,
        retirement_id: &RetirementId,
    ) -> Result<Option<RuntimeRetirementCheckpoint>, RuntimeError> {
        let Some(journal) = &mut self.journal else {
            return Err(RuntimeError::Storage(
                "runtime retirement requires persistent storage".into(),
            ));
        };
        let Some(record) = journal
            .authority_checkpoint(
                persistence::AUTHORITY_KIND_GEWYVERN_RETIREMENT,
                retirement_id.as_str(),
            )
            .map_err(RuntimeError::Storage)?
        else {
            return Ok(None);
        };
        let checkpoint: RuntimeRetirementCheckpoint = serde_json::from_slice(&record.payload)
            .map_err(|_| RuntimeError::Storage("retirement checkpoint is invalid JSON".into()))?;
        checkpoint.validate().map_err(|error| {
            RuntimeError::Storage(format!("invalid retirement checkpoint: {error}"))
        })?;
        if checkpoint.revision != record.revision
            || checkpoint.state.retirement_id != *retirement_id
            || retirement_phase_label(checkpoint.state.phase) != record.phase
        {
            return Err(RuntimeError::Storage(
                "retirement checkpoint identity or revision diverged".into(),
            ));
        }
        Ok(Some(checkpoint))
    }

    pub fn complete_retirement_effect(
        &mut self,
        lease: &EffectLease,
        outcome: &[u8],
        checkpoint: &RuntimeRetirementCheckpoint,
    ) -> Result<(), RuntimeError> {
        checkpoint.validate().map_err(RuntimeError::Retirement)?;
        if checkpoint.revision != 2 || checkpoint.state.phase != RetirementPhase::Failed {
            return Err(RuntimeError::InvalidEffectOutcome(
                "retirement effect completion requires a revision-2 failed checkpoint",
            ));
        }
        let payload = serde_json::to_vec(checkpoint)
            .map_err(|error| RuntimeError::Storage(error.to_string()))?;
        let Some(journal) = &mut self.journal else {
            return Err(RuntimeError::Storage(
                "runtime retirement requires persistent storage".into(),
            ));
        };
        journal
            .complete_effect_with_authority_checkpoint(
                lease,
                outcome,
                persistence::AUTHORITY_KIND_GEWYVERN_RETIREMENT,
                checkpoint.state.retirement_id.as_str(),
                retirement_phase_label(checkpoint.state.phase),
                checkpoint.revision,
                &payload,
            )
            .map_err(RuntimeError::Storage)
    }

    pub fn complete_retirement_effect_and_unregister(
        &mut self,
        lease: &EffectLease,
        outcome: &[u8],
        service_retired: &RuntimeRetirementCheckpoint,
    ) -> Result<RuntimeRetirementSnapshot, RuntimeError> {
        service_retired
            .validate()
            .map_err(RuntimeError::Retirement)?;
        if service_retired.revision != 2
            || service_retired.state.phase != RetirementPhase::ServiceRetired
        {
            return Err(RuntimeError::InvalidEffectOutcome(
                "runtime unregistration requires a revision-2 service-retired checkpoint",
            ));
        }
        let current = self
            .retirement_checkpoint(&service_retired.state.retirement_id)?
            .ok_or_else(|| RuntimeError::Storage("retirement checkpoint was not found".into()))?;
        if current.revision != 1
            || current.state.phase != RetirementPhase::Planned
            || current.state.retirement_id != service_retired.state.retirement_id
            || current.state.provisioning_id != service_retired.state.provisioning_id
            || current.state.runtime_id != service_retired.state.runtime_id
            || current.state.target != service_retired.state.target
        {
            return Err(RuntimeError::InvalidEffectOutcome(
                "planned retirement checkpoint does not bind the retired service",
            ));
        }
        let mut retirement =
            RuntimeRetirement::resume(service_retired).map_err(RuntimeError::Retirement)?;
        let state = retirement
            .accept_runtime_unregistration()
            .map_err(RuntimeError::Retirement)?;
        let unregistered = retirement.checkpoint(3).map_err(RuntimeError::Retirement)?;
        let checkpoint_payload = serde_json::to_vec(&unregistered)
            .map_err(|error| RuntimeError::Storage(error.to_string()))?;
        let unregistration = RuntimeUnregistration {
            runtime_id: state.runtime_id.as_str().to_string(),
        };
        let unregistration_payload = serde_json::to_vec(&unregistration)
            .map_err(|error| RuntimeError::Storage(error.to_string()))?;
        let mut staged = self.control.clone();
        if !staged.unregister_runtime(&state.runtime_id) {
            return Err(RuntimeError::InvalidEffectOutcome(
                "retirement runtime is no longer registered",
            ));
        }
        let Some(journal) = &mut self.journal else {
            return Err(RuntimeError::Storage(
                "runtime retirement requires persistent storage".into(),
            ));
        };
        journal
            .commit_retirement_unregistration(
                lease,
                outcome,
                &unregistration_payload,
                state.retirement_id.as_str(),
                current.revision,
                unregistered.revision,
                &checkpoint_payload,
            )
            .map_err(RuntimeError::Storage)?;
        self.control = staged;
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
        let projection = serde_json::to_vec(&unstamped_runtime_projection(&projection))
            .map_err(|error| RuntimeError::Storage(error.to_string()))?;
        let Some(journal) = &mut self.journal else {
            return Err(RuntimeError::Storage(
                "status effect completion requires persistent storage".into(),
            ));
        };
        let timestamp = journal
            .complete_effect_with_journal(
                lease,
                JournalEntryKind::RuntimeStatusObservation,
                &payload,
                &projection,
                outcome,
            )
            .map_err(RuntimeError::Storage)?;
        stamp_runtime(&mut staged, &runtime_id, false, timestamp)?;
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
        let projection = serde_json::to_vec(&unstamped_runtime_projection(&projection))
            .map_err(|error| RuntimeError::Storage(error.to_string()))?;
        let Some(journal) = &mut self.journal else {
            return Err(RuntimeError::Storage(
                "capability effect completion requires persistent storage".into(),
            ));
        };
        let timestamp = journal
            .complete_effect_with_journal(
                lease,
                JournalEntryKind::RuntimeCapabilityObservation,
                &payload,
                &projection,
                outcome,
            )
            .map_err(RuntimeError::Storage)?;
        stamp_runtime(&mut staged, &runtime_id, false, timestamp)?;
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
        let timestamp = if let Some(journal) = &mut self.journal {
            let payload = serde_json::to_vec(&registration)
                .map_err(|error| RuntimeError::Storage(error.to_string()))?;
            journal
                .append_stamped(JournalEntryKind::RuntimeRegistration, &payload)
                .map_err(RuntimeError::Storage)?
                .created_at_unix_ms
        } else {
            persistence::unix_time_ms().map_err(RuntimeError::Storage)?
        };
        self.control
            .register_runtime(id.clone(), registration.name, registration.endpoint);
        stamp_runtime(&mut self.control, &id, true, timestamp)
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

    pub fn runtime_projection(&self, runtime_id: &RuntimeId) -> Option<&RuntimeProjection> {
        self.control.runtime_projection(runtime_id)
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
                let append = match &mut self.journal {
                    Some(journal) => Some(
                        journal
                            .append_stamped(JournalEntryKind::CommandPlan, &payload)
                            .map_err(RuntimeError::Storage)?,
                    ),
                    None => None,
                };
                let prior_revision = self
                    .control
                    .runtime_projection(command_runtime_id(&command.command).ok_or(
                        RuntimeError::Domain(DomainError::InvalidQuery {
                            reason: "debugger commands require the Leselang VM authority",
                        }),
                    )?)
                    .map(|projection| projection.revision);
                let result = match self.control.execute(command) {
                    Ok(result) => result,
                    Err(error) => {
                        if let (Some(journal), Some(append)) = (&mut self.journal, &append) {
                            journal
                                .fail(append.sequence, &error.to_string())
                                .map_err(RuntimeError::Storage)?;
                        }
                        return Err(RuntimeError::Domain(error));
                    }
                };
                if let (Some(journal), Some(append)) = (&mut self.journal, &append) {
                    let outcome = serde_json::to_vec(&result)
                        .map_err(|error| RuntimeError::Storage(error.to_string()))?;
                    journal
                        .complete(append.sequence, &outcome)
                        .map_err(RuntimeError::Storage)?;
                }
                let timestamp = append
                    .map(|entry| entry.created_at_unix_ms)
                    .map(Ok)
                    .unwrap_or_else(persistence::unix_time_ms)
                    .map_err(RuntimeError::Storage)?;
                stamp_result_if_mutated(&mut self.control, &result, prior_revision, timestamp)?;
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

fn registration_proof_from_ready(
    checkpoint: &RuntimeProvisioningCheckpoint,
) -> Result<RuntimeRegistrationProof, RuntimeError> {
    checkpoint.validate().map_err(RuntimeError::Provisioning)?;
    if checkpoint.revision != 2 || checkpoint.state.phase != ProvisioningPhase::ServiceReady {
        return Err(RuntimeError::InvalidEffectOutcome(
            "registration proof requires a revision-2 ready checkpoint",
        ));
    }
    Ok(RuntimeRegistrationProof {
        provisioning_id: checkpoint.state.provisioning_id.clone(),
        runtime_id: checkpoint.state.runtime_id.clone(),
        endpoint: checkpoint
            .state
            .endpoint
            .clone()
            .ok_or(RuntimeError::InvalidEffectOutcome(
                "ready provisioning checkpoint has no endpoint",
            ))?,
        api_credential_handle: checkpoint.state.api_credential_handle.clone().ok_or(
            RuntimeError::InvalidEffectOutcome(
                "ready provisioning checkpoint has no API credential handle",
            ),
        )?,
        trust_credential_handle: checkpoint.state.trust_credential_handle.clone().ok_or(
            RuntimeError::InvalidEffectOutcome(
                "ready provisioning checkpoint has no trust credential handle",
            ),
        )?,
        authority_owned: true,
        protocol_schema_version: PROVISIONING_SERVICE_PROTOCOL_VERSION,
    })
}

fn provisioning_phase_label(phase: ProvisioningPhase) -> &'static str {
    match phase {
        ProvisioningPhase::Planned => "planned",
        ProvisioningPhase::Installing => "installing",
        ProvisioningPhase::ServiceReady => "service_ready",
        ProvisioningPhase::RuntimeRegistered => "runtime_registered",
        ProvisioningPhase::Failed => "failed",
    }
}

fn retirement_phase_label(phase: RetirementPhase) -> &'static str {
    match phase {
        RetirementPhase::Planned => "planned",
        RetirementPhase::RetiringService => "retiring_service",
        RetirementPhase::ServiceRetired => "service_retired",
        RetirementPhase::RuntimeUnregistered => "runtime_unregistered",
        RetirementPhase::Failed => "failed",
    }
}

#[cfg(test)]
mod tests {
    use leselang_command::{LoweringContext, PlannedOperation, lower_effect, plan_runtime_deploy};
    use leselang_hir::lower;
    use leselang_syntax::parse;
    use leserpent_domain::{
        CAPABILITY_RUNTIME_DEPLOY, CAPABILITY_RUNTIME_READ, CAPABILITY_RUNTIME_REFRESH,
        CAPABILITY_RUNTIME_REGISTER, CapabilitySet, Command, CommandEnvelope, CommandId,
        CommandOrigin, Confirmation, IdempotencyKey, Principal, QueryResult, RefreshStatus,
        Revision, RuntimeCapabilityObservation, RuntimeCapabilitySnapshot,
        RuntimeSidecarStatusSnapshot, RuntimeStatusObservation, RuntimeStatusSnapshot,
    };
    use rusqlite::{Connection, params};
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

    fn planned_provisioning() -> RuntimeProvisioning {
        use leserpent_domain::bootstrap::{BootstrapTarget, BootstrapTransport, CredentialHandle};
        use leserpent_domain::provisioning::{
            CAPABILITY_RUNTIME_PROVISION, PROVISIONING_DOMAIN_SCHEMA_VERSION,
            RuntimeProvisioningIntent,
        };

        RuntimeProvisioning::plan(
            &Principal {
                id: "operator-a".into(),
            },
            &CapabilitySet::new([CAPABILITY_RUNTIME_PROVISION]),
            RuntimeProvisioningIntent {
                schema_version: PROVISIONING_DOMAIN_SCHEMA_VERSION,
                provisioning_id: ProvisioningId::new("provision-runtime-a").unwrap(),
                runtime_id: RuntimeId::new("runtime-a").unwrap(),
                target: BootstrapTarget {
                    transport: BootstrapTransport::Ssh,
                    host: "runtime-a.example".into(),
                    port: 22,
                },
                install_credential_handle: CredentialHandle::new("vault:ssh:runtime-a-example")
                    .unwrap(),
                requested_by: "operator-a".into(),
                confirmed: true,
            },
        )
        .unwrap()
    }

    fn provisioning_registration_proof() -> RuntimeRegistrationProof {
        use leserpent_domain::bootstrap::CredentialHandle;
        use leserpent_domain::provisioning::PROVISIONING_SERVICE_PROTOCOL_VERSION;

        RuntimeRegistrationProof {
            provisioning_id: ProvisioningId::new("provision-runtime-a").unwrap(),
            runtime_id: RuntimeId::new("runtime-a").unwrap(),
            endpoint: "https://runtime-a.example:9411/".into(),
            api_credential_handle: CredentialHandle::new("vault:gewyvern-api:runtime-a").unwrap(),
            trust_credential_handle: CredentialHandle::new("vault:gewyvern-ca:runtime-a").unwrap(),
            authority_owned: true,
            protocol_schema_version: PROVISIONING_SERVICE_PROTOCOL_VERSION,
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

    fn persist_queued_orchestra_run(runtime: &mut ControlRuntime, runtime_id: &str, suffix: &str) {
        let run_id = format!("orun-{suffix}");
        let request_id = format!("request-{suffix}");
        let run = format!(
            "{{\"runId\":\"{run_id}\",\"runtimeId\":\"{runtime_id}\",\"planId\":\"test\",\"outcome\":\"queued\",\"executedAt\":\"2026-01-01T00:00:00Z\",\"completedAt\":null,\"requestId\":\"{request_id}\"}}"
        );
        let event = format!(
            "{{\"eventId\":0,\"runId\":\"{run_id}\",\"runtimeId\":\"{runtime_id}\",\"eventType\":\"run_queued\",\"fromOutcome\":null,\"toOutcome\":\"queued\",\"summary\":\"\",\"recordedAt\":\"2026-01-01T00:00:00Z\"}}"
        );
        runtime
            .persist_orchestra_run_event(
                &run_id,
                runtime_id,
                Some(&request_id),
                "run_queued",
                None,
                "queued",
                "queued",
                "2026-01-01T00:00:00Z",
                run.as_bytes(),
                event.as_bytes(),
            )
            .unwrap();
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
        let registered_at;
        let updated_at;
        {
            let mut runtime = ControlRuntime::open(&path).unwrap();
            let projection = runtime
                .register_runtime(
                    RuntimeId::new("runtime-a").unwrap(),
                    "Runtime A",
                    "https://runtime-a.invalid",
                )
                .unwrap();
            registered_at = projection
                .registered_at_unix_ms
                .expect("persistent registration must receive an authority timestamp");
            assert_eq!(projection.updated_at_unix_ms, Some(registered_at));
            std::thread::sleep(Duration::from_millis(2));
            let refresh = refresh_plan(projection.revision);
            runtime.execute_plan(refresh.clone()).unwrap();
            let refreshed = runtime
                .runtime_projection(&RuntimeId::new("runtime-a").unwrap())
                .unwrap()
                .clone();
            updated_at = refreshed
                .updated_at_unix_ms
                .expect("persistent mutation must advance the authority timestamp");
            assert!(updated_at >= registered_at);
            std::thread::sleep(Duration::from_millis(2));
            runtime.execute_plan(refresh).unwrap();
            assert_eq!(
                runtime
                    .runtime_projection(&RuntimeId::new("runtime-a").unwrap())
                    .unwrap()
                    .updated_at_unix_ms,
                Some(updated_at),
                "idempotent replay must not refresh the authority timestamp"
            );
        }

        let mut recovered = ControlRuntime::open(&path).unwrap();
        let PlanResult::Query(QueryResult::RuntimeList { runtimes, .. }) =
            recovered.execute_plan(list_plan()).unwrap()
        else {
            panic!("runtime.list must return a query result");
        };
        assert_eq!(runtimes.len(), 1);
        assert_eq!(runtimes[0].refresh_count, 1);
        assert_eq!(runtimes[0].registered_at_unix_ms, Some(registered_at));
        assert_eq!(runtimes[0].updated_at_unix_ms, Some(updated_at));
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
        assert_eq!(schema, 18);
        assert_eq!(migration_count, 18);
        assert_eq!(legacy_timestamp, 0);
        drop(connection);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn schema_11_bootstrap_checkpoint_migrates_to_shared_authority_storage() {
        use leserpent_domain::bootstrap::{
            BOOTSTRAP_DOMAIN_SCHEMA_VERSION, BootstrapIntent, BootstrapTarget, BootstrapTransport,
            CAPABILITY_HOST_BOOTSTRAP, CredentialHandle,
        };

        let path = temp_journal("v11-authority-checkpoint-migration");
        let bootstrap_id = BootstrapId::new("bootstrap-migrate-1").unwrap();
        let bootstrap = DeploymentBootstrap::plan(
            &Principal {
                id: "operator-a".into(),
            },
            &CapabilitySet::new([CAPABILITY_HOST_BOOTSTRAP]),
            BootstrapIntent {
                schema_version: BOOTSTRAP_DOMAIN_SCHEMA_VERSION,
                bootstrap_id: bootstrap_id.clone(),
                target: BootstrapTarget {
                    transport: BootstrapTransport::Ssh,
                    host: "host.example".into(),
                    port: 22,
                },
                credential_handle: CredentialHandle::new("vault:ssh:host-example").unwrap(),
                requested_by: "operator-a".into(),
                confirmed: true,
            },
        )
        .unwrap();
        let checkpoint = bootstrap.checkpoint(1).unwrap();
        let mut runtime = ControlRuntime::open(&path).unwrap();
        runtime
            .enqueue_bootstrap_effect(
                "bootstrap-migrate-effect",
                "leserpent.host.bootstrap",
                b"request",
                3,
                &checkpoint,
            )
            .unwrap();
        drop(runtime);

        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE bootstrap_handoffs (
                     bootstrap_id TEXT PRIMARY KEY,
                     revision INTEGER NOT NULL CHECK (revision >= 1),
                     phase TEXT NOT NULL CHECK (
                         phase IN ('planned', 'deploying', 'bootstrapped', 'session_bound', 'failed')
                     ),
                     checkpoint BLOB NOT NULL CHECK (length(checkpoint) <= 65536),
                     updated_at_unix_ms INTEGER NOT NULL CHECK (updated_at_unix_ms >= 0)
                 ) STRICT;
                 CREATE INDEX bootstrap_handoffs_by_phase
                     ON bootstrap_handoffs (phase, updated_at_unix_ms DESC);
                 INSERT INTO bootstrap_handoffs
                     (bootstrap_id, revision, phase, checkpoint, updated_at_unix_ms)
                 SELECT operation_id, revision, phase, checkpoint, updated_at_unix_ms
                 FROM authority_checkpoints WHERE operation_kind = 'daemon_bootstrap';
                 DROP INDEX authority_checkpoints_by_kind_phase;
                 DROP TABLE authority_checkpoints;
                 DROP TABLE runtime_unregistration_replay_horizon;
                 DROP TABLE runtime_unregistration_operations;
                 DROP TABLE orchestra_delete_replay_horizon;
                 DROP TABLE orchestra_delete_generation;
                 DROP TABLE orchestra_delete_operations;
                 DELETE FROM runtime_schema_migrations
                 WHERE version IN (12, 13, 14, 15, 16, 17, 18);
                 UPDATE runtime_metadata SET value = 11 WHERE key = 'schema_version';",
            )
            .unwrap();
        drop(connection);

        let mut migrated = ControlRuntime::open(&path).unwrap();
        assert_eq!(
            migrated
                .bootstrap_checkpoint(&bootstrap_id)
                .unwrap()
                .unwrap(),
            checkpoint
        );
        drop(migrated);
        let connection = Connection::open(&path).unwrap();
        let (schema, bootstrap_rows, provisioning_rows): (i64, i64, i64) = connection
            .query_row(
                "SELECT
                     (SELECT value FROM runtime_metadata WHERE key = 'schema_version'),
                     (SELECT COUNT(*) FROM authority_checkpoints
                      WHERE operation_kind = 'daemon_bootstrap'),
                     (SELECT COUNT(*) FROM authority_checkpoints
                      WHERE operation_kind = 'gewyvern_provisioning')",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!((schema, bootstrap_rows, provisioning_rows), (18, 1, 0));
        drop(connection);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn schema_14_unregistration_operations_gain_schema_owned_generations() {
        let path = temp_journal("v14-unregistration-generation-migration");
        let mut runtime = ControlRuntime::open(&path).unwrap();
        for suffix in ["a", "b"] {
            let runtime_id = RuntimeId::new(format!("runtime-migrate-{suffix}")).unwrap();
            let projection = runtime
                .register_runtime(
                    runtime_id.clone(),
                    format!("Runtime Migrate {suffix}"),
                    format!("https://runtime-migrate-{suffix}.invalid"),
                )
                .unwrap();
            runtime
                .unregister_runtimes(
                    CommandId::new(format!("unregister-migrate-{suffix}")).unwrap(),
                    vec![RuntimeUnregisterTarget {
                        runtime_id,
                        expected_revision: projection.revision,
                    }],
                )
                .unwrap();
        }
        drop(runtime);

        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "ALTER TABLE runtime_unregistration_operations
                     RENAME TO runtime_unregistration_operations_v15;
                 CREATE TABLE runtime_unregistration_operations (
                     operation_id TEXT PRIMARY KEY,
                     request BLOB NOT NULL CHECK (length(request) <= 65536),
                     deleted_runtime_count INTEGER NOT NULL CHECK (deleted_runtime_count >= 0),
                     deleted_run_count INTEGER NOT NULL CHECK (deleted_run_count >= 0),
                     deleted_event_count INTEGER NOT NULL CHECK (deleted_event_count >= 0),
                     removed_at_unix_ms INTEGER NOT NULL CHECK (removed_at_unix_ms >= 0)
                 ) STRICT;
                 INSERT INTO runtime_unregistration_operations
                     (operation_id, request, deleted_runtime_count, deleted_run_count,
                      deleted_event_count, removed_at_unix_ms)
                 SELECT operation_id, request, deleted_runtime_count, deleted_run_count,
                        deleted_event_count, removed_at_unix_ms
                 FROM runtime_unregistration_operations_v15 ORDER BY generation ASC;
                 DROP TABLE runtime_unregistration_operations_v15;
                 DROP TABLE runtime_unregistration_replay_horizon;
                 DROP TABLE orchestra_delete_replay_horizon;
                 DROP TABLE orchestra_delete_generation;
                 DROP TABLE orchestra_delete_operations;
                 DELETE FROM runtime_schema_migrations WHERE version IN (17, 18);
                 DELETE FROM runtime_schema_migrations WHERE version = 16;
                 DELETE FROM runtime_schema_migrations WHERE version = 15;
                 UPDATE runtime_metadata SET value = 14 WHERE key = 'schema_version';",
            )
            .unwrap();
        drop(connection);

        drop(ControlRuntime::open(&path).unwrap());
        let connection = Connection::open(&path).unwrap();
        let operations = connection
            .prepare(
                "SELECT operation_id, generation
                 FROM runtime_unregistration_operations ORDER BY generation ASC",
            )
            .unwrap()
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let horizon: (i64, i64) = connection
            .query_row(
                "SELECT next_generation, evicted_through_generation
                 FROM runtime_unregistration_replay_horizon WHERE id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(
            operations,
            vec![
                ("unregister-migrate-a".into(), 1),
                ("unregister-migrate-b".into(), 2),
            ]
        );
        assert_eq!(horizon, (3, 0));
        drop(connection);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn schema_15_gains_durable_orchestra_delete_receipts() {
        let path = temp_journal("v15-orchestra-delete-receipt-migration");
        drop(ControlRuntime::open(&path).unwrap());
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "DROP TABLE orchestra_delete_replay_horizon;
                 DROP TABLE orchestra_delete_generation;
                 DROP TABLE orchestra_delete_operations;
                 DELETE FROM runtime_schema_migrations WHERE version = 18;
                 DELETE FROM runtime_schema_migrations WHERE version = 17;
                 DELETE FROM runtime_schema_migrations WHERE version = 16;
                 UPDATE runtime_metadata SET value = 15
                 WHERE key = 'schema_version';",
            )
            .unwrap();
        drop(connection);

        drop(ControlRuntime::open(&path).unwrap());
        let connection = Connection::open(&path).unwrap();
        let state: (i64, i64, i64, i64) = connection
            .query_row(
                "SELECT
                     (SELECT value FROM runtime_metadata
                      WHERE key = 'schema_version'),
                     (SELECT COUNT(*) FROM runtime_schema_migrations
                      WHERE version = 16),
                     (SELECT COUNT(*) FROM orchestra_delete_operations),
                     (SELECT next_generation FROM orchestra_delete_generation
                      WHERE id = 1)",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(state, (18, 1, 0, 1));
        drop(connection);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn schema_16_receipts_gain_a_lossless_replay_horizon() {
        let path = temp_journal("v16-orchestra-delete-horizon-migration");
        let command_id = CommandId::new("orchestra-delete-v16").unwrap();
        let targets = vec!["runtime-delete-v16".to_string()];
        let first = {
            let mut runtime = ControlRuntime::open(&path).unwrap();
            persist_queued_orchestra_run(&mut runtime, &targets[0], "v16");
            runtime
                .delete_orchestra_runtimes_idempotent(command_id.clone(), &targets)
                .unwrap()
        };
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "DROP TABLE orchestra_delete_replay_horizon;
                 DELETE FROM runtime_schema_migrations WHERE version IN (17, 18);
                 UPDATE runtime_metadata SET value = 16
                 WHERE key = 'schema_version';",
            )
            .unwrap();
        drop(connection);

        let mut migrated = ControlRuntime::open(&path).unwrap();
        assert_eq!(
            migrated.orchestra_delete_replay_horizon().unwrap(),
            OrchestraDeleteReplayHorizon {
                capacity: 4_096,
                retained: 1,
                oldest_generation: Some(1),
                newest_generation: Some(1),
                next_generation: 2,
                evicted_through_generation: 0,
                protected_from_generation: Some(1),
                checkpointed_through_generation: Some(1),
            }
        );
        let replay = migrated
            .delete_orchestra_runtimes_idempotent(command_id, &targets)
            .unwrap();
        assert!(replay.replayed);
        assert_eq!(replay.operation_generation, first.operation_generation);
        assert_eq!(replay.committed_at_unix_ms, first.committed_at_unix_ms);
        assert_eq!(replay.deleted_run_count, first.deleted_run_count);
        assert_eq!(replay.deleted_event_count, first.deleted_event_count);
        drop(migrated);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn sqlite_journal_rejects_incomplete_current_schema() {
        let path = temp_journal("incomplete-v18");
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
                 INSERT INTO runtime_metadata (key, value) VALUES ('schema_version', 18);",
            )
            .unwrap();
        drop(connection);

        assert!(matches!(
            ControlRuntime::open(&path),
            Err(RuntimeError::Storage(ref error)) if error.contains("invalid runtime journal schema 18")
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
            .execute(
                "DELETE FROM runtime_schema_migrations WHERE version = 12",
                [],
            )
            .unwrap();
        connection
            .execute(
                "DELETE FROM runtime_schema_migrations WHERE version = 13",
                [],
            )
            .unwrap();
        connection
            .execute(
                "DELETE FROM runtime_schema_migrations WHERE version = 14",
                [],
            )
            .unwrap();
        connection
            .execute(
                "DELETE FROM runtime_schema_migrations WHERE version = 15",
                [],
            )
            .unwrap();
        connection
            .execute(
                "DELETE FROM runtime_schema_migrations WHERE version = 16",
                [],
            )
            .unwrap();
        connection
            .execute(
                "DELETE FROM runtime_schema_migrations WHERE version = 17",
                [],
            )
            .unwrap();
        connection
            .execute(
                "DELETE FROM runtime_schema_migrations WHERE version = 18",
                [],
            )
            .unwrap();
        connection
            .execute("DROP TABLE orchestra_delete_replay_horizon", [])
            .unwrap();
        connection
            .execute("DROP TABLE orchestra_delete_generation", [])
            .unwrap();
        connection
            .execute("DROP TABLE orchestra_delete_operations", [])
            .unwrap();
        connection
            .execute("DROP TABLE runtime_unregistration_replay_horizon", [])
            .unwrap();
        connection
            .execute("DROP TABLE runtime_unregistration_operations", [])
            .unwrap();
        connection
            .execute("DROP TABLE authority_checkpoints", [])
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
        assert_eq!(schema, 18);
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
                if error.contains("invalid runtime journal schema 18 journal kind")
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
                 VALUES (19, 0)",
                [],
            )
            .unwrap();
        assert!(matches!(
            ControlRuntime::open(&path),
            Err(RuntimeError::Storage(ref error))
                if error.contains("invalid runtime journal schema 18 migration history")
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
                 DROP TABLE authority_checkpoints;
                 DROP TABLE runtime_unregistration_replay_horizon;
                 DROP TABLE runtime_unregistration_operations;
                 DROP TABLE orchestra_delete_replay_horizon;
                 DROP TABLE orchestra_delete_generation;
                 DROP TABLE orchestra_delete_operations;
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
        assert_eq!(schema, 18);
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
    fn provisioning_checkpoint_is_atomic_restart_safe_and_registration_fenced() {
        use leserpent_domain::bootstrap::CredentialHandle;
        use leserpent_domain::provisioning::GewyvernServiceReceipt;

        let path = temp_journal("provisioning-authority-checkpoint");
        let provisioning_id = ProvisioningId::new("provision-runtime-a").unwrap();
        let mut provisioning = planned_provisioning();
        let planned = provisioning.checkpoint(1).unwrap();
        let mut runtime = ControlRuntime::open(&path).unwrap();
        runtime
            .enqueue_provisioning_effect(
                "provision-runtime-a-effect",
                "gewyvern.host.provision",
                b"bounded-request",
                3,
                &planned,
            )
            .unwrap();
        runtime
            .enqueue_provisioning_effect(
                "provision-runtime-a-effect",
                "gewyvern.host.provision",
                b"bounded-request",
                3,
                &planned,
            )
            .unwrap();
        assert!(
            runtime
                .enqueue_provisioning_effect(
                    "provision-runtime-a-effect",
                    "gewyvern.host.provision",
                    b"different-request",
                    3,
                    &planned,
                )
                .is_err()
        );

        let lease = runtime
            .claim_effect("worker-a", Duration::from_secs(30))
            .unwrap()
            .unwrap();
        provisioning.begin().unwrap();
        provisioning
            .accept_service(GewyvernServiceReceipt {
                provisioning_id: provisioning_id.clone(),
                runtime_id: RuntimeId::new("runtime-a").unwrap(),
                endpoint: "https://runtime-a.example:9411/".into(),
                api_credential_handle: CredentialHandle::new("vault:gewyvern-api:runtime-a")
                    .unwrap(),
                trust_credential_handle: CredentialHandle::new("vault:gewyvern-ca:runtime-a")
                    .unwrap(),
            })
            .unwrap();
        let ready = provisioning.checkpoint(2).unwrap();
        runtime
            .complete_provisioning_effect(&lease, b"service-ready", &ready)
            .unwrap();
        drop(runtime);

        let mut restarted = ControlRuntime::open(&path).unwrap();
        let restored = restarted
            .provisioning_checkpoint(&provisioning_id)
            .unwrap()
            .unwrap();
        assert_eq!(restored.state.phase, ProvisioningPhase::ServiceReady);
        assert!(!restored.state.install_credential_present);
        let registered = restarted
            .accept_provisioning_registration(&provisioning_id, provisioning_registration_proof())
            .unwrap();
        assert_eq!(registered.phase, ProvisioningPhase::RuntimeRegistered);
        assert_eq!(
            restarted
                .accept_provisioning_registration(
                    &provisioning_id,
                    provisioning_registration_proof(),
                )
                .unwrap(),
            registered
        );
        drop(restarted);

        let mut final_runtime = ControlRuntime::open(&path).unwrap();
        let final_checkpoint = final_runtime
            .provisioning_checkpoint(&provisioning_id)
            .unwrap()
            .unwrap();
        assert_eq!(final_checkpoint.revision, 3);
        assert_eq!(
            final_checkpoint.state.phase,
            ProvisioningPhase::RuntimeRegistered
        );
        assert_eq!(
            final_runtime
                .runtime_projection(&RuntimeId::new("runtime-a").unwrap())
                .unwrap()
                .endpoint,
            "https://runtime-a.example:9411/"
        );
        drop(final_runtime);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn lost_lease_rolls_back_registration_and_registered_checkpoint_together() {
        use leserpent_domain::bootstrap::CredentialHandle;
        use leserpent_domain::provisioning::GewyvernServiceReceipt;

        let path = temp_journal("provisioning-registration-atomic-rollback");
        let provisioning_id = ProvisioningId::new("provision-runtime-a").unwrap();
        let mut provisioning = planned_provisioning();
        let planned = provisioning.checkpoint(1).unwrap();
        let mut runtime = ControlRuntime::open(&path).unwrap();
        runtime
            .enqueue_provisioning_effect(
                "provision-runtime-a-effect",
                "gewyvern.host.provision",
                b"bounded-request",
                3,
                &planned,
            )
            .unwrap();
        let lease = runtime
            .claim_effect("worker-a", Duration::from_millis(1))
            .unwrap()
            .unwrap();
        provisioning.begin().unwrap();
        provisioning
            .accept_service(GewyvernServiceReceipt {
                provisioning_id: provisioning_id.clone(),
                runtime_id: RuntimeId::new("runtime-a").unwrap(),
                endpoint: "https://runtime-a.example:9411/".into(),
                api_credential_handle: CredentialHandle::new("vault:gewyvern-api:runtime-a")
                    .unwrap(),
                trust_credential_handle: CredentialHandle::new("vault:gewyvern-ca:runtime-a")
                    .unwrap(),
            })
            .unwrap();
        let ready = provisioning.checkpoint(2).unwrap();
        std::thread::sleep(Duration::from_millis(5));

        assert!(matches!(
            runtime.complete_provisioning_effect_and_register(&lease, b"service-ready", &ready),
            Err(RuntimeError::Storage(ref error)) if error.contains("lease was lost or expired")
        ));
        assert_eq!(
            runtime
                .provisioning_checkpoint(&provisioning_id)
                .unwrap()
                .unwrap(),
            planned
        );
        assert!(
            runtime
                .runtime_projection(&RuntimeId::new("runtime-a").unwrap())
                .is_none()
        );
        drop(runtime);

        let connection = Connection::open(&path).unwrap();
        let registrations: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM runtime_journal WHERE kind = 'runtime_registration'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(registrations, 0);
        drop(connection);
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
    fn sidecar_discovery_intake_is_durable_and_restart_safe() {
        let path = temp_journal("sidecar-discovery-replay");
        let runtime_id = RuntimeId::new("runtime-sidecar").unwrap();
        {
            let mut runtime = ControlRuntime::open(&path).unwrap();
            let registered = runtime
                .register_runtime(
                    runtime_id.clone(),
                    "Runtime Sidecar",
                    "https://runtime.invalid",
                )
                .unwrap();
            let result = runtime
                .execute_plan(CommandPlan {
                    schema_version: leserpent_domain::COMMAND_PLAN_SCHEMA_VERSION,
                    required_capability: CAPABILITY_RUNTIME_REGISTER.into(),
                    operation: PlannedOperation::Command(CommandEnvelope {
                        schema_version: leserpent_domain::DOMAIN_SCHEMA_VERSION,
                        command_id: CommandId::new("sidecar-discovery-command").unwrap(),
                        idempotency_key: IdempotencyKey::new("sidecar-discovery-key").unwrap(),
                        expected_revision: Some(registered.revision),
                        principal: Principal {
                            id: "web-bridge".into(),
                        },
                        capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_REGISTER]),
                        origin: CommandOrigin::CompatibilityAdapter,
                        confirmation: Confirmation::Confirmed,
                        dry_run: false,
                        command: Command::RuntimeDiscoveryIntake {
                            runtime_id: runtime_id.clone(),
                            capabilities: None,
                            status: None,
                            sidecar_status: Some(Box::new(RuntimeSidecarStatusSnapshot {
                                status_source: "fetch_failed".into(),
                                status_fetched_at: None,
                                status_fetch_error: Some("sidecar_fetch_failed".into()),
                                healthy: false,
                                daemon_status: "fetch_failed".into(),
                                target_count: None,
                                learning_active: false,
                                learned_routes: 0,
                                has_evidence_chain_enrichment: false,
                                has_diagnostic_opinion: false,
                                last_error: Some("sidecar_fetch_failed".into()),
                                memory: None,
                            })),
                        },
                    }),
                })
                .unwrap();
            let PlanResult::Command(result) = result else {
                panic!("sidecar discovery must return a command result");
            };
            assert_eq!(
                result
                    .runtime
                    .sidecar_status
                    .as_ref()
                    .unwrap()
                    .status_source,
                "fetch_failed"
            );
        }

        let recovered = ControlRuntime::open(&path).unwrap();
        let projection = recovered.runtime_projection(&runtime_id).unwrap();
        assert_eq!(
            projection
                .sidecar_status
                .as_ref()
                .unwrap()
                .status_fetch_error
                .as_deref(),
            Some("sidecar_fetch_failed")
        );
        assert!(projection.updated_at_unix_ms.is_some());
        drop(recovered);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn orchestra_run_and_event_persist_atomically_with_idempotent_read_back() {
        let path = temp_journal("orchestra-atomic-persistence");
        let mut runtime = ControlRuntime::open(&path).unwrap();
        let run = br#"{"runId":"orun-1","runtimeId":"runtime-a","planId":"test","outcome":"queued","executedAt":"2026-01-01T00:00:00Z","completedAt":null,"requestId":"request-1"}"#;
        let event = br#"{"eventId":0,"runId":"orun-1","runtimeId":"runtime-a","eventType":"run_queued","fromOutcome":null,"toOutcome":"queued","summary":"","recordedAt":"2026-01-01T00:00:00Z"}"#;
        let first = runtime
            .persist_orchestra_run_event(
                "orun-1",
                "runtime-a",
                Some("request-1"),
                "run_queued",
                None,
                "queued",
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
                None,
                "queued",
                "queued",
                "2026-01-01T00:00:00Z",
                run,
                event,
            )
            .unwrap();
        assert_eq!(replay.run, run);
        assert_eq!(replay.event, event);
        assert_eq!(replay.event_count, 1);

        let changed_run = br#"{"runId":"orun-1","runtimeId":"runtime-a","planId":"test","outcome":"running","executedAt":"2026-01-01T00:00:00Z","completedAt":null,"requestId":"request-1"}"#;
        let changed_event = br#"{"eventId":0,"runId":"orun-1","runtimeId":"runtime-a","eventType":"run_queued","fromOutcome":null,"toOutcome":"queued","summary":"drift","recordedAt":"2026-01-01T00:00:00Z"}"#;
        assert!(
            runtime
                .persist_orchestra_run_event(
                    "orun-1",
                    "runtime-a",
                    Some("request-1"),
                    "run_queued",
                    None,
                    "queued",
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
                    None,
                    "queued",
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
                None,
                "queued",
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
        let duplicate_request_run = br#"{"runId":"orun-2","runtimeId":"runtime-a","planId":"test","outcome":"queued","executedAt":"2026-01-01T00:00:01Z","completedAt":null,"requestId":"request-1"}"#;
        let duplicate_request_event = br#"{"eventId":0,"runId":"orun-2","runtimeId":"runtime-a","eventType":"run_queued","fromOutcome":null,"toOutcome":"queued","summary":"","recordedAt":"2026-01-01T00:00:01Z"}"#;
        assert!(
            runtime
                .persist_orchestra_run_event(
                    "orun-2",
                    "runtime-a",
                    Some("request-1"),
                    "run_queued",
                    None,
                    "queued",
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
                    None,
                    "queued",
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
        for index in 0..32 {
            let run_id = format!("bounded-{index:02}");
            let request_id = format!("bounded-request-{index:02}");
            let recorded_at = format!("2026-01-01T00:01:{index:02}Z");
            let bounded_run = format!(
                "{{\"runId\":\"{run_id}\",\"runtimeId\":\"runtime-a\",\"planId\":\"test\",\"outcome\":\"queued\",\"executedAt\":\"{recorded_at}\",\"completedAt\":null,\"requestId\":\"{request_id}\"}}"
            );
            let bounded_event = format!(
                "{{\"eventId\":0,\"runId\":\"{run_id}\",\"runtimeId\":\"runtime-a\",\"eventType\":\"run_queued\",\"fromOutcome\":null,\"toOutcome\":\"queued\",\"summary\":\"\",\"recordedAt\":\"{recorded_at}\"}}"
            );
            runtime
                .persist_orchestra_run_event(
                    &run_id,
                    "runtime-a",
                    Some(&request_id),
                    "run_queued",
                    None,
                    "queued",
                    "queued",
                    &recorded_at,
                    bounded_run.as_bytes(),
                    bounded_event.as_bytes(),
                )
                .unwrap();
        }
        let run_id = "bounded-32";
        let request_id = "bounded-request-32";
        let recorded_at = "2026-01-01T00:01:32Z";
        let bounded_run = format!(
            "{{\"runId\":\"{run_id}\",\"runtimeId\":\"runtime-a\",\"planId\":\"test\",\"outcome\":\"queued\",\"executedAt\":\"{recorded_at}\",\"completedAt\":null,\"requestId\":\"{request_id}\"}}"
        );
        let bounded_event = format!(
            "{{\"eventId\":0,\"runId\":\"{run_id}\",\"runtimeId\":\"runtime-a\",\"eventType\":\"run_queued\",\"fromOutcome\":null,\"toOutcome\":\"queued\",\"summary\":\"\",\"recordedAt\":\"{recorded_at}\"}}"
        );
        let connection = Connection::open(&path).unwrap();
        connection
            .execute(
                "UPDATE orchestra_runs SET updated_at_unix_ms = 4000000000000
                 WHERE runtime_id = 'runtime-a'",
                [],
            )
            .unwrap();
        connection
            .execute_batch(
                "CREATE TRIGGER ignore_orchestra_retention_delete
                 BEFORE DELETE ON orchestra_runs
                 WHEN OLD.runtime_id = 'runtime-a'
                 BEGIN
                     SELECT RAISE(IGNORE);
                 END;",
            )
            .unwrap();
        assert!(
            runtime
                .persist_orchestra_run_event(
                    run_id,
                    "runtime-a",
                    Some(request_id),
                    "run_queued",
                    None,
                    "queued",
                    "queued",
                    recorded_at,
                    bounded_run.as_bytes(),
                    bounded_event.as_bytes(),
                )
                .is_err()
        );
        let retained_after_rollback: (i64, i64, i64) = connection
            .query_row(
                "SELECT
                     (SELECT COUNT(*) FROM orchestra_runs WHERE runtime_id = 'runtime-a'),
                     (SELECT COUNT(*) FROM orchestra_events WHERE runtime_id = 'runtime-a'),
                     (SELECT COUNT(*) FROM orchestra_runs WHERE run_id = 'bounded-31')",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(retained_after_rollback, (32, 32, 1));
        connection
            .execute_batch("DROP TRIGGER ignore_orchestra_retention_delete;")
            .unwrap();
        drop(connection);
        runtime
            .persist_orchestra_run_event(
                run_id,
                "runtime-a",
                Some(request_id),
                "run_queued",
                None,
                "queued",
                "queued",
                recorded_at,
                bounded_run.as_bytes(),
                bounded_event.as_bytes(),
            )
            .unwrap();
        let bounded = runtime
            .load_orchestra_history(Some("runtime-a"), None, 0, 64)
            .unwrap();
        assert_eq!(bounded.runs.len(), 32);
        assert!(bounded.runs.iter().all(|run| {
            !run.windows(b"bounded-31".len())
                .any(|value| value == b"bounded-31")
        }));
        let connection = Connection::open(&path).unwrap();
        let evicted_counts: (i64, i64, i64, i64, i64) = connection
            .query_row(
                "SELECT
                     (SELECT COUNT(*) FROM orchestra_runs WHERE runtime_id = 'runtime-a'),
                     (SELECT COUNT(*) FROM orchestra_events WHERE runtime_id = 'runtime-a'),
                     (SELECT COUNT(*) FROM orchestra_runs WHERE run_id = 'bounded-31'),
                     (SELECT COUNT(*) FROM orchestra_events WHERE run_id = 'bounded-31'),
                     (SELECT updated_at_unix_ms FROM orchestra_runs
                      WHERE run_id = 'bounded-32')",
                [],
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
            .unwrap();
        assert_eq!(evicted_counts, (32, 32, 0, 0, 4000000000001));
        let (lookahead_run_id, lookahead_envelope): (String, Vec<u8>) = connection
            .query_row(
                "SELECT run_id, envelope FROM orchestra_runs
                 WHERE runtime_id = 'runtime-a'
                 ORDER BY updated_at_unix_ms DESC, run_id ASC
                 LIMIT 1 OFFSET 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        let mut mismatched_run: serde_json::Value =
            serde_json::from_slice(&lookahead_envelope).unwrap();
        mismatched_run["outcome"] = serde_json::json!("succeeded");
        connection
            .execute(
                "UPDATE orchestra_runs SET envelope = ?1 WHERE run_id = ?2",
                rusqlite::params![
                    serde_json::to_vec(&mismatched_run).unwrap(),
                    lookahead_run_id
                ],
            )
            .unwrap();
        assert!(
            runtime
                .load_orchestra_history(Some("runtime-a"), None, 0, 1)
                .is_err()
        );
        connection
            .execute(
                "UPDATE orchestra_runs SET envelope = ?1 WHERE run_id = ?2",
                rusqlite::params![lookahead_envelope, lookahead_run_id],
            )
            .unwrap();
        connection
            .execute(
                "UPDATE orchestra_runs SET request_id = 'lookahead-request-drift'
                 WHERE run_id = ?1",
                [&lookahead_run_id],
            )
            .unwrap();
        assert!(
            runtime
                .load_orchestra_history(Some("runtime-a"), None, 0, 1)
                .is_err()
        );
        assert!(runtime.load_orchestra_history(None, None, 0, 1).is_err());
        assert!(
            runtime
                .load_orchestra_history(Some("runtime-a"), Some(&lookahead_run_id), 0, 1)
                .is_err()
        );
        drop(connection);
        drop(runtime);
        let connection = Connection::open(&path).unwrap();
        let schema: i64 = connection
            .query_row(
                "SELECT value FROM runtime_metadata WHERE key = 'schema_version'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(schema, 18);
        drop(connection);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn orchestra_append_rejects_mismatched_native_envelopes_before_commit() {
        let path = temp_journal("orchestra-native-envelope-fence");
        let mut runtime = ControlRuntime::open(&path).unwrap();
        let canonical_run = serde_json::json!({
            "runId": "orun-native",
            "runtimeId": "runtime-a",
            "planId": "test",
            "outcome": "queued",
            "executedAt": "2026-01-01T00:00:00Z",
            "completedAt": null,
            "requestId": "request-native"
        });
        let canonical_event = serde_json::json!({
            "eventId": 0,
            "runId": "orun-native",
            "runtimeId": "runtime-a",
            "eventType": "run_queued",
            "fromOutcome": null,
            "toOutcome": "queued",
            "summary": "",
            "recordedAt": "2026-01-01T00:00:00Z"
        });
        let mut malformed = Vec::new();
        for (label, field, value) in [
            ("run identity", "runId", serde_json::json!("orun-other")),
            (
                "run runtime identity",
                "runtimeId",
                serde_json::json!("runtime-b"),
            ),
            ("run request identity", "requestId", serde_json::Value::Null),
            ("run outcome", "outcome", serde_json::json!("running")),
            (
                "run execution time",
                "executedAt",
                serde_json::json!("2026-01-01T00:00:01Z"),
            ),
        ] {
            let mut run = canonical_run.clone();
            run[field] = value;
            malformed.push((label, run, canonical_event.clone()));
        }
        for (label, field, value) in [
            ("event identity", "eventId", serde_json::json!(99)),
            (
                "event run identity",
                "runId",
                serde_json::json!("orun-other"),
            ),
            (
                "event runtime identity",
                "runtimeId",
                serde_json::json!("runtime-b"),
            ),
            ("event type", "eventType", serde_json::json!("run_started")),
            (
                "event source outcome",
                "fromOutcome",
                serde_json::json!("queued"),
            ),
            (
                "event target outcome",
                "toOutcome",
                serde_json::json!("running"),
            ),
            (
                "event recording time",
                "recordedAt",
                serde_json::json!("2026-01-01T00:00:01Z"),
            ),
        ] {
            let mut event = canonical_event.clone();
            event[field] = value;
            malformed.push((label, canonical_run.clone(), event));
        }
        for (label, run, event) in malformed {
            let run = serde_json::to_vec(&run).unwrap();
            let event = serde_json::to_vec(&event).unwrap();
            assert!(
                runtime
                    .persist_orchestra_run_event(
                        "orun-native",
                        "runtime-a",
                        Some("request-native"),
                        "run_queued",
                        None,
                        "queued",
                        "queued",
                        "2026-01-01T00:00:00Z",
                        &run,
                        &event,
                    )
                    .is_err(),
                "{label} drift must be rejected"
            );
        }

        let connection = Connection::open(&path).unwrap();
        let counts: (i64, i64) = connection
            .query_row(
                "SELECT
                     (SELECT COUNT(*) FROM orchestra_runs),
                     (SELECT COUNT(*) FROM orchestra_events)",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(counts, (0, 0));
        drop(connection);

        let run = serde_json::to_vec(&canonical_run).unwrap();
        let event = serde_json::to_vec(&canonical_event).unwrap();
        let stored = runtime
            .persist_orchestra_run_event(
                "orun-native",
                "runtime-a",
                Some("request-native"),
                "run_queued",
                None,
                "queued",
                "queued",
                "2026-01-01T00:00:00Z",
                &run,
                &event,
            )
            .unwrap();
        assert_eq!(stored.event_count, 1);
        assert_eq!(stored.run, run);
        assert_eq!(stored.event, event);

        drop(runtime);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn orchestra_multi_runtime_delete_validates_receipt_cascade_and_preservation() {
        let path = temp_journal("orchestra-delete-snapshot");
        let mut runtime = ControlRuntime::open(&path).unwrap();
        persist_queued_orchestra_run(&mut runtime, "runtime-delete-a", "delete-a");
        persist_queued_orchestra_run(&mut runtime, "runtime-delete-b", "delete-b");
        persist_queued_orchestra_run(&mut runtime, "runtime-keep", "keep");
        let targets = vec!["runtime-delete-a".into(), "runtime-delete-b".into()];

        let connection = Connection::open(&path).unwrap();
        let unrelated_before: (Vec<u8>, i64, Vec<u8>, i64) = connection
            .query_row(
                "SELECT run.envelope, run.updated_at_unix_ms,
                        event.envelope, event.created_at_unix_ms
                 FROM orchestra_runs AS run
                 JOIN orchestra_events AS event ON event.run_id = run.run_id
                 WHERE run.runtime_id = 'runtime-keep'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        connection
            .execute(
                "UPDATE orchestra_events SET runtime_id = 'runtime-keep'
                 WHERE run_id = 'orun-delete-a'",
                [],
            )
            .unwrap();
        assert!(runtime.delete_orchestra_runtimes(&targets).is_err());
        let counts_after_ownership_rejection: (i64, i64) = connection
            .query_row(
                "SELECT
                     (SELECT COUNT(*) FROM orchestra_runs),
                     (SELECT COUNT(*) FROM orchestra_events)",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(counts_after_ownership_rejection, (3, 3));
        connection
            .execute(
                "UPDATE orchestra_events SET runtime_id = 'runtime-delete-a'
                 WHERE run_id = 'orun-delete-a'",
                [],
            )
            .unwrap();
        connection
            .execute(
                "UPDATE orchestra_events SET envelope = x'7b7d'
                 WHERE run_id = 'orun-delete-a'",
                [],
            )
            .unwrap();
        connection
            .execute_batch(
                "CREATE TRIGGER ignore_orchestra_multi_delete
                 BEFORE DELETE ON orchestra_runs
                 WHEN OLD.runtime_id = 'runtime-delete-b'
                 BEGIN
                     SELECT RAISE(IGNORE);
                 END;",
            )
            .unwrap();
        assert!(runtime.delete_orchestra_runtimes(&targets).is_err());
        let counts_after_ignored_delete: (i64, i64) = connection
            .query_row(
                "SELECT
                     (SELECT COUNT(*) FROM orchestra_runs),
                     (SELECT COUNT(*) FROM orchestra_events)",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(counts_after_ignored_delete, (3, 3));

        connection
            .execute_batch(
                "DROP TRIGGER ignore_orchestra_multi_delete;
                 CREATE TRIGGER mutate_unrelated_orchestra_run
                 AFTER DELETE ON orchestra_runs
                 WHEN OLD.runtime_id = 'runtime-delete-a'
                 BEGIN
                     UPDATE orchestra_runs SET envelope = envelope
                     WHERE runtime_id = 'runtime-keep';
                 END;",
            )
            .unwrap();
        assert!(runtime.delete_orchestra_runtimes(&targets).is_err());
        let unrelated_after_rollback: (Vec<u8>, i64, Vec<u8>, i64) = connection
            .query_row(
                "SELECT run.envelope, run.updated_at_unix_ms,
                        event.envelope, event.created_at_unix_ms
                 FROM orchestra_runs AS run
                 JOIN orchestra_events AS event ON event.run_id = run.run_id
                 WHERE run.runtime_id = 'runtime-keep'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(unrelated_after_rollback, unrelated_before);
        connection
            .execute_batch("DROP TRIGGER mutate_unrelated_orchestra_run;")
            .unwrap();
        drop(connection);

        let deleted = runtime.delete_orchestra_runtimes(&targets).unwrap();
        assert_eq!(deleted.deleted_runtime_count, 2);
        assert_eq!(deleted.deleted_run_count, 2);
        assert_eq!(deleted.deleted_event_count, 2);
        let connection = Connection::open(&path).unwrap();
        let final_counts: (i64, i64, i64, i64) = connection
            .query_row(
                "SELECT
                     (SELECT COUNT(*) FROM orchestra_runs
                      WHERE runtime_id IN ('runtime-delete-a', 'runtime-delete-b')),
                     (SELECT COUNT(*) FROM orchestra_events
                      WHERE runtime_id IN ('runtime-delete-a', 'runtime-delete-b')),
                     (SELECT COUNT(*) FROM orchestra_runs WHERE runtime_id = 'runtime-keep'),
                     (SELECT COUNT(*) FROM orchestra_events WHERE runtime_id = 'runtime-keep')",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(final_counts, (0, 0, 1, 1));
        let unrelated_after_commit: (Vec<u8>, i64, Vec<u8>, i64) = connection
            .query_row(
                "SELECT run.envelope, run.updated_at_unix_ms,
                        event.envelope, event.created_at_unix_ms
                 FROM orchestra_runs AS run
                 JOIN orchestra_events AS event ON event.run_id = run.run_id
                 WHERE run.runtime_id = 'runtime-keep'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(unrelated_after_commit, unrelated_before);
        drop(connection);

        let replay = runtime.delete_orchestra_runtimes(&targets).unwrap();
        assert_eq!(replay.deleted_runtime_count, 0);
        assert_eq!(replay.deleted_run_count, 0);
        assert_eq!(replay.deleted_event_count, 0);
        drop(runtime);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn idempotent_orchestra_delete_receipt_survives_restart_and_conflicts_fail_closed() {
        let path = temp_journal("orchestra-delete-command");
        let mut runtime = ControlRuntime::open(&path).unwrap();
        persist_queued_orchestra_run(&mut runtime, "runtime-delete-command", "command");
        persist_queued_orchestra_run(&mut runtime, "runtime-delete-keep", "command-keep");
        let targets = vec!["runtime-delete-command".to_string()];
        let command_id = CommandId::new("orchestra-delete-command-a").unwrap();

        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "CREATE TRIGGER ignore_orchestra_delete_command
                 BEFORE DELETE ON orchestra_runs
                 WHEN OLD.runtime_id = 'runtime-delete-command'
                 BEGIN
                     SELECT RAISE(IGNORE);
                 END;",
            )
            .unwrap();
        assert!(
            runtime
                .delete_orchestra_runtimes_idempotent(command_id.clone(), &targets)
                .is_err()
        );
        let rollback_counts: (i64, i64) = connection
            .query_row(
                "SELECT
                     (SELECT COUNT(*) FROM orchestra_runs
                      WHERE runtime_id = 'runtime-delete-command'),
                     (SELECT COUNT(*) FROM orchestra_delete_operations)",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(rollback_counts, (1, 0));
        connection
            .execute_batch("DROP TRIGGER ignore_orchestra_delete_command;")
            .unwrap();
        drop(connection);

        let first = runtime
            .delete_orchestra_runtimes_idempotent(command_id.clone(), &targets)
            .unwrap();
        assert_eq!(first.operation_generation, 1);
        assert_eq!(first.runtime_ids, targets);
        assert_eq!(first.deleted_runtime_count, 1);
        assert_eq!(first.deleted_run_count, 1);
        assert_eq!(first.deleted_event_count, 1);
        assert!(!first.replayed);
        drop(runtime);

        let mut restarted = ControlRuntime::open(&path).unwrap();
        let replay = restarted
            .delete_orchestra_runtimes_idempotent(command_id.clone(), &targets)
            .unwrap();
        assert_eq!(replay.operation_generation, first.operation_generation);
        assert_eq!(replay.deleted_runtime_count, first.deleted_runtime_count);
        assert_eq!(replay.deleted_run_count, first.deleted_run_count);
        assert_eq!(replay.deleted_event_count, first.deleted_event_count);
        assert_eq!(replay.committed_at_unix_ms, first.committed_at_unix_ms);
        assert!(replay.replayed);
        assert!(matches!(
            restarted
                .delete_orchestra_runtimes_idempotent(command_id, &["runtime-delete-keep".into()]),
            Err(RuntimeError::Domain(
                DomainError::IdempotencyConflict { .. }
            ))
        ));
        let second = restarted
            .delete_orchestra_runtimes_idempotent(
                CommandId::new("orchestra-delete-command-b").unwrap(),
                &["runtime-delete-keep".into()],
            )
            .unwrap();
        assert_eq!(second.operation_generation, 2);
        assert!(!second.replayed);

        drop(restarted);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn orchestra_delete_audit_checkpoint_compacts_only_the_covered_prefix() {
        let path = temp_journal("orchestra-delete-audit-checkpoint");
        let targets = vec!["runtime-cleanup-checkpoint".to_string()];
        let mut runtime = ControlRuntime::open(&path).unwrap();
        for generation in 1..=3 {
            let receipt = runtime
                .delete_orchestra_runtimes_idempotent(
                    CommandId::new(format!("orchestra-cleanup-{generation}")).unwrap(),
                    &targets,
                )
                .unwrap();
            assert_eq!(receipt.operation_generation, generation);
        }
        assert_eq!(
            runtime.orchestra_delete_replay_horizon().unwrap(),
            OrchestraDeleteReplayHorizon {
                capacity: 4_096,
                retained: 3,
                oldest_generation: Some(1),
                newest_generation: Some(3),
                next_generation: 4,
                evicted_through_generation: 0,
                protected_from_generation: Some(1),
                checkpointed_through_generation: None,
            }
        );
        assert!(
            runtime
                .checkpoint_orchestra_delete_replay_horizon(3, 2)
                .is_err()
        );
        let checkpointed = runtime
            .checkpoint_orchestra_delete_replay_horizon(2, 3)
            .unwrap();
        assert_eq!(
            checkpointed,
            OrchestraDeleteReplayHorizon {
                capacity: 4_096,
                retained: 2,
                oldest_generation: Some(2),
                newest_generation: Some(3),
                next_generation: 4,
                evicted_through_generation: 1,
                protected_from_generation: Some(2),
                checkpointed_through_generation: Some(3),
            }
        );
        assert!(
            runtime
                .checkpoint_orchestra_delete_replay_horizon(1, 3)
                .is_err()
        );
        drop(runtime);

        let mut restarted = ControlRuntime::open(&path).unwrap();
        assert_eq!(
            restarted.orchestra_delete_replay_horizon().unwrap(),
            checkpointed
        );
        for generation in 2..=3 {
            let replay = restarted
                .delete_orchestra_runtimes_idempotent(
                    CommandId::new(format!("orchestra-cleanup-{generation}")).unwrap(),
                    &targets,
                )
                .unwrap();
            assert!(replay.replayed);
            assert_eq!(replay.operation_generation, generation);
        }
        drop(restarted);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn orchestra_delete_audit_checkpoint_rejects_corrupted_eviction_candidates() {
        let path = temp_journal("orchestra-delete-audit-checkpoint-corruption");
        let targets = vec!["runtime-cleanup-checkpoint-corruption".to_string()];
        let mut runtime = ControlRuntime::open(&path).unwrap();
        for generation in 1..=2 {
            runtime
                .delete_orchestra_runtimes_idempotent(
                    CommandId::new(format!("orchestra-cleanup-corruption-{generation}")).unwrap(),
                    &targets,
                )
                .unwrap();
        }

        let connection = Connection::open(&path).unwrap();
        connection
            .execute(
                "UPDATE orchestra_delete_operations
                 SET request = X'6E6F742D6A736F6E'
                 WHERE generation = 1",
                [],
            )
            .unwrap();
        drop(connection);

        assert!(
            runtime
                .checkpoint_orchestra_delete_replay_horizon(2, 2)
                .is_err()
        );
        assert_eq!(
            runtime.orchestra_delete_replay_horizon().unwrap(),
            OrchestraDeleteReplayHorizon {
                capacity: 4_096,
                retained: 2,
                oldest_generation: Some(1),
                newest_generation: Some(2),
                next_generation: 3,
                evicted_through_generation: 0,
                protected_from_generation: Some(1),
                checkpointed_through_generation: None,
            }
        );
        drop(runtime);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn orchestra_delete_pinned_horizon_reports_saturation_and_checkpoint_restores_admission() {
        let path = temp_journal("orchestra-delete-pinned-horizon");
        let mut runtime = ControlRuntime::open(&path).unwrap();
        let request = serde_json::to_vec(&vec!["runtime-cleanup-saturated".to_string()]).unwrap();
        let mut connection = Connection::open(&path).unwrap();
        let transaction = connection.transaction().unwrap();
        {
            let mut insert = transaction
                .prepare(
                    "INSERT INTO orchestra_delete_operations
                         (operation_id, generation, request, deleted_runtime_count,
                          deleted_run_count, deleted_event_count, committed_at_unix_ms)
                     VALUES (?1, ?2, ?3, 0, 0, 0, ?2)",
                )
                .unwrap();
            for generation in 1..=ORCHESTRA_DELETE_REPLAY_HORIZON {
                insert
                    .execute(params![
                        format!("orchestra-cleanup-saturated-{generation}"),
                        i64::try_from(generation).unwrap(),
                        &request,
                    ])
                    .unwrap();
            }
        }
        transaction
            .execute(
                "UPDATE orchestra_delete_generation SET next_generation = 4097 WHERE id = 1",
                [],
            )
            .unwrap();
        transaction
            .execute(
                "UPDATE orchestra_delete_replay_horizon
                 SET protected_from_generation = 1 WHERE id = 1",
                [],
            )
            .unwrap();
        transaction.commit().unwrap();
        drop(connection);

        let saturated = runtime.orchestra_delete_replay_horizon().unwrap();
        assert_eq!(saturated.retained, saturated.capacity);
        assert_eq!(saturated.available_capacity(), 0);
        assert!(saturated.saturated());
        assert!(saturated.admission_blocked());
        assert!(matches!(
            runtime.delete_orchestra_runtimes_idempotent(
                CommandId::new("orchestra-cleanup-saturated-overflow").unwrap(),
                &["runtime-cleanup-saturated".into()],
            ),
            Err(RuntimeError::OrchestraDeleteReplayHorizonSaturated)
        ));

        let checkpointed = runtime
            .checkpoint_orchestra_delete_replay_horizon(4_096, 4_096)
            .unwrap();
        assert_eq!(checkpointed.retained, 1);
        assert_eq!(checkpointed.available_capacity(), 4_095);
        assert!(!checkpointed.saturated());
        assert!(!checkpointed.admission_blocked());
        let admitted = runtime
            .delete_orchestra_runtimes_idempotent(
                CommandId::new("orchestra-cleanup-saturated-admitted").unwrap(),
                &["runtime-cleanup-saturated".into()],
            )
            .unwrap();
        assert_eq!(admitted.operation_generation, 4_097);

        drop(runtime);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn orchestra_delete_replay_admission_pressure_has_stable_threshold_boundaries() {
        let horizon = |available_capacity: u64,
                       protected_from_generation: Option<u64>|
         -> OrchestraDeleteReplayHorizon {
            OrchestraDeleteReplayHorizon {
                capacity: 4_096,
                retained: 4_096 - available_capacity,
                oldest_generation: Some(1),
                newest_generation: Some(4_096 - available_capacity),
                next_generation: 4_097 - available_capacity,
                evicted_through_generation: 0,
                protected_from_generation,
                checkpointed_through_generation: None,
            }
        };

        assert_eq!(
            horizon(513, Some(1)).admission_pressure(),
            OrchestraDeleteReplayAdmissionPressure::Healthy
        );
        assert_eq!(
            horizon(512, Some(1)).admission_pressure(),
            OrchestraDeleteReplayAdmissionPressure::Warning
        );
        assert_eq!(
            horizon(128, Some(1)).admission_pressure(),
            OrchestraDeleteReplayAdmissionPressure::Critical
        );
        assert_eq!(
            horizon(0, Some(1)).admission_pressure(),
            OrchestraDeleteReplayAdmissionPressure::Blocked
        );
        assert_eq!(
            horizon(0, None).admission_pressure(),
            OrchestraDeleteReplayAdmissionPressure::Healthy
        );
        assert!(!horizon(513, Some(1)).operator_action_required());
        assert!(horizon(512, Some(1)).operator_action_required());
        assert_eq!(
            horizon(256, Some(1)).admission_pressure_with_hysteresis(
                OrchestraDeleteReplayAdmissionPressure::Critical
            ),
            OrchestraDeleteReplayAdmissionPressure::Critical
        );
        assert_eq!(
            horizon(257, Some(1)).admission_pressure_with_hysteresis(
                OrchestraDeleteReplayAdmissionPressure::Critical
            ),
            OrchestraDeleteReplayAdmissionPressure::Warning
        );
        assert_eq!(
            horizon(768, Some(1)).admission_pressure_with_hysteresis(
                OrchestraDeleteReplayAdmissionPressure::Warning
            ),
            OrchestraDeleteReplayAdmissionPressure::Warning
        );
        assert_eq!(
            horizon(769, Some(1)).admission_pressure_with_hysteresis(
                OrchestraDeleteReplayAdmissionPressure::Warning
            ),
            OrchestraDeleteReplayAdmissionPressure::Healthy
        );

        let mut lagged = horizon(512, Some(1));
        assert_eq!(lagged.checkpoint_lag_generations(), lagged.retained);
        lagged.checkpointed_through_generation = Some(3_000);
        assert_eq!(
            lagged.checkpoint_lag_generations(),
            lagged.newest_generation.unwrap() - 3_000
        );
    }

    #[test]
    fn orchestra_append_validates_its_complete_post_write_snapshot_before_commit() {
        let path = temp_journal("orchestra-post-write-snapshot");
        let mut runtime = ControlRuntime::open(&path).unwrap();
        let run = br#"{"runId":"orun-post-write","runtimeId":"runtime-a","planId":"test","outcome":"queued","executedAt":"2026-01-01T00:00:00Z","completedAt":null,"requestId":"request-post-write"}"#;
        let event = br#"{"eventId":0,"runId":"orun-post-write","runtimeId":"runtime-a","eventType":"run_queued","fromOutcome":null,"toOutcome":"queued","summary":"","recordedAt":"2026-01-01T00:00:00Z"}"#;
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "CREATE TRIGGER corrupt_orchestra_event_after_insert
                 AFTER INSERT ON orchestra_events
                 BEGIN
                     UPDATE orchestra_events SET event_type = 'trigger_corrupted'
                     WHERE event_id = NEW.event_id;
                 END;",
            )
            .unwrap();
        assert!(
            runtime
                .persist_orchestra_run_event(
                    "orun-post-write",
                    "runtime-a",
                    Some("request-post-write"),
                    "run_queued",
                    None,
                    "queued",
                    "queued",
                    "2026-01-01T00:00:00Z",
                    run,
                    event,
                )
                .is_err()
        );
        let counts: (i64, i64) = connection
            .query_row(
                "SELECT
                     (SELECT COUNT(*) FROM orchestra_runs),
                     (SELECT COUNT(*) FROM orchestra_events)",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(counts, (0, 0));

        connection
            .execute_batch(
                "DROP TRIGGER corrupt_orchestra_event_after_insert;
                 CREATE TRIGGER drift_orchestra_generation_after_insert
                 AFTER INSERT ON orchestra_events
                 BEGIN
                     UPDATE orchestra_events
                     SET created_at_unix_ms = NEW.created_at_unix_ms + 1
                     WHERE event_id = NEW.event_id;
                 END;",
            )
            .unwrap();
        assert!(
            runtime
                .persist_orchestra_run_event(
                    "orun-post-write",
                    "runtime-a",
                    Some("request-post-write"),
                    "run_queued",
                    None,
                    "queued",
                    "queued",
                    "2026-01-01T00:00:00Z",
                    run,
                    event,
                )
                .is_err()
        );
        let counts: (i64, i64) = connection
            .query_row(
                "SELECT
                     (SELECT COUNT(*) FROM orchestra_runs),
                     (SELECT COUNT(*) FROM orchestra_events)",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(counts, (0, 0));
        connection
            .execute_batch("DROP TRIGGER drift_orchestra_generation_after_insert;")
            .unwrap();
        drop(connection);

        let stored = runtime
            .persist_orchestra_run_event(
                "orun-post-write",
                "runtime-a",
                Some("request-post-write"),
                "run_queued",
                None,
                "queued",
                "queued",
                "2026-01-01T00:00:00Z",
                run,
                event,
            )
            .unwrap();
        assert_eq!(stored.run, run);
        assert_eq!(stored.event, event);
        assert_eq!(stored.event_count, 1);

        drop(runtime);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn orchestra_append_rejects_corrupted_retained_history_before_replay_or_extension() {
        let path = temp_journal("orchestra-retained-append-fence");
        let mut runtime = ControlRuntime::open(&path).unwrap();
        let queued_run = br#"{"runId":"orun-retained","runtimeId":"runtime-a","planId":"test","outcome":"queued","executedAt":"2026-01-01T00:00:00Z","completedAt":null,"requestId":"request-retained"}"#;
        let queued_event = br#"{"eventId":0,"runId":"orun-retained","runtimeId":"runtime-a","eventType":"run_queued","fromOutcome":null,"toOutcome":"queued","summary":"","recordedAt":"2026-01-01T00:00:00Z"}"#;
        runtime
            .persist_orchestra_run_event(
                "orun-retained",
                "runtime-a",
                Some("request-retained"),
                "run_queued",
                None,
                "queued",
                "queued",
                "2026-01-01T00:00:00Z",
                queued_run,
                queued_event,
            )
            .unwrap();

        let connection = Connection::open(&path).unwrap();
        let corrupted_origin = br#"{"eventId":0,"runId":"orun-retained","runtimeId":"runtime-a","eventType":"run_queued","fromOutcome":"failed","toOutcome":"queued","summary":"","recordedAt":"2026-01-01T00:00:00Z"}"#;
        connection
            .execute(
                "UPDATE orchestra_events SET envelope = ?1 WHERE run_id = 'orun-retained'",
                [corrupted_origin.as_slice()],
            )
            .unwrap();
        assert!(
            runtime
                .persist_orchestra_run_event(
                    "orun-retained",
                    "runtime-a",
                    Some("request-retained"),
                    "run_queued",
                    Some("failed"),
                    "queued",
                    "queued",
                    "2026-01-01T00:00:00Z",
                    queued_run,
                    corrupted_origin,
                )
                .is_err()
        );
        connection
            .execute(
                "UPDATE orchestra_events SET envelope = ?1 WHERE run_id = 'orun-retained'",
                [queued_event.as_slice()],
            )
            .unwrap();

        let drifted_run = br#"{"runId":"orun-retained","runtimeId":"runtime-a","planId":"test","outcome":"queued","executedAt":"2026-01-01T00:00:00Z","completedAt":null,"requestId":"request-other"}"#;
        connection
            .execute(
                "UPDATE orchestra_runs SET envelope = ?1 WHERE run_id = 'orun-retained'",
                [drifted_run.as_slice()],
            )
            .unwrap();
        assert!(
            runtime
                .persist_orchestra_run_event(
                    "orun-retained",
                    "runtime-a",
                    Some("request-other"),
                    "run_queued",
                    None,
                    "queued",
                    "queued",
                    "2026-01-01T00:00:00Z",
                    drifted_run,
                    queued_event,
                )
                .is_err()
        );
        connection
            .execute(
                "UPDATE orchestra_runs SET envelope = ?1 WHERE run_id = 'orun-retained'",
                [queued_run.as_slice()],
            )
            .unwrap();

        let mismatched_predecessor = br#"{"eventId":0,"runId":"orun-retained","runtimeId":"runtime-a","eventType":"corrupted","fromOutcome":null,"toOutcome":"queued","summary":"","recordedAt":"2026-01-01T00:00:00Z"}"#;
        connection
            .execute(
                "UPDATE orchestra_events SET envelope = ?1 WHERE run_id = 'orun-retained'",
                [mismatched_predecessor.as_slice()],
            )
            .unwrap();
        let running_run = br#"{"runId":"orun-retained","runtimeId":"runtime-a","planId":"test","outcome":"running","executedAt":"2026-01-01T00:00:00Z","completedAt":null,"requestId":"request-retained"}"#;
        let running_event = br#"{"eventId":0,"runId":"orun-retained","runtimeId":"runtime-a","eventType":"run_started","fromOutcome":"queued","toOutcome":"running","summary":"","recordedAt":"2026-01-01T00:00:01Z"}"#;
        assert!(
            runtime
                .persist_orchestra_run_event(
                    "orun-retained",
                    "runtime-a",
                    Some("request-retained"),
                    "run_started",
                    Some("queued"),
                    "running",
                    "running",
                    "2026-01-01T00:00:01Z",
                    running_run,
                    running_event,
                )
                .is_err()
        );
        let retained: (Vec<u8>, i64) = connection
            .query_row(
                "SELECT envelope,
                        (SELECT COUNT(*) FROM orchestra_events WHERE run_id = 'orun-retained')
                 FROM orchestra_runs WHERE run_id = 'orun-retained'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(retained, (queued_run.to_vec(), 1));
        connection
            .execute(
                "UPDATE orchestra_events SET envelope = ?1 WHERE run_id = 'orun-retained'",
                [queued_event.as_slice()],
            )
            .unwrap();
        drop(connection);

        let extended = runtime
            .persist_orchestra_run_event(
                "orun-retained",
                "runtime-a",
                Some("request-retained"),
                "run_started",
                Some("queued"),
                "running",
                "running",
                "2026-01-01T00:00:01Z",
                running_run,
                running_event,
            )
            .unwrap();
        assert_eq!(extended.event_count, 2);

        drop(runtime);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn orchestra_event_sequence_is_fenced_inside_the_authority_transaction() {
        let path = temp_journal("orchestra-sequence-fence");
        let mut runtime = ControlRuntime::open(&path).unwrap();
        let queued_run = br#"{"runId":"orun-sequence","runtimeId":"runtime-a","planId":"test","outcome":"queued","executedAt":"2026-01-01T00:00:01Z","completedAt":null,"requestId":"request-sequence"}"#;
        let queued_event = br#"{"eventId":0,"runId":"orun-sequence","runtimeId":"runtime-a","eventType":"run_queued","fromOutcome":null,"toOutcome":"queued","summary":"","recordedAt":"2026-01-01T00:00:01Z"}"#;
        runtime
            .persist_orchestra_run_event(
                "orun-sequence",
                "runtime-a",
                Some("request-sequence"),
                "run_queued",
                None,
                "queued",
                "queued",
                "2026-01-01T00:00:01Z",
                queued_run,
                queued_event,
            )
            .unwrap();

        let succeeded_run = br#"{"runId":"orun-sequence","runtimeId":"runtime-a","planId":"test","outcome":"succeeded","executedAt":"2026-01-01T00:00:01Z","completedAt":"2026-01-01T00:00:03Z","requestId":"request-sequence"}"#;
        let skipped_event = br#"{"eventId":0,"runId":"orun-sequence","runtimeId":"runtime-a","eventType":"run_succeeded","fromOutcome":"queued","toOutcome":"succeeded","summary":"","recordedAt":"2026-01-01T00:00:03Z"}"#;
        assert!(
            runtime
                .persist_orchestra_run_event(
                    "orun-sequence",
                    "runtime-a",
                    Some("request-sequence"),
                    "run_succeeded",
                    Some("queued"),
                    "succeeded",
                    "succeeded",
                    "2026-01-01T00:00:03Z",
                    succeeded_run,
                    skipped_event,
                )
                .is_err()
        );

        let running_run = br#"{"runId":"orun-sequence","runtimeId":"runtime-a","planId":"test","outcome":"running","executedAt":"2026-01-01T00:00:01Z","completedAt":null,"requestId":"request-sequence"}"#;
        let running_event = br#"{"eventId":0,"runId":"orun-sequence","runtimeId":"runtime-a","eventType":"run_started","fromOutcome":"queued","toOutcome":"running","summary":"","recordedAt":"2026-01-01T00:00:02Z"}"#;
        for (from_outcome, recorded_at) in [
            (Some("failed"), "2026-01-01T00:00:02Z"),
            (Some("queued"), "2026-01-01T00:00:00Z"),
            (Some("queued"), "2026-01-01T01:00:00+02:00"),
        ] {
            let from_outcome_json = from_outcome
                .map(|outcome| format!("\"{outcome}\""))
                .unwrap_or_else(|| "null".into());
            let rejected_event = format!(
                "{{\"eventId\":0,\"runId\":\"orun-sequence\",\"runtimeId\":\"runtime-a\",\"eventType\":\"run_started\",\"fromOutcome\":{from_outcome_json},\"toOutcome\":\"running\",\"summary\":\"\",\"recordedAt\":\"{recorded_at}\"}}"
            );
            assert!(
                runtime
                    .persist_orchestra_run_event(
                        "orun-sequence",
                        "runtime-a",
                        Some("request-sequence"),
                        "run_started",
                        from_outcome,
                        "running",
                        "running",
                        recorded_at,
                        running_run,
                        rejected_event.as_bytes(),
                    )
                    .is_err()
            );
        }
        let running = runtime
            .persist_orchestra_run_event(
                "orun-sequence",
                "runtime-a",
                Some("request-sequence"),
                "run_started",
                Some("queued"),
                "running",
                "running",
                "2026-01-01T00:00:02Z",
                running_run,
                running_event,
            )
            .unwrap();
        assert_eq!(running.event_count, 2);
        let replay = runtime
            .persist_orchestra_run_event(
                "orun-sequence",
                "runtime-a",
                Some("request-sequence"),
                "run_started",
                Some("queued"),
                "running",
                "running",
                "2026-01-01T00:00:02Z",
                running_run,
                running_event,
            )
            .unwrap();
        assert_eq!(replay.event_count, 2);

        let succeeded = runtime
            .persist_orchestra_run_event(
                "orun-sequence",
                "runtime-a",
                Some("request-sequence"),
                "run_succeeded",
                Some("running"),
                "succeeded",
                "succeeded",
                "2026-01-01T00:00:03Z",
                succeeded_run,
                br#"{"eventId":0,"runId":"orun-sequence","runtimeId":"runtime-a","eventType":"run_succeeded","fromOutcome":"running","toOutcome":"succeeded","summary":"","recordedAt":"2026-01-01T00:00:03Z"}"#,
            )
            .unwrap();
        assert_eq!(succeeded.event_count, 3);
        assert!(
            runtime
                .persist_orchestra_run_event(
                    "orun-sequence",
                    "runtime-a",
                    Some("request-sequence"),
                    "run_failed_after_terminal",
                    Some("succeeded"),
                    "failed",
                    "failed",
                    "2026-01-01T00:00:04Z",
                    br#"{"runId":"orun-sequence","runtimeId":"runtime-a","planId":"test","outcome":"failed","executedAt":"2026-01-01T00:00:01Z","completedAt":"2026-01-01T00:00:04Z","requestId":"request-sequence"}"#,
                    br#"{"eventId":0,"runId":"orun-sequence","runtimeId":"runtime-a","eventType":"run_failed_after_terminal","fromOutcome":"succeeded","toOutcome":"failed","summary":"","recordedAt":"2026-01-01T00:00:04Z"}"#,
                )
                .is_err()
        );
        let history = runtime
            .load_orchestra_history(Some("runtime-a"), Some("orun-sequence"), 0, 64)
            .unwrap();
        assert_eq!(history.events.len(), 3);
        assert_eq!(
            history
                .events
                .iter()
                .map(|(event_id, _)| *event_id)
                .collect::<Vec<_>>(),
            [1, 2, 3]
        );
        let second_page = runtime
            .load_orchestra_history(Some("runtime-a"), Some("orun-sequence"), 1, 1)
            .unwrap();
        assert_eq!(second_page.events.len(), 1);
        assert_eq!(second_page.events[0].0, 2);
        assert_eq!(second_page.next_offset, Some(2));

        let connection = Connection::open(&path).unwrap();
        let corrupted_origin = br#"{"eventId":0,"runId":"orun-sequence","runtimeId":"runtime-a","eventType":"run_queued","fromOutcome":"failed","toOutcome":"queued","summary":"","recordedAt":"2026-01-01T00:00:01Z"}"#;
        connection
            .execute(
                "UPDATE orchestra_events SET envelope = ?1 WHERE event_id = 1",
                [corrupted_origin.as_slice()],
            )
            .unwrap();
        assert!(
            runtime
                .load_orchestra_history(Some("runtime-a"), Some("orun-sequence"), 1, 1)
                .is_err()
        );

        let mismatched_origin = br#"{"eventId":0,"runId":"orun-sequence","runtimeId":"runtime-a","eventType":"corrupted","fromOutcome":null,"toOutcome":"queued","summary":"","recordedAt":"2026-01-01T00:00:01Z"}"#;
        connection
            .execute(
                "UPDATE orchestra_events SET envelope = ?1 WHERE event_id = 1",
                [mismatched_origin.as_slice()],
            )
            .unwrap();
        assert!(
            runtime
                .load_orchestra_history(Some("runtime-a"), Some("orun-sequence"), 0, 64)
                .is_err()
        );
        drop(connection);
        drop(runtime);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn runtime_unregistration_is_atomic_durable_and_replayable() {
        let path = temp_journal("runtime-unregistration");
        let runtime_id = RuntimeId::new("runtime-a").unwrap();
        let command_id = CommandId::new("runtime-unregister-a").unwrap();
        let target;
        {
            let mut runtime = ControlRuntime::open(&path).unwrap();
            let projection = runtime
                .register_runtime(runtime_id.clone(), "Runtime A", "https://runtime-a.invalid")
                .unwrap();
            target = RuntimeUnregisterTarget {
                runtime_id: runtime_id.clone(),
                expected_revision: projection.revision,
            };
            runtime
                .persist_orchestra_run_event(
                    "orun-unregister",
                    runtime_id.as_str(),
                    Some("request-unregister"),
                    "run_queued",
                    None,
                    "queued",
                    "queued",
                    "2026-01-01T00:00:00Z",
                    br#"{"runId":"orun-unregister","runtimeId":"runtime-a","planId":"test","outcome":"queued","executedAt":"2026-01-01T00:00:00Z","completedAt":null,"requestId":"request-unregister"}"#,
                    br#"{"eventId":0,"runId":"orun-unregister","runtimeId":"runtime-a","eventType":"run_queued","fromOutcome":null,"toOutcome":"queued","summary":"","recordedAt":"2026-01-01T00:00:00Z"}"#,
                )
                .unwrap();

            let connection = Connection::open(&path).unwrap();
            connection
                .execute_batch(
                    "CREATE TRIGGER ignore_unregistration_orchestra_delete
                     BEFORE DELETE ON orchestra_runs
                     BEGIN
                         SELECT RAISE(IGNORE);
                     END;",
                )
                .unwrap();
            assert!(
                runtime
                    .unregister_runtimes(command_id.clone(), vec![target.clone()])
                    .is_err()
            );
            assert!(runtime.runtime_projection(&runtime_id).is_some());
            let rollback_counts: (i64, i64, i64, i64) = connection
                .query_row(
                    "SELECT
                         (SELECT COUNT(*) FROM orchestra_runs),
                         (SELECT COUNT(*) FROM orchestra_events),
                         (SELECT COUNT(*) FROM runtime_unregistration_operations),
                         (SELECT COUNT(*) FROM runtime_journal
                          WHERE kind = 'runtime_unregistration')",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .unwrap();
            assert_eq!(rollback_counts, (1, 1, 0, 0));
            connection
                .execute_batch("DROP TRIGGER ignore_unregistration_orchestra_delete;")
                .unwrap();
            connection
                .execute_batch(
                    "CREATE TRIGGER corrupt_unregistration_journal_after_insert
                     AFTER INSERT ON runtime_journal
                     WHEN NEW.kind = 'runtime_unregistration'
                     BEGIN
                         UPDATE runtime_journal SET payload = x'7b7d'
                         WHERE sequence = NEW.sequence;
                     END;",
                )
                .unwrap();
            assert!(matches!(
                runtime.unregister_runtimes(command_id.clone(), vec![target.clone()]),
                Err(RuntimeError::Storage(ref error))
                    if error == "runtime unregistration journal tombstone is inconsistent"
            ));
            let journal_fault_counts: (i64, i64, i64, i64) = connection
                .query_row(
                    "SELECT
                         (SELECT COUNT(*) FROM orchestra_runs),
                         (SELECT COUNT(*) FROM orchestra_events),
                         (SELECT COUNT(*) FROM runtime_unregistration_operations),
                         (SELECT COUNT(*) FROM runtime_journal
                          WHERE kind = 'runtime_unregistration')",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .unwrap();
            assert_eq!(journal_fault_counts, (1, 1, 0, 0));
            connection
                .execute_batch("DROP TRIGGER corrupt_unregistration_journal_after_insert;")
                .unwrap();
            drop(connection);

            let first = runtime
                .unregister_runtimes(command_id.clone(), vec![target.clone()])
                .unwrap();
            assert!(!first.replayed);
            assert!(first.operation_generation > 0);
            assert_eq!(first.removed.as_slice(), std::slice::from_ref(&target));
            assert_eq!(first.deleted_orchestra_runtime_count, 1);
            assert_eq!(first.deleted_orchestra_run_count, 1);
            assert_eq!(first.deleted_orchestra_event_count, 1);
            assert!(runtime.runtime_projection(&runtime_id).is_none());
            assert!(
                runtime
                    .load_orchestra_history(Some(runtime_id.as_str()), None, 0, 1)
                    .unwrap()
                    .runs
                    .is_empty()
            );
            let lookup = runtime
                .runtime_unregistration_receipt(command_id.clone())
                .unwrap();
            assert_eq!(lookup.command_id, command_id);
            assert_eq!(lookup.replay_horizon.retained, 1);
            assert_eq!(lookup.replay_horizon.oldest_generation, Some(1));
            let receipt = lookup.receipt.unwrap();
            assert_eq!(receipt.operation_generation, first.operation_generation);
            assert_eq!(receipt.removed.as_slice(), std::slice::from_ref(&target));
            assert_eq!(receipt.deleted_orchestra_runtime_count, 1);
            assert_eq!(receipt.deleted_orchestra_run_count, 1);
            assert_eq!(receipt.deleted_orchestra_event_count, 1);
            assert_eq!(receipt.removed_at_unix_ms, first.removed_at_unix_ms);
            let missing = runtime
                .runtime_unregistration_receipt(
                    CommandId::new("runtime-unregister-command-missing").unwrap(),
                )
                .unwrap();
            assert!(missing.receipt.is_none());
            assert_eq!(missing.replay_horizon, lookup.replay_horizon);

            let replay = runtime
                .unregister_runtimes(command_id.clone(), vec![target.clone()])
                .unwrap();
            assert!(replay.replayed);
            assert_eq!(replay.operation_generation, first.operation_generation);
            assert_eq!(replay.removed_at_unix_ms, first.removed_at_unix_ms);

            persist_queued_orchestra_run(
                &mut runtime,
                runtime_id.as_str(),
                "unregister-reappeared",
            );
            assert!(matches!(
                runtime.unregister_runtimes(command_id.clone(), vec![target.clone()]),
                Err(RuntimeError::Storage(ref error))
                    if error == "runtime unregistration Orchestra tombstone is inconsistent"
            ));
            assert!(matches!(
                runtime.runtime_unregistration_receipt(command_id.clone()),
                Err(RuntimeError::Storage(ref error))
                    if error == "runtime unregistration Orchestra tombstone is inconsistent"
            ));
            let cleanup = runtime
                .delete_orchestra_runtimes(&[runtime_id.as_str().to_string()])
                .unwrap();
            assert_eq!(cleanup.deleted_runtime_count, 1);
            assert_eq!(cleanup.deleted_run_count, 1);
            assert_eq!(cleanup.deleted_event_count, 1);

            let canonical_request = serde_json::to_vec(&vec![target.clone()]).unwrap();
            let mut noncanonical_request = vec![b' '];
            noncanonical_request.extend_from_slice(&canonical_request);
            let connection = Connection::open(&path).unwrap();
            connection
                .execute(
                    "UPDATE runtime_unregistration_operations SET request = ?1
                     WHERE operation_id = ?2",
                    params![noncanonical_request, command_id.as_str(),],
                )
                .unwrap();
            drop(connection);
            assert!(matches!(
                runtime.unregister_runtimes(command_id.clone(), vec![target.clone()]),
                Err(RuntimeError::Storage(ref error))
                    if error == "runtime unregistration operation request is not canonical"
            ));

            let connection = Connection::open(&path).unwrap();
            connection
                .execute(
                    "UPDATE runtime_unregistration_operations
                     SET request = ?1, deleted_runtime_count = 0
                     WHERE operation_id = ?2",
                    params![canonical_request, command_id.as_str()],
                )
                .unwrap();
            drop(connection);
            assert!(matches!(
                runtime.unregister_runtimes(command_id.clone(), vec![target.clone()]),
                Err(RuntimeError::Storage(ref error))
                    if error == "runtime unregistration operation receipt is inconsistent"
            ));

            let connection = Connection::open(&path).unwrap();
            connection
                .execute(
                    "UPDATE runtime_unregistration_operations
                     SET deleted_runtime_count = ?1
                     WHERE operation_id = ?2",
                    params![
                        i64::from(first.deleted_orchestra_runtime_count),
                        command_id.as_str(),
                    ],
                )
                .unwrap();
            drop(connection);
            assert!(
                runtime
                    .unregister_runtimes(command_id.clone(), vec![target.clone()])
                    .unwrap()
                    .replayed
            );

            let canonical_tombstone = serde_json::to_vec(&RuntimeUnregistration {
                runtime_id: runtime_id.as_str().to_string(),
            })
            .unwrap();
            let connection = Connection::open(&path).unwrap();
            connection
                .execute(
                    "UPDATE runtime_journal SET payload = x'7b7d'
                     WHERE kind = 'runtime_unregistration'
                       AND created_at_unix_ms = ?1",
                    [first.removed_at_unix_ms],
                )
                .unwrap();
            drop(connection);
            assert!(matches!(
                runtime.unregister_runtimes(command_id.clone(), vec![target.clone()]),
                Err(RuntimeError::Storage(ref error))
                    if error == "runtime unregistration journal tombstone is inconsistent"
            ));
            let connection = Connection::open(&path).unwrap();
            connection
                .execute(
                    "UPDATE runtime_journal SET payload = ?1
                     WHERE kind = 'runtime_unregistration'
                       AND created_at_unix_ms = ?2",
                    params![&canonical_tombstone, first.removed_at_unix_ms],
                )
                .unwrap();
            drop(connection);

            let connection = Connection::open(&path).unwrap();
            connection
                .execute(
                    "INSERT INTO runtime_journal
                         (kind, payload, created_at_unix_ms)
                     VALUES ('runtime_unregistration', ?1, ?2)",
                    params![&canonical_tombstone, first.removed_at_unix_ms],
                )
                .unwrap();
            let duplicate_sequence = connection.last_insert_rowid();
            drop(connection);
            assert!(matches!(
                runtime.unregister_runtimes(command_id.clone(), vec![target.clone()]),
                Err(RuntimeError::Storage(ref error))
                    if error == "runtime unregistration journal tombstone is inconsistent"
            ));
            let connection = Connection::open(&path).unwrap();
            connection
                .execute(
                    "DELETE FROM runtime_journal WHERE sequence = ?1",
                    [duplicate_sequence],
                )
                .unwrap();
            drop(connection);

            runtime.control.register_runtime(
                runtime_id.clone(),
                "Unexpected Runtime A",
                "https://unexpected-runtime-a.invalid",
            );
            assert!(matches!(
                runtime.unregister_runtimes(command_id.clone(), vec![target.clone()]),
                Err(RuntimeError::Storage(ref error))
                    if error == "runtime unregistration projection tombstone is inconsistent"
            ));
            assert!(runtime.control.unregister_runtime(&runtime_id));

            runtime.create_snapshot().unwrap();
            runtime.create_snapshot().unwrap();
            let connection = Connection::open(&path).unwrap();
            let compacted_journal_counts: (i64, i64) = connection
                .query_row(
                    "SELECT
                         (SELECT COUNT(*) FROM runtime_journal
                          WHERE kind = 'runtime_unregistration'
                            AND created_at_unix_ms = ?1),
                         (SELECT COUNT(*) FROM runtime_journal
                          WHERE kind = 'runtime_registration')",
                    [first.removed_at_unix_ms],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap();
            assert_eq!(compacted_journal_counts, (1, 0));
            drop(connection);
            assert!(
                runtime
                    .unregister_runtimes(command_id.clone(), vec![target.clone()])
                    .unwrap()
                    .replayed
            );
            assert!(matches!(
                runtime.unregister_runtimes(
                    command_id.clone(),
                    vec![RuntimeUnregisterTarget {
                        runtime_id: runtime_id.clone(),
                        expected_revision: Revision(target.expected_revision.0 + 1),
                    }],
                ),
                Err(RuntimeError::Domain(
                    DomainError::IdempotencyConflict { .. }
                ))
            ));
        }

        let mut recovered = ControlRuntime::open(&path).unwrap();
        assert!(recovered.runtime_projection(&runtime_id).is_none());
        assert!(
            recovered
                .unregister_runtimes(command_id, vec![target])
                .unwrap()
                .replayed
        );
        drop(recovered);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn runtime_unregistration_replay_horizon_rolls_over_atomically_and_compacts_safely() {
        let path = temp_journal("runtime-unregistration-horizon");
        let oldest_runtime_id = RuntimeId::new("runtime-horizon-oldest").unwrap();
        let oldest_command_id = CommandId::new("unregister-horizon-oldest").unwrap();
        let mut runtime = ControlRuntime::open(&path).unwrap();
        let oldest_projection = runtime
            .register_runtime(
                oldest_runtime_id.clone(),
                "Runtime Horizon Oldest",
                "https://runtime-horizon-oldest.invalid",
            )
            .unwrap();
        let oldest_target = RuntimeUnregisterTarget {
            runtime_id: oldest_runtime_id.clone(),
            expected_revision: oldest_projection.revision,
        };
        let oldest = runtime
            .unregister_runtimes(oldest_command_id.clone(), vec![oldest_target])
            .unwrap();
        let oldest_tombstone = serde_json::to_vec(&RuntimeUnregistration {
            runtime_id: oldest_runtime_id.as_str().to_string(),
        })
        .unwrap();

        let mut connection = Connection::open(&path).unwrap();
        let transaction = connection.transaction().unwrap();
        for index in 0..RUNTIME_UNREGISTRATION_REPLAY_HORIZON {
            let runtime_id = RuntimeId::new(format!("runtime-horizon-seed-{index:03}")).unwrap();
            let target = RuntimeUnregisterTarget {
                runtime_id: runtime_id.clone(),
                expected_revision: Revision(u64::try_from(index).unwrap() + 1),
            };
            let request = serde_json::to_vec(&vec![target]).unwrap();
            let tombstone = serde_json::to_vec(&RuntimeUnregistration {
                runtime_id: runtime_id.as_str().to_string(),
            })
            .unwrap();
            let removed_at_unix_ms = oldest
                .removed_at_unix_ms
                .checked_add(i64::try_from(index).unwrap() + 1)
                .unwrap();
            transaction
                .execute(
                    "INSERT INTO runtime_journal
                         (kind, payload, created_at_unix_ms)
                     VALUES ('runtime_unregistration', ?1, ?2)",
                    params![tombstone, removed_at_unix_ms],
                )
                .unwrap();
            transaction
                .execute(
                    "INSERT INTO runtime_unregistration_operations
                         (operation_id, generation, request, deleted_runtime_count, deleted_run_count,
                          deleted_event_count, removed_at_unix_ms)
                     VALUES (?1, ?2, ?3, 0, 0, 0, ?4)",
                    params![
                        format!("unregister-horizon-seed-{index:03}"),
                        i64::try_from(index).unwrap() + 2,
                        request,
                        removed_at_unix_ms,
                    ],
                )
                .unwrap();
        }
        transaction
            .execute(
                "UPDATE runtime_unregistration_replay_horizon
                 SET next_generation = ?1 WHERE id = 1",
                [i64::try_from(RUNTIME_UNREGISTRATION_REPLAY_HORIZON).unwrap() + 2],
            )
            .unwrap();
        transaction.commit().unwrap();
        let full_window: (i64, i64) = connection
            .query_row(
                "SELECT
                     (SELECT COUNT(*) FROM runtime_unregistration_operations),
                     (SELECT COUNT(*) FROM runtime_journal
                      WHERE kind = 'runtime_unregistration')",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(
            full_window,
            (
                i64::try_from(RUNTIME_UNREGISTRATION_REPLAY_HORIZON + 1).unwrap(),
                i64::try_from(RUNTIME_UNREGISTRATION_REPLAY_HORIZON + 1).unwrap(),
            )
        );
        let newest_seed_index = RUNTIME_UNREGISTRATION_REPLAY_HORIZON - 1;
        let newest_seed_target = RuntimeUnregisterTarget {
            runtime_id: RuntimeId::new(format!("runtime-horizon-seed-{newest_seed_index:03}"))
                .unwrap(),
            expected_revision: Revision(u64::try_from(newest_seed_index).unwrap() + 1),
        };
        assert!(
            runtime
                .unregister_runtimes(
                    CommandId::new(format!("unregister-horizon-seed-{newest_seed_index:03}"))
                        .unwrap(),
                    vec![newest_seed_target],
                )
                .unwrap()
                .replayed
        );
        let lookup_converged_window: (i64, i64, i64, i64) = connection
            .query_row(
                "SELECT
                     (SELECT COUNT(*) FROM runtime_unregistration_operations),
                     (SELECT COUNT(*) FROM runtime_unregistration_operations
                      WHERE operation_id = 'unregister-horizon-oldest'),
                     (SELECT COUNT(*) FROM runtime_unregistration_operations
                      WHERE operation_id = 'unregister-horizon-seed-000'),
                     (SELECT COUNT(*) FROM runtime_journal
                      WHERE kind = 'runtime_unregistration' AND payload = ?1)",
                [&oldest_tombstone],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(
            lookup_converged_window,
            (
                i64::try_from(RUNTIME_UNREGISTRATION_REPLAY_HORIZON).unwrap(),
                0,
                1,
                1,
            )
        );
        assert_eq!(
            runtime.runtime_unregistration_replay_horizon().unwrap(),
            RuntimeUnregistrationReplayHorizon {
                capacity: 256,
                retained: 256,
                oldest_generation: Some(2),
                newest_generation: Some(257),
                next_generation: 258,
                evicted_through_generation: 1,
            }
        );

        let boundary_runtime_id = RuntimeId::new("runtime-horizon-boundary").unwrap();
        let boundary_projection = runtime
            .register_runtime(
                boundary_runtime_id.clone(),
                "Runtime Horizon Boundary",
                "https://runtime-horizon-boundary.invalid",
            )
            .unwrap();
        let boundary_target = RuntimeUnregisterTarget {
            runtime_id: boundary_runtime_id.clone(),
            expected_revision: boundary_projection.revision,
        };
        let boundary_command_id = CommandId::new("unregister-horizon-boundary").unwrap();
        connection
            .execute_batch(
                "CREATE TRIGGER ignore_oldest_unregistration_operation_delete
                 BEFORE DELETE ON runtime_unregistration_operations
                 WHEN OLD.operation_id = 'unregister-horizon-seed-000'
                 BEGIN
                     SELECT RAISE(IGNORE);
                 END;",
            )
            .unwrap();
        assert!(matches!(
            runtime.unregister_runtimes(
                boundary_command_id.clone(),
                vec![boundary_target.clone()],
            ),
            Err(RuntimeError::Storage(ref error))
                if error
                    == "runtime unregistration replay horizon eviction is inconsistent"
        ));
        assert!(runtime.runtime_projection(&boundary_runtime_id).is_some());
        let failed_rollover: (i64, i64, i64) = connection
            .query_row(
                "SELECT
                     (SELECT COUNT(*) FROM runtime_unregistration_operations
                      WHERE operation_id = 'unregister-horizon-seed-000'),
                     (SELECT COUNT(*) FROM runtime_unregistration_operations
                      WHERE operation_id = 'unregister-horizon-boundary'),
                     (SELECT COUNT(*) FROM runtime_journal
                      WHERE kind = 'runtime_unregistration' AND payload = ?1)",
                [&oldest_tombstone],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(failed_rollover, (1, 0, 1));
        assert_eq!(
            runtime.runtime_unregistration_replay_horizon().unwrap(),
            RuntimeUnregistrationReplayHorizon {
                capacity: 256,
                retained: 256,
                oldest_generation: Some(2),
                newest_generation: Some(257),
                next_generation: 258,
                evicted_through_generation: 1,
            }
        );
        connection
            .execute_batch("DROP TRIGGER ignore_oldest_unregistration_operation_delete;")
            .unwrap();
        let boundary = runtime
            .unregister_runtimes(boundary_command_id, vec![boundary_target])
            .unwrap();
        assert!(!boundary.replayed);
        assert_eq!(boundary.operation_generation, 258);
        let rolled_window: (i64, i64, i64, i64, i64) = connection
            .query_row(
                "SELECT
                     (SELECT COUNT(*) FROM runtime_unregistration_operations),
                     (SELECT COUNT(*) FROM runtime_unregistration_operations
                      WHERE operation_id = 'unregister-horizon-oldest'),
                     (SELECT COUNT(*) FROM runtime_unregistration_operations
                      WHERE operation_id = 'unregister-horizon-seed-000'),
                     (SELECT COUNT(*) FROM runtime_unregistration_operations
                      WHERE operation_id = 'unregister-horizon-seed-001'),
                     (SELECT COUNT(*) FROM runtime_journal
                      WHERE kind = 'runtime_unregistration' AND payload = ?1)",
                [&oldest_tombstone],
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
            .unwrap();
        assert_eq!(
            rolled_window,
            (
                i64::try_from(RUNTIME_UNREGISTRATION_REPLAY_HORIZON).unwrap(),
                0,
                0,
                1,
                1,
            )
        );
        assert_eq!(
            runtime.runtime_unregistration_replay_horizon().unwrap(),
            RuntimeUnregistrationReplayHorizon {
                capacity: 256,
                retained: 256,
                oldest_generation: Some(3),
                newest_generation: Some(258),
                next_generation: 259,
                evicted_through_generation: 2,
            }
        );

        runtime.create_snapshot().unwrap();
        runtime.create_snapshot().unwrap();
        let compacted_oldest_tombstone: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM runtime_journal
                 WHERE kind = 'runtime_unregistration' AND payload = ?1",
                [&oldest_tombstone],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(compacted_oldest_tombstone, 0);

        let reused_projection = runtime
            .register_runtime(
                oldest_runtime_id.clone(),
                "Runtime Horizon Reused",
                "https://runtime-horizon-reused.invalid",
            )
            .unwrap();
        let reused_target = RuntimeUnregisterTarget {
            runtime_id: oldest_runtime_id.clone(),
            expected_revision: reused_projection.revision,
        };
        let reused = runtime
            .unregister_runtimes(oldest_command_id.clone(), vec![reused_target.clone()])
            .unwrap();
        assert!(!reused.replayed);
        assert_eq!(reused.operation_generation, 259);
        assert!(runtime.runtime_projection(&oldest_runtime_id).is_none());
        let reused_window: (i64, i64, i64, i64) = connection
            .query_row(
                "SELECT
                     (SELECT COUNT(*) FROM runtime_unregistration_operations),
                     (SELECT COUNT(*) FROM runtime_unregistration_operations
                      WHERE operation_id = 'unregister-horizon-oldest'),
                     (SELECT COUNT(*) FROM runtime_unregistration_operations
                      WHERE operation_id = 'unregister-horizon-seed-001'),
                     (SELECT COUNT(*) FROM runtime_journal
                      WHERE kind = 'runtime_unregistration' AND payload = ?1)",
                [&oldest_tombstone],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(
            reused_window,
            (
                i64::try_from(RUNTIME_UNREGISTRATION_REPLAY_HORIZON).unwrap(),
                1,
                0,
                1,
            )
        );
        assert_eq!(
            runtime.runtime_unregistration_replay_horizon().unwrap(),
            RuntimeUnregistrationReplayHorizon {
                capacity: 256,
                retained: 256,
                oldest_generation: Some(4),
                newest_generation: Some(259),
                next_generation: 260,
                evicted_through_generation: 3,
            }
        );

        runtime.create_snapshot().unwrap();
        runtime.create_snapshot().unwrap();
        drop(connection);
        drop(runtime);
        let mut recovered = ControlRuntime::open(&path).unwrap();
        assert!(recovered.runtime_projection(&oldest_runtime_id).is_none());
        assert!(
            recovered
                .unregister_runtimes(oldest_command_id, vec![reused_target])
                .unwrap()
                .replayed
        );
        drop(recovered);
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

    #[test]
    fn retirement_unregistration_is_atomic_and_restart_safe() {
        use leserpent_domain::bootstrap::CredentialHandle;
        use leserpent_domain::provisioning::GewyvernServiceReceipt;
        use leserpent_domain::retirement::{
            CAPABILITY_RUNTIME_RETIRE, GewyvernRetirementReceipt, RETIREMENT_DOMAIN_SCHEMA_VERSION,
            RuntimeRetirementIntent,
        };

        let path = temp_journal("retirement-unregistration");
        let mut runtime = ControlRuntime::open(&path).unwrap();
        let mut provisioning = planned_provisioning();
        let planned = provisioning.checkpoint(1).unwrap();
        runtime
            .enqueue_provisioning_effect(
                "provision-runtime-a-effect",
                "gewyvern.runtime.provision",
                b"provision-request",
                3,
                &planned,
            )
            .unwrap();
        let provision_lease = runtime
            .claim_effect("worker-a", Duration::from_secs(30))
            .unwrap()
            .unwrap();
        provisioning.begin().unwrap();
        provisioning
            .accept_service(GewyvernServiceReceipt {
                provisioning_id: planned.state.provisioning_id.clone(),
                runtime_id: planned.state.runtime_id.clone(),
                endpoint: "https://runtime-a.example:9411/".into(),
                api_credential_handle: CredentialHandle::new("vault:gewyvern-api:runtime-a")
                    .unwrap(),
                trust_credential_handle: CredentialHandle::new("vault:gewyvern-ca:runtime-a")
                    .unwrap(),
            })
            .unwrap();
        runtime
            .complete_provisioning_effect_and_register(
                &provision_lease,
                b"service-ready",
                &provisioning.checkpoint(2).unwrap(),
            )
            .unwrap();

        let retirement_id = RetirementId::new("retire-runtime-a").unwrap();
        let mut retirement = RuntimeRetirement::plan(
            &Principal {
                id: "operator-a".into(),
            },
            &CapabilitySet::new([CAPABILITY_RUNTIME_RETIRE]),
            RuntimeRetirementIntent {
                schema_version: RETIREMENT_DOMAIN_SCHEMA_VERSION,
                retirement_id: retirement_id.clone(),
                provisioning_id: planned.state.provisioning_id.clone(),
                runtime_id: planned.state.runtime_id.clone(),
                target: planned.state.target.clone(),
                retirement_credential_handle: CredentialHandle::new("vault:ssh:retire-runtime-a")
                    .unwrap(),
                requested_by: "operator-a".into(),
                confirmed: true,
            },
        )
        .unwrap();
        let retirement_planned = retirement.checkpoint(1).unwrap();
        runtime
            .enqueue_retirement_effect(
                "retire-runtime-a-effect",
                "gewyvern.runtime.retire",
                b"retirement-request",
                3,
                &retirement_planned,
            )
            .unwrap();
        let retirement_lease = runtime
            .claim_effect("worker-a", Duration::from_secs(30))
            .unwrap()
            .unwrap();
        retirement.begin().unwrap();
        retirement
            .accept_service_retirement(GewyvernRetirementReceipt {
                retirement_id: retirement_id.clone(),
                provisioning_id: planned.state.provisioning_id,
                runtime_id: planned.state.runtime_id.clone(),
                service_retired: true,
            })
            .unwrap();
        let state = runtime
            .complete_retirement_effect_and_unregister(
                &retirement_lease,
                b"service-retired",
                &retirement.checkpoint(2).unwrap(),
            )
            .unwrap();
        assert_eq!(state.phase, RetirementPhase::RuntimeUnregistered);
        assert!(
            runtime
                .runtime_projection(&planned.state.runtime_id)
                .is_none()
        );
        drop(runtime);

        let mut restarted = ControlRuntime::open(&path).unwrap();
        assert!(
            restarted
                .runtime_projection(&planned.state.runtime_id)
                .is_none()
        );
        let checkpoint = restarted
            .retirement_checkpoint(&retirement_id)
            .unwrap()
            .unwrap();
        assert_eq!(checkpoint.revision, 3);
        assert_eq!(checkpoint.state.phase, RetirementPhase::RuntimeUnregistered);
        drop(restarted);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn lost_retirement_lease_preserves_registration_and_planned_checkpoint() {
        use leserpent_domain::bootstrap::CredentialHandle;
        use leserpent_domain::provisioning::GewyvernServiceReceipt;
        use leserpent_domain::retirement::{
            CAPABILITY_RUNTIME_RETIRE, GewyvernRetirementReceipt, RETIREMENT_DOMAIN_SCHEMA_VERSION,
            RuntimeRetirementIntent,
        };

        let path = temp_journal("retirement-lease-rollback");
        let mut runtime = ControlRuntime::open(&path).unwrap();
        let mut provisioning = planned_provisioning();
        let provision_planned = provisioning.checkpoint(1).unwrap();
        runtime
            .enqueue_provisioning_effect(
                "provision-runtime-a-effect",
                "gewyvern.runtime.provision",
                b"provision-request",
                3,
                &provision_planned,
            )
            .unwrap();
        let provision_lease = runtime
            .claim_effect("worker-a", Duration::from_secs(30))
            .unwrap()
            .unwrap();
        provisioning.begin().unwrap();
        provisioning
            .accept_service(GewyvernServiceReceipt {
                provisioning_id: provision_planned.state.provisioning_id.clone(),
                runtime_id: provision_planned.state.runtime_id.clone(),
                endpoint: "https://runtime-a.example:9411/".into(),
                api_credential_handle: CredentialHandle::new("vault:gewyvern-api:runtime-a")
                    .unwrap(),
                trust_credential_handle: CredentialHandle::new("vault:gewyvern-ca:runtime-a")
                    .unwrap(),
            })
            .unwrap();
        runtime
            .complete_provisioning_effect_and_register(
                &provision_lease,
                b"service-ready",
                &provisioning.checkpoint(2).unwrap(),
            )
            .unwrap();

        let retirement_id = RetirementId::new("retire-runtime-a").unwrap();
        let mut retirement = RuntimeRetirement::plan(
            &Principal {
                id: "operator-a".into(),
            },
            &CapabilitySet::new([CAPABILITY_RUNTIME_RETIRE]),
            RuntimeRetirementIntent {
                schema_version: RETIREMENT_DOMAIN_SCHEMA_VERSION,
                retirement_id: retirement_id.clone(),
                provisioning_id: provision_planned.state.provisioning_id.clone(),
                runtime_id: provision_planned.state.runtime_id.clone(),
                target: provision_planned.state.target.clone(),
                retirement_credential_handle: CredentialHandle::new("vault:ssh:retire-runtime-a")
                    .unwrap(),
                requested_by: "operator-a".into(),
                confirmed: true,
            },
        )
        .unwrap();
        let retirement_planned = retirement.checkpoint(1).unwrap();
        runtime
            .enqueue_retirement_effect(
                "retire-runtime-a-effect",
                "gewyvern.runtime.retire",
                b"retirement-request",
                3,
                &retirement_planned,
            )
            .unwrap();
        let lease = runtime
            .claim_effect("worker-a", Duration::from_millis(1))
            .unwrap()
            .unwrap();
        retirement.begin().unwrap();
        retirement
            .accept_service_retirement(GewyvernRetirementReceipt {
                retirement_id: retirement_id.clone(),
                provisioning_id: provision_planned.state.provisioning_id,
                runtime_id: provision_planned.state.runtime_id.clone(),
                service_retired: true,
            })
            .unwrap();
        std::thread::sleep(Duration::from_millis(5));
        assert!(matches!(
            runtime.complete_retirement_effect_and_unregister(
                &lease,
                b"service-retired",
                &retirement.checkpoint(2).unwrap()
            ),
            Err(RuntimeError::Storage(ref error)) if error.contains("lease was lost or expired")
        ));
        assert!(
            runtime
                .runtime_projection(&provision_planned.state.runtime_id)
                .is_some()
        );
        assert_eq!(
            runtime
                .retirement_checkpoint(&retirement_id)
                .unwrap()
                .unwrap(),
            retirement_planned
        );
        drop(runtime);
        fs::remove_file(path).unwrap();
    }
}
