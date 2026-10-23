use super::*;
use crate::{
    BranchOutcome, EffectErrorClass, FailedEffect, MergeWait, StructuredEffectBatch,
    StructuredField, Value,
};

// Frozen journal policy: use the pre-migration value traversal and literal
// limits, not the migrated terminal/token/retry validators.
fn frozen_terminal(step: &Step) -> Result<(), Fault> {
    let message = match step {
        Step::Done(value) => {
            return crate::value_validation_tests::legacy_validate_value(value, 1)
                .map(|_| ())
                .map_err(|_| {
                    journal_fault("LSV4007", "completed journal output exceeds runtime limit")
                });
        }
        Step::Cancelled(c)
            if !c.continuation.as_str().is_empty() && c.observed_at_ms <= i64::MAX as u64 =>
        {
            return Ok(());
        }
        Step::Failed(f)
            if f.retry_count <= 16
                && !f.error.code.is_empty()
                && f.error.code.len() <= 64
                && f.error
                    .code
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
                && !f.error.message.is_empty()
                && f.error.message.len() <= 1024 =>
        {
            return Ok(());
        }
        Step::Fault(f) if !f.code.is_empty() && f.code.len() <= 32 => return Ok(()),
        Step::Effect(_) | Step::Effects(_) | Step::Waiting(_) | Step::Yield(_) => {
            "journal completion must be a terminal step"
        }
        _ => "journal fault has an invalid diagnostic code",
    };
    Err(journal_fault("LSV4007", message))
}

fn frozen_outcome(step: Step) -> Result<BranchOutcome, Fault> {
    frozen_terminal(&step)?;
    Ok(match step {
        Step::Done(value) => BranchOutcome::Value(value),
        Step::Cancelled(value) => BranchOutcome::Cancelled(value),
        Step::Failed(value) => BranchOutcome::Failed(value),
        Step::Fault(value) => BranchOutcome::Fault(value),
        _ => unreachable!(),
    })
}

fn frozen_sequence<'a>(steps: impl IntoIterator<Item = &'a Step>) -> Result<Option<Step>, Fault> {
    let mut items = 0;
    for (index, step) in steps.into_iter().enumerate() {
        frozen_terminal(step)?;
        if index >= 64 {
            return Err(journal_fault(
                "LSV4007",
                "sequential result observations exceed runtime bounds",
            ));
        }
        let Step::Done(value) = step else {
            return Ok(Some(step.clone()));
        };
        items += crate::value_validation_tests::legacy_validate_value(value, 0)?;
        if items > 10_000 {
            return Ok(Some(Step::Fault(journal_fault(
                "LSV2404",
                "sequential output exceeds runtime bounds",
            ))));
        }
    }
    Ok(None)
}

#[test]
fn terminal_and_cancellation_policy_match_frozen_bytes_limits_and_precedence() {
    let request = crate::continuation_contract_tests::requests().remove(0);
    let mut cases = vec![
        Step::Effect(Box::new(request.clone())),
        Step::Effects(Box::new(StructuredEffectBatch {
            merge_token: request.continuation.token.clone(),
            branches: vec![],
        })),
        Step::Waiting(MergeWait {
            merge_token: request.continuation.token.clone(),
            completed_branches: 1,
            total_branches: 0,
        }),
        Step::Yield(request.continuation.clone()),
        Step::Done(Value::UiFocus {
            node_id: String::new(),
        }),
        Step::Done(Value::Structured { fields: vec![] }),
    ];
    let mut value = Value::UiFocus {
        node_id: "a".into(),
    };
    for _ in 0..19 {
        cases.push(Step::Done(value.clone()));
        value = Value::Structured {
            fields: vec![StructuredField {
                name: "a".into(),
                value: Box::new(value),
            }],
        };
    }
    for token in [
        "".into(),
        "legacy-group".into(),
        "continuation-01".into(),
        "continuation-0".into(),
        "a.b_c-d".into(),
        "bad/token".into(),
        "a".repeat(128),
        "a".repeat(129),
        "\u{e9}".repeat(64),
    ] {
        for reason in [
            CancellationReason::Requested,
            CancellationReason::DeadlineExceeded,
        ] {
            for observed_at_ms in [0, 1, i64::MAX as u64, i64::MAX as u64 + 1, u64::MAX] {
                cases.push(Step::Cancelled(Cancellation {
                    continuation: ContinuationToken::from_wire(token.clone()),
                    reason: reason.clone(),
                    observed_at_ms,
                }));
            }
        }
    }
    for class in [EffectErrorClass::Transient, EffectErrorClass::Permanent] {
        for code in [
            "".into(),
            "retry.ok-1".into(),
            "a".repeat(64),
            "a".repeat(65),
            "bad code".into(),
            "\u{e9}".into(),
        ] {
            for message in [
                String::new(),
                "x".into(),
                "m".repeat(1024),
                "m".repeat(1025),
                "\u{e9}".repeat(512),
            ] {
                for retry_count in [0, 16, 17, u32::MAX] {
                    cases.push(Step::Failed(FailedEffect {
                        error: EffectError {
                            class,
                            code: code.clone(),
                            message: message.clone(),
                        },
                        retry_count,
                    }));
                }
            }
        }
    }
    for code in [
        String::new(),
        "!".into(),
        "f".repeat(32),
        "f".repeat(33),
        "\u{e9}".repeat(16),
        "\u{e9}".repeat(17),
    ] {
        for message in [
            String::new(),
            "m".repeat(4096),
            "m".repeat(4097),
            "\u{e9}".repeat(2048),
        ] {
            cases.push(Step::Fault(Fault {
                code: code.clone(),
                message,
            }));
        }
    }
    assert_eq!(cases.len(), 379);
    let mut other = request.continuation.clone();
    other.token = ContinuationToken::from_wire("bad/token".into());
    for step in cases {
        let wire = serde_json::to_vec(&step).unwrap();
        assert_eq!(validate_terminal_step(&step), frozen_terminal(&step));
        assert_eq!(
            terminal_step_outcome(step.clone()),
            frozen_outcome(step.clone())
        );
        for image in [&request.continuation, &other] {
            let expected = frozen_terminal(&step).and_then(|()| match &step {
                Step::Cancelled(c) if c.continuation == image.token => Ok(()),
                _ => Err(journal_fault(
                    "LSV4007",
                    "cancellation does not match continuation image",
                )),
            });
            assert_eq!(validate_cancellation_step(image, &step), expected);
        }
        assert_eq!(serde_json::to_vec(&step).unwrap(), wire);
    }
}

#[test]
fn sequence_summary_matches_frozen_limits_and_never_reads_after_first_stop() {
    let mut control = leserpent_domain::InMemoryControlPlane::default();
    let runtime = control.register_runtime(
        leselang_host_contract::RuntimeId::new("runtime-a").unwrap(),
        "runtime",
        "http://runtime.local",
    );
    let value = |count| {
        Step::Done(Value::RuntimeList {
            revision: crate::Revision(7),
            runtimes: vec![runtime.clone(); count],
        })
    };
    let cases = [
        value(0),
        value(1),
        value(5_000),
        value(5_001),
        value(10_001),
        Step::Done(Value::UiFocus {
            node_id: String::new(),
        }),
        Step::Fault(journal_fault("ORIGINAL", "first fault")),
        Step::Fault(journal_fault("", "invalid fault")),
        Step::Cancelled(Cancellation {
            continuation: ContinuationToken::from_wire("legacy".into()),
            reason: CancellationReason::Requested,
            observed_at_ms: 1,
        }),
    ];
    let mut count = 0;
    for a in &cases {
        for b in &cases {
            for c in &cases {
                let steps = [a, b, c];
                assert_eq!(sequence_stop(steps), frozen_sequence(steps));
                count += 1;
            }
        }
    }
    assert_eq!(count, 729);
    for count in [0, 63, 64, 65, 66] {
        let steps = vec![&cases[0]; count];
        assert_eq!(sequence_stop(steps.clone()), frozen_sequence(steps));
    }
    let stop = &cases[6];
    let guarded = std::iter::once(stop).chain(std::iter::from_fn(|| panic!("observed after stop")));
    assert_eq!(sequence_stop(guarded), Ok(Some(stop.clone())));
    let over = [&cases[3], &cases[3]]
        .into_iter()
        .chain(std::iter::from_fn(|| panic!("observed after overflow")));
    assert_eq!(
        sequence_stop(over),
        Ok(Some(Step::Fault(journal_fault(
            "LSV2404",
            "sequential output exceeds runtime bounds"
        ))))
    );
}
