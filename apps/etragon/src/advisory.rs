use crate::contract::{AnalysisAugmentation, AnalysisSnapshotInput, EngineOutput};
use crate::engine::{
    CandidateAugmenter, ExternalAnalysisEngine, PassPipeline, RecommendationAugmenter, RerankPass,
};

#[derive(Clone, Debug, Default)]
pub struct MockMlAdvisoryEngine;

#[derive(Clone, Debug, Default)]
pub struct MockRecommendationAugmenter;

#[derive(Clone, Debug, Default)]
pub struct MockScoreRerankPass;

impl CandidateAugmenter for MockMlAdvisoryEngine {
    fn candidate_augmentations(
        &self,
        snapshot: &AnalysisSnapshotInput,
    ) -> Vec<AnalysisAugmentation> {
        let augmentation = if snapshot.ambiguous && !snapshot.competing_hypotheses.is_empty() {
            AnalysisAugmentation::new(
                "ml-candidate",
                "ml_candidate_multi_hypothesis",
                "multiple plausible hypotheses remain; keep the candidate set open for later rerank",
                "candidate",
                Some(format!(
                    "{{\"module\":\"{}\",\"competing_hypotheses\":{}}}",
                    snapshot.primary_module_kind,
                    json_string_list(&snapshot.competing_hypotheses)
                )),
            )
        } else if snapshot.primary_failure_confidence == "medium"
            && snapshot.primary_failure_basis == "missing_transition"
        {
            AnalysisAugmentation::new(
                "ml-candidate",
                "ml_candidate_observe_longer",
                "the signal is consistent with a timeout-shaped path; observe a longer runtime window before narrowing further",
                "candidate",
                Some(format!(
                    "{{\"module\":\"{}\",\"failure_detail\":\"{}\"}}",
                    snapshot.primary_module_kind, snapshot.primary_failure_detail
                )),
            )
        } else if snapshot.primary_failure_confidence == "high"
            && snapshot.primary_failure_basis == "direct_protocol_signal"
        {
            AnalysisAugmentation::new(
                "ml-candidate",
                "ml_candidate_targeted_escalation",
                "the protocol signal is direct enough to support targeted downstream escalation or correlation",
                "candidate",
                Some(format!(
                    "{{\"module\":\"{}\",\"failure_mode\":\"{}\"}}",
                    snapshot.primary_module_kind, snapshot.primary_failure_mode
                )),
            )
        } else {
            AnalysisAugmentation::new(
                "ml-candidate",
                "ml_candidate_manual_review",
                "the current signal is still advisory; keep the case available for manual or model-assisted review",
                "candidate",
                Some(format!(
                    "{{\"module\":\"{}\",\"failure_confidence\":\"{}\",\"failure_basis\":\"{}\"}}",
                    snapshot.primary_module_kind,
                    snapshot.primary_failure_confidence,
                    snapshot.primary_failure_basis
                )),
            )
        };

        vec![augmentation]
    }

    fn pass_name(&self) -> &'static str {
        "MockMlAdvisoryEngine"
    }
}

impl RecommendationAugmenter for MockRecommendationAugmenter {
    fn recommendation_augmentations(
        &self,
        snapshot: &AnalysisSnapshotInput,
        current: &[AnalysisAugmentation],
    ) -> Vec<AnalysisAugmentation> {
        let first_candidate = current
            .first()
            .map(|item| item.name.as_str())
            .unwrap_or("none");
        let recommendation = if snapshot.ambiguous {
            AnalysisAugmentation::new(
                "ml-recommendation",
                "ml_recommend_keep_candidate_set_open",
                "keep the candidate set open and defer hard remediation until rerank or more evidence arrives",
                "candidate",
                Some(format!(
                    "{{\"primary_module_kind\":\"{}\",\"seed_candidate\":\"{}\"}}",
                    snapshot.primary_module_kind, first_candidate
                )),
            )
        } else {
            AnalysisAugmentation::new(
                "ml-recommendation",
                "ml_recommend_manual_queue",
                "queue the case for a manual or model-assisted follow-up step after candidate generation",
                "candidate",
                Some(format!(
                    "{{\"primary_module_kind\":\"{}\",\"seed_candidate\":\"{}\"}}",
                    snapshot.primary_module_kind, first_candidate
                )),
            )
        };
        vec![recommendation]
    }

    fn pass_name(&self) -> &'static str {
        "MockRecommendationAugmenter"
    }
}

impl RerankPass for MockScoreRerankPass {
    fn rerank(&self, _snapshot: &AnalysisSnapshotInput, current: &mut Vec<AnalysisAugmentation>) {
        if let Some(first) = current.first_mut() {
            first.confidence = "ranked-candidate".into();
        }
    }

    fn pass_name(&self) -> &'static str {
        "MockScoreRerankPass"
    }
}

impl ExternalAnalysisEngine for MockMlAdvisoryEngine {
    fn analyze(&self, snapshot: &AnalysisSnapshotInput) -> EngineOutput {
        let pipeline = PassPipeline::new().with_candidate(self);
        pipeline.analyze(snapshot)
    }
}

fn json_string_list(items: &[String]) -> String {
    let body = items
        .iter()
        .map(|item| format!("\"{}\"", item.replace('\\', "\\\\").replace('"', "\\\"")))
        .collect::<Vec<_>>()
        .join(",");
    format!("[{}]", body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_engine_prefers_multi_hypothesis_signal() {
        let mut snapshot = AnalysisSnapshotInput::from_core_fields(
            "http_request_response",
            "no_response",
            "request_sent_no_reply",
            "low",
            "missing_transition",
        );
        snapshot.ambiguous = true;
        snapshot.competing_hypotheses = vec![
            "module:name_resolution".into(),
            "module:tls_handshake".into(),
        ];

        let output = MockMlAdvisoryEngine.analyze(&snapshot);
        assert_eq!(output.augmentations.len(), 1);
        assert_eq!(
            output.augmentations[0].name,
            "ml_candidate_multi_hypothesis"
        );
        assert_eq!(
            output.augmentations[0].producer_pass.as_deref(),
            Some("MockMlAdvisoryEngine")
        );
    }

    #[test]
    fn mock_engine_emits_timeout_candidate_for_missing_transition() {
        let snapshot = AnalysisSnapshotInput::from_core_fields(
            "http_request_response",
            "no_response",
            "request_sent_no_reply",
            "medium",
            "missing_transition",
        );

        let output = MockMlAdvisoryEngine.analyze(&snapshot);
        assert_eq!(output.augmentations[0].name, "ml_candidate_observe_longer");
    }

    #[test]
    fn mock_engine_emits_targeted_candidate_for_direct_signal() {
        let snapshot = AnalysisSnapshotInput::from_core_fields(
            "authentication_exchange",
            "server_denied",
            "access_denied",
            "high",
            "direct_protocol_signal",
        );

        let output = MockMlAdvisoryEngine.analyze(&snapshot);
        assert_eq!(
            output.augmentations[0].name,
            "ml_candidate_targeted_escalation"
        );
    }

    #[test]
    fn recommendation_augmenter_uses_existing_candidates() {
        let snapshot = AnalysisSnapshotInput::from_core_fields(
            "http_request_response",
            "no_response",
            "request_sent_no_reply",
            "medium",
            "missing_transition",
        );
        let candidates = MockMlAdvisoryEngine.candidate_augmentations(&snapshot);
        let recommendations =
            MockRecommendationAugmenter.recommendation_augmentations(&snapshot, &candidates);
        assert_eq!(recommendations.len(), 1);
        assert_eq!(recommendations[0].name, "ml_recommend_manual_queue");
        assert!(
            recommendations[0]
                .data_json
                .as_deref()
                .unwrap_or_default()
                .contains("ml_candidate_observe_longer")
        );
    }
}
