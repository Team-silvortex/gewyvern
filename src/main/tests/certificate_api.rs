use super::*;

#[test]
fn capabilities_advertise_runtime_certificates_surface() {
    let snapshot = ApiSnapshot::default();
    let (_, _, body) = api_response_for_request("/v1/capabilities", &snapshot);
    assert!(body.contains("\"runtime_certificates\":true"));
    assert!(body.contains("\"runtime_certificate_policy\":true"));
    assert!(body.contains("\"runtime_certificate_state\":true"));
    assert!(body.contains("/v1/runtime/certificates.json"));
    assert!(body.contains("/v1/runtime/certificate-policy.json"));
    assert!(body.contains("/v1/runtime/certificate-state.json"));
}

#[test]
fn runtime_certificates_surface_is_exposed() {
    let snapshot = ApiSnapshot::default();
    let (status, content_type, body) =
        api_response_for_request("/v1/runtime/certificates.json", &snapshot);
    assert_eq!(status, 200);
    assert_eq!(content_type, "application/json; charset=utf-8");
    assert!(body.contains("\"surface\":\"runtime_certificates\""));
    assert!(body.contains("\"summary\":{"));
    assert!(body.contains("\"policy\":{"));
    assert!(body.contains("\"reason_codes\":["));
}

#[test]
fn runtime_certificate_policy_surface_is_exposed() {
    let snapshot = ApiSnapshot::default();
    let (status, content_type, body) =
        api_response_for_request("/v1/runtime/certificate-policy.json", &snapshot);
    assert_eq!(status, 200);
    assert_eq!(content_type, "application/json; charset=utf-8");
    assert!(body.contains("\"surface\":\"runtime_certificate_policy\""));
    assert!(body.contains("\"reasons\":["));
}

#[test]
fn runtime_certificate_state_surface_is_exposed() {
    let snapshot = ApiSnapshot::default();
    let (status, content_type, body) =
        api_response_for_request("/v1/runtime/certificate-state.json", &snapshot);
    assert_eq!(status, 200);
    assert_eq!(content_type, "application/json; charset=utf-8");
    assert!(body.contains("\"surface\":\"runtime_certificate_state\""));
    assert!(body.contains("\"summary\":{"));
    assert!(body.contains("\"rotation_records\":["));
    assert!(body.contains("\"revocation_records\":["));
}
