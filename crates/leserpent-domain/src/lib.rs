use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Serialize};

pub const DOMAIN_SCHEMA_VERSION: u32 = 1;
pub const COMMAND_PLAN_SCHEMA_VERSION: u32 = 1;
pub const DOMAIN_SNAPSHOT_SCHEMA_VERSION: u32 = 1;
pub const CAPABILITY_RUNTIME_READ: &str = "runtime.read";
pub const CAPABILITY_RUNTIME_REFRESH: &str = "runtime.refresh";
pub const RUNTIME_STATUS_REFRESH_EFFECT_KIND: &str = "gewyvern.status.refresh";

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct RuntimeId(String);

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct CommandId(String);

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct IdempotencyKey(String);

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct Revision(pub u64);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Principal {
    pub id: String,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct CapabilitySet(BTreeSet<String>);

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandOrigin {
    Gui,
    Cli,
    Leselang,
    Model,
    CompatibilityAdapter,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Confirmation {
    NotRequired,
    Confirmed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Command {
    RuntimeRefresh { runtime_id: RuntimeId },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Query {
    RuntimeList { filter: RuntimeListFilter },
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeTags {
    pub environment: Option<String>,
    pub cluster: Option<String>,
    pub role: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeListFilter {
    pub environment: Option<String>,
    pub cluster: Option<String>,
    pub role: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeStatusSnapshot {
    pub status_source: String,
    pub status_fetched_at: Option<String>,
    pub status_fetch_error: Option<String>,
    pub has_latest_snapshot: bool,
    pub snapshot_kind: Option<String>,
    pub target_count: Option<u64>,
    pub has_summary_json: bool,
    pub has_analysis_json: bool,
    pub has_training_example_json: bool,
    pub has_training_dataset_manifest: bool,
    pub has_export_json: bool,
    pub has_report_json: bool,
    pub has_report_html: bool,
    pub has_external_sidecar_context: bool,
    pub has_external_evidence_chain_enrichment: bool,
    pub has_external_diagnostic_opinion: bool,
    pub resilience_degraded: bool,
    pub resilience_status: Option<String>,
    pub resilience_summary: Option<String>,
    pub socket_service_status: Option<String>,
    pub socket_consecutive_idle_timeouts: Option<u64>,
    pub socket_total_idle_timeouts: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeStatusObservation {
    pub runtime_id: String,
    pub expected_revision: Revision,
    pub status: RuntimeStatusSnapshot,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CommandEnvelope {
    pub schema_version: u32,
    pub command_id: CommandId,
    pub idempotency_key: IdempotencyKey,
    pub expected_revision: Option<Revision>,
    pub principal: Principal,
    pub capabilities: CapabilitySet,
    pub origin: CommandOrigin,
    pub confirmation: Confirmation,
    pub dry_run: bool,
    pub command: Command,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct QueryEnvelope {
    pub schema_version: u32,
    pub principal: Principal,
    pub capabilities: CapabilitySet,
    pub query: Query,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CommandPlan {
    pub schema_version: u32,
    pub required_capability: String,
    pub operation: PlannedOperation,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
pub enum PlannedOperation {
    Query(QueryEnvelope),
    Command(CommandEnvelope),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommandPlanError {
    UnsupportedPlanSchema { actual: u32, expected: u32 },
    UnsupportedDomainSchema { actual: u32, expected: u32 },
    CapabilityMismatch { expected: &'static str },
    MissingCapability { capability: &'static str },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeProjection {
    pub id: RuntimeId,
    pub name: String,
    pub endpoint: String,
    pub revision: Revision,
    pub refresh_count: u64,
    pub refresh_status: RefreshStatus,
    pub tags: RuntimeTags,
    pub status: RuntimeStatusSnapshot,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RefreshStatus {
    NeverRequested,
    Pending,
    Ready,
    Failed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DomainEvent {
    RuntimeRefreshRequested {
        runtime_id: RuntimeId,
        revision: Revision,
        command_id: CommandId,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandStatus {
    Planned,
    Applied,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CommandResult {
    pub command_id: CommandId,
    pub status: CommandStatus,
    pub runtime: RuntimeProjection,
    pub events: Vec<DomainEvent>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DomainSnapshot {
    pub schema_version: u32,
    pub revision: Revision,
    pub runtimes: Vec<RuntimeProjection>,
    pub applied_commands: Vec<AppliedCommandSnapshot>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AppliedCommandSnapshot {
    pub principal_id: String,
    pub idempotency_key: IdempotencyKey,
    pub command: Command,
    pub result: CommandResult,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DomainSnapshotError {
    UnsupportedSchema { actual: u32, expected: u32 },
    Invalid { reason: &'static str },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum QueryResult {
    RuntimeList {
        revision: Revision,
        runtimes: Vec<RuntimeProjection>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DomainError {
    InvalidIdentifier {
        field: &'static str,
    },
    InvalidSchemaVersion {
        actual: u32,
        expected: u32,
    },
    Unauthorized {
        capability: &'static str,
    },
    RuntimeNotFound {
        runtime_id: String,
    },
    RevisionConflict {
        expected: Revision,
        actual: Revision,
    },
    IdempotencyConflict {
        key: String,
    },
}

#[derive(Clone, Debug)]
struct AppliedCommand {
    command: Command,
    result: CommandResult,
}

#[derive(Clone, Default)]
pub struct InMemoryControlPlane {
    revision: u64,
    runtimes: BTreeMap<RuntimeId, RuntimeProjection>,
    applied: BTreeMap<(String, IdempotencyKey), AppliedCommand>,
}

impl RuntimeId {
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        validated_identifier("runtime_id", value.into()).map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl CommandId {
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        validated_identifier("command_id", value.into()).map(Self)
    }
}

impl IdempotencyKey {
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        validated_identifier("idempotency_key", value.into()).map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl CapabilitySet {
    pub fn new(values: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self(values.into_iter().map(Into::into).collect())
    }

    pub fn contains(&self, capability: &str) -> bool {
        self.0.contains(capability)
    }
}

impl CommandPlan {
    pub fn validate(&self) -> Result<(), CommandPlanError> {
        if self.schema_version != COMMAND_PLAN_SCHEMA_VERSION {
            return Err(CommandPlanError::UnsupportedPlanSchema {
                actual: self.schema_version,
                expected: COMMAND_PLAN_SCHEMA_VERSION,
            });
        }
        let (required_capability, domain_schema, capabilities) = match &self.operation {
            PlannedOperation::Query(envelope) => match &envelope.query {
                Query::RuntimeList { .. } => (
                    CAPABILITY_RUNTIME_READ,
                    envelope.schema_version,
                    &envelope.capabilities,
                ),
            },
            PlannedOperation::Command(envelope) => match &envelope.command {
                Command::RuntimeRefresh { .. } => (
                    CAPABILITY_RUNTIME_REFRESH,
                    envelope.schema_version,
                    &envelope.capabilities,
                ),
            },
        };
        if domain_schema != DOMAIN_SCHEMA_VERSION {
            return Err(CommandPlanError::UnsupportedDomainSchema {
                actual: domain_schema,
                expected: DOMAIN_SCHEMA_VERSION,
            });
        }
        if self.required_capability != required_capability {
            return Err(CommandPlanError::CapabilityMismatch {
                expected: required_capability,
            });
        }
        if !capabilities.contains(required_capability) {
            return Err(CommandPlanError::MissingCapability {
                capability: required_capability,
            });
        }
        Ok(())
    }
}

impl fmt::Display for CommandPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedPlanSchema { actual, expected } => write!(
                formatter,
                "unsupported command plan schema {actual}, expected {expected}"
            ),
            Self::UnsupportedDomainSchema { actual, expected } => write!(
                formatter,
                "unsupported domain schema {actual}, expected {expected}"
            ),
            Self::CapabilityMismatch { expected } => {
                write!(formatter, "operation requires capability '{expected}'")
            }
            Self::MissingCapability { capability } => {
                write!(formatter, "plan is missing capability '{capability}'")
            }
        }
    }
}

impl std::error::Error for CommandPlanError {}

impl RuntimeListFilter {
    pub fn normalized(self) -> Self {
        Self {
            environment: normalize_filter_value(self.environment),
            cluster: normalize_filter_value(self.cluster),
            role: normalize_filter_value(self.role),
        }
    }
}

impl Default for RuntimeStatusSnapshot {
    fn default() -> Self {
        Self {
            status_source: "unobserved".to_string(),
            status_fetched_at: None,
            status_fetch_error: None,
            has_latest_snapshot: false,
            snapshot_kind: None,
            target_count: None,
            has_summary_json: false,
            has_analysis_json: false,
            has_training_example_json: false,
            has_training_dataset_manifest: false,
            has_export_json: false,
            has_report_json: false,
            has_report_html: false,
            has_external_sidecar_context: false,
            has_external_evidence_chain_enrichment: false,
            has_external_diagnostic_opinion: false,
            resilience_degraded: false,
            resilience_status: None,
            resilience_summary: None,
            socket_service_status: None,
            socket_consecutive_idle_timeouts: None,
            socket_total_idle_timeouts: None,
        }
    }
}

impl InMemoryControlPlane {
    pub fn snapshot(&self) -> DomainSnapshot {
        DomainSnapshot {
            schema_version: DOMAIN_SNAPSHOT_SCHEMA_VERSION,
            revision: Revision(self.revision),
            runtimes: self.runtimes.values().cloned().collect(),
            applied_commands: self
                .applied
                .iter()
                .map(
                    |((principal_id, idempotency_key), applied)| AppliedCommandSnapshot {
                        principal_id: principal_id.clone(),
                        idempotency_key: idempotency_key.clone(),
                        command: applied.command.clone(),
                        result: applied.result.clone(),
                    },
                )
                .collect(),
        }
    }

    pub fn from_snapshot(snapshot: DomainSnapshot) -> Result<Self, DomainSnapshotError> {
        snapshot.validate()?;
        let runtimes = snapshot
            .runtimes
            .into_iter()
            .map(|runtime| (runtime.id.clone(), runtime))
            .collect();
        let applied = snapshot
            .applied_commands
            .into_iter()
            .map(|entry| {
                (
                    (entry.principal_id, entry.idempotency_key),
                    AppliedCommand {
                        command: entry.command,
                        result: entry.result,
                    },
                )
            })
            .collect();
        Ok(Self {
            revision: snapshot.revision.0,
            runtimes,
            applied,
        })
    }

    pub fn register_runtime(
        &mut self,
        id: RuntimeId,
        name: impl Into<String>,
        endpoint: impl Into<String>,
    ) -> RuntimeProjection {
        self.register_runtime_with_metadata(
            id,
            name,
            endpoint,
            RuntimeTags::default(),
            RuntimeStatusSnapshot::default(),
        )
    }

    pub fn register_runtime_with_metadata(
        &mut self,
        id: RuntimeId,
        name: impl Into<String>,
        endpoint: impl Into<String>,
        tags: RuntimeTags,
        status: RuntimeStatusSnapshot,
    ) -> RuntimeProjection {
        self.revision += 1;
        let projection = RuntimeProjection {
            id: id.clone(),
            name: name.into(),
            endpoint: endpoint.into(),
            revision: Revision(self.revision),
            refresh_count: 0,
            refresh_status: RefreshStatus::NeverRequested,
            tags,
            status,
        };
        self.runtimes.insert(id, projection.clone());
        projection
    }

    pub fn query(&self, envelope: QueryEnvelope) -> Result<QueryResult, DomainError> {
        validate_schema(envelope.schema_version)?;
        validate_principal(&envelope.principal)?;
        require_capability(&envelope.capabilities, CAPABILITY_RUNTIME_READ)?;
        match envelope.query {
            Query::RuntimeList { filter } => {
                let filter = filter.normalized();
                let mut runtimes = self
                    .runtimes
                    .values()
                    .filter(|runtime| matches_filter(runtime, &filter))
                    .cloned()
                    .collect::<Vec<_>>();
                runtimes.sort_by(|left, right| {
                    left.name
                        .to_ascii_lowercase()
                        .cmp(&right.name.to_ascii_lowercase())
                        .then_with(|| left.id.cmp(&right.id))
                });
                Ok(QueryResult::RuntimeList {
                    revision: Revision(self.revision),
                    runtimes,
                })
            }
        }
    }

    pub fn execute(&mut self, envelope: CommandEnvelope) -> Result<CommandResult, DomainError> {
        validate_schema(envelope.schema_version)?;
        validate_principal(&envelope.principal)?;
        match &envelope.command {
            Command::RuntimeRefresh { .. } => {
                require_capability(&envelope.capabilities, CAPABILITY_RUNTIME_REFRESH)?;
            }
        }

        let idempotency_scope = (
            envelope.principal.id.clone(),
            envelope.idempotency_key.clone(),
        );
        if let Some(applied) = self.applied.get(&idempotency_scope) {
            return if applied.command == envelope.command {
                Ok(applied.result.clone())
            } else {
                Err(DomainError::IdempotencyConflict {
                    key: envelope.idempotency_key.as_str().to_string(),
                })
            };
        }

        match envelope.command.clone() {
            Command::RuntimeRefresh { runtime_id } => {
                let current = self.runtimes.get(&runtime_id).cloned().ok_or_else(|| {
                    DomainError::RuntimeNotFound {
                        runtime_id: runtime_id.as_str().to_string(),
                    }
                })?;
                if let Some(expected) = envelope.expected_revision
                    && expected != current.revision
                {
                    return Err(DomainError::RevisionConflict {
                        expected,
                        actual: current.revision,
                    });
                }

                let mut next = current;
                next.revision = Revision(self.revision + 1);
                next.refresh_count += 1;
                next.refresh_status = RefreshStatus::Pending;
                let event = DomainEvent::RuntimeRefreshRequested {
                    runtime_id: runtime_id.clone(),
                    revision: next.revision,
                    command_id: envelope.command_id.clone(),
                };
                let result = CommandResult {
                    command_id: envelope.command_id,
                    status: if envelope.dry_run {
                        CommandStatus::Planned
                    } else {
                        CommandStatus::Applied
                    },
                    runtime: next.clone(),
                    events: vec![event],
                };

                if !envelope.dry_run {
                    self.revision += 1;
                    self.runtimes.insert(runtime_id, next);
                    self.applied.insert(
                        idempotency_scope,
                        AppliedCommand {
                            command: envelope.command,
                            result: result.clone(),
                        },
                    );
                }
                Ok(result)
            }
        }
    }

    pub fn complete_runtime_status_refresh(
        &mut self,
        runtime_id: &RuntimeId,
        expected_revision: Revision,
        status: RuntimeStatusSnapshot,
    ) -> Result<RuntimeProjection, DomainError> {
        let current =
            self.runtimes
                .get(runtime_id)
                .cloned()
                .ok_or_else(|| DomainError::RuntimeNotFound {
                    runtime_id: runtime_id.as_str().to_string(),
                })?;
        if current.revision != expected_revision {
            return Err(DomainError::RevisionConflict {
                expected: expected_revision,
                actual: current.revision,
            });
        }

        self.revision += 1;
        let mut next = current;
        next.revision = Revision(self.revision);
        next.refresh_status = if status.status_fetch_error.is_some() {
            RefreshStatus::Failed
        } else {
            RefreshStatus::Ready
        };
        next.status = status;
        self.runtimes.insert(runtime_id.clone(), next.clone());
        Ok(next)
    }
}

impl DomainSnapshot {
    pub fn validate(&self) -> Result<(), DomainSnapshotError> {
        if self.schema_version != DOMAIN_SNAPSHOT_SCHEMA_VERSION {
            return Err(DomainSnapshotError::UnsupportedSchema {
                actual: self.schema_version,
                expected: DOMAIN_SNAPSHOT_SCHEMA_VERSION,
            });
        }
        let mut runtime_ids = BTreeSet::new();
        for runtime in &self.runtimes {
            validated_identifier("runtime_id", runtime.id.as_str().to_string()).map_err(|_| {
                DomainSnapshotError::Invalid {
                    reason: "invalid runtime identifier",
                }
            })?;
            if runtime.revision > self.revision {
                return Err(DomainSnapshotError::Invalid {
                    reason: "runtime revision exceeds snapshot revision",
                });
            }
            if !runtime_ids.insert(runtime.id.clone()) {
                return Err(DomainSnapshotError::Invalid {
                    reason: "duplicate runtime identifier",
                });
            }
        }
        let mut idempotency_scopes = BTreeSet::new();
        for applied in &self.applied_commands {
            validated_identifier("principal.id", applied.principal_id.clone()).map_err(|_| {
                DomainSnapshotError::Invalid {
                    reason: "invalid applied-command principal",
                }
            })?;
            validated_identifier(
                "idempotency_key",
                applied.idempotency_key.as_str().to_string(),
            )
            .map_err(|_| DomainSnapshotError::Invalid {
                reason: "invalid applied-command idempotency key",
            })?;
            if !idempotency_scopes.insert((
                applied.principal_id.clone(),
                applied.idempotency_key.clone(),
            )) {
                return Err(DomainSnapshotError::Invalid {
                    reason: "duplicate applied-command idempotency scope",
                });
            }
            let Command::RuntimeRefresh { runtime_id } = &applied.command;
            if runtime_id != &applied.result.runtime.id
                || !runtime_ids.contains(runtime_id)
                || applied.result.runtime.revision > self.revision
            {
                return Err(DomainSnapshotError::Invalid {
                    reason: "applied command does not match snapshot runtime state",
                });
            }
        }
        Ok(())
    }
}

impl fmt::Display for DomainSnapshotError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedSchema { actual, expected } => write!(
                formatter,
                "unsupported domain snapshot schema {actual}, expected {expected}"
            ),
            Self::Invalid { reason } => write!(formatter, "invalid domain snapshot: {reason}"),
        }
    }
}

impl std::error::Error for DomainSnapshotError {}

impl fmt::Display for DomainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidIdentifier { field } => write!(formatter, "invalid {field}"),
            Self::InvalidSchemaVersion { actual, expected } => {
                write!(
                    formatter,
                    "unsupported schema version {actual}, expected {expected}"
                )
            }
            Self::Unauthorized { capability } => {
                write!(formatter, "missing capability '{capability}'")
            }
            Self::RuntimeNotFound { runtime_id } => {
                write!(formatter, "runtime '{runtime_id}' was not found")
            }
            Self::RevisionConflict { expected, actual } => write!(
                formatter,
                "revision conflict: expected {}, actual {}",
                expected.0, actual.0
            ),
            Self::IdempotencyConflict { key } => {
                write!(formatter, "idempotency key '{key}' was reused")
            }
        }
    }
}

impl std::error::Error for DomainError {}

fn validated_identifier(field: &'static str, value: String) -> Result<String, DomainError> {
    let valid = !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':' | b'.'));
    valid
        .then_some(value)
        .ok_or(DomainError::InvalidIdentifier { field })
}

fn validate_schema(actual: u32) -> Result<(), DomainError> {
    (actual == DOMAIN_SCHEMA_VERSION)
        .then_some(())
        .ok_or(DomainError::InvalidSchemaVersion {
            actual,
            expected: DOMAIN_SCHEMA_VERSION,
        })
}

fn validate_principal(principal: &Principal) -> Result<(), DomainError> {
    validated_identifier("principal.id", principal.id.clone()).map(|_| ())
}

fn require_capability(
    capabilities: &CapabilitySet,
    capability: &'static str,
) -> Result<(), DomainError> {
    capabilities
        .contains(capability)
        .then_some(())
        .ok_or(DomainError::Unauthorized { capability })
}

fn normalize_filter_value(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

fn matches_filter(runtime: &RuntimeProjection, filter: &RuntimeListFilter) -> bool {
    matches_tag(&runtime.tags.environment, &filter.environment)
        && matches_tag(&runtime.tags.cluster, &filter.cluster)
        && matches_tag(&runtime.tags.role, &filter.role)
}

fn matches_tag(actual: &Option<String>, expected: &Option<String>) -> bool {
    expected.as_ref().is_none_or(|expected| {
        actual
            .as_ref()
            .is_some_and(|actual| actual.eq_ignore_ascii_case(expected))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn refresh_envelope(runtime_id: RuntimeId, command_id: &str, key: &str) -> CommandEnvelope {
        CommandEnvelope {
            schema_version: DOMAIN_SCHEMA_VERSION,
            command_id: CommandId::new(command_id).unwrap(),
            idempotency_key: IdempotencyKey::new(key).unwrap(),
            expected_revision: Some(Revision(1)),
            principal: Principal {
                id: "operator".to_string(),
            },
            capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_REFRESH]),
            origin: CommandOrigin::Cli,
            confirmation: Confirmation::NotRequired,
            dry_run: false,
            command: Command::RuntimeRefresh { runtime_id },
        }
    }

    #[test]
    fn runtime_list_is_sorted_and_capability_gated() {
        let mut control = InMemoryControlPlane::default();
        control.register_runtime(RuntimeId::new("runtime-b").unwrap(), "B", "http://b");
        control.register_runtime(RuntimeId::new("runtime-a").unwrap(), "A", "http://a");

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
        let QueryResult::RuntimeList { runtimes, .. } = result;
        assert_eq!(runtimes[0].id.as_str(), "runtime-a");
        assert_eq!(runtimes[1].id.as_str(), "runtime-b");
    }

    #[test]
    fn snapshot_restore_preserves_projection_and_idempotency() {
        let runtime_id = RuntimeId::new("runtime-a").unwrap();
        let mut control = InMemoryControlPlane::default();
        control.register_runtime(runtime_id.clone(), "A", "http://a");
        let envelope = refresh_envelope(runtime_id, "command-a", "effect-a");
        let first = control.execute(envelope.clone()).unwrap();

        let snapshot = control.snapshot();
        snapshot.validate().unwrap();
        let mut restored = InMemoryControlPlane::from_snapshot(snapshot).unwrap();
        assert_eq!(restored.execute(envelope).unwrap(), first);
        assert_eq!(restored.snapshot(), control.snapshot());
    }

    #[test]
    fn runtime_refresh_is_idempotent_and_revision_checked() {
        let mut control = InMemoryControlPlane::default();
        let runtime_id = RuntimeId::new("runtime-a").unwrap();
        control.register_runtime(runtime_id.clone(), "A", "http://a");
        let other_runtime_id = RuntimeId::new("runtime-b").unwrap();
        control.register_runtime(other_runtime_id.clone(), "B", "http://b");
        let command = refresh_envelope(runtime_id.clone(), "command-1", "refresh-a");
        let mut command = command;
        command.expected_revision = Some(Revision(1));

        let first = control.execute(command.clone()).unwrap();
        let replay = control.execute(command).unwrap();
        assert_eq!(first, replay);
        assert_eq!(first.runtime.refresh_count, 1);

        let mut conflicting = refresh_envelope(other_runtime_id, "command-2", "refresh-a");
        conflicting.expected_revision = Some(Revision(2));
        assert!(matches!(
            control.execute(conflicting),
            Err(DomainError::IdempotencyConflict { .. })
        ));
    }

    #[test]
    fn dry_run_does_not_consume_revision_or_idempotency_key() {
        let mut control = InMemoryControlPlane::default();
        let runtime_id = RuntimeId::new("runtime-a").unwrap();
        control.register_runtime(runtime_id.clone(), "A", "http://a");
        let mut command = refresh_envelope(runtime_id, "command-1", "refresh-a");
        command.dry_run = true;

        let preview = control.execute(command.clone()).unwrap();
        assert_eq!(preview.status, CommandStatus::Planned);
        command.dry_run = false;
        let applied = control.execute(command).unwrap();
        assert_eq!(applied.status, CommandStatus::Applied);
        assert_eq!(applied.runtime.revision, Revision(2));
    }

    #[test]
    fn runtime_refresh_rejects_missing_capability_and_stale_revision() {
        let mut control = InMemoryControlPlane::default();
        let runtime_id = RuntimeId::new("runtime-a").unwrap();
        control.register_runtime(runtime_id.clone(), "A", "http://a");

        let mut unauthorized = refresh_envelope(runtime_id.clone(), "command-1", "refresh-a");
        unauthorized.capabilities = CapabilitySet::default();
        assert!(matches!(
            control.execute(unauthorized),
            Err(DomainError::Unauthorized {
                capability: CAPABILITY_RUNTIME_REFRESH
            })
        ));

        let mut stale = refresh_envelope(runtime_id, "command-2", "refresh-b");
        stale.expected_revision = Some(Revision(99));
        assert!(matches!(
            control.execute(stale),
            Err(DomainError::RevisionConflict {
                expected: Revision(99),
                actual: Revision(1)
            })
        ));
    }

    #[test]
    fn idempotency_keys_are_scoped_to_the_principal() {
        let mut control = InMemoryControlPlane::default();
        let runtime_id = RuntimeId::new("runtime-a").unwrap();
        control.register_runtime(runtime_id.clone(), "A", "http://a");

        let first = refresh_envelope(runtime_id.clone(), "command-1", "shared-key");
        control.execute(first).unwrap();

        let mut second = refresh_envelope(runtime_id, "command-2", "shared-key");
        second.principal.id = "another-operator".to_string();
        second.expected_revision = Some(Revision(2));
        let result = control.execute(second).unwrap();
        assert_eq!(result.runtime.revision, Revision(3));
        assert_eq!(result.runtime.refresh_count, 2);
    }

    #[test]
    fn runtime_list_matches_legacy_filter_and_name_ordering() {
        let mut control = InMemoryControlPlane::default();
        control.register_runtime_with_metadata(
            RuntimeId::new("runtime-z").unwrap(),
            "alpha",
            "http://z",
            RuntimeTags {
                environment: Some("Production".to_string()),
                cluster: Some("east".to_string()),
                role: Some("edge".to_string()),
            },
            RuntimeStatusSnapshot::default(),
        );
        control.register_runtime_with_metadata(
            RuntimeId::new("runtime-a").unwrap(),
            "Bravo",
            "http://a",
            RuntimeTags {
                environment: Some("production".to_string()),
                cluster: Some("west".to_string()),
                role: Some("edge".to_string()),
            },
            RuntimeStatusSnapshot::default(),
        );

        let result = control
            .query(QueryEnvelope {
                schema_version: DOMAIN_SCHEMA_VERSION,
                principal: Principal {
                    id: "operator".to_string(),
                },
                capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
                query: Query::RuntimeList {
                    filter: RuntimeListFilter {
                        environment: Some(" production ".to_string()),
                        cluster: None,
                        role: Some("EDGE".to_string()),
                    },
                },
            })
            .unwrap();
        let QueryResult::RuntimeList { runtimes, .. } = result;
        assert_eq!(runtimes.len(), 2);
        assert_eq!(runtimes[0].id.as_str(), "runtime-z");
        assert_eq!(runtimes[1].id.as_str(), "runtime-a");
    }

    #[test]
    fn status_refresh_completion_is_revision_checked_and_records_failures() {
        let mut control = InMemoryControlPlane::default();
        let runtime_id = RuntimeId::new("runtime-a").unwrap();
        control.register_runtime(runtime_id.clone(), "A", "http://a");
        let requested = control
            .execute(refresh_envelope(
                runtime_id.clone(),
                "command-1",
                "refresh-a",
            ))
            .unwrap();
        assert_eq!(requested.runtime.refresh_status, RefreshStatus::Pending);

        let failed_status = RuntimeStatusSnapshot {
            status_source: "fetch_failed".to_string(),
            status_fetch_error: Some("connection refused".to_string()),
            ..RuntimeStatusSnapshot::default()
        };
        assert!(matches!(
            control.complete_runtime_status_refresh(
                &runtime_id,
                Revision(1),
                failed_status.clone()
            ),
            Err(DomainError::RevisionConflict { .. })
        ));
        let completed = control
            .complete_runtime_status_refresh(&runtime_id, Revision(2), failed_status)
            .unwrap();
        assert_eq!(completed.refresh_status, RefreshStatus::Failed);
        assert_eq!(completed.revision, Revision(3));
    }
}
