use super::*;

#[test]
fn protocol_entry_surface_endpoint_exposes_tls_client_shelf_context() {
    let snapshot = ApiSnapshot::default();
    let (status, _, body) =
        api_response_for_request("/v1/protocols/tls/entries/client/surface.json", &snapshot);
    assert_eq!(status, 200);
    assert!(body.contains("\"protocol\":\"tls\""));
    assert!(body.contains("\"entry\":\"client\""));
    assert!(body.contains("\"selected_is_default\":true"));
    assert!(body.contains("\"key\":\"client\""));
    assert!(body.contains("\"page\":\"docs/book/reference-tls-client-surface.md\""));
    assert!(body.contains("\"entry_aliases\":[\"initiator\",\"tls-client\",\"tls_client\"]"));
}

#[test]
fn protocol_entry_surface_endpoint_exposes_tls_server_shelf_context() {
    let snapshot = ApiSnapshot::default();
    let (status, _, body) =
        api_response_for_request("/v1/protocols/tls/entries/server/surface.json", &snapshot);
    assert_eq!(status, 200);
    assert!(body.contains("\"protocol\":\"tls\""));
    assert!(body.contains("\"entry\":\"server\""));
    assert!(body.contains("\"selected_is_default\":false"));
    assert!(body.contains("\"key\":\"server\""));
    assert!(body.contains("\"page\":\"docs/book/reference-tls-server-surface.md\""));
    assert!(body.contains("\"entry_aliases\":[\"acceptor\",\"tls-server\",\"tls_server\"]"));
}
