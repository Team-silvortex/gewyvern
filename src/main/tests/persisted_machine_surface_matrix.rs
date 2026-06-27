use super::*;
use crate::data_api::persist_api_snapshot;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

struct EnvGuard {
    key: &'static str,
    previous: Option<String>,
}

impl EnvGuard {
    fn set(key: &'static str, value: impl Into<String>) -> Self {
        let previous = std::env::var(key).ok();
        unsafe {
            std::env::set_var(key, value.into());
        }
        Self { key, previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        match &self.previous {
            Some(value) => unsafe {
                std::env::set_var(self.key, value);
            },
            None => unsafe {
                std::env::remove_var(self.key);
            },
        }
    }
}

fn temp_dir(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "gewyvern-persisted-surface-matrix-{label}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

fn demo_persisted_export() -> ExportBundle {
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy")
        .expect("http_request_path DSL should compile");
    annotate_export_trust(
        run_binding_demo(binding),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
    )
}

fn persisted_single_snapshot(state_home: &PathBuf, target_name: &str) -> (ApiSnapshot, PathBuf) {
    let _guard = test_guard();
    set_external_analysis_config(None);
    let export = demo_persisted_export();
    let analysis = analysis_snapshot(&export);
    let state = Arc::new(Mutex::new(Arc::new(ApiSnapshot::default())));

    update_api_snapshot_for_single(
        &state,
        ApiRenderedTarget {
            name: target_name.into(),
            primary_module_family: analysis.primary_module_family.clone(),
            evidence_posture: analysis.evidence_posture.clone(),
            automation_outcome: analysis.automation_outcome.clone(),
            summary_text: summary_line(target_name, &export),
            summary_json: summary_json(target_name, &export),
            findings_json: findings_json(target_name, &export),
            analysis_json: analysis_snapshot_json(&analysis),
            training_example_json: training_example_json_with_analysis(
                target_name,
                &export,
                &analysis,
            ),
            has_external_sidecar_context: false,
            has_external_evidence_chain_enrichment: false,
            has_external_diagnostic_opinion: false,
            has_external_capability_profile: false,
            external_capability_status: None,
            external_hint_status: None,
            external_context_status: None,
            external_sidecar_trust_level: None,
            external_sidecar_consumption_mode: None,
            export_json: export.to_json(),
            report_json: scan_report_json(&[(target_name.to_string(), export.clone())]),
            report_html: scan_report_html(&[(target_name.to_string(), export.clone())]),
        },
    );
    persist_api_snapshot(&state).unwrap();

    let latest_root = state_home
        .join("latest")
        .join("api")
        .join("v1")
        .join("latest");
    (state.lock().unwrap().as_ref().clone(), latest_root)
}

#[test]
fn persisted_machine_surfaces_match_live_api_for_core_payloads() {
    let _lock = env_test_lock()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let root = temp_dir("core");
    let state_root = root.join("state");
    let _state = EnvGuard::set("GEWY_STATE_HOME", state_root.to_string_lossy());

    let target_name = "dsl_demo";
    let (snapshot, latest_root) = persisted_single_snapshot(&state_root, target_name);

    let (_, _, live_summary) = api_response_for_request("/v1/latest/summary.json", &snapshot);
    let (_, _, live_analysis) = api_response_for_request("/v1/latest/analysis.json", &snapshot);
    let (_, _, live_training) =
        api_response_for_request("/v1/latest/training-example.json", &snapshot);
    let (_, _, live_dataset) =
        api_response_for_request("/v1/latest/training-dataset.json", &snapshot);
    let (_, _, live_export) = api_response_for_request("/v1/latest/export.json", &snapshot);

    let persisted_summary = fs::read_to_string(latest_root.join("summary.json")).unwrap();
    let persisted_analysis = fs::read_to_string(latest_root.join("analysis.json")).unwrap();
    let persisted_training = fs::read_to_string(latest_root.join("training-example.json")).unwrap();
    let persisted_dataset = fs::read_to_string(latest_root.join("training-dataset.json")).unwrap();
    let persisted_export = fs::read_to_string(latest_root.join("export.json")).unwrap();

    assert_eq!(live_summary.as_ref(), persisted_summary);
    assert_eq!(live_analysis.as_ref(), persisted_analysis);
    assert_eq!(live_training.as_ref(), persisted_training);
    assert_eq!(live_dataset.as_ref(), persisted_dataset);
    assert_eq!(live_export.as_ref(), persisted_export);

    let target_root = latest_root.join("targets").join(target_name);
    assert_eq!(
        live_summary.as_ref(),
        fs::read_to_string(target_root.join("summary.json")).unwrap()
    );
    assert_eq!(
        live_analysis.as_ref(),
        fs::read_to_string(target_root.join("analysis.json")).unwrap()
    );
    assert_eq!(
        live_training.as_ref(),
        fs::read_to_string(target_root.join("training-example.json")).unwrap()
    );
    assert_eq!(
        api_response_for_request(
            "/v1/latest/targets/dsl_demo/training-dataset.json",
            &snapshot,
        )
        .2
        .as_ref(),
        fs::read_to_string(target_root.join("training-dataset.json")).unwrap()
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn persisted_machine_surfaces_match_live_api_for_runtime_and_capability_routes() {
    let _lock = env_test_lock()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let root = temp_dir("runtime");
    let state_root = root.join("state");
    let _state = EnvGuard::set("GEWY_STATE_HOME", state_root.to_string_lossy());

    let (snapshot, latest_root) = persisted_single_snapshot(&state_root, "dsl_demo");

    let (_, _, live_targets) = api_response_for_request("/v1/latest/targets", &snapshot);
    let (_, _, live_meta) = api_response_for_request("/v1/latest/meta", &snapshot);
    let (_, _, live_digest) =
        api_response_for_request("/v1/latest/runtime-capability-digest.json", &snapshot);
    let (_, _, live_certs) = api_response_for_request("/v1/runtime/certificates.json", &snapshot);
    let (_, _, live_policy) =
        api_response_for_request("/v1/runtime/certificate-policy.json", &snapshot);
    let (_, _, live_state) =
        api_response_for_request("/v1/runtime/certificate-state.json", &snapshot);

    assert_eq!(
        live_targets.as_ref(),
        fs::read_to_string(latest_root.join("targets.json")).unwrap()
    );
    assert_eq!(
        live_meta.as_ref(),
        fs::read_to_string(latest_root.join("meta.json")).unwrap()
    );
    assert_eq!(
        live_digest.as_ref(),
        fs::read_to_string(latest_root.join("runtime-capability-digest.json")).unwrap()
    );
    assert_eq!(
        live_certs.as_ref(),
        fs::read_to_string(latest_root.join("runtime-certificates.json")).unwrap()
    );
    assert_eq!(
        live_policy.as_ref(),
        fs::read_to_string(latest_root.join("runtime-certificate-policy.json")).unwrap()
    );
    assert_eq!(
        live_state.as_ref(),
        fs::read_to_string(latest_root.join("runtime-certificate-state.json")).unwrap()
    );

    assert!(live_targets.contains("\"targets\":[\"dsl_demo\"]"));
    assert!(live_digest.contains("\"surface\":\"runtime_capability_digest\""));
    assert!(live_certs.contains("\"surface\":\"runtime_certificates\""));
    assert!(live_policy.contains("\"surface\":\"runtime_certificate_policy\""));
    assert!(live_state.contains("\"surface\":\"runtime_certificate_state\""));

    fs::remove_dir_all(&root).unwrap();
}
