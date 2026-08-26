use std::collections::BTreeMap;

use leserpent_domain::{
    RuntimeCapabilitySnapshot, RuntimeListFilter, RuntimeProjection, RuntimeSidecarStatusSnapshot,
    RuntimeStatusSnapshot,
};
use leserpent_protocol::MAX_PROTOCOL_MESSAGE_BYTES;
use leserpent_runtime::ControlRuntime;
use serde_json::{Value, json};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

const MAX_FILTER_VALUE_BYTES: usize = 128;
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
    Runtimes(RuntimeListFilter),
    Sessions,
    CleanupPlan(RuntimeListFilter),
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
    let filtered = match path {
        "/v1/fleet/summary"
        | "/v1/fleet/attention-summary"
        | "/v1/fleet/runtimes-needing-attention"
        | "/v1/runtimes"
        | "/v1/runtimes/cleanup-plan" => Some(parse_filter(query)?),
        "/v1/capabilities" | "/v1/sessions" if query.is_some() => {
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
        "/v1/runtimes" => Some(ConsoleApiRoute::Runtimes(
            filtered.expect("filtered route has a filter"),
        )),
        "/v1/sessions" => Some(ConsoleApiRoute::Sessions),
        "/v1/runtimes/cleanup-plan" => Some(ConsoleApiRoute::CleanupPlan(
            filtered.expect("filtered route has a filter"),
        )),
        _ => None,
    })
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

pub(crate) fn render_api(
    route: &ConsoleApiRoute,
    runtime: &ControlRuntime,
) -> Result<Vec<u8>, String> {
    let (_, all_runtimes) = runtime.runtime_event_state();
    let value = match route {
        ConsoleApiRoute::Capabilities => capabilities_value(runtime.persistence_enabled()),
        ConsoleApiRoute::Sessions => json!({ "sessions": [] }),
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
        ConsoleApiRoute::CleanupPlan(filter) => cleanup_plan_value(filter),
    };
    let bytes = serde_json::to_vec(&value).map_err(|error| error.to_string())?;
    if bytes.len() > MAX_PROTOCOL_MESSAGE_BYTES {
        return Err("Rust Web compatibility projection exceeds the protocol response limit".into());
    }
    Ok(bytes)
}

fn capabilities_value(persistence_enabled: bool) -> Value {
    json!({
        "service": "leserpentd",
        "version": env!("CARGO_PKG_VERSION"),
        "role": "rust-web-read-only",
        "routes": [
            "/v1/capabilities",
            "/v1/fleet/summary",
            "/v1/fleet/attention-summary",
            "/v1/fleet/runtimes-needing-attention",
            "/v1/runtimes",
            "/v1/sessions",
            "/v1/runtimes/cleanup-plan",
            "/v1/wire",
            "/v1/events",
            "/v1/leselang-export"
        ],
        "persistence": {
            "statePath": null,
            "backupStatePath": null,
            "lastSavedAt": null,
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
        "runtimePosture": {
            "coreReady": true,
            "persistenceReady": persistence_enabled,
            "degradedButOperable": !persistence_enabled,
            "optionalAdapters": []
        }
    })
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

fn runtime_value(runtime: &RuntimeProjection) -> Value {
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
                "support": if supported { "fully_supported" } else { "unsupported" },
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

fn cleanup_plan_value(filter: &RuntimeListFilter) -> Value {
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
    let action = |kind: &str| {
        json!({
            "kind": kind,
            "runtimeCount": 0,
            "sessionCount": 0,
            "targets": [],
            "planToken": "rust-web-read-only",
            "challenge": Value::Null,
        })
    };
    json!({
        "filter": filter_value(filter),
        "riskLevel": risk_level,
        "failed": action("failed"),
        "unobserved": action("unobserved"),
        "slice": action("slice"),
    })
}

#[cfg(test)]
mod tests {
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
        let body = render_api(&route, &runtime).unwrap();
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
            serde_json::from_slice(&render_api(&filtered, &runtime).unwrap()).unwrap();
        assert!(value["runtimes"].as_array().unwrap().is_empty());
    }
}
