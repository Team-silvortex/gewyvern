mod support;

use etragon::AnalysisSnapshotInput;
use support::read_fixture;

#[test]
fn parses_missing_transition_fixture() {
    let snapshot = AnalysisSnapshotInput::from_gewyvern_analysis_json(&read_fixture(
        "missing_transition_analysis.json",
    ))
    .expect("fixture should parse");

    assert_eq!(snapshot.primary_module_kind, "http_request_response");
    assert_eq!(snapshot.primary_failure_confidence, "medium");
    assert!(!snapshot.ambiguous);
    assert!(snapshot.competing_hypotheses.is_empty());
}

#[test]
fn parses_ambiguous_fixture_with_competing_hypotheses() {
    let snapshot = AnalysisSnapshotInput::from_gewyvern_analysis_json(&read_fixture(
        "ambiguous_analysis.json",
    ))
    .expect("fixture should parse");

    assert!(snapshot.ambiguous);
    assert_eq!(snapshot.competing_hypotheses.len(), 2);
    assert_eq!(snapshot.competing_hypotheses[0], "module:name_resolution");
}
