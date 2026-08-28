use std::collections::BTreeMap;

use leserpent_domain::{
    Revision, RuntimeCapabilitySnapshot, RuntimeId, RuntimeListFilter, RuntimeProjection,
    RuntimeSidecarStatusSnapshot, RuntimeStatusSnapshot,
};
use leserpent_protocol::MAX_PROTOCOL_MESSAGE_BYTES;
use leserpent_runtime::ControlRuntime;
use ring::digest;
use serde_json::{Value, json};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::web_console_error::{ConsoleWriteError, ConsoleWriteStatus};
use crate::web_console_orchestra;

const MAX_FILTER_VALUE_BYTES: usize = 128;
pub(crate) const MAX_REGISTRATION_PLAN_BYTES: usize = 8 * 1024;
pub(crate) const MAX_REGISTRATION_REQUEST_BYTES: usize = 16 * 1024;
pub(crate) const MAX_CLEANUP_REQUEST_BYTES: usize = 1024;
pub(crate) const MAX_ORCHESTRA_COMMAND_BYTES: usize = 4 * 1024;
pub(crate) const MAX_ATOMIC_CLEANUP_TARGETS: usize = 128;
pub(crate) const PERSISTENCE_EXPORT_SCHEMA_VERSION: u32 = 1;
const MAX_EXPORTED_ORCHESTRA_RUNS: usize = 4_096;
const FALLBACK_TIMESTAMP: &str = "1970-01-01T00:00:00Z";

macro_rules! asset {
    ($name:literal) => {
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../apps/leserpent/src/Leserpent/wwwroot/",
            $name
        ))
    };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ConsoleAsset {
    pub(crate) payload: &'static [u8],
    pub(crate) content_type: &'static str,
    pub(crate) cache_control: &'static str,
    pub(crate) document: bool,
}

pub(crate) fn find_asset(path: &str) -> Option<ConsoleAsset> {
    let (payload, content_type, cache_control, document): (
        &'static [u8],
        &'static str,
        &'static str,
        bool,
    ) = match path {
        "/" | "/index.html" => (
            asset!("index.html"),
            "text/html; charset=utf-8",
            "no-store",
            true,
        ),
        "/app.js" => (
            asset!("app.js"),
            "text/javascript; charset=utf-8",
            "no-cache",
            false,
        ),
        "/styles.css" => (
            asset!("styles.css"),
            "text/css; charset=utf-8",
            "no-cache",
            false,
        ),
        "/protocol-reading.css" => (
            asset!("protocol-reading.css"),
            "text/css; charset=utf-8",
            "no-cache",
            false,
        ),
        "/branding/gewyvern-mark.svg" => (
            asset!("branding/gewyvern-mark.svg"),
            "image/svg+xml",
            "public, max-age=86400",
            false,
        ),
        "/branding/leserpent-icon.png" => (
            asset!("branding/leserpent-icon.png"),
            "image/png",
            "public, max-age=86400",
            false,
        ),
        _ => return None,
    };
    Some(ConsoleAsset {
        payload,
        content_type,
        cache_control,
        document,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ConsoleApiRoute {
    Capabilities,
    FleetSummary(RuntimeListFilter),
    FleetAttentionSummary(RuntimeListFilter),
    FleetAttentionList(RuntimeListFilter),
    FleetRefreshAll(RuntimeListFilter),
    FleetRefreshCapabilities(RuntimeListFilter),
    FleetRefreshStatus(RuntimeListFilter),
    Runtimes(RuntimeListFilter),
    Sessions,
    PersistenceExport,
    PersistenceImport,
    PersistenceSave,
    OrchestraPlan(RuntimeId),
    OrchestraRuns(RuntimeId),
    OrchestraRunEvents {
        runtime_id: RuntimeId,
        run_id: String,
    },
    OrchestraFleetRuns,
    OrchestraExecute {
        runtime_id: RuntimeId,
        plan_id: String,
    },
    OrchestraCancel {
        runtime_id: RuntimeId,
        run_id: String,
    },
    OrchestraRetry {
        runtime_id: RuntimeId,
        run_id: String,
    },
    OrchestraSession(RuntimeId),
    CleanupPlan(RuntimeListFilter),
    RuntimeCleanup(CleanupKind, RuntimeListFilter),
    RegistrationPlan,
    Registration,
    RuntimeDelete(RuntimeId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CleanupKind {
    Failed,
    Unobserved,
    Slice,
}

impl CleanupKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Failed => "failed",
            Self::Unobserved => "unobserved",
            Self::Slice => "slice",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ConsoleApiMethod {
    Get,
    PostEmpty,
    PostJson,
}

impl ConsoleApiRoute {
    pub(crate) fn method(&self) -> ConsoleApiMethod {
        match self {
            Self::Capabilities
            | Self::FleetSummary(_)
            | Self::FleetAttentionSummary(_)
            | Self::FleetAttentionList(_)
            | Self::Runtimes(_)
            | Self::Sessions
            | Self::PersistenceExport
            | Self::OrchestraPlan(_)
            | Self::OrchestraRuns(_)
            | Self::OrchestraRunEvents { .. }
            | Self::OrchestraFleetRuns
            | Self::CleanupPlan(_) => ConsoleApiMethod::Get,
            Self::FleetRefreshAll(_)
            | Self::FleetRefreshCapabilities(_)
            | Self::FleetRefreshStatus(_)
            | Self::RuntimeDelete(_)
            | Self::PersistenceSave
            | Self::OrchestraCancel { .. } => ConsoleApiMethod::PostEmpty,
            Self::RuntimeCleanup(_, _)
            | Self::RegistrationPlan
            | Self::Registration
            | Self::PersistenceImport
            | Self::OrchestraExecute { .. }
            | Self::OrchestraRetry { .. }
            | Self::OrchestraSession(_) => ConsoleApiMethod::PostJson,
        }
    }

    pub(crate) fn max_json_body_bytes(&self) -> Option<usize> {
        match self {
            Self::RegistrationPlan => Some(MAX_REGISTRATION_PLAN_BYTES),
            Self::Registration => Some(MAX_REGISTRATION_REQUEST_BYTES),
            Self::RuntimeCleanup(_, _) => Some(MAX_CLEANUP_REQUEST_BYTES),
            Self::PersistenceImport => Some(MAX_PROTOCOL_MESSAGE_BYTES),
            Self::OrchestraExecute { .. }
            | Self::OrchestraRetry { .. }
            | Self::OrchestraSession(_) => Some(MAX_ORCHESTRA_COMMAND_BYTES),
            _ => None,
        }
    }

    pub(crate) fn accepted_response(&self) -> bool {
        matches!(
            self,
            Self::OrchestraExecute { .. }
                | Self::OrchestraCancel { .. }
                | Self::OrchestraRetry { .. }
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ConsoleRouteError {
    InvalidTarget,
    InvalidQuery,
}

pub(crate) fn parse_api_route(target: &str) -> Result<Option<ConsoleApiRoute>, ConsoleRouteError> {
    if target.contains('#') || !target.starts_with('/') {
        return Err(ConsoleRouteError::InvalidTarget);
    }
    let (path, query) = target
        .split_once('?')
        .map_or((target, None), |(path, query)| (path, Some(query)));
    if let Some(route) = parse_orchestra_route(path, query)? {
        return Ok(Some(route));
    }
    if let Some(runtime_id) = path
        .strip_prefix("/v1/runtimes/")
        .and_then(|path| path.strip_suffix("/delete"))
    {
        if query.is_some() || runtime_id.is_empty() || runtime_id.contains('/') {
            return Err(ConsoleRouteError::InvalidTarget);
        }
        let runtime_id = decode_path_component(runtime_id)?;
        let runtime_id =
            RuntimeId::new(runtime_id).map_err(|_| ConsoleRouteError::InvalidTarget)?;
        return Ok(Some(ConsoleApiRoute::RuntimeDelete(runtime_id)));
    }
    let filtered = match path {
        "/v1/fleet/summary"
        | "/v1/fleet/attention-summary"
        | "/v1/fleet/runtimes-needing-attention"
        | "/v1/fleet/refresh-all"
        | "/v1/fleet/refresh-capabilities"
        | "/v1/fleet/refresh-status"
        | "/v1/runtimes"
        | "/v1/runtimes/cleanup-plan"
        | "/v1/runtimes/delete-failed"
        | "/v1/runtimes/delete-unobserved"
        | "/v1/runtimes/delete-slice" => Some(parse_filter(query)?),
        "/v1/capabilities"
        | "/v1/sessions"
        | "/v1/persistence/export"
        | "/v1/persistence/import"
        | "/v1/persistence/save"
        | "/v1/runtimes/registration-plan"
        | "/v1/runtimes/register"
            if query.is_some() =>
        {
            return Err(ConsoleRouteError::InvalidQuery);
        }
        _ => None,
    };
    Ok(match path {
        "/v1/capabilities" => Some(ConsoleApiRoute::Capabilities),
        "/v1/fleet/summary" => Some(ConsoleApiRoute::FleetSummary(
            filtered.expect("filtered route has a filter"),
        )),
        "/v1/fleet/attention-summary" => Some(ConsoleApiRoute::FleetAttentionSummary(
            filtered.expect("filtered route has a filter"),
        )),
        "/v1/fleet/runtimes-needing-attention" => Some(ConsoleApiRoute::FleetAttentionList(
            filtered.expect("filtered route has a filter"),
        )),
        "/v1/fleet/refresh-all" => Some(ConsoleApiRoute::FleetRefreshAll(
            filtered.expect("filtered route has a filter"),
        )),
        "/v1/fleet/refresh-capabilities" => Some(ConsoleApiRoute::FleetRefreshCapabilities(
            filtered.expect("filtered route has a filter"),
        )),
        "/v1/fleet/refresh-status" => Some(ConsoleApiRoute::FleetRefreshStatus(
            filtered.expect("filtered route has a filter"),
        )),
        "/v1/runtimes" => Some(ConsoleApiRoute::Runtimes(
            filtered.expect("filtered route has a filter"),
        )),
        "/v1/sessions" => Some(ConsoleApiRoute::Sessions),
        "/v1/persistence/export" => Some(ConsoleApiRoute::PersistenceExport),
        "/v1/persistence/import" => Some(ConsoleApiRoute::PersistenceImport),
        "/v1/persistence/save" => Some(ConsoleApiRoute::PersistenceSave),
        "/v1/runtimes/cleanup-plan" => Some(ConsoleApiRoute::CleanupPlan(
            filtered.expect("filtered route has a filter"),
        )),
        "/v1/runtimes/delete-failed" => Some(ConsoleApiRoute::RuntimeCleanup(
            CleanupKind::Failed,
            filtered.expect("filtered route has a filter"),
        )),
        "/v1/runtimes/delete-unobserved" => Some(ConsoleApiRoute::RuntimeCleanup(
            CleanupKind::Unobserved,
            filtered.expect("filtered route has a filter"),
        )),
        "/v1/runtimes/delete-slice" => Some(ConsoleApiRoute::RuntimeCleanup(
            CleanupKind::Slice,
            filtered.expect("filtered route has a filter"),
        )),
        "/v1/runtimes/registration-plan" => Some(ConsoleApiRoute::RegistrationPlan),
        "/v1/runtimes/register" => Some(ConsoleApiRoute::Registration),
        _ => None,
    })
}

fn parse_orchestra_route(
    path: &str,
    query: Option<&str>,
) -> Result<Option<ConsoleApiRoute>, ConsoleRouteError> {
    let Some(path) = path.strip_prefix("/v1/orchestra/") else {
        return Ok(None);
    };
    if query.is_some() {
        return Err(ConsoleRouteError::InvalidQuery);
    }
    let segments = path.split('/').collect::<Vec<_>>();
    if segments.iter().any(|segment| segment.is_empty()) {
        return Err(ConsoleRouteError::InvalidTarget);
    }
    let route = match segments.as_slice() {
        ["runs"] => ConsoleApiRoute::OrchestraFleetRuns,
        ["plans", runtime_id] => ConsoleApiRoute::OrchestraPlan(parse_runtime_path_id(runtime_id)?),
        ["plans", runtime_id, "session"] => {
            ConsoleApiRoute::OrchestraSession(parse_runtime_path_id(runtime_id)?)
        }
        ["plans", runtime_id, plan_id, "execute"] => ConsoleApiRoute::OrchestraExecute {
            runtime_id: parse_runtime_path_id(runtime_id)?,
            plan_id: parse_orchestra_path_id(plan_id)?,
        },
        ["runtimes", runtime_id, "runs"] => {
            ConsoleApiRoute::OrchestraRuns(parse_runtime_path_id(runtime_id)?)
        }
        ["runtimes", runtime_id, "runs", run_id, "events"] => ConsoleApiRoute::OrchestraRunEvents {
            runtime_id: parse_runtime_path_id(runtime_id)?,
            run_id: parse_orchestra_path_id(run_id)?,
        },
        ["runtimes", runtime_id, "runs", run_id, "cancel"] => ConsoleApiRoute::OrchestraCancel {
            runtime_id: parse_runtime_path_id(runtime_id)?,
            run_id: parse_orchestra_path_id(run_id)?,
        },
        ["runtimes", runtime_id, "runs", run_id, "retry"] => ConsoleApiRoute::OrchestraRetry {
            runtime_id: parse_runtime_path_id(runtime_id)?,
            run_id: parse_orchestra_path_id(run_id)?,
        },
        _ => return Ok(None),
    };
    Ok(Some(route))
}

fn parse_runtime_path_id(value: &str) -> Result<RuntimeId, ConsoleRouteError> {
    RuntimeId::new(decode_path_component(value)?).map_err(|_| ConsoleRouteError::InvalidTarget)
}

fn parse_orchestra_path_id(value: &str) -> Result<String, ConsoleRouteError> {
    let value = decode_path_component(value)?;
    if value.is_empty()
        || value.len() > MAX_FILTER_VALUE_BYTES
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':' | b'.'))
    {
        return Err(ConsoleRouteError::InvalidTarget);
    }
    Ok(value)
}

fn decode_path_component(value: &str) -> Result<String, ConsoleRouteError> {
    if value.len() > MAX_FILTER_VALUE_BYTES * 3 {
        return Err(ConsoleRouteError::InvalidTarget);
    }
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut cursor = 0;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'%' if cursor + 2 < bytes.len() => {
                let high = hex(bytes[cursor + 1]).ok_or(ConsoleRouteError::InvalidTarget)?;
                let low = hex(bytes[cursor + 2]).ok_or(ConsoleRouteError::InvalidTarget)?;
                decoded.push((high << 4) | low);
                cursor += 3;
            }
            b'%' => return Err(ConsoleRouteError::InvalidTarget),
            byte => {
                decoded.push(byte);
                cursor += 1;
            }
        }
        if decoded.len() > MAX_FILTER_VALUE_BYTES {
            return Err(ConsoleRouteError::InvalidTarget);
        }
    }
    let decoded = String::from_utf8(decoded).map_err(|_| ConsoleRouteError::InvalidTarget)?;
    if decoded.chars().any(char::is_control) {
        return Err(ConsoleRouteError::InvalidTarget);
    }
    Ok(decoded)
}

fn parse_filter(query: Option<&str>) -> Result<RuntimeListFilter, ConsoleRouteError> {
    let Some(query) = query else {
        return Ok(RuntimeListFilter::default());
    };
    if query.is_empty() {
        return Err(ConsoleRouteError::InvalidQuery);
    }
    let mut filter = RuntimeListFilter::default();
    for pair in query.split('&') {
        let (name, value) = pair
            .split_once('=')
            .ok_or(ConsoleRouteError::InvalidQuery)?;
        let value = decode_query_component(value)?;
        let slot = match name {
            "environment" => &mut filter.environment,
            "cluster" => &mut filter.cluster,
            "role" => &mut filter.role,
            _ => return Err(ConsoleRouteError::InvalidQuery),
        };
        if slot.is_some() {
            return Err(ConsoleRouteError::InvalidQuery);
        }
        *slot = (!value.trim().is_empty()).then(|| value.trim().to_string());
    }
    Ok(filter)
}

fn decode_query_component(value: &str) -> Result<String, ConsoleRouteError> {
    if value.len() > MAX_FILTER_VALUE_BYTES * 3 {
        return Err(ConsoleRouteError::InvalidQuery);
    }
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut cursor = 0;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'+' => {
                decoded.push(b' ');
                cursor += 1;
            }
            b'%' if cursor + 2 < bytes.len() => {
                let high = hex(bytes[cursor + 1]).ok_or(ConsoleRouteError::InvalidQuery)?;
                let low = hex(bytes[cursor + 2]).ok_or(ConsoleRouteError::InvalidQuery)?;
                decoded.push((high << 4) | low);
                cursor += 3;
            }
            b'%' => return Err(ConsoleRouteError::InvalidQuery),
            byte => {
                decoded.push(byte);
                cursor += 1;
            }
        }
        if decoded.len() > MAX_FILTER_VALUE_BYTES {
            return Err(ConsoleRouteError::InvalidQuery);
        }
    }
    let decoded = String::from_utf8(decoded).map_err(|_| ConsoleRouteError::InvalidQuery)?;
    if decoded.chars().any(char::is_control) {
        return Err(ConsoleRouteError::InvalidQuery);
    }
    Ok(decoded)
}

fn hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
pub(crate) fn render_api(
    route: &ConsoleApiRoute,
    runtime: &mut ControlRuntime,
    writer_enabled: bool,
) -> Result<Vec<u8>, ConsoleWriteError> {
    render_api_with_registration(route, runtime, writer_enabled, false)
}

pub(crate) fn render_api_with_registration(
    route: &ConsoleApiRoute,
    runtime: &mut ControlRuntime,
    writer_enabled: bool,
    registration_enabled: bool,
) -> Result<Vec<u8>, ConsoleWriteError> {
    let (_, all_runtimes) = runtime.runtime_event_state();
    let value = match route {
        ConsoleApiRoute::Capabilities => capabilities_value(
            runtime.persistence_enabled(),
            runtime
                .latest_snapshot_created_at_unix_ms()
                .map_err(|_| "Rust Web persistence status projection failed")?,
            writer_enabled,
            registration_enabled,
        ),
        ConsoleApiRoute::Sessions => json!({ "sessions": [] }),
        ConsoleApiRoute::PersistenceExport => {
            persistence_export_value(runtime).map_err(|error| {
                if error.contains("registration recovery") {
                    ConsoleWriteError::new(
                        ConsoleWriteStatus::Conflict,
                        "persistence_export_not_quiescent",
                        "Rust Web persistence export is blocked by registration recovery",
                    )
                } else {
                    ConsoleWriteError::projection_failed()
                }
            })?
        }
        ConsoleApiRoute::OrchestraPlan(runtime_id) => {
            web_console_orchestra::plan_value(runtime, runtime_id)?
        }
        ConsoleApiRoute::OrchestraRuns(runtime_id) => {
            web_console_orchestra::runtime_runs_value(runtime, runtime_id)?
        }
        ConsoleApiRoute::OrchestraRunEvents { runtime_id, run_id } => {
            web_console_orchestra::run_events_value(runtime, runtime_id, run_id)?
        }
        ConsoleApiRoute::OrchestraFleetRuns => web_console_orchestra::fleet_runs_value(runtime)?,
        ConsoleApiRoute::FleetSummary(filter) => {
            let runtimes = filtered_runtimes(&all_runtimes, filter);
            json!({
                "filter": filter_value(filter),
                "summary": fleet_summary_value(&runtimes),
            })
        }
        ConsoleApiRoute::FleetAttentionSummary(filter) => {
            let runtimes = filtered_runtimes(&all_runtimes, filter);
            let attention = runtimes
                .iter()
                .filter_map(|runtime| attention_value(runtime))
                .collect::<Vec<_>>();
            let critical = attention
                .iter()
                .filter(|item| item["severity"] == "critical")
                .count();
            let warning = attention.len().saturating_sub(critical);
            let mut reason_counts = BTreeMap::<String, usize>::new();
            for item in &attention {
                if let Some(reasons) = item["reasons"].as_array() {
                    for reason in reasons.iter().filter_map(Value::as_str) {
                        *reason_counts.entry(reason.to_string()).or_default() += 1;
                    }
                }
            }
            json!({
                "filter": filter_value(filter),
                "summary": {
                    "criticalCount": critical,
                    "warningCount": warning,
                    "reasonCounts": reason_counts,
                },
            })
        }
        ConsoleApiRoute::FleetAttentionList(filter) => {
            let runtimes = filtered_runtimes(&all_runtimes, filter);
            let attention = runtimes
                .iter()
                .filter_map(|runtime| attention_value(runtime))
                .collect::<Vec<_>>();
            json!({ "filter": filter_value(filter), "runtimes": attention })
        }
        ConsoleApiRoute::Runtimes(filter) => {
            let runtimes = filtered_runtimes(&all_runtimes, filter)
                .into_iter()
                .map(runtime_value)
                .collect::<Vec<_>>();
            json!({ "filter": filter_value(filter), "runtimes": runtimes })
        }
        ConsoleApiRoute::CleanupPlan(filter) => cleanup_plan_value(filter, &all_runtimes),
        ConsoleApiRoute::FleetRefreshAll(_)
        | ConsoleApiRoute::FleetRefreshCapabilities(_)
        | ConsoleApiRoute::FleetRefreshStatus(_)
        | ConsoleApiRoute::RuntimeCleanup(_, _)
        | ConsoleApiRoute::RegistrationPlan
        | ConsoleApiRoute::Registration
        | ConsoleApiRoute::PersistenceImport
        | ConsoleApiRoute::PersistenceSave
        | ConsoleApiRoute::OrchestraExecute { .. }
        | ConsoleApiRoute::OrchestraCancel { .. }
        | ConsoleApiRoute::OrchestraRetry { .. }
        | ConsoleApiRoute::OrchestraSession(_)
        | ConsoleApiRoute::RuntimeDelete(_) => {
            return Err(ConsoleWriteError::projection_failed());
        }
    };
    let bytes = serde_json::to_vec(&value).map_err(|_| ConsoleWriteError::projection_failed())?;
    if bytes.len() > MAX_PROTOCOL_MESSAGE_BYTES {
        return Err(ConsoleWriteError::new(
            ConsoleWriteStatus::InternalServerError,
            "web_response_too_large",
            "Rust Web response exceeded the protocol limit",
        ));
    }
    Ok(bytes)
}

fn capabilities_value(
    persistence_enabled: bool,
    last_saved_at_unix_ms: Option<u64>,
    writer_enabled: bool,
    registration_enabled: bool,
) -> Value {
    let registration_available = writer_enabled && persistence_enabled && registration_enabled;
    json!({
        "service": "leserpentd",
        "version": env!("CARGO_PKG_VERSION"),
        "role": if writer_enabled { "rust-web-native-writer" } else { "rust-web-read-only" },
        "routes": [
            "/v1/capabilities",
            "/v1/fleet/summary",
            "/v1/fleet/attention-summary",
            "/v1/fleet/runtimes-needing-attention",
            "/v1/runtimes",
            "/v1/sessions",
            "/v1/persistence/save",
            "/v1/persistence/export",
            "/v1/persistence/import",
            "/v1/orchestra/plans/{id}",
            "/v1/orchestra/plans/{id}/{planId}/execute",
            "/v1/orchestra/runtimes/{id}/runs",
            "/v1/orchestra/runtimes/{id}/runs/{runId}/events",
            "/v1/orchestra/runs",
            "/v1/orchestra/runtimes/{id}/runs/{runId}/cancel",
            "/v1/orchestra/runtimes/{id}/runs/{runId}/retry",
            "/v1/orchestra/plans/{id}/session",
            "/v1/runtimes/cleanup-plan",
            "/v1/runtimes/delete-failed",
            "/v1/runtimes/delete-unobserved",
            "/v1/runtimes/delete-slice",
            "/v1/runtimes/registration-plan",
            "/v1/runtimes/register",
            "/v1/fleet/refresh-all",
            "/v1/fleet/refresh-capabilities",
            "/v1/fleet/refresh-status",
            "/v1/runtimes/{id}/delete",
            "/v1/wire",
            "/v1/events",
            "/v1/leselang-export"
        ],
        "persistence": {
            "statePath": null,
            "backupStatePath": null,
            "lastSavedAt": last_saved_at_unix_ms.map(|value| timestamp(Some(value))),
            "enabled": persistence_enabled,
            "schemaVersion": 1,
            "isDirty": false,
            "lastSaveError": null,
            "restoredRuntimeCount": 0,
            "restoredSessionCount": 0,
            "restoredFromSavedAt": null
        },
        "security": {
            "apiMode": "bearer_only",
            "adminTokenConfigured": true,
            "publicEndpointDiscoveryAllowed": false
        },
        "webConsole": {
            "writerMode": if writer_enabled { "daemon_owned" } else { "disabled" },
            "mutationAvailable": writer_enabled && persistence_enabled,
            "cleanupAvailable": writer_enabled && persistence_enabled,
            "cleanupAtomicTargetLimit": MAX_ATOMIC_CLEANUP_TARGETS,
            "registrationAvailable": registration_available,
            "orchestraAvailable": persistence_enabled,
            "orchestraMutationAvailable": writer_enabled && persistence_enabled,
            "orchestraSessionHandoffAvailable": false,
            "registrationBlocker": if registration_available {
                Value::Null
            } else {
                Value::String("crash_recoverable_registration_transaction".into())
            }
        },
        "runtimePosture": {
            "coreReady": true,
            "persistenceReady": persistence_enabled,
            "degradedButOperable": !persistence_enabled,
            "optionalAdapters": []
        }
    })
}

fn persistence_export_value(runtime: &mut ControlRuntime) -> Result<Value, String> {
    if runtime.persistence_enabled()
        && !runtime
            .pending_runtime_target_registrations()
            .map_err(|_| "Rust Web persistence export could not inspect registration recovery")?
            .is_empty()
    {
        return Err("Rust Web persistence export is blocked by registration recovery".into());
    }
    let (_, runtimes) = runtime.runtime_event_state();
    let orchestra_runs = exported_orchestra_runs(runtime)?;
    Ok(json!({
        "schemaVersion": PERSISTENCE_EXPORT_SCHEMA_VERSION,
        "savedAt": OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .unwrap_or_else(|_| FALLBACK_TIMESTAMP.to_string()),
        "runtimes": runtimes
            .iter()
            .map(persistence_runtime_value)
            .collect::<Vec<_>>(),
        "sessions": [],
        "orchestraRuns": orchestra_runs,
        "pendingRuntimeDeletions": [],
        "runtimeDeletionRetryAudit": [],
        "runtimeDeletionReconciliationAudit": [],
        "orchestraDeleteCheckpointMonitor": Value::Null,
        "orchestraDeleteCheckpointAlertOutbox": [],
        "pendingRuntimeRegistrations": [],
    }))
}

fn exported_orchestra_runs(runtime: &mut ControlRuntime) -> Result<Vec<Value>, String> {
    if !runtime.persistence_enabled() {
        return Ok(Vec::new());
    }
    let mut offset = 0_u32;
    let mut runs = Vec::new();
    loop {
        let page = runtime
            .load_orchestra_history(None, None, offset, 64)
            .map_err(|_| "Rust Web persistence export could not read Orchestra history")?;
        for envelope in page.runs {
            let run = serde_json::from_slice(&envelope)
                .map_err(|_| "Rust Web persistence export found invalid Orchestra history")?;
            runs.push(run);
            if runs.len() > MAX_EXPORTED_ORCHESTRA_RUNS {
                return Err(
                    "Rust Web persistence export exceeded Orchestra retention bounds".into(),
                );
            }
        }
        let Some(next_offset) = page.next_offset else {
            return Ok(runs);
        };
        if next_offset <= offset {
            return Err("Rust Web persistence export pagination did not advance".into());
        }
        offset = next_offset;
    }
}

fn filtered_runtimes<'a>(
    runtimes: &'a [RuntimeProjection],
    filter: &RuntimeListFilter,
) -> Vec<&'a RuntimeProjection> {
    runtimes
        .iter()
        .filter(|runtime| {
            tag_matches(
                runtime.tags.environment.as_deref(),
                filter.environment.as_deref(),
            ) && tag_matches(runtime.tags.cluster.as_deref(), filter.cluster.as_deref())
                && tag_matches(runtime.tags.role.as_deref(), filter.role.as_deref())
        })
        .collect()
}

fn tag_matches(actual: Option<&str>, expected: Option<&str>) -> bool {
    expected
        .is_none_or(|expected| actual.is_some_and(|actual| actual.eq_ignore_ascii_case(expected)))
}

fn filter_value(filter: &RuntimeListFilter) -> Value {
    json!({
        "environment": filter.environment,
        "cluster": filter.cluster,
        "role": filter.role,
    })
}

pub(crate) fn runtime_value(runtime: &RuntimeProjection) -> Value {
    json!({
        "runtimeId": runtime.id.as_str(),
        "name": runtime.name,
        "endpoint": runtime.endpoint,
        "sidecarEndpoint": runtime.sidecar_endpoint,
        "hasSidecarAdminToken": false,
        "registeredAt": timestamp(runtime.registered_at_unix_ms),
        "updatedAt": timestamp(runtime.updated_at_unix_ms),
        "capabilities": capability_values(&runtime.capabilities),
        "capabilitySource": runtime.capabilities.source,
        "capabilityFetchedAt": Value::Null,
        "capabilityFetchError": Value::Null,
        "tags": tags_value(runtime),
        "status": status_value(&runtime.status),
        "sidecarStatus": runtime.sidecar_status.as_ref().map(sidecar_status_value),
        "hasRuntimeAdminToken": false,
    })
}

fn persistence_runtime_value(runtime: &RuntimeProjection) -> Value {
    json!({
        "runtimeId": runtime.id.as_str(),
        "name": runtime.name,
        "endpoint": runtime.endpoint,
        "sidecarEndpoint": runtime.sidecar_endpoint,
        "registeredAt": timestamp(runtime.registered_at_unix_ms),
        "updatedAt": timestamp(runtime.updated_at_unix_ms),
        "capabilities": capability_values(&runtime.capabilities),
        "capabilitySource": "manual",
        "capabilityFetchedAt": Value::Null,
        "capabilityFetchError": Value::Null,
        "tags": tags_value(runtime),
        "status": status_value(&runtime.status),
        "sidecarStatus": runtime
            .sidecar_status
            .as_ref()
            .map(persistence_sidecar_status_value),
    })
}

fn timestamp(unix_ms: Option<u64>) -> String {
    unix_ms
        .and_then(|unix_ms| {
            OffsetDateTime::from_unix_timestamp_nanos(i128::from(unix_ms) * 1_000_000).ok()
        })
        .and_then(|timestamp| timestamp.format(&Rfc3339).ok())
        .unwrap_or_else(|| FALLBACK_TIMESTAMP.to_string())
}

fn tags_value(runtime: &RuntimeProjection) -> Value {
    json!({
        "environment": runtime.tags.environment,
        "cluster": runtime.tags.cluster,
        "role": runtime.tags.role,
    })
}

fn status_value(status: &RuntimeStatusSnapshot) -> Value {
    json!({
        "statusSource": status.status_source,
        "statusFetchedAt": status.status_fetched_at,
        "statusFetchError": status.status_fetch_error,
        "hasLatestSnapshot": status.has_latest_snapshot,
        "snapshotKind": status.snapshot_kind,
        "targetCount": status.target_count,
        "hasSummaryJson": status.has_summary_json,
        "hasAnalysisJson": status.has_analysis_json,
        "hasTrainingExampleJson": status.has_training_example_json,
        "hasTrainingDatasetManifest": status.has_training_dataset_manifest,
        "hasExportJson": status.has_export_json,
        "hasReportJson": status.has_report_json,
        "hasReportHtml": status.has_report_html,
        "hasExternalSidecarContext": status.has_external_sidecar_context,
        "hasExternalEvidenceChainEnrichment": status.has_external_evidence_chain_enrichment,
        "hasExternalDiagnosticOpinion": status.has_external_diagnostic_opinion,
        "resilienceDegraded": status.resilience_degraded,
        "resilienceStatus": status.resilience_status,
        "resilienceSummary": status.resilience_summary,
        "socketServiceStatus": status.socket_service_status,
        "socketConsecutiveIdleTimeouts": status.socket_consecutive_idle_timeouts,
        "socketTotalIdleTimeouts": status.socket_total_idle_timeouts,
    })
}

fn sidecar_status_value(status: &RuntimeSidecarStatusSnapshot) -> Value {
    json!({
        "statusSource": status.status_source,
        "statusFetchedAt": status.status_fetched_at,
        "statusFetchError": status.status_fetch_error,
        "healthy": status.healthy,
        "daemonStatus": status.daemon_status,
        "targetCount": status.target_count,
        "learningActive": status.learning_active,
        "learnedRoutes": status.learned_routes,
        "hasEvidenceChainEnrichment": status.has_evidence_chain_enrichment,
        "hasDiagnosticOpinion": status.has_diagnostic_opinion,
        "lastError": status.last_error,
        "memory": status.memory.as_ref().map(|memory| json!({
            "versionsSupported": memory.versions_supported,
            "slotCount": memory.slot_count,
            "historyCount": memory.history_count,
            "latestSlot": memory.latest_slot,
            "latestLabel": memory.latest_label,
            "latestSource": memory.latest_source,
            "fetchError": memory.fetch_error,
        })),
    })
}

fn persistence_sidecar_status_value(status: &RuntimeSidecarStatusSnapshot) -> Value {
    json!({
        "statusSource": status.status_source,
        "statusFetchedAt": status.status_fetched_at,
        "statusFetchError": status.status_fetch_error,
        "healthy": status.healthy,
        "daemonStatus": status.daemon_status,
        "targetCount": status.target_count,
        "learningActive": status.learning_active,
        "learnedRoutes": status.learned_routes,
        "hasEvidenceChainEnrichment": status.has_evidence_chain_enrichment,
        "hasDiagnosticOpinion": status.has_diagnostic_opinion,
        "lastError": status.last_error,
        "memory": status.memory.as_ref().map(|memory| json!({
            "versionsSupported": memory.versions_supported,
            "slotCount": memory.slot_count,
            "historyCount": memory.history_count,
            "latestSlot": memory.latest_slot,
            "latestLabel": memory.latest_label,
            "latestSource": memory.latest_source,
            "slots": memory.slots.iter().map(|slot| json!({
                "slot": slot.slot,
                "label": slot.label,
                "note": slot.note,
                "source": slot.source,
                "savedAt": slot.saved_at,
                "patternCount": slot.pattern_count,
                "labelCount": slot.label_count,
            })).collect::<Vec<_>>(),
            "fetchError": memory.fetch_error,
        })),
    })
}

fn capability_values(capabilities: &RuntimeCapabilitySnapshot) -> Vec<Value> {
    let mut values = BTreeMap::<String, bool>::new();
    values.insert("latest_snapshot".into(), capabilities.latest_snapshot);
    values.insert(
        "authenticated_deployment".into(),
        capabilities.authenticated_deployment,
    );
    values.insert("serve_required".into(), capabilities.serve_required);
    values.insert(
        "external_sidecar_context".into(),
        capabilities.external_sidecar_context,
    );
    values.extend(
        capabilities
            .extensions
            .iter()
            .map(|(key, supported)| (key.clone(), *supported)),
    );
    values
        .into_iter()
        .map(|(key, supported)| {
            json!({
                "key": key,
                "support": if supported { "fully_supported" } else { "not_supported" },
                "description": "daemon-authoritative capability projection",
            })
        })
        .collect()
}

fn fleet_summary_value(runtimes: &[&RuntimeProjection]) -> Value {
    let mut snapshot_kinds = BTreeMap::<String, usize>::new();
    let mut status_sources = BTreeMap::<String, usize>::new();
    let mut sidecar_status_sources = BTreeMap::<String, usize>::new();
    let mut environments = BTreeMap::<String, usize>::new();
    let mut clusters = BTreeMap::<String, usize>::new();
    let mut roles = BTreeMap::<String, usize>::new();
    for runtime in runtimes {
        increment(&mut snapshot_kinds, runtime.status.snapshot_kind.as_deref());
        increment(&mut status_sources, Some(&runtime.status.status_source));
        increment(
            &mut sidecar_status_sources,
            runtime
                .sidecar_status
                .as_ref()
                .map(|status| status.status_source.as_str()),
        );
        increment(&mut environments, runtime.tags.environment.as_deref());
        increment(&mut clusters, runtime.tags.cluster.as_deref());
        increment(&mut roles, runtime.tags.role.as_deref());
    }
    json!({
        "runtimeCount": runtimes.len(),
        "runtimesWithLatestSnapshot": runtimes.iter().filter(|runtime| runtime.status.has_latest_snapshot).count(),
        "runtimesWithSummaryJson": runtimes.iter().filter(|runtime| runtime.status.has_summary_json).count(),
        "runtimesWithAnalysisJson": runtimes.iter().filter(|runtime| runtime.status.has_analysis_json).count(),
        "runtimesWithExternalSidecarContext": runtimes.iter().filter(|runtime| runtime.status.has_external_sidecar_context).count(),
        "runtimesWithExternalEvidenceChainEnrichment": runtimes.iter().filter(|runtime| runtime.status.has_external_evidence_chain_enrichment).count(),
        "runtimesWithExternalDiagnosticOpinion": runtimes.iter().filter(|runtime| runtime.status.has_external_diagnostic_opinion).count(),
        "runtimesWithObservedStatus": runtimes.iter().filter(|runtime| !runtime.status.status_source.eq_ignore_ascii_case("unobserved")).count(),
        "runtimesWithStatusFetchFailed": runtimes.iter().filter(|runtime| runtime.status.status_source.eq_ignore_ascii_case("fetch_failed")).count(),
        "runtimesWithPairedSidecar": runtimes.iter().filter(|runtime| runtime.sidecar_endpoint.is_some()).count(),
        "runtimesWithHealthySidecar": runtimes.iter().filter(|runtime| runtime.sidecar_status.as_ref().is_some_and(|status| status.healthy)).count(),
        "runtimesWithObservedSidecarStatus": runtimes.iter().filter(|runtime| runtime.sidecar_status.as_ref().is_some_and(|status| !status.status_source.eq_ignore_ascii_case("unobserved"))).count(),
        "runtimesWithSidecarStatusFetchFailed": runtimes.iter().filter(|runtime| runtime.sidecar_status.as_ref().is_some_and(|status| status.status_source.eq_ignore_ascii_case("fetch_failed"))).count(),
        "runtimesWithSidecarEvidenceChainEnrichment": runtimes.iter().filter(|runtime| runtime.sidecar_status.as_ref().is_some_and(|status| status.has_evidence_chain_enrichment)).count(),
        "runtimesWithSidecarDiagnosticOpinion": runtimes.iter().filter(|runtime| runtime.sidecar_status.as_ref().is_some_and(|status| status.has_diagnostic_opinion)).count(),
        "snapshotKindCounts": snapshot_kinds,
        "statusSourceCounts": status_sources,
        "sidecarStatusSourceCounts": sidecar_status_sources,
        "environmentCounts": environments,
        "clusterCounts": clusters,
        "roleCounts": roles,
    })
}

fn increment(counts: &mut BTreeMap<String, usize>, value: Option<&str>) {
    if let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) {
        *counts.entry(value.to_string()).or_default() += 1;
    }
}

fn attention_value(runtime: &RuntimeProjection) -> Option<Value> {
    let status = &runtime.status;
    let idle_ready = status
        .resilience_status
        .as_deref()
        .is_some_and(|value| value.eq_ignore_ascii_case("idle_ready"))
        || status
            .socket_service_status
            .as_deref()
            .is_some_and(|value| value.eq_ignore_ascii_case("idle"));
    let mut reasons = Vec::new();
    if status.status_source.eq_ignore_ascii_case("fetch_failed") {
        reasons.push("status_fetch_failed");
    }
    if runtime
        .sidecar_status
        .as_ref()
        .is_some_and(|sidecar| sidecar.status_source.eq_ignore_ascii_case("fetch_failed"))
    {
        reasons.push("sidecar_status_fetch_failed");
    }
    if !status.has_latest_snapshot && !idle_ready {
        reasons.push("no_latest_snapshot");
    }
    if !status.has_analysis_json && !idle_ready {
        reasons.push("no_analysis_json");
    }
    if reasons.is_empty() {
        return None;
    }
    let severity = if reasons.contains(&"status_fetch_failed") {
        "critical"
    } else {
        "warning"
    };
    Some(json!({
        "runtimeId": runtime.id.as_str(),
        "name": runtime.name,
        "endpoint": runtime.endpoint,
        "tags": tags_value(runtime),
        "status": status_value(status),
        "needsAttention": true,
        "severity": severity,
        "reasons": reasons,
        "suggestedActions": [],
        "recentRecoveryActivities": [],
    }))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ConsoleCleanupTarget {
    pub(crate) runtime_id: RuntimeId,
    pub(crate) name: String,
    pub(crate) revision: Revision,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ConsoleCleanupAction {
    pub(crate) kind: CleanupKind,
    pub(crate) targets: Vec<ConsoleCleanupTarget>,
    pub(crate) plan_token: String,
    pub(crate) challenge: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ConsoleCleanupPlan {
    filter: RuntimeListFilter,
    risk_level: &'static str,
    failed: ConsoleCleanupAction,
    unobserved: ConsoleCleanupAction,
    slice: ConsoleCleanupAction,
}

impl ConsoleCleanupPlan {
    pub(crate) fn action(&self, kind: CleanupKind) -> &ConsoleCleanupAction {
        match kind {
            CleanupKind::Failed => &self.failed,
            CleanupKind::Unobserved => &self.unobserved,
            CleanupKind::Slice => &self.slice,
        }
    }

    fn value(&self) -> Value {
        json!({
            "filter": filter_value(&self.filter),
            "riskLevel": self.risk_level,
            "failed": cleanup_action_value(&self.failed),
            "unobserved": cleanup_action_value(&self.unobserved),
            "slice": cleanup_action_value(&self.slice),
        })
    }
}

pub(crate) fn build_cleanup_plan(
    filter: &RuntimeListFilter,
    all_runtimes: &[RuntimeProjection],
) -> ConsoleCleanupPlan {
    let runtimes = filtered_runtimes(all_runtimes, filter);
    let risk_level = if [
        filter.environment.as_deref(),
        filter.cluster.as_deref(),
        filter.role.as_deref(),
    ]
    .into_iter()
    .flatten()
    .any(|value| {
        value.to_ascii_lowercase().contains("prod") || value.to_ascii_lowercase().contains("live")
    }) {
        "protected"
    } else {
        "normal"
    };
    ConsoleCleanupPlan {
        filter: filter.clone(),
        risk_level,
        failed: build_cleanup_action(CleanupKind::Failed, filter, &runtimes),
        unobserved: build_cleanup_action(CleanupKind::Unobserved, filter, &runtimes),
        slice: build_cleanup_action(CleanupKind::Slice, filter, &runtimes),
    }
}

fn cleanup_plan_value(filter: &RuntimeListFilter, runtimes: &[RuntimeProjection]) -> Value {
    build_cleanup_plan(filter, runtimes).value()
}

fn build_cleanup_action(
    kind: CleanupKind,
    filter: &RuntimeListFilter,
    runtimes: &[&RuntimeProjection],
) -> ConsoleCleanupAction {
    let mut targets = runtimes
        .iter()
        .filter(|runtime| cleanup_matches(kind, runtime))
        .map(|runtime| ConsoleCleanupTarget {
            runtime_id: runtime.id.clone(),
            name: runtime.name.clone(),
            revision: runtime.revision,
        })
        .collect::<Vec<_>>();
    targets.sort_by(|left, right| {
        left.runtime_id
            .as_str()
            .to_ascii_lowercase()
            .cmp(&right.runtime_id.as_str().to_ascii_lowercase())
            .then_with(|| left.runtime_id.cmp(&right.runtime_id))
    });
    let plan_token = cleanup_plan_token(kind, filter, &targets);
    let challenge = (kind == CleanupKind::Slice).then(|| format!("CLEAR {}", targets.len()));
    ConsoleCleanupAction {
        kind,
        targets,
        plan_token,
        challenge,
    }
}

fn cleanup_matches(kind: CleanupKind, runtime: &RuntimeProjection) -> bool {
    match kind {
        CleanupKind::Failed => runtime
            .status
            .status_source
            .eq_ignore_ascii_case("fetch_failed"),
        CleanupKind::Unobserved => {
            runtime
                .status
                .status_source
                .eq_ignore_ascii_case("unobserved")
                && (runtime.status.resilience_degraded
                    || !runtime
                        .status
                        .resilience_status
                        .as_deref()
                        .is_some_and(|status| status.eq_ignore_ascii_case("idle_ready")))
        }
        CleanupKind::Slice => true,
    }
}

fn cleanup_plan_token(
    kind: CleanupKind,
    filter: &RuntimeListFilter,
    targets: &[ConsoleCleanupTarget],
) -> String {
    let mut canonical = vec![
        "runtime-cleanup-plan-v2".to_string(),
        kind.as_str().to_string(),
        filter
            .environment
            .as_deref()
            .unwrap_or_default()
            .trim()
            .to_string(),
        filter
            .cluster
            .as_deref()
            .unwrap_or_default()
            .trim()
            .to_string(),
        filter
            .role
            .as_deref()
            .unwrap_or_default()
            .trim()
            .to_string(),
    ];
    canonical.extend(targets.iter().map(|target| {
        format!(
            "runtime:{}",
            target.runtime_id.as_str().to_ascii_lowercase()
        )
    }));
    sha256_hex(canonical.join("\n").as_bytes())
}

fn cleanup_action_value(action: &ConsoleCleanupAction) -> Value {
    let targets = action
        .targets
        .iter()
        .map(|target| {
            json!({
                "runtimeId": target.runtime_id.as_str(),
                "name": target.name,
            })
        })
        .collect::<Vec<_>>();
    json!({
        "kind": action.kind.as_str(),
        "runtimeCount": action.targets.len(),
        "sessionCount": 0,
        "targets": targets,
        "planToken": action.plan_token,
        "challenge": action.challenge,
    })
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    let digest = digest::digest(&digest::SHA256, bytes);
    let mut encoded = String::with_capacity(digest.as_ref().len() * 2);
    for byte in digest.as_ref() {
        use std::fmt::Write as _;
        write!(&mut encoded, "{byte:02x}").expect("writing to a String cannot fail");
    }
    encoded
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use leserpent_domain::RuntimeId;

    use super::*;

    #[test]
    fn assets_are_exact_and_do_not_fall_back_across_paths() {
        assert!(
            find_asset("/")
                .unwrap()
                .payload
                .starts_with(b"<!DOCTYPE html>")
        );
        assert_eq!(
            find_asset("/app.js").unwrap().content_type,
            "text/javascript; charset=utf-8"
        );
        assert!(
            find_asset("/branding/leserpent-icon.png")
                .unwrap()
                .payload
                .starts_with(b"\x89PNG")
        );
        for path in [
            "//app.js",
            "/app.js?cache=false",
            "/../index.html",
            "/branding/../app.js",
            "/v1/capabilities",
        ] {
            assert!(find_asset(path).is_none(), "unexpected asset route {path}");
        }
    }

    #[test]
    fn compatibility_routes_accept_only_bounded_unique_filters() {
        assert_eq!(
            parse_api_route("/v1/runtimes?environment=prod%2Dcn&cluster=edge+one").unwrap(),
            Some(ConsoleApiRoute::Runtimes(RuntimeListFilter {
                environment: Some("prod-cn".into()),
                cluster: Some("edge one".into()),
                role: None,
            }))
        );
        for target in [
            "/v1/runtimes?",
            "/v1/runtimes?unknown=value",
            "/v1/runtimes?role=a&role=b",
            "/v1/runtimes?role=%",
            "/v1/runtimes?role=%00",
            "/v1/capabilities?verbose=true",
            "/v1/runtimes#fragment",
        ] {
            assert!(
                parse_api_route(target).is_err(),
                "unexpected route {target}"
            );
        }
        assert_eq!(parse_api_route("/v1/not-present").unwrap(), None);
        assert_eq!(
            parse_api_route("/v1/persistence/export").unwrap(),
            Some(ConsoleApiRoute::PersistenceExport)
        );
        assert_eq!(
            parse_api_route("/v1/persistence/save").unwrap(),
            Some(ConsoleApiRoute::PersistenceSave)
        );
        assert_eq!(
            parse_api_route("/v1/persistence/import").unwrap(),
            Some(ConsoleApiRoute::PersistenceImport)
        );
        assert_eq!(
            ConsoleApiRoute::PersistenceExport.method(),
            ConsoleApiMethod::Get
        );
        assert_eq!(
            ConsoleApiRoute::PersistenceSave.method(),
            ConsoleApiMethod::PostEmpty
        );
        assert_eq!(
            ConsoleApiRoute::PersistenceImport.method(),
            ConsoleApiMethod::PostJson
        );
        assert_eq!(
            ConsoleApiRoute::PersistenceImport.max_json_body_bytes(),
            Some(MAX_PROTOCOL_MESSAGE_BYTES)
        );
        assert_eq!(
            parse_api_route("/v1/orchestra/plans/runtime%3Aedge").unwrap(),
            Some(ConsoleApiRoute::OrchestraPlan(
                RuntimeId::new("runtime:edge").unwrap()
            ))
        );
        assert_eq!(
            parse_api_route("/v1/orchestra/plans/runtime-a/runtime_triage/execute").unwrap(),
            Some(ConsoleApiRoute::OrchestraExecute {
                runtime_id: RuntimeId::new("runtime-a").unwrap(),
                plan_id: "runtime_triage".into(),
            })
        );
        assert_eq!(
            parse_api_route("/v1/orchestra/runtimes/runtime-a/runs/orun%3A1/events").unwrap(),
            Some(ConsoleApiRoute::OrchestraRunEvents {
                runtime_id: RuntimeId::new("runtime-a").unwrap(),
                run_id: "orun:1".into(),
            })
        );
        assert_eq!(
            parse_api_route("/v1/orchestra/runtimes/runtime-a/runs/orun-1/cancel")
                .unwrap()
                .unwrap()
                .method(),
            ConsoleApiMethod::PostEmpty
        );
        assert!(
            parse_api_route("/v1/orchestra/runtimes/runtime-a/runs/orun-1/cancel")
                .unwrap()
                .unwrap()
                .accepted_response()
        );
        assert_eq!(
            parse_api_route("/v1/orchestra/runtimes/runtime-a/runs/orun-1/retry")
                .unwrap()
                .unwrap()
                .max_json_body_bytes(),
            Some(MAX_ORCHESTRA_COMMAND_BYTES)
        );
        for target in [
            "/v1/orchestra/plans/runtime-a?verbose=true",
            "/v1/orchestra/plans/runtime%2Fa",
            "/v1/orchestra/runtimes/runtime-a/runs/orun%2F1/events",
            "/v1/orchestra/runtimes//runs",
        ] {
            assert!(
                parse_api_route(target).is_err(),
                "unexpected Orchestra route {target}"
            );
        }
        assert_eq!(
            parse_api_route("/v1/runtimes/delete-failed?role=edge").unwrap(),
            Some(ConsoleApiRoute::RuntimeCleanup(
                CleanupKind::Failed,
                RuntimeListFilter {
                    environment: None,
                    cluster: None,
                    role: Some("edge".into()),
                }
            ))
        );
    }

    #[test]
    fn runtime_projection_is_filtered_camel_case_and_secret_free() {
        let mut runtime = ControlRuntime::default();
        runtime
            .register_runtime(
                RuntimeId::new("runtime-web").unwrap(),
                "Web runtime",
                "https://runtime.invalid",
            )
            .unwrap();
        let route = ConsoleApiRoute::Runtimes(RuntimeListFilter::default());
        let body = render_api(&route, &mut runtime, false).unwrap();
        let value: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["runtimes"][0]["runtimeId"], "runtime-web");
        assert_eq!(value["runtimes"][0]["name"], "Web runtime");
        assert!(value["runtimes"][0].get("runtime_id").is_none());
        let encoded = String::from_utf8(body).unwrap();
        for forbidden in ["pairingToken", "adminToken", "continuation", "secret"] {
            assert!(
                !encoded.contains(forbidden),
                "projection leaked {forbidden}"
            );
        }

        let filtered = ConsoleApiRoute::Runtimes(RuntimeListFilter {
            environment: Some("missing".into()),
            ..RuntimeListFilter::default()
        });
        let value: Value =
            serde_json::from_slice(&render_api(&filtered, &mut runtime, false).unwrap()).unwrap();
        assert!(value["runtimes"].as_array().unwrap().is_empty());

        let cleanup = ConsoleApiRoute::CleanupPlan(RuntimeListFilter::default());
        let value: Value =
            serde_json::from_slice(&render_api(&cleanup, &mut runtime, false).unwrap()).unwrap();
        assert_eq!(value["riskLevel"], "normal");
        assert_eq!(value["failed"]["runtimeCount"], 0);
        assert_eq!(value["unobserved"]["runtimeCount"], 1);
        assert_eq!(
            value["unobserved"]["targets"][0]["runtimeId"],
            "runtime-web"
        );
        assert_eq!(
            value["unobserved"]["planToken"],
            "f7095175a32e7757fb0f7ba036dcb7955e1bd68caecde1d2b7c250788489f095"
        );
        assert_eq!(value["slice"]["challenge"], "CLEAR 1");
        assert_eq!(
            value["slice"]["planToken"],
            "63551f58c0483ef9559dce5a9366f3244578b92d81be41b5078740f882e33c1e"
        );
    }

    #[test]
    fn persistence_export_is_legacy_compatible_bounded_and_secret_free() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "leserpent-web-export-{}-{nonce}.sqlite",
            std::process::id()
        ));
        let mut runtime = ControlRuntime::open(&path).unwrap();
        runtime
            .register_runtime(
                RuntimeId::new("runtime-export").unwrap(),
                "Export runtime",
                "https://runtime.invalid",
            )
            .unwrap();
        let run = br#"{"runId":"orun-export","runtimeId":"runtime-export","planId":"plan-export","outcome":"queued","executedAt":"2026-08-28T00:00:00Z","completedAt":null,"requestId":"request-export"}"#;
        let event = br#"{"eventId":0,"runId":"orun-export","runtimeId":"runtime-export","eventType":"run_queued","fromOutcome":null,"toOutcome":"queued","summary":"queued","recordedAt":"2026-08-28T00:00:00Z"}"#;
        runtime
            .persist_orchestra_run_event_start(
                "orun-export",
                "runtime-export",
                "request-export",
                "run_queued",
                "queued",
                "2026-08-28T00:00:00Z",
                run,
                event,
            )
            .unwrap();
        let body = render_api(&ConsoleApiRoute::PersistenceExport, &mut runtime, false).unwrap();
        let value: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["schemaVersion"], PERSISTENCE_EXPORT_SCHEMA_VERSION);
        assert_eq!(value["runtimes"][0]["runtimeId"], "runtime-export");
        assert_eq!(value["sessions"], json!([]));
        assert_eq!(value["orchestraRuns"][0]["runId"], "orun-export");
        assert_eq!(value["pendingRuntimeRegistrations"], json!([]));
        assert!(
            value["savedAt"]
                .as_str()
                .is_some_and(|value| value.ends_with('Z'))
        );
        let encoded = String::from_utf8(body).unwrap();
        for forbidden in ["pairingToken", "adminToken", "continuation", "secret"] {
            assert!(!encoded.contains(forbidden), "export leaked {forbidden}");
        }
        runtime
            .begin_runtime_target_registration(
                "operation-export-pending",
                &RuntimeId::new("runtime-export").unwrap(),
                "secret-export-pending",
                b"{}",
            )
            .unwrap();
        assert_eq!(
            render_api(&ConsoleApiRoute::PersistenceExport, &mut runtime, false)
                .unwrap_err()
                .code,
            "persistence_export_not_quiescent"
        );
        drop(runtime);
        fs::remove_file(path).unwrap();
    }
}
