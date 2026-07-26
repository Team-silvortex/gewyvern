use leserpent_domain::{
    CAPABILITY_RUNTIME_READ, CAPABILITY_RUNTIME_REFRESH, CapabilitySet, CommandOrigin,
    InMemoryControlPlane, Principal, QueryResult, RefreshStatus, Revision, RuntimeId,
};
use leserpent_protocol::MAX_PROTOCOL_MESSAGE_BYTES;
use leserpent_protocol::compatibility_v1::{
    CompatibilityError, apply_status_refresh, decode_api_error, decode_orchestra_persistence,
    decode_runtime_collection, decode_runtime_deployment_request, decode_status_refresh,
    domain_error_to_legacy, normalize_runtime_deployment_request, runtime_list_query,
    runtime_status_refresh_command, seed_runtime_collection,
};

fn operator() -> Principal {
    Principal {
        id: "compat-operator".to_string(),
    }
}

#[test]
fn legacy_runtime_list_normalizes_to_domain_filter_order_and_status() {
    let collection = decode_runtime_collection(include_bytes!(
        "fixtures/legacy-runtime-list-response-v1.json"
    ))
    .unwrap();
    let query = runtime_list_query(
        operator(),
        CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
        collection.filter.clone(),
    );
    let mut control = InMemoryControlPlane::default();
    seed_runtime_collection(&mut control, collection).unwrap();

    let QueryResult::RuntimeList { revision, runtimes } = control.query(query).unwrap() else {
        panic!("runtime list fixture must return a list result");
    };
    assert_eq!(revision, Revision(2));
    assert_eq!(runtimes.len(), 2);
    assert_eq!(runtimes[0].id.as_str(), "runtime-alpha");
    assert_eq!(runtimes[1].id.as_str(), "runtime-bravo");
    assert_eq!(runtimes[0].status.status_source, "gewyvern-api");
    assert_eq!(runtimes[0].status.target_count, Some(1));
}

#[test]
fn legacy_status_refresh_runs_as_idempotent_command_then_applies_observation() {
    let collection = decode_runtime_collection(include_bytes!(
        "fixtures/legacy-runtime-list-response-v1.json"
    ))
    .unwrap();
    let mut control = InMemoryControlPlane::default();
    seed_runtime_collection(&mut control, collection).unwrap();
    let runtime_id = RuntimeId::new("runtime-alpha").unwrap();
    let command = runtime_status_refresh_command(
        operator(),
        CapabilitySet::new([CAPABILITY_RUNTIME_REFRESH]),
        runtime_id.as_str(),
        "compat-command-1",
        "compat-refresh-alpha",
        Some(Revision(1)),
        false,
    )
    .unwrap();
    assert_eq!(command.origin, CommandOrigin::CompatibilityAdapter);

    let requested = control.execute(command.clone()).unwrap();
    assert_eq!(requested.runtime.refresh_status, RefreshStatus::Pending);
    assert_eq!(control.execute(command).unwrap(), requested);

    let response = decode_status_refresh(include_bytes!(
        "fixtures/legacy-runtime-status-refresh-response-v1.json"
    ))
    .unwrap();
    let completed = apply_status_refresh(
        &mut control,
        &runtime_id,
        requested.runtime.revision,
        response,
    )
    .unwrap();
    assert_eq!(completed.refresh_status, RefreshStatus::Ready);
    assert_eq!(completed.status.target_count, Some(2));
    assert_eq!(completed.revision, Revision(4));
}

#[test]
fn compatibility_adapter_preserves_not_found_and_rejects_identity_confusion() {
    let legacy =
        decode_api_error(include_bytes!("fixtures/legacy-runtime-not-found-v1.json")).unwrap();
    assert_eq!(legacy.error, "runtime_not_found");
    assert_eq!(legacy.runtime_id.as_deref(), Some("runtime-missing"));

    let mut control = InMemoryControlPlane::default();
    let runtime_id = RuntimeId::new("runtime-alpha").unwrap();
    control.register_runtime(runtime_id.clone(), "Alpha", "http://127.0.0.1:9411");
    let missing = control
        .complete_runtime_status_refresh(
            &RuntimeId::new("runtime-missing").unwrap(),
            Revision(1),
            Default::default(),
        )
        .unwrap_err();
    assert_eq!(domain_error_to_legacy(&missing), legacy);

    let mut response = decode_status_refresh(include_bytes!(
        "fixtures/legacy-runtime-status-refresh-response-v1.json"
    ))
    .unwrap();
    response.runtime_id = "runtime-bravo".to_string();
    assert!(matches!(
        apply_status_refresh(&mut control, &runtime_id, Revision(1), response),
        Err(CompatibilityError::RuntimeIdentityMismatch { .. })
    ));
}

#[test]
fn compatibility_decoder_enforces_the_wire_size_limit() {
    let oversized = vec![b' '; MAX_PROTOCOL_MESSAGE_BYTES + 1];
    assert!(matches!(
        decode_runtime_collection(&oversized),
        Err(CompatibilityError::Oversized { .. })
    ));
}

#[test]
fn legacy_deployment_request_freezes_the_confirmed_idempotent_write_contract() {
    let fixture = include_bytes!("fixtures/legacy-runtime-deployment-request-v1.json");
    let deployment = decode_runtime_deployment_request(fixture).unwrap();
    assert_eq!(deployment.runtime_id, "runtime-alpha");
    assert_eq!(deployment.request.request_id, "deploy-001");
    assert_eq!(deployment.request.pipeline_kind, "capture/http");
    assert_eq!(deployment.request.requested_by, "operator-a");
    assert_eq!(deployment.request.target.as_deref(), Some("service-a"));

    let mut unconfirmed: serde_json::Value = serde_json::from_slice(fixture).unwrap();
    unconfirmed["request"]["confirmed"] = serde_json::json!(false);
    assert!(matches!(
        decode_runtime_deployment_request(&serde_json::to_vec(&unconfirmed).unwrap()),
        Err(CompatibilityError::InvalidDeployment(_))
    ));

    let mut drifted: serde_json::Value = serde_json::from_slice(fixture).unwrap();
    drifted["request"]["unexpected"] = serde_json::json!(true);
    assert!(matches!(
        decode_runtime_deployment_request(&serde_json::to_vec(&drifted).unwrap()),
        Err(CompatibilityError::InvalidJson(_))
    ));
}

#[test]
fn rust_normalizes_the_authoritative_legacy_deployment_intent() {
    let source = br#"{"runtimeId":"runtime-alpha","request":{"pipelineKind":" capture/http ","requestedBy":" operator-a ","confirmed":true,"requestId":" deploy-001 ","target":"  "}}"#;
    let normalized = normalize_runtime_deployment_request(source).unwrap();
    assert_eq!(normalized.request.pipeline_kind, "capture/http");
    assert_eq!(normalized.request.requested_by, "operator-a");
    assert_eq!(normalized.request.request_id, "deploy-001");
    assert_eq!(normalized.request.target, None);
}

#[test]
fn legacy_orchestra_fixture_freezes_atomic_run_event_persistence() {
    let fixture = include_bytes!("fixtures/legacy-orchestra-persistence-v1.json");
    let persistence = decode_orchestra_persistence(fixture).unwrap();
    assert_eq!(persistence.run.run_id, persistence.event.run_id);
    assert_eq!(persistence.run.runtime_id, persistence.event.runtime_id);
    assert_eq!(persistence.run.outcome, persistence.event.to_outcome);
    assert_eq!(persistence.run.request_id.as_deref(), Some("deploy-001"));
    assert_eq!(
        persistence.event.event_id, 0,
        "SQLite owns event ID allocation"
    );

    let mut mismatched: serde_json::Value = serde_json::from_slice(fixture).unwrap();
    mismatched["event"]["runId"] = serde_json::json!("orun_other");
    assert!(matches!(
        decode_orchestra_persistence(&serde_json::to_vec(&mismatched).unwrap()),
        Err(CompatibilityError::InvalidOrchestra("consistency"))
    ));

    let mut unsafe_summary: serde_json::Value = serde_json::from_slice(fixture).unwrap();
    unsafe_summary["event"]["summary"] = serde_json::json!("secret\nheader");
    assert!(matches!(
        decode_orchestra_persistence(&serde_json::to_vec(&unsafe_summary).unwrap()),
        Err(CompatibilityError::InvalidOrchestra("bounds"))
    ));

    let mut oversized_note: serde_json::Value = serde_json::from_slice(fixture).unwrap();
    oversized_note["run"]["approvalNote"] = serde_json::json!("x".repeat(1025));
    assert!(matches!(
        decode_orchestra_persistence(&serde_json::to_vec(&oversized_note).unwrap()),
        Err(CompatibilityError::InvalidOrchestra("bounds"))
    ));

    let step = persistence.run.steps[0].clone();
    let mut maximum_steps: serde_json::Value = serde_json::from_slice(fixture).unwrap();
    maximum_steps["run"]["steps"] = serde_json::to_value(vec![step.clone(); 256]).unwrap();
    assert!(decode_orchestra_persistence(&serde_json::to_vec(&maximum_steps).unwrap()).is_ok());
    maximum_steps["run"]["steps"] = serde_json::to_value(vec![step; 257]).unwrap();
    assert!(matches!(
        decode_orchestra_persistence(&serde_json::to_vec(&maximum_steps).unwrap()),
        Err(CompatibilityError::InvalidOrchestra("consistency"))
    ));
}
