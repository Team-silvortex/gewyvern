use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use gewyvern::protocol_profiles::ProtocolSurfaceSummary;
use crate::runtime_events::{
    EVENT_API_CLIENT_ACCEPT_FAILED, EVENT_API_CLIENT_OVERLOAD_REJECTED,
    EVENT_API_LISTENER_BIND_FAILED,
};
use crate::runtime_logging::{log_error_event, log_warn_event};

mod anomaly_flow_view;
mod anomaly_phase_hints;
mod json;
mod persistence;
mod protocol_catalog;
mod runtime_cluster_attention;
mod runtime_capability_digest;
mod runtime_cluster_overview;
mod resilience_status;
mod routing;
mod service;
mod training_manifest;

use self::routing::handle_api_client;
pub use self::service::start_api_service;
pub(crate) use self::training_manifest::training_sample_id;

pub type ApiState = Arc<Mutex<Arc<ApiSnapshot>>>;

const API_CLIENT_READ_TIMEOUT: Duration = Duration::from_secs(3);
const API_CLIENT_WRITE_TIMEOUT: Duration = Duration::from_secs(3);
const API_MAX_CONCURRENT_CLIENTS: usize = 128;
const API_MAX_RESPONSE_BODY_BYTES: usize = 512 * 1024;
const API_VERSION: &str = env!("CARGO_PKG_VERSION");
const API_ENDPOINTS_JSON: &str = "[\"/health\",\"/v1/capabilities\",\"/v1/runtime/resilience.json\",\"/v1/protocols\",\"/v1/protocols/<protocol>\",\"/v1/protocols/<protocol>/entries/<entry>/surface.json\",\"/v1/protocol-clusters\",\"/v1/protocol-clusters/<cluster>\",\"/v1/latest/meta\",\"/v1/latest/runtime-capability-digest.json\",\"/v1/latest/runtime-cluster-overview.json\",\"/v1/latest/runtime-cluster-attention.json\",\"/v1/latest/runtime-cluster-attention-reasons.json\",\"/v1/latest/runtime-cluster-attention-summary.json\",\"/v1/latest/targets\",\"/v1/latest/summary.txt\",\"/v1/latest/summary.json\",\"/v1/latest/findings.json\",\"/v1/latest/analysis.json\",\"/v1/latest/training-example.json\",\"/v1/latest/training-dataset.json\",\"/v1/latest/export.json\",\"/v1/latest/report.json\",\"/v1/latest/report.html\",\"/v1/latest/targets/<name>/summary.txt\",\"/v1/latest/targets/<name>/summary.json\",\"/v1/latest/targets/<name>/findings.json\",\"/v1/latest/targets/<name>/analysis.json\",\"/v1/latest/targets/<name>/anomaly-flow.json\",\"/v1/latest/targets/<name>/training-example.json\",\"/v1/latest/targets/<name>/training-dataset.json\",\"/v1/latest/targets/<name>/export.json\",\"/v1/latest/targets/<name>/report.json\",\"/v1/latest/targets/<name>/report.html\",\"/v1/latest/targets/<name>/protocol-surface.json\"]";

#[derive(Clone, Debug, Default)]
pub struct ApiSnapshot {
    pub updated_unix_ms: u128,
    pub kind: String,
    pub name: Option<String>,
    pub target_count: Option<usize>,
    pub target_names: Vec<String>,
    pub primary_module_family: Option<String>,
    pub evidence_posture: Option<String>,
    pub automation_outcome: Option<String>,
    pub summary_text: Option<String>,
    pub summary_json: Option<String>,
    pub findings_json: Option<String>,
    pub analysis_json: Option<String>,
    pub training_example_json: Option<String>,
    pub has_external_sidecar_context: bool,
    pub has_external_evidence_chain_enrichment: bool,
    pub has_external_diagnostic_opinion: bool,
    pub has_external_capability_profile: bool,
    pub external_capability_status: Option<String>,
    pub external_hint_status: Option<String>,
    pub external_context_status: Option<String>,
    pub external_sidecar_trust_level: Option<String>,
    pub external_sidecar_consumption_mode: Option<String>,
    pub export_json: Option<String>,
    pub report_json: Option<String>,
    pub report_html: Option<String>,
    pub target_snapshots: HashMap<String, ApiTargetSnapshot>,
}

#[derive(Clone, Debug, Default)]
pub struct ApiTargetSnapshot {
    pub primary_module_family: Option<String>,
    pub evidence_posture: Option<String>,
    pub automation_outcome: Option<String>,
    pub summary_text: String,
    pub summary_json: String,
    pub findings_json: String,
    pub analysis_json: String,
    pub training_example_json: String,
    pub protocol_surface_json: Option<String>,
    pub protocol_surface: Option<ProtocolSurfaceSummary>,
    pub has_external_sidecar_context: bool,
    pub has_external_evidence_chain_enrichment: bool,
    pub has_external_diagnostic_opinion: bool,
    pub has_external_capability_profile: bool,
    pub external_capability_status: Option<String>,
    pub external_hint_status: Option<String>,
    pub external_context_status: Option<String>,
    pub external_sidecar_trust_level: Option<String>,
    pub external_sidecar_consumption_mode: Option<String>,
    pub export_json: String,
    pub report_json: String,
    pub report_html: String,
}

#[derive(Clone, Debug)]
pub struct ApiRenderedTarget {
    pub name: String,
    pub primary_module_family: String,
    pub evidence_posture: String,
    pub automation_outcome: String,
    pub summary_text: String,
    pub summary_json: String,
    pub findings_json: String,
    pub analysis_json: String,
    pub training_example_json: String,
    pub has_external_sidecar_context: bool,
    pub has_external_evidence_chain_enrichment: bool,
    pub has_external_diagnostic_opinion: bool,
    pub has_external_capability_profile: bool,
    pub external_capability_status: Option<String>,
    pub external_hint_status: Option<String>,
    pub external_context_status: Option<String>,
    pub external_sidecar_trust_level: Option<String>,
    pub external_sidecar_consumption_mode: Option<String>,
    pub export_json: String,
    pub report_json: String,
    pub report_html: String,
}

impl ApiRenderedTarget {
    pub fn into_snapshot(self) -> ApiTargetSnapshot {
        let protocol_surface = protocol_catalog::api_protocol_surface_for_target(&self.name);
        let protocol_surface_json = protocol_surface
            .as_ref()
            .map(protocol_catalog::api_protocol_surface_json);
        ApiTargetSnapshot {
            primary_module_family: Some(self.primary_module_family),
            evidence_posture: Some(self.evidence_posture),
            automation_outcome: Some(self.automation_outcome),
            summary_text: self.summary_text,
            summary_json: self.summary_json,
            findings_json: self.findings_json,
            analysis_json: self.analysis_json,
            training_example_json: self.training_example_json,
            protocol_surface_json,
            protocol_surface,
            has_external_sidecar_context: self.has_external_sidecar_context,
            has_external_evidence_chain_enrichment: self.has_external_evidence_chain_enrichment,
            has_external_diagnostic_opinion: self.has_external_diagnostic_opinion,
            has_external_capability_profile: self.has_external_capability_profile,
            external_capability_status: self.external_capability_status,
            external_hint_status: self.external_hint_status,
            external_context_status: self.external_context_status,
            external_sidecar_trust_level: self.external_sidecar_trust_level,
            external_sidecar_consumption_mode: self.external_sidecar_consumption_mode,
            export_json: self.export_json,
            report_json: self.report_json,
            report_html: self.report_html,
        }
    }
}

#[cfg(test)]
pub(crate) fn api_snapshot_meta_json(snapshot: &ApiSnapshot) -> String {
    json::api_snapshot_meta_json(snapshot)
}

#[cfg(test)]
pub(crate) fn api_response_for_request<'a>(
    path: &str,
    snapshot: &'a ApiSnapshot,
) -> (u16, &'static str, std::borrow::Cow<'a, str>) {
    routing::api_response_for_request(path, snapshot)
}

pub(crate) fn persist_api_snapshot(state: &ApiState) -> Result<(), String> {
    let snapshot = {
        let guard = state.lock().expect("api snapshot mutex poisoned");
        guard.clone()
    };
    persistence::persist_latest_snapshot(&snapshot)
}

pub fn update_api_snapshot_for_single(state: &ApiState, rendered: ApiRenderedTarget) {
    let target_name = rendered.name.clone();
    let has_external_sidecar_context = rendered.has_external_sidecar_context;
    let has_external_evidence_chain_enrichment = rendered.has_external_evidence_chain_enrichment;
    let has_external_diagnostic_opinion = rendered.has_external_diagnostic_opinion;
    let has_external_capability_profile = rendered.has_external_capability_profile;
    let external_capability_status = rendered.external_capability_status.clone();
    let external_hint_status = rendered.external_hint_status.clone();
    let external_context_status = rendered.external_context_status.clone();
    let external_sidecar_trust_level = rendered.external_sidecar_trust_level.clone();
    let external_sidecar_consumption_mode = rendered.external_sidecar_consumption_mode.clone();
    let target_snapshot = rendered.clone().into_snapshot();
    let mut target_snapshots = HashMap::new();
    target_snapshots.insert(target_name.clone(), target_snapshot);
    let mut guard = state.lock().expect("api snapshot mutex poisoned");
    *guard = Arc::new(ApiSnapshot {
        updated_unix_ms: current_unix_ms(),
        kind: "single".into(),
        name: Some(target_name.clone()),
        target_count: Some(1),
        target_names: vec![target_name],
        primary_module_family: Some(rendered.primary_module_family),
        evidence_posture: Some(rendered.evidence_posture),
        automation_outcome: Some(rendered.automation_outcome),
        summary_text: Some(rendered.summary_text),
        summary_json: Some(rendered.summary_json),
        findings_json: Some(rendered.findings_json),
        analysis_json: Some(rendered.analysis_json),
        training_example_json: Some(rendered.training_example_json),
        has_external_sidecar_context,
        has_external_evidence_chain_enrichment,
        has_external_diagnostic_opinion,
        has_external_capability_profile,
        external_capability_status,
        external_hint_status,
        external_context_status,
        external_sidecar_trust_level,
        external_sidecar_consumption_mode,
        export_json: Some(rendered.export_json),
        report_json: Some(rendered.report_json),
        report_html: Some(rendered.report_html),
        target_snapshots,
    });
}

pub fn update_api_snapshot_for_scan(
    state: &ApiState,
    targets: Vec<ApiRenderedTarget>,
    summary_text: String,
    summary_json: String,
    analysis_json: String,
    training_example_json: String,
    report_json: String,
    report_html: String,
) {
    let rollup = scan_rollup_profile(&targets);
    let mut target_snapshots = HashMap::new();
    let mut target_names = Vec::with_capacity(targets.len());
    let mut has_external_sidecar_context = false;
    let mut has_external_evidence_chain_enrichment = false;
    let mut has_external_diagnostic_opinion = false;
    let mut has_external_capability_profile = false;
    let mut external_capability_status = None;
    let mut external_hint_status = None;
    let mut external_context_status = None;
    let mut external_sidecar_trust_level = None;
    let mut external_sidecar_consumption_mode = None;
    for rendered in targets {
        has_external_sidecar_context |= rendered.has_external_sidecar_context;
        has_external_evidence_chain_enrichment |= rendered.has_external_evidence_chain_enrichment;
        has_external_diagnostic_opinion |= rendered.has_external_diagnostic_opinion;
        has_external_capability_profile |= rendered.has_external_capability_profile;
        if external_capability_status.is_none() {
            external_capability_status = rendered.external_capability_status.clone();
        }
        if external_hint_status.is_none() {
            external_hint_status = rendered.external_hint_status.clone();
        }
        if external_context_status.is_none() {
            external_context_status = rendered.external_context_status.clone();
        }
        if external_sidecar_trust_level.is_none() {
            external_sidecar_trust_level = rendered.external_sidecar_trust_level.clone();
        }
        if external_sidecar_consumption_mode.is_none() {
            external_sidecar_consumption_mode = rendered.external_sidecar_consumption_mode.clone();
        }
        target_names.push(rendered.name.clone());
        target_snapshots.insert(rendered.name.clone(), rendered.into_snapshot());
    }
    let mut guard = state.lock().expect("api snapshot mutex poisoned");
    *guard = Arc::new(ApiSnapshot {
        updated_unix_ms: current_unix_ms(),
        kind: "scan".into(),
        name: None,
        target_count: Some(target_names.len()),
        target_names,
        primary_module_family: rollup.primary_module_family,
        evidence_posture: rollup.evidence_posture,
        automation_outcome: rollup.automation_outcome,
        summary_text: Some(summary_text),
        summary_json: Some(summary_json),
        analysis_json: Some(analysis_json),
        training_example_json: Some(training_example_json),
        has_external_sidecar_context,
        has_external_evidence_chain_enrichment,
        has_external_diagnostic_opinion,
        has_external_capability_profile,
        external_capability_status,
        external_hint_status,
        external_context_status,
        external_sidecar_trust_level,
        external_sidecar_consumption_mode,
        findings_json: None,
        export_json: None,
        report_json: Some(report_json),
        report_html: Some(report_html),
        target_snapshots,
    });
}

#[derive(Default)]
struct ScanRollupProfile {
    primary_module_family: Option<String>,
    evidence_posture: Option<String>,
    automation_outcome: Option<String>,
}

fn scan_rollup_profile(targets: &[ApiRenderedTarget]) -> ScanRollupProfile {
    let automation_outcome = scan_rollup_bucket(targets, |target| &target.automation_outcome);
    let evidence_posture = scan_rollup_bucket(targets, |target| &target.evidence_posture);
    let focus_rank = targets
        .iter()
        .map(|target| automation_outcome_rank(&target.automation_outcome))
        .min();
    let primary_module_family = focus_rank.and_then(|rank| {
        let focus_targets = targets
            .iter()
            .filter(|target| automation_outcome_rank(&target.automation_outcome) == rank)
            .collect::<Vec<_>>();
        scan_rollup_bucket_from_refs(&focus_targets, |target| &target.primary_module_family)
            .or_else(|| scan_rollup_bucket(targets, |target| &target.primary_module_family))
    });
    ScanRollupProfile {
        primary_module_family,
        evidence_posture,
        automation_outcome,
    }
}

fn scan_rollup_bucket(
    targets: &[ApiRenderedTarget],
    value: impl Fn(&ApiRenderedTarget) -> &str,
) -> Option<String> {
    let refs = targets.iter().collect::<Vec<_>>();
    scan_rollup_bucket_from_refs(&refs, value)
}

fn scan_rollup_bucket_from_refs<'a>(
    targets: &[&'a ApiRenderedTarget],
    value: impl Fn(&ApiRenderedTarget) -> &str,
) -> Option<String> {
    let mut counts = HashMap::<&str, usize>::new();
    for target in targets {
        *counts.entry(value(target)).or_insert(0) += 1;
    }
    counts
        .into_iter()
        .max_by(|(left, left_count), (right, right_count)| {
            left_count
                .cmp(right_count)
                .then_with(|| {
                    rollup_bucket_rank(left)
                        .cmp(&rollup_bucket_rank(right))
                        .reverse()
                })
                .then_with(|| right.cmp(left))
        })
        .map(|(value, _)| value.to_string())
}

fn rollup_bucket_rank(value: &str) -> usize {
    automation_outcome_priority(value)
        .or_else(|| evidence_posture_priority(value))
        .unwrap_or(usize::MAX / 2)
}

fn automation_outcome_rank(value: &str) -> usize {
    automation_outcome_priority(value).unwrap_or(usize::MAX)
}

fn automation_outcome_priority(value: &str) -> Option<usize> {
    match value {
        "targeted_escalation" => Some(0),
        "collect_more_evidence" => Some(1),
        "multi_hypothesis" => Some(2),
        "manual_review" => Some(3),
        "advisory_only" => Some(4),
        _ => None,
    }
}

fn evidence_posture_priority(value: &str) -> Option<usize> {
    match value {
        "direct_protocol_signal" => Some(0),
        "missing_transition" => Some(1),
        "ambiguous_multi_hypothesis" => Some(2),
        "heuristic_summary" => Some(3),
        "unverified_ingest" => Some(4),
        _ => None,
    }
}

fn current_unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_rollup_profile_prefers_attention_first_spine() {
        let targets = vec![
            ApiRenderedTarget {
                name: "target-a".into(),
                primary_module_family: "database".into(),
                evidence_posture: "heuristic_summary".into(),
                automation_outcome: "manual_review".into(),
                summary_text: String::new(),
                summary_json: String::new(),
                findings_json: String::new(),
                analysis_json: String::new(),
                training_example_json: String::new(),
                has_external_sidecar_context: false,
                has_external_evidence_chain_enrichment: false,
                has_external_diagnostic_opinion: false,
                has_external_capability_profile: false,
                external_capability_status: None,
                external_hint_status: None,
                external_context_status: None,
                external_sidecar_trust_level: None,
                external_sidecar_consumption_mode: None,
                export_json: String::new(),
                report_json: String::new(),
                report_html: String::new(),
            },
            ApiRenderedTarget {
                name: "target-b".into(),
                primary_module_family: "request-response".into(),
                evidence_posture: "missing_transition".into(),
                automation_outcome: "collect_more_evidence".into(),
                summary_text: String::new(),
                summary_json: String::new(),
                findings_json: String::new(),
                analysis_json: String::new(),
                training_example_json: String::new(),
                has_external_sidecar_context: false,
                has_external_evidence_chain_enrichment: false,
                has_external_diagnostic_opinion: false,
                has_external_capability_profile: false,
                external_capability_status: None,
                external_hint_status: None,
                external_context_status: None,
                external_sidecar_trust_level: None,
                external_sidecar_consumption_mode: None,
                export_json: String::new(),
                report_json: String::new(),
                report_html: String::new(),
            },
            ApiRenderedTarget {
                name: "target-c".into(),
                primary_module_family: "request-response".into(),
                evidence_posture: "direct_protocol_signal".into(),
                automation_outcome: "targeted_escalation".into(),
                summary_text: String::new(),
                summary_json: String::new(),
                findings_json: String::new(),
                analysis_json: String::new(),
                training_example_json: String::new(),
                has_external_sidecar_context: false,
                has_external_evidence_chain_enrichment: false,
                has_external_diagnostic_opinion: false,
                has_external_capability_profile: false,
                external_capability_status: None,
                external_hint_status: None,
                external_context_status: None,
                external_sidecar_trust_level: None,
                external_sidecar_consumption_mode: None,
                export_json: String::new(),
                report_json: String::new(),
                report_html: String::new(),
            },
        ];

        let rollup = scan_rollup_profile(&targets);
        assert_eq!(
            rollup.primary_module_family.as_deref(),
            Some("request-response")
        );
        assert_eq!(
            rollup.evidence_posture.as_deref(),
            Some("direct_protocol_signal")
        );
        assert_eq!(
            rollup.automation_outcome.as_deref(),
            Some("targeted_escalation")
        );
    }
}
