use leselang_command::reference_value::{
    MAX_STRUCTURED_VALUE_DEPTH, validate_branch_outcome, validate_value, value_validation_limits,
};
use leselang_command::{
    BranchOutcome, Cancellation, CancellationReason, ContinuationToken, EffectError,
    EffectErrorClass, FailedEffect, StructuredField, Value,
};
use leselang_host_contract::Revision;
use leselang_runtime_core::Fault;
use leserpent_hir::computation::{ScalarValue, StringListValue};

fn focus(node: &str) -> Value {
    Value::UiFocus {
        node_id: node.into(),
    }
}

fn group(rows: Vec<(&str, Value)>) -> Value {
    Value::Structured {
        fields: rows
            .into_iter()
            .map(|(name, value)| StructuredField {
                name: name.into(),
                value: Box::new(value),
            })
            .collect(),
    }
}

#[test]
fn released_value_limits_count_items_without_allocating_a_vm() {
    let limits = value_validation_limits();
    assert_eq!(
        (limits.max_depth, limits.max_fields, limits.max_query_items),
        (16, 64, 10_000)
    );
    let value = group(vec![
        (
            "a",
            Value::RuntimeList {
                revision: Revision(1),
                runtimes: vec![],
            },
        ),
        (
            "b",
            Value::Scalar {
                value: ScalarValue::StringList(StringListValue(vec![])),
            },
        ),
        (
            "c",
            Value::Scalar {
                value: ScalarValue::StringList(StringListValue(vec!["x".into(), "y".into()])),
            },
        ),
        ("d", focus("node-a")),
    ]);
    let bytes = serde_json::to_vec(&value).unwrap();
    assert_eq!(validate_value(&value, 0), Ok(4));
    assert_eq!(
        validate_value(&value, MAX_STRUCTURED_VALUE_DEPTH - 1),
        Ok(4)
    );
    assert_eq!(
        validate_value(&value, MAX_STRUCTURED_VALUE_DEPTH)
            .unwrap_err()
            .message,
        "structured value nesting exceeds runtime bounds"
    );
    assert_eq!(serde_json::to_vec(&value).unwrap(), bytes);
}

#[test]
fn value_admission_preserves_depth_name_child_order_and_local_duplicate_scopes() {
    for (value, depth, message) in [
        (
            focus("invalid node"),
            usize::MAX,
            "structured value nesting exceeds runtime bounds",
        ),
        (group(vec![]), 0, "structured value exceeds runtime bounds"),
        (
            group(vec![("a", focus("bad node")), ("a", focus("ok"))]),
            0,
            "structured value exceeds runtime bounds",
        ),
        (
            group(vec![("a", focus("ok")), ("a", focus("bad node"))]),
            0,
            "structured value fields are invalid",
        ),
        (
            group(vec![("bad name", focus("bad node"))]),
            16,
            "structured value fields are invalid",
        ),
        (
            group(vec![("a", focus("bad node"))]),
            16,
            "structured value nesting exceeds runtime bounds",
        ),
    ] {
        assert_eq!(
            validate_value(&value, depth),
            Err(Fault {
                code: "LSV2403".into(),
                message: message.into()
            })
        );
    }
    assert_eq!(
        validate_value(
            &group(vec![
                ("a", group(vec![("x", focus("a"))])),
                ("b", group(vec![("x", focus("b"))])),
            ]),
            0
        ),
        Ok(2)
    );
}

#[test]
fn terminal_validation_retains_lexical_tokens_and_metadata_bounds_not_authority() {
    // Opaque/legacy lexical identities remain data, not canonical resume tokens.
    for token in ["legacy-group", "continuation-01", "continuation-0"] {
        for reason in [
            CancellationReason::Requested,
            CancellationReason::DeadlineExceeded,
        ] {
            let outcome = BranchOutcome::Cancelled(Cancellation {
                continuation: ContinuationToken::from_wire(token.into()),
                reason,
                observed_at_ms: i64::MAX as u64,
            });
            assert_eq!(validate_branch_outcome(&outcome), Ok(0));
        }
    }
    let mut cancelled = Cancellation {
        continuation: ContinuationToken::from_wire("legacy-group".into()),
        reason: CancellationReason::Requested,
        observed_at_ms: i64::MAX as u64 + 1,
    };
    assert!(validate_branch_outcome(&BranchOutcome::Cancelled(cancelled.clone())).is_err());
    cancelled.observed_at_ms = 0;
    cancelled.continuation = ContinuationToken::from_wire("bad/token".into());
    assert!(validate_branch_outcome(&BranchOutcome::Cancelled(cancelled)).is_err());
    for class in [EffectErrorClass::Permanent, EffectErrorClass::Transient] {
        let mut failure = FailedEffect {
            error: EffectError {
                class,
                code: "A".repeat(64),
                message: "m".repeat(1024),
            },
            retry_count: 16,
        };
        assert_eq!(
            validate_branch_outcome(&BranchOutcome::Failed(failure.clone())),
            Ok(0)
        );
        failure.retry_count = 17;
        assert!(validate_branch_outcome(&BranchOutcome::Failed(failure.clone())).is_err());
        failure.retry_count = 0;
        failure.error.message.clear();
        assert!(validate_branch_outcome(&BranchOutcome::Failed(failure)).is_err());
    }
    assert_eq!(
        validate_branch_outcome(&BranchOutcome::Fault(Fault {
            code: "!".repeat(32),
            message: String::new(),
        })),
        Ok(0)
    );
}

#[test]
fn structured_outcomes_include_merge_depth_and_keep_bounds_separate_from_decode() {
    let mut value = focus("a");
    for _ in 0..15 {
        value = group(vec![("a", value)]);
    }
    assert_eq!(validate_value(&value, 0), Ok(1));
    let outcome = BranchOutcome::Value(value);
    let bytes = serde_json::to_vec(&outcome).unwrap();
    let decoded: BranchOutcome = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        validate_branch_outcome(&decoded).unwrap_err().message,
        "structured value nesting exceeds runtime bounds"
    );
    assert_eq!(serde_json::to_vec(&decoded).unwrap(), bytes);
    let fault = BranchOutcome::Fault(Fault {
        code: String::new(),
        message: "private payload".into(),
    });
    let invalid = validate_branch_outcome(&fault).unwrap_err();
    assert_eq!(
        invalid,
        Fault {
            code: "LSV2403".into(),
            message: "structured merge branch outcome is invalid".into()
        }
    );
    assert!(!invalid.message.contains("private payload"));
}
