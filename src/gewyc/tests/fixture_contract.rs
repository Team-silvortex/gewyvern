use super::*;

fn read_fixture(path: &str) -> String {
    std::fs::read_to_string(path).unwrap()
}

#[test]
fn frontend_fixture_keeps_wrapper_and_grouped_frontend_fields() {
    let json = read_fixture("docs/fixtures/gewyc_frontend_udp_process_debug.json");

    assert_valid_json_document(&json);
    assert!(json.contains("\"surface_id\": \"gewyc.frontend\""));
    assert!(json.contains("\"schema_version\": 1"));
    assert!(json.contains("\"compatibility\": \"grouped_payload_preferred\""));
    assert!(json.contains("\"payload\": {"));
    assert!(json.contains("\"report\": {"));
    assert!(json.contains("\"authoring\": {"));
    assert!(json.contains("\"counts\": {"));
}

#[test]
fn stages_fixture_keeps_grouped_status_and_counts_fields() {
    let json = read_fixture("docs/fixtures/gewyc_stages_udp_process_debug.json");

    assert_valid_json_document(&json);
    assert!(json.contains("\"surface_id\": \"gewyc.stages\""));
    assert!(json.contains("\"payload\": {"));
    assert!(json.contains("\"status\": {"));
    assert!(json.contains("\"parse_ok\": true"));
    assert!(json.contains("\"validation_ok\": true"));
    assert!(json.contains("\"diagnostics_ok\": true"));
    assert!(json.contains("\"counts\": {"));
    assert!(json.contains("\"validation_fragments\": 3"));
}

#[test]
fn explain_success_fixture_keeps_summary_focus_shape() {
    let json = read_fixture("docs/fixtures/gewyc_explain_validation_udp_process_debug.json");

    assert_valid_json_document(&json);
    assert!(json.contains("\"surface_id\": \"gewyc.explain\""));
    assert!(json.contains("\"payload\": {"));
    assert!(json.contains("\"summary\": {"));
    assert!(json.contains("\"stage_status\": {"));
    assert!(json.contains("\"analysis\": {"));
    assert!(json.contains("\"shape_notes\": {"));
    assert!(json.contains("\"excerpts\": {"));
    assert!(json.contains("\"focused_report\": {"));
}

#[test]
fn explain_parse_failure_fixture_keeps_parse_excerpt_shape() {
    let json = read_fixture("docs/fixtures/gewyc_explain_parse_failure.json");

    assert_valid_json_document(&json);
    assert!(json.contains("\"surface_id\": \"gewyc.explain\""));
    assert!(json.contains("\"parse_ok\": false"));
    assert!(json.contains("\"parse_source\": {"));
    assert!(json.contains("\"line_text\": \"|> use(:missing_function)\""));
}

#[test]
fn explain_validation_failure_fixture_keeps_validation_excerpt_shape() {
    let json = read_fixture("docs/fixtures/gewyc_explain_validation_failure.json");

    assert_valid_json_document(&json);
    assert!(json.contains("\"surface_id\": \"gewyc.explain\""));
    assert!(json.contains("\"validation_ok\": false"));
    assert!(json.contains("\"validation\": {"));
    assert!(json.contains("\"unsupported_payload_offsets\": ["));
    assert!(json.contains("\"rule_index\": 0"));
    assert!(json.contains("\"model\": \"broken_offsets_model\""));
}
