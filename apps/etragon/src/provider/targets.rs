//! Target discovery is data, never authority to select another origin or URL.
use super::{AnalysisSource, MAX_RESPONSE_BYTES, ProviderError};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::BTreeSet, path::PathBuf};

pub const MAX_INDEX_TARGETS: usize = 256;
pub const MAX_TARGETS: usize = 64;

pub struct TargetIndexSource {
    source: AnalysisSource,
}

impl TargetIndexSource {
    pub fn new(endpoint: String, ca_bundle: Option<PathBuf>) -> Result<Self, ProviderError> {
        let uri: http::Uri = endpoint.parse().map_err(|_| ProviderError::InvalidConfig)?;
        if uri.path() != "/v1/latest/targets" {
            return Err(ProviderError::InvalidConfig);
        }
        Ok(Self {
            source: AnalysisSource::new(endpoint, ca_bundle)?,
        })
    }

    pub fn fetch_index(&self) -> Result<String, ProviderError> {
        self.source.fetch_json()
    }

    pub fn fetch_target(&self, segment: &str) -> Result<String, ProviderError> {
        let path = encoded_target_segment(segment)?;
        // The validated index endpoint has no query, fragment or userinfo.
        let endpoint = format!("{}/{path}/analysis.json", self.source.endpoint);
        AnalysisSource::new(endpoint, self.source.ca_bundle.clone())?.fetch_json()
    }
}

#[derive(Deserialize)]
struct TargetIndex {
    target_refs: Vec<TargetRef>,
    target_count: Option<usize>,
}

#[derive(Deserialize)]
struct TargetRef {
    path_segment: String,
}

pub fn parse_target_index(
    input: &str,
    filter: Option<&str>,
    limit: usize,
) -> Result<Vec<String>, ProviderError> {
    if input.len() > MAX_RESPONSE_BYTES {
        return Err(ProviderError::TooLarge);
    }
    if !(1..=MAX_TARGETS).contains(&limit) {
        return Err(ProviderError::InvalidConfig);
    }
    let index: TargetIndex =
        serde_json::from_str(input).map_err(|_| ProviderError::InvalidResponse)?;
    if index.target_refs.len() > MAX_INDEX_TARGETS {
        return Err(ProviderError::TooLarge);
    }
    if index
        .target_count
        .is_some_and(|count| count != index.target_refs.len())
    {
        return Err(ProviderError::InvalidResponse);
    }
    let mut all = BTreeSet::new();
    for target in index.target_refs {
        encoded_target_segment(&target.path_segment)?;
        if !all.insert(target.path_segment) {
            return Err(ProviderError::InvalidResponse);
        }
    }
    let selected = all
        .into_iter()
        .filter(|name| filter.is_none_or(|filter| name.starts_with(filter)))
        .collect::<Vec<_>>();
    if selected.len() > limit {
        return Err(ProviderError::TooLarge);
    }
    Ok(selected)
}

pub fn encoded_target_segment(segment: &str) -> Result<String, ProviderError> {
    if segment.is_empty()
        || segment.len() > 256
        || matches!(segment, "." | "..")
        || segment
            .chars()
            .any(|c| c.is_control() || matches!(c, '/' | '\\'))
    {
        return Err(ProviderError::InvalidResponse);
    }
    let mut encoded = String::with_capacity(segment.len());
    for byte in segment.bytes() {
        if byte.is_ascii_alphanumeric() || b"._~:-".contains(&byte) {
            encoded.push(byte as char);
        } else {
            use std::fmt::Write;
            write!(encoded, "%{byte:02X}").map_err(|_| ProviderError::InvalidResponse)?;
        }
    }
    Ok(encoded)
}

pub fn stamp_target_output(
    output: &mut crate::EngineOutput,
    name: &str,
    incarnation: Option<u64>,
) -> Result<(), ProviderError> {
    encoded_target_segment(name)?;
    for item in &mut output.augmentations {
        let mut data = match item.data_json.as_deref() {
            Some(data) => {
                serde_json::from_str::<Value>(data).map_err(|_| ProviderError::InvalidResponse)?
            }
            None => json!({}),
        };
        let object = data.as_object_mut().ok_or(ProviderError::InvalidResponse)?;
        object.insert("target_path_segment".into(), json!(name));
        object.insert("target_incarnation".into(), json!(incarnation));
        object.insert(
            "source_scope".into(),
            json!(if incarnation.is_some() {
                "resident-target-index"
            } else {
                "one-shot-target-index"
            }),
        );
        item.data_json = Some(data.to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn index_is_structural_bounded_unique_and_filterable() {
        let input = include_str!("../../tests/fixtures/targets_index.json");
        assert_eq!(
            parse_target_index(input, Some("scan:"), 1).unwrap(),
            ["scan:http:request"]
        );
        assert_eq!(
            parse_target_index(r#"{"target_count":0,"target_refs":[]}"#, None, 1).unwrap(),
            Vec::<String>::new()
        );
        assert!(parse_target_index(input, None, 1).is_err());
        for value in [
            json!({"nested":{"target_refs":[{"path_segment":"x"}]}}),
            json!({"target_count":2,"target_refs":[{"path_segment":"x"}]}),
            json!({"target_refs":[{"path_segment":"x"},{"path_segment":"x"}]}),
            json!({"target_refs":[{"path_segment":"../secret"}]}),
        ] {
            assert!(parse_target_index(&value.to_string(), None, 24).is_err());
        }
        let huge = json!({"target_refs":(0..257).map(|n| json!({"path_segment":format!("t{n}")})).collect::<Vec<_>>()});
        assert_eq!(
            parse_target_index(&huge.to_string(), Some("absent"), 1).unwrap_err(),
            ProviderError::TooLarge
        );
        assert!(parse_target_index(r#"{"target_refs":[],"target_refs":[]}"#, None, 24).is_err());
    }

    #[test]
    fn advertised_urls_are_ignored_and_segments_are_encoded_once() {
        let input = r#"{"target_refs":[{"path_segment":"scan:?%2F# space","url_path":"https://evil.invalid/steal"}]}"#;
        let names = parse_target_index(input, None, 24).unwrap();
        assert_eq!(
            encoded_target_segment(&names[0]).unwrap(),
            "scan:%3F%252F%23%20space"
        );
        for bad in ["", ".", "..", "a/b", "a\\b", "a\n"] {
            assert!(encoded_target_segment(bad).is_err());
        }
        for url in [
            "http://127.0.0.1/other",
            "http://127.0.0.1/v1/latest/targets?next=evil",
            "http://u:p@127.0.0.1/v1/latest/targets",
        ] {
            assert!(TargetIndexSource::new(url.into(), None).is_err());
        }
    }
}
