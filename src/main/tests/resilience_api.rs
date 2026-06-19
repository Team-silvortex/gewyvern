use super::*;
use crate::external_analysis::{reset_external_fault_state, test_guard};
use crate::socket_resilience::{SocketLoopHealth, reset_socket_resilience_status};

#[test]
fn resilience_endpoint_reports_default_shape() {
    let _guard = test_guard();
    reset_external_fault_state();
    reset_socket_resilience_status();
    let snapshot = ApiSnapshot::default();
    let (status, _, body) = api_response_for_request("/v1/runtime/resilience.json", &snapshot);
    assert_eq!(status, 200);
    assert!(body.contains("\"surface\":\"runtime_resilience\""));
    assert!(body.contains("\"external_analysis\""));
    assert!(body.contains("\"socket_service\""));
    assert!(body.contains("\"degraded\":false"));
    assert!(body.contains("\"status\":\"healthy\""));
    assert!(body.contains("\"severity\":\"ok\""));
    assert!(body.contains("\"recommended_actions\":[\"no operator action required\"]"));
}

#[test]
fn health_endpoint_exposes_resilience_degraded_flag() {
    let _guard = test_guard();
    reset_external_fault_state();
    reset_socket_resilience_status();
    let mut health = SocketLoopHealth::default();
    let _ = health.record_failure();
    let snapshot = ApiSnapshot::default();
    let (status, _, body) = api_response_for_request("/health", &snapshot);
    assert_eq!(status, 200);
    assert!(body.contains("\"resilience_degraded\":true"));
    reset_external_fault_state();
    reset_socket_resilience_status();
}

#[test]
fn resilience_endpoint_reports_socket_backoff_guidance() {
    let _guard = test_guard();
    reset_external_fault_state();
    reset_socket_resilience_status();
    let mut health = SocketLoopHealth::default();
    let _ = health.record_failure();
    let _ = health.record_failure();
    let snapshot = ApiSnapshot::default();
    let (status, _, body) = api_response_for_request("/v1/runtime/resilience.json", &snapshot);
    assert_eq!(status, 200);
    assert!(body.contains("\"status\":\"degraded\""));
    assert!(body.contains("\"severity\":\"warning\""));
    assert!(body.contains("\"socket_service\":{\"consecutive_failures\":2"));
    assert!(body.contains("\"status\":\"backing_off\""));
    assert!(body.contains("inspect recent socket clients for malformed or incomplete sessions"));
    reset_external_fault_state();
    reset_socket_resilience_status();
}
