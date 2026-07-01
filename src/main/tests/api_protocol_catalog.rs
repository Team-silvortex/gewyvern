use super::*;

#[test]
fn capabilities_advertise_protocol_catalog_surfaces() {
    let snapshot = ApiSnapshot::default();
    let (status, _, body) = api_response_for_request("/v1/capabilities", &snapshot);
    assert_eq!(status, 200);
    assert!(body.contains("\"protocol_catalog\":true"));
    assert!(body.contains("\"protocol_cluster_catalog\":true"));
    assert!(body.contains("\"protocol_surface_catalog\":true"));
    assert!(body.contains("\"target_protocol_reading\":true"));
    assert!(body.contains("\"debug_session\":true"));
    assert!(body.contains("\"/v1/protocols\""));
    assert!(body.contains("\"/v1/protocols/<protocol>/entries/<entry>/surface.json\""));
    assert!(body.contains("\"/v1/latest/debug-session.json\""));
    assert!(body.contains("\"/v1/latest/targets/<name>/protocol-reading.json\""));
    assert!(body.contains("\"/v1/latest/targets/<name>/debug-session.json\""));
}

#[test]
fn protocol_catalog_endpoint_lists_mysql_and_entry_metadata() {
    let snapshot = ApiSnapshot::default();
    let (status, content_type, body) = api_response_for_request("/v1/protocols", &snapshot);
    assert_eq!(status, 200);
    assert_eq!(content_type, "application/json; charset=utf-8");
    assert!(body.contains("\"surface\":\"protocol_catalog\""));
    assert!(body.contains("\"protocol\":\"mysql\""));
    assert!(body.contains("\"default_entry\":\"session\""));
    assert!(body.contains("\"entry_count\":"));
    assert!(body.contains("\"cluster_hint\":{"));
    assert!(body.contains("\"mode\":\"query\""));
}

#[test]
fn protocol_summary_endpoint_returns_ldap_entries() {
    let snapshot = ApiSnapshot::default();
    let (status, _, body) = api_response_for_request("/v1/protocols/ldap", &snapshot);
    assert_eq!(status, 200);
    assert!(body.contains("\"protocol\":\"ldap\""));
    assert!(body.contains("\"default_entry\":\"sync\""));
    assert!(body.contains("\"cluster_hint\":{"));
    assert!(body.contains("\"key\":\"identity-directory-access\""));
    assert!(body.contains("\"mode\":\"bind\""));
    assert!(body.contains("\"mode\":\"search\""));
}

#[test]
fn protocol_cluster_catalog_endpoint_groups_cache_queue_stream_families() {
    let snapshot = ApiSnapshot::default();
    let (status, content_type, body) = api_response_for_request("/v1/protocol-clusters", &snapshot);
    assert_eq!(status, 200);
    assert_eq!(content_type, "application/json; charset=utf-8");
    assert!(body.contains("\"surface\":\"protocol_cluster_catalog\""));
    assert!(body.contains("\"key\":\"cache-queue-stream\""));
    assert!(body.contains("\"protocol\":\"redis\""));
    assert!(body.contains("\"protocol\":\"amqp\""));
}

#[test]
fn protocol_cluster_endpoint_returns_identity_directory_access_view() {
    let snapshot = ApiSnapshot::default();
    let (status, _, body) =
        api_response_for_request("/v1/protocol-clusters/identity-directory-access", &snapshot);
    assert_eq!(status, 200);
    assert!(body.contains("\"key\":\"identity-directory-access\""));
    assert!(body.contains("\"protocol\":\"ldap\""));
    assert!(body.contains("\"protocol\":\"ssh\""));
}

#[test]
fn protocol_entry_surface_endpoint_returns_redis_shelf_context() {
    let snapshot = ApiSnapshot::default();
    let (status, _, body) =
        api_response_for_request("/v1/protocols/redis/entries/zadd/surface.json", &snapshot);
    assert_eq!(status, 200);
    assert!(body.contains("\"protocol\":\"redis\""));
    assert!(body.contains("\"entry\":\"zadd\""));
    assert!(body.contains("\"selected_is_default\":false"));
    assert!(body.contains("\"cluster_hint\":{"));
    assert!(body.contains("\"key\":\"cache-queue-stream\""));
    assert!(body.contains("\"key\":\"sorted-set\""));
    assert!(body.contains("\"page\":\"docs/book/reference-redis-sorted-set-surface.md\""));
}

#[test]
fn protocol_entry_surface_endpoint_exposes_reading_companions() {
    let snapshot = ApiSnapshot::default();
    let (status, _, body) = api_response_for_request(
        "/v1/protocols/https/entries/connect/surface.json",
        &snapshot,
    );
    assert_eq!(status, 200);
    assert!(body.contains("\"selected_overlay\":null"));
    assert!(body.contains("\"overlays\":[{"));
    assert!(body.contains("\"reading_companions\":[{\"protocol\":\"tls\",\"entry\":\"client\",\"via_overlay\":\"https\""));
}

#[test]
fn target_protocol_reading_endpoint_returns_next_protocol_steps() {
    let mut snapshot = ApiSnapshot::default();
    snapshot
        .target_snapshots
        .insert("scan:http3:request".into(), ApiTargetSnapshot::default());
    let (status, content_type, body) = api_response_for_request(
        "/v1/latest/targets/scan:http3:request/protocol-reading.json",
        &snapshot,
    );
    assert_eq!(status, 200);
    assert_eq!(content_type, "application/json; charset=utf-8");
    assert!(body.contains("\"surface\":\"target_protocol_reading\""));
    assert!(body.contains("\"target\":\"scan:http3:request\""));
    assert!(body.contains("\"kind\":\"primary\""));
    assert!(body.contains("\"kind\":\"companion\""));
    assert!(body.contains("\"protocol\":\"quic\""));
    assert!(body.contains("\"entry\":\"initial\""));
}

#[test]
fn target_protocol_reading_endpoint_rejects_non_protocol_targets_cleanly() {
    let mut snapshot = ApiSnapshot::default();
    snapshot
        .target_snapshots
        .insert("custom-target".into(), ApiTargetSnapshot::default());
    let (status, _, body) = api_response_for_request(
        "/v1/latest/targets/custom-target/protocol-reading.json",
        &snapshot,
    );
    assert_eq!(status, 404);
    assert!(body.contains("\"error\":\"protocol_reading_unavailable\""));
    assert!(body.contains("\"target\":\"custom-target\""));
}

#[test]
fn debug_session_endpoint_unifies_focus_and_next_steps() {
    let mut snapshot = ApiSnapshot {
        kind: "scan".into(),
        target_names: vec!["scan:http3:request".into()],
        ..ApiSnapshot::default()
    };
    snapshot.target_snapshots.insert(
        "scan:http3:request".into(),
        ApiTargetSnapshot {
            evidence_posture: Some("missing_transition".into()),
            automation_outcome: Some("collect_more_evidence".into()),
            analysis_json: "{\"target_status\":\"attention\",\"primary_failure_stage\":\"request_response\",\"primary_failure_mode\":\"no_response\",\"primary_failure_detail\":\"request_sent_no_reply\",\"primary_failure_basis\":\"missing_transition\",\"operator_guidance_action\":\"collect_more_runtime_evidence\",\"operator_guidance_summary\":\"collect the missing response\",\"missing_transitions\":[\"send_request->receive_response\"]}".into(),
            protocol_surface: gewyvern::protocol_profiles::protocol_surface("http3", "request"),
            ..ApiTargetSnapshot::default()
        },
    );

    let (status, content_type, body) =
        api_response_for_request("/v1/latest/debug-session.json", &snapshot);
    assert_eq!(status, 200);
    assert_eq!(content_type, "application/json; charset=utf-8");
    assert!(body.contains("\"surface\":\"debug_session\""));
    assert!(body.contains("\"scope\":\"snapshot\""));
    assert!(body.contains("\"recommended_focus\":{"));
    assert!(body.contains("\"failure_spine\":{"));
    assert!(body.contains("\"operator_guidance\":{"));
    assert!(body.contains("\"debugger_posture\":{"));
    assert!(body.contains("\"state\":\"needs_evidence\""));
    assert!(body.contains("\"recommended_action\":\"collect_missing_runtime_evidence\""));
    assert!(body.contains("\"kind\":\"read_protocol_plan\""));
    assert!(body.contains("\"kind\":\"collect_missing_evidence\""));
    assert!(body.contains("/v1/latest/targets/scan:http3:request/protocol-reading.json"));
}

#[test]
fn target_debug_session_endpoint_returns_one_target_session() {
    let mut snapshot = ApiSnapshot::default();
    snapshot.target_snapshots.insert(
        "scan:http3:request".into(),
        ApiTargetSnapshot {
            analysis_json: "{\"target_status\":\"healthy\",\"primary_failure_stage\":\"none\",\"primary_failure_mode\":\"none\",\"primary_failure_detail\":\"none\",\"primary_failure_basis\":\"none\",\"operator_guidance_action\":\"observe\",\"operator_guidance_summary\":\"target is healthy\",\"missing_transitions\":[]}".into(),
            protocol_surface: gewyvern::protocol_profiles::protocol_surface("http3", "request"),
            ..ApiTargetSnapshot::default()
        },
    );

    let (status, _, body) = api_response_for_request(
        "/v1/latest/targets/scan:http3:request/debug-session.json",
        &snapshot,
    );
    assert_eq!(status, 200);
    assert!(body.contains("\"scope\":\"target\""));
    assert!(body.contains("\"target\":\"scan:http3:request\""));
    assert!(body.contains("\"protocol\":\"http3\""));
    assert!(body.contains("\"entry\":\"request\""));
    assert!(body.contains("\"state\":\"healthy\""));
    assert!(body.contains("\"recommended_action\":\"observe_stable_baseline\""));
    assert!(body.contains(
        "\"debug_session\":\"/v1/latest/targets/scan:http3:request/debug-session.json\""
    ));
}

#[test]
fn protocol_catalog_endpoints_report_unknown_protocols_cleanly() {
    let snapshot = ApiSnapshot::default();
    let (summary_status, _, summary_body) =
        api_response_for_request("/v1/protocols/not-a-real-protocol", &snapshot);
    assert_eq!(summary_status, 404);
    assert!(summary_body.contains("\"error\":\"unknown_protocol\""));

    let (surface_status, _, surface_body) = api_response_for_request(
        "/v1/protocols/redis/entries/not-a-real-entry/surface.json",
        &snapshot,
    );
    assert_eq!(surface_status, 404);
    assert!(surface_body.contains("\"error\":\"unknown_protocol_entry\""));

    let (cluster_status, _, cluster_body) =
        api_response_for_request("/v1/protocol-clusters/not-a-real-cluster", &snapshot);
    assert_eq!(cluster_status, 404);
    assert!(cluster_body.contains("\"error\":\"unknown_protocol_cluster\""));
}
