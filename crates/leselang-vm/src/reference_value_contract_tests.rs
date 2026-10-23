use super::*;

// Frozen terminal policy: deliberately do not call the migrated validator or
// shared token/retry helpers here. Values use the pre-migration traversal oracle.
fn frozen(outcome: &BranchOutcome) -> Result<usize, Fault> {
    let valid = match outcome {
        BranchOutcome::Value(value) => {
            return value_validation_tests::legacy_validate_value(value, 2);
        }
        BranchOutcome::Cancelled(cancelled) => {
            let token = cancelled.continuation.as_str();
            !token.is_empty()
                && token.len() <= 128
                && token
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
                && cancelled.observed_at_ms <= i64::MAX as u64
        }
        BranchOutcome::Failed(failure) => {
            let error = &failure.error;
            failure.retry_count <= 16
                && !error.code.is_empty()
                && error.code.len() <= 64
                && error
                    .code
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
                && !error.message.is_empty()
                && error.message.len() <= 1024
        }
        BranchOutcome::Fault(error) => {
            !error.code.is_empty() && error.code.len() <= 32 && error.message.len() <= 4096
        }
    };
    if valid {
        Ok(0)
    } else {
        Err(Fault {
            code: "LSV2403".into(),
            message: "structured merge branch outcome is invalid".into(),
        })
    }
}

#[test]
fn branch_terminal_policy_matches_frozen_limits_bytes_and_legacy_tokens() {
    let mut outcomes = vec![];
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
                outcomes.push(BranchOutcome::Cancelled(Cancellation {
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
                    outcomes.push(BranchOutcome::Failed(FailedEffect {
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
            outcomes.push(BranchOutcome::Fault(Fault {
                code: code.clone(),
                message,
            }));
        }
    }
    assert_eq!(outcomes.len(), 354);
    for outcome in outcomes {
        let wire = serde_json::to_vec(&outcome).unwrap();
        let expected = frozen(&outcome);
        assert_eq!(validate_branch_outcome(&outcome), expected);
        assert_eq!(
            leselang_command::reference_value::validate_branch_outcome(&outcome),
            expected
        );
        assert_eq!(serde_json::to_vec(&outcome).unwrap(), wire);
    }
}
