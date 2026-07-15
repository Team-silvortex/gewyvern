use leserpent_domain::{
    CommandPlan, CommandPlanError, CommandResult, DOMAIN_SNAPSHOT_SCHEMA_VERSION, DomainError,
    DomainSnapshot, DomainSnapshotError, InMemoryControlPlane, PlannedOperation, QueryResult,
    RuntimeId, RuntimeProjection,
};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::Path;
use std::time::Duration;

mod persistence;

pub use persistence::EffectLease;
use persistence::{Journal, JournalEntryKind};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
pub enum PlanResult {
    Query(QueryResult),
    Command(CommandResult),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeError {
    InvalidPlan(CommandPlanError),
    Domain(DomainError),
    InvalidSnapshot(DomainSnapshotError),
    Storage(String),
    ReplayMismatch { sequence: i64 },
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPlan(error) => write!(formatter, "invalid command plan: {error}"),
            Self::Domain(error) => write!(formatter, "domain execution failed: {error}"),
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
            Self::InvalidSnapshot(error) => Some(error),
            Self::Storage(_) | Self::ReplayMismatch { .. } => None,
        }
    }
}

#[derive(Default)]
pub struct ControlRuntime {
    control: InMemoryControlPlane,
    journal: Option<Journal>,
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
        let (control, through_sequence) = match restored {
            Some(restored) => restored,
            None if snapshot_error.is_none() => (InMemoryControlPlane::default(), 0),
            None => {
                return Err(RuntimeError::Storage(snapshot_error.unwrap()));
            }
        };
        let entries = journal
            .load(through_sequence)
            .map_err(RuntimeError::Storage)?;
        let mut runtime = Self {
            control,
            journal: None,
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
            }
        }
        runtime.journal = Some(journal);
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

    pub fn claim_effect(
        &mut self,
        worker_id: &str,
        lease_duration: Duration,
    ) -> Result<Option<EffectLease>, RuntimeError> {
        let Some(journal) = &mut self.journal else {
            return Err(RuntimeError::Storage(
                "effect scheduling requires persistent storage".into(),
            ));
        };
        journal
            .claim_effect(worker_id, lease_duration)
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

    pub fn execute_plan(&mut self, plan: CommandPlan) -> Result<PlanResult, RuntimeError> {
        if let Some(journal) = &mut self.journal {
            journal.ensure_owner().map_err(RuntimeError::Storage)?;
        }
        plan.validate().map_err(RuntimeError::InvalidPlan)?;
        match plan.operation {
            PlannedOperation::Query(query) => self
                .control
                .query(query)
                .map(PlanResult::Query)
                .map_err(RuntimeError::Domain),
            PlannedOperation::Command(command) => {
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
                Ok(PlanResult::Command(result))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use leselang_command::{LoweringContext, PlannedOperation, lower_effect};
    use leselang_hir::lower;
    use leselang_syntax::parse;
    use leserpent_domain::{
        CAPABILITY_RUNTIME_READ, CAPABILITY_RUNTIME_REFRESH, CapabilitySet, CommandId,
        CommandOrigin, Confirmation, IdempotencyKey, Principal, QueryResult, Revision,
    };
    use rusqlite::Connection;
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

    fn list_plan() -> CommandPlan {
        let program = lower(&parse("fn main() = runtime.list()")).unwrap();
        lower_effect(&program.function.effect, &context()).unwrap()
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
        assert_eq!(schema, 6);
        assert_eq!(migration_count, 6);
        assert_eq!(legacy_timestamp, 0);
        drop(connection);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn sqlite_journal_rejects_incomplete_current_schema() {
        let path = temp_journal("incomplete-v6");
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
                 INSERT INTO runtime_metadata (key, value) VALUES ('schema_version', 6);",
            )
            .unwrap();
        drop(connection);

        assert!(matches!(
            ControlRuntime::open(&path),
            Err(RuntimeError::Storage(ref error)) if error.contains("invalid runtime journal schema 6")
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
        assert_eq!(schema, 6);
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
}
