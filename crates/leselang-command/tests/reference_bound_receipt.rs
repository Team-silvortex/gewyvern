use leselang_command::reference_bound_receipt::validate_bound_group_receipt;
use leselang_command::reference_merge::{
    invalid_group_binding as invalid, validate_group_binding, validate_merge_plan,
};
use leselang_command::reference_request::DEFAULT_MAX_OUTPUT_ITEMS;
use leselang_command::{
    Cancellation, CancellationReason, ContinuationToken, ExecutionOrder, GroupResultBinding,
    MergePlan, ResultBinding, Step, Value,
};
use leselang_runtime_core::{Fault, OrderedOutcomeSummary, OutcomeOrder, ScalarValue};
use leserpent_hir::{Effect, HirBranch, Type, computation::Computation};

fn fault(code: &str, message: &str) -> Fault {
    Fault {
        code: code.into(),
        message: message.into(),
    }
}

fn overflow_fault(items: usize, limit: usize) -> Fault {
    fault(
        "LSV2404",
        &format!("structured merge returned {items} items, limit is {limit}"),
    )
}

fn sequential_overflow() -> Step {
    Step::Fault(fault("LSV2404", "sequential output exceeds runtime bounds"))
}

fn outcomes(plan: &MergePlan, pattern: usize) -> OrderedOutcomeSummary<Step> {
    let mut summary = OrderedOutcomeSummary::new(
        if plan.order.is_parallel() {
            OutcomeOrder::Parallel
        } else {
            OutcomeOrder::Sequential
        },
        64,
        DEFAULT_MAX_OUTPUT_ITEMS,
    )
    .unwrap();
    let first = || Step::Fault(fault("LSV1001", "original first stop"));
    match pattern {
        0 => summary
            .observe_success(1, |_, _| sequential_overflow())
            .unwrap(),
        1 => summary.observe_stop(first()).unwrap(),
        2 => summary
            .observe_success(DEFAULT_MAX_OUTPUT_ITEMS + 1, |_, _| sequential_overflow())
            .unwrap(),
        3 => {
            summary.observe_stop(first()).unwrap();
            summary
                .observe_success(DEFAULT_MAX_OUTPUT_ITEMS + 1, |_, _| sequential_overflow())
                .unwrap();
        }
        4 => {
            summary
                .observe_success(DEFAULT_MAX_OUTPUT_ITEMS + 1, |_, _| sequential_overflow())
                .unwrap();
            summary.observe_stop(first()).unwrap();
        }
        _ => unreachable!(),
    }
    summary
}

fn terminals() -> Vec<Step> {
    let mut steps = [
        "LSV1001", "LSV1002", "LSV1401", "LSV1402", "LSV1403", "LSV1404", "LSV1405", "LSV1406",
        "LSV1408", "LSV1409", "LSV3001", "LSV3002", "LSV2404", "unknown",
    ]
    .into_iter()
    .map(|code| Step::Fault(fault(code, "original first stop")))
    .collect::<Vec<_>>();
    steps.extend(
        [
            ScalarValue::None,
            ScalarValue::Boolean(true),
            ScalarValue::Integer(7),
        ]
        .into_iter()
        .map(|value| Step::Done(Value::Scalar { value })),
    );
    steps.push(Step::Done(Value::UiFocus {
        node_id: "next".into(),
    }));
    steps.push(Step::Cancelled(Cancellation {
        continuation: ContinuationToken::from_wire("merge-1".into()),
        reason: CancellationReason::Requested,
        observed_at_ms: 1,
    }));
    steps.push(Step::Failed(leselang_command::FailedEffect {
        error: leselang_command::EffectError {
            class: leselang_command::EffectErrorClass::Permanent,
            code: "original".into(),
            message: "original failure".into(),
        },
        retry_count: 0,
    }));
    steps.push(sequential_overflow());
    steps.push(Step::Fault(overflow_fault(
        DEFAULT_MAX_OUTPUT_ITEMS + 1,
        DEFAULT_MAX_OUTPUT_ITEMS,
    )));
    steps
}

fn saved(body: Computation) -> ResultBinding {
    ResultBinding {
        name: "saved".into(),
        locals: vec![],
        results: vec![],
        groups: vec![],
        body,
    }
}

#[test]
fn bound_receipt_policy_matches_frozen_all_forms_faults_and_output_precedence() {
    let pure = Computation::Literal {
        value: ScalarValue::None,
    };
    let host = Computation::Host {
        effect: Box::new(Effect::UiFocus {
            node_id: "next".into(),
        }),
    };
    let saved = [
        saved(pure.clone()),
        saved(host.clone()),
        saved(Computation::Choose {
            when: Box::new(Computation::Literal {
                value: ScalarValue::Boolean(true),
            }),
            then: Box::new(host),
            otherwise: Box::new(pure),
        }),
    ];
    let mut comparisons = 0;
    for parallel in [false, true] {
        for kind in 0..5 {
            let (binding, original) = group_plan_fixture(parallel, kind);
            for created in 0..=binding.reserved_sequences().count() {
                let mut plan = original.clone();
                for position in 0..created {
                    plan.branches
                        .push(binding.plan_successor_name_at(position, 64).unwrap());
                }
                validate_merge_plan(&plan).unwrap();
                let original_plan = plan.clone();
                for terminal in terminals() {
                    let original_terminal = terminal.clone();
                    for last_binding in [None, Some(&saved[0]), Some(&saved[1]), Some(&saved[2])] {
                        for pattern in 0..5 {
                            let other =
                                Step::Fault(fault("other", "not the original last receipt"));
                            for last_step in [None, Some(&terminal), Some(&other)] {
                                assert_eq!(
                                    validate_bound_group_receipt(
                                        &plan,
                                        last_binding,
                                        last_step,
                                        outcomes(&plan, pattern),
                                        Some(&terminal)
                                    ),
                                    frozen_receipt(
                                        &plan,
                                        last_binding,
                                        last_step,
                                        outcomes(&plan, pattern),
                                        Some(&terminal)
                                    ),
                                    "parallel={parallel} kind={kind} created={created} pattern={pattern} terminal={terminal:?}",
                                );
                                assert_eq!(plan, original_plan);
                                assert_eq!(terminal, original_terminal);
                                comparisons += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    assert_eq!(comparisons, 26400);
}

#[test]
fn pending_unbound_and_stopped_groups_do_not_gain_late_type_validation() {
    let (_, mut plan) = group_plan_fixture(false, 0);
    plan.result_binding.as_mut().unwrap().binding.name.clear();
    let closed = || {
        let mut summary = OrderedOutcomeSummary::new(OutcomeOrder::Sequential, 0, 0).unwrap();
        assert!(
            summary
                .observe_success(0, |_, _| sequential_overflow())
                .is_err()
        );
        summary
    };
    assert_eq!(
        validate_bound_group_receipt(&plan, None, None, closed(), None),
        Ok(())
    );
    let terminal = Step::Done(Value::Scalar {
        value: ScalarValue::None,
    });
    assert_eq!(
        validate_bound_group_receipt(&plan, None, None, closed(), Some(&terminal)),
        Err(invalid())
    );
    assert_eq!(
        validate_bound_group_receipt(&plan, None, None, outcomes(&plan, 0), Some(&terminal))
            .unwrap_err()
            .code,
        "LSV1405"
    );
    let failure = Step::Fault(fault("LSV1001", "original first stop"));
    assert_eq!(
        validate_bound_group_receipt(&plan, None, None, outcomes(&plan, 1), Some(&failure)),
        Ok(())
    );
    plan.result_binding = None;
    assert_eq!(
        validate_bound_group_receipt(&plan, None, None, closed(), Some(&terminal)),
        Ok(())
    );
}

#[test]
fn scalar_correspondence_does_not_evaluate_or_authenticate_saved_results() {
    let (_, mut plan) = group_plan_fixture(true, 0);
    plan.result_binding.as_mut().unwrap().binding.body = Computation::Literal {
        value: ScalarValue::Boolean(false),
    };
    validate_merge_plan(&plan).unwrap();
    let terminal = Step::Done(Value::Scalar {
        value: ScalarValue::Boolean(true),
    });
    // The original contract replays a committed typed scalar without recalculation.
    assert_eq!(
        validate_bound_group_receipt(&plan, None, None, outcomes(&plan, 0), Some(&terminal)),
        Ok(())
    );
    let wrong_type = Step::Done(Value::Scalar {
        value: ScalarValue::Integer(1),
    });
    assert_eq!(
        validate_bound_group_receipt(&plan, None, None, outcomes(&plan, 0), Some(&wrong_type)),
        Err(invalid())
    );
}

#[test]
fn successor_classification_preserves_frozen_shape_observation_without_admission() {
    for parallel in [false, true] {
        for kind in 0..5 {
            let (original, plan) = group_plan_fixture(parallel, kind);
            for defect in 0..5 {
                let mut binding = original.clone();
                if defect == 1 {
                    binding.successor_sequence = None;
                }
                if defect == 2 {
                    binding.pending = Effect::UiFocus {
                        node_id: "unbound".into(),
                    };
                }
                if defect == 3 {
                    binding.successor_sequence = Some(0);
                }
                if defect == 4 {
                    binding.fuel_remaining = u64::MAX;
                }
                for count in 0..=5 {
                    let mut plan = plan.clone();
                    plan.branches.resize(count, "unchecked".into());
                    assert_eq!(
                        binding.has_successor(&plan),
                        frozen_has_successor(&binding, &plan)
                    );
                }
            }
        }
    }
}

fn group_plan_fixture(parallel: bool, kind: usize) -> (GroupResultBinding, MergePlan) {
    let pure = Computation::Literal {
        value: ScalarValue::None,
    };
    let host = Computation::Host {
        effect: Box::new(Effect::UiFocus {
            node_id: "next".into(),
        }),
    };
    let capture = Computation::Bind {
        name: "next".into(),
        value: Box::new(host.clone()),
        body: Box::new(pure.clone()),
    };
    let (body, order) = match (parallel, kind) {
        (p, 0) => (
            pure.clone(),
            if p {
                ExecutionOrder::BoundParallel
            } else {
                ExecutionOrder::BoundSequential
            },
        ),
        (p, 1) => (
            host.clone(),
            if p {
                ExecutionOrder::ParallelGroupTail
            } else {
                ExecutionOrder::GroupTail
            },
        ),
        (p, 2) => (
            capture.clone(),
            if p {
                ExecutionOrder::ParallelGroupCapture
            } else {
                ExecutionOrder::GroupCapture
            },
        ),
        (p, 3) => (
            Computation::Bind {
                name: "first".into(),
                value: Box::new(host),
                body: Box::new(capture.clone()),
            },
            if p {
                ExecutionOrder::ParallelGroupDataflow
            } else {
                ExecutionOrder::GroupDataflow
            },
        ),
        (p, 4) => (
            Computation::Choose {
                when: Box::new(Computation::Literal {
                    value: ScalarValue::Boolean(false),
                }),
                then: Box::new(capture),
                otherwise: Box::new(pure),
            },
            if p {
                ExecutionOrder::ParallelGroupConditional
            } else {
                ExecutionOrder::GroupConditional
            },
        ),
        _ => unreachable!(),
    };
    let branches = ["a", "b"]
        .into_iter()
        .map(|name| HirBranch {
            name: name.into(),
            result_type: Type::UiFocus,
            effect: Effect::UiFocus {
                node_id: name.into(),
            },
        })
        .collect();
    let binding = GroupResultBinding {
        pending: if parallel {
            Effect::All { branches }
        } else {
            Effect::Sequence { steps: branches }
        },
        binding: Box::new(ResultBinding {
            name: "group".into(),
            locals: vec![],
            results: vec![],
            groups: vec![],
            body,
        }),
        fuel_remaining: 100,
        successor_sequence: (kind != 0).then_some(100),
        additional_successor_sequences: if kind == 3 { vec![101] } else { vec![] },
    };
    let plan = MergePlan {
        branches: vec!["a".into(), "b".into()],
        order,
        result_binding: Some(Box::new(binding.clone())),
    };
    (binding, plan)
}

// Frozen pre-extraction terminal decisions; no new receipt or classification helper.
fn frozen_has_successor(binding: &GroupResultBinding, plan: &MergePlan) -> bool {
    binding.successor_sequence.is_some()
        && match &binding.pending {
            Effect::All { branches } | Effect::Sequence { steps: branches } => {
                plan.branches.len() > branches.len()
            }
            _ => false,
        }
}

fn frozen_receipt(
    plan: &MergePlan,
    last_binding: Option<&ResultBinding>,
    last_step: Option<&Step>,
    outcomes: OrderedOutcomeSummary<Step>,
    terminal: Option<&Step>,
) -> Result<(), Fault> {
    let Some(binding) = &plan.result_binding else {
        return Ok(());
    };
    if let Some(terminal) = terminal {
        let summary = outcomes
            .finish(|items, limit| Step::Fault(overflow_fault(items, limit)))
            .map_err(|_| invalid())?;
        if let Some(failure) = summary.first_stop {
            if *terminal != failure {
                return Err(invalid());
            }
        } else {
            if frozen_has_successor(binding, plan) {
                if binding.captures_successor() {
                    return match terminal {
                        Step::Done(Value::Scalar { value })
                            if last_binding.is_some_and(|saved| {
                                saved.body.is_pure()
                                    || (binding.is_conditional()
                                        && saved.body.can_return_without_suspending())
                            }) && validate_group_binding(binding, plan)?
                                == leserpent_hir::Type::Scalar(value.scalar_type()) =>
                        {
                            Ok(())
                        }
                        Step::Fault(fault)
                            if matches!(
                                fault.code.as_str(),
                                "LSV1001"
                                    | "LSV1401"
                                    | "LSV1403"
                                    | "LSV1405"
                                    | "LSV1406"
                                    | "LSV1408"
                                    | "LSV3002"
                            ) || ((binding.is_dataflow() || binding.is_conditional())
                                && matches!(
                                    fault.code.as_str(),
                                    "LSV1002" | "LSV1402" | "LSV1404" | "LSV3001"
                                )) =>
                        {
                            Ok(())
                        }
                        _ => Err(invalid()),
                    };
                }
                if last_step != Some(terminal) {
                    return Err(invalid());
                }
                return Ok(());
            }
            // Replay the committed scalar/fault; never re-evaluate source during recovery.
            match terminal {
                Step::Done(Value::Scalar { value })
                    if (binding.successor_sequence.is_none()
                        || (binding.is_conditional()
                            && binding.binding.body.can_return_without_suspending()))
                        && validate_group_binding(binding, plan)?
                            == leserpent_hir::Type::Scalar(value.scalar_type()) => {}
                Step::Fault(fault)
                    if matches!(
                        fault.code.as_str(),
                        "LSV1001" | "LSV1401" | "LSV1403" | "LSV1406" | "LSV1408" | "LSV3002"
                    ) => {}
                Step::Fault(fault)
                    if binding.successor_sequence.is_some()
                        && matches!(
                            fault.code.as_str(),
                            "LSV1002" | "LSV1402" | "LSV1404" | "LSV1405" | "LSV3001" | "LSV3002"
                        ) => {}
                _ => return Err(invalid()),
            }
        }
    }
    Ok(())
}
