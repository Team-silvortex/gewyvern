use std::fmt;

use leserpent_domain::{
    CapabilitySet, Command, CommandEnvelope, CommandId, CommandOrigin, Confirmation,
    DOMAIN_SCHEMA_VERSION, DomainError, IdempotencyKey, InMemoryControlPlane, Principal, Query,
    QueryEnvelope, Revision, RuntimeId, RuntimeListFilter, RuntimeStatusSnapshot, RuntimeTags,
};
use serde::{Deserialize, Serialize};

use crate::MAX_PROTOCOL_MESSAGE_BYTES;

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyRuntimeListFilter {
    pub environment: Option<String>,
    pub cluster: Option<String>,
    pub role: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyRuntimeTags {
    pub environment: Option<String>,
    pub cluster: Option<String>,
    pub role: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyRuntimeStatusSnapshot {
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
    #[serde(default)]
    pub resilience_degraded: bool,
    #[serde(default)]
    pub resilience_status: Option<String>,
    #[serde(default)]
    pub resilience_summary: Option<String>,
    #[serde(default)]
    pub socket_service_status: Option<String>,
    #[serde(default)]
    pub socket_consecutive_idle_timeouts: Option<u64>,
    #[serde(default)]
    pub socket_total_idle_timeouts: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyRuntimeSummary {
    pub runtime_id: String,
    pub name: String,
    pub endpoint: String,
    pub tags: LegacyRuntimeTags,
    pub status: LegacyRuntimeStatusSnapshot,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyRuntimeCollectionResponse {
    pub filter: LegacyRuntimeListFilter,
    pub runtimes: Vec<LegacyRuntimeSummary>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyRuntimeStatusRefreshResponse {
    pub runtime_id: String,
    pub name: String,
    pub endpoint: String,
    pub status: LegacyRuntimeStatusSnapshot,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyApiErrorResponse {
    pub error: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_id: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompatibilityError {
    Oversized { size: usize, limit: usize },
    InvalidJson(String),
    Domain(DomainError),
    RuntimeIdentityMismatch { expected: String, actual: String },
}

pub fn decode_runtime_collection(
    bytes: &[u8],
) -> Result<LegacyRuntimeCollectionResponse, CompatibilityError> {
    decode_capped(bytes)
}

pub fn decode_status_refresh(
    bytes: &[u8],
) -> Result<LegacyRuntimeStatusRefreshResponse, CompatibilityError> {
    decode_capped(bytes)
}

pub fn decode_api_error(bytes: &[u8]) -> Result<LegacyApiErrorResponse, CompatibilityError> {
    decode_capped(bytes)
}

pub fn runtime_list_query(
    principal: Principal,
    capabilities: CapabilitySet,
    filter: LegacyRuntimeListFilter,
) -> QueryEnvelope {
    QueryEnvelope {
        schema_version: DOMAIN_SCHEMA_VERSION,
        principal,
        capabilities,
        query: Query::RuntimeList {
            filter: RuntimeListFilter::from(filter).normalized(),
        },
    }
}

#[allow(clippy::too_many_arguments)]
pub fn runtime_status_refresh_command(
    principal: Principal,
    capabilities: CapabilitySet,
    runtime_id: &str,
    command_id: &str,
    idempotency_key: &str,
    expected_revision: Option<Revision>,
    dry_run: bool,
) -> Result<CommandEnvelope, CompatibilityError> {
    Ok(CommandEnvelope {
        schema_version: DOMAIN_SCHEMA_VERSION,
        command_id: CommandId::new(command_id)?,
        idempotency_key: IdempotencyKey::new(idempotency_key)?,
        expected_revision,
        principal,
        capabilities,
        origin: CommandOrigin::CompatibilityAdapter,
        confirmation: Confirmation::NotRequired,
        dry_run,
        command: Command::RuntimeRefresh {
            runtime_id: RuntimeId::new(runtime_id)?,
        },
    })
}

pub fn seed_runtime_collection(
    control: &mut InMemoryControlPlane,
    collection: LegacyRuntimeCollectionResponse,
) -> Result<(), CompatibilityError> {
    for runtime in collection.runtimes {
        control.register_runtime_with_metadata(
            RuntimeId::new(runtime.runtime_id)?,
            runtime.name,
            runtime.endpoint,
            runtime.tags.into(),
            runtime.status.into(),
        );
    }
    Ok(())
}

pub fn apply_status_refresh(
    control: &mut InMemoryControlPlane,
    expected_runtime_id: &RuntimeId,
    expected_revision: Revision,
    response: LegacyRuntimeStatusRefreshResponse,
) -> Result<leserpent_domain::RuntimeProjection, CompatibilityError> {
    if response.runtime_id != expected_runtime_id.as_str() {
        return Err(CompatibilityError::RuntimeIdentityMismatch {
            expected: expected_runtime_id.as_str().to_string(),
            actual: response.runtime_id,
        });
    }
    control
        .complete_runtime_status_refresh(
            expected_runtime_id,
            expected_revision,
            response.status.into(),
        )
        .map_err(Into::into)
}

pub fn domain_error_to_legacy(error: &DomainError) -> LegacyApiErrorResponse {
    match error {
        DomainError::RuntimeNotFound { runtime_id } => LegacyApiErrorResponse {
            error: "runtime_not_found".to_string(),
            reason: None,
            runtime_id: Some(runtime_id.clone()),
        },
        _ => LegacyApiErrorResponse {
            error: "runtime_request_failed".to_string(),
            reason: Some(error.to_string()),
            runtime_id: None,
        },
    }
}

impl From<LegacyRuntimeListFilter> for RuntimeListFilter {
    fn from(value: LegacyRuntimeListFilter) -> Self {
        Self {
            environment: value.environment,
            cluster: value.cluster,
            role: value.role,
        }
    }
}

impl From<LegacyRuntimeTags> for RuntimeTags {
    fn from(value: LegacyRuntimeTags) -> Self {
        Self {
            environment: value.environment,
            cluster: value.cluster,
            role: value.role,
        }
    }
}

impl From<LegacyRuntimeStatusSnapshot> for RuntimeStatusSnapshot {
    fn from(value: LegacyRuntimeStatusSnapshot) -> Self {
        Self {
            status_source: value.status_source,
            status_fetched_at: value.status_fetched_at,
            status_fetch_error: value.status_fetch_error,
            has_latest_snapshot: value.has_latest_snapshot,
            snapshot_kind: value.snapshot_kind,
            target_count: value.target_count,
            has_summary_json: value.has_summary_json,
            has_analysis_json: value.has_analysis_json,
            has_training_example_json: value.has_training_example_json,
            has_training_dataset_manifest: value.has_training_dataset_manifest,
            has_export_json: value.has_export_json,
            has_report_json: value.has_report_json,
            has_report_html: value.has_report_html,
            has_external_sidecar_context: value.has_external_sidecar_context,
            has_external_evidence_chain_enrichment: value.has_external_evidence_chain_enrichment,
            has_external_diagnostic_opinion: value.has_external_diagnostic_opinion,
            resilience_degraded: value.resilience_degraded,
            resilience_status: value.resilience_status,
            resilience_summary: value.resilience_summary,
            socket_service_status: value.socket_service_status,
            socket_consecutive_idle_timeouts: value.socket_consecutive_idle_timeouts,
            socket_total_idle_timeouts: value.socket_total_idle_timeouts,
        }
    }
}

impl From<DomainError> for CompatibilityError {
    fn from(value: DomainError) -> Self {
        Self::Domain(value)
    }
}

impl fmt::Display for CompatibilityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Oversized { size, limit } => {
                write!(
                    formatter,
                    "legacy response size {size} exceeds limit {limit}"
                )
            }
            Self::InvalidJson(error) => write!(formatter, "invalid legacy JSON: {error}"),
            Self::Domain(error) => error.fmt(formatter),
            Self::RuntimeIdentityMismatch { expected, actual } => write!(
                formatter,
                "legacy runtime identity mismatch: expected '{expected}', actual '{actual}'"
            ),
        }
    }
}

impl std::error::Error for CompatibilityError {}

fn decode_capped<T: for<'de> Deserialize<'de>>(bytes: &[u8]) -> Result<T, CompatibilityError> {
    if bytes.len() > MAX_PROTOCOL_MESSAGE_BYTES {
        return Err(CompatibilityError::Oversized {
            size: bytes.len(),
            limit: MAX_PROTOCOL_MESSAGE_BYTES,
        });
    }
    serde_json::from_slice(bytes)
        .map_err(|error| CompatibilityError::InvalidJson(error.to_string()))
}
