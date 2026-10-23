use leselang_command::reference_completion::{
    deadline_cancellation, sequence_stop, terminal_step_outcome, validate_cancellation_step,
    validate_terminal_step,
};
use leselang_command::reference_value::validate_branch_outcome;
use leselang_command::{
    BranchOutcome, Cancellation, CancellationReason, ContinuationImage, ContinuationToken,
    EffectError, EffectErrorClass, FailedEffect, MergeWait, Step, StructuredField, Value,
};
use leselang_hir::{Effect, Type};
use leselang_host_contract::Revision;
use leselang_runtime_core::Fault;

fn image() -> ContinuationImage {
    ContinuationImage {
        schema_version: 1,
        token: ContinuationToken::from_wire("continuation-7".into()),
        program_counter: 1,
        expected_revision: Some(Revision(3)),
        result_type: Type::UiFocus,
        pending_effect: Effect::UiFocus {
            node_id: "a".into(),
        },
        fuel_remaining: 99,
        deadline_ms: 30_000,
        deadline_at_ms: Some(31_000),
        max_output_items: 10_000,
        result_binding: None,
        group_result: None,
    }
}

#[test]
fn journal_terminals_keep_their_distinct_legacy_policy_without_a_vm() {
    let mut value = Value::UiFocus {
        node_id: "a".into(),
    };
    for _ in 0..15 {
        value = Value::Structured {
            fields: vec![StructuredField {
                name: "a".into(),
                value: Box::new(value),
            }],
        };
    }
    let cases = [
        Step::Done(value),
        Step::Cancelled(Cancellation {
            continuation: ContinuationToken::from_wire("bad/token".into()),
            reason: CancellationReason::Requested,
            observed_at_ms: i64::MAX as u64,
        }),
        Step::Fault(Fault {
            code: "LEGACY".into(),
            message: "x".repeat(4097),
        }),
    ];
    for step in cases {
        let bytes = serde_json::to_vec(&step).unwrap();
        let decoded: Step = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(validate_terminal_step(&decoded), Ok(()));
        let outcome = terminal_step_outcome(decoded).unwrap();
        assert!(validate_branch_outcome(&outcome).is_err());
        assert_eq!(serde_json::to_vec(&step).unwrap(), bytes);
    }
    let waiting = Step::Waiting(MergeWait {
        merge_token: image().token,
        completed_branches: 0,
        total_branches: 1,
    });
    assert_eq!(
        validate_terminal_step(&waiting).unwrap_err().message,
        "journal completion must be a terminal step"
    );
    assert_eq!(terminal_step_outcome(waiting).unwrap_err().code, "LSV4007");
}

#[test]
fn terminal_conversion_moves_original_payload_allocations() {
    let cases = [
        Step::Done(Value::UiFocus {
            node_id: "node-a".into(),
        }),
        Step::Cancelled(Cancellation {
            continuation: image().token,
            reason: CancellationReason::Requested,
            observed_at_ms: 1,
        }),
        Step::Failed(FailedEffect {
            retry_count: 16,
            error: EffectError {
                class: EffectErrorClass::Transient,
                code: "BUSY".into(),
                message: "retry later".into(),
            },
        }),
        Step::Fault(Fault {
            code: "NATIVE".into(),
            message: "original message".into(),
        }),
    ];
    for step in cases {
        let pointer = match &step {
            Step::Done(Value::UiFocus { node_id }) => node_id.as_ptr(),
            Step::Cancelled(c) => c.continuation.as_str().as_ptr(),
            Step::Failed(f) => f.error.message.as_ptr(),
            Step::Fault(f) => f.message.as_ptr(),
            _ => unreachable!(),
        };
        let outcome = terminal_step_outcome(step).unwrap();
        let moved = match &outcome {
            BranchOutcome::Value(Value::UiFocus { node_id }) => node_id.as_ptr(),
            BranchOutcome::Cancelled(c) => c.continuation.as_str().as_ptr(),
            BranchOutcome::Failed(f) => f.error.message.as_ptr(),
            BranchOutcome::Fault(f) => f.message.as_ptr(),
            _ => unreachable!(),
        };
        assert_eq!(moved, pointer);
    }
}

#[test]
fn cancellation_shape_precedes_identity_but_never_proves_deadline_or_live_authority() {
    let image = image();
    let bytes = serde_json::to_vec(&image).unwrap();
    let early = deadline_cancellation(&image, 0);
    assert_eq!(validate_cancellation_step(&image, &early), Ok(()));
    assert!(
        matches!(&early, Step::Cancelled(c) if c.observed_at_ms == 0 && c.reason == CancellationReason::DeadlineExceeded)
    );
    let mut foreign = image.clone();
    foreign.token = ContinuationToken::from_wire("foreign".into());
    let invalid = deadline_cancellation(&foreign, u64::MAX);
    assert_eq!(
        validate_cancellation_step(&image, &invalid)
            .unwrap_err()
            .message,
        "journal fault has an invalid diagnostic code"
    );
    assert_eq!(
        validate_cancellation_step(&image, &deadline_cancellation(&foreign, 0))
            .unwrap_err()
            .message,
        "cancellation does not match continuation image"
    );
    assert_eq!(serde_json::to_vec(&image).unwrap(), bytes);
}

#[test]
fn sequential_summary_bounds_empty_outputs_and_does_not_visit_later_steps_after_stop() {
    let empty = Step::Done(Value::RuntimeList {
        revision: Revision(1),
        runtimes: vec![],
    });
    assert_eq!(sequence_stop(std::iter::repeat_n(&empty, 64)), Ok(None));
    assert_eq!(
        sequence_stop(std::iter::repeat_n(&empty, 65))
            .unwrap_err()
            .message,
        "sequential result observations exceed runtime bounds"
    );
    let stop = Step::Fault(Fault {
        code: "FIRST".into(),
        message: "original".into(),
    });
    let steps = std::iter::once(&stop).chain(std::iter::from_fn(|| panic!("later step observed")));
    assert_eq!(sequence_stop(steps), Ok(Some(stop.clone())));
    let invalid = Step::Done(Value::UiFocus {
        node_id: String::new(),
    });
    assert_eq!(
        sequence_stop([&invalid, &stop]).unwrap_err().message,
        "completed journal output exceeds runtime limit"
    );
}
