use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use http::Uri;
use serde::{Deserialize, Serialize};

pub mod bootstrap;
pub mod provisioning;
pub mod retirement;

pub const DOMAIN_SCHEMA_VERSION: u32 = 1;
pub const COMMAND_PLAN_SCHEMA_VERSION: u32 = 1;
pub const DOMAIN_SNAPSHOT_SCHEMA_VERSION: u32 = 1;
pub const CAPABILITY_RUNTIME_READ: &str = "runtime.read";
pub const CAPABILITY_RUNTIME_REGISTER: &str = "runtime.register";
pub const CAPABILITY_RUNTIME_REFRESH: &str = "runtime.refresh";
pub const CAPABILITY_RUNTIME_DEPLOY: &str = "runtime.deploy";
pub const CAPABILITY_ORCHESTRA_WRITE: &str = "orchestra.write";
pub const CAPABILITY_DEBUGGER_CONTROL: &str = "debugger.control";
pub const RUNTIME_STATUS_REFRESH_EFFECT_KIND: &str = "gewyvern.status.refresh";
pub const RUNTIME_CAPABILITY_DISCOVERY_EFFECT_KIND: &str = "gewyvern.capabilities.discover";
pub const RUNTIME_DEPLOYMENT_EFFECT_KIND: &str = "gewyvern.deployment.submit";
pub const MAX_RUNTIME_HISTORY_ENTRIES: usize = 32;
pub const MAX_RUNTIME_LOG_QUERY_ENTRIES: u16 = 256;
pub const MAX_RUNTIME_LOG_MESSAGE_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct RuntimeId(String);

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct CommandId(String);

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
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
#[serde(deny_unknown_fields, tag = "kind", rename_all = "snake_case")]
pub enum Command {
    RuntimeRegister {
        runtime_id: RuntimeId,
        name: String,
        endpoint: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sidecar_endpoint: Option<String>,
        tags: RuntimeTags,
    },
    RuntimeRegistrationUpdate {
        runtime_id: RuntimeId,
        name: String,
        endpoint: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sidecar_endpoint: Option<String>,
        tags: RuntimeTags,
    },
    RuntimeDiscoveryIntake {
        runtime_id: RuntimeId,
        capabilities: Option<Box<RuntimeCapabilitySnapshot>>,
        status: Option<Box<RuntimeStatusSnapshot>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sidecar_status: Option<Box<RuntimeSidecarStatusSnapshot>>,
    },
    RuntimeRefresh {
        runtime_id: RuntimeId,
    },
    RuntimeCapabilitiesRefresh {
        runtime_id: RuntimeId,
    },
    RuntimeDeploy {
        runtime_id: RuntimeId,
        pipeline_kind: String,
        target: Option<String>,
    },
    DebuggerCancel {
        session_id: String,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Query {
    RuntimeList {
        filter: RuntimeListFilter,
    },
    RuntimeInspect {
        runtime_id: RuntimeId,
    },
    RuntimeHistory {
        runtime_id: RuntimeId,
    },
    RuntimeLogs {
        runtime_id: RuntimeId,
        after_sequence: Option<u64>,
        limit: u16,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeLogLevel {
    Trace,
    Debug,
    Info,
    Warning,
    Error,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeLogRecord {
    pub sequence: u64,
    pub level: RuntimeLogLevel,
    pub message: String,
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
pub struct RuntimeSidecarMemorySlotSnapshot {
    pub slot: String,
    pub label: Option<String>,
    pub note: Option<String>,
    pub source: String,
    pub saved_at: Option<String>,
    pub pattern_count: u64,
    pub label_count: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeSidecarMemorySnapshot {
    pub versions_supported: bool,
    pub slot_count: u64,
    pub history_count: u64,
    pub latest_slot: Option<String>,
    pub latest_label: Option<String>,
    pub latest_source: Option<String>,
    pub slots: Vec<RuntimeSidecarMemorySlotSnapshot>,
    pub fetch_error: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeSidecarStatusSnapshot {
    pub status_source: String,
    pub status_fetched_at: Option<String>,
    pub status_fetch_error: Option<String>,
    pub healthy: bool,
    pub daemon_status: String,
    pub target_count: Option<u64>,
    pub learning_active: bool,
    pub learned_routes: u64,
    pub has_evidence_chain_enrichment: bool,
    pub has_diagnostic_opinion: bool,
    pub last_error: Option<String>,
    pub memory: Option<RuntimeSidecarMemorySnapshot>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeStatusObservation {
    pub runtime_id: String,
    pub expected_revision: Revision,
    pub status: RuntimeStatusSnapshot,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeStatusRefreshRequest {
    pub runtime_id: String,
    pub expected_revision: Revision,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeCapabilitySnapshot {
    pub source: String,
    pub service: String,
    pub version: String,
    pub latest_snapshot: bool,
    pub authenticated_deployment: bool,
    pub serve_required: bool,
    pub external_sidecar_context: bool,
    pub target_path_segment_encoding: String,
    pub target_direct_path_chars: String,
    pub endpoints: Vec<String>,
    pub extensions: BTreeMap<String, bool>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeCapabilityObservation {
    pub runtime_id: String,
    pub expected_revision: Revision,
    pub capabilities: RuntimeCapabilitySnapshot,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeCapabilityRefreshRequest {
    pub runtime_id: String,
    pub expected_revision: Revision,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeDeploymentRequest {
    pub runtime_id: String,
    pub request_id: String,
    pub pipeline_kind: String,
    pub requested_by: String,
    pub confirmed: bool,
    pub target: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeDeploymentOutcome {
    pub deployment_id: String,
    pub request_id: String,
    pub pipeline_kind: String,
    pub requested_by: String,
    pub status: String,
    pub accepted_unix_ms: u128,
    pub target: Option<String>,
    pub replayed: bool,
}

impl RuntimeCapabilitySnapshot {
    pub fn is_unobserved(&self) -> bool {
        self == &Self::default()
    }
}

fn command_result_projection(mut runtime: RuntimeProjection) -> RuntimeProjection {
    runtime.registered_at_unix_ms = None;
    runtime.updated_at_unix_ms = None;
    runtime
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
    InvalidPrincipal,
    InvalidDebuggerSessionId,
    InvalidRegistrationIntent,
    RegistrationConfirmationRequired,
    InvalidDeploymentIntent,
    DeploymentConfirmationRequired,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeProjection {
    pub id: RuntimeId,
    pub name: String,
    pub endpoint: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sidecar_endpoint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub registered_at_unix_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at_unix_ms: Option<u64>,
    pub revision: Revision,
    pub refresh_count: u64,
    pub refresh_status: RefreshStatus,
    pub tags: RuntimeTags,
    pub status: RuntimeStatusSnapshot,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sidecar_status: Option<RuntimeSidecarStatusSnapshot>,
    #[serde(default)]
    pub capabilities: RuntimeCapabilitySnapshot,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capabilities_observed_for_revision: Option<Revision>,
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
    RuntimeRegistered {
        runtime_id: RuntimeId,
        revision: Revision,
        command_id: CommandId,
    },
    RuntimeRegistrationUpdated {
        runtime_id: RuntimeId,
        revision: Revision,
        command_id: CommandId,
    },
    RuntimeDiscoveryIntakeApplied {
        runtime_id: RuntimeId,
        revision: Revision,
        command_id: CommandId,
    },
    RuntimeRefreshRequested {
        runtime_id: RuntimeId,
        revision: Revision,
        command_id: CommandId,
    },
    RuntimeCapabilitiesRefreshRequested {
        runtime_id: RuntimeId,
        revision: Revision,
        command_id: CommandId,
    },
    RuntimeDeploymentRequested {
        runtime_id: RuntimeId,
        revision: Revision,
        command_id: CommandId,
        request_id: String,
        pipeline_kind: String,
        requested_by: String,
        target: Option<String>,
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
// Keep the established wire/domain shape source-compatible until the v2 schema seal.
#[allow(clippy::large_enum_variant)]
pub enum QueryResult {
    RuntimeList {
        revision: Revision,
        runtimes: Vec<RuntimeProjection>,
    },
    RuntimeInspect {
        revision: Revision,
        runtime: RuntimeProjection,
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
    RuntimeAlreadyExists {
        runtime_id: String,
    },
    RuntimeEndpointConflict {
        runtime_id: String,
    },
    RevisionConflict {
        expected: Revision,
        actual: Revision,
    },
    IdempotencyConflict {
        key: String,
    },
    ConfirmationRequired,
    InvalidQuery {
        reason: &'static str,
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

    pub fn as_str(&self) -> &str {
        &self.0
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

macro_rules! impl_validated_identifier_deserialize {
    ($identifier:ident) => {
        impl<'de> Deserialize<'de> for $identifier {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Self::new(value).map_err(serde::de::Error::custom)
            }
        }
    };
}

impl_validated_identifier_deserialize!(RuntimeId);
impl_validated_identifier_deserialize!(CommandId);
impl_validated_identifier_deserialize!(IdempotencyKey);

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
        let principal = match &self.operation {
            PlannedOperation::Query(envelope) => &envelope.principal,
            PlannedOperation::Command(envelope) => &envelope.principal,
        };
        if validate_principal(principal).is_err() {
            return Err(CommandPlanError::InvalidPrincipal);
        }
        let (required_capability, domain_schema, capabilities) = match &self.operation {
            PlannedOperation::Query(envelope) => match &envelope.query {
                Query::RuntimeList { .. }
                | Query::RuntimeInspect { .. }
                | Query::RuntimeHistory { .. }
                | Query::RuntimeLogs { .. } => (
                    CAPABILITY_RUNTIME_READ,
                    envelope.schema_version,
                    &envelope.capabilities,
                ),
            },
            PlannedOperation::Command(envelope) => match &envelope.command {
                Command::RuntimeRegister {
                    name,
                    endpoint,
                    sidecar_endpoint,
                    tags,
                    ..
                } => {
                    validate_registration_intent(name, endpoint, sidecar_endpoint.as_deref(), tags)
                        .map_err(|_| CommandPlanError::InvalidRegistrationIntent)?;
                    if envelope.expected_revision.is_some() {
                        return Err(CommandPlanError::InvalidRegistrationIntent);
                    }
                    if !envelope.dry_run && envelope.confirmation != Confirmation::Confirmed {
                        return Err(CommandPlanError::RegistrationConfirmationRequired);
                    }
                    (
                        CAPABILITY_RUNTIME_REGISTER,
                        envelope.schema_version,
                        &envelope.capabilities,
                    )
                }
                Command::RuntimeRegistrationUpdate {
                    name,
                    endpoint,
                    sidecar_endpoint,
                    tags,
                    ..
                } => {
                    validate_registration_intent(name, endpoint, sidecar_endpoint.as_deref(), tags)
                        .map_err(|_| CommandPlanError::InvalidRegistrationIntent)?;
                    if envelope.expected_revision.is_none() {
                        return Err(CommandPlanError::InvalidRegistrationIntent);
                    }
                    if !envelope.dry_run && envelope.confirmation != Confirmation::Confirmed {
                        return Err(CommandPlanError::RegistrationConfirmationRequired);
                    }
                    (
                        CAPABILITY_RUNTIME_REGISTER,
                        envelope.schema_version,
                        &envelope.capabilities,
                    )
                }
                Command::RuntimeDiscoveryIntake {
                    capabilities,
                    status,
                    sidecar_status,
                    ..
                } => {
                    validate_discovery_intake(
                        capabilities.as_deref(),
                        status.as_deref(),
                        sidecar_status.as_deref(),
                    )
                    .map_err(|_| CommandPlanError::InvalidRegistrationIntent)?;
                    if envelope.expected_revision.is_none() {
                        return Err(CommandPlanError::InvalidRegistrationIntent);
                    }
                    if !envelope.dry_run && envelope.confirmation != Confirmation::Confirmed {
                        return Err(CommandPlanError::RegistrationConfirmationRequired);
                    }
                    (
                        CAPABILITY_RUNTIME_REGISTER,
                        envelope.schema_version,
                        &envelope.capabilities,
                    )
                }
                Command::RuntimeRefresh { .. } | Command::RuntimeCapabilitiesRefresh { .. } => (
                    CAPABILITY_RUNTIME_REFRESH,
                    envelope.schema_version,
                    &envelope.capabilities,
                ),
                Command::RuntimeDeploy {
                    pipeline_kind,
                    target,
                    ..
                } => {
                    validate_deployment_intent(pipeline_kind, target.as_deref())
                        .map_err(|_| CommandPlanError::InvalidDeploymentIntent)?;
                    if !envelope.dry_run && envelope.confirmation != Confirmation::Confirmed {
                        return Err(CommandPlanError::DeploymentConfirmationRequired);
                    }
                    (
                        CAPABILITY_RUNTIME_DEPLOY,
                        envelope.schema_version,
                        &envelope.capabilities,
                    )
                }
                Command::DebuggerCancel { session_id } => {
                    if validated_identifier("session_id", session_id.clone()).is_err() {
                        return Err(CommandPlanError::InvalidDebuggerSessionId);
                    }
                    (
                        CAPABILITY_DEBUGGER_CONTROL,
                        envelope.schema_version,
                        &envelope.capabilities,
                    )
                }
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
            Self::InvalidPrincipal => write!(formatter, "plan has an invalid principal"),
            Self::InvalidDebuggerSessionId => {
                write!(formatter, "debugger plan has an invalid session ID")
            }
            Self::InvalidRegistrationIntent => {
                write!(formatter, "registration plan has an invalid intent")
            }
            Self::RegistrationConfirmationRequired => {
                write!(
                    formatter,
                    "registration plan requires explicit confirmation"
                )
            }
            Self::InvalidDeploymentIntent => {
                write!(formatter, "deployment plan has an invalid intent")
            }
            Self::DeploymentConfirmationRequired => {
                write!(formatter, "deployment plan requires explicit confirmation")
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
    pub fn runtime_projection(&self, runtime_id: &RuntimeId) -> Option<&RuntimeProjection> {
        self.runtimes.get(runtime_id)
    }

    pub fn stamp_runtime_authority_time(
        &mut self,
        runtime_id: &RuntimeId,
        registered: bool,
        timestamp_unix_ms: u64,
    ) -> Result<RuntimeProjection, DomainError> {
        let runtime =
            self.runtimes
                .get_mut(runtime_id)
                .ok_or_else(|| DomainError::RuntimeNotFound {
                    runtime_id: runtime_id.as_str().to_string(),
                })?;
        if timestamp_unix_ms == 0 {
            return Ok(runtime.clone());
        }
        if registered && runtime.registered_at_unix_ms.is_none() {
            runtime.registered_at_unix_ms = Some(timestamp_unix_ms);
        }
        runtime.updated_at_unix_ms = Some(
            runtime
                .updated_at_unix_ms
                .unwrap_or_default()
                .max(timestamp_unix_ms),
        );
        Ok(runtime.clone())
    }

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
            sidecar_endpoint: None,
            registered_at_unix_ms: None,
            updated_at_unix_ms: None,
            revision: Revision(self.revision),
            refresh_count: 0,
            refresh_status: RefreshStatus::NeverRequested,
            tags,
            status,
            sidecar_status: None,
            capabilities: RuntimeCapabilitySnapshot::default(),
            capabilities_observed_for_revision: None,
        };
        self.runtimes.insert(id, projection.clone());
        projection
    }

    pub fn unregister_runtime(&mut self, runtime_id: &RuntimeId) -> bool {
        let removed = self.runtimes.remove(runtime_id).is_some();
        if removed {
            self.revision += 1;
        }
        removed
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
            Query::RuntimeInspect { runtime_id } => {
                let runtime = self.runtimes.get(&runtime_id).cloned().ok_or_else(|| {
                    DomainError::RuntimeNotFound {
                        runtime_id: runtime_id.as_str().to_string(),
                    }
                })?;
                Ok(QueryResult::RuntimeInspect {
                    revision: Revision(self.revision),
                    runtime,
                })
            }
            Query::RuntimeHistory { runtime_id } => {
                if !self.runtimes.contains_key(&runtime_id) {
                    return Err(DomainError::RuntimeNotFound {
                        runtime_id: runtime_id.as_str().to_string(),
                    });
                }
                let mut entries = self
                    .applied
                    .values()
                    .filter_map(|applied| match &applied.command {
                        Command::RuntimeRegister {
                            runtime_id: command_runtime_id,
                            ..
                        }
                        | Command::RuntimeRegistrationUpdate {
                            runtime_id: command_runtime_id,
                            ..
                        }
                        | Command::RuntimeRefresh {
                            runtime_id: command_runtime_id,
                        }
                        | Command::RuntimeCapabilitiesRefresh {
                            runtime_id: command_runtime_id,
                        }
                        | Command::RuntimeDeploy {
                            runtime_id: command_runtime_id,
                            ..
                        } if command_runtime_id == &runtime_id => Some(applied.result.clone()),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                entries.sort_by(|left, right| {
                    right
                        .runtime
                        .revision
                        .cmp(&left.runtime.revision)
                        .then_with(|| left.command_id.cmp(&right.command_id))
                });
                entries.truncate(MAX_RUNTIME_HISTORY_ENTRIES);
                Ok(QueryResult::RuntimeHistory {
                    revision: Revision(self.revision),
                    entries,
                })
            }
            Query::RuntimeLogs { .. } => Err(DomainError::InvalidQuery {
                reason: "runtime logs require the durable control runtime",
            }),
        }
    }

    pub fn execute(&mut self, envelope: CommandEnvelope) -> Result<CommandResult, DomainError> {
        validate_schema(envelope.schema_version)?;
        validate_principal(&envelope.principal)?;
        match &envelope.command {
            Command::RuntimeRegister {
                name,
                endpoint,
                sidecar_endpoint,
                tags,
                ..
            } => {
                require_capability(&envelope.capabilities, CAPABILITY_RUNTIME_REGISTER)?;
                validate_registration_intent(name, endpoint, sidecar_endpoint.as_deref(), tags)?;
                if envelope.expected_revision.is_some() {
                    return Err(DomainError::InvalidQuery {
                        reason: "runtime registration does not accept a runtime revision",
                    });
                }
                if !envelope.dry_run && envelope.confirmation != Confirmation::Confirmed {
                    return Err(DomainError::ConfirmationRequired);
                }
            }
            Command::RuntimeRegistrationUpdate {
                name,
                endpoint,
                sidecar_endpoint,
                tags,
                ..
            } => {
                require_capability(&envelope.capabilities, CAPABILITY_RUNTIME_REGISTER)?;
                validate_registration_intent(name, endpoint, sidecar_endpoint.as_deref(), tags)?;
                if envelope.expected_revision.is_none() {
                    return Err(DomainError::InvalidQuery {
                        reason: "runtime registration update requires a runtime revision",
                    });
                }
                if !envelope.dry_run && envelope.confirmation != Confirmation::Confirmed {
                    return Err(DomainError::ConfirmationRequired);
                }
            }
            Command::RuntimeDiscoveryIntake {
                capabilities,
                status,
                sidecar_status,
                ..
            } => {
                require_capability(&envelope.capabilities, CAPABILITY_RUNTIME_REGISTER)?;
                validate_discovery_intake(
                    capabilities.as_deref(),
                    status.as_deref(),
                    sidecar_status.as_deref(),
                )?;
                if envelope.expected_revision.is_none() {
                    return Err(DomainError::InvalidQuery {
                        reason: "runtime discovery intake requires a runtime revision",
                    });
                }
                if !envelope.dry_run && envelope.confirmation != Confirmation::Confirmed {
                    return Err(DomainError::ConfirmationRequired);
                }
            }
            Command::RuntimeRefresh { .. } | Command::RuntimeCapabilitiesRefresh { .. } => {
                require_capability(&envelope.capabilities, CAPABILITY_RUNTIME_REFRESH)?;
            }
            Command::RuntimeDeploy {
                pipeline_kind,
                target,
                ..
            } => {
                require_capability(&envelope.capabilities, CAPABILITY_RUNTIME_DEPLOY)?;
                validate_deployment_intent(pipeline_kind, target.as_deref())?;
                if !envelope.dry_run && envelope.confirmation != Confirmation::Confirmed {
                    return Err(DomainError::ConfirmationRequired);
                }
            }
            Command::DebuggerCancel { .. } => {
                require_capability(&envelope.capabilities, CAPABILITY_DEBUGGER_CONTROL)?;
                return Err(DomainError::InvalidQuery {
                    reason: "debugger commands require the Leselang VM authority",
                });
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
            Command::RuntimeRegister {
                runtime_id,
                name,
                endpoint,
                sidecar_endpoint,
                tags,
            } => {
                if self.runtimes.contains_key(&runtime_id) {
                    return Err(DomainError::RuntimeAlreadyExists {
                        runtime_id: runtime_id.as_str().to_string(),
                    });
                }
                if let Some(owner) = self.runtime_endpoint_owner(&endpoint, None) {
                    return Err(DomainError::RuntimeEndpointConflict {
                        runtime_id: owner.as_str().to_string(),
                    });
                }
                let next_revision = Revision(self.revision + 1);
                let runtime = RuntimeProjection {
                    id: runtime_id.clone(),
                    name,
                    endpoint,
                    sidecar_endpoint,
                    registered_at_unix_ms: None,
                    updated_at_unix_ms: None,
                    revision: next_revision,
                    refresh_count: 0,
                    refresh_status: RefreshStatus::NeverRequested,
                    tags,
                    status: RuntimeStatusSnapshot::default(),
                    sidecar_status: None,
                    capabilities: RuntimeCapabilitySnapshot::default(),
                    capabilities_observed_for_revision: None,
                };
                let result = CommandResult {
                    command_id: envelope.command_id.clone(),
                    status: if envelope.dry_run {
                        CommandStatus::Planned
                    } else {
                        CommandStatus::Applied
                    },
                    runtime: command_result_projection(runtime.clone()),
                    events: vec![DomainEvent::RuntimeRegistered {
                        runtime_id: runtime_id.clone(),
                        revision: next_revision,
                        command_id: envelope.command_id,
                    }],
                };

                if !envelope.dry_run {
                    self.revision += 1;
                    self.runtimes.insert(runtime_id, runtime);
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
            Command::RuntimeRegistrationUpdate {
                runtime_id,
                name,
                endpoint,
                sidecar_endpoint,
                tags,
            } => {
                let current = self.runtimes.get(&runtime_id).cloned().ok_or_else(|| {
                    DomainError::RuntimeNotFound {
                        runtime_id: runtime_id.as_str().to_string(),
                    }
                })?;
                let expected = envelope
                    .expected_revision
                    .expect("registration update revision was validated above");
                if expected != current.revision {
                    return Err(DomainError::RevisionConflict {
                        expected,
                        actual: current.revision,
                    });
                }
                if let Some(owner) = self.runtime_endpoint_owner(&endpoint, Some(&runtime_id)) {
                    return Err(DomainError::RuntimeEndpointConflict {
                        runtime_id: owner.as_str().to_string(),
                    });
                }

                let mut next = current;
                next.name = name;
                next.endpoint = endpoint;
                next.sidecar_endpoint = sidecar_endpoint;
                next.tags = tags;
                next.revision = Revision(self.revision + 1);
                let result = CommandResult {
                    command_id: envelope.command_id.clone(),
                    status: if envelope.dry_run {
                        CommandStatus::Planned
                    } else {
                        CommandStatus::Applied
                    },
                    runtime: command_result_projection(next.clone()),
                    events: vec![DomainEvent::RuntimeRegistrationUpdated {
                        runtime_id: runtime_id.clone(),
                        revision: next.revision,
                        command_id: envelope.command_id,
                    }],
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
            Command::RuntimeDiscoveryIntake {
                runtime_id,
                capabilities,
                status,
                sidecar_status,
            } => {
                let current = self.runtimes.get(&runtime_id).cloned().ok_or_else(|| {
                    DomainError::RuntimeNotFound {
                        runtime_id: runtime_id.as_str().to_string(),
                    }
                })?;
                let expected = envelope
                    .expected_revision
                    .expect("discovery intake revision was validated above");
                if expected != current.revision {
                    return Err(DomainError::RevisionConflict {
                        expected,
                        actual: current.revision,
                    });
                }

                let mut next = current;
                next.revision = Revision(self.revision + 1);
                if let Some(status) = status {
                    next.refresh_status = if status.status_fetch_error.is_some() {
                        RefreshStatus::Failed
                    } else {
                        RefreshStatus::Ready
                    };
                    next.status = *status;
                }
                if let Some(capabilities) = capabilities {
                    next.capabilities = *capabilities;
                    next.capabilities_observed_for_revision = Some(expected);
                }
                if let Some(sidecar_status) = sidecar_status {
                    next.sidecar_status = Some(*sidecar_status);
                }
                let result = CommandResult {
                    command_id: envelope.command_id.clone(),
                    status: if envelope.dry_run {
                        CommandStatus::Planned
                    } else {
                        CommandStatus::Applied
                    },
                    runtime: command_result_projection(next.clone()),
                    events: vec![DomainEvent::RuntimeDiscoveryIntakeApplied {
                        runtime_id: runtime_id.clone(),
                        revision: next.revision,
                        command_id: envelope.command_id,
                    }],
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
                    runtime: command_result_projection(next.clone()),
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
            Command::RuntimeCapabilitiesRefresh { runtime_id } => {
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
                let event = DomainEvent::RuntimeCapabilitiesRefreshRequested {
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
                    runtime: command_result_projection(next.clone()),
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
            Command::RuntimeDeploy {
                runtime_id,
                pipeline_kind,
                target,
            } => {
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
                let event = DomainEvent::RuntimeDeploymentRequested {
                    runtime_id: runtime_id.clone(),
                    revision: next.revision,
                    command_id: envelope.command_id.clone(),
                    request_id: envelope.idempotency_key.as_str().to_string(),
                    pipeline_kind,
                    requested_by: envelope.principal.id.clone(),
                    target,
                };
                let result = CommandResult {
                    command_id: envelope.command_id,
                    status: if envelope.dry_run {
                        CommandStatus::Planned
                    } else {
                        CommandStatus::Applied
                    },
                    runtime: command_result_projection(next.clone()),
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
            Command::DebuggerCancel { .. } => unreachable!("debugger commands returned above"),
        }
    }

    fn runtime_endpoint_owner(
        &self,
        endpoint: &str,
        excluded_runtime_id: Option<&RuntimeId>,
    ) -> Option<&RuntimeId> {
        let identity = canonical_runtime_endpoint_identity(endpoint);
        self.runtimes
            .iter()
            .find(|(runtime_id, runtime)| {
                excluded_runtime_id != Some(*runtime_id)
                    && canonical_runtime_endpoint_identity(&runtime.endpoint) == identity
            })
            .map(|(runtime_id, _)| runtime_id)
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

    pub fn complete_runtime_capability_refresh(
        &mut self,
        runtime_id: &RuntimeId,
        expected_revision: Revision,
        capabilities: RuntimeCapabilitySnapshot,
    ) -> Result<RuntimeProjection, DomainError> {
        validate_runtime_capabilities(&capabilities)?;
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
        next.capabilities = capabilities;
        next.capabilities_observed_for_revision = Some(expected_revision);
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
            if runtime.registered_at_unix_ms == Some(0)
                || runtime.updated_at_unix_ms == Some(0)
                || matches!(
                    (
                        runtime.registered_at_unix_ms,
                        runtime.updated_at_unix_ms
                    ),
                    (Some(registered), Some(updated)) if registered > updated
                )
            {
                return Err(DomainSnapshotError::Invalid {
                    reason: "runtime authority timestamps are invalid",
                });
            }
            if !runtime_ids.insert(runtime.id.clone()) {
                return Err(DomainSnapshotError::Invalid {
                    reason: "duplicate runtime identifier",
                });
            }
            if !runtime.capabilities.source.is_empty() {
                validate_runtime_capabilities(&runtime.capabilities).map_err(|_| {
                    DomainSnapshotError::Invalid {
                        reason: "invalid runtime capabilities",
                    }
                })?;
            }
            if let Some(sidecar_status) = &runtime.sidecar_status {
                validate_runtime_sidecar_status(sidecar_status).map_err(|_| {
                    DomainSnapshotError::Invalid {
                        reason: "invalid runtime sidecar status",
                    }
                })?;
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
            let runtime_id = match &applied.command {
                Command::RuntimeRegister { runtime_id, .. }
                | Command::RuntimeRegistrationUpdate { runtime_id, .. }
                | Command::RuntimeDiscoveryIntake { runtime_id, .. }
                | Command::RuntimeRefresh { runtime_id }
                | Command::RuntimeCapabilitiesRefresh { runtime_id }
                | Command::RuntimeDeploy { runtime_id, .. } => runtime_id,
                Command::DebuggerCancel { .. } => {
                    return Err(DomainSnapshotError::Invalid {
                        reason: "unsupported command in control-plane snapshot",
                    });
                }
            };
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
            Self::RuntimeAlreadyExists { runtime_id } => {
                write!(formatter, "runtime '{runtime_id}' already exists")
            }
            Self::RuntimeEndpointConflict { runtime_id } => {
                write!(
                    formatter,
                    "runtime endpoint is already owned by '{runtime_id}'"
                )
            }
            Self::RevisionConflict { expected, actual } => write!(
                formatter,
                "revision conflict: expected {}, actual {}",
                expected.0, actual.0
            ),
            Self::IdempotencyConflict { key } => {
                write!(formatter, "idempotency key '{key}' was reused")
            }
            Self::ConfirmationRequired => write!(formatter, "explicit confirmation is required"),
            Self::InvalidQuery { reason } => write!(formatter, "invalid query: {reason}"),
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

/// Validates the deployment fields shared by language, CLI, plan, and runtime boundaries.
pub fn validate_deployment_intent(
    pipeline_kind: &str,
    target: Option<&str>,
) -> Result<(), DomainError> {
    let pipeline_valid = !pipeline_kind.is_empty()
        && pipeline_kind.len() <= 128
        && pipeline_kind == pipeline_kind.trim()
        && pipeline_kind
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'/' | b'_' | b'-'));
    let target_valid = target.is_none_or(|target| {
        !target.is_empty()
            && target.len() <= 256
            && target == target.trim()
            && !target.chars().any(char::is_control)
    });
    if !pipeline_valid {
        return Err(DomainError::InvalidIdentifier {
            field: "pipeline_kind",
        });
    }
    if !target_valid {
        return Err(DomainError::InvalidIdentifier { field: "target" });
    }
    Ok(())
}

/// Validates the canonical, secret-free registration fields shared by all frontends.
pub fn validate_registration_intent(
    name: &str,
    endpoint: &str,
    sidecar_endpoint: Option<&str>,
    tags: &RuntimeTags,
) -> Result<(), DomainError> {
    let name_valid = !name.is_empty()
        && name.len() <= 128
        && name == name.trim()
        && !name.chars().any(char::is_control);
    let endpoint_valid = !endpoint.is_empty()
        && endpoint.len() <= 2048
        && endpoint == endpoint.trim()
        && !endpoint.chars().any(char::is_control);
    let sidecar_endpoint_valid = sidecar_endpoint.is_none_or(|value| {
        !value.is_empty()
            && value.len() <= 2048
            && value == value.trim()
            && !value.chars().any(char::is_control)
    });
    let tags_valid = [
        tags.environment.as_deref(),
        tags.cluster.as_deref(),
        tags.role.as_deref(),
    ]
    .into_iter()
    .flatten()
    .all(|value| {
        !value.is_empty()
            && value.len() <= 128
            && value == value.trim()
            && !value.chars().any(char::is_control)
    });
    if !name_valid {
        return Err(DomainError::InvalidIdentifier { field: "name" });
    }
    if !endpoint_valid {
        return Err(DomainError::InvalidIdentifier { field: "endpoint" });
    }
    if !sidecar_endpoint_valid {
        return Err(DomainError::InvalidIdentifier {
            field: "sidecar_endpoint",
        });
    }
    if !tags_valid {
        return Err(DomainError::InvalidIdentifier { field: "tags" });
    }
    Ok(())
}

/// Returns the stable endpoint identity used to prevent one target from being
/// registered under multiple runtime identities. Non-URI development endpoints
/// retain their exact validated representation for wire-v1 compatibility.
pub fn canonical_runtime_endpoint_identity(endpoint: &str) -> String {
    let endpoint = endpoint.trim();
    let Ok(uri) = endpoint.parse::<Uri>() else {
        return endpoint.to_string();
    };
    let (Some(scheme), Some(authority)) = (uri.scheme_str(), uri.authority()) else {
        return endpoint.to_string();
    };
    let scheme = scheme.to_ascii_lowercase();
    let mut host = authority.host().to_ascii_lowercase();
    if host.contains(':') && !host.starts_with('[') {
        host = format!("[{host}]");
    }
    let port = authority.port_u16();
    let include_port = !matches!(
        (scheme.as_str(), port),
        ("http", Some(80)) | ("https", Some(443))
    );
    let authority_identity = match (include_port, port) {
        (true, Some(port)) => format!("{host}:{port}"),
        _ => host,
    };
    let path_and_query = uri
        .path_and_query()
        .map(|value| value.as_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("/");
    format!("{scheme}://{authority_identity}{path_and_query}")
}

fn validate_runtime_capabilities(
    capabilities: &RuntimeCapabilitySnapshot,
) -> Result<(), DomainError> {
    let endpoints_valid = !capabilities.endpoints.is_empty()
        && capabilities.endpoints.len() <= 128
        && capabilities
            .endpoints
            .windows(2)
            .all(|pair| pair[0] < pair[1])
        && capabilities
            .endpoints
            .iter()
            .all(|endpoint| valid_capability_endpoint(endpoint));
    let extensions_valid = capabilities.extensions.len() <= 64
        && capabilities.extensions.keys().all(|key| {
            !key.is_empty()
                && key.len() <= 64
                && key.bytes().all(|byte| {
                    byte.is_ascii_lowercase()
                        || byte.is_ascii_digit()
                        || matches!(byte, b'_' | b'-')
                })
        });
    let version_valid = !capabilities.version.is_empty()
        && capabilities.version.len() <= 32
        && capabilities
            .version
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'+'));
    let direct_chars_valid = !capabilities.target_direct_path_chars.is_empty()
        && capabilities.target_direct_path_chars.len() <= 64
        && capabilities
            .target_direct_path_chars
            .bytes()
            .all(|byte| byte.is_ascii_graphic() || byte == b' ');
    if capabilities.source != "gewyvern-api"
        || capabilities.service != "gewyvern-api"
        || !version_valid
        || capabilities.target_path_segment_encoding != "percent-encoding"
        || !direct_chars_valid
        || !endpoints_valid
        || !extensions_valid
        || !capabilities
            .endpoints
            .iter()
            .any(|item| item == "/v1/capabilities")
        || capabilities.authenticated_deployment
            != capabilities
                .endpoints
                .iter()
                .any(|item| item == "/v1/deployments")
    {
        return Err(DomainError::InvalidQuery {
            reason: "runtime capability observation is invalid",
        });
    }
    Ok(())
}

fn validate_discovery_intake(
    capabilities: Option<&RuntimeCapabilitySnapshot>,
    status: Option<&RuntimeStatusSnapshot>,
    sidecar_status: Option<&RuntimeSidecarStatusSnapshot>,
) -> Result<(), DomainError> {
    if capabilities.is_none() && status.is_none() && sidecar_status.is_none() {
        return Err(DomainError::InvalidQuery {
            reason: "runtime discovery intake is empty",
        });
    }
    if let Some(capabilities) = capabilities {
        validate_runtime_capabilities(capabilities)?;
    }
    if let Some(status) = status {
        let bounded = |value: Option<&str>, maximum: usize| {
            value.is_none_or(|value| {
                !value.is_empty()
                    && value.len() <= maximum
                    && value == value.trim()
                    && !value.chars().any(char::is_control)
            })
        };
        if status.status_source != "gewyvern-api"
            || status.status_fetched_at.is_none()
            || status.status_fetch_error.is_some()
            || !bounded(status.status_fetched_at.as_deref(), 64)
            || !bounded(status.snapshot_kind.as_deref(), 128)
            || !bounded(status.resilience_status.as_deref(), 128)
            || !bounded(status.resilience_summary.as_deref(), 1024)
            || !bounded(status.socket_service_status.as_deref(), 128)
        {
            return Err(DomainError::InvalidQuery {
                reason: "runtime status observation is invalid",
            });
        }
    }
    if let Some(sidecar_status) = sidecar_status {
        validate_runtime_sidecar_status(sidecar_status)?;
    }
    Ok(())
}

fn validate_runtime_sidecar_status(
    status: &RuntimeSidecarStatusSnapshot,
) -> Result<(), DomainError> {
    let bounded = |value: Option<&str>, maximum: usize| {
        value.is_none_or(|value| {
            !value.is_empty()
                && value.len() <= maximum
                && value == value.trim()
                && !value.chars().any(char::is_control)
        })
    };
    let common = bounded(status.status_fetched_at.as_deref(), 64)
        && bounded(status.status_fetch_error.as_deref(), 128)
        && bounded(Some(status.daemon_status.as_str()), 128)
        && bounded(status.last_error.as_deref(), 128)
        && status.learned_routes <= 10_000_000
        && status.target_count.is_none_or(|count| count <= 10_000_000);
    let posture = (status.status_source == "etragon-api"
        && status.status_fetched_at.is_some()
        && status.status_fetch_error.is_none()
        && status
            .last_error
            .as_deref()
            .is_none_or(|error| error == "sidecar_reported_error"))
        || (status.status_source == "fetch_failed"
            && status.status_fetched_at.is_none()
            && status.status_fetch_error.as_deref() == Some("sidecar_fetch_failed")
            && status
                .last_error
                .as_deref()
                .is_none_or(|error| error == "sidecar_fetch_failed")
            && !status.healthy);
    let memory_valid = status.memory.as_ref().is_none_or(|memory| {
        memory.slot_count <= 10_000
            && memory.history_count <= 1_000_000
            && memory.slots.len() <= 128
            && bounded(memory.latest_slot.as_deref(), 128)
            && bounded(memory.latest_label.as_deref(), 256)
            && bounded(memory.latest_source.as_deref(), 128)
            && memory
                .fetch_error
                .as_deref()
                .is_none_or(|error| error == "sidecar_memory_fetch_failed")
            && memory.slots.iter().all(|slot| {
                bounded(Some(slot.slot.as_str()), 128)
                    && bounded(slot.label.as_deref(), 256)
                    && bounded(slot.note.as_deref(), 1024)
                    && bounded(Some(slot.source.as_str()), 128)
                    && bounded(slot.saved_at.as_deref(), 64)
                    && slot.pattern_count <= 10_000_000
                    && slot.label_count <= 10_000_000
            })
    });
    if common && posture && memory_valid {
        Ok(())
    } else {
        Err(DomainError::InvalidQuery {
            reason: "runtime sidecar observation is invalid",
        })
    }
}

fn valid_capability_endpoint(value: &str) -> bool {
    value.len() <= 256
        && value.starts_with('/')
        && !value.starts_with("//")
        && !value.contains(['?', '#'])
        && value.bytes().all(|byte| byte.is_ascii_graphic())
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

    fn registration_envelope(command_id: &str, key: &str) -> CommandEnvelope {
        CommandEnvelope {
            schema_version: DOMAIN_SCHEMA_VERSION,
            command_id: CommandId::new(command_id).unwrap(),
            idempotency_key: IdempotencyKey::new(key).unwrap(),
            expected_revision: None,
            principal: Principal {
                id: "operator".to_string(),
            },
            capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_REGISTER]),
            origin: CommandOrigin::Cli,
            confirmation: Confirmation::Confirmed,
            dry_run: false,
            command: Command::RuntimeRegister {
                runtime_id: RuntimeId::new("runtime-new").unwrap(),
                name: "New Runtime".to_string(),
                endpoint: "https://127.0.0.1:9443".to_string(),
                sidecar_endpoint: Some("https://127.0.0.1:9444".to_string()),
                tags: RuntimeTags {
                    environment: Some("production".to_string()),
                    cluster: Some("east".to_string()),
                    role: Some("edge".to_string()),
                },
            },
        }
    }

    fn registration_update_envelope(
        runtime_id: RuntimeId,
        expected_revision: Revision,
        command_id: &str,
        key: &str,
    ) -> CommandEnvelope {
        CommandEnvelope {
            schema_version: DOMAIN_SCHEMA_VERSION,
            command_id: CommandId::new(command_id).unwrap(),
            idempotency_key: IdempotencyKey::new(key).unwrap(),
            expected_revision: Some(expected_revision),
            principal: Principal {
                id: "operator".to_string(),
            },
            capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_REGISTER]),
            origin: CommandOrigin::Cli,
            confirmation: Confirmation::Confirmed,
            dry_run: false,
            command: Command::RuntimeRegistrationUpdate {
                runtime_id,
                name: "Updated Runtime".to_string(),
                endpoint: "https://127.0.0.1:9553".to_string(),
                sidecar_endpoint: Some("https://127.0.0.1:9554".to_string()),
                tags: RuntimeTags {
                    environment: Some("staging".to_string()),
                    cluster: Some("west".to_string()),
                    role: Some("control".to_string()),
                },
            },
        }
    }

    fn discovery_capabilities() -> RuntimeCapabilitySnapshot {
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

    fn discovery_status() -> RuntimeStatusSnapshot {
        RuntimeStatusSnapshot {
            status_source: "gewyvern-api".into(),
            status_fetched_at: Some("2026-07-20T12:00:00.0000000+08:00".into()),
            has_latest_snapshot: true,
            snapshot_kind: Some("capture".into()),
            target_count: Some(3),
            ..RuntimeStatusSnapshot::default()
        }
    }

    fn discovery_sidecar_status() -> RuntimeSidecarStatusSnapshot {
        RuntimeSidecarStatusSnapshot {
            status_source: "etragon-api".into(),
            status_fetched_at: Some("2026-07-20T12:01:00.0000000+08:00".into()),
            status_fetch_error: None,
            healthy: true,
            daemon_status: "ready".into(),
            target_count: Some(2),
            learning_active: false,
            learned_routes: 4,
            has_evidence_chain_enrichment: true,
            has_diagnostic_opinion: false,
            last_error: None,
            memory: Some(RuntimeSidecarMemorySnapshot {
                versions_supported: true,
                slot_count: 1,
                history_count: 2,
                latest_slot: Some("slot-a".into()),
                latest_label: Some("baseline".into()),
                latest_source: Some("manual".into()),
                slots: vec![RuntimeSidecarMemorySlotSnapshot {
                    slot: "slot-a".into(),
                    label: Some("baseline".into()),
                    note: None,
                    source: "manual".into(),
                    saved_at: Some("2026-07-20T11:00:00.0000000+08:00".into()),
                    pattern_count: 3,
                    label_count: 2,
                }],
                fetch_error: None,
            }),
        }
    }

    #[test]
    fn identifier_deserialization_enforces_the_constructor_contract() {
        assert_eq!(
            serde_json::from_str::<RuntimeId>(r#""runtime-a""#).unwrap(),
            RuntimeId::new("runtime-a").unwrap()
        );
        for invalid in [String::new(), "bad/id".into(), "x".repeat(129)] {
            let encoded = serde_json::to_string(&invalid).unwrap();
            assert!(serde_json::from_str::<RuntimeId>(&encoded).is_err());
            assert!(serde_json::from_str::<CommandId>(&encoded).is_err());
            assert!(serde_json::from_str::<IdempotencyKey>(&encoded).is_err());
        }
    }

    #[test]
    fn canonical_endpoint_identity_normalizes_host_case_and_default_ports() {
        assert_eq!(
            canonical_runtime_endpoint_identity("http://runtime.test"),
            canonical_runtime_endpoint_identity("HTTP://RUNTIME.TEST:80/")
        );
        assert_eq!(
            canonical_runtime_endpoint_identity("https://runtime.test/path?mode=full"),
            "https://runtime.test/path?mode=full"
        );
        assert_eq!(canonical_runtime_endpoint_identity("local"), "local");
        assert_ne!(
            canonical_runtime_endpoint_identity("https://runtime.test/path-a"),
            canonical_runtime_endpoint_identity("https://runtime.test/path-b")
        );
    }

    #[test]
    fn command_plan_rejects_an_invalid_principal_before_dispatch() {
        let mut envelope = refresh_envelope(
            RuntimeId::new("runtime-a").unwrap(),
            "command-a",
            "effect-a",
        );
        envelope.principal.id = "bad principal".into();
        let plan = CommandPlan {
            schema_version: COMMAND_PLAN_SCHEMA_VERSION,
            required_capability: CAPABILITY_RUNTIME_REFRESH.into(),
            operation: PlannedOperation::Command(envelope),
        };
        assert_eq!(plan.validate(), Err(CommandPlanError::InvalidPrincipal));
    }

    #[test]
    fn runtime_registration_is_confirmed_capability_gated_and_idempotent() {
        let mut control = InMemoryControlPlane::default();
        let envelope = registration_envelope("register-command", "register-key");

        let mut unauthorized = envelope.clone();
        unauthorized.capabilities = CapabilitySet::default();
        assert_eq!(
            control.execute(unauthorized),
            Err(DomainError::Unauthorized {
                capability: CAPABILITY_RUNTIME_REGISTER
            })
        );

        let mut unconfirmed = envelope.clone();
        unconfirmed.confirmation = Confirmation::NotRequired;
        assert_eq!(
            control.execute(unconfirmed),
            Err(DomainError::ConfirmationRequired)
        );

        let first = control.execute(envelope.clone()).unwrap();
        assert_eq!(first.status, CommandStatus::Applied);
        assert_eq!(first.runtime.revision, Revision(1));
        assert_eq!(
            first.runtime.sidecar_endpoint.as_deref(),
            Some("https://127.0.0.1:9444")
        );
        assert_eq!(control.execute(envelope.clone()).unwrap(), first);
        let stamped = control
            .stamp_runtime_authority_time(&first.runtime.id, true, 1_721_234_567_890)
            .unwrap();
        assert_eq!(stamped.registered_at_unix_ms, Some(1_721_234_567_890));
        assert_eq!(stamped.updated_at_unix_ms, Some(1_721_234_567_890));
        let restamped = control
            .stamp_runtime_authority_time(&first.runtime.id, false, 1_721_234_567_000)
            .unwrap();
        assert_eq!(
            restamped.registered_at_unix_ms,
            stamped.registered_at_unix_ms
        );
        assert_eq!(restamped.updated_at_unix_ms, stamped.updated_at_unix_ms);

        let snapshot = control.snapshot();
        snapshot.validate().unwrap();
        let mut restored = InMemoryControlPlane::from_snapshot(snapshot).unwrap();
        assert_eq!(
            restored
                .runtime_projection(&first.runtime.id)
                .unwrap()
                .updated_at_unix_ms,
            Some(1_721_234_567_890)
        );
        assert_eq!(restored.execute(envelope).unwrap(), first);
    }

    #[test]
    fn runtime_registration_rejects_invalid_duplicate_and_conflicting_intents() {
        let mut control = InMemoryControlPlane::default();
        let envelope = registration_envelope("register-command", "register-key");

        let mut invalid = envelope.clone();
        let Command::RuntimeRegister { name, .. } = &mut invalid.command else {
            unreachable!();
        };
        *name = " padded ".to_string();
        assert_eq!(
            control.execute(invalid),
            Err(DomainError::InvalidIdentifier { field: "name" })
        );

        let mut invalid_sidecar = envelope.clone();
        let Command::RuntimeRegister {
            sidecar_endpoint, ..
        } = &mut invalid_sidecar.command
        else {
            unreachable!();
        };
        *sidecar_endpoint = Some(" padded ".to_string());
        assert_eq!(
            control.execute(invalid_sidecar),
            Err(DomainError::InvalidIdentifier {
                field: "sidecar_endpoint"
            })
        );

        control.execute(envelope.clone()).unwrap();
        let mut duplicate = registration_envelope("register-duplicate", "duplicate-key");
        assert!(matches!(
            control.execute(duplicate.clone()),
            Err(DomainError::RuntimeAlreadyExists { .. })
        ));

        let Command::RuntimeRegister { runtime_id, .. } = &mut duplicate.command else {
            unreachable!();
        };
        *runtime_id = RuntimeId::new("runtime-other").unwrap();
        duplicate.idempotency_key = IdempotencyKey::new("register-key").unwrap();
        assert!(matches!(
            control.execute(duplicate),
            Err(DomainError::IdempotencyConflict { .. })
        ));
    }

    #[test]
    fn runtime_registration_create_and_update_reject_canonical_endpoint_conflicts() {
        let mut control = InMemoryControlPlane::default();
        let runtime_a = RuntimeId::new("runtime-a").unwrap();
        let runtime_b = RuntimeId::new("runtime-b").unwrap();
        control.register_runtime(runtime_a.clone(), "Runtime A", "http://runtime-a.test");
        control.register_runtime(runtime_b.clone(), "Runtime B", "http://runtime-b.test");

        let mut create = registration_envelope("register-command", "register-key");
        let Command::RuntimeRegister {
            runtime_id,
            endpoint,
            ..
        } = &mut create.command
        else {
            unreachable!();
        };
        *runtime_id = RuntimeId::new("runtime-c").unwrap();
        *endpoint = "HTTP://RUNTIME-A.TEST:80/".into();
        assert_eq!(
            control.execute(create),
            Err(DomainError::RuntimeEndpointConflict {
                runtime_id: "runtime-a".into()
            })
        );

        let mut update =
            registration_update_envelope(runtime_a, Revision(1), "update-command", "update-key");
        let Command::RuntimeRegistrationUpdate { endpoint, .. } = &mut update.command else {
            unreachable!();
        };
        *endpoint = "HTTP://RUNTIME-B.TEST:80/".into();
        let error = control.execute(update).unwrap_err();
        assert_eq!(
            error,
            DomainError::RuntimeEndpointConflict {
                runtime_id: runtime_b.as_str().into()
            }
        );
        assert!(!error.to_string().contains("runtime-b.test"));
    }

    #[test]
    fn runtime_registration_dry_run_does_not_create_or_consume_the_key() {
        let mut control = InMemoryControlPlane::default();
        let mut envelope = registration_envelope("register-command", "register-key");
        envelope.dry_run = true;
        envelope.confirmation = Confirmation::NotRequired;

        let preview = control.execute(envelope.clone()).unwrap();
        assert_eq!(preview.status, CommandStatus::Planned);
        assert!(control.runtime_projection(&preview.runtime.id).is_none());

        envelope.dry_run = false;
        envelope.confirmation = Confirmation::Confirmed;
        let applied = control.execute(envelope).unwrap();
        assert_eq!(applied.status, CommandStatus::Applied);
        assert_eq!(applied.runtime.revision, Revision(1));
    }

    #[test]
    fn runtime_registration_update_is_confirmed_revision_fenced_and_idempotent() {
        let mut control = InMemoryControlPlane::default();
        let registered = control
            .execute(registration_envelope("register-command", "register-key"))
            .unwrap();
        let mut update = registration_update_envelope(
            registered.runtime.id.clone(),
            registered.runtime.revision,
            "update-command",
            "update-key",
        );

        let mut missing_revision = update.clone();
        missing_revision.expected_revision = None;
        assert_eq!(
            control.execute(missing_revision),
            Err(DomainError::InvalidQuery {
                reason: "runtime registration update requires a runtime revision"
            })
        );

        let mut stale = update.clone();
        stale.expected_revision = Some(Revision(99));
        assert_eq!(
            control.execute(stale),
            Err(DomainError::RevisionConflict {
                expected: Revision(99),
                actual: Revision(1)
            })
        );

        update.confirmation = Confirmation::NotRequired;
        assert_eq!(
            control.execute(update.clone()),
            Err(DomainError::ConfirmationRequired)
        );
        update.confirmation = Confirmation::Confirmed;

        let applied = control.execute(update.clone()).unwrap();
        assert_eq!(applied.runtime.revision, Revision(2));
        assert_eq!(applied.runtime.name, "Updated Runtime");
        assert_eq!(applied.runtime.endpoint, "https://127.0.0.1:9553");
        assert_eq!(
            applied.runtime.sidecar_endpoint.as_deref(),
            Some("https://127.0.0.1:9554")
        );
        assert!(matches!(
            applied.events.as_slice(),
            [DomainEvent::RuntimeRegistrationUpdated {
                revision: Revision(2),
                ..
            }]
        ));
        assert_eq!(control.execute(update.clone()).unwrap(), applied);

        let snapshot = control.snapshot();
        snapshot.validate().unwrap();
        let mut restored = InMemoryControlPlane::from_snapshot(snapshot).unwrap();
        assert_eq!(restored.execute(update).unwrap(), applied);
    }

    #[test]
    fn runtime_registration_update_preview_preserves_runtime_state_without_committing() {
        let mut control = InMemoryControlPlane::default();
        let registered = control
            .execute(registration_envelope("register-command", "register-key"))
            .unwrap();
        let refreshed = control
            .execute(refresh_envelope(
                registered.runtime.id.clone(),
                "refresh-command",
                "refresh-key",
            ))
            .unwrap();
        let mut update = registration_update_envelope(
            refreshed.runtime.id.clone(),
            refreshed.runtime.revision,
            "update-command",
            "update-key",
        );
        update.dry_run = true;
        update.confirmation = Confirmation::NotRequired;

        let preview = control.execute(update.clone()).unwrap();
        assert_eq!(preview.status, CommandStatus::Planned);
        assert_eq!(preview.runtime.revision, Revision(3));
        assert_eq!(preview.runtime.refresh_count, 1);
        assert_eq!(preview.runtime.refresh_status, RefreshStatus::Pending);
        assert_eq!(
            control.runtime_projection(&refreshed.runtime.id),
            Some(&refreshed.runtime)
        );

        update.dry_run = false;
        update.confirmation = Confirmation::Confirmed;
        let applied = control.execute(update).unwrap();
        assert_eq!(applied.status, CommandStatus::Applied);
        assert_eq!(applied.runtime.refresh_count, 1);
        assert_eq!(applied.runtime.refresh_status, RefreshStatus::Pending);
    }

    #[test]
    fn runtime_discovery_intake_is_typed_revision_fenced_and_idempotent() {
        let mut control = InMemoryControlPlane::default();
        let registered = control
            .execute(registration_envelope("register-command", "register-key"))
            .unwrap();
        let capabilities = discovery_capabilities();
        let status = discovery_status();
        let sidecar_status = discovery_sidecar_status();
        let intake = CommandEnvelope {
            schema_version: DOMAIN_SCHEMA_VERSION,
            command_id: CommandId::new("discovery-command").unwrap(),
            idempotency_key: IdempotencyKey::new("discovery-key").unwrap(),
            expected_revision: Some(registered.runtime.revision),
            principal: Principal {
                id: "web-bridge".into(),
            },
            capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_REGISTER]),
            origin: CommandOrigin::CompatibilityAdapter,
            confirmation: Confirmation::Confirmed,
            dry_run: false,
            command: Command::RuntimeDiscoveryIntake {
                runtime_id: registered.runtime.id.clone(),
                capabilities: Some(Box::new(capabilities.clone())),
                status: Some(Box::new(status.clone())),
                sidecar_status: Some(Box::new(sidecar_status.clone())),
            },
        };
        let applied = control.execute(intake.clone()).unwrap();
        assert_eq!(control.execute(intake).unwrap(), applied);
        assert_eq!(applied.runtime.revision, Revision(2));
        assert_eq!(applied.runtime.capabilities, capabilities);
        assert_eq!(applied.runtime.status, status);
        assert_eq!(applied.runtime.sidecar_status, Some(sidecar_status));
        assert_eq!(
            applied.runtime.capabilities_observed_for_revision,
            Some(Revision(1))
        );
        assert!(matches!(
            applied.events.as_slice(),
            [DomainEvent::RuntimeDiscoveryIntakeApplied {
                revision: Revision(2),
                ..
            }]
        ));

        let mut invalid_sidecar = CommandEnvelope {
            schema_version: DOMAIN_SCHEMA_VERSION,
            command_id: CommandId::new("invalid-sidecar-command").unwrap(),
            idempotency_key: IdempotencyKey::new("invalid-sidecar-key").unwrap(),
            expected_revision: Some(Revision(2)),
            principal: Principal {
                id: "web-bridge".into(),
            },
            capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_REGISTER]),
            origin: CommandOrigin::CompatibilityAdapter,
            confirmation: Confirmation::Confirmed,
            dry_run: false,
            command: Command::RuntimeDiscoveryIntake {
                runtime_id: registered.runtime.id.clone(),
                capabilities: None,
                status: None,
                sidecar_status: Some(Box::new(RuntimeSidecarStatusSnapshot {
                    status_source: "fetch_failed".into(),
                    status_fetched_at: None,
                    status_fetch_error: Some("raw upstream secret".into()),
                    healthy: false,
                    daemon_status: "fetch_failed".into(),
                    target_count: None,
                    learning_active: false,
                    learned_routes: 0,
                    has_evidence_chain_enrichment: false,
                    has_diagnostic_opinion: false,
                    last_error: None,
                    memory: None,
                })),
            },
        };
        assert!(matches!(
            control.execute(invalid_sidecar.clone()),
            Err(DomainError::InvalidQuery {
                reason: "runtime sidecar observation is invalid"
            })
        ));
        if let Command::RuntimeDiscoveryIntake { sidecar_status, .. } = &mut invalid_sidecar.command
        {
            sidecar_status.as_mut().unwrap().status_fetch_error =
                Some("sidecar_fetch_failed".into());
        }
        assert!(control.execute(invalid_sidecar).is_ok());

        let mut stale = CommandEnvelope {
            schema_version: DOMAIN_SCHEMA_VERSION,
            command_id: CommandId::new("stale-discovery-command").unwrap(),
            idempotency_key: IdempotencyKey::new("stale-discovery-key").unwrap(),
            expected_revision: Some(Revision(2)),
            principal: Principal {
                id: "operator".into(),
            },
            capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_REGISTER]),
            origin: CommandOrigin::Cli,
            confirmation: Confirmation::Confirmed,
            dry_run: false,
            command: Command::RuntimeDiscoveryIntake {
                runtime_id: registered.runtime.id,
                capabilities: Some(Box::new(discovery_capabilities())),
                status: None,
                sidecar_status: None,
            },
        };
        assert!(matches!(
            control.execute(stale.clone()),
            Err(DomainError::RevisionConflict { .. })
        ));
        stale.expected_revision = Some(Revision(3));
        if let Command::RuntimeDiscoveryIntake {
            capabilities,
            status,
            ..
        } = &mut stale.command
        {
            *capabilities = None;
            *status = None;
        }
        assert!(matches!(
            control.execute(stale),
            Err(DomainError::InvalidQuery {
                reason: "runtime discovery intake is empty"
            })
        ));
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
        let QueryResult::RuntimeList { runtimes, .. } = result else {
            panic!("runtime list must return a list result");
        };
        assert_eq!(runtimes[0].id.as_str(), "runtime-a");
        assert_eq!(runtimes[1].id.as_str(), "runtime-b");
    }

    #[test]
    fn runtime_inspect_returns_one_projection_and_fails_when_missing() {
        let mut control = InMemoryControlPlane::default();
        let runtime_id = RuntimeId::new("runtime-a").unwrap();
        control.register_runtime(runtime_id.clone(), "A", "http://a");
        let envelope = |runtime_id| QueryEnvelope {
            schema_version: DOMAIN_SCHEMA_VERSION,
            principal: Principal {
                id: "operator".to_string(),
            },
            capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
            query: Query::RuntimeInspect { runtime_id },
        };
        let result = control.query(envelope(runtime_id.clone())).unwrap();
        assert!(matches!(
            result,
            QueryResult::RuntimeInspect { revision: Revision(1), runtime }
                if runtime.id == runtime_id
        ));
        assert_eq!(
            control
                .query(envelope(RuntimeId::new("missing").unwrap()))
                .unwrap_err(),
            DomainError::RuntimeNotFound {
                runtime_id: "missing".into()
            }
        );
    }

    #[test]
    fn runtime_history_is_newest_first_and_bounded() {
        let runtime_id = RuntimeId::new("runtime-a").unwrap();
        let mut control = InMemoryControlPlane::default();
        control.register_runtime(runtime_id.clone(), "A", "http://a");
        for index in 0..40 {
            let mut envelope = refresh_envelope(
                runtime_id.clone(),
                &format!("command-{index}"),
                &format!("key-{index}"),
            );
            envelope.expected_revision = Some(Revision(index + 1));
            control.execute(envelope).unwrap();
        }
        let result = control
            .query(QueryEnvelope {
                schema_version: DOMAIN_SCHEMA_VERSION,
                principal: Principal {
                    id: "operator".into(),
                },
                capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
                query: Query::RuntimeHistory {
                    runtime_id: runtime_id.clone(),
                },
            })
            .unwrap();
        let QueryResult::RuntimeHistory { revision, entries } = result else {
            panic!("runtime history must return a history result");
        };
        assert_eq!(revision, Revision(41));
        assert_eq!(entries.len(), MAX_RUNTIME_HISTORY_ENTRIES);
        assert_eq!(entries.first().unwrap().runtime.revision, Revision(41));
        assert_eq!(entries.last().unwrap().runtime.revision, Revision(10));
        assert!(entries.iter().all(|entry| entry.runtime.id == runtime_id));
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
    fn snapshot_rejects_zero_or_reversed_authority_timestamps() {
        let runtime_id = RuntimeId::new("runtime-a").unwrap();
        let mut control = InMemoryControlPlane::default();
        control.register_runtime(runtime_id, "A", "http://a");

        let mut zero = control.snapshot();
        zero.runtimes[0].registered_at_unix_ms = Some(0);
        assert!(matches!(
            zero.validate(),
            Err(DomainSnapshotError::Invalid {
                reason: "runtime authority timestamps are invalid"
            })
        ));

        let mut reversed = control.snapshot();
        reversed.runtimes[0].registered_at_unix_ms = Some(20);
        reversed.runtimes[0].updated_at_unix_ms = Some(10);
        assert!(matches!(
            reversed.validate(),
            Err(DomainSnapshotError::Invalid {
                reason: "runtime authority timestamps are invalid"
            })
        ));
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
        let QueryResult::RuntimeList { runtimes, .. } = result else {
            panic!("runtime list must return a list result");
        };
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

    #[test]
    fn capability_refresh_is_revision_checked_and_domain_validated() {
        let mut control = InMemoryControlPlane::default();
        let runtime_id = RuntimeId::new("runtime-a").unwrap();
        control.register_runtime(runtime_id.clone(), "A", "http://a");
        let capabilities = RuntimeCapabilitySnapshot {
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
        };
        assert!(matches!(
            control.complete_runtime_capability_refresh(
                &runtime_id,
                Revision(2),
                capabilities.clone()
            ),
            Err(DomainError::RevisionConflict { .. })
        ));
        let completed = control
            .complete_runtime_capability_refresh(&runtime_id, Revision(1), capabilities.clone())
            .unwrap();
        assert_eq!(completed.revision, Revision(2));
        assert_eq!(completed.capabilities, capabilities);
        assert_eq!(
            completed.capabilities_observed_for_revision,
            Some(Revision(1))
        );

        let mut invalid = completed.capabilities;
        invalid.endpoints.reverse();
        assert!(matches!(
            control.complete_runtime_capability_refresh(&runtime_id, Revision(2), invalid),
            Err(DomainError::InvalidQuery { .. })
        ));
        assert_eq!(
            control.runtime_projection(&runtime_id).unwrap().revision,
            Revision(2)
        );
    }

    #[test]
    fn deployment_requires_capability_confirmation_and_a_bounded_intent() {
        let mut control = InMemoryControlPlane::default();
        let runtime_id = RuntimeId::new("runtime-a").unwrap();
        control.register_runtime(runtime_id.clone(), "A", "http://a");
        let mut envelope = CommandEnvelope {
            schema_version: DOMAIN_SCHEMA_VERSION,
            command_id: CommandId::new("deploy-command").unwrap(),
            idempotency_key: IdempotencyKey::new("deploy-request").unwrap(),
            expected_revision: Some(Revision(1)),
            principal: Principal {
                id: "operator-a".into(),
            },
            capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_DEPLOY]),
            origin: CommandOrigin::Cli,
            confirmation: Confirmation::NotRequired,
            dry_run: false,
            command: Command::RuntimeDeploy {
                runtime_id: runtime_id.clone(),
                pipeline_kind: "http/request".into(),
                target: Some("pid:42".into()),
            },
        };
        assert_eq!(
            control.execute(envelope.clone()),
            Err(DomainError::ConfirmationRequired)
        );
        envelope.confirmation = Confirmation::Confirmed;
        let result = control.execute(envelope).unwrap();
        assert_eq!(result.status, CommandStatus::Applied);
        assert!(matches!(
            result.events.as_slice(),
            [DomainEvent::RuntimeDeploymentRequested {
                runtime_id: event_runtime_id,
                revision: Revision(2),
                request_id,
                pipeline_kind,
                requested_by,
                target: Some(target),
                ..
            }] if event_runtime_id == &runtime_id
                && request_id == "deploy-request"
                && pipeline_kind == "http/request"
                && requested_by == "operator-a"
                && target == "pid:42"
        ));
    }
}
