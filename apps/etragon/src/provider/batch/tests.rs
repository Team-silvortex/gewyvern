use super::*;
use serde_json::json;

fn config() -> ProviderConfig {
    ProviderConfig::from_json(
        r#"{"id":"test","model":"fixture","endpoint":"http://127.0.0.1:1/v1/chat/completions"}"#,
    )
    .unwrap()
}
fn index(names: &[&str]) -> String {
    json!({"target_refs":names.iter().map(|name| json!({"path_segment":name})).collect::<Vec<_>>()})
        .to_string()
}
fn input() -> String {
    include_str!("../../../tests/fixtures/missing_transition_analysis.json").to_owned()
}
fn output() -> EngineOutput {
    EngineOutput::with_augmentation(crate::AnalysisAugmentation::new(
        "external-advisory", "test", "candidate", "candidate",
        Some(json!({"target_path_segment":"forged","target_incarnation":999,"source_scope":"forged"}).to_string()),
    ))
}

#[test]
fn budget_skips_targets_without_fetching_and_stamps_nonresident_provenance() {
    let mut fetched = Vec::new();
    let mut calls = 0;
    let report = analyze_index(
        &index(&["c", "b", "a"]),
        &config(),
        &BatchOptions::default(),
        |name| {
            fetched.push(name.to_owned());
            Ok(input())
        },
        |_| {
            calls += 1;
            Ok(output())
        },
    )
    .unwrap();
    assert_eq!(fetched, ["a"]);
    assert_eq!(calls, 1);
    assert_eq!(report.analysis_runs, 1);
    assert_eq!(report.status, "partial");
    assert!(!report.complete);
    assert_eq!(report.skipped_count, 2);
    assert_eq!(
        report.targets[1].error.as_ref().unwrap().code,
        "etragon_provider_inference_budget_exhausted"
    );
    let data = &report.targets[0].output.as_ref().unwrap()["augmentations"][0]["data"];
    assert_eq!(data["target_path_segment"], "a");
    assert_eq!(data["source_scope"], "one-shot-target-index");
    assert!(data["target_incarnation"].is_null());
    assert!(report.to_json().unwrap().len() < MAX_BATCH_OUTPUT_BYTES);
}

#[test]
fn source_and_evidence_failures_do_not_spend_inference_budget_or_hide_success() {
    let report = analyze_index(
        &index(&["a", "b", "c"]),
        &config(),
        &BatchOptions::default(),
        |name| match name {
            "a" => Err(ProviderError::HttpStatus(503)),
            "b" => Ok("{}".into()),
            _ => Ok(input()),
        },
        |_| Ok(output()),
    )
    .unwrap();
    assert_eq!(report.analysis_runs, 1);
    assert_eq!(report.failed_count, 2);
    assert_eq!(report.ready_count, 1);
    assert_eq!(report.skipped_count, 0);
    assert_eq!(report.targets[0].error.as_ref().unwrap().stage, "source");
    assert_eq!(
        report.targets[0].error.as_ref().unwrap().http_status,
        Some(503)
    );
    assert_eq!(report.targets[1].error.as_ref().unwrap().stage, "evidence");
    assert!(report.targets[0].output.is_none());
    assert!(report.targets[2].output.is_some());
}

#[test]
fn provider_failures_spend_budget_without_retry_and_identical_targets_are_not_deduplicated() {
    let options = BatchOptions {
        max_inferences: 2,
        ..Default::default()
    };
    let mut calls = 0;
    let report = analyze_index(
        &index(&["a", "b", "c"]),
        &config(),
        &options,
        |_| Ok(input()),
        |_| {
            calls += 1;
            if calls == 1 {
                Err(ProviderError::HttpStatus(429))
            } else {
                Ok(output())
            }
        },
    )
    .unwrap();
    assert_eq!(calls, 2);
    assert_eq!(report.analysis_runs, 2);
    assert_eq!(report.ready_count, 1);
    assert_eq!(report.failed_count, 1);
    assert_eq!(report.skipped_count, 1);
    assert_eq!(report.targets[0].error.as_ref().unwrap().stage, "provider");
    assert_eq!(
        report.targets[0].error.as_ref().unwrap().http_status,
        Some(429)
    );
}

#[test]
fn empty_selection_is_complete_but_malformed_or_oversized_indexes_do_no_work() {
    let options = BatchOptions::default();
    let report = analyze_index(
        &index(&[]),
        &config(),
        &options,
        |_| panic!("empty index must not fetch"),
        |_| panic!("empty index must not infer"),
    )
    .unwrap();
    assert!(report.complete);
    assert_eq!(report.status, "ready");
    assert_eq!(report.target_count, 0);
    assert_eq!(report.analysis_runs, 0);
    for input in [
        index(&["a", "a"]),
        index(&["../a"]),
        "{}".into(),
        index(&["a", "b"]),
    ] {
        assert!(
            analyze_index(
                &input,
                &config(),
                &BatchOptions {
                    max_targets: 1,
                    ..Default::default()
                },
                |_| panic!("invalid index must not fetch"),
                |_| panic!("invalid index must not infer")
            )
            .is_err()
        );
    }
    let report = analyze_index(
        &index(&["skip", "scan:a"]),
        &config(),
        &BatchOptions {
            filter: Some("scan:".into()),
            max_targets: 1,
            ..Default::default()
        },
        |_| Ok(input()),
        |_| Ok(output()),
    )
    .unwrap();
    assert!(report.complete);
    assert_eq!(report.target_count, 1);
}

#[test]
fn output_budget_retains_completed_targets_and_stops_additional_model_calls() {
    let options = BatchOptions {
        max_inferences: 8,
        ..Default::default()
    };
    let mut calls = 0;
    let report = analyze_index(
        &index(&["a", "b", "c", "d"]),
        &config(),
        &options,
        |_| Ok(input()),
        |_| {
            calls += 1;
            let mut output = output();
            output.augmentations[0].summary = "x".repeat(MAX_ADVICE_BYTES / 2);
            Ok(output)
        },
    )
    .unwrap();
    assert_eq!(calls, 2);
    assert_eq!(report.ready_count, 1);
    assert_eq!(report.failed_count, 1);
    assert_eq!(report.skipped_count, 2);
    assert_eq!(report.targets[1].error.as_ref().unwrap().stage, "output");
    assert_eq!(
        report.targets[2].error.as_ref().unwrap().code,
        "etragon_provider_batch_output_limit"
    );
    assert!(report.targets[1].output.is_none());
    assert!(report.to_json().unwrap().len() <= MAX_BATCH_OUTPUT_BYTES);
}

#[test]
fn exhausted_batches_are_unavailable_and_invalid_limits_are_rejected() {
    let report = analyze_index(
        &index(&["a", "b"]),
        &config(),
        &BatchOptions::default(),
        |_| Ok(input()),
        |_| Err(ProviderError::MissingCredential),
    )
    .unwrap();
    assert_eq!(report.status, "unavailable");
    assert!(!report.complete);
    assert_eq!(report.analysis_runs, 1);
    assert_eq!(report.failed_count, 1);
    assert_eq!(report.skipped_count, 1);
    for options in [
        BatchOptions {
            max_inferences: 0,
            ..Default::default()
        },
        BatchOptions {
            max_inferences: 9,
            ..Default::default()
        },
        BatchOptions {
            max_targets: 65,
            ..Default::default()
        },
        BatchOptions {
            max_targets: 1,
            max_inferences: 2,
            ..Default::default()
        },
        BatchOptions {
            filter: Some("\n".into()),
            ..Default::default()
        },
    ] {
        assert!(options.validate().is_err());
    }
}
