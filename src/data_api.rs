use std::collections::HashMap;
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use gewyvern::protocol_profiles::{ProtocolSurfaceSummary, protocol_surface};

use crate::render_utils::{append_json_string, append_string_list_json};

mod json;
mod routing;
mod training_manifest;

use self::routing::handle_api_client;
pub(crate) use self::training_manifest::training_sample_id;

pub type ApiState = Arc<Mutex<Arc<ApiSnapshot>>>;

const API_CLIENT_READ_TIMEOUT: Duration = Duration::from_secs(3);
const API_VERSION: &str = env!("CARGO_PKG_VERSION");
const API_ENDPOINTS_JSON: &str = "[\"/health\",\"/v1/capabilities\",\"/v1/latest/meta\",\"/v1/latest/targets\",\"/v1/latest/summary.txt\",\"/v1/latest/summary.json\",\"/v1/latest/findings.json\",\"/v1/latest/analysis.json\",\"/v1/latest/training-example.json\",\"/v1/latest/training-dataset.json\",\"/v1/latest/export.json\",\"/v1/latest/report.json\",\"/v1/latest/report.html\",\"/v1/latest/targets/<name>/summary.txt\",\"/v1/latest/targets/<name>/summary.json\",\"/v1/latest/targets/<name>/findings.json\",\"/v1/latest/targets/<name>/analysis.json\",\"/v1/latest/targets/<name>/training-example.json\",\"/v1/latest/targets/<name>/training-dataset.json\",\"/v1/latest/targets/<name>/export.json\",\"/v1/latest/targets/<name>/report.json\",\"/v1/latest/targets/<name>/report.html\",\"/v1/latest/targets/<name>/protocol-surface.json\"]";

#[derive(Clone, Debug, Default)]
pub struct ApiSnapshot {
    pub updated_unix_ms: u128,
    pub kind: String,
    pub name: Option<String>,
    pub target_count: Option<usize>,
    pub target_names: Vec<String>,
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
        let protocol_surface = api_protocol_surface_for_target(&self.name);
        let protocol_surface_json = protocol_surface.as_ref().map(api_protocol_surface_json);
        ApiTargetSnapshot {
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

pub fn start_api_service(addr: &str) -> ApiState {
    let listener = TcpListener::bind(addr).unwrap_or_else(|err| {
        eprintln!("failed to bind api socket {}: {}", addr, err);
        std::process::exit(1);
    });
    let state = Arc::new(Mutex::new(Arc::new(ApiSnapshot::default())));
    let thread_state = Arc::clone(&state);
    thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let client_state = Arc::clone(&thread_state);
                    thread::spawn(move || handle_api_client(stream, client_state));
                }
                Err(_) => continue,
            }
        }
    });
    state
}

fn current_unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn api_protocol_surface_for_target(name: &str) -> Option<ProtocolSurfaceSummary> {
    let mut parts = name.splitn(3, ':');
    if parts.next()? != "scan" {
        return None;
    }
    let protocol = parts.next()?;
    let entry = parts.next()?;
    protocol_surface(protocol, entry)
}

fn api_protocol_surface_json(surface: &ProtocolSurfaceSummary) -> String {
    let mut json = String::from("{\"protocol\":");
    append_json_string(&mut json, &surface.protocol);
    json.push_str(",\"entry\":");
    append_json_string(&mut json, &surface.entry);
    json.push_str(",\"default_entry\":");
    append_json_string(&mut json, &surface.default_entry);
    json.push_str(",\"selected_is_default\":");
    json.push_str(if surface.selected_is_default {
        "true"
    } else {
        "false"
    });
    json.push_str(",\"protocol_aliases\":");
    append_string_list_json(&mut json, &surface.protocol_aliases);
    json.push_str(",\"entry_aliases\":");
    append_string_list_json(&mut json, &surface.entry_aliases);
    json.push_str(",\"sibling_entries\":");
    append_string_list_json(&mut json, &surface.sibling_entries);
    json.push_str(",\"shelf\":");
    if let Some(shelf) = surface.shelf.as_ref() {
        json.push('{');
        json.push_str("\"key\":");
        append_json_string(&mut json, &shelf.key);
        json.push_str(",\"label\":");
        append_json_string(&mut json, &shelf.label);
        json.push_str(",\"page\":");
        append_json_string(&mut json, &shelf.page);
        json.push_str(",\"entries\":");
        append_string_list_json(&mut json, &shelf.entries);
        json.push('}');
    } else {
        json.push_str("null");
    }
    json.push('}');
    json
}
