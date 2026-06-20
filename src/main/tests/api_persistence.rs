use super::*;
use crate::data_api::{persist_api_snapshot, training_sample_id};
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

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

    fn remove(key: &'static str) -> Self {
        let previous = std::env::var(key).ok();
        unsafe {
            std::env::remove_var(key);
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
        "gewyvern-api-persistence-{label}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

#[test]
fn persisted_latest_snapshot_writes_top_level_and_target_surfaces() {
    let _lock = env_lock()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let root = temp_dir("latest");
    let state_root = root.join("state");
    let _state = EnvGuard::set("GEWY_STATE_HOME", state_root.to_string_lossy());
    let _history_retention = EnvGuard::remove("GEWY_HISTORY_RETENTION");

    let _guard = test_guard();
    set_external_analysis_config(None);
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy")
        .expect("http_request_path DSL should compile");
    let export = annotate_export_trust(
        run_binding_demo(binding),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
    );
    let state = Arc::new(Mutex::new(Arc::new(ApiSnapshot::default())));
    let target_name = "scan:http:request";
    let analysis = analysis_snapshot(&export);
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
            training_example_json: training_example_json(target_name, &export),
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

    let latest_root = state_root
        .join("latest")
        .join("api")
        .join("v1")
        .join("latest");
    let target_root = latest_root.join("targets").join("scan:http:request");
    let meta = fs::read_to_string(latest_root.join("meta.json")).unwrap();
    let targets = fs::read_to_string(latest_root.join("targets.json")).unwrap();
    let dataset = fs::read_to_string(latest_root.join("training-dataset.json")).unwrap();
    let protocol_catalog = fs::read_to_string(latest_root.join("protocols.json")).unwrap();
    let protocol_clusters =
        fs::read_to_string(latest_root.join("protocol-clusters.json")).unwrap();
    let runtime_capability_digest =
        fs::read_to_string(latest_root.join("runtime-capability-digest.json")).unwrap();
    let runtime_cluster_overview =
        fs::read_to_string(latest_root.join("runtime-cluster-overview.json")).unwrap();
    let runtime_cluster_attention =
        fs::read_to_string(latest_root.join("runtime-cluster-attention.json")).unwrap();
    let runtime_cluster_attention_reasons = fs::read_to_string(
        latest_root.join("runtime-cluster-attention-reasons.json"),
    )
    .unwrap();
    let runtime_cluster_attention_summary = fs::read_to_string(
        latest_root.join("runtime-cluster-attention-summary.json"),
    )
    .unwrap();
    let protocol_delta = fs::read_to_string(latest_root.join("protocol-delta.json")).unwrap();
    let protocol_evolution =
        fs::read_to_string(latest_root.join("protocol-evolution.md")).unwrap();
    let protocol_surface = fs::read_to_string(target_root.join("protocol-surface.json")).unwrap();
    let anomaly_flow = fs::read_to_string(target_root.join("anomaly-flow.json")).unwrap();
    let protocol_summary = fs::read_to_string(
        latest_root.join("protocols").join("http").join("summary.json"),
    )
    .unwrap();
    let entry_surface = fs::read_to_string(
        latest_root
            .join("protocols")
            .join("redis")
            .join("entries")
            .join("zadd")
            .join("surface.json"),
    )
    .unwrap();
    let cluster_surface = fs::read_to_string(
        latest_root
            .join("protocol-clusters")
            .join("cache-queue-stream.json"),
    )
    .unwrap();
    let target_dataset = fs::read_to_string(target_root.join("training-dataset.json")).unwrap();

    assert!(meta.contains("\"kind\":\"single\""));
    assert!(targets.contains("\"targets\":[\"scan:http:request\"]"));
    assert!(targets.contains("\"path_segment\":\"scan:http:request\""));
    assert!(dataset.contains(&training_sample_id(target_name)));
    assert!(protocol_catalog.contains("\"surface\":\"protocol_catalog\""));
    assert!(protocol_clusters.contains("\"surface\":\"protocol_cluster_catalog\""));
    assert!(protocol_clusters.contains("\"key\":\"cache-queue-stream\""));
    assert!(runtime_capability_digest.contains("\"surface\":\"runtime_capability_digest\""));
    assert!(runtime_capability_digest.contains("\"targets_with_protocol_surface\":1"));
    assert!(runtime_cluster_overview.contains("\"surface\":\"runtime_cluster_overview\""));
    assert!(runtime_cluster_overview.contains("\"key\":\"web-proxy-request-response\""));
    assert!(runtime_cluster_attention.contains("\"surface\":\"runtime_cluster_attention\""));
    assert!(runtime_cluster_attention.contains("\"attention_cluster_count\":1"));
    assert!(runtime_cluster_attention_reasons
        .contains("\"surface\":\"runtime_cluster_attention_reasons\""));
    assert!(runtime_cluster_attention_reasons
        .contains("\"key\":\"automation.targeted_escalation\""));
    assert!(runtime_cluster_attention_summary
        .contains("\"surface\":\"runtime_cluster_attention_summary\""));
    assert!(runtime_cluster_attention_summary.contains("\"clusters\":["));
    assert!(protocol_catalog.contains("\"protocol\":\"mysql\""));
    assert_eq!(protocol_delta, "null");
    assert!(protocol_evolution.contains("# Protocol Evolution"));
    assert!(protocol_evolution.contains("No prior protocol catalog snapshot exists yet."));
    assert!(protocol_surface.contains("\"protocol\":\"http\""));
    assert!(protocol_surface.contains("\"reading_companions\":[{\"protocol\":\"dns\",\"entry\":\"tcp\",\"via_overlay\":\"doh\""));
    assert!(anomaly_flow.contains("\"surface\":\"anomaly_flow_view\""));
    assert!(protocol_summary.contains("\"protocol\":\"http\""));
    assert!(entry_surface.contains("\"protocol\":\"redis\""));
    assert!(entry_surface.contains("\"entry\":\"zadd\""));
    assert!(entry_surface.contains("\"reading_companions\":[]"));
    assert!(cluster_surface.contains("\"key\":\"cache-queue-stream\""));
    assert!(cluster_surface.contains("\"protocol\":\"redis\""));
    assert!(target_dataset.contains("\"snapshot_kind\":\"target\""));
    assert!(latest_root.join("summary.json").exists());
    assert!(target_root.join("analysis.json").exists());

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn persisted_snapshot_history_keeps_prior_refreshes_while_latest_moves_forward() {
    let _lock = env_lock()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let root = temp_dir("history");
    let state_root = root.join("state");
    let _state = EnvGuard::set("GEWY_STATE_HOME", state_root.to_string_lossy());
    let _history_retention = EnvGuard::remove("GEWY_HISTORY_RETENTION");

    let _guard = test_guard();
    set_external_analysis_config(None);
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy")
        .expect("http_request_path DSL should compile");
    let export = annotate_export_trust(
        run_binding_demo(binding),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
    );
    let state = Arc::new(Mutex::new(Arc::new(ApiSnapshot::default())));

    update_api_snapshot_for_single(
        &state,
        ApiRenderedTarget {
            name: "scan:http:request".into(),
            primary_module_family: analysis_snapshot(&export).primary_module_family,
            evidence_posture: analysis_snapshot(&export).evidence_posture,
            automation_outcome: analysis_snapshot(&export).automation_outcome,
            summary_text: summary_line("scan:http:request", &export),
            summary_json: summary_json("scan:http:request", &export),
            findings_json: findings_json("scan:http:request", &export),
            analysis_json: analysis_snapshot_json(&analysis_snapshot(&export)),
            training_example_json: training_example_json("scan:http:request", &export),
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
            report_json: scan_report_json(&[("scan:http:request".to_string(), export.clone())]),
            report_html: scan_report_html(&[("scan:http:request".to_string(), export.clone())]),
        },
    );
    persist_api_snapshot(&state).unwrap();
    let first_updated = state.lock().unwrap().updated_unix_ms;

    thread::sleep(Duration::from_millis(2));

    update_api_snapshot_for_single(
        &state,
        ApiRenderedTarget {
            name: "scan:http:response".into(),
            primary_module_family: analysis_snapshot(&export).primary_module_family,
            evidence_posture: analysis_snapshot(&export).evidence_posture,
            automation_outcome: analysis_snapshot(&export).automation_outcome,
            summary_text: summary_line("scan:http:response", &export),
            summary_json: summary_json("scan:http:response", &export),
            findings_json: findings_json("scan:http:response", &export),
            analysis_json: analysis_snapshot_json(&analysis_snapshot(&export)),
            training_example_json: training_example_json("scan:http:response", &export),
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
            report_json: scan_report_json(&[("scan:http:response".to_string(), export.clone())]),
            report_html: scan_report_html(&[("scan:http:response".to_string(), export.clone())]),
        },
    );
    persist_api_snapshot(&state).unwrap();
    let second_updated = state.lock().unwrap().updated_unix_ms;
    assert!(second_updated > first_updated);

    let latest_root = state_root
        .join("latest")
        .join("api")
        .join("v1")
        .join("latest");
    let history_root = state_root.join("history").join("api").join("v1");
    let latest_targets = fs::read_to_string(latest_root.join("targets.json")).unwrap();
    let history_index = fs::read_to_string(history_root.join("index.json")).unwrap();
    let latest_protocol_delta =
        fs::read_to_string(latest_root.join("protocol-delta.json")).unwrap();
    let latest_protocol_evolution =
        fs::read_to_string(latest_root.join("protocol-evolution.md")).unwrap();
    let current_history_protocol_delta = fs::read_to_string(
        history_root
            .join(second_updated.to_string())
            .join("protocol-delta.json"),
    )
    .unwrap();
    let current_history_protocol_evolution = fs::read_to_string(
        history_root
            .join(second_updated.to_string())
            .join("protocol-evolution.md"),
    )
    .unwrap();

    assert!(latest_targets.contains("scan:http:response"));
    assert!(
        !latest_root
            .join("targets")
            .join("scan:http:request")
            .exists()
    );
    assert!(
        history_root
            .join(first_updated.to_string())
            .join("protocol-delta.json")
            .exists()
    );
    assert!(
        history_root
            .join(first_updated.to_string())
            .join("protocols.json")
            .exists()
    );
    assert!(
        history_root
            .join(first_updated.to_string())
            .join("protocol-clusters.json")
            .exists()
    );
    assert!(
        history_root
            .join(second_updated.to_string())
            .join("protocol-clusters")
            .join("cache-queue-stream.json")
            .exists()
    );
    assert!(
        history_root
            .join(second_updated.to_string())
            .join("protocols")
            .join("http")
            .join("summary.json")
            .exists()
    );
    assert!(
        history_root
            .join(second_updated.to_string())
            .join("protocols")
            .join("redis")
            .join("entries")
            .join("zadd")
            .join("surface.json")
            .exists()
    );
    assert!(
        history_root
            .join(first_updated.to_string())
            .join("targets")
            .join("scan:http:request")
            .join("summary.json")
            .exists()
    );
    assert!(
        history_root
            .join(second_updated.to_string())
            .join("targets")
            .join("scan:http:response")
            .join("summary.json")
            .exists()
    );
    assert!(history_index.contains(&first_updated.to_string()));
    assert!(history_index.contains(&second_updated.to_string()));
    assert!(history_index.contains("\"protocol_catalog_path\":"));
    assert!(history_index.contains("\"protocol_root_path\":"));
    assert!(history_index.contains("\"protocol_delta_path\":"));
    assert!(history_index.contains("\"protocol_evolution_path\":"));
    assert!(history_index.contains("\"latest_protocol_catalog_delta\":{"));
    assert!(history_index.contains("\"status\":\"unchanged\""));
    assert!(history_index.contains("\"latest_protocol_catalog_delta_path\":"));
    assert!(latest_protocol_delta.contains("\"status\":\"unchanged\""));
    assert_eq!(latest_protocol_delta, current_history_protocol_delta);
    assert!(latest_protocol_evolution.contains("# Protocol Evolution"));
    assert!(latest_protocol_evolution.contains("No protocol catalog changes detected"));
    assert_eq!(latest_protocol_evolution, current_history_protocol_evolution);

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn persisted_snapshot_history_prunes_older_entries_beyond_retention_limit() {
    let _lock = env_lock()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let root = temp_dir("history-prune");
    let state_root = root.join("state");
    let history_root = state_root.join("history").join("api").join("v1");
    let _state = EnvGuard::set("GEWY_STATE_HOME", state_root.to_string_lossy());
    let _history_retention = EnvGuard::remove("GEWY_HISTORY_RETENTION");
    fs::create_dir_all(&history_root).unwrap();

    for value in 1u128..=40 {
        let entry_root = history_root.join(value.to_string());
        fs::create_dir_all(entry_root.join("targets")).unwrap();
        fs::write(
            entry_root.join("meta.json"),
            format!("{{\"updated_unix_ms\":{value}}}"),
        )
        .unwrap();
        fs::write(entry_root.join("targets.json"), "{\"targets\":[]}").unwrap();
    }

    let _guard = test_guard();
    set_external_analysis_config(None);
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy")
        .expect("http_request_path DSL should compile");
    let export = annotate_export_trust(
        run_binding_demo(binding),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
    );
    let state = Arc::new(Mutex::new(Arc::new(ApiSnapshot::default())));
    update_api_snapshot_for_single(
        &state,
        ApiRenderedTarget {
            name: "scan:http:request".into(),
            primary_module_family: analysis_snapshot(&export).primary_module_family,
            evidence_posture: analysis_snapshot(&export).evidence_posture,
            automation_outcome: analysis_snapshot(&export).automation_outcome,
            summary_text: summary_line("scan:http:request", &export),
            summary_json: summary_json("scan:http:request", &export),
            findings_json: findings_json("scan:http:request", &export),
            analysis_json: analysis_snapshot_json(&analysis_snapshot(&export)),
            training_example_json: training_example_json("scan:http:request", &export),
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
            report_json: scan_report_json(&[("scan:http:request".to_string(), export.clone())]),
            report_html: scan_report_html(&[("scan:http:request".to_string(), export.clone())]),
        },
    );
    persist_api_snapshot(&state).unwrap();

    let history_index = fs::read_to_string(history_root.join("index.json")).unwrap();
    let entry_count = history_index.matches("\"updated_unix_ms\":").count();

    assert_eq!(entry_count, 32);
    assert!(history_index.contains("\"schema_version\":2"));
    assert!(history_index.contains("\"minor_line\":\"v0.15.x\""));
    assert!(history_index.contains("\"history_retention\":32"));
    assert!(history_index.contains("\"lines\":[{\"line\":\"v0.15.x\""));
    assert!(!history_root.join("1").exists());
    assert!(!history_root.join("9").exists());
    assert!(history_root.join("10").exists());
    assert!(history_root.join("40").exists());

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn persisted_snapshot_history_respects_configured_retention_override() {
    let _lock = env_lock()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let root = temp_dir("history-retention-override");
    let state_root = root.join("state");
    let history_root = state_root.join("history").join("api").join("v1");
    let _state = EnvGuard::set("GEWY_STATE_HOME", state_root.to_string_lossy());
    let _history_retention = EnvGuard::set("GEWY_HISTORY_RETENTION", "4");
    fs::create_dir_all(&history_root).unwrap();

    for value in 1u128..=6 {
        let entry_root = history_root.join(value.to_string());
        fs::create_dir_all(entry_root.join("targets")).unwrap();
        fs::write(
            entry_root.join("meta.json"),
            format!("{{\"updated_unix_ms\":{value}}}"),
        )
        .unwrap();
        fs::write(entry_root.join("targets.json"), "{\"targets\":[]}").unwrap();
    }

    let _guard = test_guard();
    set_external_analysis_config(None);
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy")
        .expect("http_request_path DSL should compile");
    let export = annotate_export_trust(
        run_binding_demo(binding),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
    );
    let state = Arc::new(Mutex::new(Arc::new(ApiSnapshot::default())));
    let analysis = analysis_snapshot(&export);
    update_api_snapshot_for_single(
        &state,
        ApiRenderedTarget {
            name: "scan:http:request".into(),
            primary_module_family: analysis.primary_module_family.clone(),
            evidence_posture: analysis.evidence_posture.clone(),
            automation_outcome: analysis.automation_outcome.clone(),
            summary_text: summary_line("scan:http:request", &export),
            summary_json: summary_json("scan:http:request", &export),
            findings_json: findings_json("scan:http:request", &export),
            analysis_json: analysis_snapshot_json(&analysis),
            training_example_json: training_example_json("scan:http:request", &export),
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
            report_json: scan_report_json(&[("scan:http:request".to_string(), export.clone())]),
            report_html: scan_report_html(&[("scan:http:request".to_string(), export.clone())]),
        },
    );
    persist_api_snapshot(&state).unwrap();

    let history_index = fs::read_to_string(history_root.join("index.json")).unwrap();
    let entry_count = history_index.matches("\"updated_unix_ms\":").count();

    assert_eq!(entry_count, 4);
    assert!(history_index.contains("\"schema_version\":2"));
    assert!(history_index.contains("\"minor_line\":\"v0.15.x\""));
    assert!(history_index.contains("\"history_retention\":4"));
    assert!(history_index.contains("\"line\":\"v0.15.x\""));
    assert!(!history_root.join("1").exists());
    assert!(!history_root.join("3").exists());
    assert!(history_root.join("4").exists());
    assert!(history_root.join("6").exists());

    fs::remove_dir_all(&root).unwrap();
}
