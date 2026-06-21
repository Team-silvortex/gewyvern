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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AnalysisSnapshotInput {
    pub primary_module_kind: String,
    pub primary_failure_mode: String,
    pub primary_failure_detail: String,
    pub primary_failure_confidence: String,
    pub primary_failure_basis: String,
    pub ambiguous: bool,
    pub competing_hypotheses: Vec<String>,
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
        Ok(Self {
            primary_module_kind: extract_required_json_string(input, "primary_module_kind")?,
            primary_failure_mode: extract_required_json_string(input, "primary_failure_mode")?,
            primary_failure_detail: extract_required_json_string(input, "primary_failure_detail")?,
            primary_failure_confidence: extract_required_json_string(
                input,
                "primary_failure_confidence",
            )?,
            primary_failure_basis: extract_required_json_string(input, "primary_failure_basis")?,
            ambiguous: extract_required_json_bool(input, "ambiguous")?,
            competing_hypotheses: extract_json_string_array(input, "competing_hypotheses")?,
            augmentations: Vec::new(),
        })
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

fn extract_required_json_string(input: &str, key: &str) -> Result<String, SnapshotParseError> {
    let needle = format!("\"{}\":\"", key);
    let start = input
        .find(&needle)
        .ok_or_else(|| SnapshotParseError::new(format!("missing string field '{}'", key)))?
        + needle.len();
    let rest = &input[start..];
    let end = rest
        .find('"')
        .ok_or_else(|| SnapshotParseError::new(format!("unterminated string field '{}'", key)))?;
    Ok(rest[..end].to_string())
}

fn extract_required_json_bool(input: &str, key: &str) -> Result<bool, SnapshotParseError> {
    let needle = format!("\"{}\":", key);
    let start = input
        .find(&needle)
        .ok_or_else(|| SnapshotParseError::new(format!("missing bool field '{}'", key)))?
        + needle.len();
    let rest = &input[start..];
    if rest.starts_with("true") {
        Ok(true)
    } else if rest.starts_with("false") {
        Ok(false)
    } else {
        Err(SnapshotParseError::new(format!(
            "invalid bool field '{}'",
            key
        )))
    }
}

fn extract_json_string_array(input: &str, key: &str) -> Result<Vec<String>, SnapshotParseError> {
    let needle = format!("\"{}\":[", key);
    let start = input
        .find(&needle)
        .ok_or_else(|| SnapshotParseError::new(format!("missing array field '{}'", key)))?
        + needle.len();
    let rest = &input[start..];
    let end = rest
        .find(']')
        .ok_or_else(|| SnapshotParseError::new(format!("unterminated array field '{}'", key)))?;
    let inner = &rest[..end];
    if inner.trim().is_empty() {
        return Ok(Vec::new());
    }
    Ok(inner
        .split(',')
        .map(|item| item.trim().trim_matches('"').to_string())
        .collect())
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
