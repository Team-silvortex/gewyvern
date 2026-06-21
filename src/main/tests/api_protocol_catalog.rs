use super::*;

#[test]
fn capabilities_advertise_protocol_catalog_surfaces() {
    let snapshot = ApiSnapshot::default();
    let (status, _, body) = api_response_for_request("/v1/capabilities", &snapshot);
    assert_eq!(status, 200);
    assert!(body.contains("\"protocol_catalog\":true"));
    assert!(body.contains("\"protocol_cluster_catalog\":true"));
    assert!(body.contains("\"protocol_surface_catalog\":true"));
    assert!(body.contains("\"/v1/protocols\""));
    assert!(body.contains("\"/v1/protocols/<protocol>/entries/<entry>/surface.json\""));
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
