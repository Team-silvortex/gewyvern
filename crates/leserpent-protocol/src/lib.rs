use leselang_ui::{DebuggerProjection, UiDocument};
use leserpent_domain::bootstrap::{BootstrapId, DeploymentBootstrapSnapshot};
use leserpent_domain::{
    CapabilitySet, CommandEnvelope, CommandId, CommandResult, DOMAIN_SCHEMA_VERSION, DomainError,
    Principal, QueryEnvelope, QueryResult, RefreshStatus, Revision, RuntimeCapabilitySnapshot,
    RuntimeDeploymentOutcome, RuntimeId, RuntimeProjection, RuntimeStatusSnapshot, RuntimeTags,
};
use serde::{Deserialize, Serialize};

pub mod bootstrap;
pub mod bootstrap_installer;
pub mod bootstrap_retirement;
pub mod bootstrap_retirement_control;
pub mod compatibility_v1;
pub mod gewyvern_installer;
pub mod gewyvern_retirement;
pub mod provisioning;
pub mod retirement;
pub mod transport_safety;

pub const PROTOCOL_SCHEMA_VERSION: u32 = 1;
pub const EVENT_SCHEMA_VERSION: u32 = 1;
pub const MAX_PROTOCOL_MESSAGE_BYTES: usize = 1024 * 1024;
pub const CAPABILITY_AUTHORITY_WRITER: &str = "authority.writer";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
// Preserve the v1 Rust and wire request shape until the v2 schema seal.
#[allow(clippy::large_enum_variant)]
pub enum ProtocolRequest {
    Command(CommandEnvelope),
    Query(QueryEnvelope),
    Health(HealthRequest),
    DeploymentReceipt(DeploymentReceiptRequest),
    OrchestraPersist(OrchestraPersistenceRequest),
    OrchestraPlanCatalog(OrchestraPlanCatalogRequest),
    OrchestraRunCommand(OrchestraRunCommandRequest),
    OrchestraCancelCommand(OrchestraCancelCommandRequest),
    OrchestraRetryCommand(OrchestraRetryCommandRequest),
    OrchestraHistory(OrchestraHistoryRequest),
    OrchestraDelete(OrchestraDeleteRequest),
    OrchestraDeleteCommand(OrchestraDeleteCommandRequest),
    OrchestraDeleteReplayHorizon(OrchestraDeleteReplayHorizonRequest),
    OrchestraDeleteReplayCheckpoint(OrchestraDeleteReplayCheckpointRequest),
    RuntimeUnregister(RuntimeUnregisterRequest),
    RuntimeUnregistrationReceipt(RuntimeUnregistrationReceiptRequest),
    AuthorityWriterClaim(AuthorityWriterClaimRequest),
    BootstrapHandoff(BootstrapHandoffRequest),
    BootstrapSessionBind(BootstrapSessionBindRequest),
    DebuggerSessions(DebuggerSessionsRequest),
    DebuggerSessionStart(DebuggerSessionStartRequest),
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HealthRequest {}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DeploymentReceiptRequest {
    pub principal: Principal,
    pub capabilities: CapabilitySet,
    pub command_id: CommandId,
    pub request_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OrchestraPersistenceRequest {
    pub principal: Principal,
    pub capabilities: CapabilitySet,
    pub envelope: compatibility_v1::LegacyOrchestraPersistenceEnvelope,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OrchestraPlanCatalogRequest {
    pub principal: Principal,
    pub capabilities: CapabilitySet,
    pub runtime_id: RuntimeId,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OrchestraRunCommandRequest {
    pub principal: Principal,
    pub capabilities: CapabilitySet,
    pub command_id: CommandId,
    pub runtime_id: RuntimeId,
    pub plan_id: String,
    pub expected_plan_revision: String,
    pub confirmed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approved_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_note: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OrchestraCancelCommandRequest {
    pub principal: Principal,
    pub capabilities: CapabilitySet,
    pub command_id: CommandId,
    pub runtime_id: RuntimeId,
    pub run_id: String,
    pub confirmed: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OrchestraRetryCommandRequest {
    pub principal: Principal,
    pub capabilities: CapabilitySet,
    pub command_id: CommandId,
    pub runtime_id: RuntimeId,
    pub run_id: String,
    pub expected_plan_revision: String,
    pub confirmed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approved_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_note: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OrchestraHistoryRequest {
    pub principal: Principal,
    pub capabilities: CapabilitySet,
    pub runtime_id: Option<String>,
    pub run_id: Option<String>,
    pub offset: u32,
    pub limit: u16,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OrchestraDeleteRequest {
    pub principal: Principal,
    pub capabilities: CapabilitySet,
    pub runtime_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OrchestraDeleteCommandRequest {
    pub principal: Principal,
    pub capabilities: CapabilitySet,
    pub command_id: CommandId,
    pub runtime_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OrchestraDeleteReplayHorizonRequest {
    pub principal: Principal,
    pub capabilities: CapabilitySet,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OrchestraDeleteReplayCheckpointRequest {
    pub principal: Principal,
    pub capabilities: CapabilitySet,
    pub minimum_retained_generation: u64,
    pub observed_through_generation: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeUnregisterRequest {
    pub principal: Principal,
    pub capabilities: CapabilitySet,
    pub command_id: CommandId,
    pub targets: Vec<RuntimeUnregisterTarget>,
    pub confirmed: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeUnregisterTarget {
    pub runtime_id: RuntimeId,
    pub expected_revision: Revision,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeUnregistrationReceiptRequest {
    pub principal: Principal,
    pub capabilities: CapabilitySet,
    pub command_id: CommandId,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorityWriterClaimRequest {
    pub principal: Principal,
    pub capabilities: CapabilitySet,
    pub writer_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorityWriterFence {
    pub generation: u64,
    pub writer_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BootstrapHandoffRequest {
    pub principal: Principal,
    pub capabilities: CapabilitySet,
    pub bootstrap_id: BootstrapId,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BootstrapSessionBindRequest {
    pub principal: Principal,
    pub capabilities: CapabilitySet,
    pub bootstrap_id: BootstrapId,
    pub confirmed: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DebuggerSessionsRequest {
    pub principal: Principal,
    pub capabilities: CapabilitySet,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DebuggerSessionStartRequest {
    pub principal: Principal,
    pub capabilities: CapabilitySet,
    pub session_id: String,
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_revision: Option<Revision>,
    pub timeout_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RequestEnvelope {
    pub schema_version: u32,
    pub request: ProtocolRequest,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
// Preserve the v1 Rust and wire response shape until the v2 schema seal.
#[allow(clippy::large_enum_variant)]
pub enum ProtocolResponse {
    Command(Box<CommandResult>),
    Query(QueryResult),
    Health(HealthResponse),
    DeploymentReceipt(DeploymentReceiptResponse),
    OrchestraPersisted(OrchestraPersistenceResponse),
    OrchestraPlanCatalog(OrchestraPlanCatalogResponse),
    OrchestraRunReceipt(OrchestraRunReceiptResponse),
    OrchestraHistory(OrchestraHistoryResponse),
    OrchestraDeleted(OrchestraDeleteResponse),
    OrchestraDeleteReceipt(OrchestraDeleteReceiptResponse),
    OrchestraDeleteReplayHorizon(OrchestraDeleteReplayHorizonResponse),
    RuntimeUnregistered(RuntimeUnregisterResponse),
    RuntimeUnregistrationReceipt(RuntimeUnregistrationReceiptLookupResponse),
    AuthorityWriterClaimed(AuthorityWriterClaimResponse),
    BootstrapHandoff(DeploymentBootstrapSnapshot),
    DebuggerSessions(DebuggerSessionsResponse),
    DebuggerSessionStarted(DebuggerSessionResponse),
    DebuggerCancelled(DebuggerCancelResponse),
    Error(ProtocolError),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DebuggerSessionView {
    pub projection: DebuggerProjection,
    pub document: UiDocument,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DebuggerSessionsResponse {
    pub sessions: Vec<DebuggerSessionView>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DebuggerSessionResponse {
    pub session: DebuggerSessionView,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DebuggerMutationStatus {
    Planned,
    Applied,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DebuggerCancelResponse {
    pub command_id: CommandId,
    pub status: DebuggerMutationStatus,
    pub session: DebuggerSessionView,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audited_at_ms: Option<u64>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeploymentReceiptStatus {
    Pending,
    Completed,
    Failed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DeploymentReceiptResponse {
    pub command_id: CommandId,
    pub request_id: String,
    pub status: DeploymentReceiptStatus,
    pub attempt: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome: Option<RuntimeDeploymentOutcome>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OrchestraPersistenceResponse {
    pub envelope: compatibility_v1::LegacyOrchestraPersistenceEnvelope,
    pub event_count: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OrchestraPlanCatalogResponse {
    pub runtime_id: RuntimeId,
    pub runtime_name: String,
    pub runtime_revision: Revision,
    pub status_source: String,
    pub attention_severity: String,
    pub needs_attention: bool,
    pub attention_reasons: Vec<String>,
    pub plans: Vec<OrchestraPlan>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OrchestraPlan {
    pub plan_id: String,
    pub intent: String,
    pub title: String,
    pub summary: String,
    pub risk_level: String,
    pub execution_readiness: String,
    pub execution_mode: String,
    pub approval_mode: String,
    pub revision: String,
    pub reasons: Vec<String>,
    pub required_capabilities: Vec<String>,
    pub steps: Vec<OrchestraPlanStep>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OrchestraPlanStep {
    pub key: String,
    pub title: String,
    pub detail: String,
    pub kind: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OrchestraControlOperation {
    Run,
    Cancel,
    Retry,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OrchestraRunReceiptResponse {
    pub command_id: CommandId,
    pub operation: OrchestraControlOperation,
    pub run: compatibility_v1::LegacyOrchestraRun,
    pub replayed: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OrchestraHistoryResponse {
    pub runs: Vec<compatibility_v1::LegacyOrchestraRun>,
    pub events: Vec<compatibility_v1::LegacyOrchestraEvent>,
    pub next_offset: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OrchestraDeleteResponse {
    pub deleted_runtime_count: u32,
    pub deleted_run_count: u64,
    pub deleted_event_count: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OrchestraDeleteReceiptResponse {
    pub command_id: CommandId,
    pub operation_generation: u64,
    pub runtime_ids: Vec<String>,
    pub deleted_runtime_count: u32,
    pub deleted_run_count: u64,
    pub deleted_event_count: u64,
    pub committed_at_unix_ms: i64,
    pub replayed: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OrchestraDeleteReplayHorizonResponse {
    pub capacity: u64,
    pub retained: u64,
    pub available_capacity: u64,
    pub warning_available_capacity: u64,
    pub critical_available_capacity: u64,
    pub warning_recovery_available_capacity: u64,
    pub critical_recovery_available_capacity: u64,
    pub checkpoint_lag_generations: u64,
    pub saturated: bool,
    pub admission_state: OrchestraDeleteReplayAdmissionState,
    pub admission_pressure: OrchestraDeleteReplayAdmissionPressure,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operator_action: Option<OrchestraDeleteReplayOperatorAction>,
    pub oldest_generation: Option<u64>,
    pub newest_generation: Option<u64>,
    pub next_generation: u64,
    pub evicted_through_generation: u64,
    pub protected_from_generation: Option<u64>,
    pub checkpointed_through_generation: Option<u64>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OrchestraDeleteReplayAdmissionState {
    Ready,
    BlockedByReconciliationAudit,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OrchestraDeleteReplayAdmissionPressure {
    Healthy,
    Warning,
    Critical,
    Blocked,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OrchestraDeleteReplayOperatorAction {
    PersistAuditAndAdvanceCheckpoint,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeUnregisterResponse {
    pub command_id: CommandId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_generation: Option<u64>,
    pub removed: Vec<RuntimeUnregisterTarget>,
    pub deleted_orchestra_runtime_count: u32,
    pub deleted_orchestra_run_count: u64,
    pub deleted_orchestra_event_count: u64,
    pub removed_at_unix_ms: i64,
    pub replayed: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeUnregistrationReceipt {
    pub operation_generation: u64,
    pub removed: Vec<RuntimeUnregisterTarget>,
    pub deleted_orchestra_runtime_count: u32,
    pub deleted_orchestra_run_count: u64,
    pub deleted_orchestra_event_count: u64,
    pub removed_at_unix_ms: i64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeUnregistrationReceiptLookupResponse {
    pub command_id: CommandId,
    pub receipt: Option<RuntimeUnregistrationReceipt>,
    pub replay_horizon: RuntimeUnregistrationReplayHorizonHealth,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorityWriterClaimResponse {
    pub generation: u64,
    pub writer_id: String,
    pub replayed: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HealthResponse {
    pub status: String,
    pub authority_owned: bool,
    pub protocol_schema_version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effect_queue: Option<EffectQueueHealth>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_unregistration_replay_horizon: Option<RuntimeUnregistrationReplayHorizonHealth>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub orchestra_delete_replay_horizon: Option<OrchestraDeleteReplayHorizonResponse>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EffectQueueHealth {
    pub ready: u64,
    pub leased: u64,
    pub completed: u64,
    pub failed: u64,
    pub active: u64,
    pub terminal: u64,
    pub capacity: u64,
    pub saturated: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeUnregistrationReplayHorizonHealth {
    pub capacity: u64,
    pub retained: u64,
    pub oldest_generation: Option<u64>,
    pub newest_generation: Option<u64>,
    pub next_generation: u64,
    pub evicted_through_generation: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResponseEnvelope {
    pub schema_version: u32,
    pub response: ProtocolResponse,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProtocolError {
    pub code: String,
    pub message: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EventEnvelope {
    pub schema_version: u32,
    pub event: ProtocolEvent,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
pub enum ProtocolEvent {
    RuntimeSnapshot {
        revision: Revision,
        resumed_after: Option<Revision>,
        runtimes: Vec<RemoteRuntimeProjection>,
    },
    Heartbeat {
        revision: Revision,
    },
    ResyncRequired {
        requested_after: Revision,
        current_revision: Revision,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteRuntimeProjection {
    pub id: RuntimeId,
    pub name: String,
    pub revision: Revision,
    pub refresh_count: u64,
    pub refresh_status: RefreshStatus,
    pub tags: RuntimeTags,
    pub status: RuntimeStatusSnapshot,
    #[serde(
        default,
        skip_serializing_if = "RuntimeCapabilitySnapshot::is_unobserved"
    )]
    pub capabilities: RuntimeCapabilitySnapshot,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capabilities_observed_for_revision: Option<Revision>,
}

impl From<RuntimeProjection> for RemoteRuntimeProjection {
    fn from(runtime: RuntimeProjection) -> Self {
        Self {
            id: runtime.id,
            name: runtime.name,
            revision: runtime.revision,
            refresh_count: runtime.refresh_count,
            refresh_status: runtime.refresh_status,
            tags: runtime.tags,
            status: runtime.status,
            capabilities: runtime.capabilities,
            capabilities_observed_for_revision: runtime.capabilities_observed_for_revision,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodeError {
    Oversized { size: usize, limit: usize },
    InvalidJson(String),
    InvalidSchemaVersion { actual: u32, expected: u32 },
    InvalidDomainSchemaVersion { actual: u32, expected: u32 },
}

pub fn decode_request(bytes: &[u8]) -> Result<RequestEnvelope, DecodeError> {
    if bytes.len() > MAX_PROTOCOL_MESSAGE_BYTES {
        return Err(DecodeError::Oversized {
            size: bytes.len(),
            limit: MAX_PROTOCOL_MESSAGE_BYTES,
        });
    }
    let envelope: RequestEnvelope =
        serde_json::from_slice(bytes).map_err(|err| DecodeError::InvalidJson(err.to_string()))?;
    if envelope.schema_version != PROTOCOL_SCHEMA_VERSION {
        return Err(DecodeError::InvalidSchemaVersion {
            actual: envelope.schema_version,
            expected: PROTOCOL_SCHEMA_VERSION,
        });
    }
    let domain_version = match &envelope.request {
        ProtocolRequest::Command(command) => command.schema_version,
        ProtocolRequest::Query(query) => query.schema_version,
        ProtocolRequest::Health(_)
        | ProtocolRequest::DeploymentReceipt(_)
        | ProtocolRequest::OrchestraPersist(_)
        | ProtocolRequest::OrchestraPlanCatalog(_)
        | ProtocolRequest::OrchestraRunCommand(_)
        | ProtocolRequest::OrchestraCancelCommand(_)
        | ProtocolRequest::OrchestraRetryCommand(_)
        | ProtocolRequest::OrchestraHistory(_)
        | ProtocolRequest::OrchestraDelete(_)
        | ProtocolRequest::OrchestraDeleteCommand(_)
        | ProtocolRequest::OrchestraDeleteReplayHorizon(_)
        | ProtocolRequest::OrchestraDeleteReplayCheckpoint(_)
        | ProtocolRequest::RuntimeUnregister(_)
        | ProtocolRequest::RuntimeUnregistrationReceipt(_)
        | ProtocolRequest::AuthorityWriterClaim(_)
        | ProtocolRequest::BootstrapHandoff(_)
        | ProtocolRequest::BootstrapSessionBind(_)
        | ProtocolRequest::DebuggerSessions(_)
        | ProtocolRequest::DebuggerSessionStart(_) => DOMAIN_SCHEMA_VERSION,
    };
    if domain_version != DOMAIN_SCHEMA_VERSION {
        return Err(DecodeError::InvalidDomainSchemaVersion {
            actual: domain_version,
            expected: DOMAIN_SCHEMA_VERSION,
        });
    }
    Ok(envelope)
}

pub fn encode_request(envelope: &RequestEnvelope) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(envelope)
}

pub fn encode_response(envelope: &ResponseEnvelope) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(envelope)
}

pub fn decode_response(bytes: &[u8]) -> Result<ResponseEnvelope, DecodeError> {
    if bytes.len() > MAX_PROTOCOL_MESSAGE_BYTES {
        return Err(DecodeError::Oversized {
            size: bytes.len(),
            limit: MAX_PROTOCOL_MESSAGE_BYTES,
        });
    }
    let envelope: ResponseEnvelope =
        serde_json::from_slice(bytes).map_err(|err| DecodeError::InvalidJson(err.to_string()))?;
    if envelope.schema_version != PROTOCOL_SCHEMA_VERSION {
        return Err(DecodeError::InvalidSchemaVersion {
            actual: envelope.schema_version,
            expected: PROTOCOL_SCHEMA_VERSION,
        });
    }
    Ok(envelope)
}

pub fn encode_event(envelope: &EventEnvelope) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(envelope)
}

pub fn decode_event(bytes: &[u8]) -> Result<EventEnvelope, DecodeError> {
    if bytes.len() > MAX_PROTOCOL_MESSAGE_BYTES {
        return Err(DecodeError::Oversized {
            size: bytes.len(),
            limit: MAX_PROTOCOL_MESSAGE_BYTES,
        });
    }
    let envelope: EventEnvelope =
        serde_json::from_slice(bytes).map_err(|err| DecodeError::InvalidJson(err.to_string()))?;
    if envelope.schema_version != EVENT_SCHEMA_VERSION {
        return Err(DecodeError::InvalidSchemaVersion {
            actual: envelope.schema_version,
            expected: EVENT_SCHEMA_VERSION,
        });
    }
    Ok(envelope)
}

pub fn domain_error_response(error: &DomainError) -> ResponseEnvelope {
    let code = match error {
        DomainError::InvalidIdentifier { .. } => "invalid_identifier",
        DomainError::InvalidSchemaVersion { .. } => "invalid_domain_schema_version",
        DomainError::Unauthorized { .. } => "unauthorized",
        DomainError::RuntimeNotFound { .. } => "runtime_not_found",
        DomainError::RuntimeAlreadyExists { .. } => "runtime_already_exists",
        DomainError::RuntimeEndpointConflict { .. } => "runtime_endpoint_conflict",
        DomainError::RevisionConflict { .. } => "revision_conflict",
        DomainError::IdempotencyConflict { .. } => "idempotency_conflict",
        DomainError::ConfirmationRequired => "confirmation_required",
        DomainError::InvalidQuery { .. } => "invalid_query",
    };
    ResponseEnvelope {
        schema_version: PROTOCOL_SCHEMA_VERSION,
        response: ProtocolResponse::Error(ProtocolError {
            code: code.to_string(),
            message: error.to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use leserpent_domain::bootstrap::{
        BootstrapId, BootstrapPhase, BootstrapTarget, BootstrapTransport, CAPABILITY_HOST_BOOTSTRAP,
    };
    use leserpent_domain::{
        CAPABILITY_DEBUGGER_CONTROL, CAPABILITY_ORCHESTRA_WRITE, CAPABILITY_RUNTIME_DEPLOY,
        CAPABILITY_RUNTIME_READ, CAPABILITY_RUNTIME_REGISTER, CapabilitySet, Command,
        CommandEnvelope, CommandId, CommandOrigin, Confirmation, IdempotencyKey, Principal, Query,
        QueryEnvelope, RuntimeId, RuntimeListFilter, RuntimeSidecarStatusSnapshot, RuntimeTags,
    };

    use super::*;

    #[test]
    fn runtime_list_request_round_trips_with_explicit_versions() {
        let request = RequestEnvelope {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            request: ProtocolRequest::Query(QueryEnvelope {
                schema_version: DOMAIN_SCHEMA_VERSION,
                principal: Principal {
                    id: "operator".to_string(),
                },
                capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
                query: Query::RuntimeList {
                    filter: RuntimeListFilter::default(),
                },
            }),
        };
        let bytes = encode_request(&request).unwrap();
        assert_eq!(decode_request(&bytes).unwrap(), request);
    }

    #[test]
    fn debugger_session_request_is_strict_and_secret_free() {
        let request = RequestEnvelope {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            request: ProtocolRequest::DebuggerSessionStart(DebuggerSessionStartRequest {
                principal: Principal {
                    id: "operator".into(),
                },
                capabilities: CapabilitySet::new([CAPABILITY_DEBUGGER_CONTROL]),
                session_id: "debugger-session-a".into(),
                source: "fn main() = runtime.list()".into(),
                expected_revision: Some(Revision(7)),
                timeout_ms: 30_000,
            }),
        };
        let bytes = encode_request(&request).unwrap();
        assert_eq!(decode_request(&bytes).unwrap(), request);
        assert!(!String::from_utf8_lossy(&bytes).contains("token"));

        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        value["request"]["payload"]["credential"] =
            serde_json::Value::String("must-not-cross-the-debugger-boundary".into());
        assert!(matches!(
            decode_request(&serde_json::to_vec(&value).unwrap()),
            Err(DecodeError::InvalidJson(message)) if message.contains("unknown field")
        ));
    }

    #[test]
    fn runtime_registration_request_round_trips_as_a_strict_typed_command() {
        let request = RequestEnvelope {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            request: ProtocolRequest::Command(CommandEnvelope {
                schema_version: DOMAIN_SCHEMA_VERSION,
                command_id: CommandId::new("register-command").unwrap(),
                idempotency_key: IdempotencyKey::new("register-request").unwrap(),
                expected_revision: None,
                principal: Principal {
                    id: "web-bridge".into(),
                },
                capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_REGISTER]),
                origin: CommandOrigin::CompatibilityAdapter,
                confirmation: Confirmation::Confirmed,
                dry_run: false,
                command: Command::RuntimeRegister {
                    runtime_id: RuntimeId::new("runtime-new").unwrap(),
                    name: "Runtime New".into(),
                    endpoint: "https://127.0.0.1:9443".into(),
                    sidecar_endpoint: Some("https://127.0.0.1:9444".into()),
                    tags: RuntimeTags::default(),
                },
            }),
        };
        let bytes = encode_request(&request).unwrap();
        assert_eq!(decode_request(&bytes).unwrap(), request);

        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        value["request"]["payload"]["command"]
            .as_object_mut()
            .unwrap()
            .remove("sidecar_endpoint");
        let legacy = decode_request(&serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(matches!(
            legacy.request,
            ProtocolRequest::Command(CommandEnvelope {
                command: Command::RuntimeRegister {
                    sidecar_endpoint: None,
                    ..
                },
                ..
            })
        ));

        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        value["request"]["payload"]["command"]["pairing_token"] =
            serde_json::Value::String("must-not-cross-the-domain-boundary".into());
        assert!(matches!(
            decode_request(&serde_json::to_vec(&value).unwrap()),
            Err(DecodeError::InvalidJson(message)) if message.contains("unknown field")
        ));
    }

    #[test]
    fn runtime_registration_update_round_trips_with_an_explicit_revision_fence() {
        let request = RequestEnvelope {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            request: ProtocolRequest::Command(CommandEnvelope {
                schema_version: DOMAIN_SCHEMA_VERSION,
                command_id: CommandId::new("registration-update-command").unwrap(),
                idempotency_key: IdempotencyKey::new("registration-update-request").unwrap(),
                expected_revision: Some(Revision(7)),
                principal: Principal {
                    id: "web-bridge".into(),
                },
                capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_REGISTER]),
                origin: CommandOrigin::CompatibilityAdapter,
                confirmation: Confirmation::Confirmed,
                dry_run: false,
                command: Command::RuntimeRegistrationUpdate {
                    runtime_id: RuntimeId::new("runtime-existing").unwrap(),
                    name: "Runtime Updated".into(),
                    endpoint: "https://127.0.0.1:9553".into(),
                    sidecar_endpoint: None,
                    tags: RuntimeTags::default(),
                },
            }),
        };
        let bytes = encode_request(&request).unwrap();
        assert_eq!(decode_request(&bytes).unwrap(), request);

        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        value["request"]["payload"]["command"]["admin_token"] =
            serde_json::Value::String("must-not-cross-the-domain-boundary".into());
        assert!(matches!(
            decode_request(&serde_json::to_vec(&value).unwrap()),
            Err(DecodeError::InvalidJson(message)) if message.contains("unknown field")
        ));
    }

    #[test]
    fn runtime_discovery_intake_round_trips_without_secret_or_raw_payload_fields() {
        let request = RequestEnvelope {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            request: ProtocolRequest::Command(CommandEnvelope {
                schema_version: DOMAIN_SCHEMA_VERSION,
                command_id: CommandId::new("discovery-intake-command").unwrap(),
                idempotency_key: IdempotencyKey::new("discovery-intake-request").unwrap(),
                expected_revision: Some(Revision(2)),
                principal: Principal {
                    id: "web-bridge".into(),
                },
                capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_REGISTER]),
                origin: CommandOrigin::CompatibilityAdapter,
                confirmation: Confirmation::Confirmed,
                dry_run: false,
                command: Command::RuntimeDiscoveryIntake {
                    runtime_id: RuntimeId::new("runtime-existing").unwrap(),
                    capabilities: Some(Box::new(RuntimeCapabilitySnapshot {
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
                    })),
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
        };
        let bytes = encode_request(&request).unwrap();
        assert_eq!(decode_request(&bytes).unwrap(), request);

        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        value["request"]["payload"]["command"]["pairing_token"] =
            serde_json::Value::String("must-not-cross-the-domain-boundary".into());
        assert!(matches!(
            decode_request(&serde_json::to_vec(&value).unwrap()),
            Err(DecodeError::InvalidJson(message)) if message.contains("unknown field")
        ));
    }

    #[test]
    fn runtime_logs_request_round_trips_with_bounded_cursor() {
        let request = RequestEnvelope {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            request: ProtocolRequest::Query(QueryEnvelope {
                schema_version: DOMAIN_SCHEMA_VERSION,
                principal: Principal {
                    id: "operator".to_string(),
                },
                capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
                query: Query::RuntimeLogs {
                    runtime_id: leserpent_domain::RuntimeId::new("runtime-a").unwrap(),
                    after_sequence: Some(41),
                    limit: 128,
                },
            }),
        };
        let bytes = encode_request(&request).unwrap();
        assert_eq!(decode_request(&bytes).unwrap(), request);
    }

    #[test]
    fn canonical_runtime_list_fixtures_decode() {
        let request = decode_request(include_bytes!(
            "../tests/fixtures/runtime-list-request-v1.json"
        ))
        .expect("request fixture should decode");
        assert!(matches!(request.request, ProtocolRequest::Query(_)));

        let response = decode_response(include_bytes!(
            "../tests/fixtures/runtime-list-response-v1.json"
        ))
        .expect("response fixture should decode");
        assert!(matches!(response.response, ProtocolResponse::Query(_)));
    }

    #[test]
    fn decoder_rejects_oversized_and_unknown_version_messages() {
        let oversized = vec![b' '; MAX_PROTOCOL_MESSAGE_BYTES + 1];
        assert!(matches!(
            decode_request(&oversized),
            Err(DecodeError::Oversized { .. })
        ));

        let source = br#"{"schema_version":2,"request":{"kind":"query","payload":{"schema_version":1,"principal":{"id":"operator"},"capabilities":["runtime.read"],"query":{"kind":"runtime_list","filter":{"environment":null,"cluster":null,"role":null}}}}}"#;
        assert!(matches!(
            decode_request(source),
            Err(DecodeError::InvalidSchemaVersion { .. })
        ));
    }

    #[test]
    fn endpoint_conflict_uses_a_typed_non_disclosing_wire_error() {
        let response = domain_error_response(&DomainError::RuntimeEndpointConflict {
            runtime_id: "runtime-owner".into(),
        });
        assert!(matches!(
            response.response,
            ProtocolResponse::Error(ProtocolError { code, message })
                if code == "runtime_endpoint_conflict"
                    && message.contains("runtime-owner")
                    && !message.contains("https://")
        ));
    }

    #[test]
    fn decoders_reject_unknown_versioned_envelope_fields() {
        let request =
            br#"{"schema_version":1,"request":{"kind":"health","payload":{}},"schemaVersion":1}"#;
        assert!(matches!(
            decode_request(request),
            Err(DecodeError::InvalidJson(message)) if message.contains("unknown field")
        ));

        let response = br#"{"schema_version":1,"response":{"kind":"health","payload":{"status":"ready","authority_owned":true,"protocol_schema_version":1}},"request_id":"ignored"}"#;
        assert!(matches!(
            decode_response(response),
            Err(DecodeError::InvalidJson(message)) if message.contains("unknown field")
        ));

        let event = br#"{"schema_version":1,"event":{"kind":"heartbeat","payload":{"revision":1}},"cursor":1}"#;
        assert!(matches!(
            decode_event(event),
            Err(DecodeError::InvalidJson(message)) if message.contains("unknown field")
        ));
    }

    #[test]
    fn health_payload_rejects_unknown_fields() {
        let request =
            br#"{"schema_version":1,"request":{"kind":"health","payload":{"admin":true}}}"#;
        assert!(matches!(
            decode_request(request),
            Err(DecodeError::InvalidJson(message)) if message.contains("unknown field")
        ));

        let response = br#"{"schema_version":1,"response":{"kind":"health","payload":{"status":"ready","authority_owned":true,"protocol_schema_version":1,"effect_queue":null,"healthy":true}}}"#;
        assert!(matches!(
            decode_response(response),
            Err(DecodeError::InvalidJson(message)) if message.contains("unknown field")
        ));
    }

    #[test]
    fn health_request_round_trips_without_domain_payload() {
        let request = RequestEnvelope {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            request: ProtocolRequest::Health(HealthRequest {}),
        };
        assert_eq!(
            decode_request(&encode_request(&request).unwrap()).unwrap(),
            request
        );
    }

    #[test]
    fn deployment_receipt_round_trips_as_a_typed_terminal_response() {
        let request = RequestEnvelope {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            request: ProtocolRequest::DeploymentReceipt(DeploymentReceiptRequest {
                principal: Principal {
                    id: "operator.example".into(),
                },
                capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_DEPLOY]),
                command_id: CommandId::new("deploy-command").unwrap(),
                request_id: "deploy-1".into(),
            }),
        };
        assert_eq!(
            decode_request(&encode_request(&request).unwrap()).unwrap(),
            request
        );

        let response = ResponseEnvelope {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            response: ProtocolResponse::DeploymentReceipt(DeploymentReceiptResponse {
                command_id: CommandId::new("deploy-command").unwrap(),
                request_id: "deploy-1".into(),
                status: DeploymentReceiptStatus::Completed,
                attempt: 1,
                outcome: Some(RuntimeDeploymentOutcome {
                    deployment_id: "gdep-1".into(),
                    request_id: "deploy-1".into(),
                    pipeline_kind: "http/request".into(),
                    requested_by: "operator.example".into(),
                    status: "accepted".into(),
                    accepted_unix_ms: 1_700_000_000_000,
                    target: None,
                    replayed: false,
                }),
                error: None,
            }),
        };
        assert_eq!(
            decode_response(&encode_response(&response).unwrap()).unwrap(),
            response
        );

        let history = RequestEnvelope {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            request: ProtocolRequest::OrchestraHistory(OrchestraHistoryRequest {
                principal: Principal {
                    id: "operator-a".into(),
                },
                capabilities: CapabilitySet::new([CAPABILITY_ORCHESTRA_WRITE]),
                runtime_id: Some("runtime-a".into()),
                run_id: Some("orun-1".into()),
                offset: 0,
                limit: 64,
            }),
        };
        assert_eq!(
            decode_request(&encode_request(&history).unwrap()).unwrap(),
            history
        );
        let delete = RequestEnvelope {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            request: ProtocolRequest::OrchestraDelete(OrchestraDeleteRequest {
                principal: Principal {
                    id: "operator-a".into(),
                },
                capabilities: CapabilitySet::new([CAPABILITY_ORCHESTRA_WRITE]),
                runtime_ids: vec!["runtime-a".into()],
            }),
        };
        assert_eq!(
            decode_request(&encode_request(&delete).unwrap()).unwrap(),
            delete
        );
        let delete_command = RequestEnvelope {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            request: ProtocolRequest::OrchestraDeleteCommand(OrchestraDeleteCommandRequest {
                principal: Principal {
                    id: "operator-a".into(),
                },
                capabilities: CapabilitySet::new([CAPABILITY_ORCHESTRA_WRITE]),
                command_id: CommandId::new("orchestra-delete-runtime-a").unwrap(),
                runtime_ids: vec!["runtime-a".into()],
            }),
        };
        assert_eq!(
            decode_request(&encode_request(&delete_command).unwrap()).unwrap(),
            delete_command
        );
        let delete_receipt = ResponseEnvelope {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            response: ProtocolResponse::OrchestraDeleteReceipt(OrchestraDeleteReceiptResponse {
                command_id: CommandId::new("orchestra-delete-runtime-a").unwrap(),
                operation_generation: 7,
                runtime_ids: vec!["runtime-a".into()],
                deleted_runtime_count: 1,
                deleted_run_count: 2,
                deleted_event_count: 3,
                committed_at_unix_ms: 1_784_620_800_000,
                replayed: true,
            }),
        };
        assert_eq!(
            decode_response(&encode_response(&delete_receipt).unwrap()).unwrap(),
            delete_receipt
        );
        let replay_checkpoint = RequestEnvelope {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            request: ProtocolRequest::OrchestraDeleteReplayCheckpoint(
                OrchestraDeleteReplayCheckpointRequest {
                    principal: Principal {
                        id: "operator-a".into(),
                    },
                    capabilities: CapabilitySet::new([CAPABILITY_ORCHESTRA_WRITE]),
                    minimum_retained_generation: 4,
                    observed_through_generation: 7,
                },
            ),
        };
        assert_eq!(
            decode_request(&encode_request(&replay_checkpoint).unwrap()).unwrap(),
            replay_checkpoint
        );
        let replay_horizon = ResponseEnvelope {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            response: ProtocolResponse::OrchestraDeleteReplayHorizon(
                OrchestraDeleteReplayHorizonResponse {
                    capacity: 4_096,
                    retained: 4,
                    available_capacity: 4_092,
                    warning_available_capacity: 512,
                    critical_available_capacity: 128,
                    warning_recovery_available_capacity: 768,
                    critical_recovery_available_capacity: 256,
                    checkpoint_lag_generations: 3,
                    saturated: false,
                    admission_state: OrchestraDeleteReplayAdmissionState::Ready,
                    admission_pressure: OrchestraDeleteReplayAdmissionPressure::Healthy,
                    operator_action: None,
                    oldest_generation: Some(4),
                    newest_generation: Some(7),
                    next_generation: 8,
                    evicted_through_generation: 3,
                    protected_from_generation: Some(4),
                    checkpointed_through_generation: Some(4),
                },
            ),
        };
        assert_eq!(
            decode_response(&encode_response(&replay_horizon).unwrap()).unwrap(),
            replay_horizon
        );
        let saturated_horizon = ResponseEnvelope {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            response: ProtocolResponse::OrchestraDeleteReplayHorizon(
                OrchestraDeleteReplayHorizonResponse {
                    capacity: 4_096,
                    retained: 4_096,
                    available_capacity: 0,
                    warning_available_capacity: 512,
                    critical_available_capacity: 128,
                    warning_recovery_available_capacity: 768,
                    critical_recovery_available_capacity: 256,
                    checkpoint_lag_generations: 4_095,
                    saturated: true,
                    admission_state:
                        OrchestraDeleteReplayAdmissionState::BlockedByReconciliationAudit,
                    admission_pressure: OrchestraDeleteReplayAdmissionPressure::Blocked,
                    operator_action: Some(
                        OrchestraDeleteReplayOperatorAction::PersistAuditAndAdvanceCheckpoint,
                    ),
                    oldest_generation: Some(1),
                    newest_generation: Some(4_096),
                    next_generation: 4_097,
                    evicted_through_generation: 0,
                    protected_from_generation: Some(1),
                    checkpointed_through_generation: Some(1),
                },
            ),
        };
        assert_eq!(
            decode_response(&encode_response(&saturated_horizon).unwrap()).unwrap(),
            saturated_horizon
        );
        let unregister = RequestEnvelope {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            request: ProtocolRequest::RuntimeUnregister(RuntimeUnregisterRequest {
                principal: Principal {
                    id: "operator-a".into(),
                },
                capabilities: CapabilitySet::new([leserpent_domain::CAPABILITY_RUNTIME_UNREGISTER]),
                command_id: CommandId::new("unregister-runtime-a").unwrap(),
                targets: vec![RuntimeUnregisterTarget {
                    runtime_id: RuntimeId::new("runtime-a").unwrap(),
                    expected_revision: Revision(7),
                }],
                confirmed: true,
            }),
        };
        assert_eq!(
            decode_request(&encode_request(&unregister).unwrap()).unwrap(),
            unregister
        );
    }

    #[test]
    fn orchestra_control_contracts_are_strict_identity_bound_and_typed() {
        let principal = Principal {
            id: "operator-a".into(),
        };
        let capabilities = CapabilitySet::new([CAPABILITY_ORCHESTRA_WRITE]);
        let runtime_id = RuntimeId::new("runtime-a").unwrap();
        let plan_request = RequestEnvelope {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            request: ProtocolRequest::OrchestraPlanCatalog(OrchestraPlanCatalogRequest {
                principal: principal.clone(),
                capabilities: capabilities.clone(),
                runtime_id: runtime_id.clone(),
            }),
        };
        assert_eq!(
            decode_request(&encode_request(&plan_request).unwrap()).unwrap(),
            plan_request
        );

        let run_request = RequestEnvelope {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            request: ProtocolRequest::OrchestraRunCommand(OrchestraRunCommandRequest {
                principal: principal.clone(),
                capabilities: capabilities.clone(),
                command_id: CommandId::new("orchestra-run-0001").unwrap(),
                runtime_id: runtime_id.clone(),
                plan_id: "runtime_triage".into(),
                expected_plan_revision: "orchestra-v1-7-runtime_triage".into(),
                confirmed: true,
                approved_by: Some("operator-a".into()),
                approval_note: Some("reviewed".into()),
            }),
        };
        let bytes = encode_request(&run_request).unwrap();
        assert_eq!(decode_request(&bytes).unwrap(), run_request);
        let mut forged: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        forged["request"]["payload"]["admin_token"] =
            serde_json::Value::String("must-not-cross-the-wire".into());
        assert!(matches!(
            decode_request(&serde_json::to_vec(&forged).unwrap()),
            Err(DecodeError::InvalidJson(message)) if message.contains("unknown field")
        ));

        let cancel_request = RequestEnvelope {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            request: ProtocolRequest::OrchestraCancelCommand(OrchestraCancelCommandRequest {
                principal: principal.clone(),
                capabilities: capabilities.clone(),
                command_id: CommandId::new("orchestra-cancel-0001").unwrap(),
                runtime_id: runtime_id.clone(),
                run_id: "orun-orchestra-run-0001".into(),
                confirmed: true,
            }),
        };
        assert_eq!(
            decode_request(&encode_request(&cancel_request).unwrap()).unwrap(),
            cancel_request
        );

        let retry_request = RequestEnvelope {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            request: ProtocolRequest::OrchestraRetryCommand(OrchestraRetryCommandRequest {
                principal,
                capabilities,
                command_id: CommandId::new("orchestra-retry-0001").unwrap(),
                runtime_id: runtime_id.clone(),
                run_id: "orun-orchestra-run-0001".into(),
                expected_plan_revision: "orchestra-v1-8-runtime_triage".into(),
                confirmed: true,
                approved_by: None,
                approval_note: None,
            }),
        };
        assert_eq!(
            decode_request(&encode_request(&retry_request).unwrap()).unwrap(),
            retry_request
        );

        let response = ResponseEnvelope {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            response: ProtocolResponse::OrchestraRunReceipt(OrchestraRunReceiptResponse {
                command_id: CommandId::new("orchestra-run-0001").unwrap(),
                operation: OrchestraControlOperation::Run,
                run: compatibility_v1::LegacyOrchestraRun {
                    run_id: "orun-orchestra-run-0001".into(),
                    runtime_id: runtime_id.as_str().into(),
                    plan_id: "runtime_triage".into(),
                    outcome: "queued".into(),
                    executed_at: "2026-08-26T08:00:00Z".into(),
                    steps: Vec::new(),
                    completed_at: None,
                    attempt: 1,
                    retried_from_run_id: None,
                    approved_by: Some("operator-a".into()),
                    approval_note: Some("reviewed".into()),
                    plan_revision: Some("orchestra-v1-7-runtime_triage".into()),
                    request_id: Some("orchestra-run-0001".into()),
                },
                replayed: false,
            }),
        };
        assert_eq!(
            decode_response(&encode_response(&response).unwrap()).unwrap(),
            response
        );
    }

    #[test]
    fn bootstrap_handoff_wire_accepts_only_an_id_and_public_state() {
        let bootstrap_id = BootstrapId::new("bootstrap-wire-1").unwrap();
        let request = RequestEnvelope {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            request: ProtocolRequest::BootstrapSessionBind(BootstrapSessionBindRequest {
                principal: Principal {
                    id: "operator-a".into(),
                },
                capabilities: CapabilitySet::new([CAPABILITY_HOST_BOOTSTRAP]),
                bootstrap_id: bootstrap_id.clone(),
                confirmed: true,
            }),
        };
        let encoded = encode_request(&request).unwrap();
        assert_eq!(decode_request(&encoded).unwrap(), request);
        let mut forged: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
        forged["request"]["payload"]["authority_owned"] = serde_json::Value::Bool(true);
        assert!(matches!(
            decode_request(&serde_json::to_vec(&forged).unwrap()),
            Err(DecodeError::InvalidJson(message)) if message.contains("unknown field")
        ));

        let response = ResponseEnvelope {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            response: ProtocolResponse::BootstrapHandoff(DeploymentBootstrapSnapshot {
                bootstrap_id,
                phase: BootstrapPhase::Failed,
                target: BootstrapTarget {
                    transport: BootstrapTransport::Ssh,
                    host: "host.example".into(),
                    port: 22,
                },
                bootstrap_credential_present: false,
                daemon_id: None,
                endpoint: None,
                generation: None,
                install_profile: None,
                session_credential_handle: None,
                trust_credential_handle: None,
                fault_code: Some("transport_failure".into()),
                mutation_authorized: false,
            }),
        };
        assert_eq!(
            decode_response(&encode_response(&response).unwrap()).unwrap(),
            response
        );
    }

    #[test]
    fn orchestra_persistence_round_trips_the_atomic_legacy_envelope() {
        let envelope = compatibility_v1::decode_orchestra_persistence(include_bytes!(
            "../tests/fixtures/legacy-orchestra-persistence-v1.json"
        ))
        .unwrap();
        let request = RequestEnvelope {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            request: ProtocolRequest::OrchestraPersist(OrchestraPersistenceRequest {
                principal: Principal {
                    id: "operator-a".into(),
                },
                capabilities: CapabilitySet::new([CAPABILITY_ORCHESTRA_WRITE]),
                envelope: envelope.clone(),
            }),
        };
        assert_eq!(
            decode_request(&encode_request(&request).unwrap()).unwrap(),
            request
        );
        let response = ResponseEnvelope {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            response: ProtocolResponse::OrchestraPersisted(OrchestraPersistenceResponse {
                envelope,
                event_count: 1,
            }),
        };
        assert_eq!(
            decode_response(&encode_response(&response).unwrap()).unwrap(),
            response
        );
    }

    #[test]
    fn health_response_accepts_legacy_payload_and_round_trips_queue_pressure() {
        let legacy = br#"{"schema_version":1,"response":{"kind":"health","payload":{"status":"ready","authority_owned":true,"protocol_schema_version":1}}}"#;
        let decoded = decode_response(legacy).unwrap();
        let ProtocolResponse::Health(health) = decoded.response else {
            panic!("legacy response should remain health");
        };
        assert_eq!(health.effect_queue, None);
        assert_eq!(health.runtime_unregistration_replay_horizon, None);
        assert_eq!(health.orchestra_delete_replay_horizon, None);

        let response = ResponseEnvelope {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            response: ProtocolResponse::Health(HealthResponse {
                status: "ready".into(),
                authority_owned: true,
                protocol_schema_version: PROTOCOL_SCHEMA_VERSION,
                effect_queue: Some(EffectQueueHealth {
                    ready: 3,
                    leased: 1,
                    completed: 5,
                    failed: 2,
                    active: 4,
                    terminal: 7,
                    capacity: 10_000,
                    saturated: false,
                }),
                runtime_unregistration_replay_horizon: Some(
                    RuntimeUnregistrationReplayHorizonHealth {
                        capacity: 256,
                        retained: 12,
                        oldest_generation: Some(4),
                        newest_generation: Some(15),
                        next_generation: 16,
                        evicted_through_generation: 3,
                    },
                ),
                orchestra_delete_replay_horizon: None,
            }),
        };
        assert_eq!(
            decode_response(&encode_response(&response).unwrap()).unwrap(),
            response
        );
    }

    #[test]
    fn runtime_unregistration_receipt_preserves_generation_and_accepts_legacy_absence() {
        let legacy = br#"{"schema_version":1,"response":{"kind":"runtime_unregistered","payload":{"command_id":"runtime-unregister-a","removed":[],"deleted_orchestra_runtime_count":0,"deleted_orchestra_run_count":0,"deleted_orchestra_event_count":0,"removed_at_unix_ms":1784620800000,"replayed":true}}}"#;
        let decoded = decode_response(legacy).unwrap();
        let ProtocolResponse::RuntimeUnregistered(receipt) = decoded.response else {
            panic!("legacy response should remain a runtime-unregistration receipt");
        };
        assert_eq!(receipt.operation_generation, None);

        let current = ResponseEnvelope {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            response: ProtocolResponse::RuntimeUnregistered(RuntimeUnregisterResponse {
                command_id: CommandId::new("runtime-unregister-a").unwrap(),
                operation_generation: Some(17),
                removed: Vec::new(),
                deleted_orchestra_runtime_count: 0,
                deleted_orchestra_run_count: 0,
                deleted_orchestra_event_count: 0,
                removed_at_unix_ms: 1_784_620_800_000,
                replayed: true,
            }),
        };
        assert_eq!(
            decode_response(&encode_response(&current).unwrap()).unwrap(),
            current
        );
    }

    #[test]
    fn runtime_unregistration_receipt_lookup_round_trips_typed_presence_and_horizon() {
        let command_id = CommandId::new("runtime-unregister-a").unwrap();
        let request = RequestEnvelope {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            request: ProtocolRequest::RuntimeUnregistrationReceipt(
                RuntimeUnregistrationReceiptRequest {
                    principal: Principal {
                        id: "operator-a".into(),
                    },
                    capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
                    command_id: command_id.clone(),
                },
            ),
        };
        assert_eq!(
            decode_request(&encode_request(&request).unwrap()).unwrap(),
            request
        );

        let response = ResponseEnvelope {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            response: ProtocolResponse::RuntimeUnregistrationReceipt(
                RuntimeUnregistrationReceiptLookupResponse {
                    command_id,
                    receipt: Some(RuntimeUnregistrationReceipt {
                        operation_generation: 17,
                        removed: vec![RuntimeUnregisterTarget {
                            runtime_id: RuntimeId::new("runtime-a").unwrap(),
                            expected_revision: Revision(4),
                        }],
                        deleted_orchestra_runtime_count: 1,
                        deleted_orchestra_run_count: 2,
                        deleted_orchestra_event_count: 3,
                        removed_at_unix_ms: 1_784_620_800_000,
                    }),
                    replay_horizon: RuntimeUnregistrationReplayHorizonHealth {
                        capacity: 256,
                        retained: 1,
                        oldest_generation: Some(17),
                        newest_generation: Some(17),
                        next_generation: 18,
                        evicted_through_generation: 16,
                    },
                },
            ),
        };
        assert_eq!(
            decode_response(&encode_response(&response).unwrap()).unwrap(),
            response
        );
    }

    #[test]
    fn event_snapshot_round_trips_without_runtime_endpoint_disclosure() {
        let mut control = leserpent_domain::InMemoryControlPlane::default();
        let runtime = control.register_runtime(
            leserpent_domain::RuntimeId::new("runtime-a").unwrap(),
            "Runtime A",
            "https://secret-endpoint.invalid",
        );
        let event = EventEnvelope {
            schema_version: EVENT_SCHEMA_VERSION,
            event: ProtocolEvent::RuntimeSnapshot {
                revision: Revision(1),
                resumed_after: Some(Revision(0)),
                runtimes: vec![RemoteRuntimeProjection::from(runtime)],
            },
        };
        let encoded = encode_event(&event).unwrap();
        assert_eq!(decode_event(&encoded).unwrap(), event);
        let encoded = String::from_utf8(encoded).unwrap();
        assert!(!encoded.contains("secret-endpoint"));
        assert!(!encoded.contains("endpoint"));
    }
}
