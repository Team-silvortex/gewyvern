use super::*;

fn plans() -> Vec<MergePlan> {
    [
        ExecutionOrder::Parallel,
        ExecutionOrder::Sequential,
        ExecutionOrder::ResultChain,
        ExecutionOrder::Dataflow,
        ExecutionOrder::BoundParallel,
        ExecutionOrder::BoundSequential,
    ]
    .into_iter()
    .map(|order| {
        let branches = match order {
            ExecutionOrder::ResultChain => vec!["result".into(), "successor".into()],
            ExecutionOrder::Dataflow => vec!["step_1".into(), "step_2".into()],
            _ => vec!["a".into(), "b".into()],
        };
        let result_binding = if matches!(
            order,
            ExecutionOrder::BoundParallel | ExecutionOrder::BoundSequential
        ) {
            let members = branches
                .iter()
                .map(|name: &String| leselang_hir::HirBranch {
                    name: name.clone(),
                    result_type: Type::UiFocus,
                    effect: Effect::UiFocus {
                        node_id: name.clone(),
                    },
                })
                .collect();
            Some(Box::new(GroupResultBinding {
                pending: if order == ExecutionOrder::BoundParallel {
                    Effect::All { branches: members }
                } else {
                    Effect::Sequence { steps: members }
                },
                binding: Box::new(ResultBinding {
                    name: "group".into(),
                    locals: vec![],
                    results: vec![],
                    groups: vec![],
                    body: leselang_hir::computation::Computation::Literal {
                        value: ScalarValue::None,
                    },
                }),
                fuel_remaining: 100,
                successor_sequence: None,
                additional_successor_sequences: vec![],
            }))
        } else {
            None
        };
        let plan = MergePlan {
            branches,
            order,
            result_binding,
        };
        validate_merge_plan(&plan).unwrap();
        plan
    })
    .collect()
}

fn outcome(kind: usize, name: &str) -> BranchOutcome {
    match kind {
        0 => BranchOutcome::Value(Value::UiFocus {
            node_id: name.into(),
        }),
        1 => BranchOutcome::Value(Value::Scalar {
            value: ScalarValue::Boolean(true),
        }),
        2 => BranchOutcome::Cancelled(Cancellation {
            continuation: ContinuationToken("continuation-1".into()),
            reason: CancellationReason::Requested,
            observed_at_ms: 101,
        }),
        3 => BranchOutcome::Failed(FailedEffect {
            error: EffectError {
                class: EffectErrorClass::Permanent,
                code: "host_failed".into(),
                message: name.into(),
            },
            retry_count: 2,
        }),
        4 => BranchOutcome::Fault(Fault {
            code: "HOST".into(),
            message: name.into(),
        }),
        5 => BranchOutcome::Fault(Fault {
            code: "".into(),
            message: name.into(),
        }),
        6 => BranchOutcome::Cancelled(Cancellation {
            continuation: ContinuationToken("".into()),
            reason: CancellationReason::DeadlineExceeded,
            observed_at_ms: u64::MAX,
        }),
        7 => BranchOutcome::Failed(FailedEffect {
            error: EffectError {
                class: EffectErrorClass::Transient,
                code: "".into(),
                message: "".into(),
            },
            retry_count: MAX_SEMANTIC_RETRIES + 1,
        }),
        8 => BranchOutcome::Value(Value::Structured {
            fields: vec![StructuredField {
                name: "".into(),
                value: Box::new(Value::UiFocus {
                    node_id: name.into(),
                }),
            }],
        }),
        _ => unreachable!(),
    }
}

fn compare(plan: &MergePlan, completions: Vec<BranchCompletion>, budget: usize, finish: bool) {
    let before = serde_json::to_vec(plan).unwrap();
    let expected = frozen_merge(plan, completions.clone(), budget, finish);
    let actual = merge_declared_inner(plan, completions, budget, finish);
    assert_eq!(
        actual, expected,
        "{:?}, budget={budget}, finish={finish}",
        plan.order
    );
    assert_eq!(
        serde_json::to_vec(&actual).unwrap(),
        serde_json::to_vec(&expected).unwrap()
    );
    assert_eq!(serde_json::to_vec(plan).unwrap(), before);
}

#[test]
fn merge_completion_migration_preserves_frozen_outcomes_order_budgets_and_bindings() {
    let mut comparisons = 0;
    for plan in plans() {
        for left in 0..9 {
            for right in 0..9 {
                for reverse in [false, true] {
                    let mut completions = plan
                        .branches
                        .iter()
                        .zip([left, right])
                        .map(|(name, kind)| BranchCompletion {
                            branch: name.clone(),
                            outcome: outcome(kind, name),
                        })
                        .collect::<Vec<_>>();
                    if reverse {
                        completions.reverse();
                    }
                    for budget in [
                        0,
                        1,
                        2,
                        DEFAULT_MAX_OUTPUT_ITEMS,
                        DEFAULT_MAX_OUTPUT_ITEMS + 1,
                    ] {
                        for finish in [false, true] {
                            compare(&plan, completions.clone(), budget, finish);
                            comparisons += 1;
                        }
                    }
                }
            }
        }
    }
    assert_eq!(comparisons, 9_720);
}

#[test]
fn merge_completion_refusals_keep_plan_first_validation_and_exact_error_precedence() {
    let mut comparisons = 0;
    for original in plans() {
        for defect in 0..12 {
            let mut plan = original.clone();
            let mut completions = plan
                .branches
                .iter()
                .map(|name| BranchCompletion {
                    branch: name.clone(),
                    outcome: outcome(0, name),
                })
                .collect::<Vec<_>>();
            match defect {
                0 => {
                    completions.pop();
                }
                1 => completions.push(completions[0].clone()),
                2 => completions[1].branch = completions[0].branch.clone(),
                3 => completions[0].branch = "unknown".into(),
                4 => completions[0].branch = "invalid name".into(),
                5 => plan.branches.clear(),
                6 => plan.branches[1] = plan.branches[0].clone(),
                7 => {
                    if let Some(binding) = &mut plan.result_binding {
                        binding.fuel_remaining = MAX_EXECUTION_FUEL + 1;
                    } else {
                        plan.order = ExecutionOrder::BoundParallel;
                    }
                }
                8 => {
                    completions[0].outcome = outcome(5, "a");
                    completions[1].branch = "unknown".into();
                }
                9 => {
                    completions[0].outcome = outcome(2, "a");
                    completions[1].outcome = outcome(5, "b");
                }
                10 => completions[0].outcome = outcome(8, "a"),
                11 => completions.reverse(),
                _ => unreachable!(),
            }
            for budget in [0, DEFAULT_MAX_OUTPUT_ITEMS, DEFAULT_MAX_OUTPUT_ITEMS + 1] {
                for finish in [false, true] {
                    compare(&plan, completions.clone(), budget, finish);
                    comparisons += 1;
                }
            }
        }
    }
    assert_eq!(comparisons, 432);
}

// Frozen pre-migration completion admission, ordering and finalization.
fn frozen_merge(
    plan: &MergePlan,
    completions: Vec<BranchCompletion>,
    max_output_items: usize,
    finish_binding: bool,
) -> Result<Step, Fault> {
    validate_merge_plan(plan)?;
    if max_output_items > DEFAULT_MAX_OUTPUT_ITEMS {
        return Err(Fault {
            code: "LSV2404".to_string(),
            message: "structured merge output budget exceeds runtime bounds".to_string(),
        });
    }
    if completions.len() != plan.branches.len() {
        return Err(Fault {
            code: "LSV2402".to_string(),
            message: "structured merge completion set is incomplete".to_string(),
        });
    }

    let expected = plan
        .branches
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let mut by_branch = BTreeMap::new();
    let mut output_items = 0usize;
    for completion in completions {
        if !valid_merge_branch_name(&completion.branch)
            || !expected.contains(completion.branch.as_str())
            || by_branch.contains_key(&completion.branch)
        {
            return Err(Fault {
                code: "LSV2402".to_string(),
                message: "structured merge completion set does not match its plan".to_string(),
            });
        }
        output_items = output_items.saturating_add(validate_branch_outcome(&completion.outcome)?);
        if let (Some(binding), BranchOutcome::Value(value)) =
            (&plan.result_binding, &completion.outcome)
        {
            binding.validate_member(&completion.branch, value)?;
        }
        by_branch.insert(completion.branch, completion.outcome);
    }
    if output_items > max_output_items {
        return Err(merge_output_fault(output_items, max_output_items));
    }

    let mut fields = Vec::with_capacity(plan.branches.len());
    let mut terminal = None;
    for branch in &plan.branches {
        let outcome = by_branch.remove(branch).ok_or_else(|| Fault {
            code: "LSV2402".to_string(),
            message: "structured merge completion set is incomplete".to_string(),
        })?;
        match outcome {
            BranchOutcome::Value(value) => fields.push(StructuredField {
                name: branch.clone(),
                value: Box::new(value),
            }),
            BranchOutcome::Cancelled(cancellation) => {
                terminal = Some(Step::Cancelled(cancellation));
                break;
            }
            BranchOutcome::Failed(failure) => {
                terminal = Some(Step::Failed(failure));
                break;
            }
            BranchOutcome::Fault(fault) => {
                terminal = Some(Step::Fault(fault));
                break;
            }
        }
    }
    let step = if let Some(terminal) = terminal {
        terminal
    } else if matches!(
        plan.order,
        ExecutionOrder::ResultChain
            | ExecutionOrder::Dataflow
            | ExecutionOrder::GroupTail
            | ExecutionOrder::GroupCapture
            | ExecutionOrder::ParallelGroupTail
            | ExecutionOrder::ParallelGroupCapture
            | ExecutionOrder::GroupDataflow
            | ExecutionOrder::ParallelGroupDataflow
            | ExecutionOrder::GroupConditional
            | ExecutionOrder::ParallelGroupConditional
    ) && finish_binding
    {
        Step::Done(*fields.pop().ok_or_else(successor::invalid)?.value)
    } else {
        Step::Done(Value::Structured { fields })
    };
    if let Step::Done(value) = &step {
        validate_value(value, 1)?;
    }
    encode_json_capped(&step, MAX_JOURNAL_ENTRY_BYTES, "structured merge output")?;
    if finish_binding
        && !plan.order.has_group_successor()
        && let (Some(binding), Step::Done(value)) = (&plan.result_binding, &step)
    {
        return Ok(binding.finish(value));
    }
    Ok(step)
}
