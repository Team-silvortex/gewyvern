use super::*;

const ETRAGON_PROTOCOL_FAMILY: &str = "etragon-resident-protocol";
const ETRAGON_PROTOCOL_VERSION: u32 = 1;
const SUPPORTED_INPUT_CONTRACTS: &[&str] = &[
    "gewyvern-analysis-json-v1",
    "gewyvern-target-analysis-json-v1",
];
const SUPPORTED_OUTPUT_CONTRACTS: &[&str] = &[
    "etragon-engine-output-v1",
    "etragon-recommendation-summary-v1",
    "etragon-learning-summary-v1",
    "etragon-handoff-summary-v1",
];
const SUPPORTED_DAEMON_ROUTES: &[&str] = &[
    "/health",
    "/v1/training-labels.json",
    "/v1/memory-state.json",
    "/v1/memory-model.json",
    "/v1/memory-versions.json",
    "/v1/memory-snapshot.json",
    "/v1/protocol-capabilities.json",
    "/v1/latest/status",
    "/v1/latest/meta",
    "/v1/latest/output.json",
    "/v1/latest/recommendation-summary.json",
    "/v1/latest/learning-summary.json",
    "/v1/latest/evidence-chain-enrichment.json",
    "/v1/latest/diagnostic-opinion.json",
    "/v1/latest/handoff-summary.json",
    "/v1/latest/targets",
    "POST /v1/train/latest",
    "POST /v1/train/targets/<path-segment>",
    "POST /v1/memory-admin/save",
    "POST /v1/memory-admin/clear",
    "POST /v1/memory-admin/load",
    "POST /v1/memory-admin/delete",
];
const SUPPORTED_MINOR_RELEASE_SNAPSHOTS: &[&str] = &["0.14.x"];
const SUPPORTED_LATEST_IR_SURFACES: &[&str] = &[
    "recommendation_summary",
    "learning_summary",
    "evidence_chain_enrichment",
    "diagnostic_opinion",
    "handoff_summary",
];
const SUPPORTED_TARGET_IR_SURFACES: &[&str] = &[
    "target_output",
    "target_meta",
    "target_recommendation_summary",
    "target_learning_summary",
    "target_evidence_chain_enrichment",
    "target_diagnostic_opinion",
    "target_handoff_summary",
];
const RESIDENT_MEMORY_ANNOTATIONS: &[&str] = &[
    "pattern_memory_state",
    "pattern_memory_summary",
    "transition_policy_summary",
    "memory_drift_hint",
    "learning_judgement",
    "feedback_policy_hint",
    "recent_training_events",
    "recent_label_activity",
];
const GEWYVERN_MERGE_HINTS: &[&str] = &[
    "augmentations_only",
    "augmentations_and_guidance_context",
    "augmentations_with_operator_guidance_support",
    "sidecar_only_opinion",
    "operator_guidance_candidate",
];
const HANDOFF_READINESS_LEVELS: &[&str] = &["advisory_only", "mergeable", "automation_worthy"];
const HANDOFF_SUMMARY_FIELDS: &[&str] = &[
    "has_evidence_chain_enrichment",
    "has_diagnostic_opinion",
    "handoff_readiness",
    "gewyvern_merge_hint",
    "primary_status",
    "primary_label",
    "summary",
    "enrichment_strength_band",
    "opinion_confidence_band",
];
const HANDOFF_ROUTE_FANOUT: &[&str] = &[
    "/v1/latest/handoff-summary.json",
    "/v1/latest/meta",
    "/v1/latest/targets",
    "/v1/latest/targets/<path-segment>/meta.json",
    "/v1/latest/targets/<path-segment>/handoff-summary.json",
];
const STABLE_LATEST_IR_FIELDS: &[&str] = &[
    "recommendation_summary.top_recommendation",
    "recommendation_summary.top_candidates",
    "learning_summary.learning_active",
    "learning_summary.learned_routes",
    "learning_summary.top_learned_label",
    "learning_summary.evidence_chain_enrichment",
    "learning_summary.diagnostic_opinion",
    "handoff_summary.handoff_readiness",
    "handoff_summary.gewyvern_merge_hint",
];
const EXPERIMENTAL_LATEST_IR_FIELDS: &[&str] = &[
    "learning_summary.queue_pressure_hint",
    "learning_summary.feedback_policy_hint",
    "learning_summary.memory_drift_hint",
    "learning_summary.recent_label_activity",
];
const STABLE_TARGET_IR_FIELDS: &[&str] = &[
    "target_meta.learning_active",
    "target_meta.learned_routes",
    "target_meta.handoff_summary",
    "target_learning_summary.top_learned_label",
    "target_handoff_summary.handoff_readiness",
    "target_handoff_summary.gewyvern_merge_hint",
];
const SAFE_AUTOMATION_MERGE_HINTS: &[&str] =
    &["augmentations_only", "augmentations_and_guidance_context"];
const OPERATOR_REVIEW_MERGE_HINTS: &[&str] = &[
    "augmentations_with_operator_guidance_support",
    "sidecar_only_opinion",
    "operator_guidance_candidate",
];
const FORWARD_COMPATIBILITY_RULES: &[&str] = &[
    "unknown_top_level_fields_must_be_ignored",
    "unknown_ir_fields_must_be_ignored",
    "unknown_merge_hints_must_downgrade_to_operator_review",
    "missing_experimental_fields_must_not_break_consumers",
];

pub(super) fn learning_backend_memory_state_json(
    config: &LearningBackendConfig,
    snapshot: Option<&DaemonSnapshot>,
) -> Result<String, String> {
    let worker_state = with_learning_backend(config, |worker| worker.memory_info_json())?;
    Ok(daemon_memory_state_json(&worker_state, snapshot))
}

pub(super) fn learning_backend_model_info_json(
    config: &LearningBackendConfig,
) -> Result<String, String> {
    with_learning_backend(config, |worker| worker.model_info_json())
}

pub(super) fn learning_backend_memory_snapshot_json(
    config: &LearningBackendConfig,
) -> Result<String, String> {
    with_learning_backend(config, |worker| worker.export_memory_json())
}

pub(super) fn learning_backend_memory_versions_json(
    config: &LearningBackendConfig,
) -> Result<String, String> {
    with_learning_backend(config, |worker| worker.memory_versions_json())
}

pub(super) fn protocol_capabilities_json(config: &PythonWorkerConfig) -> Result<String, String> {
    learning_backend_protocol_capabilities_json(&LearningBackendConfig::Python(config.clone()))
}

pub(super) fn learning_backend_protocol_capabilities_json(
    config: &LearningBackendConfig,
) -> Result<String, String> {
    let worker_model_info = learning_backend_model_info_json(config)?;
    let access_policy = daemon_access_policy_from_env();
    Ok(protocol_capabilities_document_json(
        &worker_model_info,
        &access_policy,
    ))
}

pub(super) fn clear_learning_backend_memory(
    config: &LearningBackendConfig,
    latest: &Arc<Mutex<Option<DaemonSnapshot>>>,
    daemon_state_file: Option<&Path>,
    invalidation_epoch: &Arc<AtomicU64>,
) -> Result<String, String> {
    let worker_state = with_learning_backend(config, |worker| worker.clear_memory_json())?;
    let snapshot_to_persist =
        reset_snapshot_training_state(latest, daemon_state_file, invalidation_epoch)?;
    Ok(daemon_memory_state_json(
        &worker_state,
        snapshot_to_persist.as_ref(),
    ))
}

pub(super) fn load_learning_backend_memory(
    config: &LearningBackendConfig,
    memory_snapshot_json: &str,
    latest: &Arc<Mutex<Option<DaemonSnapshot>>>,
    daemon_state_file: Option<&Path>,
    invalidation_epoch: &Arc<AtomicU64>,
) -> Result<String, String> {
    let worker_state = with_learning_backend(config, |worker| {
        worker.import_memory_json(memory_snapshot_json)
    })?;
    let snapshot_to_persist =
        reset_snapshot_training_state(latest, daemon_state_file, invalidation_epoch)?;
    Ok(daemon_memory_state_json(
        &worker_state,
        snapshot_to_persist.as_ref(),
    ))
}

pub(super) fn save_learning_backend_memory_slot(
    config: &LearningBackendConfig,
    slot: &str,
    label: Option<&str>,
    note: Option<&str>,
    source: Option<&str>,
) -> Result<String, String> {
    with_learning_backend(config, |worker| {
        worker.save_memory_slot_json(slot, label, note, source)
    })
}

pub(super) fn load_learning_backend_memory_slot(
    config: &LearningBackendConfig,
    slot: &str,
    strategy: &str,
    latest: &Arc<Mutex<Option<DaemonSnapshot>>>,
    daemon_state_file: Option<&Path>,
    invalidation_epoch: &Arc<AtomicU64>,
) -> Result<String, String> {
    let worker_state = with_learning_backend(config, |worker| {
        worker.load_memory_slot_json(slot, strategy)
    })?;
    let snapshot_to_persist =
        reset_snapshot_training_state(latest, daemon_state_file, invalidation_epoch)?;
    Ok(daemon_memory_state_json(
        &worker_state,
        snapshot_to_persist.as_ref(),
    ))
}

pub(super) fn delete_learning_backend_memory_slot(
    config: &LearningBackendConfig,
    slot: &str,
) -> Result<String, String> {
    with_learning_backend(config, |worker| worker.delete_memory_slot_json(slot))
}

fn reset_snapshot_training_state(
    latest: &Arc<Mutex<Option<DaemonSnapshot>>>,
    daemon_state_file: Option<&Path>,
    invalidation_epoch: &Arc<AtomicU64>,
) -> Result<Option<DaemonSnapshot>, String> {
    let mut snapshot_to_persist = None;
    if let Ok(mut guard) = latest.lock()
        && let Some(snapshot) = guard.as_mut()
    {
        snapshot.training_history.clear();
        for target in &mut snapshot.target_outputs {
            target.training_history.clear();
        }
        snapshot_to_persist = Some(snapshot.clone());
    }
    if let (Some(path), Some(snapshot)) = (daemon_state_file, snapshot_to_persist.as_ref()) {
        write_daemon_state(path, snapshot)?;
    }
    invalidation_epoch.fetch_add(1, Ordering::Relaxed);
    Ok(snapshot_to_persist)
}

fn protocol_capabilities_document_json(
    worker_model_info_json: &str,
    access_policy: &DaemonAccessPolicy,
) -> String {
    let training_labels = training_label_specs()
        .iter()
        .map(|spec| format!("\"{}\"", escape_json_string(spec.canonical)))
        .collect::<Vec<_>>()
        .join(",");
    let input_contracts = SUPPORTED_INPUT_CONTRACTS
        .iter()
        .map(|value| format!("\"{}\"", escape_json_string(value)))
        .collect::<Vec<_>>()
        .join(",");
    let output_contracts = SUPPORTED_OUTPUT_CONTRACTS
        .iter()
        .map(|value| format!("\"{}\"", escape_json_string(value)))
        .collect::<Vec<_>>()
        .join(",");
    let daemon_routes = SUPPORTED_DAEMON_ROUTES
        .iter()
        .map(|value| format!("\"{}\"", escape_json_string(value)))
        .collect::<Vec<_>>()
        .join(",");
    let release_snapshots = SUPPORTED_MINOR_RELEASE_SNAPSHOTS
        .iter()
        .map(|value| format!("\"{}\"", escape_json_string(value)))
        .collect::<Vec<_>>()
        .join(",");
    let latest_ir_surfaces = SUPPORTED_LATEST_IR_SURFACES
        .iter()
        .map(|value| format!("\"{}\"", escape_json_string(value)))
        .collect::<Vec<_>>()
        .join(",");
    let target_ir_surfaces = SUPPORTED_TARGET_IR_SURFACES
        .iter()
        .map(|value| format!("\"{}\"", escape_json_string(value)))
        .collect::<Vec<_>>()
        .join(",");
    let resident_memory_annotations = RESIDENT_MEMORY_ANNOTATIONS
        .iter()
        .map(|value| format!("\"{}\"", escape_json_string(value)))
        .collect::<Vec<_>>()
        .join(",");
    let gewyvern_merge_hints = GEWYVERN_MERGE_HINTS
        .iter()
        .map(|value| format!("\"{}\"", escape_json_string(value)))
        .collect::<Vec<_>>()
        .join(",");
    let handoff_readiness_levels = HANDOFF_READINESS_LEVELS
        .iter()
        .map(|value| format!("\"{}\"", escape_json_string(value)))
        .collect::<Vec<_>>()
        .join(",");
    let handoff_summary_fields = HANDOFF_SUMMARY_FIELDS
        .iter()
        .map(|value| format!("\"{}\"", escape_json_string(value)))
        .collect::<Vec<_>>()
        .join(",");
    let handoff_route_fanout = HANDOFF_ROUTE_FANOUT
        .iter()
        .map(|value| format!("\"{}\"", escape_json_string(value)))
        .collect::<Vec<_>>()
        .join(",");
    let stable_latest_ir_fields = STABLE_LATEST_IR_FIELDS
        .iter()
        .map(|value| format!("\"{}\"", escape_json_string(value)))
        .collect::<Vec<_>>()
        .join(",");
    let experimental_latest_ir_fields = EXPERIMENTAL_LATEST_IR_FIELDS
        .iter()
        .map(|value| format!("\"{}\"", escape_json_string(value)))
        .collect::<Vec<_>>()
        .join(",");
    let stable_target_ir_fields = STABLE_TARGET_IR_FIELDS
        .iter()
        .map(|value| format!("\"{}\"", escape_json_string(value)))
        .collect::<Vec<_>>()
        .join(",");
    let safe_automation_merge_hints = SAFE_AUTOMATION_MERGE_HINTS
        .iter()
        .map(|value| format!("\"{}\"", escape_json_string(value)))
        .collect::<Vec<_>>()
        .join(",");
    let operator_review_merge_hints = OPERATOR_REVIEW_MERGE_HINTS
        .iter()
        .map(|value| format!("\"{}\"", escape_json_string(value)))
        .collect::<Vec<_>>()
        .join(",");
    let forward_compatibility_rules = FORWARD_COMPATIBILITY_RULES
        .iter()
        .map(|value| format!("\"{}\"", escape_json_string(value)))
        .collect::<Vec<_>>()
        .join(",");
    let api_mode = if access_policy.admin_token.is_some() {
        "loopback_or_token"
    } else {
        "loopback_only"
    };
    format!(
        concat!(
            "{{",
            "\"protocol_family\":\"{}\",",
            "\"protocol_version\":{},",
            "\"crate_version\":\"{}\",",
            "\"capability_tier\":\"resident-sidecar\",",
            "\"input_contracts\":[{}],",
            "\"output_contracts\":[{}],",
            "\"daemon_routes\":[{}],",
            "\"resident_features\":{{",
            "\"online_memory\":true,",
            "\"weighted_training\":true,",
            "\"target_batch_training\":true,",
            "\"cache_invalidation\":true,",
            "\"degraded_polling_state\":true",
            "}},",
            "\"ir_capabilities\":{{",
            "\"latest_scope\":[{}],",
            "\"target_scope\":[{}],",
            "\"resident_memory_annotations\":[{}],",
            "\"shape\":\"structured-text\",",
            "\"stability\":{{",
            "\"stable_latest_fields\":[{}],",
            "\"experimental_latest_fields\":[{}],",
            "\"stable_target_fields\":[{}]",
            "}}",
            "}},",
            "\"merge_capabilities\":{{",
            "\"recommendation_summary_merging\":true,",
            "\"target_batch_merging\":true,",
            "\"operator_guidance_projection\":true,",
            "\"gewyvern_merge_hints\":[{}],",
            "\"safe_automation_hints\":[{}],",
            "\"operator_review_hints\":[{}]",
            "}},",
            "\"security\":{{",
            "\"api_mode\":\"{}\",",
            "\"admin_token_configured\":{},",
            "\"admin_token_header\":\"{}\"",
            "}},",
            "\"handoff_capabilities\":{{",
            "\"readiness_levels\":[{}],",
            "\"summary_fields\":[{}],",
            "\"route_fanout\":[{}]",
            "}},",
            "\"training_labels\":[{}],",
            "\"training_label_count\":{},",
            "\"worker\":{},",
            "\"compatibility\":{{",
            "\"snapshot_schema\":\"gewyvern-analysis-json-v1\",",
            "\"target_index_route\":\"/v1/latest/targets\",",
            "\"minor_release_snapshots\":[{}],",
            "\"forward_compatibility_rules\":[{}]",
            "}}",
            "}}"
        ),
        escape_json_string(ETRAGON_PROTOCOL_FAMILY),
        ETRAGON_PROTOCOL_VERSION,
        env!("CARGO_PKG_VERSION"),
        input_contracts,
        output_contracts,
        daemon_routes,
        latest_ir_surfaces,
        target_ir_surfaces,
        resident_memory_annotations,
        stable_latest_ir_fields,
        experimental_latest_ir_fields,
        stable_target_ir_fields,
        gewyvern_merge_hints,
        safe_automation_merge_hints,
        operator_review_merge_hints,
        escape_json_string(api_mode),
        if access_policy.admin_token.is_some() {
            "true"
        } else {
            "false"
        },
        escape_json_string(ETRAGON_ADMIN_TOKEN_HEADER),
        handoff_readiness_levels,
        handoff_summary_fields,
        handoff_route_fanout,
        training_labels,
        training_label_specs().len(),
        worker_model_info_json,
        release_snapshots,
        forward_compatibility_rules,
    )
}
