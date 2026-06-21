use crate::contract::{
    AnalysisAugmentation, AnalysisSnapshotInput, EngineOutput, SnapshotParseError,
};

pub trait ExternalAnalysisEngine {
    fn analyze(&self, snapshot: &AnalysisSnapshotInput) -> EngineOutput;
}

pub trait CandidateAugmenter {
    fn candidate_augmentations(
        &self,
        snapshot: &AnalysisSnapshotInput,
    ) -> Vec<AnalysisAugmentation>;

    fn pass_name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

pub trait RecommendationAugmenter {
    fn recommendation_augmentations(
        &self,
        snapshot: &AnalysisSnapshotInput,
        current: &[AnalysisAugmentation],
    ) -> Vec<AnalysisAugmentation>;

    fn pass_name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

pub trait RerankPass {
    fn rerank(&self, snapshot: &AnalysisSnapshotInput, current: &mut Vec<AnalysisAugmentation>);

    fn pass_name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

#[derive(Default)]
pub struct PassPipeline<'a> {
    candidate_augmenters: Vec<&'a dyn CandidateAugmenter>,
    recommendation_augmenters: Vec<&'a dyn RecommendationAugmenter>,
    rerank_passes: Vec<&'a dyn RerankPass>,
}

impl<'a> PassPipeline<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_candidate(mut self, augmenter: &'a dyn CandidateAugmenter) -> Self {
        self.candidate_augmenters.push(augmenter);
        self
    }

    pub fn with_recommendation(mut self, augmenter: &'a dyn RecommendationAugmenter) -> Self {
        self.recommendation_augmenters.push(augmenter);
        self
    }

    pub fn with_rerank(mut self, pass: &'a dyn RerankPass) -> Self {
        self.rerank_passes.push(pass);
        self
    }
}

impl ExternalAnalysisEngine for PassPipeline<'_> {
    fn analyze(&self, snapshot: &AnalysisSnapshotInput) -> EngineOutput {
        let mut augmentations = Vec::new();
        for augmenter in &self.candidate_augmenters {
            augmentations.extend(
                augmenter
                    .candidate_augmentations(snapshot)
                    .into_iter()
                    .map(|item| item.with_producer("candidate", augmenter.pass_name())),
            );
        }
        for augmenter in &self.recommendation_augmenters {
            augmentations.extend(
                augmenter
                    .recommendation_augmentations(snapshot, &augmentations)
                    .into_iter()
                    .map(|item| item.with_producer("recommendation", augmenter.pass_name())),
            );
        }
        for pass in &self.rerank_passes {
            pass.rerank(snapshot, &mut augmentations);
            for item in &mut augmentations {
                if item.producer_stage.is_none() {
                    item.producer_stage = Some("rerank".into());
                }
                if item.producer_pass.is_none() {
                    item.producer_pass = Some(pass.pass_name().into());
                }
            }
        }
        EngineOutput { augmentations }
    }
}

pub fn append_engine_output(destination: &mut Vec<AnalysisAugmentation>, output: EngineOutput) {
    destination.extend(output.augmentations);
}

pub fn analyze_gewyvern_analysis_json(
    engine: &dyn ExternalAnalysisEngine,
    input: &str,
) -> Result<EngineOutput, SnapshotParseError> {
    let snapshot = AnalysisSnapshotInput::from_gewyvern_analysis_json(input)?;
    Ok(engine.analyze(&snapshot))
}

pub fn engine_output_json(output: &EngineOutput) -> String {
    format!(
        "{{\"augmentations\":[{}]}}",
        output
            .augmentations
            .iter()
            .map(augmentation_json)
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn augmentation_json(item: &AnalysisAugmentation) -> String {
    let producer_stage = item
        .producer_stage
        .as_deref()
        .map(|value| format!("\"{}\"", escape_json_string(value)))
        .unwrap_or_else(|| "null".into());
    let producer_pass = item
        .producer_pass
        .as_deref()
        .map(|value| format!("\"{}\"", escape_json_string(value)))
        .unwrap_or_else(|| "null".into());
    format!(
        "{{\"kind\":\"{}\",\"name\":\"{}\",\"summary\":\"{}\",\"confidence\":\"{}\",\"producer_stage\":{},\"producer_pass\":{},\"data\":{}}}",
        escape_json_string(&item.kind),
        escape_json_string(&item.name),
        escape_json_string(&item.summary),
        escape_json_string(&item.confidence),
        producer_stage,
        producer_pass,
        item.data_json.clone().unwrap_or_else(|| "null".into()),
    )
}

fn escape_json_string(input: &str) -> String {
    input.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::advisory::{MockMlAdvisoryEngine, MockRecommendationAugmenter, MockScoreRerankPass};

    #[test]
    fn analyzes_gewyvern_analysis_json_via_engine() {
        let output = analyze_gewyvern_analysis_json(
            &MockMlAdvisoryEngine,
            "{\"primary_module_kind\":\"http_request_response\",\"primary_failure_mode\":\"no_response\",\"primary_failure_detail\":\"request_sent_no_reply\",\"primary_failure_confidence\":\"medium\",\"primary_failure_basis\":\"missing_transition\",\"ambiguous\":false,\"competing_hypotheses\":[]}",
        )
        .expect("analysis json should parse");

        assert_eq!(output.augmentations.len(), 1);
        assert_eq!(output.augmentations[0].name, "ml_candidate_observe_longer");
        assert_eq!(
            output.augmentations[0].producer_stage.as_deref(),
            Some("candidate")
        );
        assert_eq!(
            output.augmentations[0].producer_pass.as_deref(),
            Some("MockMlAdvisoryEngine")
        );
        let json = engine_output_json(&output);
        assert!(json.contains("\"augmentations\":["));
        assert!(json.contains("\"name\":\"ml_candidate_observe_longer\""));
        assert!(json.contains("\"producer_stage\":\"candidate\""));
    }

    #[test]
    fn pass_pipeline_layers_candidates_recommendations_and_rerank() {
        let snapshot = AnalysisSnapshotInput::from_core_fields(
            "http_request_response",
            "no_response",
            "request_sent_no_reply",
            "medium",
            "missing_transition",
        );
        let pipeline = PassPipeline::new()
            .with_candidate(&MockMlAdvisoryEngine)
            .with_recommendation(&MockRecommendationAugmenter)
            .with_rerank(&MockScoreRerankPass);

        let output = pipeline.analyze(&snapshot);
        assert_eq!(output.augmentations.len(), 2);
        assert_eq!(output.augmentations[0].name, "ml_candidate_observe_longer");
        assert_eq!(output.augmentations[0].confidence, "ranked-candidate");
        assert_eq!(output.augmentations[1].name, "ml_recommend_manual_queue");
        assert_eq!(
            output.augmentations[0].producer_stage.as_deref(),
            Some("candidate")
        );
        assert_eq!(
            output.augmentations[1].producer_stage.as_deref(),
            Some("recommendation")
        );
    }
}
