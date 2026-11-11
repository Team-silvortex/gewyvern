#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AnalysisAugmentation {
    pub kind: String,
    pub name: String,
    pub summary: String,
    pub confidence: String,
    pub producer_stage: Option<String>,
    pub producer_pass: Option<String>,
    pub data_json: Option<String>,
}

impl AnalysisAugmentation {
    pub fn new(
        kind: impl Into<String>,
        name: impl Into<String>,
        summary: impl Into<String>,
        confidence: impl Into<String>,
        data_json: Option<String>,
    ) -> Self {
        Self {
            kind: kind.into(),
            name: name.into(),
            summary: summary.into(),
            confidence: confidence.into(),
            producer_stage: None,
            producer_pass: None,
            data_json,
        }
    }

    pub fn with_producer(mut self, stage: impl Into<String>, pass: impl Into<String>) -> Self {
        self.producer_stage = Some(stage.into());
        self.producer_pass = Some(pass.into());
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SnapshotParseError {
    pub message: String,
}

impl SnapshotParseError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize)]
pub struct AnalysisSnapshotInput {
    pub primary_module_kind: String,
    pub primary_failure_mode: String,
    pub primary_failure_detail: String,
    pub primary_failure_confidence: String,
    pub primary_failure_basis: String,
    pub ambiguous: bool,
    pub competing_hypotheses: Vec<String>,
    #[serde(skip)]
    pub augmentations: Vec<AnalysisAugmentation>,
}

impl AnalysisSnapshotInput {
    pub fn from_core_fields(
        primary_module_kind: impl Into<String>,
        primary_failure_mode: impl Into<String>,
        primary_failure_detail: impl Into<String>,
        primary_failure_confidence: impl Into<String>,
        primary_failure_basis: impl Into<String>,
    ) -> Self {
        Self {
            primary_module_kind: primary_module_kind.into(),
            primary_failure_mode: primary_failure_mode.into(),
            primary_failure_detail: primary_failure_detail.into(),
            primary_failure_confidence: primary_failure_confidence.into(),
            primary_failure_basis: primary_failure_basis.into(),
            ambiguous: false,
            competing_hypotheses: Vec::new(),
            augmentations: Vec::new(),
        }
    }

    pub fn from_gewyvern_analysis_json(input: &str) -> Result<Self, SnapshotParseError> {
        serde_json::from_str(input)
            .map_err(|_| SnapshotParseError::new("invalid analysis snapshot JSON"))
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EngineOutput {
    pub augmentations: Vec<AnalysisAugmentation>,
}

impl EngineOutput {
    pub fn with_augmentation(augmentation: AnalysisAugmentation) -> Self {
        Self {
            augmentations: vec![augmentation],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_gewyvern_analysis_json_shape() {
        let snapshot = AnalysisSnapshotInput::from_gewyvern_analysis_json(
            "{\"primary_module_kind\":\"http_request_response\",\"primary_failure_mode\":\"no_response\",\"primary_failure_detail\":\"request_sent_no_reply\",\"primary_failure_confidence\":\"medium\",\"primary_failure_basis\":\"missing_transition\",\"ambiguous\":true,\"competing_hypotheses\":[\"module:dns\",\"module:tls\"]}",
        )
        .expect("json should parse");

        assert_eq!(snapshot.primary_module_kind, "http_request_response");
        assert_eq!(snapshot.primary_failure_mode, "no_response");
        assert_eq!(snapshot.primary_failure_detail, "request_sent_no_reply");
        assert_eq!(snapshot.primary_failure_confidence, "medium");
        assert_eq!(snapshot.primary_failure_basis, "missing_transition");
        assert!(snapshot.ambiguous);
        assert_eq!(
            snapshot.competing_hypotheses,
            vec!["module:dns".to_string(), "module:tls".to_string()]
        );
    }

    #[test]
    fn augmentation_can_record_producer_metadata() {
        let augmentation = AnalysisAugmentation::new(
            "ml-candidate",
            "ml_candidate_observe_longer",
            "candidate summary",
            "candidate",
            None,
        )
        .with_producer("candidate", "MockMlAdvisoryEngine");

        assert_eq!(augmentation.producer_stage.as_deref(), Some("candidate"));
        assert_eq!(
            augmentation.producer_pass.as_deref(),
            Some("MockMlAdvisoryEngine")
        );
    }
}
