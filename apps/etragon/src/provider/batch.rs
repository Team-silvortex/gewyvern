//! Bounded one-shot inference with explicit per-target failure and budget outcomes.
use super::{
    AdvisoryRequest, ChatCompletionsProvider, ProviderConfig, ProviderError,
    targets::{MAX_TARGETS, TargetIndexSource, parse_target_index, stamp_target_output},
};
use crate::{EngineOutput, engine_output_json};
use serde::Serialize;
use serde_json::Value;

pub const MAX_BATCH_INFERENCES: usize = 8;
pub const MAX_BATCH_OUTPUT_BYTES: usize = 1024 * 1024;
// Leave room for bounded target metadata independently of provider output sizes.
const MAX_ADVICE_BYTES: usize = MAX_BATCH_OUTPUT_BYTES / 2;

pub struct BatchOptions {
    pub filter: Option<String>,
    pub max_targets: usize,
    pub max_inferences: usize,
}

impl Default for BatchOptions {
    fn default() -> Self {
        Self {
            filter: None,
            max_targets: 24,
            max_inferences: 1,
        }
    }
}

impl BatchOptions {
    pub fn validate(&self) -> Result<(), ProviderError> {
        if !(1..=MAX_TARGETS).contains(&self.max_targets)
            || !(1..=MAX_BATCH_INFERENCES.min(self.max_targets)).contains(&self.max_inferences)
            || self.filter.as_ref().is_some_and(|filter| {
                filter.is_empty() || filter.len() > 256 || filter.chars().any(char::is_control)
            })
        {
            return Err(ProviderError::InvalidConfig);
        }
        Ok(())
    }
}

#[derive(Serialize)]
pub struct BatchReport {
    pub schema_version: &'static str,
    pub mode: &'static str,
    pub provider_id: String,
    pub model: String,
    pub status: &'static str,
    pub complete: bool,
    pub target_count: usize,
    pub ready_count: usize,
    pub failed_count: usize,
    pub skipped_count: usize,
    pub analysis_runs: usize,
    pub max_inferences: usize,
    pub advisory_only: bool,
    pub operator_review_required: bool,
    pub targets: Vec<BatchTarget>,
}

#[derive(Serialize)]
pub struct BatchTarget {
    pub path_segment: String,
    pub status: &'static str,
    pub error: Option<BatchError>,
    pub output: Option<Value>,
}

#[derive(Serialize)]
pub struct BatchError {
    pub stage: &'static str,
    pub code: &'static str,
    pub http_status: Option<u16>,
}

impl BatchTarget {
    fn fail(&mut self, stage: &'static str, error: ProviderError) {
        self.status = "error";
        self.error = Some(BatchError {
            stage,
            code: error.code(),
            http_status: match error {
                ProviderError::HttpStatus(status) => Some(status),
                _ => None,
            },
        });
    }
}

impl BatchReport {
    pub fn to_json(&self) -> Result<String, ProviderError> {
        let output = serde_json::to_string(self).map_err(|_| ProviderError::InvalidResponse)?;
        if output.len() > MAX_BATCH_OUTPUT_BYTES {
            return Err(ProviderError::TooLarge);
        }
        Ok(output)
    }
}

pub fn analyze_target_batch(
    source: &TargetIndexSource,
    provider: &ChatCompletionsProvider,
    options: &BatchOptions,
) -> Result<BatchReport, ProviderError> {
    options.validate()?;
    let index = source.fetch_index()?;
    analyze_index(
        &index,
        &provider.config,
        options,
        |name| source.fetch_target(name),
        |input| provider.analyze_json(input),
    )
}

fn analyze_index(
    index: &str,
    config: &ProviderConfig,
    options: &BatchOptions,
    mut fetch: impl FnMut(&str) -> Result<String, ProviderError>,
    mut infer: impl FnMut(&str) -> Result<EngineOutput, ProviderError>,
) -> Result<BatchReport, ProviderError> {
    options.validate()?;
    let names = parse_target_index(index, options.filter.as_deref(), options.max_targets)?;
    let mut report = BatchReport {
        schema_version: "etragon-provider-batch-v1",
        mode: "external-provider-batch",
        provider_id: config.id.clone(),
        model: config.model.clone(),
        status: "ready",
        complete: true,
        target_count: names.len(),
        ready_count: 0,
        failed_count: 0,
        skipped_count: 0,
        analysis_runs: 0,
        max_inferences: options.max_inferences,
        advisory_only: true,
        operator_review_required: true,
        targets: Vec::with_capacity(names.len()),
    };
    let mut advice_bytes = 0;
    let mut output_exhausted = false;
    for name in names {
        let mut target = BatchTarget {
            path_segment: name,
            status: "skipped",
            output: None,
            error: Some(BatchError {
                stage: "budget",
                code: if output_exhausted {
                    "etragon_provider_batch_output_limit"
                } else {
                    "etragon_provider_inference_budget_exhausted"
                },
                http_status: None,
            }),
        };
        if !output_exhausted && report.analysis_runs < options.max_inferences {
            match fetch(&target.path_segment) {
                Err(error) => target.fail("source", error),
                Ok(input) => match AdvisoryRequest::from_analysis_json(&input) {
                    Err(error) => target.fail("evidence", error),
                    Ok(_) => {
                        report.analysis_runs += 1;
                        match infer(&input).and_then(|mut output| {
                            stamp_target_output(&mut output, &target.path_segment, None)?;
                            Ok(engine_output_json(&output))
                        }) {
                            Err(error) => target.fail("provider", error),
                            Ok(output) if output.len() > MAX_ADVICE_BYTES - advice_bytes => {
                                target.fail("output", ProviderError::TooLarge);
                                output_exhausted = true;
                            }
                            Ok(output) => {
                                advice_bytes += output.len();
                                target.output = Some(
                                    serde_json::from_str(&output)
                                        .map_err(|_| ProviderError::InvalidResponse)?,
                                );
                                target.status = "ready";
                                target.error = None;
                            }
                        }
                    }
                },
            }
        }
        match target.status {
            "ready" => report.ready_count += 1,
            "error" => report.failed_count += 1,
            _ => report.skipped_count += 1,
        }
        report.targets.push(target);
    }
    report.complete = report.ready_count == report.target_count;
    report.status = if report.complete {
        "ready"
    } else if report.ready_count > 0 {
        "partial"
    } else {
        "unavailable"
    };
    Ok(report)
}

#[cfg(test)]
mod tests;
