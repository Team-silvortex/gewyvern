use super::*;

#[test]
fn protocol_entry_surface_endpoint_exposes_http3_close_semantics() {
    let snapshot = ApiSnapshot::default();
    let (status, _, body) =
        api_response_for_request("/v1/protocols/http3/entries/close/surface.json", &snapshot);
    assert_eq!(status, 200);
    assert!(body.contains("\"protocol\":\"http3\""));
    assert!(body.contains("\"entry\":\"close\""));
    assert!(body.contains("\"entry_semantics\":{"));
    assert!(body.contains("\"category\":\"failure-path\""));
    assert!(
        body.contains(
            "\"operator_focus\":\"application-layer HTTP/3 request path terminated by peer connection close before steady response completion\""
        )
    );
    assert!(body.contains("\"typical_signal\":\"CONNECTION_CLOSE\""));
    assert!(body.contains("\"primary_failure_mode\":\"peer_closed\""));
    assert!(body.contains("\"primary_failure_detail\":\"transport_terminated\""));
}

#[test]
fn protocol_entry_surface_endpoint_exposes_http3_server_close_semantics() {
    let snapshot = ApiSnapshot::default();
    let (status, _, body) = api_response_for_request(
        "/v1/protocols/http3/entries/server-close/surface.json",
        &snapshot,
    );
    assert_eq!(status, 200);
    assert!(body.contains("\"protocol\":\"http3\""));
    assert!(body.contains("\"entry\":\"server-close\""));
    assert!(body.contains("\"entry_semantics\":{"));
    assert!(body.contains("\"category\":\"failure-path\""));
    assert!(
        body.contains(
            "\"operator_focus\":\"HTTP/3 server response path ended with a locally emitted connection close after request handling and response delivery had already started\""
        )
    );
    assert!(body.contains("\"typical_signal\":\"CONNECTION_CLOSE\""));
    assert!(body.contains("\"primary_failure_mode\":\"local_closed\""));
    assert!(body.contains("\"primary_failure_detail\":\"server_terminated_session\""));
}

#[test]
fn protocol_entry_surface_endpoint_exposes_hy2_close_semantics() {
    let snapshot = ApiSnapshot::default();
    let (status, _, body) =
        api_response_for_request("/v1/protocols/hy2/entries/close/surface.json", &snapshot);
    assert_eq!(status, 200);
    assert!(body.contains("\"protocol\":\"hy2\""));
    assert!(body.contains("\"entry\":\"close\""));
    assert!(body.contains("\"entry_semantics\":{"));
    assert!(body.contains("\"category\":\"failure-path\""));
    assert!(
        body.contains(
            "\"operator_focus\":\"authenticated Hysteria2 session terminated by peer connection close before relay continuity could be maintained\""
        )
    );
    assert!(body.contains("\"typical_signal\":\"CONNECTION_CLOSE\""));
    assert!(body.contains("\"primary_failure_mode\":\"peer_closed\""));
    assert!(body.contains("\"primary_failure_detail\":\"secure_session_terminated\""));
}

#[test]
fn protocol_entry_surface_endpoint_exposes_hy2_tcp_close_semantics() {
    let snapshot = ApiSnapshot::default();
    let (status, _, body) =
        api_response_for_request("/v1/protocols/hy2/entries/tcp-close/surface.json", &snapshot);
    assert_eq!(status, 200);
    assert!(body.contains("\"protocol\":\"hy2\""));
    assert!(body.contains("\"entry\":\"tcp-close\""));
    assert!(body.contains("\"entry_semantics\":{"));
    assert!(body.contains("\"category\":\"failure-path\""));
    assert!(
        body.contains(
            "\"operator_focus\":\"authenticated Hysteria2 TCP relay terminated by peer connection close after relay request and response activity had already started\""
        )
    );
    assert!(body.contains("\"typical_signal\":\"CONNECTION_CLOSE\""));
    assert!(body.contains("\"primary_failure_mode\":\"peer_closed\""));
    assert!(body.contains("\"primary_failure_detail\":\"tcp_relay_terminated\""));
}

#[test]
fn protocol_entry_surface_endpoint_exposes_hy2_udp_close_semantics() {
    let snapshot = ApiSnapshot::default();
    let (status, _, body) =
        api_response_for_request("/v1/protocols/hy2/entries/udp-close/surface.json", &snapshot);
    assert_eq!(status, 200);
    assert!(body.contains("\"protocol\":\"hy2\""));
    assert!(body.contains("\"entry\":\"udp-close\""));
    assert!(body.contains("\"entry_semantics\":{"));
    assert!(body.contains("\"category\":\"failure-path\""));
    assert!(
        body.contains(
            "\"operator_focus\":\"authenticated Hysteria2 UDP relay terminated by peer connection close after relay datagram exchange had already started\""
        )
    );
    assert!(body.contains("\"typical_signal\":\"CONNECTION_CLOSE\""));
    assert!(body.contains("\"primary_failure_mode\":\"peer_closed\""));
    assert!(body.contains("\"primary_failure_detail\":\"udp_relay_terminated\""));
}
