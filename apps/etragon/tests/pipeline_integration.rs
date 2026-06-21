mod support;

use etragon::{
    AnalysisSnapshotInput, ExternalAnalysisEngine, MockMlAdvisoryEngine,
    MockRecommendationAugmenter, MockScoreRerankPass, PassPipeline,
};
use support::read_fixture;

#[test]
fn pipeline_layers_timeout_candidate_and_manual_queue_from_fixture() {
    let snapshot = AnalysisSnapshotInput::from_gewyvern_analysis_json(&read_fixture(
        "missing_transition_analysis.json",
    ))
    .expect("fixture should parse");
    let pipeline = PassPipeline::new()
        .with_candidate(&MockMlAdvisoryEngine)
        .with_recommendation(&MockRecommendationAugmenter)
        .with_rerank(&MockScoreRerankPass);

    let output = pipeline.analyze(&snapshot);

    assert_eq!(output.augmentations.len(), 2);
    assert_eq!(output.augmentations[0].name, "ml_candidate_observe_longer");
    assert_eq!(output.augmentations[0].confidence, "ranked-candidate");
    assert_eq!(output.augmentations[1].name, "ml_recommend_manual_queue");
}

#[test]
fn pipeline_keeps_multi_hypothesis_recommendation_open_for_ambiguous_fixture() {
    let snapshot = AnalysisSnapshotInput::from_gewyvern_analysis_json(&read_fixture(
        "ambiguous_analysis.json",
    ))
    .expect("fixture should parse");
    let pipeline = PassPipeline::new()
        .with_candidate(&MockMlAdvisoryEngine)
        .with_recommendation(&MockRecommendationAugmenter);

    let output = pipeline.analyze(&snapshot);

    assert_eq!(output.augmentations.len(), 2);
    assert_eq!(
        output.augmentations[0].name,
        "ml_candidate_multi_hypothesis"
    );
    assert_eq!(
        output.augmentations[1].name,
        "ml_recommend_keep_candidate_set_open"
    );
}
