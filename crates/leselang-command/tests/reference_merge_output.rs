use std::cell::RefCell;
use std::rc::Rc;

use leselang_command::reference_group_recovery::{
    RecoveredMergeBranch, validate_unbound_group_receipt,
};
use leselang_command::reference_merge_output::{assemble_merge, terminal_merge};
use leselang_command::reference_recovery::MAX_JOURNAL_ENTRY_BYTES;
use leselang_command::reference_request::DEFAULT_MAX_OUTPUT_ITEMS;
use leselang_command::*;
use leselang_hir::computation::{Computation, ScalarValue};
use leselang_hir::{Effect, HirBranch, Type};
use leselang_runtime_core::Fault;

fn plan(order: ExecutionOrder) -> MergePlan {
    MergePlan {
        branches: vec!["a".into(), "b".into()],
        order,
        result_binding: None,
    }
}
fn bound_plan() -> MergePlan {
    let mut plan = plan(ExecutionOrder::BoundParallel);
    plan.result_binding = Some(Box::new(GroupResultBinding {
        pending: Effect::All {
            branches: plan
                .branches
                .iter()
                .map(|name| HirBranch {
                    name: name.clone(),
                    result_type: Type::UiFocus,
                    effect: Effect::UiFocus {
                        node_id: name.clone(),
                    },
                })
                .collect(),
        },
        binding: Box::new(ResultBinding {
            name: "group".into(),
            locals: vec![],
            results: vec![],
            groups: vec![],
            body: Computation::Literal {
                value: ScalarValue::None,
            },
        }),
        fuel_remaining: 100,
        successor_sequence: None,
        additional_successor_sequences: vec![],
    }));
    plan
}
fn completions() -> Vec<BranchCompletion> {
    ["b", "a"]
        .into_iter()
        .map(|name| BranchCompletion {
            branch: name.into(),
            outcome: BranchOutcome::Value(Value::UiFocus {
                node_id: name.into(),
            }),
        })
        .collect()
}
fn fault(code: &str) -> Fault {
    Fault {
        code: code.into(),
        message: "original native message".into(),
    }
}

#[test]
fn data_assembly_borrows_member_proofs_but_never_finishes_saved_binding() {
    let plan = bound_plan();
    let original = serde_json::to_vec(&plan).unwrap();
    let trace = Rc::new(RefCell::new(vec![]));
    for fail_at in [None, Some("b"), Some("a")] {
        trace.borrow_mut().clear();
        let result = assemble_merge(&plan, completions(), 2, true, |binding, name, value| {
            assert!(std::ptr::eq(
                binding,
                plan.result_binding.as_deref().unwrap()
            ));
            assert!(matches!(value, Value::UiFocus { .. }));
            trace.borrow_mut().push(name.to_string());
            if Some(name) == fail_at {
                Err(fault("HOST"))
            } else {
                Ok(())
            }
        });
        if fail_at.is_some() {
            assert_eq!(result, Err(fault("HOST")));
        } else {
            let Step::Done(Value::Structured { fields }) = result.unwrap() else {
                panic!("saved binding must not turn the aggregate into Scalar(None)");
            };
            assert_eq!(
                fields
                    .iter()
                    .map(|field| field.name.as_str())
                    .collect::<Vec<_>>(),
                ["a", "b"]
            );
        }
        let expected = if fail_at == Some("b") {
            vec!["b"]
        } else {
            vec!["b", "a"]
        };
        assert_eq!(*trace.borrow(), expected);
    }
    for panic_at in ["b", "a"] {
        trace.borrow_mut().clear();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            assemble_merge(&plan, completions(), 2, true, |_, name, _| {
                trace.borrow_mut().push(name.to_string());
                assert_ne!(name, panic_at, "owner proof unwind");
                Ok(())
            })
        }));
        assert!(result.is_err());
        assert_eq!(trace.borrow().last().unwrap(), panic_at);
        assert_eq!(trace.borrow().len(), if panic_at == "b" { 1 } else { 2 });
    }
    assert_eq!(serde_json::to_vec(&plan).unwrap(), original);
}

#[test]
fn data_limit_terminalization_preserves_all_orders_and_native_errors() {
    let orders = [
        ExecutionOrder::Parallel,
        ExecutionOrder::Sequential,
        ExecutionOrder::ResultChain,
        ExecutionOrder::Dataflow,
        ExecutionOrder::BoundParallel,
        ExecutionOrder::BoundSequential,
        ExecutionOrder::GroupTail,
        ExecutionOrder::GroupCapture,
        ExecutionOrder::ParallelGroupTail,
        ExecutionOrder::ParallelGroupCapture,
        ExecutionOrder::GroupDataflow,
        ExecutionOrder::ParallelGroupDataflow,
        ExecutionOrder::GroupConditional,
        ExecutionOrder::ParallelGroupConditional,
    ];
    for order in orders {
        let plan = plan(order);
        for code in [
            "LSV3002", "LSV2404", "LSV3001", "LSV2403", "LSV4007", "LSV4022", "HOST",
        ] {
            let error = fault(code);
            let expected = if code == "LSV3002" || (order.is_parallel() && code == "LSV2404") {
                Ok(Step::Fault(error.clone()))
            } else {
                Err(error.clone())
            };
            assert_eq!(terminal_merge(&plan, Err(error)), expected);
        }
        let step = Step::Fault(fault("ORIGINAL"));
        assert_eq!(terminal_merge(&plan, Ok(step.clone())), Ok(step));
    }
}

fn log_value(name_bytes: usize) -> Value {
    serde_json::from_value(serde_json::json!({
        "kind": "runtime_logs", "revision": 0, "runtime_id": "runtime-a",
        "runtime_name": "a".repeat(name_bytes), "entries": []
    }))
    .unwrap()
}
fn log_completion(name_bytes: usize) -> Vec<BranchCompletion> {
    vec![
        BranchCompletion {
            branch: "a".into(),
            outcome: BranchOutcome::Value(log_value(name_bytes)),
        },
        BranchCompletion {
            branch: "b".into(),
            outcome: BranchOutcome::Value(Value::UiFocus {
                node_id: "b".into(),
            }),
        },
    ]
}
fn log_rows(name_bytes: usize) -> Vec<RecoveredMergeBranch> {
    log_completion(name_bytes)
        .into_iter()
        .enumerate()
        .map(|(index, completion)| {
            let BranchOutcome::Value(value) = completion.outcome else {
                unreachable!()
            };
            (
                completion.branch,
                index as i64,
                "completed".into(),
                Some(serde_json::to_vec(&Step::Done(value)).unwrap()),
                format!("continuation-{}", index + 1),
            )
        })
        .collect()
}

#[test]
fn output_measurement_keeps_inclusive_encoded_limit_without_allocating_a_receipt_buffer() {
    let plan = plan(ExecutionOrder::Parallel);
    let small = assemble_merge(
        &plan,
        log_completion(0),
        DEFAULT_MAX_OUTPUT_ITEMS,
        true,
        |_, _, _| unreachable!(),
    )
    .unwrap();
    let overhead = serde_json::to_vec(&small).unwrap().len();
    for delta in [0, 1] {
        let count = MAX_JOURNAL_ENTRY_BYTES - overhead + delta;
        let result = assemble_merge(
            &plan,
            log_completion(count),
            DEFAULT_MAX_OUTPUT_ITEMS,
            true,
            |_, _, _| unreachable!(),
        );
        if delta == 0 {
            let expected = result.unwrap();
            assert_eq!(
                serde_json::to_vec(&expected).unwrap().len(),
                MAX_JOURNAL_ENTRY_BYTES
            );
            let rows = log_rows(count);
            assert_eq!(
                validate_unbound_group_receipt(&plan, &rows, &expected),
                Ok(())
            );
        } else {
            let error = result.unwrap_err();
            assert_eq!(error.code, "LSV3002");
            assert_eq!(
                error.message,
                "structured merge output exceeds 8388608 bytes"
            );
            let rows = log_rows(count);
            assert_eq!(
                validate_unbound_group_receipt(&plan, &rows, &Step::Fault(error)),
                Ok(())
            );
        }
    }
}

#[test]
fn receipt_policy_keeps_bound_bypass_and_decode_before_sequential_stop() {
    let terminal = Step::Fault(fault("STOP"));
    // Bound plans are checked elsewhere, even if these rows are malformed.
    assert_eq!(
        validate_unbound_group_receipt(&bound_plan(), &[], &terminal),
        Ok(())
    );
    let mut rows: Vec<RecoveredMergeBranch> = vec![
        (
            "a".into(),
            0,
            "completed".into(),
            Some(serde_json::to_vec(&terminal).unwrap()),
            "continuation-1".into(),
        ),
        (
            "b".into(),
            1,
            "completed".into(),
            Some(b"{".to_vec()),
            "continuation-2".into(),
        ),
    ];
    let plan = plan(ExecutionOrder::Sequential);
    let error = validate_unbound_group_receipt(&plan, &rows, &terminal).unwrap_err();
    assert!(error.message.starts_with("invalid journal JSON:"));
    let mut encoded = serde_json::to_vec(&Step::Done(Value::UiFocus {
        node_id: "b".into(),
    }))
    .unwrap();
    encoded.resize(MAX_JOURNAL_ENTRY_BYTES, b' ');
    rows[1].3 = Some(encoded);
    assert_eq!(
        validate_unbound_group_receipt(&plan, &rows, &terminal),
        Ok(())
    );
    rows[1].3.as_mut().unwrap().push(b' ');
    assert_eq!(
        validate_unbound_group_receipt(&plan, &rows, &terminal)
            .unwrap_err()
            .code,
        "LSV4006"
    );
}
